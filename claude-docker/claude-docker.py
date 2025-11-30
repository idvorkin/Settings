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
import time
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

    def has_active_exec_sessions(self, container_name: str) -> bool:
        """Check if container has active exec sessions (someone attached)"""
        try:
            # Use docker inspect to check for exec sessions
            result = subprocess.run(
                ["docker", "inspect", container_name, "--format", "{{json .ExecIDs}}"],
                capture_output=True,
                text=True,
                timeout=2,
            )

            if result.returncode == 0:
                exec_ids = result.stdout.strip()
                # Check if there are any exec IDs (not null and not empty array)
                if exec_ids and exec_ids != "null" and exec_ids != "[]":
                    return True

            # Alternative method: check for tmux sessions with clients attached
            check_tmux = subprocess.run(
                [
                    "docker",
                    "exec",
                    container_name,
                    "/home/linuxbrew/.linuxbrew/bin/tmux",
                    "list-clients",
                    "-t",
                    "main",
                ],
                capture_output=True,
                timeout=2,
            )

            # If tmux has clients attached, someone is connected
            if check_tmux.returncode == 0 and check_tmux.stdout:
                return True

        except (subprocess.TimeoutExpired, subprocess.CalledProcessError):
            pass

        return False

    def build_volume_mounts(self, container_name: str) -> Dict[str, Dict]:
        """Build volume mount configuration including persistent volumes"""
        mounts = {}

        # Mount allowlisted directories under /ro_host/{name}
        # NO Claude auth - container handles its own auth
        host_mount_allowlist = [
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

    def select_container_with_fzf(self) -> Optional[str]:
        """Use fzf to select a container from available containers"""
        containers = self.state.list_containers()

        if not containers:
            console.print("[yellow]No containers found[/yellow]")
            return None

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
            return None

        # Try to use fzf if available
        try:
            # Prepare container list for fzf
            fzf_input = []
            for container in containers:
                status = self.get_container_status(container["name"])
                status_icon = (
                    "🟢"
                    if status == "running"
                    else "🔴"
                    if status == "exited"
                    else "🟡"
                )

                # Check if attached
                attached_icon = ""
                if status == "running" and self.has_active_exec_sessions(
                    container["name"]
                ):
                    attached_icon = " 👤"  # Person icon for attached

                try:
                    last_used = datetime.fromisoformat(container["last_used"]).strftime(
                        "%Y-%m-%d %H:%M"
                    )
                except (ValueError, KeyError):
                    last_used = "Unknown"

                line = f"{status_icon}{attached_icon} {container['name']:<8} | Port: {container['ports']['jekyll']:<5} | Last used: {last_used}"
                fzf_input.append(line)

            # Run fzf
            process = subprocess.Popen(
                [
                    "fzf",
                    "--header=Select container to attach:",
                    "--height=40%",
                    "--layout=reverse",
                    "--info=inline",
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )

            stdout, stderr = process.communicate(input="\n".join(fzf_input))

            if process.returncode == 0 and stdout.strip():
                # Extract container name from selected line
                selected = stdout.strip()
                container_name = selected.split("|")[0].strip().split()[1]
                return container_name
            else:
                return None

        except FileNotFoundError:
            # fzf not available, fall back to numbered menu
            console.print(
                "[yellow]fzf not found, using numbered menu instead[/yellow]\n"
            )
            return self.select_container_with_menu()

    def select_container_with_menu(self) -> Optional[str]:
        """Fallback numbered menu for container selection"""
        containers = self.state.list_containers()

        if not containers:
            return None

        # Display containers
        console.print("\n[bold cyan]Available Containers:[/bold cyan]\n")
        for i, container in enumerate(containers, 1):
            status = self.get_container_status(container["name"])
            status_icon = (
                "🟢" if status == "running" else "🔴" if status == "exited" else "🟡"
            )

            # Check if attached
            attached_text = ""
            if status == "running" and self.has_active_exec_sessions(container["name"]):
                attached_text = " [magenta](attached)[/magenta]"

            try:
                last_used = datetime.fromisoformat(container["last_used"]).strftime(
                    "%Y-%m-%d %H:%M"
                )
            except (ValueError, KeyError):
                last_used = "Unknown"

            console.print(
                f"  [{i}] {status_icon} {container['name']}{attached_text} - Port: {container['ports']['jekyll']} - Last used: {last_used}"
            )

        console.print("\n  [0] Cancel\n")

        try:
            choice = IntPrompt.ask(
                "Select container",
                default=1,
                choices=[str(i) for i in range(0, len(containers) + 1)],
            )

            if choice == 0:
                return None
            elif 1 <= choice <= len(containers):
                return containers[choice - 1]["name"]
            else:
                console.print("[red]Invalid selection[/red]")
                return None
        except KeyboardInterrupt:
            return None

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
        table.add_column("Attached", justify="center")
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

            # Check if someone is attached
            attached = ""
            if status == "running":
                if self.has_active_exec_sessions(container["name"]):
                    attached = "[magenta]●[/magenta]"  # Purple dot for attached
                else:
                    attached = "[dim]○[/dim]"  # Empty circle for not attached

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
                attached,
                str(container["ports"]["jekyll"]),
                str(container["ports"]["livereload"]),
                last_used,
            )

        console.print(table)

    def attach_container(self, container_name: str = None):
        """Attach to an existing container with optional selection menu"""
        # If no container name provided, show selection menu
        if not container_name:
            container_name = self.select_container_with_fzf()
            if not container_name:
                return  # User cancelled selection

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

            # If stopped, we need to restart it interactively
            if container.status != "running":
                console.print("[yellow]⚠ Container is stopped, restarting...[/yellow]")

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

                console.print(
                    f"\n[green]🐳 Restarting container: {container_name}[/green]"
                )
                console.print("[dim]Use Ctrl+b d to detach from tmux[/dim]\n")

                # Set terminal window title
                print(f"\033]0;{container_name}\007", end="", flush=True)

                # Start the container in background
                subprocess.run(["docker", "start", container_name])

                # Small delay to ensure container is fully started
                time.sleep(0.5)

                # Check if tmux session exists, create if not
                check_session = subprocess.run(
                    [
                        "docker",
                        "exec",
                        container_name,
                        "/home/linuxbrew/.linuxbrew/bin/tmux",
                        "has-session",
                        "-t",
                        "main",
                    ],
                    capture_output=True,
                )

                if check_session.returncode != 0:
                    # No session exists, create one
                    subprocess.run(
                        [
                            "docker",
                            "exec",
                            "-d",
                            container_name,
                            "/home/linuxbrew/.linuxbrew/bin/tmux",
                            "new-session",
                            "-d",
                            "-s",
                            "main",
                            "/home/linuxbrew/.linuxbrew/bin/zsh",
                        ]
                    )
                    # Small delay to ensure tmux session is fully initialized
                    time.sleep(0.5)

                # Attach to tmux session
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
            else:
                # Container is running, attach to existing tmux session
                self.state.update_last_used(container_name)

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

                console.print(
                    f"\n[green]🐳 Attaching to running container: {container_name}[/green]\n"
                )

                # Set terminal window title
                print(f"\033]0;{container_name}\007", end="", flush=True)

                # Check if tmux session exists, create if not
                check_session = subprocess.run(
                    [
                        "docker",
                        "exec",
                        container_name,
                        "/home/linuxbrew/.linuxbrew/bin/tmux",
                        "has-session",
                        "-t",
                        "main",
                    ],
                    capture_output=True,
                )

                if check_session.returncode != 0:
                    # No session exists, create one
                    subprocess.run(
                        [
                            "docker",
                            "exec",
                            "-d",
                            container_name,
                            "/home/linuxbrew/.linuxbrew/bin/tmux",
                            "new-session",
                            "-d",
                            "-s",
                            "main",
                            "/home/linuxbrew/.linuxbrew/bin/zsh",
                        ]
                    )
                    # Small delay to ensure tmux session is fully initialized
                    time.sleep(0.5)

                # Now attach to tmux session
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

        # Run container in detached mode first, then attach
        cmd = ["docker", "run", "-d", "-t", "--name", container_name, "--hostname", container_name]

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
        # Start with a shell command that creates tmux session and keeps container running
        cmd.extend(
            [
                image,
                "sh",
                "-c",
                "/home/linuxbrew/.linuxbrew/bin/tmux new-session -d -s main /home/linuxbrew/.linuxbrew/bin/zsh && tail -f /dev/null",
            ]
        )

        # Display port info before starting
        console.print(
            Panel(
                f"[green]Jekyll:[/green] http://localhost:{jekyll_port}\n"
                f"[green]LiveReload:[/green] {livereload_port}",
                title="Container Ports",
                border_style="green",
            )
        )

        console.print(f"\n[green]🐳 Creating container: {container_name}[/green]")

        self.state.update_last_used(container_name)

        # Create the container in detached mode
        result = subprocess.run(cmd, capture_output=True, text=True)

        if result.returncode != 0:
            console.print(f"[red]❌ Failed to create container: {result.stderr}[/red]")
            self.state.remove_container(container_name)
            return

        console.print("[green]✓ Container created successfully[/green]")

        # Small delay to ensure container and tmux are fully initialized
        time.sleep(0.5)

        # Now attach to the tmux session in the container
        console.print(
            f"\n[green]🐳 Attaching to tmux session in container: {container_name}[/green]\n"
        )

        # Set terminal window title
        print(f"\033]0;{container_name}\007", end="", flush=True)

        # Attach to the tmux session that's already running
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
            # Use the new attach method with picker
            manager.attach_container()
            break
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
def new(image: str = typer.Argument(DEFAULT_IMAGE, help="Docker image to use")):
    """Create a new container (alias for create)"""
    create(image)


@app.command()
def attach(
    name: str = typer.Argument(..., help="Container name to attach to"),
):
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
def start_all():
    """Start all stopped containers without attaching"""
    manager = DockerManager()
    containers = manager.state.list_containers()

    if not containers:
        console.print("[yellow]No containers found[/yellow]")
        return

    restarted = 0
    already_running = 0
    failed = 0

    for container_info in containers:
        container_name = container_info["name"]
        status = manager.get_container_status(container_name)

        if status == "running":
            console.print(f"[dim]● {container_name} already running[/dim]")
            already_running += 1
            continue
        elif status == "not found":
            console.print(f"[yellow]⚠ {container_name} not found, skipping[/yellow]")
            manager.state.remove_container(container_name)
            failed += 1
            continue

        # Container is stopped, restart it
        try:
            result = subprocess.run(
                ["docker", "start", container_name],
                capture_output=True,
                text=True,
            )
            if result.returncode == 0:
                console.print(f"[green]✓ Started {container_name}[/green]")
                manager.state.update_last_used(container_name)
                restarted += 1
            else:
                console.print(f"[red]❌ Failed to start {container_name}: {result.stderr}[/red]")
                failed += 1
        except Exception as e:
            console.print(f"[red]❌ Error starting {container_name}: {e}[/red]")
            failed += 1

    # Summary
    console.print(
        f"\n[bold]Summary:[/bold] {restarted} started, {already_running} already running, {failed} failed"
    )


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

    # Claude runs in YOLO mode in container - no host credentials needed

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
