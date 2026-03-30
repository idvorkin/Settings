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
import shlex
import time
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
SIDE_EDIT_PANE_OPTION = "@side_edit_pane_id"


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
    """Get tmux window-local option value (matches Rust -wqv convention)"""
    return (
        run_tmux_command(["tmux", "show-option", "-wqv", option], capture_output=True)
        or ""
    )


def set_tmux_option(option: str, value: str) -> None:
    """Set tmux window-local option (matches Rust -w convention)"""
    run_tmux_command(["tmux", "set-option", "-w", option, value])


def get_caller_pane_id() -> str | None:
    """Return the pane ID of the shell that invoked this script.

    Uses $TMUX_PANE which tmux sets in every pane's environment.
    This is reliable even when the window is in the background, unlike
    'tmux display-message -p' which targets the currently active pane.
    """
    return os.environ.get("TMUX_PANE") or None


def get_window_option(option: str, window_target: str | None = None) -> str:
    """Get a window-local tmux option value."""
    cmd = ["tmux", "show-option", "-wqv"]
    if window_target:
        cmd.extend(["-t", window_target])
    cmd.append(option)
    return run_tmux_command(cmd, capture_output=True) or ""


def set_window_option(
    option: str, value: str, window_target: str | None = None
) -> None:
    """Set a window-local tmux option value."""
    cmd = ["tmux", "set-option", "-w"]
    if window_target:
        cmd.extend(["-t", window_target])
    cmd.extend([option, value])
    run_tmux_command(cmd)


def get_panes_in_window(window_target: str) -> list[str]:
    """Return list of pane IDs in the window that contains window_target."""
    result = run_tmux_command(
        ["tmux", "list-panes", "-t", window_target, "-F", "#{pane_id}"],
        capture_output=True,
    )
    if not result:
        return []
    return [p for p in result.split("\n") if p]


def get_pane_cwd(pane_id: str) -> str:
    """Return the current working directory of pane_id, or '' on error."""
    return (
        run_tmux_command(
            ["tmux", "display-message", "-t", pane_id, "-p", "#{pane_current_path}"],
            capture_output=True,
        )
        or ""
    )


def is_vim_in_pane(pane_id: str) -> bool:
    """Return True if nvim/vim is the foreground process in pane_id."""
    pid_str = run_tmux_command(
        ["tmux", "display-message", "-t", pane_id, "-p", "#{pane_pid}"],
        capture_output=True,
    )
    if not pid_str or not pid_str.isdigit():
        return False
    process_info = get_process_info(int(pid_str))
    return is_vim_running(process_info)


def is_pane_safe_to_adopt(pane_id: str) -> bool:
    """Return True if pane is running vim/nvim or an idle shell (safe for side-edit).

    Returns False if pane is running another foreground process (tig, REPL, htop, etc.)
    to avoid injecting keystrokes into arbitrary programs.
    """
    pid_str = run_tmux_command(
        ["tmux", "display-message", "-t", pane_id, "-p", "#{pane_pid}"],
        capture_output=True,
    )
    if not pid_str or not pid_str.isdigit():
        return False
    process_info = get_process_info(int(pid_str))
    if is_vim_running(process_info):
        return True
    # Idle shell: zsh/bash/sh with no non-utility children
    if process_info.get("name") in (
        "zsh",
        "bash",
        "sh",
    ) and not has_non_utility_children(process_info):
        return True
    return False


def ensure_two_panes(
    command: str | None = None, caller_pane_id: str | None = None
) -> tuple[list[str], bool]:
    """Ensure window has at least 2 panes, return (pane IDs, created_new_pane).

    Args:
        command: Optional command to run in the new pane when created
        caller_pane_id: Pane ID to anchor to (defaults to $TMUX_PANE / active pane).
                        Fixes background-window targeting when provided.
    """
    target = caller_pane_id

    list_cmd = ["tmux", "list-panes", "-F", "#{pane_id}"]
    if target:
        list_cmd = ["tmux", "list-panes", "-t", target, "-F", "#{pane_id}"]

    panes = run_tmux_command(list_cmd, capture_output=True)
    if not panes:
        return ([], False)

    pane_list = [p for p in panes.split("\n") if p]
    if len(pane_list) == 1:
        cwd = get_pane_cwd(target) if target else "#{pane_current_path}"
        split_cmd = ["tmux", "split-window", "-h", "-c", cwd]
        if target:
            split_cmd.extend(["-t", target])
        if command:
            split_cmd.append(command)
        run_tmux_command(split_cmd)

        layout_cmd = ["tmux", "select-layout", "even-horizontal"]
        if target:
            layout_cmd = ["tmux", "select-layout", "-t", target, "even-horizontal"]
        run_tmux_command(layout_cmd)

        panes = run_tmux_command(list_cmd, capture_output=True)
        pane_list = [p for p in panes.split("\n") if p] if panes else []
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


