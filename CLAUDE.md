# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview
This is Igor Dvorkin's personal settings/dotfiles repository - a comprehensive configuration management system for multiple platforms (macOS, Linux/Alpine, Windows/WSL) and development tools. It includes Python CLI utilities, editor configurations, and workflow automation patterns.

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