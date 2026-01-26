#!/opt/homebrew/bin/uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
#     "rich",
#     "icecream",
#     "pydantic",
#     "pyperclip",
#     "pyobjc-framework-Quartz",
#     "pyobjc-framework-Cocoa",
#     "pillow",
#     "psutil",
# ]
# ///

# Flow Help Page: https://www.flow.app/help#documentation

import sys
import json
import os
from pathlib import Path
import hashlib
import pickle


# Lazy loaded imports for full functionality
def load_full_imports():
    global typer, print, subprocess, CompletedProcess, ic, List, BaseModel, Field
    global pyperclip, Quartz, CG, AppKit, math, datetime, time, Annotated, psutil

    from datetime import datetime
    import time
    import typer
    from typing_extensions import Annotated
    from rich import print
    import subprocess
    from subprocess import CompletedProcess
    from icecream import ic
    from typing import List
    from pydantic import BaseModel, Field
    import pyperclip
    import math
    import psutil

    try:
        import Quartz
        import Quartz.CoreGraphics as CG
        import AppKit
    except ImportError:
        # These are optional, only needed for screenshot/windowing features not core to all commands
        # print("Warning: pyobjc modules (Quartz, AppKit) not found. Some features might be unavailable.", file=sys.stderr)
        Quartz = None
        CG = None
        AppKit = None

    return typer.Typer(
        help="A Yabai helper - Window management and screenshot utilities",
        no_args_is_help=True,
    )


def get_script_hash():
    """Calculate hash of the current script file to detect changes."""
    script_path = Path(__file__)
    return hashlib.md5(script_path.read_bytes()).hexdigest()


def get_cache_path():
    """Get path to cache directory and ensure it exists."""
    cache_dir = Path.home() / "tmp" / ".cache" / "y_script"
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


# Parameter completions for commands that support them (defined early for fast path)
PARAM_COMPLETIONS: dict[str, dict[str, list[str]]] = {
    "p_foo": {
        "color": ["red", "green", "blue", "yellow", "purple"],
        "size": ["small", "medium", "large", "xlarge"],
    },
    "p_bar": {
        "fruit": ["apple", "banana", "cherry", "date"],
        "count": ["1", "2", "3", "5", "10"],
    },
}

# Commands with dynamic completions (queried at runtime)
DYNAMIC_COMPLETION_COMMANDS = {"ter"}


def _get_iterm_tabs() -> list[tuple[str, str, str]]:
    """Get iTerm2 tabs via AppleScript. Returns list of (title, 'iTerm2 tab', 'iterm:win:tab:session')."""
    import subprocess as sp

    script = """
    tell application "iTerm2"
        set output to ""
        set winIdx to 0
        repeat with w in windows
            set winIdx to winIdx + 1
            set tabIdx to 0
            repeat with t in tabs of w
                set tabIdx to tabIdx + 1
                set sessIdx to 0
                repeat with s in sessions of t
                    set sessIdx to sessIdx + 1
                    set output to output & (name of s) & "|||" & winIdx & ":" & tabIdx & ":" & sessIdx & "\\n"
                end repeat
            end repeat
        end repeat
        return output
    end tell
    """
    try:
        result = sp.run(
            ["osascript", "-e", script],
            capture_output=True,
            text=True,
            timeout=3,
        )
        if result.returncode != 0:
            return []

        tabs = []
        for line in result.stdout.strip().split("\n"):
            if "|||" in line:
                title, coords = line.split("|||", 1)
                # coords is "win:tab:session"
                tabs.append((title.strip(), "iTerm2 tab", f"iterm:{coords.strip()}"))
        return tabs
    except Exception:
        return []


def _get_ghostty_tabs() -> list[tuple[str, str, str]]:
    """Get Ghostty tabs via Window menu. Returns list of (title, 'Ghostty tab', 'ghostty:title')."""
    import subprocess as sp

    script = """
    tell application "System Events"
        tell process "Ghostty"
            get name of every menu item of menu "Window" of menu bar 1
        end tell
    end tell
    """
    try:
        result = sp.run(
            ["osascript", "-e", script],
            capture_output=True,
            text=True,
            timeout=3,
        )
        if result.returncode != 0:
            return []

        tabs = []
        # Parse the comma-separated list from AppleScript
        items = result.stdout.strip().split(", ")
        for item in items:
            # Tab entries look like terminal titles, skip menu commands
            # Menu commands are things like "Minimize", "Zoom", etc.
            skip_items = {
                "Minimize",
                "Minimize All",
                "Zoom",
                "Zoom All",
                "Fill",
                "Center",
                "missing value",
                "Move & Resize",
                "Full Screen Tile",
                "Remove Window from Set",
                "Toggle Full Screen",
                "Show/Hide All Terminals",
                "Show Previous Tab",
                "Show Next Tab",
                "Move Tab to New Window",
                "Merge All Windows",
                "Zoom Split",
                "Select Previous Split",
                "Select Next Split",
                "Select Split",
                "Resize Split",
                "Return To Default Size",
                "Float on Top",
                "Use as Default",
                "Bring All to Front",
                "Arrange in Front",
            }
            if item not in skip_items and item:
                tabs.append((item, "Ghostty tab", f"ghostty:{item}"))
        return tabs
    except Exception:
        return []


def _get_terminal_windows_for_completion() -> list[tuple[str, str, str]]:
    """Get terminal windows/tabs for completion. Returns list of (title, description, identifier)."""
    import subprocess as sp

    results = []

    # Get iTerm2 tabs (more granular than yabai windows)
    results.extend(_get_iterm_tabs())

    # Get Ghostty tabs from Window menu
    results.extend(_get_ghostty_tabs())

    # Get other terminal windows from yabai (Terminal, Warp, etc. - not Ghostty/iTerm)
    terminal_apps_yabai = ["Terminal", "Warp", "Alacritty", "kitty"]
    try:
        result = sp.run(
            ["yabai", "-m", "query", "--windows"],
            capture_output=True,
            text=True,
            timeout=2,
        )
        if result.returncode == 0:
            import json as j

            windows = j.loads(result.stdout)
            for win in windows:
                app = win.get("app", "")
                if app in terminal_apps_yabai and not win.get("is-minimized", False):
                    title = win.get("title", "")
                    win_id = win.get("id", 0)
                    display_title = title[:50] + "..." if len(title) > 50 else title
                    results.append((display_title, f"{app} window", f"yabai:{win_id}"))
    except Exception:
        pass

    return results


def get_cached_commands_list() -> list[tuple[str, str]] | None:
    """Load command list from cache if valid. Returns list of (name, subtitle) tuples."""
    cache_path = get_cache_path()
    if not cache_path.exists():
        return None
    try:
        with open(cache_path, "rb") as f:
            cached_data = pickle.load(f)
        if cached_data["hash"] != get_script_hash():
            return None
        # Parse the cached JSON to extract command list
        import json as json_mod

        data = json_mod.loads(cached_data["commands"])
        return [(item["title"], item["subtitle"]) for item in data["items"]]
    except Exception:
        return None


def _make_item(
    title: str, subtitle: str, arg: str, autocomplete: str | None = None
) -> dict:
    """Build an Alfred item dict, excluding None values."""
    item = {"title": title, "subtitle": subtitle, "arg": arg}
    if autocomplete is not None:
        item["autocomplete"] = autocomplete
    return item


def fast_alfred_complete(query: str) -> str:
    """Fast path for alfred-complete using cached commands."""
    has_trailing_space = query.endswith(" ")
    query_stripped = query.strip()
    parts = query_stripped.split() if query_stripped else []

    # Handle dynamic completions first (before cache check)
    # These need to run even without cache since they query live data
    if len(parts) >= 1:
        cmd = parts[0].replace("-", "_")
        if cmd in DYNAMIC_COMPLETION_COMMANDS:
            items = []
            param_values = parts[1:] if len(parts) > 1 else []

            if cmd == "ter":
                terminal_windows = _get_terminal_windows_for_completion()
                filter_text = (
                    param_values[0].lower()
                    if param_values and not has_trailing_space
                    else ""
                )

                for title, description, identifier in terminal_windows:
                    if not filter_text or filter_text in title.lower():
                        arg = f"ter {identifier}"
                        items.append(
                            _make_item(
                                title,
                                description,
                                arg,
                                None,
                            )
                        )

                if not items:
                    items.append(
                        _make_item(
                            "No terminal windows found", "Try opening a terminal", "ter"
                        )
                    )

            return json.dumps({"items": items}, indent=2)

    # Now check cache for regular completions
    commands = get_cached_commands_list()
    if commands is None:
        return None  # Fall back to slow path

    items = []

    def has_completions(cmd_key: str) -> bool:
        return cmd_key in PARAM_COMPLETIONS or cmd_key in DYNAMIC_COMPLETION_COMMANDS

    if len(parts) == 0:
        # Show all commands - all get autocomplete (with space if has params, without if not)
        for name, subtitle in commands:
            cmd_key = name.replace("-", "_")
            autocomplete = f"{name} " if has_completions(cmd_key) else name
            items.append(_make_item(name, subtitle, name, autocomplete))
    elif len(parts) == 1 and not has_trailing_space:
        # Partial command - filter matching commands
        prefix = parts[0].lower()
        for name, subtitle in commands:
            if name.lower().startswith(prefix) or prefix in name.lower():
                cmd_key = name.replace("-", "_")
                autocomplete = f"{name} " if has_completions(cmd_key) else name
                items.append(_make_item(name, subtitle, name, autocomplete))
    else:
        # Command entered, show parameter completions
        cmd = parts[0].replace("-", "_")
        param_values = parts[1:] if len(parts) > 1 else []

        # Dynamic completions are handled at the top of the function
        # so we only handle static PARAM_COMPLETIONS here
        if cmd in PARAM_COMPLETIONS:
            param_names = list(PARAM_COMPLETIONS[cmd].keys())

            if has_trailing_space:
                current_param_idx = len(param_values)
            else:
                current_param_idx = max(0, len(param_values) - 1)

            if current_param_idx < len(param_names):
                param_name = param_names[current_param_idx]
                options = PARAM_COMPLETIONS[cmd][param_name]

                filter_text = ""
                if not has_trailing_space and len(param_values) > current_param_idx:
                    filter_text = param_values[current_param_idx].lower()

                for option in options:
                    if not filter_text or option.lower().startswith(filter_text):
                        cmd_display = cmd.replace("_", "-")
                        full_args = param_values[:current_param_idx] + [option]
                        arg = f"{cmd_display} {' '.join(full_args)}"

                        next_param_idx = current_param_idx + 1
                        autocomplete = (
                            f"{arg} " if next_param_idx < len(param_names) else None
                        )

                        items.append(
                            _make_item(
                                option,
                                f"{param_name} for {cmd_display}",
                                arg,
                                autocomplete,
                            )
                        )
            else:
                cmd_display = cmd.replace("_", "-")
                arg = f"{cmd_display} {' '.join(param_values)}"
                items.append(_make_item(f"Run: {arg}", "Press Enter to execute", arg))
        else:
            cmd_display = cmd.replace("_", "-")
            arg = query_stripped
            items.append(_make_item(f"Run: {arg}", "Press Enter to execute", arg))

    return json.dumps({"items": items}, indent=2)


