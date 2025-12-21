#!uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
#     "pydantic",
#     "psutil",
# ]
# ///

import typer
import subprocess
import json
import os
from pathlib import Path
from pydantic import BaseModel
import psutil

# Constants
LAYOUT_STATE_OPTION = "@layout_state"
THIRD_STATE_OPTION = "@third_state"
STATE_HORIZONTAL = "horizontal"
STATE_VERTICAL = "vertical"
STATE_THIRD_HORIZONTAL = "third_horizontal"
STATE_THIRD_VERTICAL = "third_vertical"
STATE_NORMAL = "normal"


# Helper functions
def run_tmux_command(cmd: list[str], capture_output: bool = False) -> str | None:
    """Run tmux command with consistent error handling"""
    try:
        if capture_output:
            return (
                subprocess.check_output(cmd, stderr=subprocess.DEVNULL)
                .decode("utf-8")
                .strip()
            )
        subprocess.run(cmd, check=True)
        return None
    except subprocess.CalledProcessError:
        return None


def get_tmux_option(option: str) -> str:
    """Get tmux global option value"""
    return (
        run_tmux_command(["tmux", "show-option", "-gqv", option], capture_output=True)
        or ""
    )


def set_tmux_option(option: str, value: str) -> None:
    """Set tmux global option"""
    run_tmux_command(["tmux", "set-option", "-g", option, value])


def ensure_two_panes(command: str | None = None) -> tuple[list[str], bool]:
    """Ensure window has at least 2 panes, return (pane IDs, created_new_pane)

    Args:
        command: Optional command to run in the new pane when created
    """
    panes = run_tmux_command(
        ["tmux", "list-panes", "-F", "#{pane_id}"], capture_output=True
    )
    if not panes:
        return ([], False)

    pane_list = panes.split("\n")
    if len(pane_list) == 1:
        # Create split with optional command
        if command:
            run_tmux_command(
                ["tmux", "split-window", "-h", "-c", "#{pane_current_path}", command]
            )
        else:
            run_tmux_command(
                ["tmux", "split-window", "-h", "-c", "#{pane_current_path}"]
            )
        run_tmux_command(["tmux", "select-layout", "even-horizontal"])
        panes = run_tmux_command(
            ["tmux", "list-panes", "-F", "#{pane_id}"], capture_output=True
        )
        pane_list = panes.split("\n") if panes else []
        return (pane_list, True)

    return (pane_list, False)


def get_layout_orientation() -> str | None:
    """Detect if current layout is horizontal or vertical"""
    pane_info = run_tmux_command(
        ["tmux", "list-panes", "-F", "#{pane_left},#{pane_top}"], capture_output=True
    )
    if not pane_info:
        return None

    lines = pane_info.split("\n")
    if len(lines) < 2:
        return None

    pane1_left, _ = map(int, lines[0].split(","))
    pane2_left, _ = map(int, lines[1].split(","))

    return STATE_HORIZONTAL if pane1_left != pane2_left else STATE_VERTICAL


def process_tree_has_pattern(process_info: dict, patterns: list[str]) -> bool:
    """Check if any pattern exists in process tree cmdline"""
    cmdline = process_info.get("cmdline", "").lower()
    if any(pattern in cmdline for pattern in patterns):
        return True

    return any(
        process_tree_has_pattern(child, patterns)
        for child in process_info.get("children", [])
    )


def get_all_tmux_panes() -> list[str]:
    """Get all pane IDs from tmux"""
    panes = run_tmux_command(
        ["tmux", "list-panes", "-a", "-F", "#{pane_id}"], capture_output=True
    )
    return panes.split("\n") if panes else []


def set_tmux_title(title: str, pane_id: str | None = None):
    """Set tmux window title"""
    if not title:
        return

    # First disable automatic renaming
    if pane_id:
        run_tmux_command(["tmux", "set", "-t", pane_id, "automatic-rename", "off"])
    else:
        run_tmux_command(["tmux", "set", "automatic-rename", "off"])

    # Then set the window title
    if pane_id:
        window_id = run_tmux_command(
            ["tmux", "display-message", "-t", pane_id, "-p", "#{window_id}"],
            capture_output=True,
        )
        if window_id:
            run_tmux_command(["tmux", "rename-window", "-t", window_id, title])
    else:
        run_tmux_command(["tmux", "rename-window", title])


def generate_title(process_info: dict, short_path: str) -> str:
    """Generate a title based on process information"""
    if is_aider_running(process_info):
        return f"ai {short_path}"
    elif is_claude_code_running(process_info):
        return f"claude {short_path}"
    elif is_vim_running(process_info):
        return f"vi {short_path}"
    elif is_docker_running(process_info):
        return f"docker {short_path}"
    elif just_cmd := get_just_command(process_info):
        return just_cmd
    elif python_cmd := get_python_command(process_info):
        return python_cmd
    elif process_info.get("name") == "zsh" and not has_non_utility_children(
        process_info
    ):
        return f"z {short_path}"
    else:
        return process_info.get("name", "")


app = typer.Typer(help="A Tmux helper utility", no_args_is_help=True)


