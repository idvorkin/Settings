import sys
import vim
from datetime import datetime
from os import path
import pathlib
import shutil


def MakeDailyPage():
    # Todo make this be path relative so works on unix.
    date_in_format = datetime.now().strftime("%Y-%m-%d")
    fileName = path.join(
        path.expanduser("~"), f"gits/igor2/750words/{date_in_format}.md"
    )
    templateFileName  = path.join(
        path.expanduser("~"), "gits/igor2/750words/daily_template.md"
    )
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
