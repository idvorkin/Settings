# rmux_helper pick-links Specification

## Invocation

- `rmux_helper pick-links` — launches the TUI picker popup. Binds to `C-a L` in tmux.
- `rmux_helper pick-links --json` — emits detected items as JSON to stdout and exits (no enrichment, no TUI). Useful for scripts and tests.
- `rmux_helper pick-links --enrich-deadline-ms N` — overrides the default 3-second enrichment deadline. `0` disables gh enrichment entirely.

## Categories (fixed display order)

1. Commands — commands an agent asked the human to run (see [Command detection](#command-detection))
2. Pull Requests — `github.com/OWNER/REPO/pull/N`
3. Issues — `github.com/OWNER/REPO/issues/N`
4. Commits — `github.com/OWNER/REPO/commit/SHA`
5. Files — `github.com/OWNER/REPO/(blob|tree)/REF/PATH`
6. Repos — `github.com/OWNER/REPO` (bare)
7. Gists — `gist.github.com/OWNER/ID`
8. Blog — host ∈ `BLOG_HOSTS` (v1: `idvorkin.github.io`)
9. Other links — any `https?://` not matched above
10. Servers — `ssh` context + Tailscale (`c-NNNN`, `*.ts.net`)
11. IPs — IPv4 with version-string suppression

## Command detection

Heuristics only, no model call. Tuned to how Claude Code draws into a pane: inline-code backticks are stripped, fenced code blocks lose their fences, and the agent's own tool calls render as `⎿  $ cmd`. A command is detected when:

1. A line starts with `! cmd` or `$ cmd`, after skipping gutter glyphs (`⏺ ❯ ▎ │ >`), list markers (`- `, `* `, `• `, `1. `) and backticks wrapping the whole line. The prompt echo of a human's own `! cmd` counts too.
2. `` `! cmd` `` appears anywhere in a line (agents that keep backticks).
3. It is one of the lines right after a cue line: a line ending in `:` that contains `run`, `paste`, `type`, `execute` or `enter` ("Run this:", "type this in the prompt:"). One blank line may sit between the cue and the block. The block ends at a blank line or the first line that is not a command, and is capped at 8 lines.

Lines starting with `⎿` (tool calls and their output) never yield commands. Every candidate must look like a command: its first word, past `VAR=x` assignments and `sudo`, starts with `/`, `.` or `~`, is a shell builtin (`cd`, `export`, `source`, …), or names a file on `PATH`. That check is what keeps prose after a cue line out.

Hard-wrapped commands are joined back: a following line indented deeper than the command's line, or any line after a trailing `\`, is appended with one space.

Row columns: key = the marker (`!`, `$`, or `run` for a cue block), repo-or-host = `—`, title = the command itself (the whole command is the payload, so it is not stripped from its context line the way URLs are). The copied text is the command without its `!`/`$` marker. Detection runs under both tmux and herdr.

## Dedup

Row key = `(category, canonical)`. Duplicates collapse into one row with a `×N` count.

## Ordering

Categories: fixed order above. Within a category: most-recent line first (closest to bottom).

## Columns

| Col           | Color                        | Content                                                     |
| ------------- | ---------------------------- | ----------------------------------------------------------- |
| key           | LightYellow                  | `#N`, `SHA[:7]`, filename, host, IP                         |
| repo-or-host  | LightGreen                   | repo name, host, or `—`                                     |
| glyph + title | state-colored + LightMagenta | state glyph from enriched gh view; `context` line otherwise |
| count         | LightCyan                    | `×N` only when N > 1                                        |

## Navigation

- `↑`/`↓` or `C-p`/`C-n`: move selection (headers are selectable — do not skip)
- `→` or `Enter` on category header: drill into that category
- `1`–`9`: jump into Nth non-empty category (query must be empty)
- `←`: drill out (in drilled-in mode)
- `Esc`: drill out (first press) or quit (if already flat)
- `Tab` / `S-Tab`: reserved, no-op
- `F2`: swap to `pick-tui` (bidirectional). **tmux only** — under herdr the key is inert (no-op), since `pick-tui` execs a tmux-only picker and herdr already has its own session picker (prefix+w).
- `?` or `F1`: toggle the modal help overlay. Any key dismisses it (the
  dismissing key is consumed, so it cannot double as a navigation or action
  key). `?` is intercepted before the generic filter-query char handler, so
  it does not type into the search field.
- `C-l`: toggle layout (horizontal/vertical)
- `C-c`: clear query or quit

## Actions

Default `Enter` (on leaf):

- Commands → OSC 52 yank, under every backend and OS. A command is copied to paste, never run and never opened.
- URL categories → **on macOS builds**: `open` (same as `o`), under both tmux and herdr. **Elsewhere** (e.g. the Linux devvm, which has no local browser): OSC 52 yank + print URL to stdout
- Servers / IPs → **under tmux**: `tmux new-window -t "$pane_id" -c '#{pane_current_path}' "ssh <host>"`. **Under herdr**: `Ssh` is not offered (opening a tmux window isn't meaningful there), so these rows default to the same OSC 52 yank as URL categories — a dead `Enter` key would be worse than copying the host.

Override keys (query must be empty — lowercase letters otherwise type into search):

- `y` — yank (OSC 52)
- `o` — `open`/`xdg-open`. Inert on Commands rows.
- `g` — `gh <kind> view --web -R OWNER/REPO <id>` (GitHub rows only)
- `s` — force ssh. **tmux only** — under herdr the key guard requires tmux, so `s` falls through to the generic filter-query handler and types into the search field instead.

The OSC 52 yank mechanism itself differs by backend: under tmux the payload goes out via `tmux set-buffer -w` (tmux forwards OSC 52 to attached clients); under herdr `pick-links` writes the OSC 52 escape directly to `/dev/tty` for herdr to parse out of the pane's pty and forward to the client's clipboard.

## Filtering

Token-based substring match. Tokens split on whitespace; letter/digit boundaries split once per transition (multi-digit tokens stay whole — Divergence 1 from `pick-tui`).

Category tag `cmd`/`pr`/`issue`/`commit`/`file`/`repo`/`gist`/`blog`/`link`/`server`/`ip` prefixes each row's search string with a `\x1f` separator (Divergence 2).

Digit-only tokens match the `key` column only.

## Divergences from `PICKER_SPEC.md`

1. **Multi-digit tokens are NOT split per digit.** `pick-tui` splits `14` → `[1,4]` to match tmux index `1;4`; the link picker treats `14` as one token because PR number `14` must not match `#1;4`.
2. **Tag prefix uses `\x1f` separator.** Ensures the category tag is matched as a whole word, not as a substring leaking into titles.
3. **Headers are selectable.** `pick-tui` skips session headers when navigating; `pick-links` keeps category headers selectable so `Enter` on a header can drill into that category.

## Cross-picker shortcut

**tmux only.** `F2` cleanly tears down the TUI (`disable_raw_mode` + `LeaveAlternateScreen` + drop terminal + flush) then `execvp`s the sibling binary with `TMUX_PANE` forwarded. OSC 52 is NOT written on `F2` — it's a swap, not an action. Under herdr, `F2` is gated on the detected multiplexer and never reaches this code path at all.

## OSC 52 timing

Common to both backends: TUI exits → `disable_raw_mode` → `LeaveAlternateScreen` → drop `Terminal` → flush stdout/stderr (all inside `tui::run`, before the action is dispatched).

Backend-specific dispatch after that:

- **tmux**: spawn `tmux set-buffer -w <payload>` (the raw payload, not a pre-built OSC 52 escape — tmux constructs and forwards the OSC 52 sequence to attached clients itself) → `exit(0)`.
- **herdr**: open `/dev/tty` → write `\e]52;c;<base64>` terminated with **BEL** (`\x07`), not ST (`\e\\`) → flush tty → `exit(0)`.

## Empty state

When scrollback contains no detectable items, the TUI is not entered. `pick_links` prints `pick-links: no links, servers, or IPs in scrollback` to stderr and exits 0.

## Scrollback capture

`pick-links` scans the current pane's scrollback only — no cross-pane, no cross-session, no external sources. The multiplexer is auto-detected and capture takes one of two paths:

- **tmux**: the pane is resolved from `$TMUX_PANE`, falling back to `tmux display-message -p '#{client_active_pane}'`. Scrollback comes from `tmux capture-pane -e -J -S -300 -E -`.
- **herdr**: the pane is resolved from `$HERDR_PANE_ID`, falling back to the layout's focused pane. Scrollback comes from `herdr pane read <id> --source recent-unwrapped --lines 300 --format ansi`.

History depth is **capped at 300 lines above the visible pane top** on both backends. The visible pane content is always included in full; the cap only limits how deep into scrollback we go. This keeps results relevant to recent work — a 50 000-line `history-limit` buffer produces stale context from days-old sessions that drowns real results in noise. 300 lines is roughly several screens of recent scrollback.

Both backends join soft-wrapped lines so URLs that wrapped across terminal rows read back whole (tmux's `-J`; herdr's `recent-unwrapped` source). Both capture with escapes so OSC 8 hyperlinks keep their target: an agent printing `#14` or `chop#14` as a link to a PR is detected as that PR, exactly what a click would open. `flatten_ansi` then rewrites each hyperlinked span to `text URL` (just `text` when it already shows the URL) and strips every other escape before detection — raw `\x1b` sequences leak through ratatui's cell rendering into the popup pty and corrupt the display. herdr 0.9.1 drops OSC 8 targets from `pane read` (requested upstream in herdrdev/herdr discussion #4235), so today only tmux recovers them. Under herdr, `pick-links` instead guesses the repo for GitHub short refs (`#14`, `chop#14`, `owner/repo#14`, `PR#14`) from the scrollback's GitHub URLs, the pane's working-directory repo, and `~/gits`, skipping ambiguous names; see `src/link_picker/shortref.rs` for the resolution rules.
