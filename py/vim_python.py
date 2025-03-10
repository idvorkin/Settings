#!python3.11

from datetime import datetime, timedelta
from os import path, chdir
import typer
from pathlib import Path
import random
import subprocess
from zoneinfo import ZoneInfo

app = typer.Typer()


def pathBasedAtIgor2(filepath):
    return path.join(path.expanduser("~"), f"gits/igor2/{filepath}")


def MakeTemplatePage(date, directory, template_name):
    date_in_format = date.strftime("%Y-%m-%d")
    fileName = pathBasedAtIgor2(f"/{directory}/{date_in_format}.md")
    templateFileName = pathBasedAtIgor2(f"/{directory}/{template_name}.md")
    isAlreadyCreated = path.isfile(fileName)

    if isAlreadyCreated:
        pass
        # print(fileName + " exists")
    else:
        with open(templateFileName, "r") as fileTemplate:
            content = fileTemplate.read()
            content = content.replace("!template_date!", date_in_format)
            with open(fileName, "w") as fileWrite:
                fileWrite.write(content)

    # print(f"output: {fileName}")
    # print(f"template: {templateFileName}")
    # print(fileName)

    chdir(f"{pathBasedAtIgor2(directory)}")
    try:
        # import vim
        # vim.command("next " + fileName)
        # vim.command("lcd %:p:h")  # Goto current Directory
        # vim.command("9999")  # Goto last line.
        pass

    except ImportError:
        print("VIM not found")
    return fileName, directory


def NowPST():
    return datetime.now(ZoneInfo("America/Los_Angeles"))


def LocalToRemote(file):
    return file.replace(
        path.expanduser("~"), "scp://ec2-user@lightsail//home/ec2-user/"
    )


def make_remote_call(commands):
    cmd = "ssh lightsail_no_forward /home/linuxbrew/.linuxbrew/bin/python3.11 /home/ec2-user/settings/py/vim_python.py "
    # Print the cmd to stderr
    # print(cmd + commands, file=sys.stderr)
    out = subprocess.run(cmd + commands, shell=True, capture_output=True)
    if out.returncode != 0:
        print("stderr:\n", out.stderr)


@app.command()
def MakeDailyPage(
    daysoffset: int | None = None, date: str | None = None, remote: bool = False
):
    """
    Create a daily page for either an offset from today or a specific date.
    Only one of daysoffset or date can be provided.
    If neither is provided, defaults to today.
    If date is provided without year (e.g. "3-25"), defaults to current year.

    Args:
        daysoffset: Number of days offset from today (mutually exclusive with date)
        date: Date in YYYY-MM-DD or MM-DD format (mutually exclusive with daysoffset)
        remote: Whether to create the page remotely
    """
    if daysoffset is not None and date is not None:
        print("Error: Cannot specify both daysoffset and date")
        return

    if date is not None:
        try:
            # Try full date format first
            target_date = datetime.strptime(date, "%Y-%m-%d")
        except ValueError:
            try:
                # Try month-day format, defaulting to current year
                current_year = NowPST().year
                target_date = datetime.strptime(f"{current_year}-{date}", "%Y-%m-%d")
            except ValueError:
                print("Error: date must be in YYYY-MM-DD or MM-DD format")
                return
    else:
        target_date = NowPST() + timedelta(days=daysoffset or 0)

    new_file, directory = MakeTemplatePage(target_date, "750words", "daily_template")
    if remote:
        if date is not None:
            make_remote_call(f"makedailypage --date={date}")
        else:
            make_remote_call(f"makedailypage --daysoffset={daysoffset}")
        print(LocalToRemote(new_file))
    else:
        print(new_file)


@app.command()
def RandomBlogPost():
    blog_path = Path.home() / Path("blog")
    files = []
    files.extend(list(blog_path.glob("_posts/*md")))
    files.extend(list(blog_path.glob("_d/*md")))
    list(blog_path.glob("_td/*md"))
    random_post = random.choice(files)
    print(random_post)


@app.command()
def MakeWeeklyReport(weekoffset: int = 0, remote: bool = False):
    now = NowPST()
    startOfWeek = now - timedelta(days=now.weekday()) + timedelta(days=weekoffset * 7)

    # Make to sart of week.
    new_file, path = MakeTemplatePage(startOfWeek, "week_report", "week_template")
    if remote:
        make_remote_call(f"makeweeklyreport --weekoffset={weekoffset}")
        print(LocalToRemote(new_file))
    else:
        print(new_file)


@app.command()
def profile_io(n: int = 3):
    """Write a temporary file and read it back, and print the timings"""
    import time
    import tempfile

    for i in range(n + 1):
        print("Starting", i)
        start_time = time.time()
        with tempfile.NamedTemporaryFile(delete=False) as tmp_file:
            tmp_file.write(b"This is a test file")
            tmp_file_path = tmp_file.name
        write_time = time.time()

        # Read and verify content to measure actual read operation
        with open(tmp_file_path, "r") as file:
            content = file.read()
            assert content == "This is a test file"

        read_time = time.time()

        write_duration = write_time - start_time
        read_duration = read_time - write_time
        print(f"{i}: Write/Read:  {write_duration:.5f}/{read_duration:.5f} seconds")


@app.command()
def make_convo():
    import os
    import shutil
    import tempfile

    # Create a temporary file
    temp_file, temp_path = tempfile.mkstemp(suffix=".convo.md")

    try:
        # Open the temporary file in write mode
        with open(temp_path, "w") as temp:
            # Open the source file in read mode
            with open(
                os.path.expanduser("~/gits/nlp/convos/default.convo.md"), "r"
            ) as source:
                # Copy the content of the source file into the temporary file
                shutil.copyfileobj(source, temp)

    finally:
        # Close the temporary file
        os.close(temp_file)
    print(temp_path)


if __name__ == "__main__":
    app()
