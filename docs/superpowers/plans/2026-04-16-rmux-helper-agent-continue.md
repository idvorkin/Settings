# rmux_helper agent-continue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rmux_helper agent-continue` and `rmux_helper agent-yolo-continue` subcommands that scan the caller's tmux pane scrollback for the most recent `claude --resume <UUID>` line and re-exec it in place. Extensible to codex via an `AgentDef` registry.

**Architecture:** A new `agent_continue` module hosting the registry, pure `find_resume_target` + `build_exec_argv` core, and a thin wrapper that resolves the caller's pane (reusing `parent-pid-tree`'s `resolve_pane_by_parent_chain`), captures its buffer via a new `TmuxProvider::capture_pane` trait method, and `execvp`s through `$SHELL -ic '<launcher> <args> <id>'`. Mirrors the humble-object split already used for `parent-pid-tree`.

**Tech Stack:** Rust 2021, `clap` subcommands, `regex = "1"`, existing `TmuxProvider` / `ProcReader` traits in `src/main.rs`.

**Reference:** Design doc at `docs/superpowers/specs/2026-04-16-rmux-helper-agent-continue-design.md`.

---

## File Structure

**Create:**
- `rust/tmux_helper/src/agent_continue.rs` — module containing `AgentDef` registry, `FindOutcome`/`Match`, pure `find_resume_target`, pure `build_exec_argv`, `trait PaneCapturer` + `RealPaneCapturer`, `agent_continue_cmd` wrapper, unit tests at bottom.

**Modify:**
- `rust/tmux_helper/src/main.rs`:
  - Add `mod agent_continue;`
  - Add `AgentContinue` / `AgentYoloContinue` variants to `Commands`
  - Dispatch them in `main()`
  - Promote `TmuxProvider`, `ProcReader`, `RealTmuxProvider`, `RealProcReader`, `TmuxError`, `resolve_pane_by_parent_chain` to `pub(crate)`
  - Extend `TmuxProvider` with `capture_pane(pane_id, window) -> Result<String, TmuxError>` (real impl + update existing mock)
  - Add thin helper `pub(crate) fn resolve_caller_pane_id(self_pid, tmux, proc) -> Result<String, i32>` — returns `Ok(pane_id)` or `Err(exit_code)`.
- `rust/tmux_helper/CLAUDE.md` — add `agent-continue` / `agent-yolo-continue` to command list and add a section describing behavior, flags, exit codes.

**Not modified (YAGNI):**
- `shared/.tmux.conf` — no keybinding this round. Igor runs it by name.
- `.cargo` / dep additions — regex crate already present.

---

### Task 1: Scaffold module + command variants (no logic yet)

**Files:**
- Create: `rust/tmux_helper/src/agent_continue.rs`
- Modify: `rust/tmux_helper/src/main.rs:1-5`, `rust/tmux_helper/src/main.rs:39-118`, `rust/tmux_helper/src/main.rs:2985-3000`

- [ ] **Step 1: Create `agent_continue.rs` with placeholder `cmd` fn that returns exit 3**

```rust
//! Scan the caller's tmux pane scrollback for the most recent AI-agent resume
//! command (`claude --resume <UUID>`) and exec it in place. See
//! docs/superpowers/specs/2026-04-16-rmux-helper-agent-continue-design.md.

/// Entry point for `agent-continue` / `agent-yolo-continue`. Returns a process
/// exit code. Callers should `std::process::exit(rv)` with it.
pub(crate) fn cmd(_yolo: bool, _window: usize, _dry_run: bool) -> i32 {
    eprintln!("agent-continue: not yet implemented");
    3
}
```

- [ ] **Step 2: Wire the module into `main.rs`**

At the top of `src/main.rs`, after `mod picker;`, add:

```rust
mod agent_continue;
```

- [ ] **Step 3: Add the two subcommand variants to `Commands` enum**

In `src/main.rs`, inside the `Commands` enum (after the `InstallCompletions` variant, before the closing brace around line 118), add:

```rust
    /// Resume the most recent agent session found in the caller's pane scrollback.
    ///
    /// Scans the last N lines of the owning tmux pane for `claude --resume <UUID>`
    /// (extensible to other agents via the registry in `agent_continue.rs`).
    /// Exactly one match → exec `<launcher> --resume <id>` through `$SHELL -ic`.
    /// Zero matches → exit 1. Multiple distinct matches → exit 2 (refuses to guess).
    AgentContinue {
        /// How many lines of scrollback to scan (default 50).
        #[arg(long, default_value_t = 50)]
        window: usize,
        /// Print the command that would run and exit 0 instead of exec'ing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Same as `agent-continue`, but launches through the permissive wrapper
    /// (`yolo-claude` for claude). Requires a container — the wrapper enforces
    /// this via `_require_container`.
    AgentYoloContinue {
        #[arg(long, default_value_t = 50)]
        window: usize,
        #[arg(long)]
        dry_run: bool,
    },
```

- [ ] **Step 4: Dispatch the variants from `main()`**

In `src/main.rs`, inside the `match cli.command { ... }` block (around line 2985), add two arms before the closing brace:

```rust
        Some(Commands::AgentContinue { window, dry_run }) => {
            std::process::exit(agent_continue::cmd(false, window, dry_run));
        }
        Some(Commands::AgentYoloContinue { window, dry_run }) => {
            std::process::exit(agent_continue::cmd(true, window, dry_run));
        }
```

- [ ] **Step 5: Verify the binary compiles and advertises the new commands**

Run:
```bash
cd rust/tmux_helper
cargo build 2>&1 | tail -5
```
Expected: `Finished ... profile` (no errors).

Run:
```bash
cargo run --quiet -- agent-continue --help 2>&1 | head -20
cargo run --quiet -- agent-yolo-continue --help 2>&1 | head -20
```
Expected: both print `--window <WINDOW>` and `--dry-run` flags.

- [ ] **Step 6: Run all existing tests to confirm no regressions**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok. 234 passed; 0 failed`.

- [ ] **Step 7: Commit**

```bash
git add rust/tmux_helper/src/main.rs rust/tmux_helper/src/agent_continue.rs
git commit -m "agent-continue: scaffold subcommands and module"
```

---

### Task 2: Extend `TmuxProvider` with `capture_pane` + promote visibility

**Files:**
- Modify: `rust/tmux_helper/src/main.rs:1970-2045` (trait + real impl), `rust/tmux_helper/src/main.rs:3871` (mock), plus any `trait X` / `struct X` declarations that need `pub(crate)`.

- [ ] **Step 1: Add failing test for `RealTmuxProvider::capture_pane` argv construction**

There's no pure helper yet — add one. In `src/main.rs`, in the existing `#[cfg(test)] mod tests { ... }` block (search for `mod tests` near end of file), add:

```rust
    #[test]
    fn test_capture_pane_args_shape() {
        let args = capture_pane_args("%12", 75);
        assert_eq!(
            args,
            vec![
                "capture-pane".to_string(),
                "-p".to_string(),
                "-J".to_string(),
                "-S".to_string(),
                "-75".to_string(),
                "-E".to_string(),
                "-".to_string(),
                "-t".to_string(),
                "%12".to_string(),
            ]
        );
    }
```

Run:
```bash
cargo test test_capture_pane_args_shape 2>&1 | tail -5
```
Expected: FAIL — `cannot find function 'capture_pane_args'`.

- [ ] **Step 2: Add the pure helper**

In `src/main.rs`, just above `struct RealTmuxProvider;` (line 2011), add:

```rust
/// Build the argv for `tmux capture-pane -p -J -S -<window> -E - -t <pane_id>`.
/// Pulled out so unit tests can assert the shape without spawning tmux.
pub(crate) fn capture_pane_args(pane_id: &str, window: usize) -> Vec<String> {
    vec![
        "capture-pane".to_string(),
        "-p".to_string(),
        "-J".to_string(),
        "-S".to_string(),
        format!("-{}", window),
        "-E".to_string(),
        "-".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
    ]
}
```