# Early exit for alfred commands
if len(sys.argv) >= 2 and sys.argv[1] == "alfred":
    # Handle alfred command
    cached_result = load_cached_commands()
    if cached_result:
        print(cached_result)
        sys.exit(0)
    print("Cache Status: Miss", file=sys.stderr)
    app = load_full_imports()
elif len(sys.argv) >= 2 and sys.argv[1] == "alfred-complete":
    # Handle alfred-complete command (fast path)
    query = sys.argv[2] if len(sys.argv) > 2 else ""
    result = fast_alfred_complete(query)
    if result:
        print(result)
        sys.exit(0)
    # Fall back to slow path if cache miss
    print("Cache Status: Miss (alfred-complete)", file=sys.stderr)
    app = load_full_imports()
else:
    # Not an alfred command, load full imports
    app = load_full_imports()

FLOW_HELP_URL = "https://www.flow.app/help#documentation"

# Python tools path
PY_TOOLS_PATH = Path.home() / ".local" / "bin"

_ = """

~/settings/config/yabai/yabairc

"""


def ensure_directory_exists(path):
    """Create a directory if it doesn't exist."""
    os.makedirs(path, exist_ok=True)


# AI Coding pro-tip, pump json into AI, and ask it to generate the pydantic types for you
# Then use typing to avoid the long pain of type errors


class Frame(BaseModel):
    x: float
    y: float
    w: float
    h: float


class Window(BaseModel):
    id: int
    pid: int
    app: str
    title: str
    scratchpad: str
    frame: Frame
    role: str = Field(default="")
    subrole: str = Field(default="")
    root_window: bool = Field(alias="root-window")
    display: int
    space: int
    level: int
    sub_level: int = Field(alias="sub-level")
    layer: str
    sub_layer: str = Field(alias="sub-layer")
    opacity: float
    split_type: str = Field(alias="split-type")
    split_child: str = Field(alias="split-child")
    stack_index: int = Field(alias="stack-index")
    can_move: bool = Field(alias="can-move")
    can_resize: bool = Field(alias="can-resize")
    has_focus: bool = Field(alias="has-focus")
    has_shadow: bool = Field(alias="has-shadow")
    has_parent_zoom: bool = Field(alias="has-parent-zoom")
    has_fullscreen_zoom: bool = Field(alias="has-fullscreen-zoom")
    has_ax_reference: bool = Field(alias="has-ax-reference")
    is_native_fullscreen: bool = Field(alias="is-native-fullscreen")
    is_visible: bool = Field(alias="is-visible")
    is_minimized: bool = Field(alias="is-minimized")
    is_hidden: bool = Field(alias="is-hidden")
    is_floating: bool = Field(alias="is-floating")
    is_sticky: bool = Field(alias="is-sticky")
    is_grabbed: bool = Field(alias="is-grabbed")


class Windows(BaseModel):
    windows: List[Window]


class Display(BaseModel):
    id: int
    uuid: str
    index: int
    label: str
    frame: Frame
    spaces: List[int]
    has_focus: bool = Field(alias="has-focus")


class Displays(BaseModel):
    displays: List[Display]


def call_yabai(prompt) -> CompletedProcess:
    yabi_root = Path("~/homebrew/bin/yabai/").expanduser()

    # Split the prompt to run the path
    prompt_parts = prompt.split()
    command = [str(yabi_root)] + prompt_parts

    # Run the command in the remote shell
    try:
        out = subprocess.run(command, check=True, capture_output=True, text=True)
        return out
    except subprocess.CalledProcessError as e:
        print(f"An error occurred: {e.stderr}")
        raise e


def send_key(key_code):
    # https://superuser.com/questions/368026/can-i-use-a-terminal-command-to-switch-to-a-specific-space-in-os-x-10-6
    out = subprocess.run(
        [
            "osascript",
            "-e",
            f'tell application "System Events" to key code {key_code} using control down',
        ]
    )
    ic(out)


@app.command()
def hflip():
    """Flip the current space horizontally along the y-axis"""
    call_yabai("-m space --mirror y-axis")


# Constants
MIN_SPACES_FOR_SWITCHING = 1
MIN_DISPLAYS_FOR_SWITCHING = 1


def _switch_space_on_display(direction: int):
    """Helper function to switch spaces on current display. Direction: -1 for west, 1 for east."""
    current_display = get_active_display()
    spaces_on_display = [s for s in get_spaces() if s["display"] == current_display.id]

    if len(spaces_on_display) <= MIN_SPACES_FOR_SWITCHING:
        return  # No point in switching if only one space

    # Sort spaces by index to ensure proper order
    spaces_on_display.sort(key=lambda s: s["index"])

    current_space = get_current_space()
    current_index = next(
        i for i, s in enumerate(spaces_on_display) if s["id"] == current_space["id"]
    )
    target_index = (current_index + direction) % len(spaces_on_display)
    target_space_id = spaces_on_display[target_index]["id"]

    call_yabai(f"-m space --focus {target_space_id}")


@app.command()
def swest():
    """Switch to the space to the west (left) of current space (cycles around)"""
    _switch_space_on_display(-1)


@app.command()
def seast():
    """Switch to the space to the east (right) of current space (cycles around)"""
    _switch_space_on_display(1)


@app.command()
def fleft():
    """Focus the window to the left of the current window"""
    call_yabai("-m window --focus west")


@app.command()
def fup():
    """Focus the window above the current window"""
    call_yabai("-m window --focus north")


@app.command()
def fdown():
    """Focus the window below the current window"""
    call_yabai("-m window --focus south")


@app.command()
def fright():
    """Focus the window to the right of the current window"""
    call_yabai("-m window --focus east")


@app.command()
def restart():
    call_yabai("--restart-service")


@app.command()
def reset():
    """Alias for restart"""
    restart()


@app.command()
def start():
    """Start the yabai window manager service"""
    call_yabai("--start-service")


@app.command()
def stop():
    """Stop the yabai window manager service"""
    call_yabai("--stop-service")


@app.command()
def rotate():
    """Rotate the current space layout by 90 degrees"""
    call_yabai("-m space --rotate 90")


@app.command()
def zoom():
    """Toggle fullscreen zoom for the focused window"""
    call_yabai("-m window --toggle zoom-fullscreen")


@app.command()
def fullscreen():
    """Toggle native macOS fullscreen mode for the focused window"""
    subprocess.run(
        [
            "osascript",
            "-e",
            'tell application "System Events" to key code 3 using {control down, command down}',
        ]
    )


@app.command()
def fs():
    """Alias for fullscreen - Toggle native macOS fullscreen mode"""
    fullscreen()


@app.command()
def close():
    """Close the currently focused window"""
    call_yabai("-m window --close")


@app.command()
def cycle():
    """Cycle through windows by repeatedly swapping with the previous window"""
    win_result = call_yabai("-m query --windows --window last")
    if win_result.returncode != 0:
        typer.echo("Failed to query yabai windows")
        raise typer.Exit(code=1)

    try:
        win_data = json.loads(win_result.stdout)
        win_id = win_data["id"]
    except (json.JSONDecodeError, KeyError) as e:
        typer.echo(f"Failed to parse window data: {e}")
        raise typer.Exit(code=1)

    while True:
        swap_result = call_yabai(f"-m window {win_id} --swap prev")
        if swap_result.returncode != 0:
            break


