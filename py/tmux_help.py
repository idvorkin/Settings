#!/usr/bin/env python3

from typing import List
from pydantic import BaseModel
import typer
import subprocess

app = typer.Typer()

def run_tmux_command(command: str) -> str:
    """Run a tmux command and return its output"""
    result = subprocess.run(['tmux'] + command.split(), 
                          capture_output=True, 
                          text=True)
    return result.stdout.strip()

@app.command()
def list_commands():
    """List all tmux commands with their descriptions"""
    help_output = run_tmux_command("list-commands")
    for line in help_output.split('\n'):
        print(line)

@app.command()
def show_bindings():
    """Show all current tmux key bindings"""
    bindings = run_tmux_command("list-keys")
    for line in bindings.split('\n'):
        print(line)

if __name__ == "__main__":
    app()