Run:
```bash
cargo test test_capture_pane_args_shape 2>&1 | tail -5
```
Expected: PASS.

- [ ] **Step 3: Extend `TmuxProvider` trait**

In `src/main.rs` around line 1970, add a third method to the `TmuxProvider` trait:

```rust
    /// Capture the recent scrollback of `pane_id` via
    /// `tmux capture-pane -p -J -S -<window> -E -`. Returns the captured text.
    /// Errors propagate as `TmuxError::ListFailed` for spawn/read problems,
    /// `TmuxError::NotRunning` if tmux returns non-zero.
    fn capture_pane(&self, pane_id: &str, window: usize) -> Result<String, TmuxError>;
```

- [ ] **Step 4: Implement `capture_pane` on `RealTmuxProvider`**

Inside `impl TmuxProvider for RealTmuxProvider { ... }` (around line 2013), add:

```rust
    fn capture_pane(&self, pane_id: &str, window: usize) -> Result<String, TmuxError> {
        let args = capture_pane_args(pane_id, window);
        let output = Command::new("tmux")
            .args(args.iter().map(String::as_str))
            .output()
            .map_err(TmuxError::ListFailed)?;
        if !output.status.success() {
            return Err(TmuxError::NotRunning);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
```

- [ ] **Step 5: Update `MockTmuxProvider` so existing tests compile**

Find `impl TmuxProvider for MockTmuxProvider` (around line 3871). Add an empty implementation of `capture_pane` that returns a stored string — or, for the existing parent-pid-tree tests that don't exercise captures, simply:

```rust
        fn capture_pane(&self, _pane_id: &str, _window: usize) -> Result<String, TmuxError> {
            Ok(String::new())
        }
```

- [ ] **Step 6: Promote items needed by `agent_continue.rs` to `pub(crate)`**

In `src/main.rs`, change these declarations (current form → new form):

- `trait TmuxProvider {` → `pub(crate) trait TmuxProvider {`
- `trait ProcReader {` → `pub(crate) trait ProcReader {`
- `struct RealTmuxProvider;` → `pub(crate) struct RealTmuxProvider;`
- `struct RealProcReader;` → `pub(crate) struct RealProcReader;`
- `fn resolve_pane_by_parent_chain<F>(` → `pub(crate) fn resolve_pane_by_parent_chain<F>(`
- Whatever enum `TmuxError` currently is (find with `grep -n "enum TmuxError"`): add `pub(crate)` before `enum`.

- [ ] **Step 7: Run tests — all pass**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok. 235 passed` (the 234 prior + 1 new `test_capture_pane_args_shape`).

- [ ] **Step 8: Commit**

```bash
git add rust/tmux_helper/src/main.rs
git commit -m "agent-continue: extend TmuxProvider with capture_pane, promote visibility"
```

---

### Task 3: Agent registry + core types + `find_resume_target` (TDD)

**Files:**
- Modify: `rust/tmux_helper/src/agent_continue.rs`

- [ ] **Step 1: Add types, registry, and failing tests**

Replace the contents of `rust/tmux_helper/src/agent_continue.rs` with:

```rust
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
        // Simulates a buffer where an earlier chat message quoted the UUID
        // AND the user subsequently ran /status which printed the same UUID.
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
}
```

- [ ] **Step 2: Run the new tests — they should all pass**

```bash
cargo test --lib agent_continue:: 2>&1 | tail -10
```
Expected: `test result: ok. 7 passed; 0 failed`.

- [ ] **Step 3: Run the full suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok. 242 passed` (235 prior + 7 new).

- [ ] **Step 4: Commit**

```bash
git add rust/tmux_helper/src/agent_continue.rs
git commit -m "agent-continue: AgentDef registry and find_resume_target core"
```

---

### Task 4: `build_exec_argv` pure helper (TDD)

**Files:**
- Modify: `rust/tmux_helper/src/agent_continue.rs`

- [ ] **Step 1: Add failing tests at bottom of `tests` module**

At the end of the `mod tests { ... }` block in `agent_continue.rs`, add:

```rust
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
```

- [ ] **Step 2: Run the tests — they should FAIL**

```bash
cargo test --lib agent_continue::tests::argv 2>&1 | tail -10
```
Expected: `error[E0425]: cannot find function 'build_exec_argv'`.

- [ ] **Step 3: Implement `build_exec_argv`**

In `agent_continue.rs`, after the `find_resume_target` function, add:

```rust
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
```

- [ ] **Step 4: Run the tests — they should PASS**

```bash
cargo test --lib agent_continue::tests::argv 2>&1 | tail -5
```
Expected: `test result: ok. 3 passed`.

- [ ] **Step 5: Commit**

```bash
git add rust/tmux_helper/src/agent_continue.rs
git commit -m "agent-continue: build_exec_argv for shell -ic launch"
```

---

### Task 5: `agent_continue_cmd` wrapper + stderr formatting (TDD)

**Files:**
- Modify: `rust/tmux_helper/src/agent_continue.rs`
- Modify: `rust/tmux_helper/src/main.rs` (wire the real `cmd` through to `run_agent_continue`)

- [ ] **Step 1: Add a trait-based outcome helper + formatter with failing tests**

At the top of the `tests` module in `agent_continue.rs`, add this extra test (below the argv tests):

```rust
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
```

- [ ] **Step 2: Run the test — it FAILS**

```bash
cargo test --lib agent_continue::tests::format_ambiguous 2>&1 | tail -5
```
Expected: `cannot find function 'format_ambiguous_stderr'`.

- [ ] **Step 3: Implement the formatter**

In `agent_continue.rs`, after `build_exec_argv`, add:

```rust
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
    for m in matches {
        let _ = writeln!(
            s,
            "  line -{off}: {text}",
            off = m.line_offset_from_bottom,
            text = m.matched_text,
        );
    }
    s.push_str("Run the one you want manually, or narrow with --window <N>.");
    s
}
```

Run:
```bash
cargo test --lib agent_continue::tests::format_ambiguous 2>&1 | tail -5
```
Expected: PASS.

- [ ] **Step 4: Add failing test for the wrapper outcome (dry-run happy path)**

Still in `agent_continue.rs` `tests` module, add:

```rust
    /// Snapshot of everything a wrapper writes out, keyed by exit code.
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
        // exit_code is only meaningful if exec fails. The wrapper still fills
        // exec_argv for the caller to invoke execvp.
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
```

- [ ] **Step 5: Run tests — they FAIL**

```bash
cargo test --lib agent_continue:: 2>&1 | tail -10
```
Expected: compile errors — `cannot find function 'run_agent_continue'`, `type AgentContinueInput`, `type CmdOutcome`.

- [ ] **Step 6: Implement the pure wrapper**

In `agent_continue.rs`, after `format_ambiguous_stderr`, add:

```rust
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
/// do. The caller (below, in `cmd`) is responsible for actually printing and
/// `execvp`ing.
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
                // Quote the cmd-string portion for readability; since we
                // always use single quotes and the command has no single
                // quotes itself (uuid + ascii), this is safe.
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
```

- [ ] **Step 7: Run all agent_continue tests — they PASS**

```bash
cargo test --lib agent_continue:: 2>&1 | tail -10
```
Expected: `test result: ok. 12 passed` (7 prior + 3 argv + 1 format + 5 wrapper tests = actually let me recount: 7 find + 3 argv + 1 format + 5 wrapper = 16 — the test-runner will show the real number, just verify zero failures).

- [ ] **Step 8: Run the full suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok.` with **zero failures**. Count should be prior-total + new tests added here.

- [ ] **Step 9: Commit**

```bash
git add rust/tmux_helper/src/agent_continue.rs
git commit -m "agent-continue: run_agent_continue wrapper and ambiguous formatter"
```

---

### Task 6: Wire the real `cmd` through `TmuxProvider` + `execvp`

**Files:**
- Modify: `rust/tmux_helper/src/agent_continue.rs` (replace the stub `cmd` with the real one)

- [ ] **Step 1: Import real types at the top of `agent_continue.rs`**

Add after the existing `use regex::Regex;`:

```rust
use std::ffi::CString;

