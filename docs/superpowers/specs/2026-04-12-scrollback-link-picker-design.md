# Scrollback Link Picker Design

## Summary

A ratatui-based TUI picker that scans the current tmux pane's scrollback for
actionable items (GitHub PRs, issues, commits, files, repos, blog posts, other
URLs, ssh servers, IP addresses), groups them by category, and lets you
fuzzy-filter, drill into one category, and act on the selected row. Invoked
from tmux (bound to `C-a L`) or as a standalone shell command.

Sits alongside the existing `rmux_helper pick-tui` session picker — same
chrome, same keymap family, same dynamic-column layout — but over scrollback
items instead of tmux units.

## Goals

- **Find any recent link in seconds**, without manual scrollback + mouse selection.
- **Meaningful titles via parallel `gh` enrichment** — GitHub PRs, issues,
  and commits show their real title, state, and author pulled from `gh`
  calls fanned out concurrently, bounded by a global deadline. The TUI
  shows rich titles when enrichment lands in time and falls back to the
  scrollback `context` line when it doesn't.
- **Context-aware actions** — `ssh` a server, open a URL, view a PR in the browser.
- **Works from any tmux pane** — Mac host tmux, devvm tmux, nested tmux — because the underlying capture is `tmux capture-pane`, not iTerm-specific. Non-tmux terminals are unsupported in v1 (see Error Handling: "Not inside tmux" exits with a message); an iTerm2 Python-API fallback is tracked under v2 roadmap.
- **Reachable from `pick-tui`** — pressing `F2` in the session picker
  jumps straight into the link picker for the currently highlighted pane,
  without returning to the shell. Bidirectional: `F2` in the link picker
  returns to the session picker.

## Non-goals (v1)

- Fetching a URL's content and walking its outbound links ("recursive link
  walking"). v2, if ever.
- Blog-post title scraping (HTML `<title>` fetch). Blog rows show URL path
  only in v1.
- IPv6 address detection.
- Bare commit SHAs (without a surrounding URL). False positives from log hex
  are too common.
- General FQDN detection from free text. Only `ssh`-context hostnames and
  Tailscale-shaped names are trusted.
- Multi-select / batch actions.
- Frecency / history of previously-opened links.
- Streaming enrichment updates into the TUI after launch. v1 blocks on
  the enrichment deadline, then launches the TUI with a frozen snapshot.
  v2 may stream updates as they arrive.

## Execution Flow

