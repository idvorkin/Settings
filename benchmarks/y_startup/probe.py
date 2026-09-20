"""Deliberately scoped dispatch probe, not a replacement for Y."""

import os
import subprocess
import sys


def main():
    args = sys.argv[1:]
    if args == ["noop"]:
        return 0
    directions = {"left": "west", "right": "east", "up": "north", "down": "south"}
    if len(args) != 2 or args[0] != "focus" or args[1] not in directions:
        return 2
    result = subprocess.run(
        [os.environ["Y_BENCH_YABAI"], "-m", "window", "--focus", directions[args[1]]],
        capture_output=True,
    )
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