def get_all_pane_info() -> list[dict]:
    """Get pane_id, window_id, window_name, and pane_pid for all panes in one call"""
    # Batch fetch all info we need in a single tmux command
    result = run_tmux_command(
        [
            "tmux",
            "list-panes",
            "-a",
            "-F",
            "#{pane_id}\t#{window_id}\t#{window_name}\t#{pane_pid}",
        ],
        capture_output=True,
    )
    if not result:
        return []

    panes = []
    for line in result.split("\n"):
        if not line:
            continue
        parts = line.split("\t")
        if len(parts) == 4:
            panes.append(
                {
                    "pane_id": parts[0],
                    "window_id": parts[1],
                    "window_name": parts[2],
                    "pane_pid": int(parts[3]) if parts[3].isdigit() else 0,
                }
            )
    return panes


# Cache for git repo lookups (cleared each rename_all call)
_git_repo_cache: dict[str, str | None] = {}


def set_tmux_title(
    title: str,
    pane_id: str | None = None,
    window_id: str | None = None,
    current_name: str | None = None,
):
    """Set tmux window title, skipping if unchanged

    Args:
        title: New window title
        pane_id: Optional pane ID (used to find window if window_id not provided)
        window_id: Optional window ID (avoids extra tmux call if provided)
        current_name: Current window name (skips rename if matches title)
    """
    if not title:
        return

    # Skip if title hasn't changed
    if current_name is not None and current_name == title:
        return

    # First disable automatic renaming
    if pane_id:
        run_tmux_command(["tmux", "set", "-t", pane_id, "automatic-rename", "off"])
    else:
        run_tmux_command(["tmux", "set", "automatic-rename", "off"])

    # Then set the window title
    if window_id:
        run_tmux_command(["tmux", "rename-window", "-t", window_id, title])
    elif pane_id:
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


def get_git_repo_name(cwd: str, use_cache: bool = False) -> str | None:
    """Get the name of the git repository from the given directory

    Args:
        cwd: Current working directory
        use_cache: If True, use cached result for this directory
    """
    if not cwd:
        return None

    # Check cache first
    if use_cache and cwd in _git_repo_cache:
        return _git_repo_cache[cwd]

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
        result = os.path.basename(git_root)
    except subprocess.CalledProcessError:
        result = None

    if use_cache:
        _git_repo_cache[cwd] = result
    return result


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
    """Get the process ID of the specified tmux pane.

    When pane_id is None, uses $TMUX_PANE (caller's pane) to avoid
    the background-window bug where display-message targets the active pane.
    """
    target = pane_id or get_caller_pane_id()
    cmd = ["tmux", "display-message"]
    if target:
        cmd.extend(["-t", target])
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
    """Rename all tmux panes based on their current state

    Optimizations:
    - Fetches all pane info in a single tmux command
    - Skips renaming if window name hasn't changed
    - Caches git repo lookups per directory
    - Only renames first pane per window (all panes in a window share the name)
    """
    global _git_repo_cache
    _git_repo_cache.clear()  # Clear cache at start of batch operation

    panes = get_all_pane_info()

    # Track which windows we've already renamed (all panes in a window share the name)
    renamed_windows: set[str] = set()

    for pane in panes:
        window_id = pane["window_id"]

        # Skip if we've already renamed this window
        if window_id in renamed_windows:
            continue
        renamed_windows.add(window_id)

        # Get process info for this pane
        pane_pid = pane["pane_pid"]
        if pane_pid <= 0:
            continue

        process_info = get_process_info(pane_pid)
        if not process_info:
            continue

        # Get current working directory from the process info
        cwd = process_info.get("cwd", "")

        # Get the short path (with caching)
        git_repo = get_git_repo_name(cwd, use_cache=True)
        short_path = get_short_path(cwd, git_repo)

        # Generate title
        title = generate_title(process_info, short_path)

        # Set the title, skipping if unchanged
        set_tmux_title(
            title,
            pane_id=pane["pane_id"],
            window_id=window_id,
            current_name=pane["window_name"],
        )