Happy path and error exits for `rmux_helper pick-links`, in order. This is
the canonical sequence — subsystem sections below refine individual steps
but must not reorder them. Ordering matters most around the OSC 52 flush
(which must land after the TUI's terminal restore) and the `F2` exec path
(which must skip OSC 52 entirely).

```text
fn pick_links(args) -> ExitCode {
  // 1. Resolve pane
  pane_id = env("TMUX_PANE")
         ?? tmux_display_message("#{client_active_pane}")   // if $TMUX set
         ?? return exit(1, "not inside tmux")

  // 2. Capture (sync, fallible)
  raw = tmux_capture_pane(pane_id)
        .map_err(|e| return exit(1, e))                     // pane killed race

  // 3. Detect (sync, pure, ~50ms budget, RegexSet single-pass)
  rows = detect::parse(raw)                                 // Vec<Row>, canonical+deduped
  if args.json { println!(json(rows)); return exit(0) }     // --json short-circuit
  if rows.is_empty() { /* fall through to TUI with empty-state message */ }

  // 4. Enrich (async boundary, single block_on)
  //    SIGINT here -> process exits 130; gh children reaped by init.
  rows = Runtime::new()?.block_on(async {
    let cache = cache::load_or_reset();                     // corrupt -> silently reset
    let (hits, misses) = cache.split(rows);
    let fresh = fanout(misses, deadline=args.enrich_deadline_ms, cap=8).await;
    cache.merge_and_write_atomic(&fresh);                   // last-writer-wins, no flock
    hits.chain(fresh).chain(unenriched_fallbacks).collect()
  });

  // 5. TUI (sync crossterm; tokio runtime already dropped)
  loop {
    event = tui.next_event();
    match event {
      Action(row, kind) => { action = Some((row, kind)); break }
      F2                => { action = Some(SwapToPickTui); break }
      Quit              => { action = None; break }
    }
  }

  // 6. Teardown (order is load-bearing)
  disable_raw_mode(); LeaveAlternateScreen; drop(terminal);
  stdout.flush(); stderr.flush();

  // 7. Dispatch (post-teardown so OSC 52 is not swallowed by raw mode)
  match action {
    None                 => exit(130),                      // Esc / C-c
    Some((row, Yank))    => { write_osc52(row); println!("{row.url}"); exit(0) }
    Some((row, Open))    => { spawn("open", row.url); exit(0) }
    Some((row, GhWeb))   => { spawn("gh", ["...", "--web"]); exit(0) }
    Some((row, Ssh))     => { tmux_new_window("ssh", row.host); exit(0) }
    Some(SwapToPickTui)  => { execvp("rmux_helper", ["pick-tui"]) }
    // ^ execvp never returns on success; on ENOENT/EACCES the picker loops
    //   back to step 5 with a bottom-bar error (degenerate case; rare).
  }
}
```

Key ordering invariants visible in this sequence (each is also asserted
under Invariants below):

- OSC 52 is written **after** `disable_raw_mode` + `LeaveAlternateScreen`
  and **before** `exit` — inside raw mode tmux can drop the sequence,
  after exit the process is gone.
- The `SwapToPickTui` branch (F2) runs `execvp` **without** first writing
  OSC 52 — it is a picker swap, not an action.
- `Runtime::new().block_on(...)` returns before the TUI starts, so by the
  time F2 fires there is no tokio runtime to shut down. Revisit if v2
  streaming enrichment moves the runtime across the TUI boundary.
- SIGINT during step 4 exits 130 with the terminal still in cooked mode —
  no teardown needed because the TUI has not yet taken the terminal.

## Invocation

```bash
rmux_helper pick-links         # launch the TUI picker
rmux_helper pick-links --json  # emit JSON of detected items, no TUI (for scripts/tests)
```

Tmux binding in `shared/.tmux.conf`:

```tmux
bind-key L display-popup -E -w 95% -h 95% \
  "TMUX_PANE=#{pane_id} rmux_helper pick-links"
```

The default tmux binding for `C-a L` is `switch-client -l` (last client),
which Igor's config does not currently rebind. Overriding it is
intentional — the new link picker is more useful day-to-day than "switch
to last client," which is already reachable via other means.

`TMUX_PANE=#{pane_id}` is set explicitly on the bind-key so the popup's
child process receives the originating pane ID even when tmux doesn't
propagate `$TMUX_PANE` through the popup pty. This is the only correct
way to identify the pane to capture — the popup is not a pane itself.

Help and alias conventions follow the existing `pick-tui` pattern (help
section at top of `.tmux.conf`, command alias in the aliases block).

**Prerequisite:** `set-option -g set-clipboard on` must be set in tmux
(already the case in `shared/.tmux.conf`) so OSC 52 yank sequences
propagate to the outer terminal emulator. Without it, `y` / default-Enter
copy will silently no-op.

## Scrollback Capture

Source = the current tmux pane's full scrollback:

```bash
tmux capture-pane -p -J -e -S - -E - -t "$TMUX_PANE"
```

- `-S -` / `-E -`: from start of history to end of visible area.
- `-J`: join wrapped lines (so a URL broken across two display rows is one logical line).
- `-e`: include ANSI escapes for the preview pane.
- `-t "$TMUX_PANE"`: the invoking pane, not the popup's own pane.

If `$TMUX_PANE` is unset:

- If `$TMUX` is set, ask tmux directly:
  `tmux display-message -p -t '#{client_active_pane}' '#{pane_id}'`.
  Note that when launched from a `display-popup`, `$TMUX_PANE` is _not_
  reliably forwarded by tmux, which is why the key binding sets it
  explicitly — this branch is the fallback for invocation from a plain
  shell inside a tmux pane.
- If `$TMUX` is unset, exit 1 with
  `pick-links: not inside tmux; nothing to capture` on stderr.

If `$TMUX_PANE` is set but `tmux capture-pane -t "$TMUX_PANE"` returns
nonzero (pane was killed between invocation and capture), exit 1 with
the underlying tmux error on stderr. The pane-lookup and capture are two
separate tmux calls and the pane can vanish between them; this is not
treated as a crash.

No iTerm2 Python API fallback in v1 — keeping it cross-terminal.

Capture is one-shot at launch; no live refresh.

## Categories & Detection

Categories are listed in this display order. Detection is applied per line;
each line can contribute to multiple categories if multiple distinct items
appear in it.

| #   | Category          | Detection                                                         | `key` / `repo-or-host`       |
| --- | ----------------- | ----------------------------------------------------------------- | ---------------------------- |
| 1   | **Pull Requests** | `github.com/OWNER/REPO/pull/N(?:#\S*)?`                           | `#N` / `REPO`                |
| 2   | **Issues**        | `github.com/OWNER/REPO/issues/N(?:#\S*)?`                         | `#N` / `REPO`                |
| 3   | **Commits**       | `github.com/OWNER/REPO/commit/[a-f0-9]{7,40}`                     | `SHA[:7]` / `REPO`           |
| 4   | **Files**         | `github.com/OWNER/REPO/(blob\|tree)/REF/PATH(?:#L\d+(?:-L\d+)?)?` | `basename[:Lstart]` / `REPO` |
| 5   | **Repos**         | `github.com/OWNER/REPO` with nothing after                        | `OWNER` / `REPO`             |
| 6   | **Blog**          | host ∈ blog allowlist (v1: `idvorkin.github.io`)                  | last path segment / host     |
| 7   | **Other links**   | any `https?://` not matched above                                 | last path segment / host     |
| 8   | **Servers**       | `ssh` context OR Tailscale-shaped names                           | hostname / `—`               |
| 9   | **IPs**           | IPv4 with version-string suppression                              | dotted-quad / `—`            |

The `context` column (see Layout) comes from the scrollback line where the
item was found, not from the item itself, so every category gets a readable
third column for free. For GitHub PR/Issue/Commit rows, the context column
is replaced by the enriched `gh` title when enrichment succeeds (see
Enrichment section).

## Enrichment (parallel `gh` fan-out)

GitHub PR, Issue, and Commit rows are enriched with real titles and state
via parallel `gh` calls before the TUI launches.

### Pipeline

The pipeline is strictly phased: **capture → parse → enrich → TUI**. Each
phase fully completes before the next begins. There are no threads feeding
data into the running TUI in v1 (streaming enrichment is v2). Concretely:

- **Capture** and **parse** are synchronous Rust on the main thread.
- **Enrich** runs a `tokio` current-thread runtime constructed on the main
  thread (`Runtime::new()?.block_on(enrich(...))`). The TUI is crossterm
  (sync); it is never invoked from within an async context, and no
  `tokio::spawn_blocking` trampoline is needed because the TUI only starts
  after `block_on` returns.
- **TUI** takes ownership of a `Vec<Row>` and never re-enters tokio.

This phasing keeps the async boundary tiny (one function: `enrich(rows,
deadline) -> Vec<Row>`) and lets the detection module remain a leaf crate
with no async dependencies.

1. **Parse & dedup** (fast, synchronous, <50ms). Produces the full list of
   rows across all categories.
2. **Cache lookup** — for each PR/Issue/Commit row, check
   `~/.cache/rmux_helper/gh-links.json` for a cached enrichment (keyed by
   canonical URL, TTL 1 hour). Cache hits are applied immediately.
3. **Parallel fan-out** — for the remaining PR/Issue/Commit rows, issue
   concurrent `gh` calls:
   - PR: `gh pr view N -R OWNER/REPO --json title,state,author,isDraft`
   - Issue: `gh issue view N -R OWNER/REPO --json title,state,author`
   - Commit: `gh api repos/OWNER/REPO/commits/SHA --jq '{title: .commit.message | split("\n")[0], author: .commit.author.name}'`
4. **Deadline** — global 3-second budget for the whole fan-out (not
   per-call). Whatever finishes in time gets used; the rest fall back to
   the `context` line. Budget is configurable via `--enrich-deadline-ms`
   (default 3000, `0` disables enrichment entirely).
5. **Cache write** — successful results are merged into the cache JSON.
6. **TUI launch** — ratatui starts with a frozen snapshot of enriched +
   fallback rows. No in-flight updates during the session (that's v2).

### Concurrency cap

`gh` calls run concurrently via a bounded semaphore (default 8 in-flight).
This avoids opening 100+ processes if scrollback contains 100 distinct PRs,
while still saturating typical use cases.

### Degradation

- **`gh` missing from PATH** — enrichment is skipped entirely; all rows
  use context column. Top bar shows `(gh not found — install for PR titles)`
  for one second at startup, then disappears.
- **`gh` present but not authenticated** — same as missing. No retry.
- **Network unreachable** — deadline hits, rows fall back to context.
- **Individual call errors** (404, private repo without access, bad SHA) —
  that row falls back to context silently.
- **Malformed `gh` JSON output** (schema drift across `gh` versions, mid-
  stream truncation, non-UTF-8) — treated as an individual call error:
  that row falls back to context. `serde_json::from_slice` failure is
  swallowed at the row level; no panic, no global abort, no log.

### Cache format

```json
{
  "version": 1,
  "entries": {
    "https://github.com/idvorkin-ai-tools/settings/pull/68": {
      "fetched_at": "2026-04-12T10:23:15Z",
      "title": "rbv: show install hint when bv is missing",
      "state": "MERGED",
      "author": "idvorkin-ai-tools",
      "is_draft": false
    }
  }
}
```

Cache file is rewritten atomically (temp file + rename). Corrupt cache is
deleted and rebuilt silently.

**Concurrent writers.** Two pickers invoked in parallel (two panes, two
popups) may both write the cache. The contract is last-writer-wins at the
file level — each writer reads, merges its own new entries, and renames
over the existing file. Entries written by one picker but not yet visible
to the other are re-fetched on the next run; this is acceptable because
`gh` calls are idempotent and the cache is a latency optimization, not a
source of truth. No advisory file lock (`flock`) in v1; if contention
becomes visible in practice, add one.

**SIGINT during enrichment.** The 3-second enrichment window runs before
the TUI acquires the terminal, so the controlling terminal's Ctrl-C still
delivers SIGINT normally. On SIGINT the process exits 130 immediately;
in-flight `gh` child processes are reaped by the default tokio runtime
drop (which kills the async tasks but not the already-spawned child
processes — they are left to finish and be reaped by init, which is fine
because `gh` is short-lived and stateless). No custom signal handler in
v1.

### Enriched display

When enrichment succeeds, the leaf row shows the enriched title in the
`context` column, prefixed with a state glyph:

| State                    | Glyph | Color          |
| ------------------------ | ----- | -------------- |
| Open PR / open issue     | `◉`   | `LightGreen`   |
| Merged PR                | `●`   | `LightMagenta` |
| Closed PR / closed issue | `✕`   | `DarkGray`     |
| Draft PR                 | `◐`   | `LightYellow`  |
| Commit                   | `⎇`   | `LightBlue`    |

Example enriched row:

```
├─ #68  settings  ● rbv: show install hint when bv is missing  ×2
```

Non-GitHub categories (Blog, Other, Servers, IPs) are unaffected — they use
the context line unchanged.

### URL regex (shared)

```
https?://[^\s<>"'`()\[\]{}]+
```

Trailing `.,;:!?)]}>'"` is stripped after match (common punctuation artifacts
from prose). A trailing `/` is preserved (semantically significant for some
sites).

