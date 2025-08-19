#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "typer", "rich", "docker",
# ]
# ///

"""
Docker Container Manager for Claude Development Environment

This script manages Docker containers for the Claude development environment,
providing an interactive menu to create, attach, and manage containers with
proper state persistence.
"""

import json
import os
import re
import socket
import stat
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, List

import docker
import typer
from rich.console import Console
from rich.panel import Panel
from rich.prompt import Prompt, IntPrompt, Confirm
from rich.table import Table
from docker.errors import NotFound, APIError

app = typer.Typer(
    help="Claude Docker Container Manager", no_args_is_help=False, add_completion=False
)
console = Console()

DEFAULT_IMAGE = "claude-docker:dev"
STATE_FILE = Path.home() / ".claude-docker-state.json"
DOCKER_DIR = Path(__file__).parent.absolute()
PORT_SEARCH_RANGE = 100
DEFAULT_JEKYLL_PORT = 5000
DEFAULT_LIVERELOAD_PORT = 35729
CONTAINER_NAME_PATTERN = re.compile(r"^C-\d+$")

# Terminal configuration constants
TRUE_COLOR_TERMINALS = [
    "truecolor",
    "direct",
    "ghostty",
    "kitty",
    "alacritty",
    "wezterm",
]
TERM_TRUE_COLOR = "xterm-direct"
TERM_256_COLOR = "xterm-256color"


def determine_container_term() -> str:
    """Determine the best TERM value for container based on host terminal.

    Returns:
        Terminal type string optimized for container use
    """
    host_term = os.environ.get("TERM", TERM_256_COLOR)

    # Check if host supports true color
    if any(term_type in host_term.lower() for term_type in TRUE_COLOR_TERMINALS):
        return TERM_TRUE_COLOR

    return TERM_256_COLOR


class ContainerState:
    """Manages container state persistence"""

    def __init__(self, state_file: Path = STATE_FILE):
        self.state_file = state_file
        self.state = self._load_state()

    def _load_state(self) -> Dict:
        """Load state from file or create new"""
        if self.state_file.exists():
            try:
                with open(self.state_file) as f:
                    return json.load(f)
            except json.JSONDecodeError:
                console.print("[yellow]⚠ Invalid state file, creating new one[/yellow]")
        return {"containers": []}

    def save(self):
        """Save state to file with proper permissions"""
        with open(self.state_file, "w") as f:
            json.dump(self.state, f, indent=2)
        # Set restrictive permissions (owner read/write only)
        self.state_file.chmod(stat.S_IRUSR | stat.S_IWUSR)

    def add_container(
        self, name: str, image: str, jekyll_port: int, livereload_port: int
    ):
        """Add a new container to state"""
        container = {
            "name": name,
            "image": image,
            "ports": {"jekyll": jekyll_port, "livereload": livereload_port},
            "created_at": datetime.now().isoformat(),
            "last_used": datetime.now().isoformat(),
        }
        self.state["containers"].append(container)
        self.save()

    def update_last_used(self, name: str):
        """Update last used timestamp for container"""
        for container in self.state["containers"]:
            if container["name"] == name:
                container["last_used"] = datetime.now().isoformat()
                self.save()
                break

    def remove_container(self, name: str):
        """Remove container from state"""
        self.state["containers"] = [
            c for c in self.state["containers"] if c["name"] != name
        ]
        self.save()

    def get_container(self, name: str) -> Optional[Dict]:
        """Get container info by name"""
        for container in self.state["containers"]:
            if container["name"] == name:
                return container
        return None

    def list_containers(self) -> List[Dict]:
        """Get all containers"""
        return self.state["containers"]


