#!python3.12

from __future__ import annotations
import typer
import subprocess
from subprocess import CompletedProcess
from pathlib import Path
from icecream import ic
import json
from typing import List
from pydantic import BaseModel

_ = """

~/settings/shared/aerospace.toml

"""

app = typer.Typer(help="A Aerospace helper", no_args_is_help=True)

# AI Coding pro-tip, pump json into AI, and ask it to generate the pydantic types for you
# Then use typing to avoid the long pain of type errors


def call_aerospace(prompt: str | List[str]) -> CompletedProcess:
    yabi_root = Path("~/homebrew/bin/aerospace/").expanduser()

    # If prompt is a string, split it into parts
    if isinstance(prompt, str):
        prompt_parts = prompt.split()
    elif isinstance(prompt, list):
        prompt_parts = prompt
    else:
        raise ValueError("Prompt must be a string or a list of strings.")

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
def mleft():
    call_aerospace("move left")
    pass


@app.command()
def mright():
    call_aerospace("move right")
    pass


@app.command()
def swest():
    send_key(123)


@app.command()
def seast():
    send_key(124)


@app.command()
def fleft():
    call_aerospace("focus left")


@app.command()
def fright():
    call_aerospace("focus right")


@app.command()
def ftoggle():
    call_aerospace("focus-back-and-forth")


@app.command()
def reload():
    call_aerospace("reload-config")


@app.command()
def reset():
    call_aerospace("balance-sizes")


@app.command()
def start():
    # call_yabai("--start-service")
    pass


@app.command()
def stop():
    # call_yabai("--stop-service")
    pass


@app.command()
def rotate():
    # call_yabai("-m space --rotate 90")
    pass


@app.command()
def accordian():
    call_aerospace("layout accordion")


@app.command()
def zoom():
    call_aerospace("layout tiles accordion")


@app.command()
def tile():
    call_aerospace("layout tiles")


@app.command()
def close():
    call_aerospace("close")


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


class DisplayResolution(BaseModel):
    width: int
    height: int
    scaling_factor: float


def get_display_resolution() -> DisplayResolution:
    from Quartz import (
        CGDisplayBounds,
        CGDisplayPixelsWide,
        CGDisplayPixelsHigh,
        CGMainDisplayID,
    )

    display_id = CGMainDisplayID()
    width = CGDisplayPixelsWide(display_id)
    height = CGDisplayPixelsHigh(display_id)
    bounds = CGDisplayBounds(display_id)

    # Calculate the scaling factor
    scaling_factor = width / bounds.size.width

    return DisplayResolution(width=width, height=height, scaling_factor=scaling_factor)


@app.command()
def half():
    resolution = get_display_resolution()
    new_width = resolution.width // 2
    call_aerospace(f"resize smart {new_width}")


@app.command()
def third():
    resolution = get_display_resolution()
    new_width = resolution.width // 3
    call_aerospace(f"resize smart {new_width}")


@app.command()
def t1():
    third()


@app.command()
def t2():
    resolution = get_display_resolution()
    new_width = int(resolution.width * 0.66)
    call_aerospace(f"resize smart {new_width}")


@app.command()
def warp1():
    # Make left window 1/3 of the screen
    call_aerospace("move-node-to-workspace 1")


@app.command()
def warp2():
    # Make left window 1/3 of the screen
    call_aerospace("move-node-to-workspace 2")


@app.command()
def warp3():
    # Make left window 1/3 of the screen
    call_aerospace("move-node-to-workspace 3")


@app.command()
def warp4():
    # Make left window 1/3 of the screen
    call_aerospace("move-node-to-workspace 4")


@app.command()
def w1():
    call_aerospace("workspace 1")


@app.command()
def w2():
    call_aerospace("workspace 2")


@app.command()
def w3():
    call_aerospace("workspace 3")


@app.command()
def w4():
    call_aerospace("workspace 3")


@app.command()
def debug():
    # ic(get_displays())
    # ic([w for w in get_windows().windows])
    print("workspaces")
    print(call_aerospace("list-workspaces --empty no --monitor all").stdout)

    print("active workspaces")
    print(call_aerospace("list-workspaces --focused").stdout)

    print("monitors")
    print(call_aerospace("list-monitors").stdout)

    print("windows: {monitor}|{workspace}|{window}")
    list_windows = [
        "list-windows",
        "--all",
        "--format",
        "%{monitor-id} | %{workspace} | %{window-id}%{right-padding} | %{app-name}%{right-padding} | %{window-title} ",
    ]

    print(call_aerospace(list_windows).stdout)
    ic(get_displays())


class AlfredItems(BaseModel):
    class Item(BaseModel):
        title: str
        subtitle: str
        arg: str

    items: List[Item]


@app.command()
def zzfocus(window):
    call_aerospace(f"focus --window-id {window}")


@app.command()
def alfred_windows():
    list_windows = [
        "list-windows",
        "--all",
        "--format",
        "%{monitor-id} : %{workspace} : %{window-id}:   %{app-name} : %{window-title} ",
    ]

    out = call_aerospace(list_windows).stdout
    items: list[AlfredItems.Item] = []

    def focus_command(window_id):
        # recall this will be sent to a.py
        return f"zzfocus {window_id}"

    for line in out.split("\n"):
        if len(line.split(":")) < 3:
            continue
        monitor = line.split(":")[0]
        workspace = line.split(":")[1]
        window_id = line.split(":")[2]
        app = "".join(line.split(":")[3:])
        items.append(
            AlfredItems.Item(
                title=app,
                subtitle=f"{monitor}:{workspace}",
                arg=focus_command(window_id),
            )
        )
        ic(monitor, workspace, window_id, app)

    print(AlfredItems(items=items).model_dump_json(indent=4))


@app.command()
def keybindings():
    print("https://nikitabobko.github.io/AeroSpace/guide#default-config")
    print("Zoom Toggle: Alt-, (a zoom is better) ")
    print("Focus Left/Right: alt + hl: ")
    print("Goto workspace N:  alt+N: ")
    print("Warp to workspace N:  alt+shift+N: ")
    print("Grow Shrink:  alt+shift+/alt+shift-")
    pass


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
