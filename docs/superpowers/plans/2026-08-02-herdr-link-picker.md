# Scrollback link picker under herdr Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `rmux_helper pick-links` works under herdr as well as tmux, bound to `prefix+shift+l`, with the multiplexer plumbing shared with `third` instead of duplicated.

**Architecture:** A new `src/mux.rs` owns everything both multiplexer-aware commands need: the `Multiplexer` enum, a pure `classify()` + env-reading `detect()`, the `herdr` CLI shell, and the herdr `pane layout` JSON types. `herdr_third.rs` and `link_picker/` both consume it. `link_picker/mod.rs` keeps its orchestration shape; three I/O touchpoints (resolve, capture, yank) become backend-dispatched with pure, unit-tested argv/payload builders.

**Tech Stack:** Rust (edition 2021), anyhow, serde/serde_json, ratatui/crossterm, `base64 = "0.22"` (new), `herdr` CLI.

**Spec:** `docs/superpowers/specs/2026-08-02-herdr-link-picker-design.md`

## Global Constraints

- Branch is `herdr-link-picker` (already created off the merged `main`). Never push to main; explicit `git add` by filename only; conventional commit messages.
- Pre-commit hooks run on commit. After every `git commit`, confirm the output contains the `[herdr-link-picker <sha>] <subject>` line — if it is missing, a hook reformatted staged files and the commit did **not** land; re-stage and retry.
- The `typos` hook rejects common misspelling fragments when they appear as standalone hyphenated words; if it fires, reword the prose rather than adding an ignore rule.
- Build and test with the shared cache: prefix cargo commands with `CARGO_TARGET_DIR=$HOME/.cache/cargo-target`, run from `rust/tmux_helper/`.
- The tmux code path must stay behaviorally unchanged; its existing tests (`yank_argv_passes_dash_w_and_single_payload_arg`, `capture_pane_caps_history_at_300_lines`) must keep passing untouched.
- Detection precedence, exact: `TMUX_PANE` or `TMUX` set → Tmux; else `HERDR_PANE_ID` or `HERDR_ENV` set → Herdr; else Unknown.
- Scrollback depth stays one shared constant: `SCROLLBACK_HISTORY_LINES = 300`.
- Empirically verified herdr CLI semantics (2026-08-02 against herdr 0.7.5 source at `d742e51` — do not re-derive):
  - `herdr pane read <PANE_ID> --source recent-unwrapped --lines <N>` — **pane id is positional and comes first**; flags after it. Wrong order yields `unknown option: <value>`.
  - `herdr pane layout [--pane <ID>]` returns `result.layout` with `panes[]`, `splits[]`, and `focused_pane_id`. Bare invocation targets the focused tab.
  - Popups (`type = "popup"`) receive `HERDR_ENV=1` but **not** `HERDR_PANE_ID` (herdr `src/app/popup.rs:154`).
  - herdr parses OSC 52 written by any pane process and forwards it to the client clipboard (herdr `src/pane.rs:1851`).

---

### Task 1: Shared `src/mux.rs`

**Files:**

- Create: `rust/tmux_helper/src/mux.rs`
- Modify: `rust/tmux_helper/src/main.rs` (add `mod mux;`; delete the local `Multiplexer` enum + `detect_multiplexer` near line 1162; update the branch at line 1049)
- Modify: `rust/tmux_helper/src/herdr_third.rs` (delete its `herdr_cli` at line 106 and layout types at lines 11-40ish; import from `mux`; add `focused_pane_id` to the test helper)

**Interfaces:**

- Consumes: nothing.
- Produces (Tasks 2 and 3 rely on these exact names):
  - `pub enum Multiplexer { Tmux, Herdr, Unknown }` — derives `PartialEq, Debug, Clone, Copy`
  - `pub fn classify(tmux_pane: Option<&str>, tmux: Option<&str>, herdr_pane_id: Option<&str>, herdr_env: Option<&str>) -> Multiplexer`
  - `pub fn detect() -> Multiplexer`
  - `pub fn herdr_cli(args: &[&str]) -> anyhow::Result<String>`
  - `pub struct LayoutResponse { pub result: LayoutResult }`, `pub struct LayoutResult { pub layout: TabLayout }`
  - `pub struct TabLayout { pub focused_pane_id: String, pub panes: Vec<PaneEntry>, pub splits: Vec<SplitEntry> }`
  - `pub struct PaneEntry { pub pane_id: String, pub rect: Rect }`, `pub struct SplitEntry { pub direction: String, pub ratio: f64 }`, `pub struct Rect { pub x: i64, pub y: i64 }`
  - `pub fn fetch_layout(pane: Option<&str>) -> anyhow::Result<TabLayout>`

