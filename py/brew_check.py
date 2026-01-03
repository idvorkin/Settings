#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "questionary"]
# ///
"""
Brew package checker and installer.

Checks which packages are missing and optionally installs them.

Bootstrap (before Python exists):
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  brew install git tmux zsh neovim python3 uv
"""

import platform
import subprocess

import questionary
import typer
from rich.console import Console
from rich.table import Table

app = typer.Typer(
    no_args_is_help=True,
    pretty_exceptions_enable=False,
    add_completion=False,
)
console = Console()

IS_MACOS = platform.system() == "Darwin"

# Taps required before installing certain packages (Mac only)
MAC_TAPS = [
    "FelixKratz/formulae",  # borders
    "koekeishiya/formulae",  # yabai
    "homebrew/cask-fonts",  # fonts
]

# Package categories - each is (category_name, package_list)
# fmt: off
FORMULA_CATEGORIES: list[tuple[str, list[str]]] = [
    ("Essential", "git tmux zsh neovim python3 uv".split()),
    ("Modern CLI", "zoxide bat eza procs htop starship yazi television bottom rg fastfetch".split()),
    ("Rust", ["rust"]),
    ("File Search", "fd fzf ag".split()),
    ("Git Tools", "lazygit gh git-extras git-delta diff-so-fancy difftastic tig pre-commit".split()),
    ("Data Tools", "jq yq dasel fx pup".split()),
    ("Network", "curlie httpie xh wget w3m nmap autossh mosh".split()),
    ("Containers", "docker docker-compose lazydocker".split()),
    ("Media", "ffmpeg yt-dlp imagemagick optipng png-quant mpv".split()),
    ("Docs", "pandoc glow grip".split()),
    ("LSPs", "lua-language-server typos-lsp pyright rust-analyzer universal-ctags".split()),
    ("System", "just cpulimit gdu ncdu duf openssl zip npm carapace atuin".split()),
    ("Image Viewers", "viu timg".split()),
    ("AI Tools", "llm gemini-cli claude-code".split()),
    ("Cloud/DB", "s3cmd pgcli fselect cloc 1password-cli gcloud-cli".split()),
]

MAC_FORMULA_CATEGORIES: list[tuple[str, list[str]]] = [
    ("Mac Window Mgmt", "borders jordanbaird-ice koekeishiya/formulae/yabai".split()),
    ("Mac Media", "iina pngpaste".split()),
]

CASK_CATEGORIES: list[tuple[str, list[str]]] = [
    ("Browsers", "google-chrome microsoft-edge".split()),
    ("Terminal", "iterm2 ghostty".split()),
    ("Productivity", "alfred 1password meetingbar finicky".split()),
    ("Window Mgmt", "alt-tab karabiner-elements".split()),
    ("Video", "losslesscut capcut qlvideo".split()),
    ("Communication", "signal tailscale".split()),
    ("Display", ["betterdisplay"]),
    ("Utilities", "the-unarchiver keycastr kindavim".split()),
    ("Fonts", ["font-hack-nerd-font"]),
]
# fmt: on


def get_all_formulae() -> list[str]:
    """Get all formulae as flat list."""
    result = []
    for _, pkgs in FORMULA_CATEGORIES:
        result.extend(pkgs)
    if IS_MACOS:
        for _, pkgs in MAC_FORMULA_CATEGORIES:
            result.extend(pkgs)
    return result


def get_all_casks() -> list[str]:
    """Get all casks as flat list."""
    if not IS_MACOS:
        return []
    result = []
    for _, pkgs in CASK_CATEGORIES:
        result.extend(pkgs)
    return result


