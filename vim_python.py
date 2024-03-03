#!python3

from datetime import datetime, timedelta
from os import path, chdir
import typer
from pathlib import Path
import random
import subprocess

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
    # groan no timzone in standard library - make sure to install pytz.
    from pytz import timezone

    return datetime.now().astimezone(timezone("US/Pacific"))


def LocalToRemote(file):
    return file.replace(
        path.expanduser("~"), "scp://ec2-user@lightsail//home/ec2-user/"
    )


def make_remote_call(commands):
    cmd = "ssh lightsail_no_forward python3 /home/ec2-user/settings/vim_python.py "
    # execute the commamnd in the shell
    subprocess.run(cmd + commands, shell=True)


@app.command()
def MakeDailyPage(daysoffset: int = 0, remote: bool = False):
    new_file, directory = MakeTemplatePage(
        NowPST() + timedelta(days=daysoffset), "750words", "daily_template"
    )
    if remote:
        make_remote_call(f"makedailpage {daysoffset}")
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
        make_remote_call(f"makedailpage {weekoffset}")
        print(LocalToRemote(new_file))
    else:
        print(new_file)


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
