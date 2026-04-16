# rmux_helper `agent-continue` / `agent-yolo-continue` — design

**Status:** design approved, pending implementation plan
**Date:** 2026-04-16
**Scope:** `rust/tmux_helper/`

## Problem

When an AI coding agent (today: `claude`; tomorrow: `codex`, others) exits with a
`/status`-style line like:

```
claude --resume 1c37051f-212c-41d4-a1d8-9382706fbfa9
```

the user wants to resume that same session — in-place, in the same tmux pane —
without hand-copying the UUID. Sometimes they want the agent's normal invocation
(`claude`); sometimes they want the permissive wrapper (`yolo-claude`). The UUID
sits a few lines above the shell prompt in pane scrollback.

Hand-copying is error-prone and interrupts flow. Scanning the pane and re-exec'ing
is mechanical; delegate it.

## Commands

Two new `rmux_helper` subcommands, both agent-agnostic:

| Subcommand | Launcher chosen | Intended use |
|---|---|---|
| `rmux_helper agent-continue` | `AgentDef.launcher` | normal resume |
| `rmux_helper agent-yolo-continue` | `AgentDef.yolo_launcher` | full-permissions resume (container only) |

Both accept `--window <N>` (default 50) to control how many lines of scrollback to scan.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | agent exec'd successfully (the `execvp` syscall replaces the process, so this code is observed only under `--dry-run`) |
| 1 | no resume command found in the last N lines of the pane |
| 2 | ambiguous — 2+ distinct `(agent, id)` tuples found in window |
| 3 | infrastructure failure — can't resolve caller pane, `tmux capture-pane` failed, `$SHELL` unset, or `execvp` itself returned an error (lost race: exec failed after argv was built) |

## Agent registry

Day-one registry holds one entry (`claude`). New agents are a struct literal.

```rust
struct AgentDef {
    /// Display name used in error messages.
    name: &'static str,
    /// Regex matched against each buffer line; first capture group = session id.
    resume_regex: &'static str,
    /// Binary or shell-function name to exec for normal resume.
    launcher: &'static str,
    /// Binary or shell-function name to exec for yolo resume.
    yolo_launcher: &'static str,
    /// Args that precede the session id in the resume command.
    resume_args: &'static [&'static str],
}

const AGENTS: &[AgentDef] = &[
    AgentDef {
        name: "claude",
        resume_regex: r"\bclaude\s+--resume\s+([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b",
        launcher: "claude",
        yolo_launcher: "yolo-claude",
        resume_args: &["--resume"],
    },
];
```

The regex matches `claude --resume <uuid-v4-shape>` anywhere on a line. It will
match regardless of preceding shell prompt, ANSI color escapes having been
stripped by `tmux capture-pane` (capture-pane emits uncolored text by default),
or quoting in Claude-chat output.

Codex addition when needed: append another `AgentDef` with codex's actual resume
syntax. No other code changes required.

## Behavior

### Caller pane resolution

Use `rmux_helper`'s existing `resolve_pane_by_parent_chain` helper (the same one
backing `parent-pid-tree`). Walk `/proc/<self>/stat` PPID chain against
`tmux list-panes -a -F '#{pane_id} #{pane_pid}'` until a match is found. On
failure → exit 3 with the same stderr wording `parent-pid-tree` uses.

**Do not** use `tmux display-message -p '#{pane_id}'` — that returns the
focused pane, not the caller's. (Documented hazard in `rust/tmux_helper/CLAUDE.md`.)

### Buffer capture

```
tmux capture-pane -p -J -S -<N> -E - -t <pane_id>
```

- `-p` — print to stdout
- `-J` — join soft-wrapped lines (a UUID split across two terminal rows still matches)
- `-S -<N> -E -` — last N lines of scrollback+visible; `N` from `--window`, default 50

### Scan

Walk each line. For each line, try every `AgentDef.resume_regex`. Collect
`(line_idx_from_bottom, agent, id)` for every match. De-duplicate by `(agent, id)` —
the same session appearing on multiple lines (e.g. both `/status` output and a
later copy-paste) counts once.

### Outcomes

| Distinct `(agent, id)` tuples after de-dupe | Behavior |
|---|---|
| 0 | stderr: `no agent resume command found in last <N> lines of pane <%id>. Try --window <N> to widen.` — exit 1 |
| 1 | build argv, `execvp` — exit 0 (process replaced) |
| 2+ | stderr: `rmux_helper agent-continue: found <k> resume targets in last <N> lines — refusing to guess.` followed by one line per match of the form `  line -<offset>: <matched substring>`. Closing line: `Run the one you want manually, or narrow with --window <N>.` — exit 2 |

Sample ambiguous output:

```
rmux_helper agent-continue: found 2 resume targets in last 50 lines — refusing to guess.
  line -3:  claude --resume 1c37051f-212c-41d4-a1d8-9382706fbfa9
  line -41: claude --resume a82e1ff4-6b4d-4e2e-9f0b-3a7d1e5c8a42
Run the one you want manually, or narrow with --window <N>.
```

Rationale for bail-not-pick (vs. a TUI picker): Igor explicitly wants the
zero-argument path to be safe; when it's ambiguous he'd rather the tool hand
control back than silently guess. A picker is future work (`--pick`) if the
pattern recurs.

### Launch

Uniform code path for both binary and shell-function launchers:

