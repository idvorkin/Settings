//! Commands an agent asked the human to run, pulled from scrollback.
//!
//! Heuristics only (no model call), tuned to how Claude Code renders into a
//! pane: backticks are stripped from inline code, fenced blocks lose their
//! fences, and the agent's own tool calls show as `⎿  $ cmd`. Detected:
//!
//! 1. A line starting with `! cmd` or `$ cmd` (after gutter glyphs, list
//!    markers, and wrapping backticks).
//! 2. `` `! cmd` `` anywhere in a line — agents that keep backticks.
//! 3. The lines right after a cue line such as "Run this:" or "paste this
//!    into the prompt:" — how a fenced block reads once its fences are gone.
//!
//! Every candidate must look like a command: its first word (past `VAR=x`
//! assignments and `sudo`) is a path, a shell builtin, or an executable on
//! `PATH`. That is what keeps prose after a cue line out of the list.

use super::detect::{Category, Item};
use regex::Regex;
use std::sync::OnceLock;

/// Leading glyphs Claude Code draws in its gutter: response bullet, prompt
/// echo, quote bar, box edge. `⎿` is deliberately absent — it marks the
/// agent's own tool calls and output, which are not asks of the human.
const GUTTER: &[char] = &['⏺', '❯', '▎', '│', '>'];

const CUE_WORDS: &[&str] = &["run", "paste", "type", "execute", "enter"];

/// Max lines taken from one cue's block. Keeps a missed block end from
/// swallowing a screen of text.
const MAX_BLOCK_LINES: usize = 8;

/// Max hard-wrapped (deeper-indent) lines joined onto one command.
/// Trailing-`\` continuations are bounded by `MAX_BLOCK_LINES` instead.
const MAX_CONTINUATIONS: usize = 3;

const BUILTINS: &[&str] = &[
    ".", "alias", "cd", "eval", "exec", "export", "popd", "pushd", "set", "source", "unset",
];

/// Find commands in `lines`. `is_exe` answers "is this word on PATH?" —
/// injected so tests don't depend on the machine.
pub(crate) fn find(lines: &[&str], is_exe: &dyn Fn(&str) -> bool) -> Vec<Item> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim_start().starts_with('⎿') {
            i += 1;
            continue;
        }
        if let Some((marker, cmd)) = marked_command(line) {
            let (cmd, used) = join_continuations(cmd, line, &lines[i + 1..]);
            push(&mut out, &cmd, marker, i, is_exe);
            i += 1 + used;
            continue;
        }
        for cap in inline_regex().captures_iter(line) {
            push(&mut out, cap[1].trim(), "!", i, is_exe);
        }
        if is_cue(line) {
            i += 1 + take_block(lines, i + 1, is_exe, &mut out);
            continue;
        }
        i += 1;
    }
    out
}

/// Real `is_exe`: `word` is a file in some `PATH` directory.
/// ponytail: ignores the executable bit and shell aliases/functions; a
/// false positive only means one extra row after a cue line.
/// The name must match exactly: macOS filesystems are case-insensitive, so
/// `What` would otherwise hit `/usr/bin/what`.
pub(crate) fn on_path(word: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| in_dirs(word, &path))
}

fn in_dirs(word: &str, path: &std::ffi::OsStr) -> bool {
    std::env::split_paths(path).any(|dir| {
        dir.join(word).is_file()
            && std::fs::read_dir(&dir)
                .is_ok_and(|entries| entries.flatten().any(|e| e.file_name() == word))
    })
}

fn inline_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`!\s*([^`]+)`").unwrap())
}

/// Strip gutter glyphs, list markers, and whitespace from the front.
fn strip_prefix(line: &str) -> &str {
    let mut s = line.trim_start_matches(|c: char| c.is_whitespace() || GUTTER.contains(&c));
    for bullet in ["- ", "* ", "• "] {
        if let Some(rest) = s.strip_prefix(bullet) {
            s = rest;
        }
    }
    // Numbered list item: `1. cmd`
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        if let Some(rest) = s[digits..].strip_prefix(". ") {
            s = rest;
        }
    }
    s.trim_start()
}

/// `! cmd` / `$ cmd` at the start of the line → (marker, cmd).
fn marked_command(line: &str) -> Option<(&'static str, &str)> {
    let s = strip_prefix(line);
    let s = s
        .strip_prefix('`')
        .map_or(s, |inner| inner.strip_suffix('`').unwrap_or(inner));
    for marker in ["!", "$"] {
        if let Some(rest) = s.strip_prefix(marker) {
            if rest.starts_with(' ') && !rest.trim().is_empty() {
                return Some((marker, rest.trim()));
            }
        }
    }
    None
}

