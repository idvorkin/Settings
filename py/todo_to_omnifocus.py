#!python3

import subprocess
import urllib.parse
import typer
from icecream import ic
import pyperclip
from typing_extensions import Annotated
import re


app = typer.Typer(help="A helper to move my daily todos into omnifocus")


def fixup_task(task):
    # remove task markers, and markdown headers
    task = task.replace("☐", "")
    # replace start of line with a {number}. to nothing, e.g. 1., or 2. or 3.
    task = re.sub(r"^\s*\d+\.\s*", "", task)

    if "☑" in task or "CUT" in task:
        # task completed, return early
        return ""

    return task


def add_task_to_omnifocus(task):
    if len(task) < 3:
        # too short, bug
        return

    tags = ""
    if "work" in task.lower():
        tags = "work"
        # remove work from original string, regardless of case
        import re

        task = re.sub("work:", "", task, flags=re.IGNORECASE)
        task = re.sub("work", "", task, flags=re.IGNORECASE)

    params = {
        "name": task,
        "autosave": "true",
        "flag": "true",
        "project": "today",
        "tags": tags,
    }

    str_parms = urllib.parse.urlencode(params, quote_via=urllib.parse.quote)
    ic(str_parms)

    url = "omnifocus:///add?" + str_parms
    ic("Running", url)
    subprocess.run(["open", url], check=True)


@app.command()
def debug(print_only: Annotated[bool, typer.Option] = typer.Option(False)):
    ic(print_only)
    clipboard_content = pyperclip.paste()
    lines = clipboard_content.split("\n")
    lines = list(set(lines))
    if len(lines) > 15:
        print("Probably a bug you have too much clipboard")
        return

    for line in lines:
        ic(line)
        task = fixup_task(line)
        ic(task)
        if not print_only:
            add_task_to_omnifocus(task)


if __name__ == "__main__":
    app()
