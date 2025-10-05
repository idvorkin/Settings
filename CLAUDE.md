# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview
This is Igor Dvorkin's personal settings/dotfiles repository - a comprehensive configuration management system for multiple platforms (macOS, Linux/Alpine, Windows/WSL) and development tools. It includes Python CLI utilities, editor configurations, and workflow automation patterns.

## Setup Instructions

### macOS Setup
Run the bootstrap script to install all dependencies:
```bash
./bootstrap.sh
```

Key installation files:
- **Brew packages**: `shared/brew_packages.sh` - Contains all Homebrew packages including development tools, LSPs, and utilities
- **Mac-specific setup**: `mac/install.sh` - macOS-specific configurations and cask applications
- **Shared setup**: `shared/shared_install.sh` - Cross-platform configurations

### Homebrew Package Management
- **Non-cask packages**: Add to `shared/brew_packages.sh` using format `brew_packages="$brew_packages package_name"`
- **Cask applications**: Add to `mac/install.sh` using format `brew install --cask app-name`
- See `.cursor/rules/104-brew-packages.mdc` for detailed package management guidelines

### Required Packages
Essential packages are installed via Homebrew from `shared/brew_packages.sh`:
- Development tools: `git`, `tmux`, `zsh`, `neovim`
- Language servers: `lua-language-server`, `typos-lsp`
- Python tools: `uv`, `pipx`, `ruff`
- CLI utilities: `bat`, `eza`, `fzf`, `ripgrep`, `fd`
- Git enhancements: `gh`, `lazygit`, `git-delta`

### Python Environment Setup
Python tools are managed via UV for speed and consistency:
```bash
# Install UV and pipx
brew install uv pipx

# Install Python development tools
uv tool install --force ruff
uv tool install --force black
uv tool install --force mypy
```

## Python Development Conventions

### UV Shebang Usage
All Python scripts should use UV shebangs for easy deployment:
```python
#!/usr/bin/env uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "pydantic"]
# ///
```

### CLI Framework
Use Typer for all command-line interfaces:
```python
import typer
app = typer.Typer(
    no_args_is_help=True,
    pretty_exceptions_enable=False,
    add_completion=False
)
```

### Type Annotations
Always use type annotations with Python 3.13+ style:
```python
def process_items(items: list[str]) -> dict[str, int]:
    return {item: len(item) for item in items}
```

### Data Validation
Use Pydantic for configuration and data validation:
```python
from pydantic import BaseModel, Field

class Config(BaseModel):
    api_key: str = Field(..., description="API key for service")
    timeout: int = Field(30, description="Timeout in seconds")
```

### Terminal Output
Use Rich for enhanced terminal output:
```python
from rich.console import Console
from rich.progress import track

console = Console()
console.print("[green]Success![/green] Operation completed")
```

### Testing Organization
- Test files mirror source structure: `py/foo.py` → `py/test_foo.py`
- Use pytest fixtures for shared test data
- Mock external dependencies
- Test both success and error cases

## Code Quality Standards