class DockerManager:
    """Manages Docker operations"""

    def __init__(self):
        try:
            self.client = docker.from_env()
        except docker.errors.DockerException as e:
            console.print(f"[red]❌ Docker not available: {e}[/red]")
            sys.exit(1)
        self.state = ContainerState()

    def find_free_port(self, start_port: int) -> Optional[int]:
        """Find a free port starting from start_port"""
        for port in range(start_port, start_port + PORT_SEARCH_RANGE):
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                try:
                    s.bind(("", port))
                    return port
                except OSError:
                    continue
        return None

    def get_container_status(self, name: str) -> str:
        """Get container status (running/stopped/not found)"""
        try:
            container = self.client.containers.get(name)
            return container.status
        except NotFound:
            return "not found"

    def build_volume_mounts(self, container_name: str) -> Dict[str, Dict]:
        """Build volume mount configuration including persistent volumes"""
        mounts = {}

        # Define allowlist of directories to mount under /ro_host/
        host_mount_allowlist = [
            ("~/.claude", ".claude"),
            ("~/.config/claude", ".config_claude"),
            ("~/settings", "settings"),
        ]

        # Mount allowlisted directories under /ro_host/{name}
        for path_str, mount_name in host_mount_allowlist:
            host_path = Path(path_str).expanduser().resolve()
            if host_path.exists():
                mounts[str(host_path)] = {
                    "bind": f"/ro_host/{mount_name}",
                    "mode": "ro",
                }

        # Mount current working directory (where script is run from)
        cwd = Path.cwd().resolve()
        # Get a safe directory name for mounting
        cwd_name = cwd.name or "workspace"
        mounts[str(cwd)] = {"bind": f"/ro_host/{cwd_name}", "mode": "ro"}

        # Docker directory (read-only) - keeping for backward compatibility
        mounts[str(DOCKER_DIR)] = {"bind": "/ro_host_docker", "mode": "ro"}

        return mounts

    def get_volume_name(self, container_name: str, volume_type: str = "home") -> str:
        """Get the name for a persistent volume"""
        return f"{container_name}-{volume_type}"

    def create_persistent_volumes(self, container_name: str) -> List[str]:
        """Create persistent named volumes for container"""
        volumes = []

        # Create home directory volume
        home_volume = self.get_volume_name(container_name, "home")
        try:
            self.client.volumes.create(name=home_volume)
            console.print(f"[green]✓[/green] Created persistent volume: {home_volume}")
        except APIError as e:
            if "already exists" in str(e):
                console.print(
                    f"[yellow]⚠[/yellow] Volume {home_volume} already exists (will reuse)"
                )
            else:
                raise
        volumes.append(home_volume)

        # Create workspace volume for /workspace
        workspace_volume = self.get_volume_name(container_name, "workspace")
        try:
            self.client.volumes.create(name=workspace_volume)
            console.print(
                f"[green]✓[/green] Created persistent volume: {workspace_volume}"
            )
        except APIError as e:
            if "already exists" in str(e):
                console.print(
                    f"[yellow]⚠[/yellow] Volume {workspace_volume} already exists (will reuse)"
                )
            else:
                raise
        volumes.append(workspace_volume)

        return volumes

    def build_environment(self, container_name: str) -> Dict[str, str]:
        """Build environment variables"""
        env = {}

        # Explicit allowlist of environment variables to pass through
        env_allowlist = [
            # API Keys
            "ASSEMBLYAI_API_KEY",
            "DEEPGRAM_API_KEY",
            "ELEVEN_API_KEY",
            "EXA_API_KEY",
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "GITHUB_PERSONAL_ACCESS_TOKEN",
            "GROQ_API_KEY",
            "LANGCHAIN_API_KEY",
            "ONEBUSAWAY_API_KEY",
            "OPENAI_API_KEY",
            "PPLX_API_KEY",
            "REPLICATE_API_TOKEN",
            "TONY_API_KEY",
            "TONY_STORAGE_SERVER_API_KEY",
            "VAPI_API_KEY",
        ]

        # Pass through allowlisted environment variables from host
        for var_name in env_allowlist:
            if value := os.environ.get(var_name):
                # Only add if value is not empty
                if value.strip():
                    env[var_name] = value

        # Special handling for GH_TOKEN and GITHUB_TOKEN - try 1Password if not in environment
        if "GH_TOKEN" not in env or not env.get("GH_TOKEN"):
            try:
                # Try to fetch from 1Password
                result = subprocess.run(
                    [
                        "op",
                        "read",
                        "op://Personal/GitHub AI Personal Access Token/token",
                    ],
                    capture_output=True,
                    text=True,
                    timeout=5,
                )
                if result.returncode == 0 and result.stdout.strip():
                    token = result.stdout.strip()
                    env["GH_TOKEN"] = token
                    # Also set GITHUB_TOKEN for compatibility
                    if "GITHUB_TOKEN" not in env:
                        env["GITHUB_TOKEN"] = token
                    console.print(
                        "[green]✓[/green] GitHub token retrieved from 1Password"
                    )
            except (subprocess.TimeoutExpired, FileNotFoundError):
                # op command not available or timed out
                pass

        # TODO: Get AI Tools git values

        # Hard code Git config
        git_name = "AI+idvorkin"
        git_email = "aitools-idvorkin@gmail.com"

        env["GIT_AUTHOR_NAME"] = git_name
        env["GIT_AUTHOR_EMAIL"] = git_email

        # Playwright
        env["PLAYWRIGHT_BROWSERS_PATH"] = "/home/developer/.cache/ms-playwright"

        # Container name for prompt
        env["DOCKER_CONTAINER_NAME"] = container_name

        # Terminal - use safe defaults but preserve true color if possible
        env["TERM"] = determine_container_term()

        return env

    def list_containers(self):
        """List all containers with their status"""
        containers = self.state.list_containers()

        if not containers:
            console.print("[yellow]No containers found[/yellow]")
            return

        # Clean up state for non-existent containers
        for container in containers[:]:
            status = self.get_container_status(container["name"])
            if status == "not found":
                self.state.remove_container(container["name"])
                containers.remove(container)

        if not containers:
            console.print(
                "[yellow]No containers found (cleaned up stale entries)[/yellow]"
            )
            return

        table = Table(
            title="Claude Docker Containers", show_header=True, header_style="bold cyan"
        )
        table.add_column("#", style="green", width=3)
        table.add_column("Name", style="cyan")
        table.add_column("Status", justify="center")
        table.add_column("Jekyll Port", style="green")
        table.add_column("LiveReload Port", style="green")
        table.add_column("Last Used")

        for i, container in enumerate(containers, 1):
            status = self.get_container_status(container["name"])
            status_style = (
                "green"
                if status == "running"
                else "red"
                if status == "exited"
                else "yellow"
            )

            try:
                last_used = datetime.fromisoformat(container["last_used"]).strftime(
                    "%Y-%m-%d %H:%M"
                )
            except (ValueError, KeyError):
                last_used = "Unknown"

            table.add_row(
                str(i),
                container["name"],
                f"[{status_style}]{status}[/{status_style}]",
                str(container["ports"]["jekyll"]),
                str(container["ports"]["livereload"]),
                last_used,
            )

        console.print(table)

    def attach_container(self, container_name: str):
        """Attach to an existing container"""
        # Validate container name format
        if not CONTAINER_NAME_PATTERN.match(container_name):
            console.print(
                f"[red]❌ Invalid container name format: {container_name}[/red]"
            )
            return

        container_info = self.state.get_container(container_name)
        if not container_info:
            console.print(
                f"[red]❌ Container {container_name} not found in state[/red]"
            )
            return

        try:
            container = self.client.containers.get(container_name)

            # Start if stopped
            if container.status != "running":
                console.print("[yellow]⚠ Container is stopped, starting it...[/yellow]")
                container.start()

            # Update last used
            self.state.update_last_used(container_name)

            # Display port info
            jekyll_port = container_info["ports"]["jekyll"]
            livereload_port = container_info["ports"]["livereload"]

            console.print(
                Panel(
                    f"[green]Jekyll:[/green] http://localhost:{jekyll_port}\n"
                    f"[green]LiveReload:[/green] {livereload_port}",
                    title="Container Ports",
                    border_style="green",
                )
            )

            # Attach to container
            console.print(
                f"\n[green]🐳 Attaching to container: {container_name}[/green]\n"
            )

            # Determine best TERM value for true color support
            term_value = determine_container_term()

            # Use subprocess to attach to tmux session
            subprocess.run(
                [
                    "docker",
                    "exec",
                    "-it",
                    "-e",
                    f"DOCKER_CONTAINER_NAME={container_name}",
                    "-e",
                    f"TERM={term_value}",
                    container_name,
                    "/home/linuxbrew/.linuxbrew/bin/tmux",
                    "attach-session",
                    "-t",
                    "main",
                ]
            )

        except NotFound:
            console.print(f"[red]❌ Container {container_name} not found[/red]")
            self.state.remove_container(container_name)
        except APIError as e:
            console.print(f"[red]❌ Docker API error: {e}[/red]")

    def create_container(self, image: str = DEFAULT_IMAGE):
        """Create and run a new container"""
        console.print(
            f"[green]🐳 Creating new Docker container with image: {image}[/green]\n"
        )

        # Find free ports
        jekyll_port = self.find_free_port(DEFAULT_JEKYLL_PORT)
        livereload_port = self.find_free_port(DEFAULT_LIVERELOAD_PORT)

        if not jekyll_port or not livereload_port:
            console.print("[red]❌ Failed to find free ports[/red]")
            return

        if (
            jekyll_port != DEFAULT_JEKYLL_PORT
            or livereload_port != DEFAULT_LIVERELOAD_PORT
        ):
            console.print(
                Panel(
                    f"[yellow]Using alternate ports:[/yellow]\n"
                    f"Jekyll: [green]{jekyll_port}[/green] (instead of 5000)\n"
                    f"LiveReload: [green]{livereload_port}[/green] (instead of 35729)",
                    border_style="yellow",
                )
            )

        # Generate container name based on Jekyll port
        container_name = f"C-{jekyll_port}"

        # Check if container already exists
        existing_status = self.get_container_status(container_name)
        if existing_status != "not found":
            console.print(
                f"[yellow]⚠ Container {container_name} already exists (status: {existing_status})[/yellow]"
            )

            # If it's stopped, offer to restart it
            if existing_status == "exited":
                if Confirm.ask(
                    f"Container {container_name} is stopped. Restart it?", default=True
                ):
                    self.attach_container(container_name)
                    return
                else:
                    # Find a new port if user doesn't want to restart
                    for port_offset in range(1, PORT_SEARCH_RANGE):
                        new_jekyll_port = jekyll_port + port_offset
                        new_container_name = f"C-{new_jekyll_port}"
                        if (
                            self.get_container_status(new_container_name) == "not found"
                            and self.find_free_port(new_jekyll_port) == new_jekyll_port
                        ):
                            jekyll_port = new_jekyll_port
                            livereload_port = self.find_free_port(
                                DEFAULT_LIVERELOAD_PORT + port_offset
                            )
                            container_name = new_container_name
                            console.print(
                                f"[green]Using alternate container: {container_name}[/green]"
                            )
                            break
                    else:
                        console.print(
                            "[red]❌ Could not find available container name[/red]"
                        )
                        return
            elif existing_status == "running":
                console.print(
                    f"[yellow]Container {container_name} is already running[/yellow]"
                )
                if Confirm.ask("Attach to it?", default=True):
                    self.attach_container(container_name)
                return

        # Build configuration
        volumes = self.build_volume_mounts(container_name)
        environment = self.build_environment(container_name)

        # Create persistent volumes
        self.create_persistent_volumes(container_name)

        # Display mount status
        mount_info = []

        # Show /ro_host mounts
        ro_host_mounts = []
        for host_path, mount_config in volumes.items():
            if mount_config["bind"].startswith("/ro_host/"):
                mount_name = mount_config["bind"].replace("/ro_host/", "")
                # Skip the "_rw" suffixed duplicate entries
                if not host_path.endswith("_rw"):
                    ro_host_mounts.append(f"  • {mount_name} → {host_path}")

        if "GH_TOKEN" in environment or "GITHUB_TOKEN" in environment:
            mount_info.append("[green]✓[/green] GitHub token configured")

        console.print(
            Panel("\n".join(mount_info), title="Configuration", border_style="green")
        )

        # Save container state
        self.state.add_container(container_name, image, jekyll_port, livereload_port)

        console.print(f"\n[green]✓ Container name: {container_name}[/green]\n")

        # Run container using subprocess - start in detached mode first
        cmd = ["docker", "run", "-d", "--name", container_name]

        # Add persistent volumes
        home_volume = self.get_volume_name(container_name, "home")
        workspace_volume = self.get_volume_name(container_name, "workspace")
        cmd.extend(["-v", f"{home_volume}:/home/developer"])
        cmd.extend(["-v", f"{workspace_volume}:/workspace"])

        # Add bind mounts
        for host_path, mount_config in volumes.items():
            cmd.extend(
                ["-v", f"{host_path}:{mount_config['bind']}:{mount_config['mode']}"]
            )

        # Add environment variables
        for key, value in environment.items():
            cmd.extend(["-e", f"{key}={value}"])

        # Add port mappings
        cmd.extend(["-p", f"{jekyll_port}:4000", "-p", f"{livereload_port}:35729"])

        # Add image and command
        # Use sleep infinity to keep container alive, then create tmux session inside
        cmd.extend(
            [
                image,
                "sh",
                "-c",
                "/home/linuxbrew/.linuxbrew/bin/tmux new-session -d -s main /home/linuxbrew/.linuxbrew/bin/zsh && tail -f /dev/null",
            ]
        )

        # Run the container in detached mode
        result = subprocess.run(cmd)

        if result.returncode == 0:
            console.print(
                f"\n[green]✓ Container {container_name} started with tmux session[/green]"
            )
            console.print("[dim]Container will stay alive even if you disconnect[/dim]")

            # Now attach to the container
            console.print(
                f"\n[green]🐳 Attaching to container: {container_name}[/green]\n"
            )
            self.state.update_last_used(container_name)

            # Display port info
            console.print(
                Panel(
                    f"[green]Jekyll:[/green] http://localhost:{jekyll_port}\n"
                    f"[green]LiveReload:[/green] {livereload_port}",
                    title="Container Ports",
                    border_style="green",
                )
            )

            # Attach to the tmux session
            subprocess.run(
                [
                    "docker",
                    "exec",
                    "-it",
                    "-e",
                    f"DOCKER_CONTAINER_NAME={container_name}",
                    "-e",
                    f"TERM={determine_container_term()}",
                    container_name,
                    "/home/linuxbrew/.linuxbrew/bin/tmux",
                    "attach-session",
                    "-t",
                    "main",
                ]
            )

    def delete_container(self, container_name: str, delete_volumes: bool = False):
        """Delete a container and optionally its volumes"""
        try:
            container = self.client.containers.get(container_name)
            container.remove(force=True)
            self.state.remove_container(container_name)
            console.print(f"[green]✓ Container {container_name} deleted[/green]")

            # Delete volumes if requested
            if delete_volumes:
                for volume_type in ["home", "workspace"]:
                    volume_name = self.get_volume_name(container_name, volume_type)
                    try:
                        volume = self.client.volumes.get(volume_name)
                        volume.remove()
                        console.print(f"[green]✓ Deleted volume: {volume_name}[/green]")
                    except NotFound:
                        pass
                    except APIError as e:
                        console.print(
                            f"[yellow]⚠ Could not delete volume {volume_name}: {e}[/yellow]"
                        )
        except NotFound:
            self.state.remove_container(container_name)
            console.print(
                f"[yellow]Container {container_name} not found, removed from state[/yellow]"
            )
        except APIError as e:
            console.print(f"[red]❌ Failed to delete container: {e}[/red]")