def _cycle_display(direction: int, action: str):
    """Helper function to cycle through displays. Direction: -1 for prev, 1 for next. Action: 'window' or 'display'."""
    displays = get_displays().displays
    if len(displays) <= MIN_DISPLAYS_FOR_SWITCHING:
        return  # No point in moving/switching if only one display

    current_display = get_active_display()
    current_index = next(
        i for i, d in enumerate(displays) if d.id == current_display.id
    )
    target_index = (current_index + direction) % len(displays)

    if action == "window":
        # For windows, use arrangement index (1-based)
        call_yabai(f"-m window --display {target_index + 1}")
    else:  # action == "display"
        # For display focus, use arrangement index (1-based)
        call_yabai(f"-m display --focus {target_index + 1}")


@app.command()
def wprev():
    """Move focused window to previous display (cycles around)"""
    _cycle_display(-1, "window")


@app.command()
def wnext():
    """Move focused window to next display (cycles around)"""
    _cycle_display(1, "window")


@app.command()
def wrecent():
    """Move focused window to most recently focused display"""
    call_yabai("-m window --display recent")


@app.command()
def dprev():
    """Focus previous display (cycles around)"""
    _cycle_display(-1, "display")


@app.command()
def dnext():
    """Focus next display (cycles around)"""
    _cycle_display(1, "display")


@app.command()
def drecent():
    """Focus most recently focused display"""
    call_yabai("-m display --focus recent")


@app.command()
def dlist():
    """List all displays with their IDs and focus status"""
    displays = get_displays().displays
    for i, display in enumerate(displays):
        focus_indicator = "🔸" if display.has_focus else "  "
        print(
            f"{focus_indicator} Display {i + 1}: ID={display.id}, {display.frame.w}x{display.frame.h}"
        )


@app.command()
def slist():
    """List all spaces on current display"""
    current_display = get_active_display()
    spaces_on_display = [s for s in get_spaces() if s["display"] == current_display.id]
    spaces_on_display.sort(key=lambda s: s["index"])

    current_space = get_current_space()
    for i, space in enumerate(spaces_on_display):
        focus_indicator = "🔸" if space["id"] == current_space["id"] else "  "
        space_type = space.get("type", "unknown")
        print(f"{focus_indicator} Space {i + 1}: ID={space['id']}, Type={space_type}")


@app.command()
def float():
    """Toggle float/tile mode for focused window"""
    call_yabai("-m window --toggle float")


@app.command()
def hide():
    """Minimize the currently focused window"""
    call_yabai("-m window --minimize")


def get_windows() -> Windows:
    win_result = call_yabai("-m query --windows")
    if win_result.returncode != 0:
        typer.echo("Failed to query yabai windows")
        raise typer.Exit(code=1)
    win_data = json.loads(win_result.stdout)
    windows = Windows(windows=win_data)
    return windows


def get_displays() -> Displays:
    disp_result = call_yabai("-m query --displays")
    if disp_result.returncode != 0:
        typer.echo("Failed to query yabai displays")
        raise typer.Exit(code=1)
    disp_data = json.loads(disp_result.stdout)
    displays = Displays(displays=disp_data)
    return displays


def set_width(win: Window, width: float):
    assert width >= 0 and width <= 1
    # call_yabai(f"-m window {win.id} --ratio abs:{width}")
    call_yabai(f"-m window --ratio abs:{width}")


def get_active_display():
    return [d for d in get_displays().displays if d.has_focus][0]


def get_spaces():
    """Get all spaces information"""
    spaces_result = call_yabai("-m query --spaces")
    if spaces_result.returncode != 0:
        typer.echo("Failed to query yabai spaces")
        raise typer.Exit(code=1)
    return json.loads(spaces_result.stdout)


def get_current_space():
    """Get the currently focused space"""
    spaces = get_spaces()
    return next(space for space in spaces if space.get("has-focus", False))


def get_left_most_window():
    return min(get_windows().windows, key=lambda w: w.frame.x)


@app.command()
def third():
    """Set the leftmost window width to 30% of the screen"""
    set_width(get_left_most_window(), 0.3)


@app.command()
def half():
    """Set the leftmost window width to 50% of the screen"""
    set_width(get_left_most_window(), 0.5)


@app.command()
def t2():
    """Set the leftmost window width to 66% of the screen (two thirds)"""
    set_width(get_left_most_window(), 2 / 3)


@app.command()
def debug():
    """Print debug information about current windows and displays"""
    ic(get_displays())
    ic([w for w in get_windows().windows])
    active_window = [w for w in get_windows().windows if w.has_focus][0]
    ic(active_window)


## I'm going to add new helpful commands - tbd the right file for them
def get_foreground_window_dimensions():
    # Get the list of all windows
    windows = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly, Quartz.kCGNullWindowID
    )

    # Iterate through the windows to find the topmost onscreen window
    for window in windows:
        if window.get("kCGWindowLayer") == 0:  # Usually, the topmost window has layer 0
            bounds = window.get("kCGWindowBounds")
            x = bounds.get("X", 0)
            y = bounds.get("Y", 0)
            width = bounds.get("Width", 0)
            height = bounds.get("Height", 0)
            return (x, y, width, height)

    return None


def capture_foreground_window(save_path="screenshot.png"):
    # Get the dimensions of the foreground window
    ic(save_path)
    dimensions = get_foreground_window_dimensions()
    if not dimensions:
        print("No foreground window found")
        return

    x, y, width, height = dimensions

    # Create a screenshot of the specified window bounds
    rect = Quartz.CGRectMake(x, y, width, height)
    image = CG.CGWindowListCreateImage(
        rect,
        CG.kCGWindowListOptionOnScreenOnly,
        CG.kCGNullWindowID,
        CG.kCGWindowImageDefault,
    )

    # Create an NSBitmapImageRep from the CGImage
    bitmap_rep = AppKit.NSBitmapImageRep.alloc().initWithCGImage_(image)

    # Create an NSData object from the bitmap representation
    png_data = bitmap_rep.representationUsingType_properties_(
        AppKit.NSPNGFileType, None
    )

    # Write the PNG data to a file
    png_data.writeToFile_atomically_(save_path, True)

    print(f"Screenshot of the foreground window saved to {save_path}")


@app.command()
def sss():
    """Take a screenshot of a selection"""
    os.system("screencapture -i -c")


def _get_current_quadrant(window_frame: Frame, display_frame: Frame) -> int:
    """Calculate which quadrant the window is currently in."""
    x_mid = display_frame.x + display_frame.w / 2
    y_mid = display_frame.y + display_frame.h / 2

    is_left = window_frame.x < x_mid
    is_top = window_frame.y < y_mid

    if is_left and is_top:
        return 0  # Top-left
    elif not is_left and is_top:
        return 1  # Top-right
    elif is_left and not is_top:
        return 2  # Bottom-left
    else:
        return 3  # Bottom-right


def _get_quadrant_positions(
    display_frame: Frame, window_frame: Frame
) -> List[tuple[float, float]]:
    """Get positions for each quadrant, 10 pixels away from corners."""
    margin = 10
    return [
        (display_frame.x + margin, display_frame.y + margin),  # Top-left
        (
            display_frame.x + display_frame.w - window_frame.w - margin,
            display_frame.y + margin,
        ),  # Top-right
        (
            display_frame.x + margin,
            display_frame.y + display_frame.h - window_frame.h - margin,
        ),  # Bottom-left
        (
            display_frame.x + display_frame.w - window_frame.w - margin,
            display_frame.y + display_frame.h - window_frame.h - margin,
        ),  # Bottom-right
    ]


@app.command()
def hm_rotate(
    turns: Annotated[int, typer.Argument(help="Number of turns to rotate")] = 1,
    corner: Annotated[bool, typer.Option(help="Move to corner position")] = False,
):
    """Rotate hand mirror window to the next corner"""
    hand_mirror_windows = [w for w in get_windows().windows if w.app == "Hand Mirror"]
    if not hand_mirror_windows or len(hand_mirror_windows) != 1:
        ic("Hand mirror not as expected")
        ic(hand_mirror_windows)
        return

    hand_mirror_window = hand_mirror_windows[0]
    display = get_displays().displays[hand_mirror_window.display - 1]

    current_quadrant = _get_current_quadrant(hand_mirror_window.frame, display.frame)
    new_quadrant = (current_quadrant + turns) % 4

    quadrant_positions = _get_quadrant_positions(
        display.frame, hand_mirror_window.frame
    )
    new_x, new_y = quadrant_positions[new_quadrant]

    call_yabai(f"-m window {hand_mirror_window.id} --move abs:{new_x}:{new_y}")


@app.command()
def ssa():
    """Take a screenshot of the active window and copy it to the clipboard."""
    from PIL import Image
    import io

    # Wait for Alfred window to disappear if called from Alfred
    time.sleep(0.1)

    dimensions = get_foreground_window_dimensions()
    ic(dimensions)
    if not dimensions:
        print("No active window found")
        return

    print(
        f"Active window dimensions: x={dimensions[0]}, y={dimensions[1]}, width={dimensions[2]}, height={dimensions[3]}"
    )

    screenshots_dir = Path.home() / "tmp" / "screenshots"
    ensure_directory_exists(screenshots_dir)

    current_time = datetime.now().strftime("%Y%m%d_%H_%M_%S_%f")[:-5]
    screenshot_path = screenshots_dir / f"screenshot_{current_time}.webp"
    latest_path = screenshots_dir / "latest.webp"

    # Capture the screenshot as PNG in memory
    png_data = capture_foreground_window_to_memory()

    # Convert PNG to WebP and save
    with Image.open(io.BytesIO(png_data)) as img:
        img.save(str(screenshot_path), "WEBP")
        img.save(str(latest_path), "WEBP")

    print(f"Screenshot saved to: {screenshot_path}")

    # Load the WebP image and copy to clipboard
    img = AppKit.NSImage.alloc().initWithContentsOfFile_(str(latest_path))
    pb = AppKit.NSPasteboard.generalPasteboard()
    pb.clearContents()
    pb.writeObjects_([img])