/// Claude Code hard-wraps long lines and indents the continuation deeper
/// than the first line. Join those back, plus shell `\` continuations.
/// Returns the joined command and how many following lines it consumed.
/// ponytail: a deeper-indented line that is really a separate item would
/// get glued on; fine for agent output, revisit if false joins show up.
fn join_continuations(first: &str, first_line: &str, rest: &[&str]) -> (String, usize) {
    let base = indent(first_line);
    let mut cmd = first.to_string();
    let mut used = 0;
    let mut wrapped = 0;
    for next in rest.iter().take(MAX_BLOCK_LINES) {
        if next.trim_start().starts_with(['⎿', '⏺', '❯']) || next.trim().is_empty() {
            break;
        }
        if cmd.ends_with('\\') {
            cmd.pop();
        } else if indent(next) > base && wrapped < MAX_CONTINUATIONS {
            wrapped += 1;
        } else {
            break;
        }
        cmd = format!("{} {}", cmd.trim_end(), next.trim());
        used += 1;
    }
    (cmd, used)
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// "Run this:", "paste this into the prompt:", "Export before running X:".
fn is_cue(line: &str) -> bool {
    let s = line.trim();
    if !s.ends_with(':') {
        return false;
    }
    let lower = s.to_lowercase();
    CUE_WORDS.iter().any(|w| lower.contains(w))
}

/// Take command lines after a cue at `start`, allowing one blank line
/// before the block. Stops at a blank line or the first non-command.
/// Returns the number of lines consumed.
fn take_block(
    lines: &[&str],
    start: usize,
    is_exe: &dyn Fn(&str) -> bool,
    out: &mut Vec<Item>,
) -> usize {
    let mut i = start;
    if lines.get(i).is_some_and(|l| l.trim().is_empty()) {
        i += 1;
    }
    let mut taken = 0;
    while i < lines.len() && taken < MAX_BLOCK_LINES {
        let line = lines[i];
        if line.trim().is_empty() || line.trim_start().starts_with('⎿') {
            break;
        }
        let (marker, cmd) = marked_command(line).unwrap_or(("run", strip_prefix(line)));
        let (cmd, used) = join_continuations(cmd, line, &lines[i + 1..]);
        if !push(out, &cmd, marker, i, is_exe) {
            break;
        }
        i += 1 + used;
        taken += 1;
    }
    i - start
}

/// Push `cmd` if it looks like a command. Returns whether it did.
fn push(
    out: &mut Vec<Item>,
    cmd: &str,
    marker: &str,
    line_index: usize,
    is_exe: &dyn Fn(&str) -> bool,
) -> bool {
    let cmd = cmd.trim();
    if !looks_like_command(cmd, is_exe) {
        return false;
    }
    out.push(Item {
        category: Category::Command,
        canonical: cmd.to_string(),
        key: marker.to_string(),
        repo_or_host: "—".to_string(),
        line_index,
    });
    true
}

fn looks_like_command(cmd: &str, is_exe: &dyn Fn(&str) -> bool) -> bool {
    let mut words = cmd
        .split_whitespace()
        .skip_while(|w| is_assignment(w) || *w == "sudo");
    let Some(first) = words.next() else {
        return false;
    };
    first.starts_with(['/', '.', '~']) || BUILTINS.contains(&first) || is_exe(first)
}

/// `FOO=bar` — an env assignment prefix, not the command.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(w: &str) -> bool {
        ["bash", "bd", "git", "cargo", "just", "ls"].contains(&w)
    }

    fn cmds(raw: &str) -> Vec<String> {
        let lines: Vec<&str> = raw.lines().collect();
        find(&lines, &exe)
            .into_iter()
            .map(|i| i.canonical)
            .collect()
    }

    #[test]
    fn bang_line_in_claude_output() {
        let raw = "⏺ Run it yourself by typing this in the prompt:\n\n  ! bd show settings-vlw\n\n  The output will show up here.";
        assert_eq!(cmds(raw), vec!["bd show settings-vlw"]);
    }

    #[test]
    fn bang_marker_is_the_key() {
        let lines = ["  ! bash ~/gits/settings/apply.sh"];
        let items = find(&lines, &exe);
        assert_eq!(items[0].key, "!");
        assert_eq!(items[0].category, Category::Command);
        assert_eq!(items[0].line_index, 0);
    }

    #[test]
    fn dollar_line_is_a_command() {
        assert_eq!(cmds("  $ just test"), vec!["just test"]);
    }

    #[test]
    fn tool_call_lines_are_not_human_asks() {
        let raw = "⏺ Reading the scrollback · 3s\n  ⎿  $ git status\n     --short";
        assert!(cmds(raw).is_empty());
    }

    #[test]
    fn inline_backticked_bang_anywhere_in_prose() {
        let raw = "Next, run `! git pull --rebase` and then `! just test` please.";
        assert_eq!(cmds(raw), vec!["git pull --rebase", "just test"]);
    }

    #[test]
    fn line_wrapped_in_backticks() {
        assert_eq!(cmds("`! cargo build`"), vec!["cargo build"]);
    }

    #[test]
    fn cue_block_takes_command_lines_and_stops_at_prose() {
        let raw = "Export before running iOS native commands:\n\n  export PATH=\"/opt/x:$PATH\"\n  cargo build\n  Then it works.";
        assert_eq!(
            cmds(raw),
            vec!["export PATH=\"/opt/x:$PATH\"", "cargo build"]
        );
    }

    #[test]
    fn cue_block_stops_at_blank_line() {
        let raw = "Run this:\n  git fetch\n\n  ls";
        assert_eq!(cmds(raw), vec!["git fetch"]);
    }

    #[test]
    fn prose_after_cue_is_not_a_command() {
        let raw = "Here's what to run:\n  The fix updates the hashes.";
        assert!(cmds(raw).is_empty());
    }

    #[test]
    fn colon_line_without_cue_word_takes_nothing() {
        let raw = "Other items:\n  git is fine";
        assert!(cmds(raw).is_empty());
    }

    #[test]
    fn prose_starting_with_bang_is_rejected() {
        assert!(cmds("! Important: read this first").is_empty());
    }

    #[test]
    fn path_and_builtin_and_env_prefix_count_as_commands() {
        assert_eq!(cmds("$ ./apply.sh"), vec!["./apply.sh"]);
        assert_eq!(cmds("$ cd ~/gits"), vec!["cd ~/gits"]);
        assert_eq!(cmds("$ FOO=1 sudo just up"), vec!["FOO=1 sudo just up"]);
    }

    #[test]
    fn hard_wrapped_continuation_is_joined() {
        let raw =
            "  ! bash ~/gits/settings/very/long/path/apply.sh --one\n     --two\n  Next paragraph.";
        assert_eq!(
            cmds(raw),
            vec!["bash ~/gits/settings/very/long/path/apply.sh --one --two"]
        );
    }

    #[test]
    fn backslash_continuation_is_joined() {
        let raw = "Run:\n  cargo install \\\n  --path .";
        assert_eq!(cmds(raw), vec!["cargo install --path ."]);
    }

    #[test]
    fn list_markers_are_stripped() {
        assert_eq!(
            cmds("  - $ just test\n  2. ! git pull"),
            vec!["just test", "git pull"]
        );
    }

    #[test]
    fn prompt_echo_of_a_bang_command_counts() {
        // The user's own `! cmd` echoed at Claude Code's prompt — worth re-running.
        assert_eq!(cmds("❯ ! bd ready"), vec!["bd ready"]);
    }

    #[test]
    fn prompt_echo_output_is_not_joined() {
        let raw = "❯ ! bd ready
  ⎿  ○ settings-abc open
     ○ settings-def open";
        assert_eq!(cmds(raw), vec!["bd ready"]);
    }

    #[test]
    fn long_backslash_continuation_is_joined_whole() {
        let raw = "Run this:\n  cargo run \\\n  -v a:/a \\\n  -e B=1 \\\n  -p 80:80 \\\n  image";
        assert_eq!(cmds(raw), vec!["cargo run -v a:/a -e B=1 -p 80:80 image"]);
    }

    #[test]
    fn wrapped_redirect_stays_in_the_command() {
        let raw = "  ! ls --flag\n     > /tmp/out.txt";
        assert_eq!(cmds(raw), vec!["ls --flag > /tmp/out.txt"]);
    }

    #[test]
    fn continuations_are_capped() {
        let raw = "! ls
  a
  b
  c
  d
  e";
        assert_eq!(cmds(raw), vec!["ls a b c"]);
    }

    #[test]
    fn on_path_requires_exact_case() {
        let dir = std::env::temp_dir().join(format!("commands-on-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("what"), "").unwrap();
        let path = dir.clone().into_os_string();
        assert!(in_dirs("what", &path));
        assert!(!in_dirs("What", &path));
        let lines = [
            "Run this:",
            "  what -h",
            "  What this does: refreshes the db",
        ];
        let found: Vec<String> = find(&lines, &|w| in_dirs(w, &path))
            .into_iter()
            .map(|i| i.canonical)
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found, vec!["what -h"]);
    }

    #[test]
    fn cue_block_is_capped() {
        let body: String = (0..20).map(|n| format!("  ls {n}\n")).collect();
        let raw = format!("Run these:\n{body}");
        assert_eq!(cmds(&raw).len(), MAX_BLOCK_LINES);
    }
}
