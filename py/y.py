# Flow Help Page: https://www.flow.app/help#documentation
#!/usr/bin/env python3

import sys
import json
import os
from pathlib import Path
import hashlib
import pickle


# Lazy loaded imports for full functionality
def load_full_imports():
    global typer, print, subprocess, CompletedProcess, ic, List, BaseModel, Field
    global pyperclip, Quartz, CG, AppKit, math, datetime, time

    from datetime import datetime
    import time
    import typer
    from rich import print
    import subprocess
    from subprocess import CompletedProcess
    from icecream import ic
    from typing import List
    from pydantic import BaseModel, Field
    import pyperclip
    import Quartz
    import Quartz.CoreGraphics as CG
    import AppKit
    import math

    return typer.Typer(help="A Yabai helper", no_args_is_help=True)


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


# Early exit for alfred command
if len(sys.argv) != 2 or sys.argv[1] != "alfred":
    # Not an alfred command, load full imports
    app = load_full_imports()
else:
    # Handle alfred command
    cached_result = load_cached_commands()
    if cached_result:
        print(cached_result)
        sys.exit(0)
    print("Cache Status: Miss", file=sys.stderr)
    app = load_full_imports()

FLOW_HELP_URL = "https://www.flow.app/help#documentation"

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


@app.command()
def swest():
    """Switch to the space to the west (left) of current space"""
    send_key(123)


@app.command()
def seast():
    """Switch to the space to the east (right) of current space"""
    send_key(124)


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


@app.command()
def hm_rotate(
    turns: int = typer.Argument(1, help="Number of turns to rotate"),
    corner: bool = False,
):
    """Rotate hand mirror window to the next corner"""

    hand_mirror_windows = [w for w in get_windows().windows if w.app == "Hand Mirror"]
    if not hand_mirror_windows or len(hand_mirror_windows) != 1:
        ic("Hand mirror not as expected")
        ic(hand_mirror_windows)
        return

    # find the quadrant of the screen it is in, then rotate it to be in turns
    # later quadrants
    hand_mirror_window = hand_mirror_windows[0]
    display = get_displays().displays[hand_mirror_window.display - 1]

    # Calculate which quadrant the window is currently in
    x_mid = display.frame.x + display.frame.w / 2
    y_mid = display.frame.y + display.frame.h / 2

    current_quadrant = 0
    if hand_mirror_window.frame.x < x_mid:
        if hand_mirror_window.frame.y < y_mid:
            current_quadrant = 0  # Top-left
        else:
            current_quadrant = 2  # Bottom-left
    else:
        if hand_mirror_window.frame.y < y_mid:
            current_quadrant = 1  # Top-right
        else:
            current_quadrant = 3  # Bottom-right

    # Calculate the new quadrant
    new_quadrant = (current_quadrant + turns) % 4

    # Define new positions for each quadrant
    # the per quadrant positions , shoudl be 10 pixels away from the corner

    quadrant_positions = [
        (display.frame.x + 10, display.frame.y + 10),  # Top-left
        (
            display.frame.x + display.frame.w - hand_mirror_window.frame.w - 10,
            display.frame.y + 10,
        ),  # Top-right
        (
            display.frame.x + 10,
            display.frame.y + display.frame.h - hand_mirror_window.frame.h - 10,
        ),  # Bottom-left
        (
            display.frame.x + display.frame.w - hand_mirror_window.frame.w - 10,
            display.frame.y + display.frame.h - hand_mirror_window.frame.h - 10,
        ),  # Bottom-right
    ]

    new_x, new_y = quadrant_positions[new_quadrant]

    # Move the window to the new position
    call_yabai(f"-m window {hand_mirror_window.id} --move abs:{new_x}:{new_y}")

    # call_yabai(f"-m window {hand_mirror_window.id} --move abs:0:0")


@app.command()
def ssa():
    """Take a screenshot of the active window and copy it to the clipboard."""
    from PIL import Image
    import io

    # So if I'm calle from Alfred, the Alfred window might still be on top,
    # I can eithe rlook for it, or just sleep, for 0.1s, lets do that
    time.sleep(0.1)

    dimensions = get_foreground_window_dimensions()
    ic(dimensions)
    if dimensions:
        print(
            f"Active window dimensions: x={dimensions[0]}, y={dimensions[1]}, width={dimensions[2]}, height={dimensions[3]}"
        )
    else:
        print("No active window found")
        return

    screenshots_dir = Path.home() / "tmp" / "screenshots"
    ensure_directory_exists(screenshots_dir)

    path = screenshots_dir / "latest.webp"
    current_time = datetime.now().strftime("%Y%m%d_%H_%M_%S_%f")[
        :-5
    ]  # Get current time with milliseconds
    screenshot_path = screenshots_dir / f"screenshot_{current_time}.webp"

    # Capture the screenshot as PNG in memory
    png_data = capture_foreground_window_to_memory()

    # Convert PNG to WebP
    with Image.open(io.BytesIO(png_data)) as img:
        img.save(str(screenshot_path), "WEBP")
        img.save(str(path), "WEBP")

    print(f"Screenshot saved to: {screenshot_path}")

    # Load the WebP image and copy to clipboard
    img = AppKit.NSImage.alloc().initWithContentsOfFile_(str(path))
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