```
execvp($SHELL, [$SHELL, "-ic", format!("{} {} {}", launcher, resume_args.join(" "), id)])
```

Why `$SHELL -ic` for both:

- `yolo-claude` is a zsh function defined in `shared/zsh_include.sh` — it's not
  on `PATH`, so `execvp("yolo-claude", ...)` would `ENOENT`.
- Routing both through `$SHELL -ic '<cmd>'` keeps one code path, preserves the
  `_require_container` gate inside `yolo-claude`, and inherits the user's
  interactive environment (aliases, functions, PATH adjustments) the same way
  an interactive command would.
- Extra shell process in the tree is acceptable cost for a one-shot resume.

`$SHELL` comes from the environment; if unset → exit 3.

### Flags

| Flag | Default | Purpose |
|---|---|---|
| `--window <N>` | 50 | Lines of scrollback to capture and scan |
| `--dry-run` | off | Print what would happen and exit without exec'ing. Interaction with outcomes: `Found` → prints `would exec: <shell> -ic '<cmd>'` to stdout, exits 0. `NotFound` / `Ambiguous` → behaves identically to the non-`--dry-run` case (same stderr, same exit code 1/2). `--dry-run` only shortcuts the happy path. |

## Implementation layout (humble object)

Mirrors the `parent-pid-tree` pattern already established in
`src/main.rs` (see `CLAUDE.md` in `rust/tmux_helper/` for the reference
description).

- **Traits (shell layer):**
  - `trait PaneCapturer { fn capture(&self, pane: &str, window: usize) -> Result<String> }`
  - Reuse existing `TmuxProvider` / `ProcReader` for pane resolution.
- **Pure core:**
  ```rust
  fn find_resume_target(buffer: &str, agents: &[AgentDef]) -> FindOutcome;
  enum FindOutcome {
      Found { agent: &'static AgentDef, id: String },
      NotFound,
      Ambiguous(Vec<Match>), // sorted newest-first
  }
  struct Match { line_offset_from_bottom: usize, agent_name: &'static str, id: String, matched_text: String }
  ```
- **Argv builder (pure):**
  ```rust
  fn build_exec_argv(shell: &str, launcher: &str, resume_args: &[&str], id: &str) -> Vec<String>;
  ```
- **Command wrapper (only shell-touching code):**
  ```rust
  fn agent_continue_cmd(yolo: bool, window: usize, dry_run: bool) -> i32;
  ```
  Constructs `RealPaneCapturer` / `RealTmuxProvider` / `RealProcReader`, resolves
  pane, captures buffer, calls `find_resume_target`, on `Found` builds argv and
  `execvp`s (or prints under `--dry-run`).

## Tests

All tests are unit tests — no live tmux, no `/proc`, no exec.

### `find_resume_target`

1. Empty buffer → `NotFound`.
2. One `claude --resume <uuid>` line → `Found`.
3. Two copies of the same `claude --resume <uuid>` on different lines → `Found`
   (de-duped).
4. Two *different* `claude --resume <uuid>` lines → `Ambiguous` with both, sorted
   newest-first.
5. Buffer containing a quoted-inside-chat copy plus a real `/status` line with
   the same UUID → `Found` (de-duped).
6. Buffer containing a quoted-inside-chat copy AND a real `/status` line with a
   *different* UUID → `Ambiguous`.
7. Multi-agent (future-proofing, with a fake second `AgentDef` injected in-test):
   newest match is codex, one claude line earlier → `Ambiguous`.
8. UUID not matching v4 shape (e.g. `claude --resume foo`) → `NotFound`.
9. Soft-wrap case: `-J`-joined buffer where UUID is on one line but the
   `claude --resume` prefix and UUID ended up concatenated — still matches.

### `build_exec_argv`

10. `yolo=false`: `["zsh", "-ic", "claude --resume 1c37..."]`.
11. `yolo=true`: `["zsh", "-ic", "yolo-claude --resume 1c37..."]`.
12. Multi-arg `resume_args` (future agent with e.g. `["resume", "--id"]`):
    `["zsh", "-ic", "foo resume --id 1c37..."]`.

### `PaneCapturer` plumbing

13. Mock `PaneCapturer` records the `window` it was called with — confirms
    `--window 120` reaches the capture call.

### Ambiguous-output formatting

14. Golden-string test of the ambiguous stderr block (stable ordering, stable
    wording — this is user-facing copy that shell aliases may grep against).

## Non-goals

- No TUI picker for ambiguous case (YAGNI — revisit if Igor hits ambiguity often).
- No global scan across all panes (YAGNI — overkill for the "resume what I just
  quit" workflow).
- No persistence of session IDs across pane lifetimes (the pane buffer is the
  source of truth; when it's gone, manual paste is the fallback).
- No auto-detection of `yolo` vs plain based on buffer content — the caller's
  subcommand choice determines the launcher.

## Open questions

None remaining after brainstorming. Flags, ambiguity behavior, launch model,
and extensibility for codex all resolved.

## References

- `rust/tmux_helper/src/main.rs` — `parent-pid-tree` and `Humble Object` pattern.
- `rust/tmux_helper/src/link_picker/mod.rs:194` — existing `capture_pane_args` helper (pattern for building capture argv).
- `shared/zsh_include.sh:534` — `yolo-claude` function definition.
- `rust/tmux_helper/CLAUDE.md` — pane resolution and humble-object conventions.
