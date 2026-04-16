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

/// Pure formatter for the exit-code-2 stderr block. Kept pure so its exact
/// wording (which shell aliases may grep against) is covered by unit tests.
pub(crate) fn format_ambiguous_stderr(matches: &[Match], window: usize) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "rmux_helper agent-continue: found {n} resume targets in last {window} lines — refusing to guess.",
        n = matches.len(),
        window = window,
    );
    let width = matches
        .iter()
        .map(|m| m.line_offset_from_bottom.to_string().len())
        .max()
        .unwrap_or(1);
    for m in matches {
        // Format as "line -<off>:" left-padded to align colons, then the text.
        // e.g. width=2: "line -3:  text" and "line -41: text"
        let tag = format!("line -{}:", m.line_offset_from_bottom);
        let tag_width = "line -:".len() + width; // "line -" + digits + ":"
        let _ = writeln!(s, "  {tag:<tag_width$} {text}", text = m.matched_text);
    }
    s.push_str("Run the one you want manually, or narrow with --window <N>.");
    s
}

/// Input to the pure wrapper. All I/O has already happened by the time we get
/// here — `buffer` is the captured pane text, `shell` is `$SHELL`.
pub(crate) struct AgentContinueInput {
    pub buffer: String,
    pub yolo: bool,
    pub window: usize,
    pub dry_run: bool,
    pub shell: String,
}

/// What the wrapper decided: what to print to stdout/stderr, what exit code
/// to use, and (on the Found-non-dry-run path) the argv to `execvp`.
#[derive(Debug, Default)]
pub(crate) struct CmdOutcome {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub exec_argv: Option<Vec<String>>,
}

/// Pure wrapper: given a captured buffer and flags, decide what the CLI should
/// do. The caller is responsible for actually printing and `execvp`ing.
pub(crate) fn run_agent_continue(input: AgentContinueInput) -> CmdOutcome {
    let mut out = CmdOutcome::default();
    match find_resume_target(&input.buffer, AGENTS) {
        FindOutcome::NotFound => {
            out.exit_code = 1;
            out.stderr = format!(
                "rmux_helper agent-continue: no agent resume command found in last {n} lines. Try --window <N> to widen.",
                n = input.window,
            );
        }
        FindOutcome::Ambiguous(matches) => {
            out.exit_code = 2;
            out.stderr = format_ambiguous_stderr(&matches, input.window);
        }
        FindOutcome::Found { agent_name, id } => {
            let agent = AGENTS
                .iter()
                .find(|a| a.name == agent_name)
                .expect("AGENTS must contain matched agent");
            let launcher = if input.yolo {
                agent.yolo_launcher
            } else {
                agent.launcher
            };
            let argv = build_exec_argv(&input.shell, launcher, agent.resume_args, &id);
            if input.dry_run {
                out.exit_code = 0;
                out.stdout = format!("would exec: {shell} -ic '{cmd}'", shell = input.shell, cmd = argv[2]);
            } else {
                out.exec_argv = Some(argv);
                // exit_code is irrelevant if exec succeeds; set to 3 as the
                // "exec returned" fallback value (caller may overwrite on
                // actual exec failure).
                out.exit_code = 3;
            }
        }
    }
    out
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

    #[test]
    fn format_ambiguous_stderr_lists_newest_first() {
        let ms = vec![
            Match {
                line_offset_from_bottom: 3,
                agent_name: "claude",
                id: UUID_A.to_string(),
                matched_text: format!("claude --resume {UUID_A}"),
            },
            Match {
                line_offset_from_bottom: 41,
                agent_name: "claude",
                id: UUID_B.to_string(),
                matched_text: format!("claude --resume {UUID_B}"),
            },
        ];
        let out = format_ambiguous_stderr(&ms, 50);
        assert!(
            out.contains("found 2 resume targets in last 50 lines"),
            "got: {out}"
        );
        assert!(
            out.contains(&format!("line -3:  claude --resume {UUID_A}")),
            "got: {out}"
        );
        assert!(
            out.contains(&format!("line -41: claude --resume {UUID_B}")),
            "got: {out}"
        );
        assert!(
            out.contains("Run the one you want manually, or narrow with --window <N>."),
            "got: {out}"
        );
    }

    fn run_to_outcome(
        buffer: &str,
        yolo: bool,
        window: usize,
        dry_run: bool,
        shell: &str,
    ) -> CmdOutcome {
        run_agent_continue(AgentContinueInput {
            buffer: buffer.to_string(),
            yolo,
            window,
            dry_run,
            shell: shell.to_string(),
        })
    }

    #[test]
    fn dry_run_found_prints_would_exec_and_returns_zero() {
        let out = run_to_outcome(
            &format!("noise\nclaude --resume {UUID_A}\n$"),
            false,
            50,
            true,
            "/bin/zsh",
        );
        assert_eq!(out.exit_code, 0);
        let stdout = out.stdout.trim_end();
        assert_eq!(
            stdout,
            format!("would exec: /bin/zsh -ic 'claude --resume {UUID_A}'")
        );
        assert!(out.exec_argv.is_none(), "dry-run must not request exec");
    }

    #[test]
    fn dry_run_yolo_swaps_launcher() {
        let out = run_to_outcome(
            &format!("claude --resume {UUID_A}\n$"),
            true,
            50,
            true,
            "/bin/zsh",
        );
        assert_eq!(out.exit_code, 0);
        assert!(
            out.stdout.contains("yolo-claude --resume"),
            "stdout was: {}",
            out.stdout
        );
    }

    #[test]
    fn not_found_exits_one_with_helpful_stderr() {
        let out = run_to_outcome("just a prompt\n$ _", false, 50, false, "/bin/zsh");
        assert_eq!(out.exit_code, 1);
        assert!(
            out.stderr.contains("no agent resume command found"),
            "stderr: {}",
            out.stderr
        );
        assert!(out.stderr.contains("--window"), "stderr: {}", out.stderr);
        assert!(out.exec_argv.is_none());
    }

    #[test]
    fn ambiguous_exits_two_and_never_requests_exec() {
        let buf = format!(
            "claude --resume {UUID_B}\n\
             ... 40 lines of noise ...\n\
             claude --resume {UUID_A}\n$ _"
        );
        let out = run_to_outcome(&buf, false, 50, false, "/bin/zsh");
        assert_eq!(out.exit_code, 2);
        assert!(
            out.stderr.contains("refusing to guess"),
            "stderr: {}",
            out.stderr
        );
        assert!(out.exec_argv.is_none());
    }

    #[test]
    fn found_non_dry_run_requests_exec_with_correct_argv() {
        let out = run_to_outcome(
            &format!("claude --resume {UUID_A}\n$"),
            false,
            50,
            false,
            "/bin/zsh",
        );
        let argv = out.exec_argv.expect("should request exec on Found");
        assert_eq!(
            argv,
            vec![
                "/bin/zsh".to_string(),
                "-ic".to_string(),
                format!("claude --resume {UUID_A}"),
            ]
        );
    }
}
