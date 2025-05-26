#!uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
#     "rich",
#     "pydantic",
# ]
# ///

import os
import subprocess
from pathlib import Path
import tempfile
import shutil
import typer
from rich import print

# Define the logs directory name once, and have it be zz so it shows up last in diffs
CHOP_LOGS_DIR = "zz-chop-logs"

# Create the app with a debug option
app = typer.Typer(help="A helper for managing specstory logs", no_args_is_help=True)

# Global debug flag
DEBUG = False


def debug_print(*args, **kwargs):
    """Print only if DEBUG is True."""
    if DEBUG:
        print("[DEBUG]", *args, **kwargs)


# Common setup function to check git repo and specstory directory
def _chop_setup() -> tuple[Path, Path, Path]:
    """
    Setup function to check git repo and specstory directory.
    Returns a tuple of (git_root, spec_dir, logs_dir).
    Raises an exception if not in a git repository or .specstory/history directory not found.
    """
    try:
        git_root = Path(
            subprocess.check_output(
                ["git", "rev-parse", "--show-toplevel"], text=True
            ).strip()
        )
        debug_print(f"Git root: {git_root}")
    except subprocess.CalledProcessError:
        raise typer.BadParameter("Not in a git repository")

    spec_dir = git_root / ".specstory" / "history"
    debug_print(f"Spec directory: {spec_dir}")
    if not spec_dir.is_dir():
        raise typer.BadParameter(".specstory/history directory not found in git root")

    # Create logs directory in git root if it doesn't exist
    logs_dir = git_root / CHOP_LOGS_DIR
    debug_print(f"Logs directory: {logs_dir}")
    logs_dir.mkdir(parents=True, exist_ok=True)

    return git_root, spec_dir, logs_dir


# Function to get the latest file from specstory history
def _get_latest_file(spec_dir: Path) -> str:
    """
    Get the latest file from specstory history.
    Returns the filename of the latest file.
    Raises an exception if no files found.
    """
    if not spec_dir.is_dir():
        raise typer.BadParameter(f"Directory {spec_dir} does not exist")

    # Get all files in the directory
    all_files = list(spec_dir.glob("*"))
    debug_print(f"Found {len(all_files)} files in {spec_dir}")

    # Filter out directories and hidden files
    files = [f for f in all_files if f.is_file() and not f.name.startswith(".")]
    debug_print(f"After filtering, {len(files)} files remain")

    if not files:
        raise typer.BadParameter(f"No specstory files found in {spec_dir}")

    # Sort by modification time (newest first)
    files.sort(key=lambda x: x.stat().st_mtime, reverse=True)

    # Print the top 3 files for debugging
    if DEBUG and files:
        debug_print("Top files by modification time:")
        for i, f in enumerate(files[:3]):
            mtime = f.stat().st_mtime
            debug_print(f"  {i + 1}. {f.name} (mtime: {mtime})")

    latest_file = files[0].name
    debug_print(f"Latest file: {latest_file}")
    return latest_file


# Function to sanitize home paths in a file
def _sanitize_home_paths(file_path: Path) -> None:
    """
    Replace actual home directory with $HOME in the file.
    """
    if not file_path.is_file():
        raise typer.BadParameter(f"File {file_path} does not exist")

    home_path = str(Path.home())
    temp_file = Path(tempfile.mktemp())

    # Replace actual home directory with $HOME
    with open(file_path, "r") as src, open(temp_file, "w") as dst:
        for line in src:
            dst.write(line.replace(home_path, "$HOME"))

    # Move the sanitized file back to the original location
    shutil.move(temp_file, file_path)


# Function to copy and sanitize a file
def _copy_and_sanitize(src_dir: Path, src_file: str, dest_dir: Path) -> Path:
    """
    Copy a file from src_dir to dest_dir and sanitize home paths.
    Returns the path to the copied file.
    """
    src_path = src_dir / src_file
    if not src_path.is_file():
        raise typer.BadParameter(f"Source file {src_path} does not exist")

    if not dest_dir.is_dir():
        print(f"Creating destination directory {dest_dir}")
        dest_dir.mkdir(parents=True, exist_ok=True)

    # Copy the file
    dest_path = dest_dir / src_file
    shutil.copy2(src_path, dest_path)
    print(f"Copied {src_file} to {dest_dir}/")

    # Sanitize home paths in the copied file
    _sanitize_home_paths(dest_path)
    print(f"Sanitized home paths in {src_file}")

    return dest_path


