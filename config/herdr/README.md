# herdr

Terminal workspace manager for AI coding agents — a tmux-shaped multiplexer that
also tracks which panes are running an agent and what state each one is in.

- Config: [`config.toml`](config.toml), symlinked to `~/.config/herdr/config.toml`
  by [`bootstrap.sh`](../../bootstrap.sh).
- Only the file is symlinked, not the directory — herdr writes logs, sockets, and
  `session.json` into `~/.config/herdr`.
- Validate: `herdr config check` · Reload: `prefix+r` or `herdr server reload-config`

## Mental model

The container hierarchy is three levels deep and maps cleanly onto tmux:

| herdr     | tmux    | notes                                   |
| --------- | ------- | --------------------------------------- |
| session   | server  | named, persistent; usually just one     |
| workspace | session | **also called a "space"** — see below   |
| tab       | window  |                                         |
| pane      | pane    |                                         |

**"Space" and "workspace" are the same thing.** herdr's docs use "space" only in
the sidebar/agent-panel settings (`agent_panel_sort`, `[ui.sidebar.spaces]`);
every command and keybinding says "workspace". Per the default config:
`"workspaces" is accepted as an alias for "spaces"`. There is no separate
fourth container.

**An "agent" is not a container.** It is a property of a pane. When herdr detects
a supported AI CLI running in a pane, that pane also shows up in the agent panel
with a live status (`working` / `idle` / `unknown`). The agent panel is a
cross-cutting view over panes, not a place things live.

```
session "default"
└── workspace w1  "settings"          <- a "space"
    ├── tab w1:t1 └── pane w1:p1      <- claude here => shows in agent panel
    ├── tab w1:t2 └── pane w1:p2      <- claude here => shows in agent panel
    └── tab w1:t3 └── pane w1:p3      <- plain shell => not an agent
```

### Gotcha: the agent panel groups by workspace, and cannot show cwd

If two agents are working in different repos but live in the same workspace, the
agent panel lists both under that one workspace name — which reads as though both
are in the same repo. That is a display limitation, not stale data: `herdr pane list`
reports the correct distinct `cwd` for each pane.

Two things combine to cause it:

1. `agent_panel_sort = "spaces"` groups agent rows by workspace.
2. The default agent row is `rows = [["state_icon", "workspace", "tab"], ["agent"]]`,
   and the built-in row tokens are `state_icon`, `state_text`, `workspace`, `tab`,
   `pane`, `agent`, `terminal_title`, `terminal_title_stripped`. **There is no `cwd`
   token** — a working directory column is not available without plumbing custom
   pane metadata over the socket API.

A workspace label is just a name; it does not constrain what a pane inside it can
`cd` to. So opening extra tabs in an existing workspace for *other* repos is what
produces the confusing panel.

Ways to fix it, best first:

- **One workspace per repo** (`prefix+shift+n`). This is what the agent panel is
  designed around, and it makes the workspace column meaningful again.
  Note `herdr tab` has **no move command**, so a tab cannot be relocated wholesale.
  Individual panes can move, though — `herdr pane move <PANE_ID> --tab <TAB_ID>`,
  and tab ids are workspace-scoped (`w2:t3`), so it should reach another
  workspace's tab.
- **Surface the task instead of the location** — show the agent's terminal title,
  which Claude Code keeps set to the current task. This is configured in
  [`config.toml`](config.toml). Note it is **opt-in**: `rows_by_agent` appears in
  `herdr --default-config` only as a commented example, and the real built-in
  default is `rows = [["state_icon", "workspace", "tab"], ["agent"]]` — two lines
  per agent, with no title. Without setting it explicitly the panel shows only
  workspace, tab, and agent name.
- **Sort by attention instead of grouping** — `agent_panel_sort = "priority"` turns
  the panel into a queue of who needs you, making the workspace grouping moot.

## Keybindings

Prefix is `ctrl+a`, matching `shared/.tmux.conf` (herdr's default is `ctrl+b`,
which collides with vi paging). Everything below is set in [`config.toml`](config.toml);
entries marked ✱ override a herdr default to restore tmux muscle memory.

### Session

| Keys             | Action        | tmux                    |
| ---------------- | ------------- | ----------------------- |
| `prefix+?`       | help          | `?` list-keys           |
| `prefix+d`       | detach ✱      | `d` detach              |
| `prefix+r`       | reload config ✱ | `r` source-file        |
| `prefix+shift+r` | resize mode   | —                       |
| `prefix+s`       | settings      | —                       |
| `prefix+b`       | toggle sidebar | —                      |

### Workspaces (tmux sessions)

