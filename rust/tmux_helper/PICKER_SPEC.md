# rmux_helper pick-tui Specification

## Layout

- **Horizontal (side-by-side)**: Sessions list on left (minimum needed width), preview on right (remaining space)
- **Vertical (stacked)**: Sessions on top (dynamic height to fit content), preview on bottom (remaining space)
- **Dynamic width calculation**: Sessions width calculated from actual content (fixed columns + variable pane title)
- Auto-switch to vertical when `terminal_width / 2 < sessions_width_needed`
- Manual toggle via `C-l`
- **Full screen**: Uses 100% of popup window

## Display Format

### Tree Structure

```
1 session_name                      (session header, cyan)
├─ 1;1  window    path    pane      (first pane of window)
│  └─             path    pane      (additional pane)
└─ 1;2  window    path    pane
```

### Markers

- `◀` = Current pane (white, bold)
- `◁` = Last/previous pane (yellow)

### Column Order

1. **Index** (e.g., `1;1`) - session;window
2. **Window name**
3. **Path** (short path or git repo name)
4. **Pane title** (only if meaningful)

### Column Colors

- Index: Light Yellow
- Window: Light Green
- Path: Light Magenta
- Pane: Light Cyan
- Tree lines: Dark Gray

### Column Alignment

- **Dynamic-width columns** calculated from actual content (with minimums)
- Index: min 4 chars, sized to longest index (e.g., "1;10")
- Window: min 3 chars, sized to longest prefix (e.g., "cl", "docker")
- Path: min 6 chars, sized to longest path
- Pane: variable (trailing)

### Window Name Deduplication

- If window name follows `<prefix> <path>` pattern (from rename-all), only prefix is shown
- Path column shows the actual pane path
- Avoids redundant display like "cl settings/rust settings/rust"

## Filtering Rules

- **Hide pane title** if it equals the hostname (case-insensitive)
- **Hide pane column** entirely if empty after hostname filter
- Session headers shown in Cyan (bold if current session)

## Highlighting

- **Current pane** (`◀`): White marker, subtle blue background (`rgb(40,40,60)`)
- **Last pane** (`◁`): Yellow marker, subtle orange background (`rgb(50,40,30)`)
- **Selection highlight**: Dark gray background + bold

## Navigation

- Start selection on **current pane** (not first entry)
- `↑/↓` or `C-p/C-n`: Move selection
- `Tab` or `C-o`: Toggle between current (`◀`) and last (`◁`) pane
- **Sessions are not selectable** - navigation skips session headers
- Skip separator lines when navigating
- Wrap around at list boundaries (only across windows/panes)

## Actions

- `Enter`: Switch to selected pane
- `Esc` or `C-c`: Cancel and quit
- `C-r`: Rename window (of currently selected pane)
- `C-l`: Toggle layout (horizontal/vertical)
- `?` or `C-/`: Show help overlay
- Type characters: Filter entries by text (printable ASCII only)

## Preview Pane

- Shows captured content of selected pane
- **ANSI color support**: Renders terminal colors correctly
- **Adaptive sizing**:
  - Vertical layout (wide/short): fewer lines, full width
  - Horizontal layout (narrow/tall): more lines, truncate if width < 60
- Line truncation uses `…` character
- Session headers show "Session: {name}" instead of capture

## Chrome (UI Frame)

- **Search line** (1 line): `pick> {query}_ │ ↑↓ Tab:◀◁ Enter:sel ?:help`
- Minimal borders to maximize content space

## Rename Dialog

- Popup overlay centered on screen
- Pre-filled with current session/window name
- `Enter` to confirm, `Esc` to cancel
- Renames session if selected on session header
- Renames window if selected on pane entry

## Technical Notes

- **Event draining**: Clears stale events at startup to prevent phantom keypresses
- **Control char handling**: Handles both modifier-style (`CONTROL` + `p`) and raw control chars (`\x10`)
- **Search filtering**: Only accepts printable ASCII characters (ignores control chars)
