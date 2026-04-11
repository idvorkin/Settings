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