| Keys             | Action              | tmux                    |
| ---------------- | ------------------- | ----------------------- |
| `prefix+w`       | workspace picker    | `w` → `rmux_helper pick-tui` |
| `prefix+(`       | previous workspace  | `(` switch-client -p    |
| `prefix+)`       | next workspace      | `)` switch-client -n    |
| `prefix+$`       | rename workspace ✱  | `$` rename-session      |
| `prefix+shift+n` | new workspace       | —                       |
| `prefix+shift+d` | close workspace     | prompts, `confirm_close = true` |
| `prefix+shift+g` | new git worktree    | herdr-only              |
| `prefix+g`       | goto                | herdr-only              |

### Tabs (tmux windows)

| Keys           | Action        | tmux                 |
| -------------- | ------------- | -------------------- |
| `prefix+c`     | new tab       | `c` new-window       |
| `prefix+n`     | next tab      | `n` next-window      |
| `prefix+p`     | previous tab  | `p` previous-window  |
| `prefix+1..9`  | switch tab    | `1-9` select-window  |
| `prefix+,`     | rename tab ✱  | `,` rename-window    |
| `prefix+&`     | close tab ✱   | `&` kill-window      |

### Panes

| Keys             | Action                | tmux                    |
| ---------------- | --------------------- | ----------------------- |
| `prefix+%`       | split side-by-side ✱  | `%` split-window -h     |
| `prefix+"`       | split stacked ✱       | `"` split-window -v     |
| `prefix+x`       | close pane            | `x` kill-pane           |
| `prefix+z`       | zoom                  | `z` resize-pane -Z      |
| `prefix+h/j/k/l` | focus left/down/up/right | vim-style            |
| `prefix+o`       | cycle next pane ✱     | `o` select-pane -t :.+  |
| `prefix+ctrl+o`  | cycle previous pane   | `C-o` rotate-window     |
| `prefix+;`       | last pane ✱           | `;` last-pane           |
| `prefix+{` / `}` | swap pane up/down     | `{` / `}` swap-pane     |
| `prefix+shift+k` / `shift+j` | swap pane up/down | —           |
| `prefix+shift+p` | rename pane           | —                       |
| `prefix+/`       | toggle 1/3–2/3 layout | `/` → `rmux_helper third` |

Verified against `herdr pane layout`: `split_vertical` produces a pane to the
**right** (tmux `%`), `split_horizontal` produces one **below** (tmux `"`). The
names are the opposite of tmux's flag naming, which is why the config comments
call it out.

### Scrollback and popups

| Keys            | Action                                  |
| --------------- | --------------------------------------- |
| `prefix+v`      | copy mode (tmux `C-a v`, mode-keys vi)  |
| `prefix+e`      | dump scrollback into `$EDITOR` — herdr-only |
| `prefix+ctrl+g` | lazygit popup, 90% × 90%                |
| `prefix+ctrl+t` | `tig status` popup, 90% × 90%           |

Popups mirror the `:tig` / `:gdiff` command aliases in `shared/.tmux.conf`.

## CLI

The CLI drives the running server over its unix socket and returns JSON — handy
for scripting and for checking what the UI is actually reporting.

```bash
herdr                          # launch or attach to the persistent session
herdr status                   # client and server status
herdr session list             # named sessions
herdr workspace list           # workspaces (spaces)
herdr tab list                 # tabs
herdr pane list                # panes — the only view with per-pane cwd
herdr agent list               # panes running a detected agent, with status
herdr agent explain            # why a pane did or did not register as an agent
herdr config check             # validate config.toml
herdr server reload-config     # apply config.toml without restarting
```

Pipe through `python3 -m json.tool` to read the output comfortably.

## Agent integrations

`herdr integration install <agent>` writes hooks that let an agent report its
session id back to herdr, which is what drives live status in the agent panel.
Supported: `claude`, `codex`, `cursor`, `copilot`, `devin`, `droid`, `kimi`,
`opencode`, `kilo`, `hermes`, `qodercli`, `mastracode`, `pi`, `omp`.

Installed here:

- **cursor** → [`../cursor/herdr-agent-state.sh`](../cursor/herdr-agent-state.sh)
  and [`../cursor/hooks.json`](../cursor/hooks.json). These land in this repo
  because `~/.cursor` is symlinked to `config/cursor`, so they travel to new
  machines via `bootstrap.sh`.
- **claude** → hooks in `~/.claude/settings.json`, which is **not** tracked in this
  repo. Re-run `herdr integration install claude` on a new machine.

Both generated Cursor files carry a `managed by herdr` header and are overwritten
on reinstall or upgrade — add custom hooks in sibling files rather than editing
them. `hooks.json` also hardcodes an absolute `~/.cursor` path, so it is
macOS-specific as written. Note that pre-commit's Biome reformats `hooks.json`
(tabs, trailing newline), so it will show as modified again after a reinstall;
the JSON is semantically unchanged.