use crate::{ProcReader, RealProcReader, RealTmuxProvider, TmuxError, TmuxProvider};
```

*(If any of those items are not yet `pub(crate)`, Task 2 Step 6 covers that.)*

- [ ] **Step 2: Add a pane-resolution helper**

After the `CmdOutcome` struct, add:

```rust
/// Resolve the caller's owning tmux pane id. Returns `Err(exit_code, stderr)`
/// mirroring `parent-pid-tree`'s failure modes: 2 for "tmux not running",
/// 1 for "no pane matches the caller", 3 for "cannot read /proc/self".
fn resolve_caller_pane_id(
    self_pid: u32,
    tmux: &dyn TmuxProvider,
    proc: &dyn ProcReader,
) -> Result<String, (i32, String)> {
    use crate::resolve_pane_by_parent_chain;
    let pane_pids = match tmux.list_pane_pids() {
        Ok(v) => v,
        Err(TmuxError::NotRunning) => {
            return Err((2, "rmux_helper agent-continue: tmux not running or no panes.".to_string()));
        }
        Err(TmuxError::ListFailed(e)) => {
            return Err((3, format!("rmux_helper agent-continue: tmux list-panes failed: {e}")));
        }
    };
    // Walk PPID chain starting at parent of self (i.e. the caller shell).
    let start_pid = match proc.read_ppid(self_pid) {
        Some(p) => p,
        None => {
            return Err((3, "rmux_helper agent-continue: cannot read /proc/self/stat".to_string()));
        }
    };
    let mut read = |pid: u32| proc.read_ppid(pid);
    match resolve_pane_by_parent_chain(start_pid, &pane_pids, &mut read) {
        Some(pane_id) => Ok(pane_id),
        None => Err((
            1,
            format!("rmux_helper agent-continue: no tmux pane found for caller pid {start_pid}"),
        )),
    }
}
```

- [ ] **Step 3: Replace the stub `cmd` with the real one**

Replace the entire existing `pub(crate) fn cmd(...)` near the top with:

```rust
/// Real entry point called from `main.rs`. Returns an exit code; on the Found
/// + non-dry-run path, this function calls `execvp` and does not return
/// (barring an exec failure, which produces exit 3).
pub(crate) fn cmd(yolo: bool, window: usize, dry_run: bool) -> i32 {
    let tmux = RealTmuxProvider;
    let proc = RealProcReader;
    let self_pid = std::process::id();

    let pane_id = match resolve_caller_pane_id(self_pid, &tmux, &proc) {
        Ok(id) => id,
        Err((code, msg)) => {
            eprintln!("{msg}");
            return code;
        }
    };

    let buffer = match tmux.capture_pane(&pane_id, window) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rmux_helper agent-continue: tmux capture-pane failed: {e:?}");
            return 3;
        }
    };

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let outcome = run_agent_continue(AgentContinueInput {
        buffer,
        yolo,
        window,
        dry_run,
        shell,
    });

    if !outcome.stdout.is_empty() {
        println!("{}", outcome.stdout);
    }
    if !outcome.stderr.is_empty() {
        eprintln!("{}", outcome.stderr);
    }

    if let Some(argv) = outcome.exec_argv {
        // execvp — replaces this process. If we return from here, it failed.
        let prog = CString::new(argv[0].as_bytes())
            .expect("shell path must not contain NUL");
        let c_args: Vec<CString> = argv
            .iter()
            .map(|a| CString::new(a.as_bytes()).expect("argv element must not contain NUL"))
            .collect();
        // Build argv pointers + trailing null.
        let mut ptrs: Vec<*const libc::c_char> = c_args.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // Safety: execvp takes a C-string-ptr + argv-array-ptr; CStrings outlive the call.
        unsafe {
            libc::execvp(prog.as_ptr(), ptrs.as_ptr());
        }
        // Only reached on exec failure.
        let err = std::io::Error::last_os_error();
        eprintln!(
            "rmux_helper agent-continue: execvp({}) failed: {err}",
            argv[0]
        );
        return 3;
    }

    outcome.exit_code
}
```

- [ ] **Step 4: Add `libc` dependency**

In `rust/tmux_helper/Cargo.toml`, add to `[dependencies]`:

```toml
libc = "0.2"
```

- [ ] **Step 5: Build**

```bash
cargo build 2>&1 | tail -10
```
Expected: `Finished`. Resolve any missing `pub(crate)` promotions (Task 2 Step 6) if the build complains about private items.

- [ ] **Step 6: Run all tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all prior tests still pass. The new `cmd` function is not unit-tested directly (it does real I/O and exec); its pure core is already covered.

- [ ] **Step 7: Commit**

```bash
git add rust/tmux_helper/src/agent_continue.rs rust/tmux_helper/Cargo.toml rust/tmux_helper/Cargo.lock
git commit -m "agent-continue: real cmd wrapper with execvp launch"
```

---

### Task 7: Documentation — update `rmux_helper` CLAUDE.md

**Files:**
- Modify: `rust/tmux_helper/CLAUDE.md`

- [ ] **Step 1: Add the commands to the command list**

In `rust/tmux_helper/CLAUDE.md`, find the `## Commands` section and append two bullets:

