//! `third` under herdr: toggle even <-> 1/3-2/3 via the herdr CLI.
//!
//! Humble-object split (same pattern as agent_continue.rs): `plan_third` is a
//! pure decision function over the parsed `herdr pane layout` JSON; the shell
//! functions that invoke the CLI live at the bottom and stay logic-free.

use crate::mux::{self, PaneEntry, TabLayout};
use anyhow::{bail, Result};

pub const THIRD_RATIO: f64 = 1.0 / 3.0;
pub const EVEN_RATIO: f64 = 0.5;
// First pane inside this band counts as "third active" -> toggle back to even.
// Top-exclusive so a manual 40/60 split reads as "not third" and gets the third applied.
pub const THIRD_BAND_MIN: f64 = 0.25;
pub const THIRD_BAND_MAX: f64 = 0.40;

#[derive(Debug, PartialEq)]
pub enum ThirdAction {
    /// Split `source_pane` side-by-side; the original (left) pane keeps THIRD_RATIO.
    Split { source_pane: String },
    /// Adjust the first (left/top) pane's split ratio by `amount` toward `direction`.
    Resize {
        pane: String,
        direction: &'static str,
        amount: f64,
    },
    Noop { reason: &'static str },
}

pub fn plan_third(layout: &TabLayout) -> ThirdAction {
    match layout.panes.len() {
        1 => ThirdAction::Split {
            source_pane: layout.panes[0].pane_id.clone(),
        },
        2 => {
            let split = match layout.splits.first() {
                Some(s) => s,
                None => {
                    return ThirdAction::Noop {
                        reason: "two panes but no split info in layout",
                    }
                }
            };
            let mut panes: Vec<&PaneEntry> = layout.panes.iter().collect();
            panes.sort_by_key(|p| (p.rect.x, p.rect.y));
            let first = panes[0];

            let third_active = split.ratio >= THIRD_BAND_MIN && split.ratio < THIRD_BAND_MAX;
            let target = if third_active { EVEN_RATIO } else { THIRD_RATIO };
            let delta = target - split.ratio;
            let side_by_side = split.direction == "right";
            let direction = match (side_by_side, delta > 0.0) {
                (true, true) => "right",
                (true, false) => "left",
                (false, true) => "down",
                (false, false) => "up",
            };
            ThirdAction::Resize {
                pane: first.pane_id.clone(),
                direction,
                amount: delta.abs(),
            }
        }
        _ => ThirdAction::Noop {
            reason: "third only handles 1 or 2 panes",
        },
    }
}

/// Map a planned action to the exact `herdr` CLI argument vector, or `None`
/// for `Noop`. Pure and unit-tested independent of any I/O — mirrors
/// `build_exec_argv` in `agent_continue.rs`.
fn action_args(action: &ThirdAction) -> Option<Vec<String>> {
    match action {
        ThirdAction::Split { source_pane } => Some(
            [
                "pane",
                "split",
                "--pane",
                source_pane.as_str(),
                "--direction",
                "right",
                "--ratio",
                "0.33",
                "--focus",
            ]
            .map(String::from)
            .to_vec(),
        ),
        ThirdAction::Resize { pane, direction, amount } => Some(
            [
                "pane",
                "resize",
                "--pane",
                pane.as_str(),
                "--direction",
                direction,
                "--amount",
                &format!("{amount:.4}"),
            ]
            .map(String::from)
            .to_vec(),
        ),
        ThirdAction::Noop { .. } => None,
    }
}

pub fn cmd(command: &str) -> Result<()> {
    if !command.is_empty() {
        bail!(
            "third \"<command>\" is not supported under herdr — use tmux, or herdr's \
             popup bindings (prefix+ctrl+g lazygit, prefix+ctrl+t tig)"
        );
    }
    let caller = std::env::var("HERDR_PANE_ID").ok().filter(|s| !s.is_empty());
    let layout = mux::fetch_layout(caller.as_deref())?;
    let action = plan_third(&layout);
    if let Some(args) = action_args(&action) {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        mux::herdr_cli(&arg_refs)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mux::{Rect, SplitEntry};

    // Captured verbatim from `herdr pane layout --pane w9:p1` on 2026-08-02.
    const SINGLE_PANE_JSON: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"area":{"height":56,"width":170,"x":18,"y":1},"focused_pane_id":"w9:p1","panes":[{"focused":true,"pane_id":"w9:p1","rect":{"height":56,"width":170,"x":18,"y":1}}],"splits":[],"tab_id":"w9:t1","workspace_id":"w9","zoomed":false},"type":"pane_layout"}}"#;
    const TWO_PANE_THIRD_JSON: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"area":{"height":56,"width":170,"x":18,"y":1},"focused_pane_id":"w9:p1","panes":[{"focused":true,"pane_id":"w9:p1","rect":{"height":56,"width":56,"x":18,"y":1}},{"focused":false,"pane_id":"w9:p2","rect":{"height":56,"width":114,"x":74,"y":1}}],"splits":[{"direction":"right","id":"split_0_root","ratio":0.33,"rect":{"height":56,"width":170,"x":18,"y":1}}],"tab_id":"w9:t1","workspace_id":"w9","zoomed":false},"type":"pane_layout"}}"#;

    fn parse(json: &str) -> TabLayout {
        serde_json::from_str::<crate::mux::LayoutResponse>(json)
            .expect("layout JSON should parse")
            .result
            .layout
    }

    fn two_pane(direction: &str, ratio: f64) -> TabLayout {
        // Rects only matter for left/top-first ordering; give pane 2 the larger x/y.
        let (x2, y2) = if direction == "right" { (100, 0) } else { (0, 30) };
        TabLayout {
            focused_pane_id: "w9:p1".into(),
            panes: vec![
                PaneEntry {
                    pane_id: "w9:p1".into(),
                    rect: Rect { x: 0, y: 0 },
                },
                PaneEntry {
                    pane_id: "w9:p2".into(),
                    rect: Rect { x: x2, y: y2 },
                },
            ],
            splits: vec![SplitEntry {
                direction: direction.into(),
                ratio,
            }],
        }
    }

    #[test]
    fn single_pane_splits_from_that_pane() {
        assert_eq!(
            plan_third(&parse(SINGLE_PANE_JSON)),
            ThirdAction::Split { source_pane: "w9:p1".into() }
        );
    }

    #[test]
    fn third_active_restores_even() {
        // Real captured layout at ratio 0.33 -> grow left pane to 0.5.
        match plan_third(&parse(TWO_PANE_THIRD_JSON)) {
            ThirdAction::Resize { pane, direction, amount } => {
                assert_eq!(pane, "w9:p1");
                assert_eq!(direction, "right");
                assert!((amount - 0.17).abs() < 1e-9, "amount was {amount}");
            }
            other => panic!("expected Resize, got {other:?}"),
        }
    }

    #[test]
    fn even_side_by_side_applies_third() {
        match plan_third(&two_pane("right", 0.5)) {
            ThirdAction::Resize { pane, direction, amount } => {
                assert_eq!(pane, "w9:p1");
                assert_eq!(direction, "left");
                assert!((amount - (0.5 - THIRD_RATIO)).abs() < 1e-9);
            }
            other => panic!("expected Resize, got {other:?}"),
        }
    }

    #[test]
    fn even_stacked_applies_third_upward() {
        match plan_third(&two_pane("down", 0.5)) {
            ThirdAction::Resize { direction, .. } => assert_eq!(direction, "up"),
            other => panic!("expected Resize, got {other:?}"),
        }
    }

    #[test]
    fn manual_forty_sixty_applies_third() {
        // 0.40 sits on the top-exclusive band edge: NOT third-active -> shrink to 1/3.
        match plan_third(&two_pane("right", 0.40)) {
            ThirdAction::Resize { direction, amount, .. } => {
                assert_eq!(direction, "left");
                assert!((amount - (0.40 - THIRD_RATIO)).abs() < 1e-9);
            }
            other => panic!("expected Resize, got {other:?}"),
        }
    }

    #[test]
    fn three_panes_is_noop() {
        let mut layout = two_pane("right", 0.5);
        layout.panes.push(PaneEntry {
            pane_id: "w9:p3".into(),
            rect: Rect { x: 200, y: 0 },
        });
        assert!(matches!(plan_third(&layout), ThirdAction::Noop { .. }));
    }

    #[test]
    fn two_panes_without_split_info_is_noop() {
        let mut layout = two_pane("right", 0.5);
        layout.splits.clear();
        assert!(matches!(plan_third(&layout), ThirdAction::Noop { .. }));
    }

    #[test]
    fn command_arg_is_refused_under_herdr() {
        let err = cmd("tig status").expect_err("command form must be rejected");
        assert!(err.to_string().contains("not supported under herdr"));
    }

    #[test]
    fn split_action_maps_to_exact_argv() {
        let action = ThirdAction::Split { source_pane: "w9:p1".into() };
        assert_eq!(
            action_args(&action),
            Some(
                [
                    "pane", "split", "--pane", "w9:p1", "--direction", "right", "--ratio",
                    "0.33", "--focus",
                ]
                .map(String::from)
                .to_vec()
            )
        );
    }

    #[test]
    fn resize_action_maps_to_exact_argv_and_rounds_amount() {
        let action = ThirdAction::Resize {
            pane: "w9:p1".into(),
            direction: "left",
            amount: 1.0 / 6.0, // 0.1666666... -> locks the {:.4} formatting to "0.1667"
        };
        assert_eq!(
            action_args(&action),
            Some(
                [
                    "pane", "resize", "--pane", "w9:p1", "--direction", "left", "--amount",
                    "0.1667",
                ]
                .map(String::from)
                .to_vec()
            )
        );
    }
}
