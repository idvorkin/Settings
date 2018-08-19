import sys
import vim
from datetime import datetime
from os import path
import pathlib


def MakeDailyPage():
    # Todo make this be path relative so works on unix.
    date_in_format = datetime.now().strftime("%Y-%m-%d")
    fileName = path.join(
        path.expanduser("~"), "gits/igor2/750words/" + date_in_format + ".md"
    )
    if path.isfile(fileName):
        print(fileName + " exists")
    else:
        print(fileName)
        with open(fileName, "w") as f:
            f.write("750 words for:" + date_in_format + "\n\n\n\n")
    vim.command("next " + fileName)
    vim.command("lcd %:p:h")  # Goto current Directory
    vim.command("IGWriteOn")
    vim.command("Gwrite")
    vim.command("999999")  # Goto last line.
