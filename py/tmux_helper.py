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
from typing import Optional
from pydantic import BaseModel
import psutil


def get_all_tmux_panes() -> list[str]:
    """Get all pane IDs from tmux"""
    try:
        panes = (
            subprocess.check_output(["tmux", "list-panes", "-a", "-F", "#{pane_id}"])
            .decode("utf-8")
            .strip()
            .split("\n")
        )
        return panes
    except subprocess.CalledProcessError:
        return []


def set_tmux_title(title: str, pane_id: str | None = None):
    """Set tmux window title"""
    if title:
        try:
            # First disable automatic renaming
            if pane_id:
                subprocess.run(
                    ["tmux", "set", "-t", pane_id, "automatic-rename", "off"],
                    check=True,
                )
            else:
                subprocess.run(["tmux", "set", "automatic-rename", "off"], check=True)

            # Then set the window title
            # Note: We need to get the window ID from the pane ID
            if pane_id:
                window_id = (
                    subprocess.check_output(
                        ["tmux", "display-message", "-t", pane_id, "-p", "#{window_id}"]
                    )
                    .decode("utf-8")
                    .strip()
                )
                subprocess.run(
                    ["tmux", "rename-window", "-t", window_id, title], check=True
                )
            else:
                subprocess.run(["tmux", "rename-window", title], check=True)
        except subprocess.CalledProcessError:
            pass  # Silently fail if tmux command fails


def generate_title(process_info: dict, short_path: str) -> str:
    """Generate a title based on process information"""
    if is_aider_running(process_info):
        return f"ai {short_path}"
    elif is_claude_code_running(process_info):
        return f"claude {short_path}"
    elif is_vim_running(process_info):
        return f"vi {short_path}"
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


def get_git_repo_name(cwd: str) -> Optional[str]:
    # Don't try to run git commands if we don't have a valid directory
    if not cwd:
        return None

    try:
        git_root = (
            subprocess.check_output(
                ["git", "rev-parse", "--show-toplevel"],
                stderr=subprocess.DEVNULL,
                cwd=cwd,  # Use the provided cwd
            )
            .decode("utf-8")
            .strip()
        )
        return os.path.basename(git_root)
    except subprocess.CalledProcessError:
        return None


def is_aider_running(process_info: dict) -> bool:
    """Check if aider is running in the process tree"""
    # Check current process
    if "aider" in process_info.get("cmdline", "").lower():
        return True

    # Check children recursively
    for child in process_info.get("children", []):
        if "aider" in child.get("cmdline", "").lower():
            return True
        if is_aider_running(child):  # Recursive check
            return True
    return False


def is_vim_running(process_info: dict) -> bool:
    """Check if vim/nvim is running in the process tree"""
    # Check current process
    if any(
        editor in process_info.get("cmdline", "").lower() for editor in ["vim", "nvim"]
    ):
        return True

    # Check children recursively
    for child in process_info.get("children", []):
        if any(
            editor in child.get("cmdline", "").lower() for editor in ["vim", "nvim"]
        ):
            return True
        if is_vim_running(child):  # Recursive check
            return True
    return False


def is_claude_code_running(process_info: dict) -> bool:
    """Check if Claude Code is running in the process tree"""
    # Check current process
    cmdline = process_info.get("cmdline", "").lower()
    if "@anthropic-ai/claude-code" in cmdline or "claude" in cmdline:
        return True

    # Check children recursively
    for child in process_info.get("children", []):
        child_cmdline = child.get("cmdline", "").lower()
        if "@anthropic-ai/claude-code" in child_cmdline or "claude" in child_cmdline:
            return True
        if is_claude_code_running(child):  # Recursive check
            return True
    return False


def get_tmux_pane_pid(pane_id: str | None = None) -> int:
    """Get the process ID of the specified tmux pane or current pane if none specified"""
    try:
        cmd = ["tmux", "display-message"]
        if pane_id:
            cmd.extend(["-t", pane_id])
        cmd.extend(["-p", "#{pane_pid}"])

        pane_pid = int(subprocess.check_output(cmd).decode("utf-8").strip())
        return pane_pid
    except (subprocess.CalledProcessError, ValueError):
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


def get_short_path(cwd: str, git_repo: Optional[str]) -> str:
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
    utility_processes = {
        "tmux_helper",
        "pbcopy",
        "python3.12",  # when running tmux_helper
    }
    return process.get("name", "") in utility_processes


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
    git_repo: Optional[str] = None
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


if __name__ == "__main__":
    app()
