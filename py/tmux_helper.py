#!/usr/bin/env python3

import typer
import subprocess
import json
from pathlib import Path
from typing import Optional
from pydantic import BaseModel
import sys

# Import the Window and Windows models from y.py
sys.path.append(str(Path(__file__).parent))
from y import get_windows

app = typer.Typer(help="A Tmux helper utility", no_args_is_help=True)

class TmuxInfo(BaseModel):
    cwd: str
    app: str
    title: str

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
    )
    
    print(json.dumps(info.model_dump(), indent=2))

if __name__ == "__main__":
    app()
