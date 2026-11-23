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

import json
import platform
import subprocess
from pathlib import Path

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
    "asmvik/formulae",  # yabai
    "idvorkin/tap",  # Igor's tools
]

# Package categories - each is (category_name, package_list)
# fmt: off
FORMULA_CATEGORIES: list[tuple[str, list[str]]] = [
    ("Essential", "git tmux zsh neovim python3 uv".split()),
    ("Modern CLI", "zoxide bat eza procs starship yazi bottom ripgrep".split()),
    ("Languages", "rust ruby".split()),
    ("File Search", "fd fzf".split()),
    ("Git Tools", "lazygit gh git-extras git-delta difftastic tig pre-commit prek".split()),
    ("Data Tools", "jq yq dasel".split()),
    ("Network", "mosh et".split()),
    ("Media", "ffmpeg yt-dlp imagemagick timg".split()),
    ("Docs", "pandoc grip markdownlint-cli".split()),
    ("LSPs", "lua-language-server typos-lsp pyright pyrefly marksman rust-analyzer universal-ctags".split()),
    ("System", "just gdu duf openssl npm carapace atuin".split()),
    ("AI Tools", "gemini-cli herdr beads".split()),
    ("Cloud/DB", "1password-cli gcloud-cli awscli rclone".split()),
]

MAC_FORMULA_CATEGORIES: list[tuple[str, list[str]]] = [
    ("Mac Window Mgmt", "jordanbaird-ice asmvik/formulae/yabai".split()),
    ("Igor's Tools", "idvorkin/tap/no-mistakes idvorkin/tap/lavish-axi idvorkin/tap/treehouse".split()),
    ("Mac Media", ["pngpaste"]),
    ("Mac Display", ["betterdisplaycli"]),  # lg-watchdog.sh and the lg-* helpers
    ("Mac iOS Dev", "xcodegen swiftlint xcbeautify libimobiledevice mas xcodes aria2 watchman".split()),
]

CASK_CATEGORIES: list[tuple[str, list[str]]] = [
    ("Browsers", "google-chrome microsoft-edge".split()),
    ("Terminal", "iterm2 ghostty".split()),
    ("Productivity", "alfred 1password meetingbar finicky wispr-flow".split()),
    ("Window Mgmt", "alt-tab karabiner-elements".split()),
    ("Dev", "codex orbstack".split()),  # Claude Code: native installer (CLAUDE_CODE_INSTALL)
    ("Video", "losslesscut capcut cap".split()),  # Cap: screen recorder, screen + camera as separate files
    ("Communication", "signal tailscale-app".split()),
    ("Display", ["betterdisplay"]),
    ("Utilities", "the-unarchiver keycastr kindavim calibre".split()),
    ("Fonts", ["font-hack-nerd-font"]),
]

# Mac App Store apps - (name, App Store id). Find an id with:
#   mdls -raw -name kMDItemAppStoreAdamID /Applications/<App>.app
MAS_APPS: list[tuple[str, int]] = [
    ("Xcode", 497799835),
    ("OmniFocus", 1542143627),
    ("Goodnotes", 1444383602),
    ("WhatsApp", 310633997),
    ("Telegram Lite", 946399090),
    ("TestFlight", 899247664),
    ("CapCut", 1500855883),
    ("Flow", 1423210932),
]

# Global npm packages
NPM_GLOBALS: list[str] = [
    "@xai-official/grok",
]

# Claude Code uses its native installer (self-updating), not the brew formula.
CLAUDE_CODE_INSTALL = "curl -fsSL https://claude.ai/install.sh | bash"
# fmt: on

# Repos with no brew package (Mac only) come from install_repos.py, the one list of them:
# it clones, updates and installs them. They are picked here after the brew packages.
INSTALL_REPOS = Path(__file__).resolve().parent / "install_repos.py"


def get_git_repos() -> list[dict]:
    """install_repos.py's repos: name, kind, title, about, installed (macOS only)."""
    if not IS_MACOS:
        return []
    result = subprocess.run(
        [str(INSTALL_REPOS), "list", "--json"], capture_output=True, text=True
    )
    if result.returncode != 0:
        console.print("[yellow]Warning: could not list git repos[/yellow]")
        return []
    return json.loads(result.stdout)


def by_title(repos: list[dict]) -> list[tuple[str, list[str]]]:
    """Repo names grouped by category title, in order."""
    cats: dict[str, list[str]] = {}
    for r in repos:
        cats.setdefault(r["title"], []).append(r["name"])
    return list(cats.items())


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


