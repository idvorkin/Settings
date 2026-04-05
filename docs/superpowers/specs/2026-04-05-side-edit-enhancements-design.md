# Side-Edit Enhancements Design

## Summary

Enhance `rmux_helper side-edit` and add `rmux_helper side-run` with three goals:
1. Line number support (`side-edit file.py:42`)
2. New `side-run` command for shell commands in the side pane
3. Universal pane status output on every invocation

## Commands

### `side-edit [file[:line]]`

- **With `file:line`**: Parse trailing `:N` from the file argument. Open file at that line number using `nvim +N file` (new pane) or `:e +N file` (existing nvim).
- **With `file`**: Current behavior — open file in side pane.
- **No arguments**: Print pane status only, no side effects.
- **After action**: Always print pane status to stdout.

### `side-run [--force] [command...]`

- **With command**: Run the shell command in the side pane.
  - If nvim is running in the side pane: refuse with warning ("nvim is running in the side pane, you may lose unsaved work. Use --force to kill it."). With `--force`, kill nvim and run the command.
  - If idle shell: send the command via `tmux send-keys`.
  - If no side pane exists: create one and run the command (no nvim, just shell).
- **No arguments**: Print pane status only, no side effects.
- **After action**: Always print pane status to stdout.

## Pane Status Output

Every `side-edit` and `side-run` invocation prints a status block to stdout. Format:

```
pane_id: %42
nvim: true
file: /home/user/project/foo.py
```

- `pane_id`: tmux pane ID (e.g. `%42`), or `none` if no side pane exists
- `nvim`: `true`/`false` — whether nvim is running in the pane
- `file`: current file path from nvim's `/proc/<pid>/cmdline`, or empty if not detectable / nvim not running

## Line Number Parsing

Parse the file argument for a trailing `:N` pattern:
- `file.py:42` -> file=`file.py`, line=`42`
- `file.py` -> file=`file.py`, line=`None`
- `/path/to/file.py:10` -> file=`/path/to/file.py`, line=`10`
- `file.py:notanum` -> file=`file.py:notanum`, line=`None` (treat whole thing as filename)

## Nvim File Detection

Use `/proc/<nvim_pid>/cmdline` to extract the file argument from the nvim process. This is the easy path — it returns the file nvim was launched with or the last `:e` file won't be reflected. Good enough for now; can upgrade to nvim RPC via server socket later.

Implementation: walk the process tree from the pane PID (already done by `is_vim_in_pane`), find the nvim process, read `/proc/<pid>/cmdline`, extract the last non-flag argument.

## Changes Required

### Rust (`rust/tmux_helper/src/main.rs`)

1. **Add `SideRun` variant** to `Commands` enum with `command: Option<String>` and `--force` flag
2. **Make `SideEdit.file` optional** — `Option<String>` instead of `String`
3. **Add `parse_file_line()`** — split `file:line` into `(file, Option<usize>)`
4. **Add `get_side_pane_status()`** — returns struct with pane_id, nvim running, file path
5. **Add `print_pane_status()`** — formats and prints the status block
6. **Add `get_nvim_file_from_proc()`** — reads `/proc/<pid>/cmdline` for nvim's file arg
7. **Modify `side_edit()`** — handle optional file, line numbers, always print status
8. **Add `side_run()`** — new function for running commands with --force logic
9. **Modify `open_file_in_pane()`** — accept optional line number, use `:e +N file`
10. **Modify `create_side_pane()`** — accept optional line number, use `nvim +N file`

### Tmux config (`shared/.tmux.conf`)

11. **Add `side-run` command alias**
12. **Add `side-run` to help section**