@app.command()
def rotate():
    """Toggle between even-horizontal and even-vertical layouts"""
    caller_pane_id = get_caller_pane_id()
    panes, created_new_pane = ensure_two_panes(caller_pane_id=caller_pane_id)
    if not panes:
        return

    # If we just created a second pane, set state and return
    if created_new_pane:
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL)
        return

    # Get the current layout state from tmux user option
    # If not set, default to vertical (so first toggle goes to horizontal)
    current_state = get_tmux_option(LAYOUT_STATE_OPTION)

    # Build select-layout command with background-window-safe targeting
    def select_layout(layout: str) -> None:
        cmd = ["tmux", "select-layout"]
        if caller_pane_id:
            cmd.extend(["-t", caller_pane_id])
        cmd.append(layout)
        run_tmux_command(cmd)

    # Toggle layout based on state
    if current_state == STATE_HORIZONTAL:
        select_layout("even-vertical")
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_VERTICAL)
    else:
        # Default to horizontal for any other state or if not set
        select_layout("even-horizontal")
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL)


def session_exists(session: str) -> bool:
    """Check if tmux session exists"""
    return (
        subprocess.run(
            ["tmux", "has-session", "-t", session],
            capture_output=True,
        ).returncode
        == 0
    )


def get_session_pane_pids(session: str) -> list[int]:
    """Get all pane PIDs in a session"""
    result = run_tmux_command(
        ["tmux", "list-panes", "-t", session, "-a", "-F", "#{pane_pid}"],
        capture_output=True,
    )
    if not result:
        return []
    return [int(pid) for pid in result.split("\n") if pid.isdigit()]


def is_process_running_in_session(session: str, process_name: str) -> bool:
    """Check if a process is running in any pane of the session"""
    for pane_pid in get_session_pane_pids(session):
        try:
            proc = psutil.Process(pane_pid)
            # Check the process and all children
            for p in [proc, *proc.children(recursive=True)]:
                if process_name.lower() in p.name().lower():
                    return True
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            continue
    return False


@app.command()
def launch_servers():
    """Start a 'servers' session with dev services (btm, jekyll, agent-dashboard)

    Creates windows for:
    - btm (system monitor)
    - caffeinate (Mac only - keeps machine awake)
    - jekyll serve in ~/blog (Mac only)
    - just dev in ~/gits/agent-dashboard

    Checks by running process (not window name). Disables auto-rename for these windows.
    """
    import platform

    session = "servers"
    is_mac = platform.system() == "Darwin"

    # Check if session exists
    created_new = not session_exists(session)
    if not created_new:
        print(f"Session '{session}' exists, checking processes...")
    else:
        print(f"Creating session '{session}'...")
        subprocess.run(
            ["tmux", "new-session", "-d", "-s", session, "-n", "monitor", "btm"],
            check=True,
        )
        # Disable auto-rename for this window
        run_tmux_command(
            [
                "tmux",
                "set-option",
                "-t",
                f"{session}:monitor",
                "automatic-rename",
                "off",
            ]
        )
        print("  monitor: started")

    # Helper to create window if process not running
    def ensure_process(
        name: str, process: str, cmd: list[str], cwd: Path | None = None
    ):
        if is_process_running_in_session(session, process):
            print(f"  {name}: already running")
            return False
        args = ["tmux", "new-window", "-t", session, "-n", name]
        if cwd:
            args.extend(["-c", str(cwd)])
        args.extend(cmd)
        subprocess.run(args, check=True)
        # Disable auto-rename for this window
        run_tmux_command(
            ["tmux", "set-option", "-t", f"{session}:{name}", "automatic-rename", "off"]
        )
        print(f"  {name}: started")
        return True

    # btm (only check if session already existed)
    if not created_new:
        ensure_process("monitor", "btm", ["btm"])

    # Desktop Mac only - caffeinate to keep machine awake (skip on laptops)
    if is_mac:
        # sysctl hw.model returns "Mac16,12" not "MacBook", so use system_profiler
        result = subprocess.run(
            ["system_profiler", "SPHardwareDataType"],
            capture_output=True,
            text=True,
        )
        is_laptop = "MacBook" in result.stdout
        if not is_laptop:
            ensure_process("awake", "caffeinate", ["caffeinate", "-d", "-i", "-s"])

    # Mac-only windows
    if is_mac:
        blog_dir = Path.home() / "blog"
        if blog_dir.exists():
            ensure_process("blog", "jekyll", ["jekyll", "serve"], blog_dir)
        else:
            print(f"  blog: {blog_dir} not found, skipping")

    # Agent dashboard
    agent_dir = Path.home() / "gits" / "agent-dashboard"
    if agent_dir.exists():
        ensure_process("agent", "just", ["just", "dev"], agent_dir)
    else:
        print(f"  agent: {agent_dir} not found, skipping")

    # Attach or switch to session
    if os.environ.get("TMUX"):
        os.execvp("tmux", ["tmux", "switch-client", "-t", session])
    else:
        os.execvp("tmux", ["tmux", "attach", "-t", session])


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
    caller_pane_id = get_caller_pane_id()
    panes, created_new = ensure_two_panes(
        command if command else None, caller_pane_id=caller_pane_id
    )
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
        # Get window dimensions — use caller pane as target for background safety
        dim_cmd = ["tmux", "display-message"]
        if caller_pane_id:
            dim_cmd.extend(["-t", caller_pane_id])
        dim_cmd.extend(["-p", "#{window_width},#{window_height}"])
        window_info = run_tmux_command(dim_cmd, capture_output=True)
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


