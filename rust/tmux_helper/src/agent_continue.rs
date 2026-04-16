//! Scan the caller's tmux pane scrollback for the most recent AI-agent resume
//! command (`claude --resume <UUID>`) and exec it in place. See
//! docs/superpowers/specs/2026-04-16-rmux-helper-agent-continue-design.md.

use regex::Regex;

/// A registered agent whose resume syntax we can recognize and re-launch.
#[derive(Debug)]
pub(crate) struct AgentDef {
    /// Display name used in error messages.
    pub name: &'static str,
    /// Regex matched against each buffer line. First capture group MUST be
    /// the session id.
    pub resume_regex: &'static str,
    /// Binary or shell function to exec for normal resume.
    pub launcher: &'static str,
    /// Binary or shell function to exec for yolo resume.
    pub yolo_launcher: &'static str,
    /// Args that precede the session id in the resume command, e.g. `["--resume"]`.
    pub resume_args: &'static [&'static str],
}

pub(crate) const AGENTS: &[AgentDef] = &[AgentDef {
    name: "claude",
    resume_regex: r"\bclaude\s+--resume\s+([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b",
    launcher: "claude",
    yolo_launcher: "yolo-claude",
    resume_args: &["--resume"],
}];

/// A single resume-command match found in a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    /// 1-indexed offset from the bottom of the buffer (1 = last line).
    pub line_offset_from_bottom: usize,
    pub agent_name: &'static str,
    pub id: String,
    pub matched_text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FindOutcome {
    NotFound,
    Found { agent_name: &'static str, id: String },
    /// 2+ distinct `(agent_name, id)` tuples, sorted newest-first.
    Ambiguous(Vec<Match>),
}

/// Pure core: scan `buffer` (as captured by tmux, top-to-bottom) for any
/// `AgentDef.resume_regex`. De-duplicate by `(agent_name, id)` — the same
/// session appearing on multiple lines counts once. Returns the newest (closest
/// to bottom) match when exactly one distinct tuple remains.
pub(crate) fn find_resume_target(buffer: &str, agents: &[AgentDef]) -> FindOutcome {
    let lines: Vec<&str> = buffer.lines().collect();
    let total = lines.len();
    let mut matches: Vec<Match> = Vec::new();

    let compiled: Vec<(&AgentDef, Regex)> = agents
        .iter()
        .map(|a| {
            let re = Regex::new(a.resume_regex).expect("AgentDef.resume_regex must compile");
            (a, re)
        })
        .collect();

    for (idx, line) in lines.iter().enumerate() {
        for (agent, re) in &compiled {
            if let Some(caps) = re.captures(line) {
                let id = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let matched_text = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
                matches.push(Match {
                    line_offset_from_bottom: total - idx,
                    agent_name: agent.name,
                    id,
                    matched_text,
                });
            }
        }
    }

    if matches.is_empty() {
        return FindOutcome::NotFound;
    }

    // De-dup by (agent_name, id), keeping newest (smallest line_offset_from_bottom).
    matches.sort_by_key(|m| m.line_offset_from_bottom);
    let mut seen: Vec<(&'static str, String)> = Vec::new();
    let mut unique: Vec<Match> = Vec::new();
    for m in matches {
        let key = (m.agent_name, m.id.clone());
        if !seen.iter().any(|s| s == &key) {
            seen.push(key);
            unique.push(m);
        }
    }

    if unique.len() == 1 {
        let m = unique.into_iter().next().unwrap();
        FindOutcome::Found { agent_name: m.agent_name, id: m.id }
    } else {
        FindOutcome::Ambiguous(unique)
    }
}

/// Pure argv builder for `execvp($SHELL, ...)`. Produces:
/// `[shell, "-ic", "<launcher> <resume_args...> <id>"]`.
///
/// The `$SHELL -ic` indirection is required because `yolo-claude` is a zsh
/// function, not a binary on PATH — `execvp("yolo-claude", ...)` would ENOENT.
pub(crate) fn build_exec_argv(
    shell: &str,
    launcher: &str,
    resume_args: &[&str],
    id: &str,
) -> Vec<String> {
    let mut cmd = String::from(launcher);
    for a in resume_args {
        cmd.push(' ');
        cmd.push_str(a);
    }
    cmd.push(' ');
    cmd.push_str(id);
    vec![shell.to_string(), "-ic".to_string(), cmd]
}

/// Entry point for `agent-continue` / `agent-yolo-continue`. Returns a process
/// exit code. Callers should `std::process::exit(rv)` with it.
pub(crate) fn cmd(_yolo: bool, _window: usize, _dry_run: bool) -> i32 {
    eprintln!("agent-continue: not yet implemented");
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID_A: &str = "1c37051f-212c-41d4-a1d8-9382706fbfa9";
    const UUID_B: &str = "a82e1ff4-6b4d-4e2e-9f0b-3a7d1e5c8a42";

    #[test]
    fn empty_buffer_not_found() {
        assert_eq!(find_resume_target("", AGENTS), FindOutcome::NotFound);
    }

    #[test]
    fn one_claude_resume_is_found() {
        let buf = format!("some noise\nclaude --resume {UUID_A}\n$ _");
        match find_resume_target(&buf, AGENTS) {
            FindOutcome::Found { agent_name, id } => {
                assert_eq!(agent_name, "claude");
                assert_eq!(id, UUID_A);
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_same_id_is_deduped_to_found() {
        let buf = format!(
            "claude --resume {UUID_A}\nnoise\nclaude --resume {UUID_A}\n$ _"
        );
        match find_resume_target(&buf, AGENTS) {
            FindOutcome::Found { id, .. } => assert_eq!(id, UUID_A),
            other => panic!("expected Found (deduped), got {other:?}"),
        }
    }

    #[test]
    fn two_distinct_ids_are_ambiguous_newest_first() {
        let buf = format!(
            "claude --resume {UUID_B}\nnoise\nclaude --resume {UUID_A}\n$ _"
        );
        let out = find_resume_target(&buf, AGENTS);
        let ms = match out {
            FindOutcome::Ambiguous(ms) => ms,
            other => panic!("expected Ambiguous, got {other:?}"),
        };
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].id, UUID_A, "newest (UUID_A) should come first");
        assert_eq!(ms[1].id, UUID_B);
    }

    #[test]
    fn non_uuid_text_after_resume_is_not_found() {
        let buf = "claude --resume not-a-uuid-at-all\n$ _";
        assert_eq!(find_resume_target(buf, AGENTS), FindOutcome::NotFound);
    }

    #[test]
    fn chat_quote_plus_real_status_same_id_dedupes() {
        let buf = format!(
            "assistant: I will run `claude --resume {UUID_A}` when done.\n\
             ... many lines ...\n\
             claude --resume {UUID_A}\n$ _"
        );
        match find_resume_target(&buf, AGENTS) {
            FindOutcome::Found { id, .. } => assert_eq!(id, UUID_A),
            other => panic!("expected Found (deduped), got {other:?}"),
        }
    }

    #[test]
    fn chat_quote_plus_real_status_different_id_is_ambiguous() {
        let buf = format!(
            "assistant: I will run `claude --resume {UUID_B}` when done.\n\
             ... many lines ...\n\
             claude --resume {UUID_A}\n$ _"
        );
        match find_resume_target(&buf, AGENTS) {
            FindOutcome::Ambiguous(ms) => {
                assert_eq!(ms.len(), 2);
                assert_eq!(ms[0].id, UUID_A); // newest first
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn argv_non_yolo_is_shell_ic_claude_resume_id() {
        let argv = build_exec_argv("/bin/zsh", "claude", &["--resume"], UUID_A);
        assert_eq!(
            argv,
            vec![
                "/bin/zsh".to_string(),
                "-ic".to_string(),
                format!("claude --resume {UUID_A}"),
            ]
        );
    }

    #[test]
    fn argv_yolo_uses_yolo_launcher() {
        let argv = build_exec_argv("/bin/zsh", "yolo-claude", &["--resume"], UUID_A);
        assert_eq!(argv[2], format!("yolo-claude --resume {UUID_A}"));
    }

    #[test]
    fn argv_supports_multi_arg_resume_args() {
        let argv = build_exec_argv("/bin/zsh", "foo", &["resume", "--id"], UUID_A);
        assert_eq!(argv[2], format!("foo resume --id {UUID_A}"));
    }
}
