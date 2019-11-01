from datetime import datetime, timedelta
from os import path, system, chdir
import sys

sys.path.append(f"{path.expanduser('~')}/gits/settings")


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

    # print(f"output: {fileName}")
    # print(f"template: {templateFileName}")
    # print(fileName)

    chdir(f"{pathBasedAtIgor2(directory)}")
    system(f"git fr")
    try:
        import vim
        vim.command("next " + fileName)
        vim.command("lcd %:p:h")  # Goto current Directory
        vim.command("9999")  # Goto last line.
    except ImportError:
        print("VIM not found")
    return fileName, directory


def NowPST():
    # groan no timzone in standard library so fake it - subtract nope, don't want to
    return datetime.now()


def MakeDailyPage(daysoffset=0):
    return MakeTemplatePage(NowPST()+timedelta(days=daysoffset), "750words", "daily_template")


def WCDailyPage():
    system(f'wc -w {MakeDailyPage()[0]}')

def GitCommitDailyPage():
    # f = "/".join(MakeDailyPage()[0].split("/")[-2::])
    f = MakeDailyPage()[0]
    git_cmd = (f'git add {f}')
    print (git_cmd)
    system(git_cmd)
    git_cmd = (f'git commit {f} -m 750')
    print (f"GIT: {git_cmd} EOL")
    system(git_cmd)

def MakeWeeklyReport():
    now = NowPST()
    startOfWeek = now - timedelta(days=now.weekday())

    # Make to sart of week.
    return MakeTemplatePage(startOfWeek, "week_report", "week_template")
