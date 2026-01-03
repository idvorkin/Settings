#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich"]
# ///
"""
Server discovery and management.

Finds running servers by open ports, maps them to directories and processes.
Works on both Linux and macOS.
"""

import json
import os
import platform
import subprocess
from abc import ABC, abstractmethod
from pathlib import Path

import typer
from rich.console import Console

app = typer.Typer(
    no_args_is_help=True,
    pretty_exceptions_enable=False,
    add_completion=False,
)
console = Console()


# --- Platform Adapter (Humble Object) ---


class PlatformAdapter(ABC):
    """Abstract interface for platform-specific process inspection."""

    @abstractmethod
    def get_listening_ports(self) -> dict[int, int]:
        """Return map of port -> pid for listening TCP ports."""
        ...

    @abstractmethod
    def get_process_cwd(self, pid: int) -> Path | None:
        """Get the current working directory of a process."""
        ...

    @abstractmethod
    def get_process_name(self, pid: int) -> str | None:
        """Get the process name/command."""
        ...

    @abstractmethod
    def get_process_cmdline(self, pid: int) -> str | None:
        """Get the full command line."""
        ...

    @abstractmethod
    def get_parent_pid(self, pid: int) -> int | None:
        """Get the parent process ID."""
        ...

    def get_process_tree(self, pid: int, max_depth: int = 5) -> list[dict]:
        """Get the process tree (ancestors) for a pid."""
        tree = []
        current_pid = pid
        for _ in range(max_depth):
            name = self.get_process_name(current_pid)
            if not name:
                break
            cmdline = self.get_process_cmdline(current_pid)
            tree.append(
                {
                    "pid": current_pid,
                    "name": name,
                    "cmdline": cmdline or name,
                }
            )
            ppid = self.get_parent_pid(current_pid)
            if not ppid or ppid == current_pid or ppid <= 1:
                break
            current_pid = ppid
        return tree


class LinuxAdapter(PlatformAdapter):
    """Linux implementation using /proc and ss."""

    def get_listening_ports(self) -> dict[int, int]:
        ports = {}
        try:
            result = subprocess.run(
                ["ss", "-tlnp"],
                capture_output=True,
                text=True,
            )
            for line in result.stdout.splitlines()[1:]:
                parts = line.split()
                if len(parts) >= 6:
                    addr = parts[3]
                    if ":" in addr:
                        port_str = addr.rsplit(":", 1)[-1]
                        try:
                            port = int(port_str)
                            users = parts[-1] if "pid=" in parts[-1] else ""
                            if "pid=" in users:
                                pid_str = users.split("pid=")[1].split(",")[0]
                                ports[port] = int(pid_str)
                        except ValueError:
                            pass
        except FileNotFoundError:
            pass
        return ports

    def get_process_cwd(self, pid: int) -> Path | None:
        try:
            cwd = os.readlink(f"/proc/{pid}/cwd")
            return Path(cwd)
        except (OSError, FileNotFoundError):
            return None

    def get_process_name(self, pid: int) -> str | None:
        try:
            with open(f"/proc/{pid}/comm", "r") as f:
                return f.read().strip()
        except (OSError, FileNotFoundError):
            return None

    def get_process_cmdline(self, pid: int) -> str | None:
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                # Arguments are null-separated
                cmdline = f.read().decode("utf-8", errors="ignore")
                return cmdline.replace("\x00", " ").strip()
        except (OSError, FileNotFoundError):
            return None

    def get_parent_pid(self, pid: int) -> int | None:
        try:
            with open(f"/proc/{pid}/stat", "r") as f:
                # Format: pid (comm) state ppid ...
                stat = f.read()
                # Find closing paren to skip comm which may contain spaces
                close_paren = stat.rfind(")")
                fields = stat[close_paren + 2 :].split()
                return int(
                    fields[1]
                )  # ppid is 4th field, but after (comm) it's index 1
        except (OSError, FileNotFoundError, IndexError, ValueError):
            return None


