#!/usr/bin/env uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer[all]",
#     "rich",
#     "pyperclip",
#     "requests",
#     "pydantic",
# ]
# ///

import sys
import json
import os
from pathlib import Path
import hashlib
import pickle
import time
from typing import List


# Lazy loaded imports for full functionality
def load_full_imports():
    global typer, print, Annotated, pyperclip, requests, BaseModel

    import typer
    from typing_extensions import Annotated
    from rich import print
    import pyperclip
    import requests
    from pydantic import BaseModel

    return typer.Typer(
        help="AI-powered clipboard text processing using GroqCloud",
        add_completion=False,
        no_args_is_help=True,
    )


def get_script_hash():
    """Calculate hash of the current script file to detect changes."""
    script_path = Path(__file__)
    return hashlib.md5(script_path.read_bytes()).hexdigest()


def get_cache_path():
    """Get path to cache directory and ensure it exists."""
    cache_dir = Path.home() / "tmp" / ".cache" / "ai_clip_script"
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir / "alfred_commands.cache"


def load_cached_commands():
    """Load commands from cache if valid."""
    cache_path = get_cache_path()
    if not cache_path.exists():
        print("Cache Status: Miss (No Cache)", file=sys.stderr)
        return None

    try:
        with open(cache_path, "rb") as f:
            cached_data = pickle.load(f)
    except Exception as e:
        print(f"Cache Status: Error ({str(e)})", file=sys.stderr)
        return None

    if cached_data["hash"] != get_script_hash():
        print("Cache Status: Invalid (Script Changed)", file=sys.stderr)
        return None

    print("Cache Status: Hit", file=sys.stderr)
    return cached_data["commands"]


# Early exit for alfred command
if len(sys.argv) >= 2 and sys.argv[1] == "alfred":
    # Handle alfred command
    cached_result = load_cached_commands()
    if cached_result:
        print(cached_result)
        sys.exit(0)
    print("Cache Status: Miss", file=sys.stderr)
    app = load_full_imports()
else:
    # Not an alfred command, load full imports
    app = load_full_imports()


class AlfredItems(BaseModel):
    class Item(BaseModel):
        title: str
        subtitle: str
        arg: str

    items: List[Item]


def save_commands_cache(commands):
    """Save commands to cache with current script hash."""
    cache_data = {
        "hash": get_script_hash(),
        "commands": commands,
        "timestamp": time.time(),
    }
    try:
        with open(get_cache_path(), "wb") as f:
            pickle.dump(cache_data, f)
    except Exception as e:
        print(f"Cache Save Error: {str(e)}", file=sys.stderr)


def get_clipboard_content() -> str:
    """Get text content from clipboard."""
    try:
        content = pyperclip.paste()
        if not content.strip():
            print("[red]Error: Clipboard is empty or contains no text[/red]")
            raise typer.Exit(code=1)
        return content
    except Exception as e:
        print(f"[red]Error reading clipboard: {e}[/red]")
        raise typer.Exit(code=1)


def set_clipboard_content(content: str):
    """Set text content to clipboard."""
    try:
        pyperclip.copy(content)
        print("[green]✓ Result copied to clipboard[/green]")
    except Exception as e:
        print(f"[red]Error copying to clipboard: {e}[/red]")
        raise typer.Exit(code=1)


