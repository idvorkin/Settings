//! Scan the caller's tmux pane scrollback for the most recent AI-agent resume
//! command (`claude --resume <UUID>`) and exec it in place. See
//! docs/superpowers/specs/2026-04-16-rmux-helper-agent-continue-design.md.

/// Entry point for `agent-continue` / `agent-yolo-continue`. Returns a process
/// exit code. Callers should `std::process::exit(rv)` with it.
pub(crate) fn cmd(_yolo: bool, _window: usize, _dry_run: bool) -> i32 {
    eprintln!("agent-continue: not yet implemented");
    3
}
