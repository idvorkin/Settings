#!python3

from datetime import datetime
import time
import typer
import subprocess
from subprocess import CompletedProcess
from pathlib import Path
from icecream import ic
import json
from typing import List
from pydantic import BaseModel, Field
import pyperclip
import Quartz
import Quartz.CoreGraphics as CG
import AppKit
import math
import os

_ = """

~/settings/config/yabai/yabairc

"""


app = typer.Typer(help="A Yabai helper", no_args_is_help=True)

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
    current_time = datetime.now().strftime("%Y%m%d_%H_%M_%S_%f")[:-5]  # Get current time with milliseconds
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

    iclip_dir = Path("~/gits/ipaste").expanduser()
    ensure_directory_exists(iclip_dir)
    current_time = datetime.now().strftime("%Y%m%d_%H%M%S")
    error = os.system(f"pngpaste {iclip_dir}/{current_time}.png")
    if error:
        print("Error pasting image")
        return

    os.system(f"convert {iclip_dir}/{current_time}.png {iclip_dir}/{current_time}.webp")
    os.system(f"rm {iclip_dir}/{current_time}.png")
    os.system(f"cd {iclip_dir} && git fetch && git rebase")
    os.system(f"cd {iclip_dir} && git add {current_time}.webp")
    os.system(f"cd {iclip_dir} && git commit -m 'adding image {current_time}.webp'")
    # do a push
    os.system(f"cd {iclip_dir} && git push")
    # Make a markdown include and write it to the clipboard
    template = f"![{caption}](https://raw.githubusercontent.com/idvorkin/ipaste/main/{current_time}.webp)"
    # put this on the clipboard
    pyperclip.copy(template)
    print(template)


class AlfredItems(BaseModel):
    class Item(BaseModel):
        title: str
        subtitle: str
        arg: str

    items: List[Item]


@app.command()
def alfred():
    """Generate JSON output of all commands for Alfred workflow integration"""
    #  Build a json of commands to be called from an alfred plugin workflow
    # start by reflecting to find all commands in app.
    # all_commands = app.
    commands = [c.callback.__name__.replace("_", "-") for c in app.registered_commands]  # type:ignore
    items = [AlfredItems.Item(title=c, subtitle=c, arg=c) for c in commands]
    alfred_items = AlfredItems(items=items)
    print(alfred_items.model_dump_json(indent=4))


if __name__ == "__main__":
    app()
