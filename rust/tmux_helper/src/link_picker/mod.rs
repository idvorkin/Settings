//! Scrollback link picker — top-level orchestration.
//! See spec docs/superpowers/specs/2026-04-12-scrollback-link-picker-design.md §Execution Flow.

pub mod detect;
pub mod enrich;
pub mod tui;

use anyhow::{anyhow, Result};
use std::env;
use std::process::Command;

/// Entry point for `rmux_helper pick-links`. See spec §Execution Flow.
pub fn pick_links(json: bool, enrich_deadline_ms: u64) -> Result<()> {
    // 1. Resolve pane
    let pane_id = resolve_pane_id()?;

    // 2. Capture (sync, fallible)
    let raw = capture_pane(&pane_id)?;

    // 3. Detect
    let rows = detect::parse(&raw);

    if json {
        // --json short-circuit: skip enrich + TUI entirely.
        println!("{}", serde_json::to_string(&rows)?);
        return Ok(());
    }

    // 4. Enrich (blocks inside tokio, then drops runtime before TUI)
    let rows = enrich::enrich_rows(rows, enrich_deadline_ms);

    // 5. TUI (stub in this task; real implementation in Task 14-15)
    if rows.is_empty() {
        eprintln!("pick-links: no links, servers, or IPs in scrollback");
        return Ok(());
    }
    let _action = tui::run(rows)?;

    // 6-7. Teardown + dispatch happen inside tui::run + action handler in Task 16.
    Ok(())
}

/// Resolve the tmux pane id to capture. See spec §Scrollback Capture.
fn resolve_pane_id() -> Result<String> {
    if let Ok(p) = env::var("TMUX_PANE") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    if env::var("TMUX").is_err() {
        return Err(anyhow!("pick-links: not inside tmux; nothing to capture"));
    }
    // Fallback: ask tmux directly.
    let out = Command::new("tmux")
        .args(["display-message", "-p", "-t", "#{client_active_pane}", "#{pane_id}"])
        .output()
        .map_err(|e| anyhow!("tmux display-message failed: {e}"))?;
    if !out.status.success() {
        return Err(anyhow!("tmux display-message returned nonzero"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Capture the full scrollback of `pane_id` via `tmux capture-pane`.
fn capture_pane(pane_id: &str) -> Result<String> {
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-J", "-e", "-S", "-", "-E", "-", "-t", pane_id])
        .output()
        .map_err(|e| anyhow!("tmux capture-pane failed: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(anyhow!(
            "tmux capture-pane -t {pane_id} returned nonzero: {err}"
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod orchestration_tests {
    use super::*;

    #[test]
    fn parses_json_output_from_fake_scrollback() {
        let raw = "Merge https://github.com/a/b/pull/1\nssh c-5001\n";
        let rows = detect::parse(raw);
        let json = serde_json::to_string(&rows).unwrap();
        assert!(json.contains("\"category\":\"pull_request\""));
        assert!(json.contains("\"category\":\"server\""));
    }
}