class MacOSAdapter(PlatformAdapter):
    """macOS implementation using lsof and ps."""

    def get_listening_ports(self) -> dict[int, int]:
        ports = {}
        try:
            result = subprocess.run(
                ["lsof", "-iTCP", "-sTCP:LISTEN", "-nP", "-F", "pn"],
                capture_output=True,
                text=True,
            )
            current_pid = None
            for line in result.stdout.splitlines():
                if line.startswith("p"):
                    current_pid = int(line[1:])
                elif line.startswith("n") and current_pid:
                    addr = line[1:]
                    if ":" in addr:
                        port_str = addr.rsplit(":", 1)[-1]
                        try:
                            port = int(port_str)
                            ports[port] = current_pid
                        except ValueError:
                            pass
        except FileNotFoundError:
            pass
        return ports

    def get_process_cwd(self, pid: int) -> Path | None:
        try:
            result = subprocess.run(
                ["lsof", "-p", str(pid), "-Fn", "-a", "-d", "cwd"],
                capture_output=True,
                text=True,
            )
            for line in result.stdout.splitlines():
                if line.startswith("n"):
                    return Path(line[1:])
        except FileNotFoundError:
            pass
        return None

    def get_process_name(self, pid: int) -> str | None:
        try:
            result = subprocess.run(
                ["ps", "-p", str(pid), "-o", "comm="],
                capture_output=True,
                text=True,
            )
            return result.stdout.strip() or None
        except FileNotFoundError:
            return None

    def get_process_cmdline(self, pid: int) -> str | None:
        try:
            result = subprocess.run(
                ["ps", "-p", str(pid), "-o", "args="],
                capture_output=True,
                text=True,
            )
            return result.stdout.strip() or None
        except FileNotFoundError:
            return None

    def get_parent_pid(self, pid: int) -> int | None:
        try:
            result = subprocess.run(
                ["ps", "-p", str(pid), "-o", "ppid="],
                capture_output=True,
                text=True,
            )
            return int(result.stdout.strip()) if result.stdout.strip() else None
        except (FileNotFoundError, ValueError):
            return None


def get_adapter() -> PlatformAdapter:
    """Factory: return the appropriate adapter for this platform."""
    system = platform.system()
    if system == "Linux":
        return LinuxAdapter()
    elif system == "Darwin":
        return MacOSAdapter()
    raise RuntimeError(f"Unsupported platform: {system}")


# --- Core Logic (testable with mock adapter) ---


class ServerFinder:
    """Find and inspect servers by port. Accepts adapter for testability."""

    def __init__(self, adapter: PlatformAdapter):
        self.adapter = adapter

    def find_servers(self, exclude_ports: set[int] | None = None) -> list[dict]:
        """Find all listening servers with their ports, directories, and process info."""
        exclude_ports = exclude_ports or set()
        servers = []
        ports = self.adapter.get_listening_ports()

        for port, pid in ports.items():
            if port in exclude_ports:
                continue
            cwd = self.adapter.get_process_cwd(pid)
            name = self.adapter.get_process_name(pid)
            cmdline = self.adapter.get_process_cmdline(pid)
            tree = self.adapter.get_process_tree(pid)
            servers.append(
                {
                    "port": port,
                    "pid": pid,
                    "process": name or "unknown",
                    "cmdline": cmdline or "unknown",
                    "directory": str(cwd) if cwd else "unknown",
                    "tree": tree,
                }
            )

        return sorted(servers, key=lambda s: s["port"])

    def find_for_directory(self, directory: Path) -> list[dict]:
        """Find all servers running in a specific directory."""
        directory = directory.resolve()
        matches = []
        for server in self.find_servers():
            if server["directory"] != "unknown":
                if Path(server["directory"]).resolve() == directory:
                    matches.append(server)
        return matches

    def find_by_port(self, port: int) -> dict | None:
        """Find server on a specific port."""
        for server in self.find_servers():
            if server["port"] == port:
                return server
        return None

    def find_available_port(self, start: int = 4000, end: int = 4010) -> int | None:
        """Find an available port in the given range."""
        ports_in_use = set(self.adapter.get_listening_ports().keys())
        for port in range(start, end + 1):
            if port not in ports_in_use:
                return port
        return None


# --- Utilities ---