def get_git_repo_name(cwd: str) -> str | None:
    """Get the name of the git repository from the given directory"""
    if not cwd:
        return None

    try:
        git_root = (
            subprocess.check_output(
                ["git", "rev-parse", "--show-toplevel"],
                stderr=subprocess.DEVNULL,
                cwd=cwd,
            )
            .decode("utf-8")
            .strip()
        )
        return os.path.basename(git_root)
    except subprocess.CalledProcessError:
        return None


def is_aider_running(process_info: dict) -> bool:
    """Check if aider is running in the process tree"""
    return process_tree_has_pattern(process_info, ["aider"])


def is_vim_running(process_info: dict) -> bool:
    """Check if vim/nvim is running in the process tree"""
    return process_tree_has_pattern(process_info, ["vim", "nvim"])


def is_claude_code_running(process_info: dict) -> bool:
    """Check if Claude Code is running in the process tree"""
    return process_tree_has_pattern(
        process_info, ["@anthropic-ai/claude-code", "claude"]
    )


def is_docker_running(process_info: dict) -> bool:
    """Check if docker is running in the process tree"""
    return process_tree_has_pattern(process_info, ["docker"])


def get_tmux_pane_pid(pane_id: str | None = None) -> int:
    """Get the process ID of the specified tmux pane or current pane if none specified"""
    cmd = ["tmux", "display-message"]
    if pane_id:
        cmd.extend(["-t", pane_id])
    cmd.extend(["-p", "#{pane_pid}"])

    result = run_tmux_command(cmd, capture_output=True)
    if result:
        try:
            return int(result)
        except ValueError:
            pass
    return os.getpid()  # Fallback to current process if tmux command fails


def get_process_info(pid: int) -> dict:
    """Get detailed information about a process and its children"""
    try:
        process = psutil.Process(pid)
        return {
            "pid": pid,
            "name": process.name(),
            "cmdline": " ".join(process.cmdline()),
            "cwd": process.cwd(),
            "children": [get_process_info(child.pid) for child in process.children()],
        }
    except (psutil.NoSuchProcess, psutil.AccessDenied):
        return {}


def get_short_path(cwd: str, git_repo: str | None) -> str:
    # Define path mappings
    path_mappings = {
        "idvorkin.github.io": "blog",
        "idvorkin": "me",
        # Add more mappings as needed
    }

    # If we're in a git repo, use that for the base name
    if git_repo:
        base_name = path_mappings.get(git_repo, git_repo)
        # Get the path relative to git root
        try:
            rel_path = (
                subprocess.check_output(
                    ["git", "rev-parse", "--show-prefix"], stderr=subprocess.DEVNULL
                )
                .decode("utf-8")
                .strip()
            )
            return f"{base_name}/{rel_path}" if rel_path else base_name
        except subprocess.CalledProcessError:
            return base_name

    # Not in a git repo, try to shorten the path
    home = os.path.expanduser("~")
    if cwd.startswith(home):
        # Replace home directory with ~
        short_path = "~" + cwd[len(home) :]
    else:
        short_path = cwd

    return short_path


def get_python_command(process_info: dict) -> str | None:
    """Get the python script name if it's running in the process tree"""
    # Check children for python processes
    for child in process_info.get("children", []):
        if child.get("name", "").lower().startswith("python"):
            cmdline = child.get("cmdline", "").split()
            if len(cmdline) > 1:
                script_name = Path(cmdline[1]).stem
                return f"py {script_name}"

    # Check children recursively
    for child in process_info.get("children", []):
        if result := get_python_command(child):
            return result
    return None


def get_just_command(process_info: dict) -> str | None:
    """Get the just command if it's running in the process tree"""
    # Check current process
    if process_info.get("name") == "just":
        cmd = process_info.get("cmdline", "").split()
        if len(cmd) > 1:  # Make sure there's a command after 'just'
            return cmd[1]  # Return the actual command being run

    # Check children recursively
    for child in process_info.get("children", []):
        if result := get_just_command(child):
            return result
    return None


def is_utility_process(process: dict) -> bool:
    """Check if a process is a utility that shouldn't count against plain shell detection"""
    utility_processes = {"tmux_helper", "pbcopy"}
    process_name = process.get("name", "")
    # Include any python version when running tmux_helper
    return process_name in utility_processes or process_name.startswith("python")


def has_non_utility_children(process_info: dict) -> bool:
    """Check if the process has any children that aren't utility processes"""
    children = process_info.get("children", [])
    return any(not is_utility_process(child) for child in children)


def get_tmux_session_info():
    """Get information about the current tmux session"""
    try:
        # Get current session name
        session = (
            subprocess.check_output(
                ["tmux", "display-message", "-p", "#{session_name}"]
            )
            .decode("utf-8")
            .strip()
        )

        # Get current window name
        window = (
            subprocess.check_output(["tmux", "display-message", "-p", "#{window_name}"])
            .decode("utf-8")
            .strip()
        )

        return {"session": session, "window": window}
    except subprocess.CalledProcessError:
        return {"session": "", "window": ""}


