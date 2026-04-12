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

    // 5. TUI
    if rows.is_empty() {
        eprintln!("pick-links: no links, servers, or IPs in scrollback");
        return Ok(());
    }
    let action = tui::run(rows)?;

    // 6-7. Dispatch post-TUI (terminal already restored by tui::run).
    match action {
        tui::Action::Quit => std::process::exit(130),
        tui::Action::Yank(row) => {
            yank_osc52(&row.canonical)?;
            println!("{}", row.canonical);
        }
        tui::Action::Open(row) => open_url(&row.canonical)?,
        tui::Action::GhWeb(row) => gh_web(&row)?,
        tui::Action::Ssh(row) => ssh_host(&row, &pane_id)?,
        tui::Action::SwapToPickTui => {
            use std::os::unix::process::CommandExt;
            // execvp: never returns on success
            let err = Command::new("rmux_helper").arg("pick-tui").exec();
            return Err(anyhow!("exec pick-tui failed: {err}"));
        }
    }
    Ok(())
}

fn yank_osc52(payload: &str) -> Result<()> {
    use base64::Engine;
    use std::fs::OpenOptions;
    use std::io::Write;
    let b64 = base64::engine::general_purpose::STANDARD.encode(payload);
    let mut tty = OpenOptions::new().write(true).open("/dev/tty")?;
    write!(tty, "\x1b]52;c;{b64}\x1b\\")?;
    tty.flush()?;
    Ok(())
}

fn open_url(url: &str) -> Result<()> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(cmd).arg(url).status()?;
    Ok(())
}

fn gh_web(row: &detect::Row) -> Result<()> {
    use detect::Category as C;
    let subcmd = match row.category {
        C::PullRequest => "pr",
        C::Issue => "issue",
        C::Commit | C::File | C::Repo => {
            // Fall through to plain open — gh doesn't have a generic "view" for these
            return open_url(&row.canonical);
        }
        _ => return Ok(()),
    };
    // row.key is "#N" for PR/Issue — strip "#" for gh arg.
    let id = row.key.trim_start_matches('#');
    // repo_or_host carries only the repo name; owner must come from canonical.
    // canonical is normalized: https://github.com/OWNER/REPO/...
    let owner_repo = row
        .canonical
        .strip_prefix("https://github.com/")
        .map(|s| {
            let mut parts = s.splitn(3, '/');
            let o = parts.next().unwrap_or("");
            let r = parts.next().unwrap_or("");
            format!("{o}/{r}")
        })
        .unwrap_or_else(|| row.repo_or_host.clone());
    Command::new("gh")
        .args([subcmd, "view", id, "-R", &owner_repo, "--web"])
        .status()?;
    Ok(())
}

fn ssh_host(row: &detect::Row, pane_id: &str) -> Result<()> {
    // Use tmux new-window in the originating pane's session.
    let host_arg = format!("ssh {}", row.canonical);
    let status = Command::new("tmux")
        .args(["new-window", "-t", pane_id, "-c", "#{pane_current_path}", &host_arg])
        .status()?;
    if !status.success() {
        return Err(anyhow!(
            "tmux new-window failed with exit status {}",
            status.code().unwrap_or(-1)
        ));
    }
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
    // NOTE: `-e` (include ANSI escapes) was removed because raw \x1b bytes
    // in the context column leak through ratatui's cell rendering into the
    // popup's pty and are interpreted as terminal control sequences,
    // corrupting the display. Plain text is sufficient for v1.
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-J", "-S", "-", "-E", "-", "-t", pane_id])
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
