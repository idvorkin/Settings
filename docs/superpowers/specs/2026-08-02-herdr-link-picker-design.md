# Scrollback link picker under herdr — design

Date: 2026-08-02
Status: approved by Igor (brainstorm in session), implementation pending

## Goal

`rmux_helper pick-links` (the `C-a L` scrollback link picker) works under
herdr as well as tmux, sharing one orchestration path. Bind it to
`prefix+shift+l` in herdr. Extract the multiplexer plumbing both this and
`third` need into one module rather than duplicating it.

Prior art: [`2026-04-12-scrollback-link-picker-design.md`](2026-04-12-scrollback-link-picker-design.md)
(the tmux picker) and [`2026-08-02-herdr-third-design.md`](2026-08-02-herdr-third-design.md)
(the first herdr port).

## Scope

- **In**: multiplexer auto-detection shared by `third` and `pick-links`;
  herdr capture / pane-resolve / yank; action filtering under herdr;
  `prefix+shift+l` binding; docs.
- **Out**: the Ssh action and the F2 swap-to-`pick-tui` action under herdr
  (herdr's `prefix+w` is the native picker); porting other commands;
  enrich/detect/TUI logic changes — those are backend-agnostic already.

## Findings from herdr 0.7.5 source (do not re-derive)

Cloned `github.com/herdrdev/herdr` at `d742e51` and read the relevant paths:

1. **`herdr pane read` takes the pane id FIRST** (`src/cli/pane.rs:443-451`):
   `herdr pane read <pane_id> [--source visible|recent|recent-unwrapped]
   [--lines N]`. An earlier session passed the args out of order, got
   `unknown option: recent`, and wrongly concluded the server was too old.
   Verified live on a 500-line shell pane: `--lines 400` returns 400 lines,
   and `--source recent-unwrapped` joins soft-wrapped lines. This is the
   direct equivalent of tmux `capture-pane -J -S -300 -E -`.
2. **Popups omit `HERDR_PANE_ID`** (`src/app/popup.rs:154`,
   `PaneLaunchEnv::without_pane_identity()`), but do set `HERDR_ENV=1`
   (`src/pane.rs:117`). Same shape as tmux popups, which drop `TMUX_PANE`
   but keep `TMUX` — so both backends need a focused-pane fallback.
3. **herdr captures OSC 52 from any pane and forwards it**
   (`src/pane.rs:1851` → `AppEvent::ClipboardWrite`). So a herdr yank is
   just the picker emitting OSC 52 itself; there is no herdr equivalent of
   `tmux set-buffer -w` and none is needed.

## Shared module: `src/mux.rs`

New module holding what both features need, so neither owns it:

- `pub enum Multiplexer { Tmux, Herdr, Unknown }`
- `pub fn detect() -> Multiplexer` — moved out of `main.rs`.
- `pub fn herdr_cli(args: &[&str]) -> Result<String>` — promoted from
  `herdr_third.rs`, which then consumes it. This is the DRY payoff: one
  place shells out to `herdr`, one place decides which multiplexer we are
  in.

### Detection change: `TMUX` counts, not just `TMUX_PANE`

`detect_multiplexer` currently keys on `TMUX_PANE` alone. That is
sufficient for `third` (invoked via `run-shell`, which sets it) but wrong
for `pick-links`, which runs inside a tmux `display-popup` where
`TMUX_PANE` is absent and only `TMUX` is set. New rule, in order:

1. `TMUX_PANE` or `TMUX` set → **Tmux** (nested tmux-inside-herdr still
   means tmux is what the user is looking at).
2. else `HERDR_PANE_ID` or `HERDR_ENV` set → **Herdr**.
3. else → **Unknown** (callers keep today's behavior: `third` falls
   through to the tmux path; `pick-links` errors as it does now).

**Consequence — the existing `third` binding must be updated.** It
currently reads `env -u TMUX_PANE HERDR_ENV=1 rmux_helper third`. Once
`TMUX` also implies tmux, a herdr server launched from inside tmux would
leak `TMUX` into shell bindings and re-break the case that flag was added
to fix. Both herdr bindings therefore clear **both** variables:
`env -u TMUX -u TMUX_PANE HERDR_ENV=1 rmux_helper <cmd>`.

## Per-backend seams in `link_picker`

`link_picker/mod.rs` keeps its current orchestration shape (resolve →
capture → detect → enrich → TUI → dispatch). Five touchpoints become
backend-dispatched, each with a **pure argv/payload builder** that is unit
tested, mirroring the `action_args` pattern from `herdr_third`:

| Step | tmux (unchanged) | herdr |
| --- | --- | --- |
| Resolve pane | `TMUX_PANE`, else `display-message -p -t '#{client_active_pane}' '#{pane_id}'` | `HERDR_PANE_ID`, else `focused_pane_id` from `herdr pane layout` |
| Capture | `capture-pane -p -J -S -300 -E - -t <pane>` | `pane read <pane> --source recent-unwrapped --lines 300` |
| Yank | `tmux set-buffer -w <payload>` | write OSC 52 to `/dev/tty` |
| Ssh | `tmux new-window` | not offered |
| Swap to `pick-tui` | `execvp` | not offered |

`SCROLLBACK_HISTORY_LINES = 300` stays one constant feeding both builders.

### Yank under herdr

Emit `ESC ] 52 ; c ; <base64(payload)> BEL` to `/dev/tty` after
`tui::run` has restored the terminal. `/dev/tty` rather than stdout
because stdout may be piped and the sequence must reach the pty herdr is
parsing. No tmux passthrough wrapping — that is a tmux-only concern. If
`/dev/tty` cannot be opened, return an error rather than silently
succeeding. Adds one dependency: `base64 = "0.22"`.

The existing `yank_to_clipboard_args` test (asserting `-w` and
single-argument payload) stays untouched and keeps guarding the tmux path.

### Action filtering under herdr

`App` gains a `mux: Multiplexer` field, threaded in from `pick_links` via
`tui::run(rows, mux)`:

- Hint bar drops `F2:sess` under herdr.
- `F2` becomes a no-op under herdr (it has no query semantics to fall back
  to).
- The `s` handler (Ssh) is guarded on `mux == Tmux`, so under herdr `s`
  falls through to the generic character arm and types into the search
  query like any other letter. A dead key would be strictly worse: `s` is
  currently unavailable as the first character of a search precisely
  because it triggers Ssh.
- `default_action(row, mux)` — Server/Ip rows currently default to
  `Action::Ssh`. Under herdr they default to `Action::Yank`, so Enter on a
  host copies it instead of doing nothing. This is the one behavioral
  difference a user will notice, and it is deliberate.

## Config and docs

`config/herdr/config.toml`, added with the other `[[keys.command]]` blocks
(**below every plain `[keys]` entry** — see the warning comment already in
that file; TOML array-of-tables scoping silently swallows bare keys placed
after it and `herdr config check` does not catch it):

```toml
[[keys.command]]
key = "prefix+shift+l"
type = "popup"
command = "env -u TMUX -u TMUX_PANE rmux_helper pick-links"
width = "95%"
height = "95%"
```

Matches the tmux binding's 95%×95% popup. The existing `prefix+/` `third`
binding gains `-u TMUX` in the same commit. Docs: a row in the
`config/herdr/README.md` keymap table and a note in
`rust/tmux_helper/CLAUDE.md` that `pick-links` is multiplexer-aware.

`PICKER_SPEC.md` covers `pick-tui`, not `pick-links`, so it needs no
update.

## Error handling

- herdr CLI missing, socket dead, or malformed layout JSON → error with
  context; no fallback to the tmux path.
- `Multiplexer::Unknown` in `pick-links` → today's message
  (`not inside tmux; nothing to capture`), reworded to name both
  multiplexers.

## Testing

Unit tests (pure, no multiplexer required):

- herdr capture argv: pane id first, `--source recent-unwrapped`,
  `--lines 300` derived from the shared constant.
- OSC 52 payload bytes: correct prefix/terminator, base64 correctness,
  payloads containing spaces and non-ASCII.
- `focused_pane_id` extracted from a captured `pane layout` JSON fixture.
- `default_action` returns Yank for Server/Ip under herdr, Ssh under tmux.
- Detection precedence: `TMUX` alone → Tmux; `HERDR_ENV` alone → Herdr;
  `TMUX` + `HERDR_ENV` → Tmux; neither → Unknown. Tested through a pure
  function taking the four values as parameters, so no environment
  mutation and no cross-test races.

Integration: `rmux_helper pick-links --json` run inside a herdr pane
(headless, no TUI) must emit rows detected from that pane's real
scrollback.

Manual, needs Igor: `C-a L` popup renders and is navigable under herdr;
Enter/`y` on a row lands the URL on the Mac clipboard through herdr's OSC
52 forwarding. Regression: the tmux `C-a L` popup still works unchanged.
