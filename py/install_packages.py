#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich"]
# ///
"""
Package installation manager for Igor's settings.

Handles installation of Python tools via UV with optional force/upgrade flags.
"""

import subprocess
import sys

import typer
from rich.console import Console
from rich.progress import track

app = typer.Typer(
    no_args_is_help=False,
    pretty_exceptions_enable=False,
    add_completion=False,
    help="Install and manage Python packages via UV",
)

console = Console()


def run_command(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    try:
        result = subprocess.run(
            cmd,
            check=check,
            capture_output=True,
            text=True,
        )
        return result
    except subprocess.CalledProcessError as e:
        console.print(f"[red]Error running command:[/red] {' '.join(cmd)}")
        console.print(f"[red]Error message:[/red] {e.stderr}")
        raise


# Python tools to install via UV
PYTHON_TOOLS = [
    ("aider-chat", ["--python", "python3.12"], "Code Writing helper"),
    ("ruff", [], "Fast Python linter"),
    ("httpx", [], "HTTP client with CLI"),
    ("pre-commit", [], "Git pre-commit hooks framework"),
    ("jupyterlab", [], "Jupyter notebook interface"),
    ("rich-cli", [], "Terminal formatting tool"),
    ("claude-code-log", [], "Claude Code conversation logger"),
    ("black", [], "Code formatter"),
    ("mypy", [], "Static type checker"),
    ("poetry", [], "Python package manager"),
    ("uvicorn", [], "ASGI server"),
    ("pudb", [], "Console-based visual debugger"),
]


@app.command()
def install(
    force: bool = typer.Option(
        False,
        "--force",
        help="Force reinstall all packages",
        envvar="UV_FORCE",
    ),
):
    """Install Python tools via UV."""

    # Determine install flag
    install_flag = "--force" if force else "--upgrade"
    mode = "reinstalling" if force else "updating if needed"

    console.print(
        f"\n[bold cyan]Installing Python tools with {install_flag} flag ({mode})...[/bold cyan]\n"
    )

    # Install pipxu first (pipx upgrade tool)
    console.print("[yellow]Installing pipxu via pipx...[/yellow]")
    try:
        run_command(["pipx", "install", "pipxu"], check=False)
    except Exception as e:
        console.print(f"[yellow]Warning: Could not install pipxu: {e}[/yellow]")

    # Install Python tools
    failed_tools = []
    for tool_name, extra_args, description in track(
        PYTHON_TOOLS, description="Installing Python tools..."
    ):
        try:
            cmd = ["uv", "tool", "install", install_flag, *extra_args, tool_name]
            console.print(f"  [dim]Installing {tool_name}...[/dim]")
            run_command(cmd)
            console.print(f"  [green]✓[/green] {tool_name} - {description}")
        except subprocess.CalledProcessError:
            failed_tools.append((tool_name, description))
            console.print(f"  [red]✗[/red] {tool_name} - Failed to install")

    # Summary
    console.print("\n[bold green]Installation complete![/bold green]")
    console.print(
        f"Successfully installed: {len(PYTHON_TOOLS) - len(failed_tools)}/{len(PYTHON_TOOLS)}"
    )

    if failed_tools:
        console.print("\n[bold red]Failed to install:[/bold red]")
        for tool_name, description in failed_tools:
            console.print(f"  [red]✗[/red] {tool_name} - {description}")
        sys.exit(1)


@app.command()
def list_tools():
    """List all Python tools that will be installed."""
    console.print("\n[bold cyan]Python Tools:[/bold cyan]\n")
    for tool_name, extra_args, description in PYTHON_TOOLS:
        extra = f" {' '.join(extra_args)}" if extra_args else ""
        console.print(f"  • [bold]{tool_name}[/bold]{extra}")
        console.print(f"    {description}\n")


if __name__ == "__main__":
    app()
