#!/usr/bin/env python3

import typer
import subprocess
import json
import os
from pathlib import Path
from typing import Optional
from pydantic import BaseModel
import sys
import psutil

def set_tmux_title(title: str):
    """Set tmux window title"""
    if title:
        try:
            # First disable automatic renaming
            subprocess.run(['tmux', 'set-window-option', 'automatic-rename', 'off'], check=True)
            # Then set the window title
            subprocess.run(['tmux', 'rename-window', title], check=True)
        except subprocess.CalledProcessError:
            pass  # Silently fail if tmux command fails

# Import the Window and Windows models from y.py
sys.path.append(str(Path(__file__).parent))
from y import get_windows

app = typer.Typer(help="A Tmux helper utility", no_args_is_help=True)

def get_git_repo_name() -> Optional[str]:
    try:
        git_root = subprocess.check_output(
            ['git', 'rev-parse', '--show-toplevel'],
            stderr=subprocess.DEVNULL
        ).decode('utf-8').strip()
        return os.path.basename(git_root)
    except subprocess.CalledProcessError:
        return None

def is_aider_in_tree(process_tree) -> bool:
    """Recursively check if aider is running in the process tree"""
    for item in process_tree:
        if isinstance(item, dict) and 'cmdline' in item:
            if 'aider' in item['cmdline'].lower():
                return True
        elif isinstance(item, list):
            if is_aider_in_tree(item):
                return True
    return False

def is_vim_in_tree(process_tree) -> bool:
    """Recursively check if vim/nvim is running in the process tree"""
    for item in process_tree:
        if isinstance(item, dict) and 'cmdline' in item:
            if any(editor in item['cmdline'].lower() for editor in ['vim', 'nvim']):
                return True
        elif isinstance(item, list):
            if is_vim_in_tree(item):
                return True
    return False

def get_process_tree() -> list:
    def build_tree(pid):
        try:
            process = psutil.Process(pid)
            children = process.children()
            # Include both name and cmdline for each process
            process_info = {
                'name': process.name(),
                'cmdline': ' '.join(process.cmdline())
            }
            if not children:
                return [process_info]
            return [process_info, [build_tree(child.pid) for child in children]]
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            return []

    # Get the pane process ID from tmux
    try:
        pane_pid = int(subprocess.check_output(
            ['tmux', 'display-message', '-p', '#{pane_pid}']
        ).decode('utf-8').strip())
        return build_tree(pane_pid)
    except (subprocess.CalledProcessError, ValueError):
        return []

def get_short_path(cwd: str, git_repo: Optional[str]) -> str:
    # Define path mappings
    path_mappings = {
        'idvorkin.github.io': 'blog',
        'idvorkin': 'me',
        # Add more mappings as needed
    }

    # If we're in a git repo, use that for the base name
    if git_repo:
        base_name = path_mappings.get(git_repo, git_repo)
        # Get the path relative to git root
        try:
            rel_path = subprocess.check_output(
                ['git', 'rev-parse', '--show-prefix'],
                stderr=subprocess.DEVNULL
            ).decode('utf-8').strip()
            return f"{base_name}/{rel_path}" if rel_path else base_name
        except subprocess.CalledProcessError:
            return base_name
    
    # Not in a git repo, try to shorten the path
    home = os.path.expanduser('~')
    if cwd.startswith(home):
        # Replace home directory with ~
        short_path = '~' + cwd[len(home):]
    else:
        short_path = cwd
    
    return short_path

class TmuxInfo(BaseModel):
    cwd: str
    short_path: str
    app: str
    title: str
    git_repo: Optional[str] = None
    process_tree: list

@app.command()
def info():
    """Get current directory, latest running app, and window title as JSON"""
    # Get the focused window information
    windows = get_windows()
    focused_window = next((w for w in windows.windows if w.has_focus), None)
    
    # Get current working directory using pwd
    try:
        cwd = subprocess.check_output(['pwd']).decode('utf-8').strip()
    except subprocess.CalledProcessError:
        cwd = ""

    # Get the short path
    short_path = get_short_path(cwd, get_git_repo_name())

    # Check if aider or vim is running in the process tree
    process_tree = get_process_tree()
    is_aider_running = is_aider_in_tree(process_tree)
    is_vim_running = is_vim_in_tree(process_tree)

    # Set title based on running processes
    if is_aider_running:
        title = f"ai {short_path}"
    elif is_vim_running:
        title = f"vi {short_path}"
    else:
        title = focused_window.title if focused_window else ""

    info = TmuxInfo(
        cwd=cwd,
        short_path=short_path,
        app=focused_window.app if focused_window else "",
        title=title,
        git_repo=get_git_repo_name(),
        process_tree=process_tree
    )
    
    print(json.dumps(info.model_dump(), indent=2))
    
    # Set the tmux pane title
    if is_aider_running or is_vim_running:
        set_tmux_title(title)

if __name__ == "__main__":
    app()