**Why the layout types move here:** both `third` (needs `panes`/`splits`) and `pick-links` (needs `focused_pane_id`) parse the same `herdr pane layout` response. One canonical representation is the DRY payoff this whole plan is for; leaving them in `herdr_third` would make the link picker import from a module named after an unrelated command.

- [ ] **Step 1: Create `src/mux.rs` with the pure core and its tests**

Create `rust/tmux_helper/src/mux.rs`:

```rust
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
```

- [ ] **Step 2: Register the module and run the new tests**

Add `mod mux;` to the module list at the top of `rust/tmux_helper/src/main.rs` (beside `mod agent_continue;`, `mod herdr_third;`, `mod link_picker;`, `mod picker;`).

Run: `CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test mux::`
Expected: 8 tests pass.

- [ ] **Step 3: Point `main.rs` at `mux` and delete the local copy**

In `rust/tmux_helper/src/main.rs`, delete the whole `#[derive(PartialEq, Debug)] enum Multiplexer { ... }` block and the `fn detect_multiplexer() -> Multiplexer { ... }` below it (near line 1162, just after `get_caller_pane_id`).

Change the branch at the top of `fn third` (line 1049) from:

```rust
    if detect_multiplexer() == Multiplexer::Herdr {
```

to:

```rust
    if mux::detect() == mux::Multiplexer::Herdr {
```

- [ ] **Step 4: Point `herdr_third.rs` at `mux` and delete its copies**

In `rust/tmux_helper/src/herdr_third.rs`:

1. Delete the `LayoutResponse`, `LayoutResult`, `TabLayout`, `PaneEntry`, `SplitEntry`, and `Rect` struct definitions, the `herdr_cli` function (line 106), and the `fetch_layout` function.
2. Replace the `use` block at the top with:

```rust
use crate::mux::{self, PaneEntry, SplitEntry, TabLayout};
use anyhow::{bail, Result};
```

3. In `cmd`, replace `fetch_layout(caller.as_deref())?` with `mux::fetch_layout(caller.as_deref())?` and `herdr_cli(&arg_refs)?` with `mux::herdr_cli(&arg_refs)?`.
4. In the test module, the `parse` helper now needs the mux type — change its body to:

```rust
    fn parse(json: &str) -> TabLayout {
        serde_json::from_str::<crate::mux::LayoutResponse>(json)
            .expect("layout JSON should parse")
            .result
            .layout
    }
```

5. The `two_pane` test helper (line ~203) constructs a `TabLayout` literal; add the new field as its first entry:

```rust
        TabLayout {
            focused_pane_id: "w9:p1".into(),
            panes: vec![
```

Also drop the `width`/`height` fields from the two `Rect { .. }` literals in that helper and in `three_panes_is_noop` if the compiler flags them — `mux::Rect` carries only `x` and `y`, matching what `plan_third` reads.

- [ ] **Step 5: Verify the whole suite and clippy**

Run:

```bash
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo clippy 2>&1 | tail -5
```

Expected: all tests pass (the 10 pre-existing `herdr_third` tests still pass against the moved types — this is the regression guard for the refactor). No new clippy warnings in `mux.rs`, `herdr_third.rs`, or `main.rs`.

- [ ] **Step 6: Commit**

```bash
git add rust/tmux_helper/src/mux.rs rust/tmux_helper/src/main.rs rust/tmux_helper/src/herdr_third.rs
git commit -m "refactor(rmux_helper): extract shared mux module for detection and herdr CLI"
```

---

### Task 2: herdr resolve, capture, and OSC 52 yank in `link_picker`

**Files:**

- Modify: `rust/tmux_helper/src/link_picker/mod.rs` (the `pick_links` orchestrator, `resolve_pane_id`, capture, yank, and dispatch)
- Modify: `rust/tmux_helper/Cargo.toml` (add `base64`)

