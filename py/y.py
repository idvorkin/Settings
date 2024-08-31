#!python3.11

import typer
import subprocess
from subprocess import CompletedProcess
from pathlib import Path
from icecream import ic
import json
from typing import List
from pydantic import BaseModel, Field
import pyperclip

_ = """

~/settings/config/yabai/yabairc

"""

app = typer.Typer(help="A Yabai helper", no_args_is_help=True)

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
    call_yabai("-m space --mirror y-axis")


@app.command()
def swest():
    send_key(123)


@app.command()
def seast():
    send_key(124)


@app.command()
def fleft():
    call_yabai("-m window --focus west")


@app.command()
def fup():
    call_yabai("-m window --focus north")


@app.command()
def fdown():
    call_yabai("-m window --focus south")


@app.command()
def fright():
    call_yabai("-m window --focus east")


@app.command()
def restart():
    call_yabai("--restart-service")


@app.command()
def start():
    call_yabai("--start-service")


@app.command()
def stop():
    call_yabai("--stop-service")


@app.command()
def rotate():
    call_yabai("-m space --rotate 90")


@app.command()
def zoom():
    call_yabai("-m window --toggle zoom-fullscreen")


@app.command()
def close():
    call_yabai("-m window --close")


@app.command()
def cycle():
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
    set_width(get_left_most_window(), 0.3)


@app.command()
def half():
    set_width(get_left_most_window(), 0.5)


@app.command()
def t2():
    set_width(get_left_most_window(), 2 / 3)


@app.command()
def debug():
    ic(get_displays())
    ic([w for w in get_windows().windows])


## I'm going to add new helpful commands - tbd the right file for them


@app.command()
def ghimgpaste(caption: str = ""):
    from datetime import datetime
    import os

    iclip_dir = "~/gits/ipaste"
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
    template = f"![{caption}](https://github.com/idvorkin/ipaste/blob/raw/master/{current_time}.webp)"
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
    #  Build a json of commands to be called from an alfred plugin workflow
    # start by reflecting to find all commands in app.
    # all_commands = app.
    commands = [c.callback.__name__.replace("-", "_") for c in app.registered_commands]  # type:ignore
    items = [AlfredItems.Item(title=c, subtitle=c, arg=c) for c in commands]
    alfred_items = AlfredItems(items=items)
    print(alfred_items.model_dump_json(indent=4))


if __name__ == "__main__":
    app()