### Clean Code Principles
- **DRY (Don't Repeat Yourself)**: Extract common logic into functions
- **Early Returns**: Exit functions early to reduce nesting
- **Minimize Nesting**: Keep nesting under 3 levels
- **Use Const and Types**: Leverage type system for safety
- **Humble Objects**: Keep objects simple with single responsibilities

### Pre-commit Hooks
The repository uses these linters (configured in .pre-commit-config.yaml):
- **Python**: Ruff (linting and formatting)
- **JavaScript/TypeScript**: Biome
- **Markdown**: Prettier
- **Lua**: StyLua
- **YAML/JSON**: Dasel validation

## Git Workflow

### Commit Practices
- **Clean Commits**: Each commit should be atomic and complete
- **Logical Separation**: Separate functional changes from formatting
- **Descriptive Messages**: Use conventional commit format when applicable
- **No Merge Commits**: Use rebase for clean history
- **Explicit Staging**: NEVER use `git add -A` or `git add .` - always add files explicitly by name

### Pull Request Workflow
1. Create issue first using `gh issue create`
2. Create branch from issue
3. Make changes following conventions
4. Create PR using `gh pr create`
5. Link PR to issue

## Terminal Command Conventions

### Important Usage Notes
- Use `/bin/cat` when encountering pager issues with commands
- Run Python scripts directly via shebang: `./script.py` (not `python script.py`)
- For UV-managed scripts: `uv run script.py`
- Use `&` for background processes to avoid blocking

## Development Commands

### Common Tasks
```bash
# Run tests
just test

# Install/update dependencies
uv sync

# Run pre-commit checks
pre-commit run --all-files

# Bootstrap settings (platform-specific)
./bootstrap.sh
```

### Adding Tmux Commands
To add new tmux commands via `py/tmux_helper.py`:

1. **Add command function to `py/tmux_helper.py`**:
   ```python
   @app.command()
   def command_name():
       """Description of what the command does"""
       # Use helper functions for tmux commands
       result = run_tmux_command(["tmux", "some-command"], capture_output=True)
       if result:
           # Process result
           run_tmux_command(["tmux", "other-command"])
   ```

   **Available helper functions**:
   - `run_tmux_command(cmd, capture_output=False)` - Run tmux command with error handling
   - `get_tmux_option(option)` - Get global tmux option value
   - `set_tmux_option(option, value)` - Set global tmux option
   - `ensure_two_panes()` - Ensure at least 2 panes exist, returns `(pane_list, created_new_pane)`
   - `get_layout_orientation()` - Get current layout orientation (horizontal/vertical)
   - `process_tree_has_pattern(process_info, patterns)` - Check if pattern exists in process tree

2. **Test the command**: Run `./py/tmux_helper.py command_name` directly and verify behavior with tmux commands like `tmux list-panes`

3. **Add to `shared/.tmux.conf`**:

   a. **Add to help section** (top of file, around lines 7-22):
   ```tmux
   # Keybindings:
   #   C-a <key>            - Description of what it does
   #
   # Command Aliases (use via C-a : then type command):
   #   :command_name        - Description of what it does
   ```

   b. **Add keybinding** (optional, for quick access):
   ```tmux
   bind-key <key> run-shell "tmux_helper command_name"
   ```

   c. **Add command alias** (in the command aliases section, around line 214-217):
   ```tmux
   set -s command-alias[10X] command_name='run-shell "tmux_helper command_name"'
   ```
   (Increment the number to avoid conflicts)

4. **Reload tmux config**: Press `Prefix + r` or run `tmux source-file ~/.tmux.conf`

**Example**: The `third` command toggles between even and 1/3-2/3 layouts:
- Function at `py/tmux_helper.py:413`
- Keybinding at `shared/.tmux.conf:207` (`C-a /`)
- Command alias at `shared/.tmux.conf:217` (`:third`)
- Help documentation at `shared/.tmux.conf:10-21`

## File Organization

### Python Utilities
Located in `py/` directory, each with UV shebang and Typer CLI:
- `ai_clip.py` - AI clipboard processing
- `gmail_to_todoist.py` - Gmail integration
- `tmux_helper.py` - Tmux utilities
- `gpt.py` - OpenAI API wrapper

### Conventions Directory
The `zz-chop-conventions/` directory contains shared conventions that should be followed across all projects.

## Important Warnings
- NEVER create files unless absolutely necessary
- ALWAYS prefer editing existing files
- NEVER proactively create documentation files (*.md) unless explicitly requested
- Follow existing patterns in the codebase rather than introducing new ones

## Architecture Overview
This repository implements infrastructure-as-code principles with:
- Platform-specific configurations in dedicated directories (`/mac`, `/windows`, `/alpine`)
- Shared configurations in `/shared` (git, zsh, tmux, ssh)
- Application configs in `/config` (cursor, bat, yazi, mpv, etc.)
- Neovim configuration in `/nvim` with Lua-based plugin management
- Workflow templates and AI rules in `/xnotes`