def capture_foreground_window_to_memory():
    dimensions = get_foreground_window_dimensions()
    if not dimensions:
        return None

    x, y, width, height = dimensions

    rect = Quartz.CGRectMake(x, y, width, height)
    image = CG.CGWindowListCreateImage(
        rect,
        CG.kCGWindowListOptionOnScreenOnly,
        CG.kCGNullWindowID,
        CG.kCGWindowImageDefault,
    )

    bitmap_rep = AppKit.NSBitmapImageRep.alloc().initWithCGImage_(image)
    png_data = bitmap_rep.representationUsingType_properties_(
        AppKit.NSPNGFileType, None
    )

    return png_data.bytes()


def jiggle_mouse():
    """
    Move the mouse cursor in a circular pattern and draw a yellow circle around it.
    """
    radius = 10  # Radius of the circle
    center_x, center_y = Quartz.CGEventGetLocation(Quartz.CGEventCreate(None))

    for _ in range(10):  # Repeat 10 times for 10 circles
        for angle in range(0, 360, 5):  # Move in 5-degree increments
            # Calculate new position
            radian = math.radians(angle)
            dx = radius * math.cos(radian)
            dy = radius * math.sin(radian)

            # Move mouse and update window position
            new_x, new_y = center_x + dx, center_y + dy
            Quartz.CGEventPost(
                Quartz.kCGHIDEventTap,
                Quartz.CGEventCreateMouseEvent(
                    None, Quartz.kCGEventMouseMoved, (new_x, new_y), 0
                ),
            )
            # Wait for a short interval
            time.sleep(0.001)


@app.command()
def jiggle():
    """Move the mouse cursor in a circular pattern"""
    jiggle_mouse()


def _execute_git_operations(iclip_dir: Path, filename: str) -> bool:
    """Execute git operations for uploading image. Returns True on success."""
    commands = [
        f"cd {iclip_dir} && git fetch && git pull --rebase",
        f"cd {iclip_dir} && git add {filename}",
        f'cd {iclip_dir} && git commit -m "adding image {filename}"',
        f"cd {iclip_dir} && git push",
    ]

    for cmd in commands:
        if os.system(cmd) != 0:
            return False
    return True


@app.command()
def ghimgpaste(
    caption: Annotated[str, typer.Argument(help="Caption for the image")] = "",
):
    """Save clipboard image to GitHub and copy markdown link to clipboard"""
    from datetime import datetime
    import os

    # Setup directory
    iclip_dir = Path("~/gits/ipaste").expanduser()
    ensure_directory_exists(iclip_dir)

    # Generate filename and save image
    current_time = datetime.now().strftime("%Y%m%d_%H%M%S")
    png_path = iclip_dir / f"{current_time}.png"
    webp_path = iclip_dir / f"{current_time}.webp"

    # Try to paste the image
    if os.system(f"pngpaste {png_path}") != 0:
        print("[red]Error: No image found in clipboard[/red]")
        return

    # Convert to webp
    if os.system(f"magick {png_path} {webp_path}") != 0:
        print("[red]Error converting image to webp[/red]")
        return

    # Clean up PNG
    png_path.unlink()

    # Git operations with error checking
    try:
        if not _execute_git_operations(iclip_dir, f"{current_time}.webp"):
            print("[red]Error pushing to repository. Check your git credentials.[/red]")
            return

        # Generate markdown and copy to clipboard
        template = f"![{caption}](https://raw.githubusercontent.com/idvorkin/ipaste/main/{current_time}.webp)"
        pyperclip.copy(template)
        print("[green]Successfully uploaded and copied markdown to clipboard![/green]")
        print(template)

    except Exception as e:
        print(f"[red]Error during git operations: {str(e)}[/red]")


# Parameter completions for commands that support them
PARAM_COMPLETIONS: dict[str, dict[str, list[str]]] = {
    "p_foo": {
        "color": ["red", "green", "blue", "yellow", "purple"],
        "size": ["small", "medium", "large", "xlarge"],
    },
    "p_bar": {
        "fruit": ["apple", "banana", "cherry", "date"],
        "count": ["1", "2", "3", "5", "10"],
    },
}


class AlfredItems(BaseModel):
    class Item(BaseModel):
        title: str
        subtitle: str
        arg: str
        autocomplete: str | None = None

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


@app.command()
def alfred():
    """Generate JSON output of all commands for Alfred workflow integration"""
    # Try to load from cache first
    cached_commands = load_cached_commands()
    if cached_commands:
        print(cached_commands)
        return

    print("Cache Status: Miss", file=sys.stderr)

    # If no valid cache, generate commands with docstrings
    items = []
    for cmd in app.registered_commands:
        name = cmd.callback.__name__.replace("_", "-")  # type: ignore
        doc = cmd.callback.__doc__ or name  # type: ignore
        subtitle = doc.split("\n")[0] if doc else name
        items.append(AlfredItems.Item(title=name, subtitle=subtitle, arg=name))
    alfred_items = AlfredItems(items=items)
    json_output = alfred_items.model_dump_json(indent=4, exclude_none=True)

    # Save to cache for future use
    save_commands_cache(json_output)

    print(json_output)


@app.command()
def alfred_ter():
    """Generate JSON output of terminal tabs for Alfred workflow (use with 'yt' keyword)"""
    import builtins  # Use standard print, not Rich's print (which interprets [...] as markup)

    terminals = _get_terminal_windows_for_completion()

    items = []
    for title, description, identifier in terminals:
        items.append(
            AlfredItems.Item(
                title=title,
                subtitle=description,
                arg=f"ter {identifier}",
            )
        )

    if not items:
        items.append(
            AlfredItems.Item(
                title="No terminals found",
                subtitle="Open a terminal first",
                arg="",
            )
        )

    alfred_items = AlfredItems(items=items)
    builtins.print(alfred_items.model_dump_json(indent=4, exclude_none=True))


# Flow constants
FLOW_APP_NAME = "Flow"


def call_flow(command: str, capture_output: bool = False) -> CompletedProcess:
    """Execute Flow AppleScript command with DRY pattern."""
    applescript = f'tell application "{FLOW_APP_NAME}" to {command}'
    return subprocess.run(
        ["osascript", "-e", applescript],
        capture_output=capture_output,
        text=True,
    )


def get_flow_info() -> tuple[str, str, str] | None:
    """Get Flow time, phase, and title in one call."""
    time_result = call_flow("getTime", capture_output=True)
    phase_result = call_flow("getPhase", capture_output=True)
    title_result = call_flow("getTitle", capture_output=True)

    if all(r.returncode == 0 for r in [time_result, phase_result, title_result]):
        return (
            time_result.stdout.strip(),
            phase_result.stdout.strip(),
            title_result.stdout.strip(),
        )
    return None


@app.command()
def zz():
    """Put the system to sleep"""
    os.system("pmset sleepnow")


@app.command()
def flow_go(
    title: Annotated[
        str, typer.Argument(help="Optional title for the Flow session")
    ] = "",
):
    """Start the Flow application or service, optionally with a custom title."""
    if title:
        call_flow("reset")
        call_flow(f'setTitle to "{title}"')
        print(f"Flow session titled '{title}'")

    call_flow("start")
    print("Flow started")


@app.command()
def flow_stop():
    """Stop the Flow application or service."""
    call_flow("stop")
    print("Flow stopped")
    flow_show()


@app.command()
def flow_show():
    """Show the current status of the Flow application or service."""
    call_flow("show")
    print("Flow status displayed")


@app.command()
def flow_info(
    oneline: Annotated[
        bool, typer.Option(help="Display info in one line format")
    ] = False,
):
    """Get remaining time, current phase, and session title from Flow."""
    flow_data = get_flow_info()
    if not flow_data:
        print("Failed to retrieve information from Flow.")
        return

    time_remaining, phase, title = flow_data
    if oneline:
        if phase == "Break":
            print(f"Break [{time_remaining}]")
        else:
            print(f"{title} [{time_remaining}]")
    else:
        print(f"Remaining Time: {time_remaining}")
        print(f"Current Phase: {phase}")
        print(f"Session Title: {title}")


@app.command()
def flow_get_title():
    """Get the session title from Flow."""
    title_result = call_flow("getTitle", capture_output=True)
    if title_result.returncode == 0:
        print(f"Session Title: {title_result.stdout.strip()}")
    else:
        print("Failed to retrieve session title from Flow.")


@app.command()
def flow_reset():
    """Reset the Flow session."""
    call_flow("reset")
    print("session reset")


@app.command()
def flow_rename(
    title: Annotated[str, typer.Argument(help="New title for the Flow session")],
):
    """Rename the Flow session."""
    flow_reset()
    call_flow(f'setTitle to "{title}"')
    print(f"Flow session renamed to '{title}'")
    flow_go()