class TmuxInfo(BaseModel):
    cwd: str
    short_path: str
    app: str
    title: str
    git_repo: str | None = None
    process_tree: dict


@app.command()
def info():
    """Get current directory, latest running app, and window title as JSON"""
    # Get the parent process ID from tmux
    pane_pid = get_tmux_pane_pid()
    process_info = get_process_info(pane_pid)

    # Get current working directory from the process info
    cwd = process_info.get("cwd", "")

    # Get the short path
    git_repo = get_git_repo_name(cwd)
    short_path = get_short_path(cwd, git_repo)

    # Generate title
    title = generate_title(process_info, short_path)

    info = TmuxInfo(
        cwd=cwd,
        short_path=short_path,
        app=process_info.get("name", ""),
        title=title,
        git_repo=git_repo,
        process_tree=process_info,
    )

    print(json.dumps(info.model_dump(), indent=2))

    # Always set the tmux title
    set_tmux_title(title)


@app.command()
def rename_all():
    """Rename all tmux panes based on their current state"""
    panes = get_all_tmux_panes()
    for pane_id in panes:
        # Get process info for this pane
        pane_pid = get_tmux_pane_pid(pane_id)
        process_info = get_process_info(pane_pid)

        # Get current working directory from the process info
        cwd = process_info.get("cwd", "")

        # Get the short path
        git_repo = get_git_repo_name(cwd)
        short_path = get_short_path(cwd, git_repo)

        # Generate title
        title = generate_title(process_info, short_path)

        # Set the title for this pane
        set_tmux_title(title, pane_id)


@app.command()
def rotate():
    """Toggle between even-horizontal and even-vertical layouts"""
    panes, created_new_pane = ensure_two_panes()
    if not panes:
        return

    # If we just created a second pane, set state and return
    if created_new_pane:
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL)
        return

    # Get the current layout state from tmux user option
    # If not set, default to vertical (so first toggle goes to horizontal)
    current_state = get_tmux_option(LAYOUT_STATE_OPTION)

    # Toggle layout based on state
    if current_state == STATE_HORIZONTAL:
        run_tmux_command(["tmux", "select-layout", "even-vertical"])
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_VERTICAL)
    else:
        # Default to horizontal for any other state or if not set
        run_tmux_command(["tmux", "select-layout", "even-horizontal"])
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL)


@app.command()
def third(
    command: str = typer.Argument("", help="Optional command to run in the first pane"),
):
    """Toggle between even layout and 1/3-2/3 layout (works with 2 panes)

    If command is provided, ensures 2 panes exist and runs the command in the smaller (1/3) pane.
    The command can contain spaces and will be executed as-is.

    Examples:
        tmux_helper third                      # Toggle layout
        tmux_helper third "tig status"         # Split and run tig
        tmux_helper third "git diff | delta"   # Split and run git diff
    """
    panes, created_new = ensure_two_panes(command if command else None)
    if not panes or len(panes) != 2:
        return  # Only works with exactly 2 panes

    # Detect current orientation
    orientation = get_layout_orientation()
    if not orientation:
        return

    is_horizontal = orientation == STATE_HORIZONTAL

    # Get current third state
    current_state = get_tmux_option(THIRD_STATE_OPTION)

    # If command is provided, always apply the layout (don't toggle)
    if command:
        # Reset state and apply layout
        set_tmux_option(THIRD_STATE_OPTION, STATE_NORMAL)
        current_state = STATE_NORMAL

    if current_state in [STATE_THIRD_HORIZONTAL, STATE_THIRD_VERTICAL]:
        # Restore to even layout based on current orientation
        if is_horizontal:
            run_tmux_command(["tmux", "select-layout", "even-horizontal"])
        else:
            run_tmux_command(["tmux", "select-layout", "even-vertical"])
        set_tmux_option(THIRD_STATE_OPTION, STATE_NORMAL)
    else:
        # Get window dimensions
        window_info = run_tmux_command(
            ["tmux", "display-message", "-p", "#{window_width},#{window_height}"],
            capture_output=True,
        )
        if not window_info:
            return

        window_width, window_height = map(int, window_info.split(","))

        if is_horizontal:
            # Resize first pane to 33% width (in absolute columns)
            target_width = int(window_width * 0.33)
            run_tmux_command(
                ["tmux", "resize-pane", "-t", panes[0], "-x", str(target_width)]
            )
            set_tmux_option(THIRD_STATE_OPTION, STATE_THIRD_HORIZONTAL)
        else:
            # Resize first pane to 33% height (in absolute lines)
            target_height = int(window_height * 0.33)
            run_tmux_command(
                ["tmux", "resize-pane", "-t", panes[0], "-y", str(target_height)]
            )
            set_tmux_option(THIRD_STATE_OPTION, STATE_THIRD_VERTICAL)

    # If command provided and pane already existed, send command to it
    if command and not created_new:
        run_tmux_command(["tmux", "send-keys", "-t", panes[0], command, "Enter"])

    # If command provided, focus the working pane
    if command:
        run_tmux_command(["tmux", "select-pane", "-t", panes[1]])


if __name__ == "__main__":
    app()
