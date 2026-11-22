#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "pydantic"]
# ///
"""
Clone Igor's active repos into ~/gits, update them, and run their install steps.

Cloning uses `gh repo clone`, so private repos need `gh auth login` first.
An existing checkout is updated with `git pull --ff-only`; a dirty or diverged
one is left as it is and reported. Install steps are shell commands run inside
the checkout; they run after a clone, when the pull moved HEAD, or when the
repo's marker (what the install leaves behind) is missing.
"""

import json
import subprocess
import sys
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Annotated, Literal

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


Kind = Literal["os", "mac", "ios"]
KIND_TITLES: dict[str, str] = {
    "os": "Git Repos: OS",
    "mac": "Git Repos: OSX Apps",
    "ios": "Git Repos: iOS Apps",
}


class Repo(BaseModel):
    name: str
    kind: Kind = Field("os", description="os (tools), mac (Mac app) or ios (iOS app)")
    about: str = Field("", description="One line: what it is")
    owner: str = Field(GH_OWNER, description="GitHub account that holds the repo")
    install: list[str] = Field(
        default_factory=list, description="Shell commands run in the checkout"
    )
    marker: str = Field(
        "", description="Path (~ ok) that exists once installed; empty: the checkout"
    )
    note: str = Field("", description="Manual follow-up printed after install")

    def installed(self, gits: Path) -> bool:
        if not self.marker:
            return (gits / self.name).exists()
        # exists() follows symlinks: a live-install link to a deleted build is missing
        return Path(self.marker).expanduser().exists()


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
# iOS/watch apps are clone and update only: a device build needs the signing lock and an unlocked phone (just iphone).
REPOS: list[Repo] = [
    # OS: what the Mac runs on
    Repo(name="alfred", about="Alfred workflows (Igor Tools), including y", install=[ALFRED_LINK_PREFS],
         marker="~/Library/Application Support/Alfred/Alfred.alfredpreferences",
         note="Alfred: confirm Igor Tools under Workflows and activate the Powerpack"),
    # yabai is built from the fork by alfred's recipe, so alfred must come first
    Repo(name="yabai", about="tiling window manager (fork, with macOS 27 fixes)",
         install=["just -f ../alfred/justfile -d ../alfred yabai-install"],
         marker="~/.local/share/yabai-macos27", note="yabai: re-grant Accessibility if macOS prompts"),
    Repo(name="browser-tab-helper", about="Edge extension and native host: tabs from the command line",
         install=["node tools/host.js install"],
         marker="~/Library/Application Support/Microsoft Edge/NativeMessagingHosts/browser_tab_helper.json",
         note="browser-tab-helper: edge://extensions → Developer mode → Load unpacked → ~/gits/browser-tab-helper"),
    # The larry CLI that Alfred's Agent Send (`w`) shells out to
    Repo(name="cockpit", owner="idvorkin-ai-tools", about="larry CLI: send to an agent from Alfred",
         install=["mkdir -p ~/.local/bin && ln -sfn \"$PWD/larry.py\" ~/.local/bin/larry"],
         marker="~/.local/bin/larry",
         note="larry: put {\"url\": ..., \"token\": <COCKPIT_CONTROL_TOKEN>} in ~/.config/larry/config.json (chmod 600)"),
    Repo(name="bug-kit", about="shared Swift package: in-app bug reports and what's new, for the native apps"),
    # OSX apps: install to /Applications (window-sweaters to ~/Applications)
    # Alfred's `sw` keyword drives the fork's raccoon, which the brew cask lacks
    Repo(name="window-sweaters", kind="mac", about="knitted window borders, plus Focused Window Only (fork)",
         install=["./scripts/build-app.sh", "python3 scripts/install-local.py"],
         marker="~/Applications/Window Sweaters.app"),
    Repo(name="igsnap", kind="mac", about="screenshot tool", install=["just install"],
         marker="/Applications/IgSnap.app"),
    Repo(name="magic-monitor-native", kind="mac", about="camera practice mirror with instant replay",
         install=["just install"], marker="/Applications/Magic Monitor.app"),
    Repo(name="lcd-timer", kind="mac", about="countdown that looks like a gym's LED clock",
         install=["just install"], marker="/Applications/LCD Timer.app"),
    Repo(name="audio-debugger", kind="mac", about="see and hear how a mic sounds, with 20 s replay (also iPhone)",
         install=["just install"], marker="/Applications/Audio Debugger.app"),
    # iOS apps
    Repo(name="context-grabber", kind="ios", about="a humane mirror of the week: health, places, gym timer",
         install=["just setup"]),
    Repo(name="exercise-analyzer", kind="ios", about="iPhone and Watch exercise analyzer"),
    Repo(name="think-a-card-trainer", kind="ios", about="iPhone and Watch card-memory trainer"),
]
# fmt: on


