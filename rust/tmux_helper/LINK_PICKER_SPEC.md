# rmux_helper pick-links Specification

## Categories (fixed display order)

1. Pull Requests — `github.com/OWNER/REPO/pull/N`
2. Issues — `github.com/OWNER/REPO/issues/N`
3. Commits — `github.com/OWNER/REPO/commit/SHA`
4. Files — `github.com/OWNER/REPO/(blob|tree)/REF/PATH`
5. Repos — `github.com/OWNER/REPO` (bare)
6. Blog — host ∈ `BLOG_HOSTS` (v1: `idvorkin.github.io`)
7. Other links — any `https?://` not matched above
8. Servers — `ssh` context + Tailscale (`c-NNNN`, `*.ts.net`)
9. IPs — IPv4 with version-string suppression

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
- `F2`: swap to `pick-tui` (bidirectional)
- `F1`: help (reserved, TODO v1.1)
- `C-l`: toggle layout (horizontal/vertical)
- `C-c`: clear query or quit

## Actions

Default `Enter` (on leaf):

- URL categories → OSC 52 yank + print URL to stdout
- Servers / IPs → `tmux new-window -t "$pane_id" -c '#{pane_current_path}' "ssh <host>"`

Override keys (query must be empty — lowercase letters otherwise type into search):

- `y` — yank (OSC 52)
- `o` — `open`/`xdg-open`
- `g` — `gh <kind> view --web -R OWNER/REPO <id>` (GitHub rows only)
- `s` — force ssh

## Filtering

Token-based substring match. Tokens split on whitespace; letter/digit boundaries split once per transition (multi-digit tokens stay whole — Divergence 1 from `pick-tui`).

Category tag `pr`/`issue`/`commit`/`file`/`repo`/`blog`/`link`/`server`/`ip` prefixes each row's search string with a `\x1f` separator (Divergence 2).

Digit-only tokens match the `key` column only.

## Divergences from `PICKER_SPEC.md`

1. **Multi-digit tokens are NOT split per digit.** `pick-tui` splits `14` → `[1,4]` to match tmux index `1;4`; the link picker treats `14` as one token because PR number `14` must not match `#1;4`.
2. **Tag prefix uses `\x1f` separator.** Ensures the category tag is matched as a whole word, not as a substring leaking into titles.
3. **Headers are selectable.** `pick-tui` skips session headers when navigating; `pick-links` keeps category headers selectable so `Enter` on a header can drill into that category.

## Cross-picker shortcut

`F2` cleanly tears down the TUI (`disable_raw_mode` + `LeaveAlternateScreen` + drop terminal + flush) then `execvp`s the sibling binary with `TMUX_PANE` forwarded. OSC 52 is NOT written on `F2` — it's a swap, not an action.

## OSC 52 timing

Write sequence, in order: TUI exits → `disable_raw_mode` → `LeaveAlternateScreen` → drop `Terminal` → flush stdout/stderr → open `/dev/tty` → write `\e]52;c;<base64>\e\\` → flush tty → `exit(0)`.