@app.command()
def ghimgpaste(caption: str = ""):
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
    error = os.system(f"pngpaste {png_path}")
    if error:
        print("[red]Error: No image found in clipboard[/red]")
        return

    # Convert to webp
    convert_result = os.system(f"magick {png_path} {webp_path}")
    if convert_result != 0:
        print("[red]Error converting image to webp[/red]")
        return

    # Clean up PNG
    png_path.unlink()

    # Git operations with error checking
    try:
        os.system(f"cd {iclip_dir} && git fetch && git pull --rebase")
        os.system(f"cd {iclip_dir} && git add {current_time}.webp")
        os.system(f'cd {iclip_dir} && git commit -m "adding image {current_time}.webp"')
        push_result = os.system(f"cd {iclip_dir} && git push")

        if push_result != 0:
            print("[red]Error pushing to repository. Check your git credentials.[/red]")
            return

        # Generate markdown and copy to clipboard
        template = f"![{caption}](https://raw.githubusercontent.com/idvorkin/ipaste/main/{current_time}.webp)"
        pyperclip.copy(template)
        print("[green]Successfully uploaded and copied markdown to clipboard![/green]")
        print(template)

    except Exception as e:
        print(f"[red]Error during git operations: {str(e)}[/red]")


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
    commands = [c.callback.__name__.replace("_", "-") for c in app.registered_commands]  # type:ignore
    items = [AlfredItems.Item(title=c, subtitle=c, arg=c) for c in commands]
    alfred_items = AlfredItems(items=items)
    json_output = alfred_items.model_dump_json(indent=4)

    # Save to cache for future use
    save_commands_cache(json_output)

    print(json_output)


@app.command()
def zz():
    """Put the system to sleep"""
    os.system("pmset sleepnow")


@app.command()
def flow_go():
    """Start the Flow application or service."""
    subprocess.run(["osascript", "-e", 'tell application "Flow" to start'])
    print("Flow started")


@app.command()
def flow_stop():
    """Stop the Flow application or service."""
    subprocess.run(["osascript", "-e", 'tell application "Flow" to stop'])
    print("Flow stopped")
    flow_show()  # Call flow_show to display the status


@app.command()
def flow_show():
    """Show the current status of the Flow application or service."""
    subprocess.run(["osascript", "-e", 'tell application "Flow" to show'])
    print("Flow status displayed")


@app.command()
def flow_info(oneline: bool = False):
    """Get remaining time, current phase, and session title from Flow."""
    time_result = subprocess.run(
        ["osascript", "-e", 'tell application "Flow" to getTime'],
        capture_output=True,
        text=True,
    )
    phase_result = subprocess.run(
        ["osascript", "-e", 'tell application "Flow" to getPhase'],
        capture_output=True,
        text=True,
    )
    title_result = subprocess.run(
        ["osascript", "-e", 'tell application "Flow" to getTitle'],
        capture_output=True,
        text=True,
    )

    if (
        time_result.returncode == 0
        and phase_result.returncode == 0
        and title_result.returncode == 0
    ):
        phase = phase_result.stdout.strip()
        time_remaining = time_result.stdout.strip()
        if oneline:
            if phase == "Break":
                print(f"Break [{time_remaining}]")
            else:
                print(f"{title_result.stdout.strip()} [{time_remaining}]")
        else:
            print(f"Remaining Time: {time_result.stdout.strip()}")
            print(f"Current Phase: {phase_result.stdout.strip()}")
            print(f"Session Title: {title_result.stdout.strip()}")
    else:
        print("Failed to retrieve information from Flow.")


@app.command()
def flow_get_title():
    """Get the session title from Flow."""
    title_result = subprocess.run(
        ["osascript", "-e", 'tell application "Flow" to getTitle'],
        capture_output=True,
        text=True,
    )

    if title_result.returncode == 0:
        print(f"Session Title: {title_result.stdout.strip()}")
    else:
        print("Failed to retrieve session title from Flow.")


@app.command()
def flow_rename(title: str):
    """Rename the Flow session."""
    subprocess.run(
        ["osascript", "-e", f'tell application "Flow" to setTitle to "{title}"']
    )
    print(f"Flow session renamed to '{title}'")
