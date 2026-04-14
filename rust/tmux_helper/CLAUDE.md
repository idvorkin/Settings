# rmux_helper

Fast Rust-based tmux helper for session/window/pane management.

## Important

**When modifying the picker (`pick-tui` / `pick-list`), update `PICKER_SPEC.md` to reflect any rule changes.**

The spec documents the *what* (behavior rules), not the *how* (implementation). It only covers the picker — `side-edit`, `side-run`, `rename-all`, `rotate`, `third`, etc. do not have specs and don't need PICKER_SPEC updates.

## Commands

- `pick-tui` - Native TUI picker for sessions/windows/panes
- `rename-all` - Rename all windows based on running processes
- `rotate` - Toggle between horizontal/vertical layouts
- `third` - Toggle between even and 1/3-2/3 split
- `parent-pid-tree` - Resolve caller's owning tmux pane by walking the parent-PID chain (see below)

## Building

```bash
cargo build --release
cargo install --path . --force
```

## Testing

```bash
cargo test
```

## Smoke testing against tmux

`cargo build` updates `target/`, but tmux keybindings and `$PATH` resolve to `~/.cargo/bin/rmux_helper`. After any change you want to exercise live, run `cargo install --path . --force` before invoking `rmux_helper` from a tmux session.

## `parent-pid-tree`

Resolves the calling process's owning tmux pane by walking the parent-PID chain from `/proc/<pid>/stat` against `tmux list-panes -a -F '#{pane_id} #{pane_pid}'`. Use this whenever a script needs to answer "which tmux pane am I running inside?" — **never** use `tmux display-message -p '#{pane_id}'` for this, which returns the tmux-*active* pane (the one focused in the attached client), not the caller's pane.

Typical invocations:

- `rmux_helper parent-pid-tree` — prints the owning pane id (e.g. `%35`) on stdout, exits 0. Scriptable: `pane=$(rmux_helper parent-pid-tree)`.
- `rmux_helper parent-pid-tree --json` — structured output with `pane_id`, `pane_pid`, `walked_from_pid`, and `ancestors_walked`. Useful for debugging.
- `rmux_helper parent-pid-tree --pid <N>` — resolve a different pid's owning pane instead of the caller's. Walk starts directly from `N` (no parent-of-ppid hop).
- `rmux_helper parent-pid-tree --verbose` — log the walk chain to stderr for debugging, e.g. `walked 474064 -> 4114505 -> 4114434 -> 2594534 (pane_pid) -> pane %35`.

**Exit codes**:

- `0` — pane found, id printed on stdout
- `1` — no match (caller/pid not in any tmux pane); nothing on stdout, `no tmux pane found for pid <N>` on stderr
- `2` — tmux not running or no panes
- `3` — `/proc/<self>/stat` unreadable (cannot determine caller pid)

**Why this exists**: `tmux display-message -p '#{pane_id}'` returns the focused pane, which is wrong when multiple Claude sessions run in different panes concurrently. Observed 2026-04-14: `harden-telegram`'s watchdog.py reloaded the wrong pane for ~45 minutes before diagnosis. Encapsulating the correct walk here means future tmux-integration code can call `rmux_helper parent-pid-tree` and trust the answer instead of hand-rolling the walk.

**Implementation**: see `fn resolve_pane_by_parent_chain` in `src/main.rs`. The walker is dependency-injected over `read_ppid: FnMut(u32) -> Option<u32>` so unit tests can verify the multi-session, no-match, vanished-parent, and cycle cases without touching `/proc`.

## side-edit / side-run stdout contract

`side-edit` and `side-run` (with no args, status-only) print three lines that shell scripts consume:

```
pane_id: <%N | none | ambiguous>
nvim: <true | false | unknown>
file: <path or empty>
```

- `pane_id: none` = no candidate side pane in the window (we *did* look)
- `pane_id: ambiguous` = multiple plausible candidates, refuse to route
- `nvim: unknown` = inspection failed (pid query, sysinfo race) — must NOT be collapsed to `false`

Tested by `format_pane_status` unit tests in `main.rs`. Don't rename sentinels or change the format without updating consumers.