**Interfaces:**

- Consumes: `crate::mux::{detect, fetch_layout, herdr_cli, Multiplexer}` from Task 1.
- Produces (Task 3 relies on this): `pick_links` passes a `Multiplexer` into `tui::run(rows, mux)`.
- Produces (internal, unit-tested): `pub(crate) fn herdr_capture_args(pane_id: &str) -> Vec<String>`, `pub(crate) fn osc52_payload(text: &str) -> Vec<u8>`.

- [ ] **Step 1: Write the failing tests**

Add to the `orchestration_tests` module at the bottom of `rust/tmux_helper/src/link_picker/mod.rs`:

```rust
    #[test]
    fn herdr_capture_args_put_pane_id_first_and_cap_history() {
        // herdr's CLI takes the pane id POSITIONALLY, before any flags —
        // `herdr pane read --source recent w9:p1` fails with
        // "unknown option: w9:p1". Regression guard for that arg order.
        let args = herdr_capture_args("w9:p1");
        assert_eq!(args[0], "pane");
        assert_eq!(args[1], "read");
        assert_eq!(args[2], "w9:p1", "pane id must be the first positional");
        let src = args.iter().position(|a| a == "--source").expect("needs --source");
        assert_eq!(
            args[src + 1],
            "recent-unwrapped",
            "must join soft-wrapped lines so wrapped URLs read back whole"
        );
        let lines = args.iter().position(|a| a == "--lines").expect("needs --lines");
        assert_eq!(
            args[lines + 1],
            SCROLLBACK_HISTORY_LINES.to_string(),
            "history depth must come from the shared constant"
        );
    }

    #[test]
    fn osc52_payload_wraps_base64_in_the_clipboard_escape() {
        let bytes = osc52_payload("hi");
        assert_eq!(bytes, b"\x1b]52;c;aGk=\x07".to_vec());
    }

    #[test]
    fn osc52_payload_encodes_spaces_and_non_ascii() {
        // URLs with spaces or unicode must survive the base64 hop intact.
        let text = "https://example.com/a b/\u{e9}";
        let bytes = osc52_payload(text);
        let s = String::from_utf8(bytes).expect("escape is valid utf8");
        let b64 = s
            .strip_prefix("\x1b]52;c;")
            .and_then(|s| s.strip_suffix('\x07'))
            .expect("must be wrapped in the OSC 52 prefix and BEL terminator");
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let decoded = STANDARD.decode(b64).expect("payload must be valid base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), text);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test link_picker::orchestration_tests`
Expected: FAIL — `herdr_capture_args` and `osc52_payload` not found, plus an unresolved `base64` import.

- [ ] **Step 3: Add the base64 dependency**