def get_tailscale_hostname() -> str | None:
    """Get the Tailscale hostname for this machine."""
    try:
        result = subprocess.run(
            ["tailscale", "status", "--json"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode == 0:
            data = json.loads(result.stdout)
            hostname = data.get("Self", {}).get("DNSName", "")
            return hostname.rstrip(".")
    except (subprocess.TimeoutExpired, FileNotFoundError, json.JSONDecodeError):
        pass
    return None


def get_url(port: int, hostname: str | None = None) -> str:
    """Build URL for a port, using Tailscale hostname if available."""
    host = hostname or "localhost"
    return f"http://{host}:{port}"


# --- CLI Commands ---


def get_finder() -> ServerFinder:
    """Get a finder with the appropriate platform adapter."""
    return ServerFinder(get_adapter())


@app.command()
def status(
    port_range: str = typer.Option(
        None, "--range", "-r", help="Port range to show (e.g., 4000-4010)"
    ),
    full: bool = typer.Option(
        False, "--full", "-f", help="Show full output without truncation"
    ),
    output_json: bool = typer.Option(False, "--json", "-j", help="Output as JSON"),
    process_filter: str = typer.Option(
        None, "--process", "-p", help="Filter by process name (e.g., bundle, node)"
    ),
):
    """Show all running servers."""
    hostname = get_tailscale_hostname()
    finder = get_finder()
    servers = finder.find_servers()

    # Filter by port range if specified
    if port_range:
        try:
            start, end = map(int, port_range.split("-"))
            servers = [s for s in servers if start <= s["port"] <= end]
        except ValueError:
            console.print(f"[red]Invalid range format: {port_range}[/red]")
            return

    # Filter by process name
    if process_filter:
        servers = [
            s
            for s in servers
            if process_filter.lower() in s["process"].lower()
            or process_filter.lower() in s["cmdline"].lower()
        ]

    if not servers:
        if output_json:
            print("[]")
        else:
            console.print("[dim]No servers running.[/dim]")
        return

    # Add URL to each server
    for s in servers:
        s["url"] = get_url(s["port"], hostname)

    if output_json:
        print(json.dumps(servers, indent=2))
    elif full:
        # Plain text output for agents/scripts
        for s in servers:
            console.print(f":{s['port']} (pid {s['pid']})")
            console.print(f"  dir: {s['directory']}")
            console.print(f"  url: {s['url']}")
            console.print("  tree:")
            # Show process tree with vertical lines (reversed = root first)
            tree = list(reversed(s["tree"]))
            for i, p in enumerate(tree):
                if i == 0:
                    console.print(f"    [{p['pid']}] {p['cmdline']}")
                else:
                    indent = "    " + "    " * (i - 1)
                    console.print(f"{indent}└─ [{p['pid']}] {p['cmdline']}")
            console.print()
    else:
        # Compact output
        for s in servers:
            console.print(
                f"[cyan]:{s['port']}[/cyan] [yellow]{s['process']}[/yellow] [dim]→[/dim] [blue]{s['url']}[/blue]"
            )
        console.print()
        console.print("[dim]Run with --full or --json for more details[/dim]")


@app.command()
def check(
    directory: Path = typer.Argument(None, help="Directory to check (default: cwd)"),
):
    """Check what servers are running for a directory."""
    directory = directory or Path.cwd()
    hostname = get_tailscale_hostname()
    finder = get_finder()
    servers = finder.find_for_directory(directory)

    if servers:
        console.print(f"[green]✓[/green] Servers running for [cyan]{directory}[/cyan]")
        for s in servers:
            url = get_url(s["port"], hostname)
            console.print(
                f"  :{s['port']} [yellow]{s['process']}[/yellow] → [blue]{url}[/blue]"
            )
    else:
        console.print(f"[yellow]✗[/yellow] No servers for [cyan]{directory}[/cyan]")
        available = finder.find_available_port()
        if available:
            console.print(f"  Available port: [cyan]{available}[/cyan]")


@app.command()
def port(port: int = typer.Argument(..., help="Port to check")):
    """Check what's running on a specific port."""
    hostname = get_tailscale_hostname()
    finder = get_finder()
    server = finder.find_by_port(port)

    if server:
        url = get_url(port, hostname)
        console.print(
            f"[green]✓[/green] Port [cyan]{port}[/cyan] in use (pid {server['pid']})"
        )
        console.print(f"  cmd: {server['cmdline']}")
        console.print(f"  dir: [green]{server['directory']}[/green]")
        console.print(f"  url: [blue]{url}[/blue]")
    else:
        console.print(f"[dim]Port {port} is available[/dim]")


@app.command()
def suggest(
    start: int = typer.Option(4000, "--start", "-s", help="Start of port range"),
    end: int = typer.Option(4010, "--end", "-e", help="End of port range"),
):
    """Find an available port in a range."""
    hostname = get_tailscale_hostname()
    finder = get_finder()
    available = finder.find_available_port(start, end)

    if available:
        url = get_url(available, hostname)
        console.print(f"[bold]{available}[/bold]")
        console.print(f"[dim]URL: {url}[/dim]")
    else:
        console.print(f"[red]No available ports in range {start}-{end}[/red]")


if __name__ == "__main__":
    app()