class Runner(ABC):
    """Runs commands, so tests can swap in a fake."""

    @abstractmethod
    def run(self, cmd: list[str], cwd: Path | None = None) -> bool: ...

    @abstractmethod
    def output(self, cmd: list[str], cwd: Path | None = None) -> str | None:
        """The command's stdout, stripped, or None when it fails."""


class ShellRunner(Runner):
    def run(self, cmd: list[str], cwd: Path | None = None) -> bool:
        return subprocess.run(cmd, check=False, cwd=cwd).returncode == 0

    def output(self, cmd: list[str], cwd: Path | None = None) -> str | None:
        r = subprocess.run(cmd, check=False, cwd=cwd, capture_output=True, text=True)
        return r.stdout.strip() if r.returncode == 0 else None


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


def update_repo(repo: Repo, gits: Path) -> bool | None:
    """Fast-forward the checkout. True: HEAD moved; False: no change; None: not updated."""
    cwd = gits / repo.name
    before = runner.output(["git", "rev-parse", "HEAD"], cwd)
    if not runner.run(["git", "pull", "--ff-only", "--no-autostash", "--quiet"], cwd):
        console.print(
            f"  [yellow]! {repo.name} not updated (dirty, diverged or offline)[/yellow]"
        )
        return None
    after = runner.output(["git", "rev-parse", "HEAD"], cwd)
    return before != after


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
    force: Annotated[
        bool, typer.Option("--force", "-f", help="Run install steps even if up to date")
    ] = False,
):
    """Clone or update repos, then run install steps where needed."""
    failed: list[str] = []
    notes: list[str] = []
    for repo in select_repos(names or []):
        fresh = not (GITS / repo.name).exists()
        if not clone_repo(repo, GITS):
            failed.append(repo.name)
            continue
        moved = True if fresh else update_repo(repo, GITS)
        if not (moved or force or not repo.installed(GITS)):
            if moved is False:
                console.print(f"  [dim]✓ {repo.name} up to date[/dim]")
            continue
        if not install_repo(repo, GITS):
            failed.append(repo.name)
            continue
        if repo.note:
            notes.append(repo.note)
    report(failed, notes)


@app.command("list")
def list_repos(
    as_json: Annotated[bool, typer.Option("--json", help="Machine-readable")] = False,
):
    """List the repos by kind and whether each is installed."""
    if as_json:
        rows = [
            {"name": r.name, "kind": r.kind, "title": KIND_TITLES[r.kind],
             "about": r.about, "installed": r.installed(GITS)}
            for r in REPOS
        ]  # fmt: skip
        print(json.dumps(rows))
        return
    for kind, title in KIND_TITLES.items():
        console.print(f"[bold]{title}[/bold]")
        for repo in (r for r in REPOS if r.kind == kind):
            mark = "[green]✓[/green]" if repo.installed(GITS) else "[red]✗[/red]"
            console.print(f"  {mark} {repo.name} [dim]— {repo.about}[/dim]")


if __name__ == "__main__":
    sys.exit(app())