# Function to add a file to git
def _add_to_git(git_root: Path, file_path: Path) -> None:
    """
    Add a file to git staging area.
    """
    if not (git_root / ".git").is_dir():
        raise typer.BadParameter(f"{git_root} is not a valid git repository")

    if not file_path.is_file():
        raise typer.BadParameter(f"File {file_path} does not exist")

    # Get relative path
    try:
        rel_path = file_path.relative_to(git_root)
    except ValueError:
        # If the file is not within the git root, try to find a relative path anyway
        rel_path = Path(
            subprocess.check_output(
                ["realpath", "--relative-to", str(git_root), str(file_path)], text=True
            ).strip()
        )

    # Add to git
    subprocess.run(["git", "-C", str(git_root), "add", str(rel_path)], check=True)
    print(f"Added {rel_path} to git staging area")


# Function to show the last 20 lines of a file
def _show_tail(file_path: Path) -> None:
    """
    Show the last 20 non-empty lines of a file.
    """
    if not file_path.is_file():
        raise typer.BadParameter(f"File {file_path} does not exist")

    print("\n" + "=" * 40)
    print(f"Content preview of {file_path.name}:")
    print("=" * 40)

    # Read the file and get non-empty lines
    with open(file_path, "r") as f:
        lines = [line.rstrip() for line in f.readlines()]

    # Filter out empty lines
    non_empty_lines = [line for line in lines if line.strip()]

    # Get the last 20 non-empty lines
    last_lines = non_empty_lines[-20:] if non_empty_lines else []

    # Get terminal width for line truncation
    try:
        terminal_width = os.get_terminal_size().columns
        max_line_length = max(
            60, terminal_width - 10
        )  # At least 60 chars, but adapt to terminal
    except (OSError, AttributeError):
        max_line_length = 80  # Default if can't determine terminal width

    # Print the lines with truncation for very long lines
    for line in last_lines:
        if len(line) > max_line_length:
            print(f"  {line[: max_line_length - 5]}...")
        else:
            print(f"  {line}")

    print("=" * 40 + "\n")


@app.callback()
def main(debug: bool = typer.Option(False, "--debug", help="Enable debug output")):
    """
    A helper for managing specstory logs.
    """
    global DEBUG
    DEBUG = debug
    if DEBUG:
        print("Debug mode enabled")


@app.command()
def save():
    """
    Select a file from specstory history, copy it to the logs directory, and sanitize it.
    """
    git_root, spec_dir, logs_dir = _chop_setup()

    # Use fzf to select a file, showing files in reverse date order with preview
    try:
        selected_file = subprocess.check_output(
            f'cd "{spec_dir}" && ls -t | fzf --preview "bat --style=numbers --color=always {{}}" --preview-window=right:70% --height=80% --bind \'ctrl-o:execute(tmux new-window nvim {{}})\'',
            shell=True,
            text=True,
        ).strip()
    except subprocess.CalledProcessError:
        print("No file selected")
        return

    if not selected_file:
        print("No file selected")
        return

    # Copy and sanitize the file
    copied_file = _copy_and_sanitize(spec_dir, selected_file, logs_dir)

    # Show the last 20 lines of the file
    _show_tail(copied_file)


@app.command()
def git_latest():
    """
    Get the latest file from specstory history, copy it to the logs directory, sanitize it, and add it to git.
    """
    git_root, spec_dir, logs_dir = _chop_setup()

    # Get the latest file by modification time
    latest_file = _get_latest_file(spec_dir)
    print(f"Found latest file: {latest_file}")

    # Copy and sanitize the file
    copied_file = _copy_and_sanitize(spec_dir, latest_file, logs_dir)

    # Add to git
    _add_to_git(git_root, copied_file)

    # Show the last 20 lines of the file
    _show_tail(copied_file)


@app.command()
def view_latest():
    """
    Open the latest file from specstory history in nvim.
    """
    git_root, spec_dir, logs_dir = _chop_setup()

    # Get the latest file by modification time
    latest_file = _get_latest_file(spec_dir)

    # Open the latest file in nvim
    subprocess.run(["nvim", str(spec_dir / latest_file)])


