#!uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
# ]
# ///

from datetime import datetime, timedelta
from os import path, chdir
import typer
from pathlib import Path
import random
import subprocess
from zoneinfo import ZoneInfo

app = typer.Typer(no_args_is_help=True)


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
    import sys

    cmd = "ssh lightsail_no_forward /home/ec2-user/.local/bin/vim_python "
    # Print the cmd to stderr
    # print(cmd + commands, file=sys.stderr)
    out = subprocess.run(cmd + commands, shell=True, capture_output=True)
    if out.returncode != 0:
        print("stderr:\n", out.stderr.decode(), file=sys.stderr)


@app.command()
def MakeDailyPage(
    date_input: str = typer.Argument(
        None, help="Date input: number (days offset), YYYY-MM-DD, or MM-DD format"
    ),
    remote: bool = typer.Option(False, help="Create the page remotely"),
):
    """
    Create a daily page for a date.
    If no date is provided, creates a page for today.
    """
    # Handle the date input
    if date_input is None:
        target_date = NowPST()
    else:
        # Check if input is a number (days offset)
        try:
            days_offset = int(date_input)
            # Negative the offset to go backwards in time for positive numbers
            target_date = NowPST() - timedelta(days=days_offset)
        except ValueError:
            # Not a number, try parsing as a date
            try:
                # Try full date format first
                target_date = datetime.strptime(date_input, "%Y-%m-%d")
            except ValueError:
                try:
                    # Try month-day format, defaulting to current year
                    current_year = NowPST().year
                    target_date = datetime.strptime(
                        f"{current_year}-{date_input}", "%Y-%m-%d"
                    )
                except ValueError:
                    print(
                        "Error: date must be a number (days offset), YYYY-MM-DD, or MM-DD format"
                    )
                    return

    # Create the file locally first to get the path
    new_file, directory = MakeTemplatePage(target_date, "750words", "daily_template")

    if remote:
        # For remote calls, use the same input format
        cmd = f"makedailypage {date_input}" if date_input else "makedailypage"
        import sys

        result = subprocess.run(
            f"ssh lightsail_no_forward /home/ec2-user/.local/bin/vim_python {cmd}",
            shell=True,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            # Print the remote command's output to stderr
            if result.stdout:
                print(f"Remote output: {result.stdout.strip()}", file=sys.stderr)
            print(LocalToRemote(new_file))
        else:
            print(f"Remote command failed with error: {result.stderr}", file=sys.stderr)
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
def MakeWeeklyReport(
    weekoffset: int = typer.Argument(
        0,
        help="Week offset: number of weeks from current week (negative for past weeks)",
    ),
    remote: bool = typer.Option(False, help="Create the weekly report remotely"),
):
    """
    Create a weekly report for a specific week.
    If no week offset is provided, creates a report for the current week.
    """
    now = NowPST()
    startOfWeek = now - timedelta(days=now.weekday()) + timedelta(days=weekoffset * 7)

    if remote:
        make_remote_call(f"makeweeklyreport {weekoffset}")
        # Calculate the expected file path on the remote server
        date_str = startOfWeek.strftime("%Y-%m-%d")
        remote_file = f"scp://ec2-user@lightsail//home/ec2-user/gits/igor2/week_report/{date_str}.md"
        print(remote_file)
    else:
        # Make to start of week.
        new_file, directory = MakeTemplatePage(
            startOfWeek, "week_report", "week_template"
        )
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


@app.command()
def WCDailyPage():
    """
    Create a daily page for today and open it for word count.
    This function is used by the 'dwc' alias.
    """
    target_date = NowPST()
    new_file, directory = MakeTemplatePage(target_date, "750words", "daily_template")
    print(new_file)


@app.command()
def GitCommitDailyPage():
    """
    Create a daily page for today and prepare it for git commit.
    This function is used by the 'dgc' alias.
    """
    target_date = NowPST()
    new_file, directory = MakeTemplatePage(target_date, "750words", "daily_template")

    # Change to the directory containing the file
    chdir(path.dirname(new_file))

    # Add the file to git
    try:
        subprocess.run(["git", "add", path.basename(new_file)], check=True)
        print(f"Added {path.basename(new_file)} to git staging area")
    except subprocess.CalledProcessError as e:
        print(f"Error adding file to git: {e}")

    print(new_file)


if __name__ == "__main__":
    app()