In `rust/tmux_helper/Cargo.toml`, add to `[dependencies]` (keep the list's existing ordering style):

```toml
base64 = "0.22"
```

- [ ] **Step 4: Implement the builders and the herdr I/O**

In `rust/tmux_helper/src/link_picker/mod.rs`, add near the existing `capture_pane_args`:

```rust
/// Build the argv for `herdr pane read`. The pane id is POSITIONAL and must
/// come first — herdr rejects `pane read --source recent <id>` with
/// "unknown option: <id>". `recent-unwrapped` is herdr's equivalent of
/// tmux's `capture-pane -J`: it joins soft-wrapped lines so a URL that
/// wrapped across terminal rows reads back whole.
pub(crate) fn herdr_capture_args(pane_id: &str) -> Vec<String> {
    vec![
        "pane".to_string(),
        "read".to_string(),
        pane_id.to_string(),
        "--source".to_string(),
        "recent-unwrapped".to_string(),
        "--lines".to_string(),
        SCROLLBACK_HISTORY_LINES.to_string(),
    ]
}

/// Build the OSC 52 clipboard escape for `text`.
///
/// `ESC ] 52 ; c ; <base64> BEL` — `c` is the clipboard selection. herdr
/// parses this out of the pane's pty and forwards it to the client's OS
/// clipboard, so under herdr this replaces tmux's `set-buffer -w` entirely.
/// No tmux passthrough wrapping: that is a tmux-only concern and this path
/// only runs under herdr.
pub(crate) fn osc52_payload(text: &str) -> Vec<u8> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let mut out = b"\x1b]52;c;".to_vec();
    out.extend_from_slice(STANDARD.encode(text).as_bytes());
    out.push(0x07);
    out
}

/// Write the OSC 52 escape to the controlling terminal.
///
/// `/dev/tty` rather than stdout: stdout may be piped (`pick-links --json`,
/// shell capture), and the escape has to reach the pty herdr is parsing.
fn yank_osc52(payload: &str) -> Result<()> {
    use std::io::Write;
    let mut tty = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .map_err(|e| anyhow!("cannot open /dev/tty for clipboard write: {e}"))?;
    tty.write_all(&osc52_payload(payload))
        .map_err(|e| anyhow!("failed writing OSC 52 to /dev/tty: {e}"))?;
    tty.flush()
        .map_err(|e| anyhow!("failed flushing OSC 52 to /dev/tty: {e}"))?;
    Ok(())
}

/// Capture the recent scrollback of `pane_id` via `herdr pane read`.
fn herdr_capture_pane(pane_id: &str) -> Result<String> {
    let args = herdr_capture_args(pane_id);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    crate::mux::herdr_cli(&arg_refs)
}

/// Resolve the herdr pane to capture: the launching pane when herdr set it,
/// otherwise the focused pane. Popups get `HERDR_ENV` but not
/// `HERDR_PANE_ID`, and `C-a L` is bound as a popup — so the fallback is
/// the common path, not the edge case.
fn resolve_herdr_pane_id() -> Result<String> {
    if let Ok(p) = env::var("HERDR_PANE_ID") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    let layout = crate::mux::fetch_layout(None)?;
    if layout.focused_pane_id.is_empty() {
        return Err(anyhow!(
            "pick-links: herdr reported no focused pane to capture"
        ));
    }
    Ok(layout.focused_pane_id)
}
```

- [ ] **Step 5: Branch the orchestrator**

Rewrite the head of `pub fn pick_links` in the same file so the three seams dispatch on the detected multiplexer. Replace lines 13-21 (from `pub fn pick_links` through `let rows = detect::parse(&raw);`) with:

```rust
pub fn pick_links(json: bool, enrich_deadline_ms: u64) -> Result<()> {
    let mux = crate::mux::detect();

    // 1. Resolve pane + 2. capture (sync, fallible)
    let (pane_id, raw) = match mux {
        crate::mux::Multiplexer::Tmux => {
            let pane_id = resolve_pane_id()?;
            let raw = capture_pane(&pane_id)?;
            (pane_id, raw)
        }
        crate::mux::Multiplexer::Herdr => {
            let pane_id = resolve_herdr_pane_id()?;
            let raw = herdr_capture_pane(&pane_id)?;
            (pane_id, raw)
        }
        crate::mux::Multiplexer::Unknown => {
            return Err(anyhow!(
                "pick-links: not inside tmux or herdr; nothing to capture"
            ))
        }
    };

    // 3. Detect
    let rows = detect::parse(&raw);
```

Then update the two consumers further down the same function:

- Leave the TUI call as `let action = tui::run(rows)?;` — Task 3 owns that signature change. This task must compile and pass tests on its own.
- The `Yank` arm becomes backend-aware:

```rust
        tui::Action::Yank(row) => {
            match mux {
                crate::mux::Multiplexer::Herdr => yank_osc52(&row.canonical)?,
                _ => yank_to_clipboard(&row.canonical)?,
            }
            println!("{}", row.canonical);
        }
```

Leave the `Open`, `GhWeb`, `Ssh`, and `SwapToPickTui` arms exactly as they are — under herdr the TUI will not produce `Ssh` or `SwapToPickTui` (Task 3), and `Open` is already backend-agnostic.

Finally, delete the now-dead `if env::var("TMUX").is_err()` guard inside the existing `resolve_pane_id` — the `Unknown` arm above owns that error now — leaving:

```rust
fn resolve_pane_id() -> Result<String> {
    if let Ok(p) = env::var("TMUX_PANE") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    // Fallback: ask tmux directly. Reached from display-popup, which sets
    // TMUX but not TMUX_PANE.
    let out = Command::new("tmux")
        .args(["display-message", "-p", "-t", "#{client_active_pane}", "#{pane_id}"])
        .output()
        .map_err(|e| anyhow!("tmux display-message failed: {e}"))?;
    if !out.status.success() {
        return Err(anyhow!("tmux display-message returned nonzero"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
```