def escape_for_vim_ex(path: str) -> str:
    """Escape a path for use in a vim/nvim Ex command like :e.

    Nvim Ex commands use backslash-escaping, not shell quoting.
    """
    return (
        path.replace("\\", "\\\\")
        .replace(" ", "\\ ")
        .replace("#", "\\#")
        .replace("%", "\\%")
    )


def create_side_pane(caller_pane_id: str, shell_file: str) -> str | None:
    """Split caller's pane and start nvim in the new pane.

    Args:
        caller_pane_id: Pane ID to split from.
        shell_file: Shell-quoted file path (via shlex.quote).

    Returns the new pane's ID, or None on failure.
    Respects @third_state: if active, re-applies the 1/3 sizing to caller.
    Polls briefly for the new pane's shell to be ready before sending nvim.
    """
    caller_cwd = get_pane_cwd(caller_pane_id) or os.path.expanduser("~")

    third_state = get_tmux_option(THIRD_STATE_OPTION)
    is_third_active = third_state in [STATE_THIRD_HORIZONTAL, STATE_THIRD_VERTICAL]

    # Split right from caller, capture new pane ID directly via -P -F
    new_pane_id = run_tmux_command(
        [
            "tmux",
            "split-window",
            "-h",
            "-c",
            caller_cwd,
            "-t",
            caller_pane_id,
            "-P",
            "-F",
            "#{pane_id}",
        ],
        capture_output=True,
    )
    if not new_pane_id:
        return None
    new_pane_id = new_pane_id.strip()

    # If @third_state is active, resize caller to keep its 1/3 width
    if is_third_active:
        window_width_str = run_tmux_command(
            ["tmux", "display-message", "-t", caller_pane_id, "-p", "#{window_width}"],
            capture_output=True,
        )
        if window_width_str and window_width_str.isdigit():
            target_caller_width = int(int(window_width_str) * 0.33)
            run_tmux_command(
                [
                    "tmux",
                    "resize-pane",
                    "-t",
                    caller_pane_id,
                    "-x",
                    str(target_caller_width),
                ]
            )

    # Poll for new pane readiness (up to 0.5s in 50ms steps)
    deadline = time.monotonic() + 0.5
    while time.monotonic() < deadline:
        pid_str = run_tmux_command(
            ["tmux", "display-message", "-t", new_pane_id, "-p", "#{pane_pid}"],
            capture_output=True,
        )
        if pid_str and pid_str.isdigit() and int(pid_str) > 0:
            try:
                proc = psutil.Process(int(pid_str))
                if proc.status() != psutil.STATUS_ZOMBIE:
                    break
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass
        time.sleep(0.05)

    # Launch nvim in the new pane (shell context — use shell-quoted path)
    run_tmux_command(
        ["tmux", "send-keys", "-t", new_pane_id, f"nvim {shell_file}", "Enter"]
    )

    # Restore focus to caller
    run_tmux_command(["tmux", "select-pane", "-t", caller_pane_id])
    return new_pane_id