@app.command()
def git_pick():
    """
    Select a file from specstory history, copy it to the logs directory, sanitize it, and add it to git.
    """
    git_root, spec_dir, logs_dir = _chop_setup()

    # Use fzf to select a file, showing files in reverse date order with preview
    try:
        selected_file = subprocess.check_output(
            f'cd "{spec_dir}" && ls -t | fzf --preview "bat --style=numbers --color=always {{}}" --preview-window=right:70% --height=80% --bind \'ctrl-o:execute(tmux new-window nvim {{}})\'',
            shell=True,
            text=True,
        ).strip()
    except subprocess.CalledProcessError:
        print("No file selected")
        return

    if not selected_file:
        print("No file selected")
        return

    # Copy and sanitize the file
    copied_file = _copy_and_sanitize(spec_dir, selected_file, logs_dir)

    # Add to git
    _add_to_git(git_root, copied_file)

    # Show the last 20 lines of the file
    _show_tail(copied_file)


@app.command()
def diff():
    """
    Compare the two most recent files from specstory history.
    """
    git_root, spec_dir, logs_dir = _chop_setup()

    # Get all files in the directory
    all_files = list(spec_dir.glob("*"))
    debug_print(f"Found {len(all_files)} files in {spec_dir}")

    # Filter out directories and hidden files
    files = [f for f in all_files if f.is_file() and not f.name.startswith(".")]
    debug_print(f"After filtering, {len(files)} files remain")

    if len(files) < 2:
        raise typer.BadParameter("Need at least two specstory files to compare")

    # Sort by modification time (newest first)
    files.sort(key=lambda x: x.stat().st_mtime, reverse=True)

    newest_file = files[0]
    previous_file = files[1]

    print("Comparing:")
    print(f"  NEWER: {newest_file.name}")
    print(f"  OLDER: {previous_file.name}")
    print("")

    # Use delta for a nice diff if available, otherwise fall back to diff
    if shutil.which("delta"):
        subprocess.run(["delta", str(previous_file), str(newest_file)])
    else:
        subprocess.run(["diff", "--color=always", str(previous_file), str(newest_file)])


@app.command()
def git_refresh():
    """
    Refresh staged files in the CHOP_LOGS_DIR from their originals in .specstory/history.
    If no files are staged, get the latest file and add it to git.
    """
    git_root, spec_dir, logs_dir = _chop_setup()

    # Get list of staged files in the CHOP_LOGS_DIR
    try:
        staged_files_output = subprocess.check_output(
            ["git", "-C", str(git_root), "diff", "--name-only", "--cached"], text=True
        ).strip()

        # Filter for files in CHOP_LOGS_DIR
        staged_files = (
            [
                line
                for line in staged_files_output.split("\n")
                if line.startswith(CHOP_LOGS_DIR + "/")
            ]
            if staged_files_output
            else []
        )
    except subprocess.CalledProcessError:
        staged_files = []

    if not staged_files:
        print(f"No staged files found in {CHOP_LOGS_DIR}")

        # Get the latest file by modification time as a fallback
        latest_file = _get_latest_file(spec_dir)

        # Copy and sanitize the file
        copied_file = _copy_and_sanitize(spec_dir, latest_file, logs_dir)

        # Add to git
        _add_to_git(git_root, copied_file)

        # Show the last 20 lines of the file
        _show_tail(copied_file)
        return

    print(f"Found staged files in {CHOP_LOGS_DIR}:")
    for file in staged_files:
        print(file)
    print("")

    # Process each staged file
    processed_files = []
    for file in staged_files:
        # Extract just the filename without the path
        filename = Path(file).name

        # Check if the original file exists in .specstory/history
        if not (spec_dir / filename).is_file():
            print(
                f"Warning: Original file {filename} not found in .specstory/history, skipping"
            )
            continue

        print(f"Refreshing {filename} from .specstory/history")

        # Copy and sanitize the file
        copied_file = _copy_and_sanitize(spec_dir, filename, logs_dir)

        # Re-add the file to git
        _add_to_git(git_root, copied_file)

        # Show the last 20 lines of the file
        _show_tail(copied_file)

        processed_files.append(filename)

    print(f"Refresh complete! Processed {len(processed_files)} files.")


if __name__ == "__main__":
    app()