@app.command()
def aa_list_apps(
    verbose: Annotated[
        bool, typer.Option("--verbose", "-v", help="Show detailed process information")
    ] = False,
    grouped: Annotated[
        bool,
        typer.Option("--grouped", "-g", help="Group processes by application name"),
    ] = True,
):
    """List all running applications"""
    print("\n[bold cyan]Running Applications:[/bold cyan]")
    print("=" * 60)

    apps_data = []
    for proc in psutil.process_iter(
        ["pid", "name", "exe", "memory_info", "cpu_percent", "create_time"]
    ):
        try:
            info = proc.info
            # Filter for actual applications (has an executable path in /Applications or similar)
            exe_path = info.get("exe", "")
            if exe_path and (
                "/Applications/" in exe_path or "/System/Applications/" in exe_path
            ):
                # Get app name from the .app bundle
                app_name = info["name"]
                if ".app" in exe_path:
                    app_name = (
                        exe_path.split("/")[-1].replace(".app", "")
                        if "/" in exe_path
                        else app_name
                    )

                # Clean up app name to group related processes
                base_app_name = app_name
                if grouped:
                    # Remove common suffixes that indicate helper processes
                    for suffix in [
                        " Helper (Renderer)",
                        " Helper (GPU)",
                        " Helper (Plugin)",
                        " Helper",
                        " (Renderer)",
                        " (GPU)",
                        " (Plugin)",
                    ]:
                        if app_name.endswith(suffix):
                            base_app_name = app_name.replace(suffix, "").strip()
                            break

                memory_mb = (
                    info["memory_info"].rss / 1024 / 1024
                    if info.get("memory_info")
                    else 0
                )
                cpu_percent = info.get("cpu_percent", 0) or 0.0

                apps_data.append(
                    {
                        "name": app_name,
                        "base_name": base_app_name,
                        "pid": info["pid"],
                        "exe_path": exe_path,
                        "memory_mb": memory_mb,
                        "cpu_percent": cpu_percent,
                        "create_time": info.get("create_time", 0),
                    }
                )
        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            # Process disappeared or we don't have access
            continue

    if grouped:
        # Group by base app name and aggregate stats
        from collections import defaultdict

        grouped_apps = defaultdict(list)
        for app in apps_data:
            grouped_apps[app["base_name"]].append(app)

        # Create aggregated data
        aggregated_data = []
        for base_name, processes in grouped_apps.items():
            total_memory = sum(p["memory_mb"] for p in processes)
            total_cpu = sum(p["cpu_percent"] for p in processes)
            process_count = len(processes)
            pids = [p["pid"] for p in processes]

            aggregated_data.append(
                {
                    "base_name": base_name,
                    "total_memory": total_memory,
                    "total_cpu": total_cpu,
                    "process_count": process_count,
                    "pids": pids,
                    "processes": processes,
                }
            )

        # Sort by total memory usage
        aggregated_data.sort(key=lambda x: x["total_memory"], reverse=True)

        for app_group in aggregated_data:
            memory_str = f"{app_group['total_memory']:.1f}MB"
            cpu_str = f"{app_group['total_cpu']:.1f}%"

            # Color code based on resource usage
            memory_color = (
                "red"
                if app_group["total_memory"] > 500
                else "yellow"
                if app_group["total_memory"] > 100
                else "green"
            )
            cpu_color = (
                "red"
                if app_group["total_cpu"] > 50
                else "yellow"
                if app_group["total_cpu"] > 10
                else "green"
            )

            # Show process count if more than 1
            process_info = (
                f" ({app_group['process_count']} processes)"
                if app_group["process_count"] > 1
                else ""
            )

            print(f"[bold]{app_group['base_name']}[/bold]{process_info}")
            print(
                f"    Memory: [{memory_color}]{memory_str}[/{memory_color}] | CPU: [{cpu_color}]{cpu_str}[/{cpu_color}]"
            )

            if verbose:
                for proc in app_group["processes"]:
                    print(
                        f"    └─ {proc['name']} (PID: {proc['pid']}) - {proc['memory_mb']:.1f}MB"
                    )

            print()

        print(
            f"\n[dim]Total: {len(aggregated_data)} applications ({len(apps_data)} processes)[/dim]"
        )

    else:
        # Original behavior - show individual processes
        apps_data.sort(key=lambda x: x["memory_mb"], reverse=True)

        for app in apps_data:
            memory_str = f"{app['memory_mb']:.1f}MB"
            cpu_str = f"{app['cpu_percent']:.1f}%" if app["cpu_percent"] else "0.0%"

            # Color code based on resource usage
            memory_color = (
                "red"
                if app["memory_mb"] > 500
                else "yellow"
                if app["memory_mb"] > 100
                else "green"
            )
            cpu_percent = app["cpu_percent"] or 0.0
            cpu_color = (
                "red" if cpu_percent > 50 else "yellow" if cpu_percent > 10 else "green"
            )

            print(f"[bold]{app['name']}[/bold] (PID: {app['pid']})")
            print(
                f"    Memory: [{memory_color}]{memory_str}[/{memory_color}] | CPU: [{cpu_color}]{cpu_str}[/{cpu_color}]"
            )

            if verbose:
                print(f"    Path: [dim]{app['exe_path']}[/dim]")
            print()

        print(f"\n[dim]Total: {len(apps_data)} processes[/dim]")


def extract_app_name(exe_path: str, process_name: str) -> str:
    """Extract clean application name from executable path."""
    if not exe_path:
        return process_name

    if exe_path.endswith(".app"):
        return exe_path.split("/")[-1].replace(".app", "")
    elif ".app/" in exe_path:  # Helper processes inside app bundles
        return exe_path.split(".app/")[0].split("/")[-1]

    return process_name


def terminate_process_safely(proc, timeout: float = 3.0) -> str:
    """Safely terminate a process with timeout and fallback to kill.

    Returns: 'terminated', 'killed', or raises exception
    """
    try:
        proc.terminate()
        proc.wait(timeout=timeout)
        return "terminated"
    except psutil.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=1)  # Wait a bit more for force kill
        return "killed"
    except (psutil.NoSuchProcess, psutil.AccessDenied) as e:
        raise e


