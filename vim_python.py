import vim
from datetime import datetime, timedelta
from os import path


def pathBasedAtIgor2(filepath):
    return path.join(path.expanduser("~"), f"gits/igor2/{filepath}")


def MakeTemplatePage(date, directory, template_name):
    date_in_format = date.strftime("%Y-%m-%d")
    fileName = pathBasedAtIgor2(f"/{directory}/{date_in_format}.md")
    templateFileName = pathBasedAtIgor2(f"/{directory}/{template_name}.md")
    isAlreadyCreated = path.isfile(fileName)

    if isAlreadyCreated:
        print(fileName + " exists")
    else:
        with open(templateFileName, "r") as fileTemplate:
            content = fileTemplate.read()
            content = content.replace("!template_date!", date_in_format)
            with open(fileName, "w") as fileWrite:
                fileWrite.write(content)

        print(f"output: {fileName}")
        print(f"template: {templateFileName}")
        print(fileName)

    vim.command("next " + fileName)
    vim.command("lcd %:p:h")  # Goto current Directory
    vim.command("IGWriteOn")
    vim.command("Gwrite")

    vim.command("9999")  # Goto last line.
    return


def NowPST():
    # groan no timzone in standard library so fake it - subtract nope, don't want to
    # subtract as it won't work for non standard times! Cripes - how annoying.
    # return datetime.now().astimezone(timezone("US/Pacific"))
    return datetime.now()


def MakeDailyPage(daysoffset=0):
    MakeTemplatePage(NowPST()+timedelta(days=daysoffset), "750words", "daily_template")


def MakeWeeklyReport():
    now = NowPST()
    startOfWeek = now - timedelta(days=now.weekday())

    # Make to sart of week.
    MakeTemplatePage(startOfWeek, "week_report", "week_template")