def is_formula_installed(name: str, installed_norm: set[str]) -> bool:
    """Check if a formula is installed, handling aliases and versioned kegs.

    First checks the fast set lookup; falls back to `brew list --formula <name>`
    which resolves aliases (python3 → python@3.13), versioned kegs
    (openssl → openssl@3), and provided-by relationships (npm → node).
    """
    if normalize_name(name) in installed_norm:
        return True
    # Second-chance: let brew resolve the name
    try:
        result = subprocess.run(
            ["brew", "list", "--formula", name],
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        return False
    return result.returncode == 0


def get_missing_by_category(
    installed_formulae_norm: set[str], installed_casks_norm: set[str]
) -> tuple[list[tuple[str, list[str]]], list[tuple[str, list[str]]]]:
    """Get missing packages organized by category."""
    missing_formula_cats = []
    for cat, pkgs in FORMULA_CATEGORIES:
        missing = [
            p for p in pkgs if not is_formula_installed(p, installed_formulae_norm)
        ]
        if missing:
            missing_formula_cats.append((cat, missing))

    if IS_MACOS:
        for cat, pkgs in MAC_FORMULA_CATEGORIES:
            missing = [
                p for p in pkgs if not is_formula_installed(p, installed_formulae_norm)
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
    verbose: bool = typer.Option(
        False, "--verbose", "-v", help="Show output on install failures"
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
    repos = get_git_repos()
    missing_build_cats = by_title([r for r in repos if not r["installed"]])

    if not missing_formula_cats and not missing_cask_cats and not missing_build_cats:
        console.print("[green]All packages and git repos are installed![/green]")
        if install:  # still update the repos
            install_packages([], [], verbose, [r["name"] for r in repos])
        elif pick:
            pick_and_install([], [], verbose, repos)
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
    total_builds = 0
    for cat, repos in missing_build_cats:
        table.add_row(cat, "git repo", " ".join(repos))
        total_builds += len(repos)

    console.print(table)
    console.print(
        f"\n[bold]Missing:[/bold] {total_formulae} formulae, {total_casks} casks,"
        f" {total_builds} git repos"
    )

    # Handle installation
    if install:
        all_formulae = [p for _, pkgs in missing_formula_cats for p in pkgs]
        all_casks = [p for _, pkgs in missing_cask_cats for p in pkgs]
        # Every repo: missing ones are cloned and installed, the rest updated
        all_repos = [r["name"] for r in repos]
        install_packages(all_formulae, all_casks, verbose, all_repos)
    elif pick or typer.confirm("\nInstall packages?"):
        pick_and_install(missing_formula_cats, missing_cask_cats, verbose, repos)


def pick_and_install(
    formula_cats: list[tuple[str, list[str]]],
    cask_cats: list[tuple[str, list[str]]],
    verbose: bool = False,
    repos: list[dict] | None = None,
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

    # Every repo, missing ones checked; an installed one picked is updated
    for cat, names in by_title(repos or []):
        choices.append(questionary.Separator(f"── {cat} ──"))
        for r in (r for r in repos or [] if r["name"] in names):
            state = "update" if r["installed"] else "install"
            label = f"  {r['name']} [{state}] — {r['about']}"
            checked = not r["installed"]
            choices.append(
                questionary.Choice(label, value=("build", r["name"]), checked=checked)
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
    builds = [pkg for kind, pkg in selected if kind == "build"]

    install_packages(formulae, casks, verbose, builds)


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


def install_packages(
    formulae: list[str],
    casks: list[str],
    verbose: bool = False,
    builds: list[str] | None = None,
):
    """Install the given packages, then clone and build the given repos."""
    ensure_taps()
    had_failures = False

    if formulae:
        console.print(f"\n[bold]Installing {len(formulae)} formulae...[/bold]")
        for pkg in formulae:
            console.print(f"  [cyan]{pkg}[/cyan]...")
            result = subprocess.run(
                ["brew", "install", pkg], capture_output=True, text=True
            )
            if result.returncode != 0:
                had_failures = True
                console.print("    [red]Failed[/red]")
                if verbose:
                    console.print(f"    [dim]{result.stderr.strip()}[/dim]")

    if casks:
        console.print(f"\n[bold]Installing {len(casks)} casks...[/bold]")
        for pkg in casks:
            console.print(f"  [cyan]{pkg}[/cyan]...")
            result = subprocess.run(
                ["brew", "install", "--cask", pkg], capture_output=True, text=True
            )
            if result.returncode != 0:
                had_failures = True
                console.print("    [red]Failed[/red]")
                if verbose:
                    console.print(f"    [dim]{result.stderr.strip()}[/dim]")

    if builds:
        # After the formulae: the builds need just (and Swift from the Command Line Tools).
        console.print(f"\n[bold]Git repos: {len(builds)} to clone or update...[/bold]")
        # Output streams: a build takes minutes and may ask for the keychain.
        result = subprocess.run([str(INSTALL_REPOS), "install", *builds])
        if result.returncode != 0:
            had_failures = True
            console.print("    [red]Some builds failed (output above)[/red]")

    console.print("\n[green]Done![/green]")
    if had_failures and not verbose:
        console.print("[dim]Some packages failed. Use -v to see error output.[/dim]")


def parse_mas_list(output: str) -> set[int]:
    """App Store ids from `mas list` output (lines start with the id)."""
    ids = set()
    for line in output.splitlines():
        first = line.split(maxsplit=1)[0] if line.strip() else ""
        if first.isdigit():
            ids.add(int(first))
    return ids


def missing_mas_apps(installed_ids: set[int]) -> list[tuple[str, int]]:
    return [(name, app_id) for name, app_id in MAS_APPS if app_id not in installed_ids]


@app.command()
def mas(
    install: bool = typer.Option(
        False, "--install", "-i", help="Install missing App Store apps"
    ),
):
    """Check (and optionally install) Mac App Store apps via `mas`."""
    if not IS_MACOS:
        console.print("[dim]Mac App Store apps are macOS only[/dim]")
        return
    try:
        result = subprocess.run(
            ["mas", "list"], capture_output=True, text=True, check=True
        )
    except FileNotFoundError:
        console.print("[red]mas not installed:[/red] brew install mas")
        raise typer.Exit(1)
    except subprocess.CalledProcessError as e:
        console.print(f"[red]mas list failed:[/red] {e.stderr.strip()}")
        raise typer.Exit(1)

    missing = missing_mas_apps(parse_mas_list(result.stdout))
    if not missing:
        console.print("[green]All App Store apps are installed![/green]")
        return

    for name, app_id in missing:
        console.print(f"  [yellow]missing[/yellow] {name} ({app_id})")
    if not install:
        console.print("\nRun with --install (sign in to the App Store first).")
        return

    failed = []
    for name, app_id in missing:
        console.print(f"  [cyan]{name}[/cyan]...")
        if subprocess.run(["mas", "install", str(app_id)]).returncode != 0:
            failed.append(name)
    if failed:
        console.print(f"[red]Failed:[/red] {', '.join(failed)}")
        raise typer.Exit(1)
    console.print("[green]Done![/green]")


def parse_npm_globals(ls_json: str) -> set[str]:
    """Package names from `npm ls -g --depth=0 --json`."""
    return set(json.loads(ls_json or "{}").get("dependencies", {}))


@app.command()
def npm(
    install: bool = typer.Option(
        False, "--install", "-i", help="Install missing global npm packages"
    ),
):
    """Check (and optionally install) global npm packages."""
    try:
        result = subprocess.run(
            ["npm", "ls", "-g", "--depth=0", "--json"], capture_output=True, text=True
        )
    except FileNotFoundError:
        console.print("[red]npm not installed:[/red] brew install npm")
        raise typer.Exit(1)

    missing = [p for p in NPM_GLOBALS if p not in parse_npm_globals(result.stdout)]
    if not missing:
        console.print("[green]All global npm packages are installed![/green]")
        return

    console.print(f"  [yellow]missing[/yellow] {' '.join(missing)}")
    if not install:
        console.print("\nRun with --install.")
        return
    if subprocess.run(["npm", "install", "-g", *missing]).returncode != 0:
        raise typer.Exit(1)
    console.print("[green]Done![/green]")


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
    console.print(
        f"\n[cyan]# {'4' if IS_MACOS else '3'}. Claude Code (native installer)[/cyan]"
    )
    console.print(CLAUDE_CODE_INSTALL)
    console.print(f"\n[cyan]# {'5' if IS_MACOS else '4'}. Then run this script[/cyan]")
    console.print(
        "./brew_check.py check && ./brew_check.py npm -i"
        + (" && ./brew_check.py mas -i" if IS_MACOS else "")
    )


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
        for cat, names in by_title(get_git_repos()):
            table.add_row(cat, "git repo", " ".join(names))
            total += len(names)

    console.print(table)
    console.print(f"\n[bold]Total:[/bold] {total} packages")


if __name__ == "__main__":
    app()