```markdown
- `agent-continue` - Scan the caller's pane for `claude --resume <UUID>` and exec it in place. See below.
- `agent-yolo-continue` - Same, but launches through `yolo-claude` (container only).
```

- [ ] **Step 2: Add a dedicated section below `parent-pid-tree`**

After the `## parent-pid-tree` section, add:

````markdown
## `agent-continue` / `agent-yolo-continue`

Resume the most recent AI-agent session found in the caller's tmux pane
scrollback. Scans the last 50 lines (default, override with `--window <N>`)
for any registered agent's resume syntax (today: `claude --resume <UUID>`).

Typical invocations:

- `rmux_helper agent-continue` — execs `claude --resume <id>` in the current pane via `$SHELL -ic`. Process is replaced; never returns on success.
- `rmux_helper agent-yolo-continue` — same, but launches through `yolo-claude` (the zsh wrapper that sets `--dangerously-skip-permissions`; container-only via `_require_container`).
- `rmux_helper agent-continue --window 120` — widen the scan if the resume line is further back.
- `rmux_helper agent-continue --dry-run` — print `would exec: <shell> -ic '<cmd>'` and exit 0 without exec'ing. Useful for sanity checks.

**Exit codes**:

- `0` — success on the `--dry-run` happy path (on the real path, `execvp` replaces the process so this code is not observed)
- `1` — no resume command found in the window
- `2` — 2+ distinct `(agent, id)` matches in the window; stderr lists them with line offsets
- `3` — infrastructure failure (no tmux, `/proc/self` unreadable, `execvp` returned)

**Why this exists**: reduces the resume-flow to zero arguments — after a Claude session exits, the user types `rmux_helper agent-continue` (or a shell alias) and is back inside the session, without hand-copying the UUID. The command refuses to guess when the scan finds multiple distinct sessions, so a stale scrollback cannot silently reconnect to the wrong session.

**Extending to other agents**: add a struct literal to `const AGENTS` in `src/agent_continue.rs`. Fields are `name`, `resume_regex` (first capture = session id), `launcher`, `yolo_launcher`, and `resume_args`. Nothing else needs to change.

**Implementation**: see `src/agent_continue.rs`. Humble-object split, following the same pattern as `parent-pid-tree`:

- **Humble shell** — `TmuxProvider::capture_pane` (in `main.rs`) + `resolve_caller_pane_id` + the real `execvp` call.
- **Testable core** — `find_resume_target(buffer, agents)` → `FindOutcome`, `build_exec_argv(shell, launcher, args, id)`, `format_ambiguous_stderr(matches, window)`, and `run_agent_continue(input)` → `CmdOutcome` — all pure, all unit-tested without tmux or `/proc`.
- **Command wrapper** — `cmd(yolo, window, dry_run)` in the same file.
````

