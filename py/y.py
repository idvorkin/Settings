#!python3.11

import typer
import subprocess
from pathlib import Path

# from icecream import ic
import json

app = typer.Typer(help="A Yabai helper")

_ = """

~/settings/config/yabai/yabairc

"""


def call_yabai(prompt):
    yabi_root = Path("~/homebrew/bin/yabai/").expanduser()

    # Split the prompt to run the path
    prompt_parts = prompt.split()
    command = [str(yabi_root)] + prompt_parts

    # Run the command in the remote shell
    try:
        out = subprocess.run(command, check=True, capture_output=True, text=True)
        print(out.stdout)
        return out
    except subprocess.CalledProcessError as e:
        print(f"An error occurred: {e.stderr}")


@app.command()
def hflip():
    call_yabai("-m space --mirror y-axis")


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


@app.command()
def alfred():
    #  Build a json of commands to be called from an alfred plugin workflow
    # start by reflecting to find all commands in app.
    # all_commands = app.
    commands = [c.callback.__name__.replace("-", "_") for c in app.registered_commands]
    # ic(commands)
    dicts = {"items": [{"title": c, "subtitle": c, "arg": c} for c in commands]}
    # output json to stdout
    print(json.dumps(dicts, indent=4))


if __name__ == "__main__":
    app()