@app.command()
def interactive():
    """Interactive menu for container management (default)"""
    manager = DockerManager()

    console.print(
        Panel(
            "[bold cyan]Claude Docker Container Manager[/bold cyan]",
            expand=False,
            border_style="cyan",
        )
    )

    while True:
        console.print("\n")
        manager.list_containers()
        console.print("\n")

        console.print("[green][N][/green] Create new container")
        console.print("[green][A][/green] Attach to container by number")
        console.print("[green][D][/green] Delete container")
        console.print("[green][R][/green] Refresh list")
        console.print("[green][Q][/green] Quit")
        console.print()

        choice = Prompt.ask("Choose an option", default="q").lower()

        if choice == "n":
            manager.create_container()
            break
        elif choice == "a":
            containers = manager.state.list_containers()
            if containers:
                num = IntPrompt.ask(
                    "Enter container number",
                    default=1,
                    choices=[str(i) for i in range(1, len(containers) + 1)],
                )
                if 1 <= num <= len(containers):
                    manager.attach_container(containers[num - 1]["name"])
                    break
                else:
                    console.print("[red]Invalid container number[/red]")
            else:
                console.print("[yellow]No containers available[/yellow]")
        elif choice == "d":
            containers = manager.state.list_containers()
            if containers:
                num = IntPrompt.ask(
                    "Enter container number to delete",
                    default=1,
                    choices=[str(i) for i in range(1, len(containers) + 1)],
                )
                if 1 <= num <= len(containers):
                    container_name = containers[num - 1]["name"]
                    if Confirm.ask(f"Delete container {container_name}?"):
                        delete_volumes = Confirm.ask(
                            "Also delete persistent volumes (all data will be lost)?",
                            default=False,
                        )
                        manager.delete_container(container_name, delete_volumes)
                else:
                    console.print("[red]Invalid container number[/red]")
            else:
                console.print("[yellow]No containers available[/yellow]")
        elif choice == "r":
            continue
        elif choice == "q":
            console.print("[green]Goodbye![/green]")
            break
        else:
            console.print("[red]Invalid option[/red]")


