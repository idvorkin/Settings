#!/usr/bin/env python3
"""Walk up to find .beads/, export fresh JSONL, launch bv in tree mode."""

import os
import subprocess
import sys


def find_beads_root():
    d = os.getcwd()
    while d != "/":
        if os.path.isdir(os.path.join(d, ".beads")):
            return d
        d = os.path.dirname(d)
    return None


def main():
    root = find_beads_root()
    if not root:
        print("No .beads/ found in any parent directory", file=sys.stderr)
        sys.exit(1)

    jsonl = os.path.join(root, ".beads", "beads.jsonl")

    # Fresh export from Dolt
    try:
        os.remove(jsonl)
    except FileNotFoundError:
        pass
    r = subprocess.run(["bd", "export", "--no-memories", "-o", jsonl], cwd=root)
    if r.returncode != 0:
        sys.exit(r.returncode)

    # Send 'E' keystroke after brief delay to switch to tree view
    pane_id_result = subprocess.run(
        ["tmux", "display-message", "-p", "#{pane_id}"],
        capture_output=True,
        text=True,
    )
    if pane_id_result.returncode == 0:
        pane_id = pane_id_result.stdout.strip()
        subprocess.Popen(
            ["bash", "-c", f"sleep 0.3 && tmux send-keys -t {pane_id} E"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    # Launch bv with any extra args, from the beads root
    bv_args = ["bv"] + sys.argv[1:]
    os.chdir(root)
    os.execvp("bv", bv_args)


if __name__ == "__main__":
    main()
