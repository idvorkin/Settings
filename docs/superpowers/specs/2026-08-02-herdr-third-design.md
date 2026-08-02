# rmux_helper `third` under herdr — design

Date: 2026-08-02
Status: approved by Igor (brainstorm in session), implementation pending

## Goal

`rmux_helper third` works in both tmux and herdr, auto-detecting which
multiplexer it is running under. `prefix+/` in herdr toggles even ↔ 1/3–2/3,
matching the tmux `C-a /` muscle memory. Also folds in build-speed fixes for
the rust/tmux_helper iteration loop.

## Scope

- **In**: bare `third` toggle under herdr; multiplexer auto-detection;
  `prefix+/` binding in `config/herdr/config.toml`; docs; build-speed fixes.
- **Out**: the `third "<command>"` argument form under herdr (tmux-only for
  now — herdr already covers tig/lazygit via popup bindings); porting other
  commands (`rotate`, `side-edit`, …); a general multiplexer backend trait
  (revisit when a second command needs herdr support).

## Detection

New helper `detect_multiplexer()`:

1. `TMUX_PANE` set → **tmux**. Wins when nested: tmux running inside a herdr
   pane means tmux is what the user is looking at.
2. else `HERDR_PANE_ID` or `HERDR_ENV` set → **herdr**.
3. else → neither; keep today's graceful no-op behavior.

`third()` branches on this at the top. The tmux code path is unchanged.

## Herdr behavior (mirrors tmux semantics)

1. Resolve the caller pane from `HERDR_PANE_ID`; fall back to
   `herdr pane current` (focused pane — correct for a keybinding, and covers
   `type = "shell"` bindings if they don't inject pane env; verify
   empirically).
2. Read `herdr pane layout` (JSON rects) for the caller's tab.
3. **1 pane** → split side-by-side with the left pane at ~1/3
   (`herdr pane split --direction right --ratio …`, cwd inherited from the
   caller pane). One keypress lands directly in the third layout.
4. **2 panes** → orientation from rects (different `x` → side-by-side; use
   width, else height). Toggle state is **derived, not stored**: if the
   first (left/top) pane is already ≈1/3 of the tab (25–40% band) → resize to
   even; otherwise → resize to 1/3. Left/top pane gets the 1/3, same as tmux.
5. **3+ panes** → no-op, same as the tmux path.

No persistent state under herdr (herdr has no `@option` store; deriving from
the live layout is self-correcting).

### Empirical unknowns to verify before the code hardens

- `herdr pane resize --amount <FLOAT>` semantics: delta vs absolute fraction.
- `herdr pane split --ratio <FLOAT>`: which side the ratio applies to.
- Whether `type = "shell"` key bindings inherit `HERDR_PANE_ID` /
  `HERDR_ENV` (fallback path covers the no case).
- `prefix+/` key syntax accepted by herdr config.

## Code structure

- New module `src/herdr_third.rs` (follows the `agent_continue.rs`
  humble-object precedent):
  - **Pure core**: `plan_third(layout) -> ThirdAction` where `ThirdAction` is
    `Split { … } | Resize { pane, target_fraction } | Noop`. Unit-tested
    against JSON layout fixtures. serde structs for the `pane layout` JSON.
  - **Thin shell**: invokes the `herdr` CLI (PATH), parses with serde_json
    (existing dep), executes the planned action.
- `main.rs`: `detect_multiplexer()` + a `match` at the top of `third()`.

## Config + docs

- `config/herdr/config.toml`:

  ```toml
  [[keys.command]]
  key = "prefix+/"
  type = "shell"
  command = "rmux_helper third"
  ```

- `config/herdr/README.md`: add the binding to the keymap tables.
- `rust/tmux_helper/CLAUDE.md`: note herdr support for `third` + the new dev
  loop (below).

## Build speed

- `rust/tmux_helper/Cargo.toml` `[profile.release]`: `lto = "thin"`, drop
  `codegen-units = 1`, keep `strip = true`. (Fat LTO + one codegen unit is a
  distribution profile; it dominates warm rebuild time and buys nothing
  perceptible for a subprocess-bound CLI.)
- `CARGO_TARGET_DIR=~/.cache/cargo-target` exported from the shared zsh
  config so all worktrees (herdr-created ones especially) share one dep
  cache — new-worktree builds drop from minutes (160 crates cold) to seconds.
- Dev loop documented in `rust/tmux_helper/CLAUDE.md`: `cargo build` + run
  `$CARGO_TARGET_DIR/debug/rmux_helper` while iterating;
  `cargo install --path . --force` only at live smoke-test time.

## Error handling

- `herdr` CLI missing or socket dead → message on stderr, nonzero exit.
- Malformed layout JSON → anyhow error.
- No silent fallback from herdr to the tmux path.

## Testing

- Unit tests on `plan_third` fixtures: single pane; even 2-pane (both
  orientations); third-active (resize back to even); manually-resized 40/60
  (→ apply third); 3 panes (no-op).
- Manual smoke: `C-a /` cycle in a herdr tab (1 pane → split; toggle back and
  forth). Regression: tmux `C-a /`, `:ttig`, `:tgdiff` unchanged.