@app.command()
def list():
    """List all containers"""
    manager = DockerManager()
    manager.list_containers()


@app.command()
def create(image: str = typer.Argument(DEFAULT_IMAGE, help="Docker image to use")):
    """Create a new container"""
    manager = DockerManager()
    manager.create_container(image)


@app.command()
def attach(name: str = typer.Argument(..., help="Container name to attach to")):
    """Attach to an existing container"""
    manager = DockerManager()
    manager.attach_container(name)


@app.command()
def delete(
    name: str = typer.Argument(..., help="Container name to delete"),
    volumes: bool = typer.Option(
        False, "--volumes", "-v", help="Also delete persistent volumes"
    ),
):
    """Delete a container"""
    manager = DockerManager()
    if Confirm.ask(f"Delete container {name}?"):
        if volumes or Confirm.ask(
            "Also delete persistent volumes (all data will be lost)?", default=False
        ):
            manager.delete_container(name, delete_volumes=True)
        else:
            manager.delete_container(name, delete_volumes=False)


@app.command()
def clean():
    """Remove all Claude Docker containers"""
    if not Confirm.ask("Remove ALL Claude Docker containers?", default=False):
        return

    manager = DockerManager()
    containers = manager.state.list_containers()

    delete_volumes = Confirm.ask(
        "Also delete all persistent volumes (all data will be lost)?", default=False
    )

    for container in containers:
        console.print(f"Deleting {container['name']}...")
        manager.delete_container(container["name"], delete_volumes)

    console.print("[green]✓ Cleanup complete[/green]")


