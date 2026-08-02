# rmux_helper `third` under herdr Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `rmux_helper third` auto-detects tmux vs herdr and toggles the even ↔ 1/3–2/3 layout in either; `prefix+/` works in herdr. Plus build-speed fixes (thin LTO, shared cargo target dir).

**Architecture:** New `src/herdr_third.rs` module following the `agent_continue.rs` humble-object precedent — pure `plan_third(layout) -> ThirdAction` core unit-tested on JSON fixtures, thin shell that drives the `herdr` CLI. `third()` in `main.rs` branches on a new `detect_multiplexer()` helper; the tmux path is untouched.

**Tech Stack:** Rust (edition 2021), clap 4, serde/serde_json (existing deps), `herdr` CLI over its unix socket.

**Spec:** `docs/superpowers/specs/2026-08-02-herdr-third-design.md`

## Global Constraints

- Tmux code path must be byte-for-byte unchanged (regression-checked in Task 5).
- `third "<command>"` under herdr is out of scope — must fail loudly, not silently.
- No persistent state under herdr: toggle is derived from the live layout ratio.
- "Third active" band: `0.25 <= ratio < 0.40` (top-exclusive so a manual 40/60 split → apply third).
- Guardrails: never push to main; explicit `git add` by filename only; conventional commit messages.
- Empirically verified herdr CLI semantics (2026-08-02, do not re-derive):
  - `herdr pane split --pane <ID> --direction right --ratio 0.33` → original (left) pane keeps 33%; cwd inherited from source pane.
  - `herdr pane resize --pane <ID> --direction right --amount 0.1` → **delta** fraction; grows that pane's ratio by 0.1 (e.g. 0.33 → 0.43). `left` shrinks it. Stacked splits use `down`/`up`.
  - `herdr pane layout [--pane <ID>]` → JSON with `result.layout.panes[].rect` and `result.layout.splits[]` (`direction`: `"right"`|`"down"`, `ratio` = first/left/top pane's fraction). Bare invocation targets the **focused** tab.

---

### Task 1: Build-speed fixes

**Files:**

- Modify: `rust/tmux_helper/Cargo.toml` (bottom, `[profile.release]`)
- Modify: `shared/zsh_include.sh` (near line 773, alongside `export HOMEBREW_NO_AUTO_UPDATE=1`)
- Modify: `rust/tmux_helper/CLAUDE.md` (Building + Smoke testing sections)

**Interfaces:**

- Consumes: nothing.
- Produces: fast builds for Tasks 2–5. Later tasks should prefix cargo commands with `CARGO_TARGET_DIR=$HOME/.cache/cargo-target` (their shells predate the zsh export).

- [ ] **Step 1: Relax the release profile**

In `rust/tmux_helper/Cargo.toml`, replace the `[profile.release]` block:

```toml
[profile.release]
opt-level = 3
lto = "thin"
strip = true
```

(Removes `lto = true` fat LTO and `codegen-units = 1` — a distribution profile whose mostly single-threaded whole-program LTO pass dominates every rebuild and buys nothing perceptible for a subprocess-bound CLI.)

- [ ] **Step 2: Add the shared cargo target dir to shared zsh config**

In `shared/zsh_include.sh`, next to the `export HOMEBREW_NO_AUTO_UPDATE=1` line (~773), add:

```zsh
# Shared cargo build cache: every repo/worktree reuses one dep-artifact dir,
# so fresh (herdr) worktrees skip the cold all-deps rebuild. Note this means
# build output lands in $CARGO_TARGET_DIR/{debug,release}/, not ./target/.
export CARGO_TARGET_DIR="$HOME/.cache/cargo-target"
```

- [ ] **Step 3: Verify the build works and warms the shared cache**

Run (from `rust/tmux_helper/`):

```bash
mkdir -p "$HOME/.cache/cargo-target"
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo build 2>&1 | tail -3
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo build --release 2>&1 | tail -3
```

Expected: both finish successfully (`Finished` line). The second `--release` build also proves thin-LTO compiles. Binaries land in `~/.cache/cargo-target/{debug,release}/rmux_helper`.

- [ ] **Step 4: Document the dev loop in `rust/tmux_helper/CLAUDE.md`**

Replace the "Building" section body with:

````markdown
```bash
cargo build            # dev profile — use this while iterating
cargo install --path . --force   # only when smoke-testing via tmux/herdr keybindings
```

With `CARGO_TARGET_DIR="$HOME/.cache/cargo-target"` exported (shared/zsh_include.sh),
build output lands in `$CARGO_TARGET_DIR/debug/rmux_helper`, not `./target/`, and all
worktrees share one dependency cache — fresh worktrees build in seconds.
The release profile uses thin LTO (fat LTO + codegen-units=1 was the old
distribution profile; it made every rebuild minutes long for no perceptible
runtime win in a subprocess-bound CLI).
````

Also update the "Smoke testing against tmux" section's first sentence to say `cargo build` updates `$CARGO_TARGET_DIR` (or `target/` if unset) rather than `target/`.

- [ ] **Step 5: Commit**

```bash
git add rust/tmux_helper/Cargo.toml shared/zsh_include.sh rust/tmux_helper/CLAUDE.md
git commit -m "build(rmux_helper): thin LTO + shared cargo target dir for fast worktree builds"
```

---

### Task 2: `herdr_third` pure core (layout types + `plan_third`)

**Files:**

- Create: `rust/tmux_helper/src/herdr_third.rs`
- Modify: `rust/tmux_helper/src/main.rs:1-3` (add `mod herdr_third;` beside `mod agent_continue;`)

**Interfaces:**

- Consumes: nothing (pure; serde/serde_json/anyhow are existing deps).
- Produces (Task 3 relies on these exact names):
  - `pub struct LayoutResponse` (serde) with `.result.layout: TabLayout`
  - `pub struct TabLayout { panes: Vec<PaneEntry>, splits: Vec<SplitEntry> }`
  - `pub struct PaneEntry { pane_id: String, rect: Rect }`, `pub struct SplitEntry { direction: String, ratio: f64 }`, `pub struct Rect { x: i64, y: i64, width: i64, height: i64 }`
  - `pub enum ThirdAction { Split { source_pane: String }, Resize { pane: String, direction: &'static str, amount: f64 }, Noop { reason: &'static str } }`
  - `pub fn plan_third(layout: &TabLayout) -> ThirdAction`

- [ ] **Step 1: Create the module with types and failing tests**

Create `rust/tmux_helper/src/herdr_third.rs`:

```rust
//! `third` under herdr: toggle even <-> 1/3-2/3 via the herdr CLI.
//!
//! Humble-object split (same pattern as agent_continue.rs): `plan_third` is a
//! pure decision function over the parsed `herdr pane layout` JSON; the shell
//! functions that invoke the CLI live at the bottom and stay logic-free.

use serde::Deserialize;

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
    pub width: i64,
    pub height: i64,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // Captured verbatim from `herdr pane layout --pane w9:p1` on 2026-08-02.
    const SINGLE_PANE_JSON: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"area":{"height":56,"width":170,"x":18,"y":1},"focused_pane_id":"w9:p1","panes":[{"focused":true,"pane_id":"w9:p1","rect":{"height":56,"width":170,"x":18,"y":1}}],"splits":[],"tab_id":"w9:t1","workspace_id":"w9","zoomed":false},"type":"pane_layout"}}"#;
    const TWO_PANE_THIRD_JSON: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"area":{"height":56,"width":170,"x":18,"y":1},"focused_pane_id":"w9:p1","panes":[{"focused":true,"pane_id":"w9:p1","rect":{"height":56,"width":56,"x":18,"y":1}},{"focused":false,"pane_id":"w9:p2","rect":{"height":56,"width":114,"x":74,"y":1}}],"splits":[{"direction":"right","id":"split_0_root","ratio":0.33,"rect":{"height":56,"width":170,"x":18,"y":1}}],"tab_id":"w9:t1","workspace_id":"w9","zoomed":false},"type":"pane_layout"}}"#;

    fn parse(json: &str) -> TabLayout {
        serde_json::from_str::<LayoutResponse>(json)
            .expect("layout JSON should parse")
            .result
            .layout
    }

    fn two_pane(direction: &str, ratio: f64) -> TabLayout {
        // Rects only matter for left/top-first ordering; give pane 2 the larger x/y.
        let (x2, y2) = if direction == "right" { (100, 0) } else { (0, 30) };
        TabLayout {
            panes: vec![
                PaneEntry {
                    pane_id: "w9:p1".into(),
                    rect: Rect { x: 0, y: 0, width: 100, height: 50 },
                },
                PaneEntry {
                    pane_id: "w9:p2".into(),
                    rect: Rect { x: x2, y: y2, width: 100, height: 50 },
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
            rect: Rect { x: 200, y: 0, width: 50, height: 50 },
        });
        assert!(matches!(plan_third(&layout), ThirdAction::Noop { .. }));
    }

    #[test]
    fn two_panes_without_split_info_is_noop() {
        let mut layout = two_pane("right", 0.5);
        layout.splits.clear();
        assert!(matches!(plan_third(&layout), ThirdAction::Noop { .. }));
    }
}
```

- [ ] **Step 2: Register the module and confirm tests fail-then-pass honestly**

Add to `rust/tmux_helper/src/main.rs` line 1 area, beside the existing mods:

```rust
mod herdr_third;
```

Because module + tests land together, verify the tests actually exercise the code: temporarily flip `THIRD_BAND_MAX` to `0.45`, run the tests, confirm `manual_forty_sixty_applies_third` FAILS, then restore `0.40`.

Run: `CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test herdr_third` (from `rust/tmux_helper/`)
Expected: with `0.45` → 1 failure; restored → all 7 pass.

Note: `mod herdr_third;` will emit dead-code warnings until Task 3 wires it up — that's fine, don't suppress with `#[allow]`.

- [ ] **Step 3: Commit**

```bash
git add rust/tmux_helper/src/herdr_third.rs rust/tmux_helper/src/main.rs
git commit -m "feat(rmux_helper): pure layout planner for third under herdr"
```

---

### Task 3: Detection, herdr CLI shell, and `third()` wiring

**Files:**

- Modify: `rust/tmux_helper/src/herdr_third.rs` (append shell functions + `cmd`)
- Modify: `rust/tmux_helper/src/main.rs:1047` (`fn third`) and add `detect_multiplexer` near `get_caller_pane_id` (~line 1153)

**Interfaces:**

- Consumes: Task 2's `plan_third`, `ThirdAction`, `LayoutResponse`, `TabLayout`.
- Produces: `pub fn cmd(command: &str) -> anyhow::Result<()>` in `herdr_third`; `fn detect_multiplexer() -> Multiplexer` + `enum Multiplexer { Tmux, Herdr, Unknown }` in `main.rs`.

- [ ] **Step 1: Write the failing test for the command-arg refusal**

Append to the `tests` module in `herdr_third.rs`:

```rust
    #[test]
    fn command_arg_is_refused_under_herdr() {
        let err = cmd("tig status").expect_err("command form must be rejected");
        assert!(err.to_string().contains("not supported under herdr"));
    }
```

Run: `CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test herdr_third`
Expected: FAIL — `cmd` not found.

- [ ] **Step 2: Implement the thin shell in `herdr_third.rs`**

Append above the `tests` module:

```rust
use anyhow::{bail, Context, Result};
use std::process::Command;

fn herdr_cli(args: &[&str]) -> Result<String> {
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

fn fetch_layout(pane: Option<&str>) -> Result<TabLayout> {
    let mut args = vec!["pane", "layout"];
    if let Some(p) = pane {
        args.extend(["--pane", p]);
    }
    // Without --pane, herdr reports the focused tab — correct for a keybinding.
    let json = herdr_cli(&args)?;
    let resp: LayoutResponse =
        serde_json::from_str(&json).context("unexpected JSON from `herdr pane layout`")?;
    Ok(resp.result.layout)
}

pub fn cmd(command: &str) -> Result<()> {
    if !command.is_empty() {
        bail!(
            "third \"<command>\" is not supported under herdr — use tmux, or herdr's \
             popup bindings (prefix+ctrl+g lazygit, prefix+ctrl+t tig)"
        );
    }
    let caller = std::env::var("HERDR_PANE_ID").ok().filter(|s| !s.is_empty());
    let layout = fetch_layout(caller.as_deref())?;
    match plan_third(&layout) {
        ThirdAction::Split { source_pane } => {
            herdr_cli(&[
                "pane", "split", "--pane", &source_pane, "--direction", "right",
                "--ratio", "0.33", "--focus",
            ])?;
        }
        ThirdAction::Resize { pane, direction, amount } => {
            herdr_cli(&[
                "pane", "resize", "--pane", &pane, "--direction", direction,
                "--amount", &format!("{amount:.4}"),
            ])?;
        }
        ThirdAction::Noop { .. } => {}
    }
    Ok(())
}
```

(Move the `use` lines to the top of the file with the existing `use serde::Deserialize;` — rustfmt will complain otherwise.)

- [ ] **Step 3: Add detection and branch in `main.rs`**

Near `get_caller_pane_id` (~line 1153), add:

```rust
#[derive(PartialEq, Debug)]
enum Multiplexer {
    Tmux,
    Herdr,
    Unknown,
}

/// TMUX_PANE wins when both are set: tmux running inside a herdr pane means
/// tmux is the multiplexer the user is actually looking at.
fn detect_multiplexer() -> Multiplexer {
    let set = |k: &str| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false);
    if set("TMUX_PANE") {
        Multiplexer::Tmux
    } else if set("HERDR_PANE_ID") || set("HERDR_ENV") {
        Multiplexer::Herdr
    } else {
        Multiplexer::Unknown
    }
}
```

At the top of `fn third` (line 1047), before the existing body:

```rust
fn third(command: &str) -> Result<()> {
    if detect_multiplexer() == Multiplexer::Herdr {
        return herdr_third::cmd(command);
    }
    // ... existing body completely unchanged ...
```

`Unknown` falls through to the tmux path, preserving today's graceful no-op outside both multiplexers.

- [ ] **Step 4: Run tests and clippy**

Run (from `rust/tmux_helper/`):

```bash
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo clippy 2>&1 | tail -5
```

Expected: all tests pass (8 herdr_third tests + existing suite); no new clippy warnings in `herdr_third.rs` or `third`/`detect_multiplexer`.

- [ ] **Step 5: Commit**

```bash
git add rust/tmux_helper/src/herdr_third.rs rust/tmux_helper/src/main.rs
git commit -m "feat(rmux_helper): third auto-detects tmux vs herdr"
```

---

### Task 4: herdr keybinding + docs

**Files:**

- Modify: `config/herdr/config.toml` (before the existing `[[keys.command]]` popup entries, ~line 95)
- Modify: `config/herdr/README.md` (Panes keybinding table, ~line 123)
- Modify: `rust/tmux_helper/CLAUDE.md` (Commands list, `third` entry)

**Interfaces:**

- Consumes: the installed `rmux_helper` binary (Task 5 installs it; the binding is inert until then).
- Produces: `prefix+/` in herdr.

- [ ] **Step 1: Add the binding to `config/herdr/config.toml`**

Insert before the lazygit popup block:

```toml
# --- layout ------------------------------------------------------------------
# tmux: C-a / -> rmux_helper third (toggle even <-> 1/3-2/3, split if 1 pane).
# env HERDR_ENV=1 forces herdr detection: shell bindings run detached, so we
# don't rely on herdr injecting pane env; rmux_helper then targets the
# focused tab, which is the pane the keypress came from.
[[keys.command]]
key = "prefix+/"
type = "shell"
command = "env HERDR_ENV=1 rmux_helper third"
```

- [ ] **Step 2: Validate and reload**

Run:

```bash
herdr config check
herdr server reload-config
```

Expected: check passes (proves `prefix+/` syntax parses); reload returns ok. If `prefix+/` is rejected, try `key = "prefix+slash"` and record which form worked in the README.

- [ ] **Step 3: Document**

`config/herdr/README.md` — add to the Panes table (after the swap rows):

```markdown
| `prefix+/` | toggle 1/3–2/3 layout | `/` → `rmux_helper third` |
```

`rust/tmux_helper/CLAUDE.md` — change the Commands bullet to:

```markdown
- `third` - Toggle between even and 1/3-2/3 split. Works under tmux and herdr (auto-detected via TMUX_PANE / HERDR_PANE_ID+HERDR_ENV; bare toggle only under herdr — the `third "<cmd>"` form is tmux-only)
```

- [ ] **Step 4: Commit**

```bash
git add config/herdr/config.toml config/herdr/README.md rust/tmux_helper/CLAUDE.md
git commit -m "config(herdr): bind prefix+/ to rmux_helper third"
```

---

### Task 5: Install, live smoke, tmux regression, PR

**Files:**

- No source changes (fix-forward if smoke fails — new commits, reference the failing step).

**Interfaces:**

- Consumes: everything above.
- Produces: installed binary, PR.

- [ ] **Step 1: Install the binary**

Run (from `rust/tmux_helper/`):

```bash
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo install --path . --force
rmux_helper --help | head -3
```

Expected: installs to `~/.cargo/bin/rmux_helper` in well under a minute (thin LTO + warm cache); help prints.

- [ ] **Step 2: Herdr smoke — full toggle cycle on this session's own pane**

The executing shell has `HERDR_PANE_ID` set (e.g. `w9:p1`). This visibly splits the running session's pane for a few seconds — that's expected and cleaned up below.

```bash
ratio() { herdr pane layout --pane "$HERDR_PANE_ID" | python3 -c "import json,sys; s=json.load(sys.stdin)['result']['layout']['splits']; print(s[0]['ratio'] if s else 'none')"; }
ratio                      # expect: none (1 pane)
rmux_helper third; ratio   # expect: ~0.33 (split created, left pane 1/3)
rmux_helper third; ratio   # expect: ~0.5  (restored even)
rmux_helper third; ratio   # expect: ~0.33 (third re-applied)
# cleanup: close the pane the first invocation created
herdr pane layout --pane "$HERDR_PANE_ID" | python3 -c "import json,sys; ps=[p['pane_id'] for p in json.load(sys.stdin)['result']['layout']['panes']]; import os; print([p for p in ps if p != os.environ['HERDR_PANE_ID']][0])"
herdr pane close <that-pane-id>
ratio                      # expect: none
```

Expected ratios in sequence: `none`, `~0.33`, `~0.5`, `~0.33`, then `none` after cleanup. Also verify fallback targeting once: `env -u HERDR_PANE_ID HERDR_ENV=1 rmux_helper third` must not error (it acts on the focused tab — run it only if the focused tab is safe to touch, otherwise note it as user-verified via keypress).

- [ ] **Step 3: Tmux regression — bare toggle unchanged**

```bash
tmux new-session -d -s third-smoke -x 200 -y 50
tmux send-keys -t third-smoke 'rmux_helper third' Enter && sleep 1
tmux list-panes -t third-smoke -F '#{pane_width}'   # expect 2 panes, ~66 and ~132
tmux send-keys -t third-smoke.1 'rmux_helper third' Enter && sleep 1
tmux list-panes -t third-smoke -F '#{pane_width}'   # expect ~100 and ~100 (even restored)
tmux kill-session -t third-smoke
```

Expected: widths as annotated (1/3–2/3 then even). This exercises `ensure_two_panes`, orientation detection, and `@third_state` — the untouched tmux path.

- [ ] **Step 4: Full test suite + pre-commit**

Run (from repo root):

```bash
(cd rust/tmux_helper && CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test)
pre-commit run --all-files
```

Expected: cargo tests all pass; pre-commit green (or auto-fixes that get re-staged and committed).

- [ ] **Step 5: Ask Igor to verify the keybinding live**

The one thing not verifiable headless: press `C-a /` in a herdr tab (config was reloaded in Task 4). Report back what it did. If nothing happens, check whether the binding fired at all via `herdr api` logs / `~/.config/herdr` logs, and whether `rmux_helper` is on the herdr server's PATH (the server spawns the shell command — if it launched before `~/.cargo/bin` was in PATH, use an absolute path in the binding's `command`).

