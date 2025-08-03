#!/usr/bin/env uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "pydantic"]
# ///

import subprocess
from pathlib import Path

import typer
from pydantic import BaseModel, Field
from rich.console import Console
from rich.prompt import Confirm

app = typer.Typer(
    no_args_is_help=True, pretty_exceptions_enable=False, add_completion=False
)
console = Console()


class MCPServer(BaseModel):
    name: str = Field(..., description="Server name")
    display_name: str = Field(..., description="Display name for console output")
    description: str = Field(..., description="Server description")
    transport: str = Field("stdio", description="Transport type (stdio or http)")
    command: list[str] = Field(..., description="Command arguments for claude mcp add")
    requires_settings_dir: bool = Field(
        False, description="Whether command needs settings directory"
    )


# Define all MCP servers
MCP_SERVERS = [
    MCPServer(
        name="playwright",
        display_name="Playwright MCP Server",
        description="npx-based for browser automation",
        transport="stdio",
        command=["playwright", "npx", "@playwright/mcp@latest"],
    ),
    MCPServer(
        name="github",
        display_name="GitHub MCP Server",
        description="Docker-based via stdio",
        transport="stdio",
        command=["github", "{settings_dir}/mcp_servers/github-mcp-server.sh"],
        requires_settings_dir=True,
    ),
    MCPServer(
        name="context7",
        display_name="context7 MCP Server",
        description="HTTP-based for documentation",
        transport="http",
        command=["--transport", "http", "context7", "https://mcp.context7.com/mcp"],
    ),
]


def run_command(cmd: list[str]) -> tuple[bool, str]:
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        return True, result.stdout
    except subprocess.CalledProcessError as e:
        return False, e.stderr


def remove_mcp_server(server_name: str) -> bool:
    """Remove an MCP server from Claude."""
    success, output = run_command(["claude", "mcp", "remove", server_name])
    return success


def install_mcp_server(
    server: MCPServer, settings_dir: Path, force_reinstall: bool = False
) -> bool:
    console.print(f"📦 Installing {server.display_name}...")

    # Check if already installed
    if force_reinstall:
        success, output = run_command(["claude", "mcp", "list"])
        if success and server.name in output:
            console.print(f"   Removing existing {server.name} server...")
            if not remove_mcp_server(server.name):
                console.print("   [yellow]⚠️  Could not remove existing server[/yellow]")

    # Build command
    cmd = ["claude", "mcp", "add"]

    # Process command arguments
    processed_cmd = []
    for arg in server.command:
        if server.requires_settings_dir and "{settings_dir}" in arg:
            arg = arg.replace("{settings_dir}", str(settings_dir))
            # Check if file exists for path-based commands
            if arg.endswith(".sh") and not Path(arg).exists():
                console.print(f"[red]❌ Script not found at {arg}[/red]")
                return False
        processed_cmd.append(arg)

    cmd.extend(processed_cmd)

    console.print(f"   Adding {server.name} MCP server to Claude...")
    success, output = run_command(cmd)

    if success:
        console.print(f"   [green]✅ {server.display_name} ready[/green]")
    elif "already exists" in output:
        console.print(f"   [yellow]⚠️  {server.display_name} already installed[/yellow]")
        return True  # Consider already installed as success
    else:
        console.print(f"[red]❌ Failed to add {server.name} MCP: {output}[/red]")

    return success


def verify_installation() -> bool:
    console.print("\n🔍 Verifying MCP server connections...")
    success, output = run_command(["claude", "mcp", "list"])

    if success:
        console.print(output)
        # Check all configured servers are present
        return all(server.name in output for server in MCP_SERVERS)

    return False