@app.command()
def kill_cruft(
    force: Annotated[
        bool,
        typer.Option(
            "--force", "-f", help="Actually kill processes (default is dry run)"
        ),
    ] = False,
    interactive: Annotated[
        bool,
        typer.Option("--interactive", "-i", help="Ask before killing each process"),
    ] = False,
    close_finder: Annotated[
        bool, typer.Option("--close-finder", help="Also close all Finder windows")
    ] = True,
    messages: Annotated[
        bool,
        typer.Option(
            "--messages", help="Kill messaging apps (Signal, iMessage, WhatsApp)"
        ),
    ] = False,
):
    """Kill unnecessary applications (cruft) based on whitelist.

    This function terminates applications that are considered "cruft" - non-essential
    apps that can be safely closed. OrbStack is specifically protected and will be
    minimized instead of killed.

    Args:
        force: Actually kill processes (default is dry run)
        interactive: Ask before killing each process
        close_finder: Also close all Finder windows
        messages: Include messaging apps (Signal, Messages, WhatsApp) in kill list
    """

    # Common applications that are often safe to kill when not actively used
    common_cruft = {
        "Slack",
        "Discord",
        "Teams",
        "Zoom",
        "Skype",
        "Telegram",
        "Steam",
        "Epic Games Launcher",
        "Battle.net",
        "Origin",
        "Spotify",
        "VLC",
        "IINA",
        "QuickTime Player",
        "Adobe Creative Cloud",
        "Adobe Photoshop",
        "Adobe Illustrator",
        "Adobe Premiere Pro",
        "Microsoft Word",
        "Microsoft Excel",
        "Microsoft PowerPoint",
        "Chrome",
        "Firefox",
        "Safari",
        "Edge",  # Be careful with browsers
        "Postman",
        "Insomnia",
        "TablePlus",
        "Sequel Pro",
        "Activity Monitor",
        "System Preferences",
        "Calculator",
        "Terminal",
        "Cisco Secure Client",  # Terminal (using iTerm instead), VPN client
        # Igor's cruft additions
        "StocksWidget",
        "Stocks",
        "NewsToday2",
        "News",
        "Finder",
        "System Settings",
        "GeneralSettings",
        "Photos",
        "TextEdit",
        "Keychain Access",
        "Preview",
        "Weather",
        "Numbers",
        # Apple Productivity & Media Apps
        "Pages",
        "Keynote",
        "Freeform",
        "Notes",
        "Reminders",
        "Calendar",
        "Contacts",
        "Mail",
        "TV",
        "Music",
        "Podcasts",
        "Books",
        "Maps",
        "Home",
        "TestFlight",
    }

    effective_cruft_set = set(common_cruft)  # Start with a copy

    if messages:
        messaging_apps_to_kill = {"Signal", "Messages", "WhatsApp"}
        effective_cruft_set.update(messaging_apps_to_kill)

    print("\n[bold red]Kill Cruft Analysis[/bold red]")
    print(f"Mode: {'EXECUTE' if force else 'DRY RUN'}")
    print("=" * 60)

    candidates = []

    for proc in psutil.process_iter(
        ["pid", "name", "exe", "memory_info", "cpu_percent"]
    ):
        try:
            info = proc.info
            exe_path = info.get("exe", "")

            # Only consider actual applications
            if exe_path and (
                "/Applications/" in exe_path or "/System/Applications/" in exe_path
            ):
                app_name = extract_app_name(exe_path, info["name"])

                memory_mb = (
                    info["memory_info"].rss / 1024 / 1024
                    if info.get("memory_info")
                    else 0
                )
                cpu_percent = info.get("cpu_percent", 0) or 0.0

                # Only kill apps that are in the cruft whitelist
                if app_name in effective_cruft_set:
                    candidates.append(
                        {
                            "proc": proc,
                            "name": app_name,
                            "pid": info["pid"],
                            "memory_mb": memory_mb,
                            "cpu_percent": cpu_percent,
                        }
                    )

        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            continue

    if not candidates:
        print("[green]No cruft applications found running.[/green]")
        return

    # Sort by memory usage
    candidates.sort(key=lambda x: x["memory_mb"], reverse=True)

    print(f"\n[yellow]Found {len(candidates)} potential cruft applications:[/yellow]\n")

    killed_count = 0
    for candidate in candidates:
        memory_str = f"{candidate['memory_mb']:.1f}MB"
        cpu_percent = candidate["cpu_percent"] or 0.0
        cpu_str = f"{cpu_percent:.1f}%"

        print(f"[bold]{candidate['name']}[/bold] (PID: {candidate['pid']})")
        print(f"    Memory: {memory_str} | CPU: {cpu_str}")

        should_kill = False

        if not force:
            print("    [dim]Would be killed (dry run)[/dim]")
        elif interactive:
            response = typer.prompt(f"    Kill {candidate['name']}? [y/N]", default="n")
            should_kill = response.lower() in ["y", "yes"]
        else:
            should_kill = True

        if should_kill and force:
            try:
                result = terminate_process_safely(candidate["proc"])
                if result == "terminated":
                    print("    [green]✓ Terminated[/green]")
                else:  # killed
                    print(
                        "    [orange]✓ Force killed (process didn't respond to terminate)[/orange]"
                    )
                killed_count += 1
            except (psutil.NoSuchProcess, psutil.AccessDenied) as e:
                print(f"    [red]✗ Failed to kill: {e}[/red]")

        print()

    if force:
        print(f"\n[green]Successfully killed {killed_count} applications[/green]")
    else:
        print(
            f"\n[yellow]Dry run complete. {len(candidates)} applications would be affected.[/yellow]"
        )
        print("[dim]Use --force to actually kill processes[/dim]")

    # Close Finder windows if requested
    if close_finder:
        print("\n[bold cyan]Closing Finder Windows[/bold cyan]")
        if force:
            applescript = """
            tell application "Finder"
                close every window
            end tell
            """

            try:
                result = subprocess.run(
                    ["osascript", "-e", applescript], capture_output=True, text=True
                )

                if result.returncode == 0:
                    print("[green]✓ All Finder windows closed[/green]")
                else:
                    print(
                        f"[red]✗ Failed to close Finder windows: {result.stderr}[/red]"
                    )

            except Exception as e:
                print(f"[red]✗ Error closing Finder windows: {e}[/red]")
        else:
            print("[dim]Would close all Finder windows[/dim]")

    # Minimize OrbStack if it's running
    print("\n[bold cyan]Minimizing OrbStack[/bold cyan]")
    if force:
        minimize_script = """
        tell application "System Events"
            if exists (process "OrbStack") then
                tell process "OrbStack"
                    set visible to false
                end tell
                return "minimized"
            else
                return "not running"
            end if
        end tell
        """

        try:
            result = subprocess.run(
                ["osascript", "-e", minimize_script], capture_output=True, text=True
            )

            if result.returncode == 0:
                if "minimized" in result.stdout:
                    print("[green]✓ OrbStack minimized[/green]")
                elif "not running" in result.stdout:
                    print("[dim]OrbStack is not running[/dim]")
            else:
                print(f"[red]✗ Failed to minimize OrbStack: {result.stderr}[/red]")

        except Exception as e:
            print(f"[red]✗ Error minimizing OrbStack: {e}[/red]")
    else:
        print("[dim]Would minimize OrbStack[/dim]")


@app.command()
def kill_apps(
    app_names: Annotated[List[str], typer.Argument(help="Application names to kill")],
    force: Annotated[
        bool,
        typer.Option(
            "--force", "-f", help="Actually kill processes (default is dry run)"
        ),
    ] = False,
    force_kill: Annotated[
        bool, typer.Option("--force-kill", help="Use SIGKILL instead of SIGTERM")
    ] = False,
):
    """Kill specific applications by name"""

    print("\n[bold red]Kill Applications[/bold red]")
    print(f"Target apps: {', '.join(app_names)}")
    print(f"Mode: {'EXECUTE' if force else 'DRY RUN'}")
    print("=" * 60)

    # Normalize app names for matching (lowercase, remove common suffixes)
    normalized_targets = []
    for name in app_names:
        # Handle common abbreviations and variations
        name_lower = name.lower()
        if name_lower in ["stocks", "stockswidget"]:
            normalized_targets.append("stockswidget")
        elif name_lower in ["news", "newstoday", "newstoday2"]:
            normalized_targets.append("newstoday2")
        elif name_lower in ["finder"]:
            normalized_targets.append("finder")
        elif name_lower in ["iina"]:
            normalized_targets.append("iina")
        else:
            normalized_targets.append(name_lower)

    candidates = []

    for proc in psutil.process_iter(["pid", "name", "exe", "memory_info"]):
        try:
            info = proc.info
            exe_path = info.get("exe", "")

            # Only consider actual applications
            if exe_path and (
                "/Applications/" in exe_path or "/System/Applications/" in exe_path
            ):
                app_name = extract_app_name(exe_path, info["name"])

                # Check if this app matches any of our targets
                app_name_lower = app_name.lower()
                for target in normalized_targets:
                    if target in app_name_lower or app_name_lower.startswith(target):
                        memory_mb = (
                            info["memory_info"].rss / 1024 / 1024
                            if info.get("memory_info")
                            else 0
                        )

                        candidates.append(
                            {
                                "proc": proc,
                                "name": app_name,
                                "pid": info["pid"],
                                "memory_mb": memory_mb,
                                "matched_target": target,
                            }
                        )
                        break

        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            continue

    if not candidates:
        print("[yellow]No matching applications found![/yellow]")
        return

    # Group by app name
    from collections import defaultdict

    grouped_candidates = defaultdict(list)
    for candidate in candidates:
        base_name = candidate["name"]
        # Remove helper suffixes for grouping
        for suffix in [
            " Helper (Renderer)",
            " Helper (GPU)",
            " Helper (Plugin)",
            " Helper",
        ]:
            if base_name.endswith(suffix):
                base_name = base_name.replace(suffix, "").strip()
                break
        grouped_candidates[base_name].append(candidate)

    print(
        f"\n[yellow]Found {len(candidates)} processes from {len(grouped_candidates)} applications:[/yellow]\n"
    )

    killed_count = 0
    for app_name, processes in grouped_candidates.items():
        total_memory = sum(p["memory_mb"] for p in processes)
        process_count = len(processes)

        print(
            f"[bold]{app_name}[/bold] ({process_count} process{'es' if process_count > 1 else ''})"
        )
        print(f"    Total Memory: {total_memory:.1f}MB")

        for candidate in processes:
            print(
                f"    └─ {candidate['name']} (PID: {candidate['pid']}) - {candidate['memory_mb']:.1f}MB"
            )

            if force:
                try:
                    if force_kill:
                        candidate["proc"].kill()  # SIGKILL
                        print("      [red]✓ Force killed[/red]")
                    else:
                        result = terminate_process_safely(candidate["proc"])
                        if result == "terminated":
                            print("      [green]✓ Terminated[/green]")
                        else:  # killed
                            print(
                                "      [orange]✓ Force killed (process didn't respond)[/orange]"
                            )
                    killed_count += 1
                except (psutil.NoSuchProcess, psutil.AccessDenied) as e:
                    print(f"      [red]✗ Failed: {e}[/red]")
            else:
                print(
                    f"      [dim]Would be {'force killed' if force_kill else 'terminated'}[/dim]"
                )

        print()

    if force:
        print(f"\n[green]Successfully killed {killed_count} processes[/green]")
    else:
        print(
            f"\n[yellow]Dry run complete. {len(candidates)} processes would be affected.[/yellow]"
        )
        print("[dim]Use --force to actually kill processes[/dim]")


@app.command()
def aa_close_finder_windows():
    """Close all open Finder windows"""
    print("\n[bold cyan]Closing Finder Windows[/bold cyan]")
    print("=" * 40)

    # AppleScript to close all Finder windows
    applescript = """
    tell application "Finder"
        close every window
    end tell
    """

    try:
        result = subprocess.run(
            ["osascript", "-e", applescript], capture_output=True, text=True
        )

        if result.returncode == 0:
            print("[green]✓ All Finder windows closed successfully[/green]")
        else:
            print(f"[red]✗ Failed to close Finder windows: {result.stderr}[/red]")

    except Exception as e:
        print(f"[red]✗ Error running AppleScript: {e}[/red]")


@app.command()
def close_windows(
    app_name: Annotated[
        str, typer.Argument(help="Application name to close windows for")
    ] = "Finder",
):
    """Close all windows for a specific application"""
    print(f"\n[bold cyan]Closing {app_name} Windows[/bold cyan]")
    print("=" * 40)

    # AppleScript to close all windows of a specific app
    applescript = f'''
    tell application "{app_name}"
        close every window
    end tell
    '''

    try:
        result = subprocess.run(
            ["osascript", "-e", applescript], capture_output=True, text=True
        )

        if result.returncode == 0:
            print(f"[green]✓ All {app_name} windows closed successfully[/green]")
        else:
            print(f"[red]✗ Failed to close {app_name} windows: {result.stderr}[/red]")

    except Exception as e:
        print(f"[red]✗ Error running AppleScript: {e}[/red]")


