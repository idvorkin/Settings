#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich"]
# ///
"""Walk up to find .beads/, export fresh JSONL, launch beads_viewer_rust in tree mode."""

import os
import subprocess

import typer
from rich.console import Console

app = typer.Typer(
    no_args_is_help=False,
    pretty_exceptions_enable=False,
    add_completion=False,
)
console = Console()


def find_beads_root() -> str | None:
    """Walk up from cwd to find the nearest directory containing .beads/."""
    d = os.getcwd()
    while d != "/":
        if os.path.isdir(os.path.join(d, ".beads")):
            return d
        d = os.path.dirname(d)
    return None


@app.command(
    context_settings={"allow_extra_args": True, "ignore_unknown_options": True},
)
def main(ctx: typer.Context) -> None:
    """Export beads JSONL and launch beads_viewer_rust from the nearest .beads/ workspace."""
    root = find_beads_root()
    if not root:
        console.print("[red]No .beads/ found in any parent directory[/red]")
        raise typer.Exit(1)

    jsonl = os.path.join(root, ".beads", "beads.jsonl")

    # Fresh export from Dolt
    try:
        os.remove(jsonl)
    except FileNotFoundError:
        pass

    try:
        r = subprocess.run(["br", "export", "--no-memories", "-o", jsonl], cwd=root)
    except FileNotFoundError:
        console.print("[red]'br' command not found on PATH[/red]")
        raise typer.Exit(127)
    if r.returncode != 0:
        raise typer.Exit(r.returncode)

    # Launch beads_viewer_rust with any extra args, from the beads root
    bv_args = ["beads_viewer_rust"] + ctx.args
    os.chdir(root)
    try:
        os.execvp("beads_viewer_rust", bv_args)
    except FileNotFoundError:
        console.print("[red]'beads_viewer_rust' command not found on PATH[/red]")
        console.print(
            "Install with: [cyan]cargo install beads_viewer_rust[/cyan]"
        )
        console.print(
            "See: [blue]https://github.com/Dicklesworthstone/beads_viewer_rust[/blue]"
        )
        raise typer.Exit(127)


if __name__ == "__main__":
    app()
