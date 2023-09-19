#!python3

import subprocess

import subprocess
import urllib.parse
import typer
from icecream import ic
import pyperclip


app = typer.Typer(help="A helper to move my daily todos into omnifocus")

def add_task_to_omnifocus(task):

    if "☑" in task or "CUT" in task:
       # task completed, return early
       return

    if len(task) < 3:
       # too short, bug
       return


    # remove task markers, and markdown headers
    task = task.replace("☐", "")
    task = task.replace("1.", "")

    tags= ""
    if "work" in task.lower():
        tags="work"
        # remove work from original string, regardless of case
        import re;
        task = re.sub('work', '', task, flags=re.IGNORECASE)

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
    ic(url)
    subprocess.run(['open', url], check=True)

@app.command()
def debug():
    clipboard_content = pyperclip.paste()
    lines = clipboard_content.split('\n')
    lines = list(set(lines))
    if  len(lines) > 15:
        print("Probably a bug you have too much clipboard")
        return

    for line in lines:
        ic(line)
        add_task_to_omnifocus(line)

if __name__ == "__main__":
    app()
