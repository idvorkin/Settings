#!python3

import subprocess
import urllib.parse
import typer
from icecream import ic
import pyperclip
from typing_extensions import Annotated
import re
import json
from enum import Enum
from difflib import SequenceMatcher


app = typer.Typer(
    help="OmniFocus CLI - Manage OmniFocus from the command line",
    rich_markup_mode="rich",
    no_args_is_help=True
)


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

    try:
        subprocess.run(["open", url], check=True)
    except subprocess.CalledProcessError as e:
        print(f"Error: {e}")
        print(f"Failed to add task: {task}")
        print("Continuing")


@app.command()
def add_from_clipboard(
    print_only: Annotated[bool, typer.Option(help="Preview tasks without adding them")] = False
):
    """Add tasks from clipboard to OmniFocus (one task per line)"""
    ic(print_only)
    clipboard_content = pyperclip.paste()
    
    # Split into lines and clean up each line
    lines = [line.strip() for line in clipboard_content.split("\n") if line.strip()]
    
    # First clean up all tasks
    cleaned_tasks = []
    for line in lines:
        task = fixup_task(line)
        if task:  # Only include non-empty tasks
            cleaned_tasks.append(task)
    
    # Then deduplicate, keeping order
    seen = set()
    unique_tasks = []
    for task in cleaned_tasks:
        if task.lower() not in seen:  # Case-insensitive deduplication
            seen.add(task.lower())
            unique_tasks.append(task)
    
    if len(unique_tasks) > 25:
        print("Probably a bug you have too much clipboard")
        return
    
    # Print summary
    print(f"\nFound {len(lines)} lines, {len(cleaned_tasks)} valid tasks, {len(unique_tasks)} unique tasks")
    print("\nTasks to add:")
    print("-------------")
    
    for task in unique_tasks:
        print(f"• {task}")
        if not print_only:
            add_task_to_omnifocus(task)


class View(str, Enum):
    by_project = "by_project"
    by_tag = "by_tag"


@app.command()
def list_tasks(view: Annotated[View, typer.Option(help="Group tasks by 'by_project' or 'by_tag'")] = View.by_project):
    """List all tasks, grouped by either project or tag"""
    javascript = '''
        function run() {
            const of = Application('OmniFocus');
            of.includeStandardAdditions = true;
            
            const doc = of.defaultDocument;
            const tasks = doc.flattenedTasks.whose({completed: false})();
            
            const taskList = tasks.map(task => ({
                name: task.name(),
                project: task.containingProject() ? task.containingProject().name() : "No Project",
                flagged: task.flagged(),
                tags: Array.from(task.tags()).map(t => t.name()),
                dueDate: task.dueDate() ? task.dueDate().toISOString() : ""
            }));
            
            return JSON.stringify(taskList);
        }
    '''
    
    try:
        result = subprocess.run(
            ["osascript", "-l", "JavaScript", "-e", javascript],
            capture_output=True,
            text=True,
            check=True
        )
        tasks = json.loads(result.stdout)
        
        if view == View.by_project:
            # Group tasks by project
            groups = {}
            for task in tasks:
                project = task['project']
                if project not in groups:
                    groups[project] = []
                groups[project].append(task)
            
            # Sort projects by name, with "today" first if it exists
            sorted_groups = sorted(groups.keys())
            if "today" in sorted_groups:
                sorted_groups.remove("today")
                sorted_groups.insert(0, "today")
                
            print("\nTasks by Project:")
            print("================")
            
        else:  # by_tag
            # Group tasks by tag, tasks can appear multiple times if they have multiple tags
            groups = {"No Tags": []}  # Default group for untagged tasks
            for task in tasks:
                if not task['tags']:
                    groups["No Tags"].append(task)
                else:
                    for tag in task['tags']:
                        if tag not in groups:
                            groups[tag] = []
                        groups[tag].append(task)
            
            sorted_groups = sorted(groups.keys())
            print("\nTasks by Tag:")
            print("============")
        
        for group in sorted_groups:
            if group not in groups or not groups[group]:
                continue
                
            task_count = len(groups[group])
            print(f"\n{group} ({task_count}):")
            for task in groups[group]:
                project = f" ({task['project']})" if view == View.by_tag else ""
                flag = "🚩 " if task['flagged'] else ""
                tags = f" [{', '.join(task['tags'])}]" if view == View.by_project and task['tags'] else ""
                due = f" [Due: {task['dueDate'].split('T')[0]}]" if task['dueDate'] else ""
                print(f"  • {flag}{task['name']}{project}{tags}{due}")
            
    except subprocess.CalledProcessError as e:
        print(f"Error: {e}")
        print("Failed to get tasks")
        print("Make sure OmniFocus is running and Automation is enabled")