### GitHub canonicalization

All GitHub rows canonicalize to:

```
https://github.com/OWNER/REPO[/pull/N|/issues/N|/commit/SHA|/blob|tree/REF/PATH]
```

- Strip `?utm_*`, `?tab=`, `?w=`, `?notification_referrer_id`, `?diff=`
  query params.
- Strip URL fragment unless it's `#LN` on a file URL (line anchors are kept).
- `github.com` and `www.github.com` collapse to the same canonical host.

### Server detection

Two sources:

1. **ssh context** — matches of the form:

   ```
   \bssh\b(?:\s+-\S+)*\s+(?:([a-zA-Z_][\w-]*)@)?([a-zA-Z0-9][\w.-]*[a-zA-Z0-9])\b
   ```

   The `host` group is captured; the `user@` prefix is discarded for dedup
   (so `ssh igor@c-5001` and `ssh c-5001` merge).

2. **Tailscale-shaped names** — matches of:

   ```
   \bc-\d{4,5}\b
   \b[a-z][a-z0-9-]*\.ts\.net\b
   ```

   These match even outside `ssh` context because they're unambiguous in
   Igor's environment. (The `c-\d{4,5}` pattern is specifically chosen to
   match `c-5001` and not match `c-2` or `c-123456`.)

### IP detection