def _call_ai_clip(command: str, description: str):
    """Helper function to call ai-clip with given command and description"""
    try:
        print(f"[cyan]{description}[/cyan]")
        ai_clip_path = PY_TOOLS_PATH / "ai-clip"

        # Check if ai-clip exists and is executable
        if not ai_clip_path.exists():
            print(f"[red]Error: ai-clip not found at {ai_clip_path}[/red]")
            return

        # Load GROQ_API_KEY from secretBox.json if not in environment
        groq_api_key = os.getenv("GROQ_API_KEY")
        if not groq_api_key:
            try:
                import json

                secret_box_path = Path.home() / "gits" / "igor2" / "secretBox.json"
                if secret_box_path.exists():
                    with open(secret_box_path) as f:
                        secrets = json.load(f)
                    groq_api_key = secrets.get("GROQ_API_KEY")
                    if groq_api_key:
                        os.environ["GROQ_API_KEY"] = groq_api_key
                        print("[dim]Loaded GROQ_API_KEY from secretBox.json[/dim]")
                    else:
                        print(
                            "[red]Error: GROQ_API_KEY not found in secretBox.json[/red]"
                        )
                        return
                else:
                    print(
                        f"[red]Error: secretBox.json not found at {secret_box_path}[/red]"
                    )
                    return
            except Exception as e:
                print(f"[red]Error loading GROQ_API_KEY from secretBox.json: {e}[/red]")
                return

        result = subprocess.run(
            [str(ai_clip_path), command], capture_output=True, text=True
        )

        if result.returncode == 0:
            # Print the output from ai-clip
            print(result.stdout)
        else:
            print(f"[red]Error running ai-clip (exit code {result.returncode}):[/red]")
            if result.stderr:
                print(f"[red]stderr: {result.stderr}[/red]")
            if result.stdout:
                print(f"[red]stdout: {result.stdout}[/red]")

    except Exception as e:
        print(f"[red]Failed to run AI {command}: {e}[/red]")


@app.command()
def ai_fix():
    """Fix spelling and grammar in clipboard using AI (calls installed ai-clip command)"""
    _call_ai_clip("fix", "🤖 Fixing clipboard text with AI...")

    # Show completion notification
    try:
        ux_path = PY_TOOLS_PATH / "ux"
        subprocess.run(
            [str(ux_path), "center", "AI Fix Complete! ✨", "--seconds", "1"],
            capture_output=True,
            check=False,
        )
    except Exception as e:
        print(f"[dim]Note: Could not show completion popup: {e}[/dim]")


@app.command()
def ai_rhyme():
    """Transform clipboard text to Dr. Seuss style using AI (calls installed ai-clip command)"""
    _call_ai_clip("seuss", "🎭 Transforming clipboard text to Dr. Seuss style...")


def _find_highest_numbered_file(directory: Path, prefix: str) -> int:
    """Find the highest number in files matching prefix<N>.png pattern.

    Args:
        directory: Directory to search in
        prefix: File prefix (e.g., "base")

    Returns:
        Highest number found, or 0 if no matching files exist
    """
    import re

    pattern = re.compile(rf"^{re.escape(prefix)}(\d+)\.png$")
    highest = 0

    if directory.exists():
        for file in directory.iterdir():
            if file.is_file():
                match = pattern.match(file.name)
                if match:
                    num = int(match.group(1))
                    highest = max(highest, num)

    return highest


def _find_most_recent_date_file(directory: Path) -> datetime | None:
    """Find the most recent date in files matching YYYY-M-D.png or YYYY-MM-DD.png pattern.

    Args:
        directory: Directory to search in

    Returns:
        Most recent datetime.date found, or None if no matching files exist
    """
    import re

    # Pattern to match YYYY-M-D.png or YYYY-MM-DD.png
    pattern = re.compile(r"^(\d{4})-(\d{1,2})-(\d{1,2})\.png$")
    most_recent = None

    if directory.exists():
        for file in directory.iterdir():
            if file.is_file():
                match = pattern.match(file.name)
                if match:
                    try:
                        year = int(match.group(1))
                        month = int(match.group(2))
                        day = int(match.group(3))
                        file_date = datetime(year, month, day)

                        if most_recent is None or file_date > most_recent:
                            most_recent = file_date
                    except ValueError:
                        # Invalid date, skip
                        continue

    return most_recent


@app.command()
def clipbase(
    directory: Annotated[
        str, typer.Argument(help="Directory to save images (default: ~/tmp)")
    ] = "",
):
    """Save clipboard image with sequential naming (base1.png, base2.png, ...)"""
    # Use provided directory or default to ~/tmp
    target_dir = Path(directory).expanduser() if directory else Path.home() / "tmp"
    ensure_directory_exists(target_dir)

    # Find the highest numbered file
    prefix = "base"
    highest_num = _find_highest_numbered_file(target_dir, prefix)
    next_num = highest_num + 1

    # Create the new filename
    filename = f"{prefix}{next_num}.png"
    file_path = target_dir / filename

    # Try to paste the image from clipboard
    if os.system(f"pngpaste {file_path}") != 0:
        print("[red]Error: No image found in clipboard[/red]")
        return

    # Copy full path to clipboard
    full_path = str(file_path.absolute())
    pyperclip.copy(full_path)
    print(f"[green]Image saved to: {full_path}[/green]")
    print("[green]Full path copied to clipboard![/green]")


@app.command()
def clipdays(
    directory: Annotated[
        str, typer.Argument(help="Directory to save images (default: ~/tmp)")
    ] = "",
):
    """Save clipboard image with date-based naming (YYYY-M-D.png), auto-incrementing by day"""
    from datetime import timedelta

    # Use provided directory or default to ~/tmp
    target_dir = Path(directory).expanduser() if directory else Path.home() / "tmp"
    ensure_directory_exists(target_dir)

    # Find the most recent date file
    most_recent_date = _find_most_recent_date_file(target_dir)

    if most_recent_date is None:
        # No existing date files, start with today
        next_date = datetime.now()
    else:
        # Increment by one day
        next_date = most_recent_date + timedelta(days=1)

    # Create the new filename (using single-digit format for month/day)
    filename = f"{next_date.year}-{next_date.month}-{next_date.day}.png"
    file_path = target_dir / filename

    # Try to paste the image from clipboard
    if os.system(f"pngpaste {file_path}") != 0:
        print("[red]Error: No image found in clipboard[/red]")
        return

    # Copy full path to clipboard
    full_path = str(file_path.absolute())
    pyperclip.copy(full_path)
    print(f"[green]Image saved to: {full_path}[/green]")
    print("[green]Full path copied to clipboard![/green]")


@app.command()
def cliptofile():
    """Save clipboard image to a random file in tmp and copy the full path to clipboard"""
    import uuid

    # Create tmp directory if it doesn't exist
    tmp_dir = Path.home() / "tmp"
    ensure_directory_exists(tmp_dir)

    # Generate random filename
    random_filename = f"clip_{uuid.uuid4().hex[:8]}.png"
    file_path = tmp_dir / random_filename

    # Try to paste the image from clipboard
    if os.system(f"pngpaste {file_path}") != 0:
        print("[red]Error: No image found in clipboard[/red]")
        return

    # Copy full path to clipboard
    full_path = str(file_path.absolute())
    pyperclip.copy(full_path)
    print(f"[green]Image saved to: {full_path}[/green]")
    print("[green]Full path copied to clipboard![/green]")


def _focus_iterm_tab(coords: str) -> bool:
    """Focus an iTerm2 tab by coordinates (win:tab:session). Returns True on success."""
    parts = coords.split(":")
    if len(parts) != 3:
        return False
    win_idx, tab_idx, sess_idx = parts

    script = f"""
    tell application "iTerm2"
        activate
        set targetWindow to window {win_idx}
        tell targetWindow
            select tab {tab_idx}
            tell tab {tab_idx}
                select session {sess_idx}
            end tell
        end tell
    end tell
    """
    result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    return result.returncode == 0


def _focus_ghostty_tab(title: str) -> bool:
    """Focus a Ghostty tab by clicking its Window menu item. Returns True on success."""
    # Escape quotes in title for AppleScript
    escaped_title = title.replace('"', '\\"')

    script = f'''
    tell application "Ghostty"
        activate
        reopen
    end tell
    delay 0.2
    tell application "System Events"
        tell process "Ghostty"
            set frontmost to true
            click menu item "{escaped_title}" of menu "Window" of menu bar 1
        end tell
    end tell
    '''
    result = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    return result.returncode == 0


def _ter_log(msg: str):
    """Log to ~/tmp/y_ter.log for debugging."""
    from datetime import datetime

    log_file = Path.home() / "tmp" / "y_ter.log"
    try:
        with open(log_file, "a") as f:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            f.write(f"[{timestamp}] {msg}\n")
    except Exception:
        pass


