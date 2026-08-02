//! Multiplexer plumbing shared by every mux-aware command (`third`,
//! `pick-links`): which multiplexer are we in, the `herdr` CLI shell, and
//! the herdr `pane layout` JSON shape.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::process::Command;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Multiplexer {
    Tmux,
    Herdr,
    Unknown,
}

/// Decide the multiplexer from the four environment variables that matter.
///
/// Split out from `detect` so precedence is unit-testable without mutating
/// the process environment — env mutation races across cargo's parallel
/// test threads.
///
/// `TMUX_PANE` OR `TMUX` means tmux: `run-shell` sets the former,
/// `display-popup` sets only the latter, and tmux nested inside a herdr
/// pane still means tmux is what the user is looking at.
pub fn classify(
    tmux_pane: Option<&str>,
    tmux: Option<&str>,
    herdr_pane_id: Option<&str>,
    herdr_env: Option<&str>,
) -> Multiplexer {
    let set = |v: Option<&str>| v.is_some_and(|s| !s.is_empty());
    if set(tmux_pane) || set(tmux) {
        Multiplexer::Tmux
    } else if set(herdr_pane_id) || set(herdr_env) {
        Multiplexer::Herdr
    } else {
        Multiplexer::Unknown
    }
}

pub fn detect() -> Multiplexer {
    let var = |k: &str| std::env::var(k).ok();
    classify(
        var("TMUX_PANE").as_deref(),
        var("TMUX").as_deref(),
        var("HERDR_PANE_ID").as_deref(),
        var("HERDR_ENV").as_deref(),
    )
}

#[derive(Deserialize, Debug)]
pub struct LayoutResponse {
    pub result: LayoutResult,
}

#[derive(Deserialize, Debug)]
pub struct LayoutResult {
    pub layout: TabLayout,
}

#[derive(Deserialize, Debug)]
pub struct TabLayout {
    #[serde(default)]
    pub focused_pane_id: String,
    pub panes: Vec<PaneEntry>,
    #[serde(default)]
    pub splits: Vec<SplitEntry>,
}

#[derive(Deserialize, Debug)]
pub struct PaneEntry {
    pub pane_id: String,
    pub rect: Rect,
}

#[derive(Deserialize, Debug)]
pub struct SplitEntry {
    pub direction: String,
    pub ratio: f64,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
}

pub fn herdr_cli(args: &[&str]) -> Result<String> {
    let out = Command::new("herdr")
        .args(args)
        .output()
        .context("failed to run `herdr` — is it installed and on PATH?")?;
    if !out.status.success() {
        bail!(
            "herdr {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn fetch_layout(pane: Option<&str>) -> Result<TabLayout> {
    let mut args = vec!["pane", "layout"];
    if let Some(p) = pane {
        args.extend(["--pane", p]);
    }
    // Without --pane, herdr reports the focused tab — correct for a
    // keybinding or popup, which is where pane identity is absent.
    let json = herdr_cli(&args)?;
    let resp: LayoutResponse =
        serde_json::from_str(&json).context("unexpected JSON from `herdr pane layout`")?;
    Ok(resp.result.layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_pane_alone_is_tmux() {
        assert_eq!(classify(Some("%3"), None, None, None), Multiplexer::Tmux);
    }

    #[test]
    fn tmux_var_alone_is_tmux() {
        // tmux display-popup drops TMUX_PANE but keeps TMUX — pick-links
        // runs in exactly that context.
        assert_eq!(
            classify(None, Some("/tmp/tmux-1000/default,123,0"), None, None),
            Multiplexer::Tmux
        );
    }

    #[test]
    fn herdr_env_alone_is_herdr() {
        // herdr popups set HERDR_ENV but omit HERDR_PANE_ID.
        assert_eq!(classify(None, None, None, Some("1")), Multiplexer::Herdr);
    }

    #[test]
    fn herdr_pane_id_alone_is_herdr() {
        assert_eq!(classify(None, None, Some("w9:p1"), None), Multiplexer::Herdr);
    }

    #[test]
    fn tmux_wins_when_both_present() {
        assert_eq!(
            classify(Some("%3"), None, Some("w9:p1"), Some("1")),
            Multiplexer::Tmux
        );
    }

    #[test]
    fn nothing_set_is_unknown() {
        assert_eq!(classify(None, None, None, None), Multiplexer::Unknown);
    }

    #[test]
    fn empty_strings_do_not_count_as_set() {
        assert_eq!(classify(Some(""), Some(""), Some(""), Some("")), Multiplexer::Unknown);
    }

    #[test]
    fn layout_json_exposes_focused_pane_id() {
        // Captured verbatim from `herdr pane layout` on 2026-08-02.
        let json = r#"{"id":"cli:pane:layout","result":{"layout":{"area":{"height":56,"width":170,"x":18,"y":1},"focused_pane_id":"w9:p1","panes":[{"focused":true,"pane_id":"w9:p1","rect":{"height":56,"width":170,"x":18,"y":1}}],"splits":[],"tab_id":"w9:t1","workspace_id":"w9","zoomed":false},"type":"pane_layout"}}"#;
        let layout = serde_json::from_str::<LayoutResponse>(json)
            .expect("layout JSON should parse")
            .result
            .layout;
        assert_eq!(layout.focused_pane_id, "w9:p1");
        assert_eq!(layout.panes.len(), 1);
    }
}
