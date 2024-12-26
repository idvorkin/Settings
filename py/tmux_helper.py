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
    is_aider_running = 'aider' in process_info.get('cmdline', '').lower()
    is_vim_running = any(editor in process_info.get('cmdline', '').lower() for editor in ['vim', 'nvim'])
    
    # Set title based on running processes
    if is_aider_running:
        title = f"ai {short_path}"
    elif is_vim_running:
        title = f"vi {short_path}"
    else:
        # Check if we're in a plain shell (just zsh)
        is_plain_shell = (
            process_info.get('name') == 'zsh' 
            and not process_info.get('children', [])
        )
        if is_plain_shell:
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