@app.command()
def volumes():
    """List all Docker volumes used by Claude containers"""
    manager = DockerManager()
    containers = manager.state.list_containers()

    if not containers:
        console.print("[yellow]No containers found[/yellow]")
        return

    table = Table(
        title="Claude Docker Volumes", show_header=True, header_style="bold cyan"
    )
    table.add_column("Container", style="cyan")
    table.add_column("Volume Name", style="green")
    table.add_column("Type", style="yellow")
    table.add_column("Status", justify="center")

    for container in containers:
        container_name = container["name"]
        for volume_type in ["home", "workspace"]:
            volume_name = manager.get_volume_name(container_name, volume_type)
            try:
                manager.client.volumes.get(volume_name)
                status = "[green]exists[/green]"
            except docker.errors.NotFound:
                status = "[red]missing[/red]"

            table.add_row(
                container_name,
                volume_name,
                volume_type,
                status,
            )

    console.print(table)


@app.command()
def rebuild(
    no_cache: bool = typer.Option(
        True, "--no-cache/--cache", help="Build without using cache"
    ),
    image_tag: str = typer.Option(
        "dev", "--tag", "-t", help="Image tag to build (dev, minimal, claude)"
    ),
):
    """Force rebuild the Docker image"""
    console.print(
        f"[bold cyan]🔨 Rebuilding Docker image: claude-docker:{image_tag}[/bold cyan]\n"
    )

    # Map tag to build script
    build_scripts = {
        "dev": "build-dev.sh",
        "minimal": "build-minimal.sh",
        "claude": "build-claude.sh",
    }

    if image_tag not in build_scripts:
        console.print(f"[red]❌ Invalid image tag: {image_tag}[/red]")
        console.print(f"Available tags: {', '.join(build_scripts.keys())}")
        return

    build_script = DOCKER_DIR / build_scripts[image_tag]

    if not build_script.exists():
        console.print(f"[red]❌ Build script not found: {build_script}[/red]")
        return

    # Build the Docker image
    cmd = ["bash", str(build_script)]

    if no_cache:
        console.print(
            "[yellow]Building without cache (this may take a while)...[/yellow]\n"
        )
        # For no-cache builds, we need to modify the docker build command
        # Most build scripts use docker build, so we'll set an env var
        env = os.environ.copy()
        env["DOCKER_BUILD_NO_CACHE"] = "1"
        result = subprocess.run(cmd, cwd=str(DOCKER_DIR), env=env)
    else:
        console.print("[yellow]Building with cache...[/yellow]\n")
        result = subprocess.run(cmd, cwd=str(DOCKER_DIR))

    if result.returncode == 0:
        console.print(
            f"\n[green]✓ Successfully rebuilt claude-docker:{image_tag}[/green]"
        )
        console.print(
            f"[dim]You can now create a container with: ./run-docker.py create claude-docker:{image_tag}[/dim]"
        )
    else:
        console.print(
            f"\n[red]❌ Build failed with exit code {result.returncode}[/red]"
        )