@app.command()
def install(
    settings_dir: Path = typer.Option(
        Path.home() / "settings", help="Path to settings directory"
    ),
    skip_confirm: bool = typer.Option(
        False, "--yes", "-y", help="Skip confirmation prompt"
    ),
    force: bool = typer.Option(
        False,
        "--force",
        "-f",
        help="Force reinstall by removing existing servers first",
    ),
):
    """Install and configure MCP servers in Claude."""
    server_names = ", ".join(s.name for s in MCP_SERVERS)
    console.print(
        f"[bold cyan]🚀 Installing MCP Servers in Claude ({server_names})...[/bold cyan]\n"
    )

    console.print("This script will install and configure:")
    for server in MCP_SERVERS:
        console.print(f"  • {server.display_name} ({server.description})")
    console.print()

    if force:
        console.print(
            "[yellow]⚠️  Force mode: Will reinstall existing servers[/yellow]\n"
        )

    if not skip_confirm and not Confirm.ask("Continue?"):
        console.print("Installation cancelled")
        raise typer.Exit(0)

    console.print()

    # Install all servers
    results = []
    for server in MCP_SERVERS:
        success = install_mcp_server(server, settings_dir, force_reinstall=force)
        results.append(success)
        console.print()

    # Verify installation
    all_connected = verify_installation()

    # Final status
    console.print(
        "\n[bold green]✨ MCP Server installation in Claude complete![/bold green]\n"
    )

    if not all_connected:
        console.print("[yellow]⚠️  Some servers may not be fully connected[/yellow]\n")

    console.print("Next steps:")
    console.print(
        "  1. Ensure GITHUB_PERSONAL_ACCESS_TOKEN is set in environment or secretBox.json"
    )
    console.print("  2. Restart Claude Desktop to apply changes")
    console.print("  3. Verify servers are connected: [cyan]claude mcp list[/cyan]")

    if not all(results):
        raise typer.Exit(1)


@app.command()
def list():
    """List installed MCP servers."""
    success, output = run_command(["claude", "mcp", "list"])

    if success:
        console.print(output)
    else:
        console.print(f"[red]Failed to list MCP servers: {output}[/red]")
        raise typer.Exit(1)


@app.command()
def remove(
    server_name: str = typer.Argument(..., help="Name of the MCP server to remove"),
):
    """Remove an MCP server from Claude."""
    if not Confirm.ask(f"Remove MCP server '{server_name}'?"):
        console.print("Removal cancelled")
        raise typer.Exit(0)

    success, output = run_command(["claude", "mcp", "remove", server_name])

    if success:
        console.print(f"[green]✅ Removed MCP server '{server_name}'[/green]")
    else:
        console.print(f"[red]Failed to remove server: {output}[/red]")
        raise typer.Exit(1)


@app.command()
def uninstall_all(
    skip_confirm: bool = typer.Option(
        False, "--yes", "-y", help="Skip confirmation prompt"
    ),
):
    """Remove all configured MCP servers from Claude."""
    console.print(
        "[bold red]🗑️  Uninstalling all MCP servers from Claude...[/bold red]\n"
    )

    # Get current list of servers
    success, output = run_command(["claude", "mcp", "list"])
    if not success:
        console.print("[red]Failed to get list of installed servers[/red]")
        raise typer.Exit(1)

    # Extract server names from output
    installed_servers = []
    for server in MCP_SERVERS:
        if server.name in output:
            installed_servers.append(server)

    if not installed_servers:
        console.print("[yellow]No MCP servers found to uninstall[/yellow]")
        return

    console.print("The following servers will be removed:")
    for server in installed_servers:
        console.print(f"  • {server.display_name}")
    console.print()

    if not skip_confirm and not Confirm.ask("Continue with uninstall?"):
        console.print("Uninstall cancelled")
        raise typer.Exit(0)

    console.print()

    # Remove each server
    removed_count = 0
    for server in installed_servers:
        console.print(f"Removing {server.display_name}...")
        if remove_mcp_server(server.name):
            console.print(f"  [green]✅ Removed {server.name}[/green]")
            removed_count += 1
        else:
            console.print(f"  [red]❌ Failed to remove {server.name}[/red]")

    console.print(
        f"\n[bold]Removed {removed_count} of {len(installed_servers)} servers[/bold]"
    )

    if removed_count < len(installed_servers):
        raise typer.Exit(1)


if __name__ == "__main__":
    app()