- [ ] **Step 3: Commit**

```bash
git add rust/tmux_helper/CLAUDE.md
git commit -m "docs: document agent-continue and agent-yolo-continue"
```

---

### Task 8: Smoke test + install

**Files:** none modified; live validation only.

- [ ] **Step 1: Install the new binary into `~/.cargo/bin`**

```bash
cd rust/tmux_helper
cargo install --path . --force 2>&1 | tail -5
```
Expected: `Installed package 'rmux_helper v0.1.0'`.

- [ ] **Step 2: Confirm `--help` output**

```bash
rmux_helper agent-continue --help 2>&1
rmux_helper agent-yolo-continue --help 2>&1
```
Expected: both show `--window <WINDOW>` (default 50) and `--dry-run`.

- [ ] **Step 3: Dry-run against a synthetic buffer (outside tmux)**

This is a negative test — confirms the tool fails gracefully when not in a tmux pane:

```bash
env -u TMUX rmux_helper agent-continue --dry-run 2>&1
echo "exit=$?"
```
Expected: exit code 2 (tmux not running) or 1 (no match) with a helpful stderr line starting `rmux_helper agent-continue:`.

- [ ] **Step 4: Dry-run inside a tmux pane whose scrollback contains a `claude --resume <UUID>` line**

Igor will run this himself in a live pane that has recently-run `claude --resume`. Report:

```bash
rmux_helper agent-continue --dry-run
```

Expected output (single line to stdout):
```
would exec: /bin/zsh -ic 'claude --resume <the-uuid>'
```

If stdout says "would exec:" with the correct UUID → smoke test passes.

If exit code 2 with "refusing to guess" → the scrollback has multiple distinct UUIDs visible. Confirm via `--window 20` or manually running the preferred UUID.

If exit code 1 "no agent resume command found" → widen with `--window 200` or confirm the `/status` output actually printed the UUID.

- [ ] **Step 5: Report results to Igor**

Post the live stdout/exit code. If happy-path is green, proceed to push; if not, debug via the `systematic-debugging` skill.

---

### Task 9: Final branch integration

**Files:** none.

- [ ] **Step 1: Confirm the branch has a clean diff against `main`**

```bash
git fetch origin main
git log --oneline origin/main..HEAD
```
Expected: 6 or so commits (spec + 5-7 implementation commits).

- [ ] **Step 2: Run all tests one more time**

```bash
cd rust/tmux_helper && cargo test 2>&1 | tail -5
```
Expected: `test result: ok.` with zero failures.

- [ ] **Step 3: Hand off to `superpowers:finishing-a-development-branch`**

Igor's rule is "no direct pushes to main". Options the skill presents: push feature branch + open PR, or hold locally. Default recommendation: push to fork (`idvorkin-ai-tools` remote if configured) and open PR with link to spec and plan.

---

## Self-Review Checklist

- [x] Spec coverage: every section of `2026-04-16-rmux-helper-agent-continue-design.md` maps to a task.
  - Commands → Tasks 1, 6
  - Exit codes → Tasks 5, 6
  - Agent registry → Task 3
  - Caller pane resolution → Task 6 (`resolve_caller_pane_id`)
  - Buffer capture → Task 2 (`capture_pane_args` + `TmuxProvider::capture_pane`)
  - Scan + outcomes (Found / NotFound / Ambiguous) → Tasks 3, 5
  - Launch model (`$SHELL -ic`) → Tasks 4, 6
  - Flags (`--window`, `--dry-run`) → Tasks 1, 5
  - Humble-object layout → Tasks 2, 3, 5, 6
  - Tests (14 unit tests called out in spec) → Tasks 2, 3, 4, 5
- [x] Placeholder scan: no TBDs / TODOs / "similar to Task N" / abstract "add error handling".
- [x] Type consistency: `AgentDef` field names, `FindOutcome` variants, `Match` fields, `CmdOutcome` shape all match across tasks.
- [x] Every code step shows the actual code. Every test step shows the actual test.
- [x] Every command step shows the expected output.
