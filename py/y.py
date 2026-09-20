#!/usr/bin/env python3
"""Compatibility launcher for Y, now bundled with the Igor Tools Alfred workflow."""

import os
from pathlib import Path
import shutil
import sys


def app():
    default = (
        Path.home()
        / "gits/alfred/workflows/user.workflow.2859BD3B-9CA5-4360-A379-355F434F1908/y/y.py"
    )
    script = Path(os.environ.get("IGOR_Y_SCRIPT") or default).expanduser().resolve()
    if not script.is_file() or script == Path(__file__).resolve():
        print(
            "Y now lives in Igor Tools. Pull ~/gits/alfred, or set IGOR_Y_SCRIPT "
            "to the installed workflow's y/y.py.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    uv = shutil.which("uv")
    if uv is None:
        print("Y requires uv. Install it with: brew install uv", file=sys.stderr)
        raise SystemExit(1)
    os.execv(uv, [uv, "run", "--script", str(script), *sys.argv[1:]])


if __name__ == "__main__":
    app()