def open_file_in_pane(pane_id: str, shell_file: str, vim_file: str) -> None:
    """Open a file in an existing pane, reusing nvim if already running.

    Args:
        pane_id: Target pane ID.
        shell_file: Shell-quoted file path (via shlex.quote) for launching nvim.
        vim_file: Vim Ex-escaped file path (via escape_for_vim_ex) for :e command.

    Key sequence handles: normal, insert, visual, command-line, and terminal modes.
    Double Escape + C-\\ C-n ensures we reach normal mode from any state.
    """
    if is_vim_in_pane(pane_id):
        # Double Escape handles insert/visual/command-line modes
        run_tmux_command(["tmux", "send-keys", "-t", pane_id, "Escape"])
        run_tmux_command(["tmux", "send-keys", "-t", pane_id, "Escape"])
        # C-\ C-n exits terminal mode (no-op in normal mode)
        run_tmux_command(["tmux", "send-keys", "-t", pane_id, "C-\\"])
        run_tmux_command(["tmux", "send-keys", "-t", pane_id, "C-n"])
        # Now reliably in normal mode — open the file (vim escaping, not shell)
        run_tmux_command(
            ["tmux", "send-keys", "-t", pane_id, f":e {vim_file}", "Enter"]
        )
    else:
        # No nvim running — launch it fresh (shell context — use shell quoting)
        run_tmux_command(
            ["tmux", "send-keys", "-t", pane_id, f"nvim {shell_file}", "Enter"]
        )


@app.command()
def side_edit(
    file: str = typer.Argument(..., help="File path to open in the side pane"),
):
    """Open a file in a side nvim pane, reusing an existing side pane when possible.

    Uses $TMUX_PANE to reliably target the caller's window even when backgrounded.
    Stores the side pane ID as a window-local option (@side_edit_pane_id).

    Examples:
        tmux_helper side-edit /path/to/file.py
        tmux_helper side-edit ~/notes/todo.md
    """
    # 1. Verify we are inside tmux
    caller_pane_id = get_caller_pane_id()
    if not caller_pane_id:
        print("ERROR: $TMUX_PANE is not set. side-edit must be run inside a tmux pane.")
        raise typer.Exit(1)

    # 2. Resolve file path (expand ~ and make absolute relative to caller's cwd)
    file_path = Path(os.path.expanduser(file))
    if not file_path.is_absolute():
        caller_cwd = get_pane_cwd(caller_pane_id)
        if caller_cwd:
            file_path = Path(caller_cwd) / file_path
        else:
            file_path = Path.cwd() / file_path
    shell_file = shlex.quote(str(file_path))  # For shell: nvim '/path/to/file'
    vim_file = escape_for_vim_ex(str(file_path))  # For nvim Ex: :e /path/to/file

    # 3. Enumerate panes in caller's window
    window_panes = get_panes_in_window(caller_pane_id)
    if not window_panes:
        print("ERROR: Could not list panes in current window.")
        raise typer.Exit(1)

    # 4. Look up stored side-edit pane
    stored_pane_id = get_window_option(
        SIDE_EDIT_PANE_OPTION, window_target=caller_pane_id
    )

    side_pane_id: str | None = None
    if (
        stored_pane_id
        and stored_pane_id in window_panes
        and stored_pane_id != caller_pane_id
    ):
        side_pane_id = stored_pane_id

    # 5. Decide what to do
    if side_pane_id is None:
        other_panes = [p for p in window_panes if p != caller_pane_id]

        if len(other_panes) == 0:
            # Only caller pane — create a split
            side_pane_id = create_side_pane(caller_pane_id, shell_file)
            if side_pane_id is None:
                print("ERROR: Failed to create side pane.")
                raise typer.Exit(1)
            set_window_option(
                SIDE_EDIT_PANE_OPTION, side_pane_id, window_target=caller_pane_id
            )
            return  # _create_side_pane already launched nvim and restored focus

        elif len(other_panes) == 1:
            candidate = other_panes[0]
            if not is_pane_safe_to_adopt(candidate):
                print(
                    "ERROR: The other pane is running a foreground process. "
                    "Close it or use a 1-pane window so side-edit can create its own."
                )
                raise typer.Exit(1)
            # Pane is idle shell or vim — safe to adopt
            side_pane_id = candidate
            set_window_option(
                SIDE_EDIT_PANE_OPTION, side_pane_id, window_target=caller_pane_id
            )

        else:
            # 2+ other panes — can't safely pick one
            print(
                f"ERROR: Window has {len(other_panes)} other panes and no registered "
                f"side-edit pane. Close extra panes or run from a 1-pane window."
            )
            raise typer.Exit(1)

    # 6. Open file in the side pane
    open_file_in_pane(side_pane_id, shell_file, vim_file)

    # 7. Restore focus to caller
    run_tmux_command(["tmux", "select-pane", "-t", caller_pane_id])


if __name__ == "__main__":
    app()