- [ ] **Step 6: Run the tests**

Run:

```bash
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test link_picker
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo clippy 2>&1 | tail -5
```

Expected: the three new tests pass; the pre-existing `yank_argv_passes_dash_w_and_single_payload_arg`, `yank_argv_passes_payload_with_spaces_as_one_arg`, and `capture_pane_caps_history_at_300_lines` still pass unchanged. No new clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add rust/tmux_helper/Cargo.toml rust/tmux_helper/Cargo.lock rust/tmux_helper/src/link_picker/mod.rs
git commit -m "feat(rmux_helper): capture and yank via herdr in pick-links"
```

---

### Task 3: TUI action filtering per backend

**Files:**

- Modify: `rust/tmux_helper/src/link_picker/tui.rs` (`Action` consumers, `App`, `run`, `draw`, `handle_key`, `default_action`, test helper at line ~274)
- Modify: `rust/tmux_helper/src/link_picker/mod.rs` (the `tui::run` call site)

**Interfaces:**

- Consumes: `crate::mux::Multiplexer` (Task 1); `pick_links`'s local `mux` binding (Task 2).
- Produces: `pub fn run(rows: Vec<Row>, mux: Multiplexer) -> Result<Action>`; `pub(crate) fn default_action(row: &Row, mux: Multiplexer) -> Action`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `rust/tmux_helper/src/link_picker/tui.rs`:

```rust
    fn server_row() -> Row {
        Row {
            category: Category::Server,
            canonical: "c-5001".into(),
            key: "c-5001".into(),
            repo_or_host: "—".into(),
            context: "ssh c-5001".into(),
            enriched: None,
            count: 1,
            most_recent_line: 0,
        }
    }

    #[test]
    fn server_row_defaults_to_ssh_under_tmux() {
        assert!(matches!(
            default_action(&server_row(), Multiplexer::Tmux),
            Action::Ssh(_)
        ));
    }

    #[test]
    fn server_row_defaults_to_yank_under_herdr() {
        // Ssh is not offered under herdr, so Enter on a host must copy it
        // rather than silently doing nothing.
        assert!(matches!(
            default_action(&server_row(), Multiplexer::Herdr),
            Action::Yank(_)
        ));
    }

    #[test]
    fn s_key_types_into_the_query_under_herdr() {
        // Under tmux `s` fires Ssh; under herdr it must fall through to the
        // generic character arm so `s` is usable as a search character.
        let mut app = App::new(vec![server_row()], Multiplexer::Herdr);
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Char('s'));
        assert!(app.action.is_none(), "must not fire an action under herdr");
        assert_eq!(app.query, "s", "must type into the search query instead");
    }

    #[test]
    fn s_key_fires_ssh_under_tmux() {
        let mut app = App::new(vec![server_row()], Multiplexer::Tmux);
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Char('s'));
        assert!(matches!(app.action, Some(Action::Ssh(_))));
        assert!(app.query.is_empty(), "must not also type into the query");
    }

    #[test]
    fn f2_is_inert_under_herdr() {
        // herdr's own prefix+w is the session picker; swapping to pick-tui
        // would exec a tmux-only TUI.
        let mut app = App::new(vec![server_row()], Multiplexer::Herdr);
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::F(2));
        assert!(app.action.is_none());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test link_picker::tui`
Expected: FAIL — `App::new` takes one argument, `default_action` takes one argument, `Multiplexer` unresolved.

- [ ] **Step 3: Thread the multiplexer through the TUI**

In `rust/tmux_helper/src/link_picker/tui.rs`:

1. Add to the imports at the top: `use crate::mux::Multiplexer;`
2. Add the field to `struct App` (after `show_help: bool`):

```rust
    mux: Multiplexer,
```

3. Change `impl App`'s constructor signature and initializer:

```rust
    fn new(rows: Vec<Row>, mux: Multiplexer) -> Self {
        let mut app = Self {
            rows,
            filtered: Vec::new(),
            categories_present: Vec::new(),
            list_state: ListState::default(),
            query: String::new(),
            drilled_in: None,
            horizontal: true,
            show_help: false,
            action: None,
            mux,
        };
        app.rebuild_filter();
        app
    }