@app.command()
def test():
    """Run tests in a new container"""
    console.print("[green]🧪 Running tests in container[/green]\n")

    cmd = [
        "docker",
        "run",
        "--rm",
        "-v",
        f"{Path.home() / '.gitconfig'}:/home/developer/.gitconfig:ro",
        "-e",
        f"GITHUB_TOKEN={os.environ.get('GITHUB_TOKEN', '')}",
        "--network",
        "host",
        DEFAULT_IMAGE,
        "bash",
        "-c",
        """
        cd ~/repos/blog
        echo '📦 Installing dependencies...'
        npm install
        echo '🧪 Running unit tests...'
        npm test
        echo '🎭 Running Playwright tests...'
        npx playwright test --reporter=list
        """,
    ]

    subprocess.run(cmd)


@app.command()
def shell(image: str = typer.Argument(DEFAULT_IMAGE, help="Docker image to use")):
    """Start a bash shell in a new container"""
    manager = DockerManager()
    console.print(f"[green]🐚 Starting bash shell with image: {image}[/green]\n")

    # Create container but override command to use bash
    manager.create_container(image)


@app.command()
def check():
    """Check prerequisites"""
    checks = []

    # GitHub token
    if os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN"):
        checks.append("[green]✓[/green] GitHub token is set (environment)")
    else:
        # Try to fetch from 1Password
        try:
            result = subprocess.run(
                ["op", "read", "op://Personal/GitHub AI Personal Access Token/token"],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0 and result.stdout.strip():
                checks.append("[green]✓[/green] GitHub token available (1Password)")
            else:
                checks.append("[yellow]⚠[/yellow] GitHub token not set")
                checks.append("  Export GITHUB_TOKEN=ghp_your_token_here")
                checks.append("  Or configure in 1Password")
        except (subprocess.TimeoutExpired, FileNotFoundError):
            checks.append(
                "[yellow]⚠[/yellow] GitHub token not set (1Password not available)"
            )
            checks.append("  Export GITHUB_TOKEN=ghp_your_token_here")

    # Git config
    if (Path.home() / ".gitconfig").exists():
        checks.append("[green]✓[/green] Git config found")
    else:
        checks.append("[yellow]⚠[/yellow] No .gitconfig found")

    # SSH
    if (Path.home() / ".ssh").exists():
        checks.append("[green]✓[/green] SSH config found")
    else:
        checks.append("[yellow]⚠[/yellow] No .ssh directory found")

    # Claude credentials
    claude_dirs = [Path.home() / ".claude", Path.home() / ".config" / "claude"]
    if any(d.exists() for d in claude_dirs):
        checks.append("[green]✓[/green] Claude credentials found")
    else:
        checks.append("[yellow]⚠[/yellow] Claude credentials not found")
        checks.append("  Run 'claude auth login' first")

    # Docker
    try:
        client = docker.from_env()
        version = client.version()
        checks.append(
            f"[green]✓[/green] Docker is running (version {version['Version']})"
        )
    except Exception:
        checks.append("[red]❌[/red] Docker is not running")

    console.print(
        Panel("\n".join(checks), title="Prerequisites Check", border_style="cyan")
    )

    # Check API keys that will be passed through
    env_allowlist = [
        "ANTHROPIC_API_KEY",
        "ASSEMBLYAI_API_KEY",
        "DEEPGRAM_API_KEY",
        "ELEVEN_API_KEY",
        "EXA_API_KEY",
        "GITHUB_TOKEN",
        "GITHUB_PERSONAL_ACCESS_TOKEN",
        "GOOGLE_API_KEY",
        "GROQ_API_KEY",
        "LANGCHAIN_API_KEY",
        "ONEBUSAWAY_API_KEY",
        "OPENAI_API_KEY",
        "PPLX_API_KEY",
        "REPLICATE_API_TOKEN",
        "TONY_API_KEY",
        "TONY_STORAGE_SERVER_API_KEY",
        "VAPI_API_KEY",
        "ZEP_API_KEY",
        "TWILIO_ACCOUNT_SID",
        "TWILIO_AUTH_TOKEN",
        "TWILIO_FROM_NUMBER",
        "IFTTT_WEBHOOK_KEY",
        "IFTTT_WEBHOOK_SMS_EVENT",
        "BING_SEARCH_URL",
    ]

    api_keys = []
    for var_name in sorted(env_allowlist):
        if value := os.environ.get(var_name):
            if value.strip():
                # Show first few chars for security
                display_value = value[:8] + "..." if len(value) > 12 else "***"
                api_keys.append(f"[green]✓[/green] {var_name}: {display_value}")
        else:
            api_keys.append(f"[dim]○ {var_name}: not set[/dim]")

    if api_keys:
        console.print(
            Panel("\n".join(api_keys), title="API Keys & Tokens", border_style="cyan")
        )


if __name__ == "__main__":
    # Default to interactive mode if no command given
    if len(sys.argv) == 1:
        interactive()
    else:
        app()