@app.command()
def ter(
    target: Annotated[
        str, typer.Argument(help="Tab identifier, window ID, or title substring")
    ] = "",
    list_only: Annotated[
        bool, typer.Option("--list", "-l", help="List terminals without focusing")
    ] = False,
    debug: Annotated[
        bool,
        typer.Option("--debug", "-d", help="Enable debug logging to ~/tmp/y_ter.log"),
    ] = False,
):
    """Switch focus to terminal tab/window

    Without arguments: lists available terminals/tabs.
    With identifier: focuses that specific tab/window (from Alfred completions).
    With text: searches for terminal with matching title and focuses it.

    Identifier formats:
      iterm:W:T:S  - iTerm2 window W, tab T, session S
      ghostty:TITLE - Ghostty tab by title
      yabai:ID     - Yabai window ID
      <number>     - Yabai window ID (legacy)
      <text>       - Search by title substring

    Debug log: ~/tmp/y_ter.log
    """
    if debug:
        _ter_log(f"ter called with target='{target}' list_only={list_only}")

    terminal_apps = ["Ghostty", "iTerm2", "Terminal", "Warp", "Alacritty", "kitty"]

    # No target - list available terminals
    if not target or list_only:
        from rich.markup import escape

        terminals = _get_terminal_windows_for_completion()
        if not terminals:
            print("[yellow]No terminal windows found[/yellow]")
            return
        if debug:
            _ter_log(f"Listing {len(terminals)} terminals")
        print("[bold]Available terminals:[/bold]")
        for title, description, identifier in terminals:
            # Escape Rich markup in titles and identifiers (e.g., [mosh] would be interpreted as markup)
            safe_title = escape(title)
            safe_id = escape(identifier)
            print(f"  {safe_title} [dim]({description})[/dim]")
            print(f"    [dim]y ter {safe_id}[/dim]")
        return

    # Handle specific identifier formats
    if target:
        if debug:
            _ter_log(f"Processing target: '{target}'")

        # iTerm2 tab identifier
        if target.startswith("iterm:"):
            coords = target[6:]  # Remove "iterm:" prefix
            if debug:
                _ter_log(f"Focusing iTerm2 tab with coords: {coords}")
            if _focus_iterm_tab(coords):
                if debug:
                    _ter_log("iTerm2 focus SUCCESS")
                return
            if debug:
                _ter_log("iTerm2 focus FAILED")
            print(f"[yellow]Failed to focus iTerm2 tab {coords}[/yellow]")
            return

        # Ghostty tab identifier
        if target.startswith("ghostty:"):
            title = target[8:]  # Remove "ghostty:" prefix
            if debug:
                _ter_log(f"Focusing Ghostty tab with title: '{title}'")
            if _focus_ghostty_tab(title):
                if debug:
                    _ter_log("Ghostty focus SUCCESS")
                return
            if debug:
                _ter_log("Ghostty focus FAILED")
            print(f"[yellow]Failed to focus Ghostty tab '{title}'[/yellow]")
            return

        # Yabai window identifier
        if target.startswith("yabai:"):
            win_id = target[6:]  # Remove "yabai:" prefix
            if debug:
                _ter_log(f"Focusing yabai window with id: {win_id}")
            call_yabai(f"-m window --focus {win_id}")
            return

        # Try as title substring FIRST - search iTerm tabs, Ghostty tabs, and yabai windows
        target_lower = target.lower()
        if debug:
            _ter_log(f"Searching for title containing: '{target_lower}'")

        # Search Ghostty tabs
        for title, _, identifier in _get_ghostty_tabs():
            if target_lower in title.lower():
                if debug:
                    _ter_log(f"Found Ghostty match: '{title}'")
                ghostty_title = identifier[8:]  # Remove "ghostty:" prefix
                if _focus_ghostty_tab(ghostty_title):
                    if debug:
                        _ter_log("Ghostty focus SUCCESS")
                    return

        # Search iTerm tabs
        for title, _, identifier in _get_iterm_tabs():
            if target_lower in title.lower():
                if debug:
                    _ter_log(f"Found iTerm match: '{title}'")
                coords = identifier[6:]  # Remove "iterm:" prefix
                if _focus_iterm_tab(coords):
                    if debug:
                        _ter_log("iTerm focus SUCCESS")
                    return

        # Search other terminal windows via yabai
        try:
            windows = get_windows().windows
            for win in windows:
                if win.app in terminal_apps and target_lower in win.title.lower():
                    if debug:
                        _ter_log(f"Found yabai match: '{win.title}' (id={win.id})")
                    call_yabai(f"-m window --focus {win.id}")
                    return
        except Exception:
            pass

        # Last resort: try as plain window ID (legacy/backwards compat)
        try:
            win_id = int(target)
            if debug:
                _ter_log(f"Trying as yabai window ID: {win_id}")
            call_yabai(f"-m window --focus {win_id}")
            return
        except ValueError:
            pass
        except Exception as e:
            if debug:
                _ter_log(f"Yabai window ID failed: {e}")

        print(f"[yellow]No terminal matching '{target}'[/yellow]")


@app.command()
def p_foo(
    color: Annotated[str, typer.Argument(help="Color choice")] = "",
    size: Annotated[str, typer.Argument(help="Size choice")] = "",
):
    """Test command with color and size parameters"""
    print(f"[green]p_foo called with color={color}, size={size}[/green]")


@app.command()
def p_bar(
    fruit: Annotated[str, typer.Argument(help="Fruit choice")] = "",
    count: Annotated[str, typer.Argument(help="Count choice")] = "",
):
    """Test command with fruit and count parameters"""
    print(f"[green]p_bar called with fruit={fruit}, count={count}[/green]")


def _get_all_commands() -> list[tuple[str, str]]:
    """Get all registered commands with their docstrings."""
    commands = []
    for cmd in app.registered_commands:
        name = cmd.callback.__name__.replace("_", "-")  # type: ignore
        doc = cmd.callback.__doc__ or ""  # type: ignore
        # Get first line of docstring
        subtitle = doc.split("\n")[0] if doc else name
        commands.append((name, subtitle))
    return commands


@app.command()
def alfred_complete(
    query: Annotated[str, typer.Argument(help="Query string from Alfred")] = "",
):
    """Generate Alfred completions for commands and parameters.

    Handles three cases:
    1. Empty query: show all commands
    2. Partial command: filter commands
    3. Command with space: show parameter completions
    """
    # Don't strip - we need to detect trailing space for parameter completion
    has_trailing_space = query.endswith(" ")
    query_stripped = query.strip()
    parts = query_stripped.split() if query_stripped else []

    items: list[AlfredItems.Item] = []

    if len(parts) == 0:
        # Show all commands
        for name, subtitle in _get_all_commands():
            # Commands with params get autocomplete for drilling down
            cmd_key = name.replace("-", "_")
            autocomplete = f"{name} " if cmd_key in PARAM_COMPLETIONS else None
            items.append(
                AlfredItems.Item(
                    title=name,
                    subtitle=subtitle,
                    arg=name,
                    autocomplete=autocomplete,
                )
            )
    elif len(parts) == 1 and not has_trailing_space:
        # Partial command - filter matching commands
        prefix = parts[0].lower()
        for name, subtitle in _get_all_commands():
            if name.lower().startswith(prefix) or prefix in name.lower():
                cmd_key = name.replace("-", "_")
                autocomplete = f"{name} " if cmd_key in PARAM_COMPLETIONS else None
                items.append(
                    AlfredItems.Item(
                        title=name,
                        subtitle=subtitle,
                        arg=name,
                        autocomplete=autocomplete,
                    )
                )
    else:
        # Command entered, show parameter completions
        cmd = parts[0].replace("-", "_")
        param_values = parts[1:] if len(parts) > 1 else []

        if cmd in PARAM_COMPLETIONS:
            param_names = list(PARAM_COMPLETIONS[cmd].keys())
            current_param_idx = len(param_values)

            # If we have a trailing space, we're ready for next param
            if has_trailing_space:
                current_param_idx = len(param_values)
            else:
                # Still typing current param, filter it
                current_param_idx = max(0, len(param_values) - 1)

            if current_param_idx < len(param_names):
                param_name = param_names[current_param_idx]
                options = PARAM_COMPLETIONS[cmd][param_name]

                # Filter if user is typing
                filter_text = ""
                if not has_trailing_space and len(param_values) > current_param_idx:
                    filter_text = param_values[current_param_idx].lower()

                for option in options:
                    if not filter_text or option.lower().startswith(filter_text):
                        # Build the full arg with all previous params + this option
                        cmd_display = cmd.replace("_", "-")
                        full_args = param_values[:current_param_idx] + [option]
                        arg = f"{cmd_display} {' '.join(full_args)}"

                        # Autocomplete for drilling to next param
                        next_param_idx = current_param_idx + 1
                        if next_param_idx < len(param_names):
                            autocomplete = f"{arg} "
                        else:
                            autocomplete = None

                        items.append(
                            AlfredItems.Item(
                                title=option,
                                subtitle=f"{param_name} for {cmd_display}",
                                arg=arg,
                                autocomplete=autocomplete,
                            )
                        )
            else:
                # All params filled, show final command
                cmd_display = cmd.replace("_", "-")
                arg = f"{cmd_display} {' '.join(param_values)}"
                items.append(
                    AlfredItems.Item(
                        title=f"Run: {arg}",
                        subtitle="Press Enter to execute",
                        arg=arg,
                    )
                )
        else:
            # Command doesn't have param completions, just show it
            cmd_display = cmd.replace("_", "-")
            arg = query
            items.append(
                AlfredItems.Item(
                    title=f"Run: {arg}",
                    subtitle="Press Enter to execute",
                    arg=arg,
                )
            )

    alfred_items = AlfredItems(items=items)
    print(alfred_items.model_dump_json(indent=2, exclude_none=True))


if __name__ == "__main__":
    app()