def call_grok_maverick(prompt: str, content: str) -> str:
    """Call GroqCloud API with the given prompt and content."""
    api_key = os.getenv("GROQ_API_KEY")
    if not api_key:
        print("[red]Error: GROQ_API_KEY environment variable not set[/red]")
        print(
            "[yellow]Please set your Groq API key: export GROQ_API_KEY=your_key_here[/yellow]"
        )
        print("[dim]Get your API key from: https://console.groq.com/[/dim]")
        raise typer.Exit(code=1)

    try:
        headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        }

        payload = {
            "model": "meta-llama/llama-4-maverick-17b-128e-instruct",  # Llama 4 Maverick - Latest multimodal model
            "messages": [
                {"role": "system", "content": prompt},
                {"role": "user", "content": content},
            ],
            "max_tokens": 2000,
            "temperature": 0.3,
        }

        print("[dim]Calling GroqCloud API...[/dim]")
        response = requests.post(
            "https://api.groq.com/openai/v1/chat/completions",
            headers=headers,
            json=payload,
            timeout=30,
        )

        if response.status_code != 200:
            print(f"[red]API Error: {response.status_code} - {response.text}[/red]")
            raise typer.Exit(code=1)

        response_data = response.json()

        if "choices" not in response_data or not response_data["choices"]:
            print("[red]Error: No response from GroqCloud API[/red]")
            raise typer.Exit(code=1)

        result = response_data["choices"][0]["message"]["content"].strip()
        return result

    except requests.exceptions.RequestException as e:
        print(f"[red]Error calling GroqCloud API: {e}[/red]")
        raise typer.Exit(code=1)
    except json.JSONDecodeError as e:
        print(f"[red]Error parsing API response: {e}[/red]")
        raise typer.Exit(code=1)
    except Exception as e:
        print(f"[red]Unexpected error: {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def fix():
    """Fix spelling and punctuation in clipboard content using GroqCloud"""
    print("[cyan]🔧 Fixing spelling and punctuation...[/cyan]")

    content = get_clipboard_content()
    print(f"[dim]Original content ({len(content)} chars):[/dim]")
    print(f"[dim]{content[:100]}{'...' if len(content) > 100 else ''}[/dim]")

    prompt = """You are a copy editor. Fix any spelling errors, punctuation mistakes, and grammatical issues in the following text. Keep the original meaning and tone intact. Return only the corrected text without any additional commentary or explanations."""

    try:
        result = call_grok_maverick(prompt, content)
        print(f"\n[green]✓ Fixed text ({len(result)} chars):[/green]")
        print(result)

        set_clipboard_content(result)

    except Exception as e:
        print(f"[red]Failed to process text: {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def seuss():
    """Transform clipboard content into Dr. Seuss style using GroqCloud"""
    print("[cyan]🎭 Transforming to Dr. Seuss style...[/cyan]")

    content = get_clipboard_content()
    print(f"[dim]Original content ({len(content)} chars):[/dim]")
    print(f"[dim]{content[:100]}{'...' if len(content) > 100 else ''}[/dim]")

    prompt = """Transform the following text into the whimsical, rhyming style of Dr. Seuss. Use his characteristic rhythm, playful language, and creative wordplay. Keep the core message but make it fun and bouncy like his children's books. Return only the transformed text."""

    try:
        result = call_grok_maverick(prompt, content)
        print(f"\n[green]✓ Seussified text ({len(result)} chars):[/green]")
        print(result)

        set_clipboard_content(result)

    except Exception as e:
        print(f"[red]Failed to process text: {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def custom(
    prompt: Annotated[
        str, typer.Argument(help="Custom prompt for text transformation")
    ],
):
    """Apply custom transformation to clipboard content using GroqCloud"""
    print(f"[cyan]🤖 Applying custom transformation: {prompt}[/cyan]")

    content = get_clipboard_content()
    print(f"[dim]Original content ({len(content)} chars):[/dim]")
    print(f"[dim]{content[:100]}{'...' if len(content) > 100 else ''}[/dim]")

    full_prompt = (
        f"{prompt}\n\nReturn only the transformed text without additional commentary."
    )

    try:
        result = call_grok_maverick(full_prompt, content)
        print(f"\n[green]✓ Transformed text ({len(result)} chars):[/green]")
        print(result)

        set_clipboard_content(result)

    except Exception as e:
        print(f"[red]Failed to process text: {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def summarize():
    """Summarize clipboard content using GroqCloud"""
    print("[cyan]📝 Summarizing content...[/cyan]")

    content = get_clipboard_content()
    print(f"[dim]Original content ({len(content)} chars):[/dim]")
    print(f"[dim]{content[:100]}{'...' if len(content) > 100 else ''}[/dim]")

    prompt = """Provide a clear, concise summary of the following text. Capture the main points and key information while being much shorter than the original. Return only the summary."""

    try:
        result = call_grok_maverick(prompt, content)
        print(f"\n[green]✓ Summary ({len(result)} chars):[/green]")
        print(result)

        set_clipboard_content(result)

    except Exception as e:
        print(f"[red]Failed to process text: {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def explain():
    """Explain clipboard content in simple terms using GroqCloud"""
    print("[cyan]💡 Explaining content...[/cyan]")

    content = get_clipboard_content()
    print(f"[dim]Original content ({len(content)} chars):[/dim]")
    print(f"[dim]{content[:100]}{'...' if len(content) > 100 else ''}[/dim]")

    prompt = """Explain the following text in simple, clear terms that anyone can understand. Break down complex concepts and provide context where helpful. Return only the explanation."""

    try:
        result = call_grok_maverick(prompt, content)
        print(f"\n[green]✓ Explanation ({len(result)} chars):[/green]")
        print(result)

        set_clipboard_content(result)

    except Exception as e:
        print(f"[red]Failed to process text: {e}[/red]")
        raise typer.Exit(code=1)


@app.command()
def alfred():
    """Generate JSON output of all commands for Alfred workflow integration"""
    # Try to load from cache first
    cached_commands = load_cached_commands()
    if cached_commands:
        print(cached_commands)
        return

    print("Cache Status: Miss", file=sys.stderr)

    # If no valid cache, generate commands
    commands = [
        c.callback.__name__.replace("_", "-")
        for c in app.registered_commands
        if c.callback.__name__ != "alfred"
    ]  # type:ignore

    command_descriptions = {
        "fix": "Fix spelling and punctuation",
        "seuss": "Transform to Dr. Seuss style",
        "custom": "Apply custom transformation",
        "summarize": "Summarize content",
        "explain": "Explain in simple terms",
    }

    items = [
        AlfredItems.Item(title=c, subtitle=command_descriptions.get(c, c), arg=c)
        for c in commands
    ]
    alfred_items = AlfredItems(items=items)
    json_output = alfred_items.model_dump_json(indent=4)

    # Save to cache for future use
    save_commands_cache(json_output)

    print(json_output)


if __name__ == "__main__":
    app()
