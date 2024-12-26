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


app = typer.Typer(help="A Tmux helper utility", no_args_is_help=True)

def get_git_repo_name(cwd: str) -> Optional[str]:
    try:
        git_root = subprocess.check_output(
            ['git', 'rev-parse', '--show-toplevel'],
            stderr=subprocess.DEVNULL,
            cwd=cwd  # Use the provided cwd
        ).decode('utf-8').strip()
        return os.path.basename(git_root)
    except subprocess.CalledProcessError:
        return None

def is_aider_running(process_info: dict) -> bool:
    """Check if aider is running in the process tree"""
    # Check current process
    if 'aider' in process_info.get('cmdline', '').lower():
        return True
    
    # Check children recursively
    for child in process_info.get('children', []):
        if 'aider' in child.get('cmdline', '').lower():
            return True
        if is_aider_running(child):  # Recursive check
            return True
    return False

def is_vim_running(process_info: dict) -> bool:
    """Check if vim/nvim is running in the process tree"""
    # Check current process
    if any(editor in process_info.get('cmdline', '').lower() for editor in ['vim', 'nvim']):
        return True
    
    # Check children recursively
    for child in process_info.get('children', []):
        if any(editor in child.get('cmdline', '').lower() for editor in ['vim', 'nvim']):
            return True
        if is_vim_running(child):  # Recursive check
            return True
    return False

def get_tmux_pane_pid() -> int:
    """Get the process ID of the current tmux pane"""
    try:
        pane_pid = int(subprocess.check_output(
            ['tmux', 'display-message', '-p', '#{pane_pid}']
        ).decode('utf-8').strip())
        return pane_pid
    except (subprocess.CalledProcessError, ValueError):
        return os.getpid()  # Fallback to current process if tmux command fails

def get_process_info(pid: int) -> dict:
    """Get detailed information about a process and its children"""
    try:
        process = psutil.Process(pid)
        return {
            'pid': pid,
            'name': process.name(),
            'cmdline': ' '.join(process.cmdline()),
            'cwd': process.cwd(),
            'children': [get_process_info(child.pid) for child in process.children()]
        }
    except (psutil.NoSuchProcess, psutil.AccessDenied):
        return {}

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

def is_utility_process(process: dict) -> bool:
    """Check if a process is a utility that shouldn't count against plain shell detection"""
    utility_processes = {
        'tmux_helper',
        'pbcopy',
        'python3.12'  # when running tmux_helper
    }
    return process.get('name', '') in utility_processes

def has_non_utility_children(process_info: dict) -> bool:
    """Check if the process has any children that aren't utility processes"""
    children = process_info.get('children', [])
    return any(not is_utility_process(child) for child in children)

def get_tmux_session_info():
    """Get information about the current tmux session"""
    try:
        # Get current session name
        session = subprocess.check_output(
            ['tmux', 'display-message', '-p', '#{session_name}']
        ).decode('utf-8').strip()
        
        # Get current window name
        window = subprocess.check_output(
            ['tmux', 'display-message', '-p', '#{window_name}']
        ).decode('utf-8').strip()
        
        return {'session': session, 'window': window}
    except subprocess.CalledProcessError:
        return {'session': '', 'window': ''}

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
    cwd = process_info.get('cwd', '')
    
    # Get the short path
    git_repo = get_git_repo_name(cwd)
    short_path = get_short_path(cwd, git_repo)

    # Check process tree for running applications
    # Check process tree for running applications
    if is_aider_running(process_info):
        title = f"ai {short_path}"
    elif is_vim_running(process_info):
        title = f"vi {short_path}"
    elif process_info.get('name') == 'zsh' and not has_non_utility_children(process_info):
        # Only check for plain shell after checking for special apps
        title = f"z {short_path}"
    else:
        title = process_info.get('name', '')

    info = TmuxInfo(
        cwd=cwd,
        short_path=short_path,
        app=process_info.get('name', ''),
        title=title,
        git_repo=git_repo,
        process_tree=process_info
    )
    
    print(json.dumps(info.model_dump(), indent=2))
    
    # Always set the tmux title
    set_tmux_title(title)

if __name__ == "__main__":
    app()
