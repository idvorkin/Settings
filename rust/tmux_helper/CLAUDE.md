# rmux_helper

Fast Rust-based tmux helper for session/window/pane management.

## Important

**When modifying rmux_helper, update `PICKER_SPEC.md` to reflect any rule changes.**

The spec documents the *what* (behavior rules), not the *how* (implementation).

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
