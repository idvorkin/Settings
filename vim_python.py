import vim
from datetime import datetime, timedelta
from os import path
import shutil


def pathBasedAtIgor2(filepath):
    return path.join(
        path.expanduser("~"), f"gits/igor2/{filepath}"
    )


def MakeTemplatePage(date, directory, template_name):
    date_in_format = date.strftime("%Y-%m-%d")
    fileName = pathBasedAtIgor2(f"/{directory}/{date_in_format}.md")
    templateFileName = pathBasedAtIgor2(f"/{directory}/{template_name}.md")
    isAlreadyCreated = path.isfile(fileName)

    if isAlreadyCreated:
        print(fileName + " exists")
    else:
        print(f"output: {fileName}")
        print(f"template: {templateFileName}")
        print(fileName)
        shutil.copyfile(templateFileName, fileName)

    vim.command("next " + fileName)
    vim.command("lcd %:p:h")  # Goto current Directory
    vim.command("IGWriteOn")
    vim.command("Gwrite")

    if not isAlreadyCreated:
        vim.command(f"%s/!template_date!/{date_in_format}/")  # Goto first line

    vim.command("9999")  # Goto last line.
    return


def MakeDailyPage():
    MakeTemplatePage(datetime.now(), "750words", "daily_template")


def MakeWeeklyReport():
    now = datetime.now().date()
    startOfWeek = now - timedelta(days=now.weekday())

    # Make to sart of week.
    MakeTemplatePage(startOfWeek, "week_report", "week_template")