def get_installed_packages() -> tuple[set[str], set[str]]:
    """Get currently installed brew formulae and casks."""
    formulae: set[str] = set()
    casks: set[str] = set()

    try:
        result = subprocess.run(
            ["brew", "list", "--formula", "-1"],
            capture_output=True,
            text=True,
            check=True,
        )
        formulae = set(result.stdout.strip().splitlines())
    except (subprocess.CalledProcessError, FileNotFoundError):
        console.print("[yellow]Warning: Could not get installed formulae[/yellow]")

    if IS_MACOS:
        try:
            result = subprocess.run(
                ["brew", "list", "--cask", "-1"],
                capture_output=True,
                text=True,
                check=True,
            )
            casks = set(result.stdout.strip().splitlines())
        except (subprocess.CalledProcessError, FileNotFoundError):
            console.print("[yellow]Warning: Could not get installed casks[/yellow]")

    return formulae, casks


def normalize_name(name: str) -> str:
    """Normalize package name (handles tap/pkg format)."""
    return name.split("/")[-1] if "/" in name else name


def get_missing_by_category(
    installed_formulae_norm: set[str], installed_casks_norm: set[str]
) -> tuple[list[tuple[str, list[str]]], list[tuple[str, list[str]]]]:
    """Get missing packages organized by category."""
    missing_formula_cats = []
    for cat, pkgs in FORMULA_CATEGORIES:
        missing = [p for p in pkgs if normalize_name(p) not in installed_formulae_norm]
        if missing:
            missing_formula_cats.append((cat, missing))

    if IS_MACOS:
        for cat, pkgs in MAC_FORMULA_CATEGORIES:
            missing = [
                p for p in pkgs if normalize_name(p) not in installed_formulae_norm
            ]
            if missing:
                missing_formula_cats.append((cat, missing))

    missing_cask_cats = []
    if IS_MACOS:
        for cat, pkgs in CASK_CATEGORIES:
            missing = [p for p in pkgs if normalize_name(p) not in installed_casks_norm]
            if missing:
                missing_cask_cats.append((cat, missing))

    return missing_formula_cats, missing_cask_cats


@app.command()
def check(
    install: bool = typer.Option(
        False, "--install", "-i", help="Install all missing packages"
    ),
    pick: bool = typer.Option(
        False, "--pick", "-p", help="Pick which packages to install"
    ),
):
    """Check which brew packages are missing."""
    if not IS_MACOS:
        console.print("[dim]Note: Skipping casks and mac formulae (macOS only)[/dim]\n")

    installed_formulae, installed_casks = get_installed_packages()
    installed_formulae_norm = {normalize_name(p) for p in installed_formulae}
    installed_casks_norm = {normalize_name(p) for p in installed_casks}

    missing_formula_cats, missing_cask_cats = get_missing_by_category(
        installed_formulae_norm, installed_casks_norm
    )

    if not missing_formula_cats and not missing_cask_cats:
        console.print("[green]All packages are installed![/green]")
        return

    # Display missing by category
    table = Table(title="Missing Brew Packages")
    table.add_column("Category", style="cyan")
    table.add_column("Type", style="dim")
    table.add_column("Packages", style="yellow")

    total_formulae = 0
    total_casks = 0
    for cat, pkgs in missing_formula_cats:
        table.add_row(cat, "formula", " ".join(pkgs))
        total_formulae += len(pkgs)
    for cat, pkgs in missing_cask_cats:
        table.add_row(cat, "cask", " ".join(pkgs))
        total_casks += len(pkgs)

    console.print(table)
    console.print(
        f"\n[bold]Missing:[/bold] {total_formulae} formulae, {total_casks} casks"
    )

    # Handle installation
    if install:
        all_formulae = [p for _, pkgs in missing_formula_cats for p in pkgs]
        all_casks = [p for _, pkgs in missing_cask_cats for p in pkgs]
        install_packages(all_formulae, all_casks)
    elif pick:
        pick_and_install(missing_formula_cats, missing_cask_cats)
    elif typer.confirm("\nInstall packages?"):
        pick_and_install(missing_formula_cats, missing_cask_cats)