```
(?<![\w.])(?:\d{1,3}\.){3}\d{1,3}(?![\w.])
```

Plus these suppressions:

- Preceded by `v` with optional whitespace (version strings: `v4.6.0.1`).
- Followed by `.\d` (longer dotted sequences: `1.2.3.4.5` → not an IP).
- Any octet > 255 — rejected.
- `0.0.0.0` and `255.255.255.255` — kept (valid in context).
- Private ranges **not** filtered (Igor's Tailscale is `100.64.0.0/10`).

## Deduplication & Ordering

- **Row key** = `(category, canonical_string)`. `canonical_string` is the
  canonical URL for link rows, the bare host/IP for server/IP rows.
- Duplicates within a category collapse into a single row with a `×N`
  occurrence count (displayed only when N > 1).
- **Ordering within a category** = most-recent first (= largest line index
  in the captured scrollback = closest to the bottom).
- **Ordering of categories** = fixed as listed above. Empty categories hide
  their header entirely.

## Layout & Display

Reuses the `pick-tui` chrome wholesale: ratatui frame, dynamic column widths,
horizontal/vertical auto-switch, `ansi_to_tui` preview.

### Top-bar

```
pick> {query}_  │ ↑↓ Enter:act →:drill y:yank o:open g:gh F2:sess ?:help
```

In drilled-in mode the top-bar shows a breadcrumb:

```
pick> {query}_  │ Links › Pull Requests  │ ↑↓ Enter:act ←:back F2:sess ?:help
```

### Rows

Category headers are selectable (unlike session picker headers). Two row
kinds:

- **Category header**: `⊟ Category (count)` — Cyan, bold. Shows total matched
  rows in that category post-filter.
- **Leaf**: indented `├─`/`└─` tree prefix + four columns, same coloring as
  `pick-tui`:

  | Col            | Content                                                                                   | Color          | Min width |
  | -------------- | ----------------------------------------------------------------------------------------- | -------------- | --------- |
  | `key`          | `#68`, `a22bc17`, filename, host, IP                                                      | `LightYellow`  | 6         |
  | `repo-or-host` | `settings`, `idvorkin.github.io`, `—`                                                     | `LightGreen`   | 6         |
  | `context`      | scrollback line the item was found on, URL/host stripped, whitespace collapsed, truncated | `LightMagenta` | variable  |
  | `count`        | `×N` (only when N > 1)                                                                    | `LightCyan`    | 0         |

The `context` column is the v1 stand-in for enriched titles. It takes the
_most recent_ scrollback line where the item appeared, removes the matched
URL/host/IP substring, collapses internal whitespace, trims, and truncates
with `…` to fit the column. This means a PR URL that appeared on a
`Merge pull request #68 from idvorkin-ai-tools/rbv-install-hint` line
displays that line as the title — free enrichment without API calls.

When the scrollback line is _just_ the URL with no surrounding text, the
context falls back to the URL path.

Example (mix of enriched titles and context-line fallbacks; `context`
column is truncated to fit):

```
⊟ Pull Requests (3)
├─ #68    settings        ● rbv: show install hint when bv is…    ×2
├─ #67    settings        ● feat: add `just setup` and post-merge…
└─ #1234  claude-code     ◉ fix subagent hang on large plans
⊟ Issues (1)
└─ #42    settings        ◉ tmux pick hangs on M-x in drill-in
⊟ Commits (1)
└─ a22bc17 settings       ⎇ Merge pull request #68 from idvorkin-…
⊟ Files (1)
└─ picker.rs:L42 settings src/picker.rs:42 — fn draw_picker
⊟ Blog (2)
├─ posts  idvorkin.github.io  /posts/ai-agents/
└─ posts  idvorkin.github.io  /posts/pro-tips/
⊟ Servers (1)
└─ c-5001 —                   ssh c-5001 "uname -a"                 ×7
⊟ IPs (1)
└─ 100.64.1.5 —               Tailscale peer 100.64.1.5 is online
```

### Preview pane

Shows the scrollback context around the _most recent_ occurrence of the
selected row: 3 lines above, the matching line, and 3 lines below, with ANSI
colors preserved via `ansi_to_tui`. For category headers, the preview shows a
short summary: `Category: Pull Requests — 3 matches, newest: #68`.

### Layout modes

Same as `pick-tui`: horizontal (list left, preview right) when
`area.width / 2 >= list_width_needed`, otherwise vertical. `C-l` toggles.

### Unicode width in dynamic columns

GitHub PR titles routinely contain CJK, emoji, and combining characters.
Column-width math uses the `unicode-width` crate (`UnicodeWidthStr::width`)
for both the needed-width calculation and the truncation step, not
`str::len` or `chars().count()`. The truncation ellipsis (`…`) is counted
as width 1. This matters for the `context` column specifically, where
enriched titles are displayed unmodified and can easily be double-width.

### Detection performance

Scrollback capture can exceed a megabyte on long-lived panes. Detection
runs each line through a precompiled `regex::RegexSet` of all nine
category regexes (one pass), then for each matching line re-runs the
individual regex only on the matched categories. This keeps the common
case (scrollback with no interesting content) at O(input) with a single
DFA traversal. Detection is bounded to complete inside the 50ms budget
mentioned under Pipeline step 1; if a pathological input overruns that,
the budget is a target, not a hard deadline (the user just waits).

## Navigation

### Flat mode (default)

- `↑` / `C-p` / `↓` / `C-n` — move selection, skipping separators but _not_
  category headers (they are selectable).
- `Enter` on a **leaf** — perform default action (see Actions below).
- `Enter` on a **category header** — drill into that category.
- `→` — drill into the category of the currently selected row (leaf or
  header).
- `1`–`9` — jump straight into the Nth non-empty category's drilled-in
  view (1-indexed, in display order). If N exceeds the count of non-empty
  categories, no-op.
- `←` — no-op in flat mode.
- `Tab` / `S-Tab` — no-op (reserved for future toggle; do not bind to
  drill-in/out because those should be reachable without leaving the home
  row via `→` or `Enter`).
- `F2` — **switch to session picker** (`pick-tui`). Current picker exits;
  session picker launches. See Cross-picker shortcut below.
- Typing printable ASCII — append to query; filter rows and headers.
- `C-l` — toggle layout.
- `?` / `C-/` — help overlay.
- `Esc` — quit. `C-c` — clear query if non-empty, else quit.

### Drilled-in mode

- `←` — pop back to flat mode. Query is **preserved** across the
  transition (so you can drill in, refine, then pop out with your query
  still active).
- `Esc` — first press pops back to flat mode (like `←`); second press
  quits. This keeps `Esc` as the universal "back / quit" key without
  needing a separate drill-out binding.
- `→` / `Enter on header` — no-op (already drilled in, no header visible).
- Everything else — same as flat mode.

Only one level of drill-down in v1. No sub-grouping by repo inside a
category. Recursive "walk into fetched content" is v2.

### Cross-picker shortcut

`F2` toggles between the session picker (`pick-tui`) and the link picker
(`pick-links`). The shortcut is bidirectional and stateless — whichever
picker is open, `F2` closes it and launches the other against the same
originating tmux pane.

Implementation contract:

- Inside `pick-tui`: `F2` cleanly tears down the TUI and execs
  `rmux_helper pick-links`. Teardown sequence, in order:
  1. `disable_raw_mode()`
  2. `execute!(terminal.backend_mut(), LeaveAlternateScreen)`
  3. drop `terminal` (forces a final crossterm flush)
  4. `io::stdout().flush()?` and `io::stderr().flush()?`
  5. `std::os::unix::process::CommandExt::exec()` on the target command
     with `TMUX_PANE` forwarded in the environment
- Inside `pick-links`: `F2` does the mirror — exec `rmux_helper pick-tui`.
- **Never yank on F2.** The `F2` path must not write OSC 52 bytes (see
  Clipboard Bridge) — it's a picker swap, not an action.
- **Runtime teardown.** Inside `pick-links` the tokio runtime has already
  returned from `block_on(enrich(...))` before the TUI started, so there
  is no running runtime at `F2` time — nothing to shut down. If streaming
  enrichment ever moves into v2, the F2 path will need to add a runtime
  shutdown step before `exec`.
- `exec` replaces the process image, so the picker's own exit code is
  irrelevant — the incoming process owns the terminal and decides its own
  exit. If `exec` itself fails (`ENOENT` on the target binary, `EACCES`)
  the picker stays open and shows a bottom-bar error (see Error Handling).
- No "restore to previous picker" on `Esc` after a `F2` swap. The user
  exits normally with `Esc` / `C-c` and lands back at the shell (or the
  tmux popup closes). Preserving a picker-history stack is out of scope.

PICKER_SPEC.md must be updated in the same changeset to document `F2` on
the `pick-tui` side.

`F2` was chosen because function keys don't collide with the "typing
letters filters the search" rule that both pickers share, and F1 is
already taken for help. `Alt-L` was considered but Alt-modified keys are
inconsistent across terminal emulators.

### Filtering semantics

Same token-based fuzzy match as `pick-tui`, with two deliberate
divergences called out below. Shared rules:

- Query splits on whitespace; all tokens must be present.
- Auto-split at letter/digit boundaries: `pr68` → `pr` + `68`.
- Non-digit tokens match the row's full search string (see below).
- Digit tokens match **the `key` column only** (so `68` finds `#68` but not
  a random `68` in a context line).
- Case-insensitive.

**Divergence 1 — multi-digit token rule.** `pick-tui` splits multi-digit
tokens per-digit (`14` matches `1;4`) because tmux indices are
one-digit-per-position. That rule is _not_ inherited here: `68` must
substring-match the key column as `68`, so `#68` matches but `#683` also
matches (substring), while `#86` does not. This is intentional — PR
numbers are whole integers, not position-indexed, so per-digit splitting
would produce spurious matches.

**Divergence 2 — category short-name tag matches on prefix only.** Each
row's search string begins with a category short-name tag followed by a
unit-separator byte (`\x1f`):

```
<tag>\x1f<key> <repo-or-host> <context> <canonical>
```

where `<tag>` ∈ `pr`, `issue`, `commit`, `file`, `repo`, `blog`, `link`,
`server`, `ip`. Because tokens are substring-matched and `\x1f` never
appears in user text or in any rendered column, typing `pr` matches only
via the tag prefix and never matches "prose" in a context line. Typing
`link` matches the Other-links category specifically, not all URL-shaped
rows. In drilled-in mode the tag is still present on each row but
matching it is a no-op (you're already in one category).

Columns composing the row's search string, after the `\x1f` separator, in
order:

1. The `key` column.
2. The `repo-or-host` column.
3. The `context` column.
4. The canonical URL / host / IP string.

In flat mode, a category header is shown iff at least one of its rows
matches. In drilled-in mode, the header is replaced by the breadcrumb.

## Actions

All actions take the selected leaf and branch on category.

### Default (`Enter`)

| Category                                              | Action                                                                                                                                   |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| PRs / Issues / Commits / Files / Repos / Blog / Other | **Yank** (OSC 52) + **print URL to stdout**                                                                                              |
| Servers / IPs                                         | `tmux new-window -t "$TMUX_PANE" -c '#{pane_current_path}' "ssh <host>"` — new window in the session of the invoking pane, not the popup |

### Override keys (on selected leaf)

- `y` — **Yank only**: copy the canonical URL or host to clipboard via OSC 52.
  Prints `yanked: <value>` to stdout. Works across tmux/iTerm/SSH boundaries
  because OSC 52 rides the terminal escape stream.
- `o` — **Open**: run `open` (macOS) or `xdg-open` (Linux) on the URL. For
  Server/IP rows, opens `http://host` in a browser (useful for admin UIs).
  Exits the picker on success.
- `g` — **GitHub web**: for PR / Issue / Commit / File / Repo rows, runs
  `gh <kind> view --web -R OWNER/REPO <N-or-ref>`. No-op for non-GitHub
  categories. Requires `gh` on PATH; on failure (gh missing, not
  authenticated) the picker shows a one-line error in the bottom bar and
  stays open rather than exiting. Note: this shells out to `gh` to open a
  browser, it does not fetch or parse data — distinct from the blocking
  pre-TUI enrichment fan-out under Enrichment above.
- `s` — **ssh**: force ssh action on any row where a hostname can be
  extracted (Server, IP, or a URL's host).

### Exit behavior

- Action success — picker exits 0 and the spawned action runs (new tmux
  window, browser, etc.). Any output (`yanked: …`, the URL) goes to stdout
  of the picker process so a script wrapper can consume it.
- `Esc` / `C-c` with no pending action — exit 130 (conventional "canceled").

## Clipboard Bridge (devvm → Mac host)

The picker may run inside the devvm (Linux OrbStack VM) while the user's
clipboard lives on the Mac host. Copy actions use the OSC 52 sequence:

```
\e]52;c;<base64-of-payload>\e\\
```

written directly to `/dev/tty`. Tmux must have `set-option -g set-clipboard
on` (it does in `shared/.tmux.conf` already) so the sequence propagates to
the outer terminal. iTerm2 honors OSC 52 by default; Ghostty honors it when
enabled.

No shell-out to `pbcopy` / `xclip` / `wl-copy` — OSC 52 is the only path that
works uniformly across Mac-host tmux, devvm tmux, and bare shells.

**Write ordering:** the OSC 52 escape must be written _after_ ratatui
`disable_raw_mode()` + `LeaveAlternateScreen` but _before_ `std::process::exit`
/ returning from `main`. Writing inside raw mode risks tmux dropping the
sequence; writing after process exit is too late. Implementation: flush
the terminal restore, open `/dev/tty` for writing, write the bytes, drop
the handle, then exit.

## Configuration

v1 has a single configuration constant, a hostname allowlist for the **Blog**
category:

```rust
const BLOG_HOSTS: &[&str] = &["idvorkin.github.io"];
```

Adding a blog site means editing this list and rebuilding. A real config file
(TOML next to `rmux_helper`'s binary, or `$XDG_CONFIG_HOME/rmux_helper/`)
is deferred until the list grows past ~3 entries.

## File Layout

```
rust/tmux_helper/
  src/
    main.rs                # existing CLI dispatch — adds `pick-links` subcommand
    picker.rs              # existing — gains F2 handler that execs pick-links
    link_picker/
      mod.rs               # public entry: capture → detect → enrich → TUI
      detect.rs            # pure regex/parsing layer, unit-tested
      enrich.rs            # parallel gh fan-out + cache, deadline-bounded
      tui.rs               # ratatui rendering + key handling
  LINK_PICKER_SPEC.md      # new: behavior contract (mirror of PICKER_SPEC.md style)
  PICKER_SPEC.md           # existing — gets F2 cross-picker entry
```

Detection lives in its own module with no ratatui / tokio dependency, so
it's unit-testable with plain `cargo test` over fixture strings. The
enrichment module owns the tokio runtime; it is the only module that
touches async code, keeping the TUI and detection layers sync.

Cache file: `~/.cache/rmux_helper/gh-links.json` (XDG-conforming; override
with `$XDG_CACHE_HOME`).

### New Cargo dependencies

This feature is the first async code in `rmux_helper`. `Cargo.toml`
gains, at minimum:

- `tokio = { version = "1", features = ["rt", "process", "time", "sync", "macros"] }` — current-thread runtime, `Command`, `timeout`, `Semaphore`. Not `rt-multi-thread`; single-thread is sufficient for `gh` fan-out.
- `regex = "1"` — category detection.
- `serde = { version = "1", features = ["derive"] }` and `serde_json = "1"` — cache format and `--json` output.
- `unicode-width = "0.2"` — column math (see Layout).
- `base64 = "0.22"` — OSC 52 payload encoding.
- `dirs = "5"` (or manual `$XDG_CACHE_HOME`/`$HOME` lookup) — cache path resolution.

These roughly double the clean release build time of the crate
(currently small — only `ratatui`/`crossterm`/`sysinfo`). The
`link_picker` module is **not** gated behind a Cargo feature — there's no
"lean" build of `rmux_helper` to protect — so the deps are unconditional.
Detection's unit tests compile only `detect.rs` + `regex` + `unicode-width`,
so `cargo test -p rmux_helper detect::` stays fast even as the async code
grows.

## Error Handling

- **Not inside tmux** — exit 1 with `pick-links: not inside tmux; nothing
to capture` on stderr. No TUI shown.
- **Empty scrollback / zero items** — render the TUI with a centered
  `No links, servers, or IPs in scrollback` message; `Esc` / `Enter` exit 0.
- **Capture failure** — exit 1 with the underlying `tmux capture-pane`
  error on stderr.
- **Enrichment deadline hit** — silent. Rows that didn't get back in time
  use context-line fallback. Top bar shows `enriched M/N` briefly at
  launch (N = rows eligible for enrichment, M = how many succeeded).
- **`gh` missing / unauthenticated** — top bar shows
  `(gh not found — install for PR titles)` for ~1s at launch; enrichment
  is skipped for the session, no retries.
- **Cache corrupt** — cache file is deleted and rebuilt silently.
- **Action failure** (`open`, `xdg-open`, `gh`, `tmux new-window`) — bottom-
  bar error, picker stays open. User can retry with a different key or
  quit.
- **`F2` cross-picker exec failure** — picker stays open, bottom-bar
  error: `exec rmux_helper pick-tui: <errno>`.

## Invariants (for tests)

Detection (pure, unit-tested):

- **Idempotent on the same scrollback**: two runs over the same input
  produce the same JSON output (stable ordering = category index, then
  most-recent-first within, ties broken by canonical string).
- **Dedup correctness**: `ssh igor@c-5001`, `ssh c-5001`, and a bare `c-5001`
  in a log line all merge into one Server row with `×3`.
- **No cross-category duplication** for the same canonical URL: a PR URL
  contributes to Pull Requests only, not to Other links, even though it
  matches the generic URL regex.
- **URL trailing punctuation stripped**: `"see https://github.com/a/b/pull/1."`
  yields `…/pull/1`, not `…/pull/1.`.
- **IP suppression**: `"claude-opus 4.6.0.1"` does not yield `4.6.0.1` as
  an IP (prefixed with version-shaped token).
- **Version range kept**: `"100.64.1.5 is up"` yields `100.64.1.5` as IP.

Enrichment (integration-tested with `gh` stubbed):

- **Deadline is a single shared wall clock, not per-call**: the 3000ms
  budget starts when enrichment begins. Any `gh` call still in flight
  when the budget expires is cancelled and its row falls back to the
  context line.
- **Cache hits skip the network**: if all eligible rows are cache-hit,
  total enrichment time is under 50ms (file read + JSON parse).
- **Concurrency cap is honored**: at most 8 `gh` processes in flight at
  any moment, regardless of total row count.
- **Fallback is row-level, not global**: one row failing to enrich does
  not cancel the others.
- **Malformed `gh` JSON falls back gracefully**: a `gh` call returning
  `{"title": 42}` (wrong type), a truncated stream, or non-UTF-8 bytes is
  treated as a row-level failure and does not panic or abort the fan-out.
- **Cache write is atomic**: on crash mid-write, the cache file is either
  the old version or the new version, never a truncated mix.

Filtering:

- **Category tag does not leak into content match**: a `context` column
  containing the literal string `prose` does not match the `pr` token.
  Implemented via the `\x1f`-separator trick described under Filtering
  semantics.
- **Digit-token substring (not per-digit split)**: token `68` matches key
  `#68` and key `#683`, but not key `#86`. This is the documented
  divergence from `pick-tui`.

Layout / rendering:

- **Unicode-aware column widths**: a `context` column containing a
  double-width CJK title does not cause later rows to shift by one
  character. Enforced via `unicode-width::UnicodeWidthStr::width` in
  column-budget math; unit-testable with fixture strings.
- **Context column truncation is canonical**: for a given column-width
  budget, the truncated-with-ellipsis form of a row's context is stable
  across runs (idempotent), so snapshot tests on rendered output are
  meaningful.
- **Empty categories render no header**: a category with zero matching
  rows (pre-filter or post-filter) emits no `⊟ Category (0)` line.
- **Category display order is fixed**: categories always render in the
  order defined in the Categories table (PR, Issue, Commit, File, Repo,
  Blog, Other, Server, IP), regardless of which was matched first in
  scrollback.

Navigation:

- **Query preserved across drill transitions**: drill into a category
  with query `foo`, press `←` to pop back — the query is still `foo` and
  the flat-mode filter is re-applied with it.
- **`Esc` double-press to quit from drill-in**: first `Esc` pops to flat
  mode (equivalent to `←`), second `Esc` quits. A single `Esc` from flat
  mode quits directly.

Action layer:

- **OSC 52 shape**: yanked bytes always written to `/dev/tty` directly, never
  to stdout, so stdout stays a pure data channel for script wrappers.
- **OSC 52 timing**: the escape sequence is written after raw mode is
  disabled and the alternate screen is left, but before process exit.
- **F2 does not yank**: the `F2` swap path must not write any OSC 52
  bytes, even if the currently-selected row would have yanked on `Enter`.
- **F2 exec terminal cleanliness**: the terminal is fully restored
  (raw-mode off, alternate screen left, stdout/stderr flushed) before
  `execvp`, so the incoming picker sees a clean terminal state.
- **SIGINT before TUI**: Ctrl-C during the enrichment window exits 130
  without rendering the TUI; no OSC 52 escape is emitted.

## Out of scope (v2+ roadmap)

- **Streaming enrichment updates** — push `gh` results into the running
  TUI via an mpsc channel so rows enrich visibly after launch instead of
  blocking on a pre-TUI deadline.
- **Blog-post title scraping** — fetch blog URLs' HTML and extract
  `<title>` for the `context` column. Needs its own timeout + cache story.
- **Recursive link walking** — "walk into" a URL by fetching its content
  and re-parsing for nested links, usable with `→` repeatedly.
- **Frecency** — last-opened rows float to the top of their category.
- **Multi-select + batch** — mark several rows with `Space`, act on all.
- **IPv6 detection**.
- **Config file** for blog allowlist, server-name patterns, enabled categories.
- **iTerm2 Python API fallback** for non-tmux terminals, using
  `iterm2.Session.async_get_contents` over the full scrollback range.
- **Picker-history stack** — `Esc` after an `F2` swap could return to the
  previous picker instead of the shell.