```

4. Change `pub fn run(rows: Vec<Row>) -> Result<Action>` to `pub fn run(rows: Vec<Row>, mux: Multiplexer) -> Result<Action>`, and its body's `let mut app = App::new(rows);` to `let mut app = App::new(rows, mux);`.
5. In `fn on_enter`, change `self.action = Some(default_action(&row));` to `self.action = Some(default_action(&row, self.mux));`.
6. Change `default_action` to:

```rust
/// Default Enter action per category (see spec §Actions → Default).
/// Under herdr, Server/Ip rows fall back to Yank — the Ssh action is
/// tmux-only, and a dead Enter key would be worse than copying the host.
pub(crate) fn default_action(row: &Row, mux: Multiplexer) -> Action {
    match row.category {
        Category::Server | Category::Ip if mux == Multiplexer::Tmux => Action::Ssh(row.clone()),
        _ => Action::Yank(row.clone()),
    }
}
```

7. Guard the two tmux-only key handlers in `handle_key`:

```rust
        KeyCode::F(2) if app.mux == Multiplexer::Tmux => {
            app.action = Some(Action::SwapToPickTui)
        }
```

```rust
        KeyCode::Char('s')
            if mods.is_empty() && app.query.is_empty() && app.mux == Multiplexer::Tmux =>
        {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Ssh(row));
            }
        }
```

The `s` arm must stay **above** the generic `KeyCode::Char(c)` arm so that under tmux it still wins, and under herdr the guard fails and the generic arm types the character.

8. In `fn draw`, make the hint bar backend-aware — replace the `let top = ...` block with:

```rust
    let sess_hint = if app.mux == Multiplexer::Tmux {
        " F2:sess"
    } else {
        ""
    };
    let top = if let Some(cat) = app.drilled_in {
        format!(
            "pick> {}_  │ Links › {}  │ ↑↓ Enter:act ←:back ?:help{}",
            app.query,
            cat.display(),
            sess_hint
        )
    } else {
        format!(
            "pick> {}_  │ ↑↓ Enter:act →:drill y:yank o:open g:gh ?:help{}",
            app.query, sess_hint
        )
    };
```

9. Update the existing test helper `app_with_one_row` (line ~274) so pre-existing tests keep asserting tmux behavior: change `App::new(vec![row])` to `App::new(vec![row], Multiplexer::Tmux)`.

- [ ] **Step 4: Update the call site**

In `rust/tmux_helper/src/link_picker/mod.rs`, change `let action = tui::run(rows)?;` to:

```rust
    let action = tui::run(rows, mux)?;
```

- [ ] **Step 5: Run the tests**

Run:

```bash
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo clippy 2>&1 | tail -5
```

Expected: the 5 new tests pass; every pre-existing `tui` test still passes. No new clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add rust/tmux_helper/src/link_picker/tui.rs rust/tmux_helper/src/link_picker/mod.rs
git commit -m "feat(rmux_helper): filter pick-links actions by multiplexer"
```

---

### Task 4: herdr keybinding and docs

**Files:**

- Modify: `config/herdr/config.toml` (the `[[keys.command]]` section — add the new block, and add `-u TMUX` to the existing `prefix+/` block)
- Modify: `config/herdr/README.md` (Scrollback and popups table)
- Modify: `rust/tmux_helper/CLAUDE.md` (Commands list)

**Interfaces:**

- Consumes: the installed binary (Task 5 installs it; the binding is inert until then).
- Produces: `prefix+shift+l` in herdr.

- [ ] **Step 1: Add the binding**

