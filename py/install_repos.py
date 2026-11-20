#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "pydantic"]
# ///
"""
Clone Igor's active repos into ~/gits and run their install steps.

Cloning uses `gh repo clone`, so private repos need `gh auth login` first.
Install steps are shell commands run inside the checkout; each is meant to be
safe to re-run.
"""

import subprocess
import sys
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Annotated

import typer
from pydantic import BaseModel, Field
from rich.console import Console

app = typer.Typer(
    no_args_is_help=True,
    pretty_exceptions_enable=False,
    add_completion=False,
    help="Clone and install Igor's active repos",
)

console = Console()

GITS = Path.home() / "gits"
GH_OWNER = "idvorkin"


class Repo(BaseModel):
    name: str
    owner: str = Field(GH_OWNER, description="GitHub account that holds the repo")
    install: list[str] = Field(
        default_factory=list, description="Shell commands run in the checkout"
    )
    note: str = Field("", description="Manual follow-up printed after install")


# Link the checkout as Alfred's preferences, backing up whatever was there.
ALFRED_LINK_PREFS = """
prefs="$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences"
[ "$(readlink "$prefs")" = "$PWD" ] && exit 0
pkill -x Alfred; pkill -x "Alfred Preferences"
mkdir -p "$(dirname "$prefs")"
if [ -e "$prefs" ] || [ -L "$prefs" ]; then mv -n "$prefs" "$prefs.backup-$(date +%Y%m%d-%H%M%S)"; fi
ln -s "$PWD" "$prefs"
./tools/alfred_local_prefs.py apply
open -b com.runningwithcrayons.Alfred
"""

# fmt: off
REPOS: list[Repo] = [
    # Tools
    Repo(name="alfred", install=[ALFRED_LINK_PREFS],
         note="Alfred: confirm Igor Tools under Workflows and activate the Powerpack"),
    # yabai is built from the fork by alfred's recipe, so alfred must come first
    Repo(name="yabai", install=["just -f ../alfred/justfile -d ../alfred yabai-install"],
         note="yabai: re-grant Accessibility if macOS prompts"),
    Repo(name="browser-tab-helper", install=["node tools/host.js install"],
         note="browser-tab-helper: edge://extensions → Developer mode → Load unpacked → ~/gits/browser-tab-helper"),
    Repo(name="igsnap", install=["just install"]),
    # The larry CLI that Alfred's Agent Send (`w`) shells out to
    Repo(name="cockpit", owner="idvorkin-ai-tools",
         install=["mkdir -p ~/.local/bin && ln -sfn \"$PWD/larry.py\" ~/.local/bin/larry"],
         note="larry: put {\"url\": ..., \"token\": <COCKPIT_CONTROL_TOKEN>} in ~/.config/larry/config.json (chmod 600)"),
    # Alfred's `sw` keyword drives the fork's raccoon, which the brew cask lacks
    Repo(name="window-sweaters", install=["./scripts/build-app.sh", "python3 scripts/install-local.py"]),
    # Native Swift apps: Mac ones install to /Applications, iOS/watch ones are clone only
    Repo(name="magic-monitor-native", install=["just install"]),
    Repo(name="context-grabber", install=["just setup"]),
    Repo(name="exercise-analyzer"),
    Repo(name="bug-kit"),
    Repo(name="think-a-card-trainer"),
    Repo(name="lcd-timer", install=["just install"]),
    Repo(name="audio-debugger", install=["just live-install"]),
]
# fmt: on


class Runner(ABC):
    """Runs commands, so tests can swap in a fake."""

    @abstractmethod
    def run(self, cmd: list[str], cwd: Path | None = None) -> bool: ...


class ShellRunner(Runner):
    def run(self, cmd: list[str], cwd: Path | None = None) -> bool:
        return subprocess.run(cmd, check=False, cwd=cwd).returncode == 0


runner: Runner = ShellRunner()


def select_repos(names: list[str]) -> list[Repo]:
    """Return the named repos in REPOS order, or all of them when none are named."""
    if not names:
        return REPOS
    known = {r.name for r in REPOS}
    unknown = [n for n in names if n not in known]
    if unknown:
        console.print(f"[red]Unknown repos: {', '.join(unknown)}[/red]")
        raise typer.Exit(1)
    return [r for r in REPOS if r.name in names]


def clone_repo(repo: Repo, gits: Path) -> bool:
    """Clone the repo unless a checkout already exists."""
    if (gits / repo.name).exists():
        console.print(f"  [dim]✓ {repo.name} already cloned[/dim]")
        return True
    gits.mkdir(parents=True, exist_ok=True)
    console.print(f"  [yellow]Cloning {repo.name}...[/yellow]")
    return runner.run(["gh", "repo", "clone", f"{repo.owner}/{repo.name}"], cwd=gits)


def install_repo(repo: Repo, gits: Path) -> bool:
    """Run the repo's install steps, stopping at the first failure."""
    for step in repo.install:
        console.print(f"  [yellow]Installing {repo.name}...[/yellow]")
        if not runner.run(["bash", "-c", step], cwd=gits / repo.name):
            return False
    return True


def report(failed: list[str], notes: list[str]) -> None:
    if notes:
        console.print("\n[bold cyan]Manual follow-ups:[/bold cyan]")
        for note in notes:
            console.print(f"  • {note}")
    if failed:
        console.print(f"\n[bold red]Failed: {', '.join(failed)}[/bold red]")
        raise typer.Exit(1)
    console.print("\n[bold green]All repos done.[/bold green]")


@app.command()
def clone(
    names: Annotated[
        list[str] | None, typer.Argument(help="Repos to clone (default: all)")
    ] = None,
):
    """Clone repos into ~/gits, skipping ones already there."""
    failed = [r.name for r in select_repos(names or []) if not clone_repo(r, GITS)]
    report(failed, [])


@app.command()
def install(
    names: Annotated[
        list[str] | None, typer.Argument(help="Repos to install (default: all)")
    ] = None,
):
    """Clone repos, then run each one's install steps."""
    failed: list[str] = []
    notes: list[str] = []
    for repo in select_repos(names or []):
        if not clone_repo(repo, GITS) or not install_repo(repo, GITS):
            failed.append(repo.name)
            continue
        if repo.note:
            notes.append(repo.note)
    report(failed, notes)


@app.command("list")
def list_repos():
    """List the repos and whether each is cloned."""
    for repo in REPOS:
        mark = "[green]✓[/green]" if (GITS / repo.name).exists() else "[red]✗[/red]"
        steps = "install steps" if repo.install else "clone only"
        console.print(f"  {mark} {repo.name} [dim]({steps})[/dim]")


if __name__ == "__main__":
    sys.exit(app())
