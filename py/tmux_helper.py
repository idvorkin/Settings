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

def get_process_tree() -> list:
    def build_tree(pid):
        try:
            process = psutil.Process(pid)
            children = process.children()
            if not children:
                return [process.name()]
            return [process.name(), [build_tree(child.pid) for child in children]]
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            return []

    # Get the current process and its parent
    current_process = psutil.Process()
    return build_tree(current_process.ppid())

class TmuxInfo(BaseModel):
    cwd: str
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

    info = TmuxInfo(
        cwd=cwd,
        app=focused_window.app if focused_window else "",
        title=focused_window.title if focused_window else "",
        git_repo=get_git_repo_name(),
        process_tree=get_process_tree()
    )
    
    print(json.dumps(info.model_dump(), indent=2))

if __name__ == "__main__":
    app()