def pick_and_install(
    formula_cats: list[tuple[str, list[str]]], cask_cats: list[tuple[str, list[str]]]
):
    """Interactive picker to select packages to install."""
    choices = []

    # Build choices with category headers
    for cat, pkgs in formula_cats:
        choices.append(questionary.Separator(f"── {cat} (formulae) ──"))
        for pkg in pkgs:
            choices.append(
                questionary.Choice(f"  {pkg}", value=("formula", pkg), checked=True)
            )

    for cat, pkgs in cask_cats:
        choices.append(questionary.Separator(f"── {cat} (casks) ──"))
        for pkg in pkgs:
            choices.append(
                questionary.Choice(f"  {pkg}", value=("cask", pkg), checked=True)
            )

    if not choices:
        return

    selected = questionary.checkbox(
        "Select packages to install (space to toggle, enter to confirm):",
        choices=choices,
    ).ask()

    if not selected:
        console.print("[yellow]No packages selected[/yellow]")
        return

    formulae = [pkg for kind, pkg in selected if kind == "formula"]
    casks = [pkg for kind, pkg in selected if kind == "cask"]

    install_packages(formulae, casks)


def ensure_taps():
    """Ensure all required taps are added."""
    if not IS_MACOS:
        return
    console.print("[bold]Checking taps...[/bold]")
    for tap in MAC_TAPS:
        result = subprocess.run(["brew", "tap", tap], capture_output=True, text=True)
        if result.returncode == 0:
            console.print(f"  [green]✓[/green] {tap}")
        else:
            console.print(f"  [red]✗[/red] {tap}")


def install_packages(formulae: list[str], casks: list[str]):
    """Install the given packages."""
    ensure_taps()

    if formulae:
        console.print(f"\n[bold]Installing {len(formulae)} formulae...[/bold]")
        for pkg in formulae:
            console.print(f"  [cyan]{pkg}[/cyan]...")
            result = subprocess.run(["brew", "install", pkg], capture_output=True)
            if result.returncode != 0:
                console.print("    [red]Failed[/red]")

    if casks:
        console.print(f"\n[bold]Installing {len(casks)} casks...[/bold]")
        for pkg in casks:
            console.print(f"  [cyan]{pkg}[/cyan]...")
            result = subprocess.run(
                ["brew", "install", "--cask", pkg], capture_output=True
            )
            if result.returncode != 0:
                console.print("    [red]Failed[/red]")

    console.print("\n[green]Done![/green]")


@app.command()
def bootstrap():
    """Print bootstrap commands (run before Python exists)."""
    essentials = " ".join(dict(FORMULA_CATEGORIES)["Essential"])
    console.print("[bold]Run these commands to bootstrap:[/bold]\n")
    console.print("[cyan]# 1. Install Homebrew[/cyan]")
    console.print(
        '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'
    )
    if IS_MACOS:
        taps = " && ".join(f"brew tap {t}" for t in MAC_TAPS)
        console.print("\n[cyan]# 2. Add taps (macOS)[/cyan]")
        console.print(taps)
        console.print("\n[cyan]# 3. Install essentials[/cyan]")
    else:
        console.print("\n[cyan]# 2. Install essentials[/cyan]")
    console.print(f"brew install {essentials}")
    console.print(f"\n[cyan]# {'4' if IS_MACOS else '3'}. Then run this script[/cyan]")
    console.print("./brew_check.py check")


@app.command()
def list_all():
    """List all defined packages by category."""
    table = Table(title="All Brew Packages")
    table.add_column("Category", style="cyan")
    table.add_column("Type", style="dim")
    table.add_column("Packages", style="yellow")

    total = 0
    for cat, pkgs in FORMULA_CATEGORIES:
        table.add_row(cat, "formula", " ".join(pkgs))
        total += len(pkgs)

    if IS_MACOS:
        for cat, pkgs in MAC_FORMULA_CATEGORIES:
            table.add_row(cat, "formula", " ".join(pkgs))
            total += len(pkgs)
        for cat, pkgs in CASK_CATEGORIES:
            table.add_row(cat, "cask", " ".join(pkgs))
            total += len(pkgs)

    console.print(table)
    console.print(f"\n[bold]Total:[/bold] {total} packages")


if __name__ == "__main__":
    app()