In `config/herdr/config.toml`, add immediately after the existing `prefix+/` layout block (still **below every plain `[keys]` entry** — the file's own NOTE comment explains why; TOML scopes bare keys after an array-of-tables header into that table and `herdr config check` does not catch it):

```toml
# tmux: C-a L -> rmux_helper pick-links (scrollback link/PR/host picker).
# Popup at 95%x95% matches the tmux display-popup binding. `env -u TMUX
# -u TMUX_PANE` forces herdr detection: if the herdr server was launched
# from inside tmux it would leak those vars into this command, and tmux
# wins the detection precedence.
[[keys.command]]
key = "prefix+shift+l"
type = "popup"
command = "env -u TMUX -u TMUX_PANE rmux_helper pick-links"
width = "95%"
height = "95%"
```

- [ ] **Step 2: Add `-u TMUX` to the existing `third` binding**

In the same file, change the `prefix+/` block's command from:

```toml
command = "env HERDR_ENV=1 rmux_helper third"
```

to:

```toml
command = "env -u TMUX -u TMUX_PANE HERDR_ENV=1 rmux_helper third"
```

Required by Task 1's detection change: `TMUX` alone now means tmux, so a herdr server launched from inside tmux would otherwise send `prefix+/` down the tmux path. Update that block's comment to say it clears both variables.

- [ ] **Step 3: Validate the config**

`~/.config/herdr/config.toml` symlinks to the **primary** `~/settings` checkout, not this worktree — so `herdr config check` validates the wrong file unless the symlink is swapped. Validate the branch's file with a guaranteed restore:

```bash
LIVE=~/.config/herdr/config.toml
ORIG=$(readlink "$LIVE")
trap 'ln -sfn "$ORIG" "$LIVE"' EXIT
ln -sfn "$PWD/config/herdr/config.toml" "$LIVE"
herdr config check
ln -sfn "$ORIG" "$LIVE"
trap - EXIT
readlink "$LIVE"
```

Expected: `config: ok`, then the readlink prints the original primary-checkout path. Do **not** run `herdr server reload-config` while swapped. If `prefix+shift+l` is rejected, try `key = "prefix+L"` and record which form was accepted in the README row.

Also verify TOML scoping did not regress:

```bash
python3 -c "
import tomllib
d = tomllib.load(open('config/herdr/config.toml','rb'))
k = d['keys']
need = ['copy_mode','edit_scrollback','open_notification_target','navigate_workspace_up','navigate_workspace_down','navigate_pane_left','navigate_pane_down','navigate_pane_up','navigate_pane_right']
missing = [x for x in need if x not in k]
assert not missing, missing
assert all(set(c) <= {'key','type','command','width','height'} for c in k['command']), k['command']
print('TOML scoping OK:', len(k['command']), 'command blocks')
"
```

Expected: `TOML scoping OK: 4 command blocks`.

- [ ] **Step 4: Document**

In `config/herdr/README.md`, add to the "Scrollback and popups" table (after the `prefix+e` row):

```markdown
| `prefix+shift+l` | scrollback link picker, 95% × 95%        |
```

In `rust/tmux_helper/CLAUDE.md`, add to the Commands list after the `third` bullet:

```markdown
- `pick-links` - Scrollback link/PR/host picker. Works under tmux and herdr (auto-detected); under herdr the ssh and F2-swap actions are not offered and yank goes out as OSC 52
```

- [ ] **Step 5: Commit**

```bash
git add config/herdr/config.toml config/herdr/README.md rust/tmux_helper/CLAUDE.md
git commit -m "config(herdr): bind prefix+shift+l to rmux_helper pick-links"
```

---

### Task 5: Install, smoke, tmux regression, PR

**Files:**

- No source changes expected. If smoke or regression fails, stop and report — do not patch code (the controller decides fix-forward).

**Interfaces:**

- Consumes: everything above.
- Produces: installed binary, PR.

- [ ] **Step 1: Install**

Run from `rust/tmux_helper/`:

```bash
CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo install --path . --force
rmux_helper pick-links --help | head -5
```

Expected: installs in well under a minute; help prints.

- [ ] **Step 2: herdr smoke — headless `--json` against a real pane**

The executing shell has `HERDR_PANE_ID` set. `--json` short-circuits before the TUI, so this is safe headless:

```bash
rmux_helper pick-links --json | python3 -m json.tool | head -20
rmux_helper pick-links --json | python3 -c "import json,sys; rows=json.load(sys.stdin); print('rows:', len(rows)); print('categories:', sorted({r['category'] for r in rows}))"
```

Expected: valid JSON, one or more rows, categories drawn from this session's scrollback (it contains GitHub PR URLs, so `pull_request` or `link` should appear). Zero rows is a FAILURE for this step — it means capture returned nothing.

Also prove the popup path (no `HERDR_PANE_ID`, focused-pane fallback):

```bash
env -u HERDR_PANE_ID HERDR_ENV=1 rmux_helper pick-links --json | python3 -c "import json,sys; print('fallback rows:', len(json.load(sys.stdin)))"
```

Expected: a row count, no error. This reads the focused tab, which is safe — it only reads.

- [ ] **Step 3: Prove the capture reaches herdr's scrollback depth**

```bash
rmux_helper pick-links --json | python3 -c "import json,sys; rows=json.load(sys.stdin); print('max line offset seen:', max((r['most_recent_line'] for r in rows), default=0))"
```

Expected: a number greater than the visible pane height (~56) if this session's scrollback holds older links, confirming `--lines 300` is really reaching back. If it is at or below 56, note it in the report rather than failing — it may simply mean all detected links are recent.

- [ ] **Step 4: tmux regression — the picker still works there**

```bash
tmux new-session -d -s links-smoke -x 200 -y 50
tmux send-keys -t links-smoke 'echo https://github.com/idvorkin/settings/pull/101 && echo ssh c-5001' Enter
sleep 1
tmux send-keys -t links-smoke 'rmux_helper pick-links --json > /tmp/links-smoke.json 2>/tmp/links-smoke.err; echo done' Enter
sleep 3
python3 -c "import json; rows=json.load(open('/tmp/links-smoke.json')); cats=sorted({r['category'] for r in rows}); print('tmux rows:', len(rows), 'categories:', cats); assert 'pull_request' in cats, cats; assert 'server' in cats, cats; print('tmux regression OK')"
tmux kill-session -t links-smoke
```

Expected: `tmux regression OK`. This exercises the untouched tmux resolve + capture path end to end.

- [ ] **Step 5: Full suite and pre-commit**

Run from the repo root:

```bash
(cd rust/tmux_helper && CARGO_TARGET_DIR=$HOME/.cache/cargo-target cargo test)
pre-commit run --all-files 2>&1 | tail -20
```

Expected: cargo tests all pass. For pre-commit, any failure must be triaged against this branch's file footprint (`git diff --name-only main...HEAD`) — failures on files this branch never touched are pre-existing repo backlog; note them and move on. Failures on this branch's files must be fixed and committed.

- [ ] **Step 6: Report the manual verification the controller must surface**

Two things cannot be verified headless — name them in the report:

1. `C-a L` in a live herdr tab renders the popup and is navigable.
2. Enter/`y` on a row lands the URL on the Mac clipboard via herdr's OSC 52 forwarding.

- [ ] **Step 7: Push and open the PR**

```bash
git log --oneline main..HEAD
gh pr list --head herdr-link-picker --state all --json number,state
git push -u origin herdr-link-picker
gh pr create --base main --title "pick-links works under herdr" --body "$(cat <<'EOF'
## Summary
- `rmux_helper pick-links` auto-detects tmux vs herdr and works in both; `prefix+shift+l` bound in herdr as a 95%×95% popup
- Shared `src/mux.rs` now owns multiplexer detection, the `herdr` CLI shell, and the `pane layout` types — `third` and `pick-links` both consume it instead of duplicating
- Detection now treats `TMUX` (not just `TMUX_PANE`) as tmux, since `display-popup` drops `TMUX_PANE`; both herdr bindings clear `TMUX` and `TMUX_PANE`
- Under herdr: yank emits OSC 52 (herdr forwards it to the client clipboard), ssh and F2-swap are not offered, and Enter on a host copies it

Spec: docs/superpowers/specs/2026-08-02-herdr-link-picker-design.md
Plan: docs/superpowers/plans/2026-08-02-herdr-link-picker.md

## Test plan
- [x] Unit tests: detection precedence, herdr capture argv order, OSC 52 payload bytes, per-backend action defaults
- [x] herdr headless smoke: `pick-links --json` returns rows from real scrollback, including the no-`HERDR_PANE_ID` popup path
- [x] tmux regression: `pick-links --json` in a scratch session still detects PR + host rows
- [ ] Igor: `C-a L` popup renders under herdr; yank reaches the Mac clipboard

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

If `gh pr list` shows a MERGED PR for this branch, stop and report instead of pushing. If the push is denied, fork per the repo CLAUDE.md: `gh repo fork --remote-name fork && git push fork herdr-link-picker && gh pr create --head idvorkin-ai-tools:herdr-link-picker --base main ...`.
