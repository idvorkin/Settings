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
            yank_to_clipboard(&row.canonical)?;
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

/// Push `payload` onto the clipboard via `tmux set-buffer -w`.
///
/// The `-w` flag tells tmux to also emit OSC 52 to each attached client's
/// pty, which is the path verified end-to-end (devvm → tmux server →
/// terminal emulator → OS clipboard). The earlier direct write to
/// `/dev/tty` also worked but went through a different code path and was
/// harder to diagnose when intermediate links broke; routing through tmux
/// unifies yank with the rest of `rmux_helper`'s "everything goes through
/// tmux" architecture (capture-pane, display-message, new-window).
///
/// Requires tmux to be running — but so does the whole picker (scrollback
/// capture fails earlier without it), so this introduces no new dependency.
fn yank_to_clipboard(payload: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["set-buffer", "-w", payload])
        .status()
        .map_err(|e| anyhow!("tmux set-buffer failed to spawn: {e}"))?;
    if !status.success() {
        return Err(anyhow!(
            "tmux set-buffer -w returned nonzero: {}",
            status.code().unwrap_or(-1)
        ));
    }
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

/// History depth (lines above the visible pane top) that pick-links scans.
/// Capped deliberately — a full 50k-line `history-limit` buffer produces
/// stale context from days-old work and drowns real results in noise.
/// 300 lines is roughly several screens of recent scrollback, enough to
/// catch "that PR URL I pasted ten minutes ago" without going deeper.
pub(crate) const SCROLLBACK_HISTORY_LINES: u32 = 300;

/// Build the argv for `tmux capture-pane`. Pulled out of `capture_pane` so a
/// unit test can assert the history cap stays in place across refactors.
///
/// `-S -N` starts N lines above the top of the visible pane; `-E -` ends at
/// the bottom of the visible pane. `-J` joins soft-wrapped lines so URLs
/// that wrapped across terminal rows read back whole.
pub(crate) fn capture_pane_args(pane_id: &str) -> Vec<String> {
    // NOTE: `-e` (include ANSI escapes) was intentionally dropped earlier —
    // raw \x1b bytes leaked through ratatui's cell rendering into the popup
    // pty and corrupted the display. Plain text is sufficient.
    vec![
        "capture-pane".to_string(),
        "-p".to_string(),
        "-J".to_string(),
        "-S".to_string(),
        format!("-{SCROLLBACK_HISTORY_LINES}"),
        "-E".to_string(),
        "-".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
    ]
}

/// Capture the recent scrollback of `pane_id` via `tmux capture-pane`.
fn capture_pane(pane_id: &str) -> Result<String> {
    let args = capture_pane_args(pane_id);
    let out = Command::new("tmux")
        .args(&args)
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

    #[test]
    fn capture_pane_caps_history_at_300_lines() {
        // Regression guard: the deliberate 300-line history cap must not
        // silently revert to `-S -` (full history) during refactors.
        // Going deeper surfaced ancient stale context in earlier --json
        // dumps and drowned recent work in noise.
        let args = capture_pane_args("%42");
        let s_idx = args
            .iter()
            .position(|a| a == "-S")
            .expect("capture-pane must pass -S");
        assert_eq!(
            args[s_idx + 1],
            "-300",
            "history depth must stay capped at 300 lines above visible top"
        );
        let e_idx = args
            .iter()
            .position(|a| a == "-E")
            .expect("capture-pane must pass -E");
        assert_eq!(
            args[e_idx + 1],
            "-",
            "end must be bottom of visible pane"
        );
        assert!(args.contains(&"-J".to_string()), "must join wrapped lines");
        assert!(args.contains(&"%42".to_string()), "must target the pane");
        assert!(
            !args.iter().any(|a| a == "-e"),
            "must NOT include ANSI escapes (they corrupt popup rendering)"
        );
    }
}
