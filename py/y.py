#!python3

import typer
import subprocess
from pathlib import Path

app = typer.Typer(help="A Yabai helper")

_ = """

~/settings/config/yabai/yabairc

"""


def call_yabi(prompt):
    yabi_root = Path("~/homebrew/bin/yabai/").expanduser()

    # Split the prompt to run the path
    prompt_parts = prompt.split()
    command = [str(yabi_root)] + prompt_parts

    # Run the command in the remote shell
    try:
        out = subprocess.run(command, check=True, capture_output=True, text=True)
        print(out.stdout)
    except subprocess.CalledProcessError as e:
        print(f"An error occurred: {e.stderr}")


@app.command()
def hflip():
    call_yabi("-m space --mirror y-axis")


@app.command()
def restart():
    call_yabi("--restart-service")


@app.command()
def start():
    call_yabi("--start-service")


def stop():
    call_yabi("--stop-service")


if __name__ == "__main__":
    app()