- [ ] **Step 6: Push branch and open PR**

```bash
git log --oneline main..HEAD           # sanity: only this feature's commits
~/.local/bin/git-safe-push origin fix-rmux-helper || git push -u origin fix-rmux-helper
gh pr create --title "rmux_helper third works under herdr + faster builds" --body "$(cat <<'EOF'
## Summary
- `rmux_helper third` auto-detects tmux vs herdr (TMUX_PANE wins when nested) and toggles even ↔ 1/3–2/3 under both
- herdr toggle state is derived from the live split ratio (band 0.25 ≤ r < 0.40) — no stored state
- `prefix+/` bound in herdr config (`type = "shell"`)
- Build speed: thin LTO (was fat LTO + codegen-units=1) and shared `CARGO_TARGET_DIR` so herdr worktrees skip cold rebuilds

Spec: docs/superpowers/specs/2026-08-02-herdr-third-design.md
Plan: docs/superpowers/plans/2026-08-02-herdr-third.md

## Test plan
- [x] 8 unit tests on `plan_third` (real captured layout JSON fixtures)
- [x] Live herdr toggle cycle (split → even → third) via CLI
- [x] Tmux regression: bare toggle in scratch session (1/3–2/3 ↔ even)
- [ ] Igor: `C-a /` keypress in herdr

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

If push is denied for the AI-tools account, fork per CLAUDE.md: `gh repo fork --remote-name fork && git push fork fix-rmux-helper && gh pr create --head idvorkin-ai-tools:fix-rmux-helper ...`.