# Keep the old list_flagged as an alias for backward compatibility
@app.command(name="flagged")
def list_flagged():
    """Show all flagged tasks with their creation dates"""
    javascript = '''
        function run() {
            const of = Application('OmniFocus');
            of.includeStandardAdditions = true;
            
            const doc = of.defaultDocument;
            const tasks = doc.flattenedTasks.whose({completed: false, flagged: true})();
            
            const taskList = tasks.map(task => ({
                name: task.name(),
                project: task.containingProject() ? task.containingProject().name() : "No Project",
                tags: Array.from(task.tags()).map(t => t.name()),
                dueDate: task.dueDate() ? task.dueDate().toISOString() : "",
                creationDate: task.creationDate() ? task.creationDate().toISOString() : ""
            }));
            
            return JSON.stringify(taskList);
        }
    '''
    
    try:
        result = subprocess.run(
            ["osascript", "-l", "JavaScript", "-e", javascript],
            capture_output=True,
            text=True,
            check=True
        )
        tasks = json.loads(result.stdout)
        
        print("\nFlagged Tasks:")
        print("=============")
        for task in tasks:
            project = f" ({task['project']})"
            tags = f" [{', '.join(task['tags'])}]" if task['tags'] else ""
            due = f" [Due: {task['dueDate'].split('T')[0]}]" if task['dueDate'] else ""
            created = f" [Created: {task['creationDate'].split('T')[0]}]" if task['creationDate'] else ""
            print(f"• {task['name']}{project}{tags}{due}{created}")
            
    except subprocess.CalledProcessError as e:
        print(f"Error: {e}")
        print("Failed to get flagged tasks")
        print("Make sure OmniFocus is running and Automation is enabled")


def find_duplicates(tasks):
    """Find duplicate tasks based on name similarity"""
    # Filter out tasks that are just project names
    active_tasks = [task for task in tasks 
                   if task['name'] != task['project']  # Skip if task name matches project name
                   and task['name'].strip() != ""      # Skip empty names
                   and not task['completed']]          # Skip completed tasks
    
    duplicates = []
    for i, task1 in enumerate(active_tasks):
        for j, task2 in enumerate(active_tasks[i + 1:], i + 1):
            # Compare task names, ignoring case and extra whitespace
            similarity = SequenceMatcher(None, 
                                      task1['name'].lower().strip(), 
                                      task2['name'].lower().strip()).ratio()
            if similarity > 0.8:  # 80% similarity threshold
                duplicates.append((task1, task2, similarity))
    return duplicates


@app.command()
def show_duplicates():
    """Find and interactively review potential duplicate tasks"""
    applescript = '''
        tell application "OmniFocus"
            tell default document
                set taskList to {}
                repeat with t in (every flattened task where completed is false)
                    set taskName to name of t
                    set projectName to "No Project"
                    try
                        if containing project of t is not missing value then
                            set projectName to name of containing project of t
                        end if
                    end try
                    set taskId to id of t
                    set end of taskList to {|name|:taskName, project:projectName, id:taskId, completed:false}
                end repeat
                return taskList as text
            end tell
        end tell
    '''
    
    try:
        # First switch to All perspective
        subprocess.run(["open", "omnifocus:///perspective/All"], check=True)
        
        # Get all tasks
        result = subprocess.run(
            ["osascript", "-e", applescript],
            capture_output=True,
            text=True,
            check=True
        )
        tasks = json.loads(result.stdout)
        
        # Find duplicates
        duplicates = find_duplicates(tasks)
        
        if not duplicates:
            print("No potential duplicates found!")
            return
            
        print(f"\nFound {len(duplicates)} potential duplicate task sets.")
        print("I'll show you each set one at a time in OmniFocus.")
        print("(Make sure you're in the All perspective to see all tasks)")
        
        for i, (task1, task2, similarity) in enumerate(duplicates, 1):
            print(f"\nDuplicate Set {i} of {len(duplicates)} (Similarity: {similarity:.1%}):")
            print(f"• {task1['name']}")
            print(f"  In project: {task1['project']}")
            print(f"• {task2['name']}")
            print(f"  In project: {task2['project']}")
            
            # Use task IDs to create a more precise search
            url = f"omnifocus:///task/{task1['id']}"
            subprocess.run(["open", url], check=True)
            input("Press Enter to see the second task...")
            
            url = f"omnifocus:///task/{task2['id']}"
            subprocess.run(["open", url], check=True)
            
            choice = input("\nPress Enter after reviewing/closing the duplicate (or 'q' to quit): ").strip()
            if choice.lower() == 'q':
                print("Stopped reviewing duplicates.")
                break
                
    except subprocess.CalledProcessError as e:
        print(f"Error: {e}")
        print("Failed to get tasks")
        print("Make sure OmniFocus is running and Automation is enabled")


@app.command()
def close_task(task_name: str):
    """Mark a specific task as completed by searching for its name"""
    # First search for the task to show matches
    search_url = f"omnifocus:///search?q={urllib.parse.quote(task_name)}"
    subprocess.run(["open", search_url], check=True)
    
    # Ask which task to close
    print(f"\nOmniFocus is now showing tasks matching: {task_name}")
    print("Please select the task you want to close in OmniFocus")
    input("Press Enter when ready to close the selected task...")
    
    # Now close the selected task
    url = "omnifocus:///paste?target=selected&content=completed"
    subprocess.run(["open", url], check=True)
    print("Closed selected task")


if __name__ == "__main__":
    app()
