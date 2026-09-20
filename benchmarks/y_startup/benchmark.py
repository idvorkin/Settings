#!/usr/bin/env python3
"""Benchmark fresh Y processes with warm OS caches and no desktop actions."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import sys
import time


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def run(command, env=None, **kwargs):
    try:
        return subprocess.run(
            [str(arg) for arg in command],
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
            check=True,
            **kwargs,
        )
    except subprocess.CalledProcessError as error:
        print(error.stderr, file=sys.stderr)
        raise


def prepare_source(source, destination):
    # Benchmark-only substitutions: private caches and a harmless yabai fixture.
    # Keep the same replacements in baseline and candidate snapshots.
    replacements = {
        'cache_dir = Path.home() / "tmp" / ".cache" / "y_script"': 'cache_dir = Path(os.environ["Y_BENCH_CACHE"])',
        'yabi_root = Path("~/homebrew/bin/yabai/").expanduser()': 'yabi_root = Path(os.environ["Y_BENCH_YABAI"])',
    }
    for old, new in replacements.items():
        if source.count(old) != 1:
            raise ValueError(f"Source shape changed: {old}")
        source = source.replace(old, new)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(source)
    return hashlib.sha256(source.encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--baseline-ref", default="6531346")
    parser.add_argument("--runs", type=int, default=25)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--uv", action="store_true", help="Include uv script launch")
    parser.add_argument(
        "--console-python", help="Also measure installed console interpreter"
    )
    args = parser.parse_args()
    if args.runs < 2 or args.warmups < 1:
        parser.error("Use at least two runs and one warmup")
    out = HERE / "out"
    out.mkdir(exist_ok=True)
    env = dict(os.environ, GOCACHE=str(out / "go-cache"))
    env["Y_BENCH_YABAI"] = "/usr/bin/true"
    run(["rustc", "-O", HERE / "probe.rs", "-o", out / "probe-rust"], env)
    run(["go", "build", "-o", out / "probe-go", HERE / "probe.go"], env)
    sources = {
        "baseline": run(
            ["git", "show", f"{args.baseline_ref}:py/y.py"], cwd=ROOT
        ).stdout,
        "candidate": (ROOT / "py/y.py").read_text(),
    }
    cases = []
    hashes = {}
    full_programs = []
    for variant, source in sources.items():
        script = out / variant / "y.py"
        hashes[variant] = prepare_source(source, script)
        variant_env = dict(env, Y_BENCH_CACHE=str(out / variant / "cache"))
        cache = Path(variant_env["Y_BENCH_CACHE"]) / "alfred_commands.cache"
        cache.unlink(missing_ok=True)
        run([args.python, script, "alfred"], variant_env)
        full_programs.append(([args.python, script], variant_env))
        for label, command in (
            ("test-command", ["p-foo", "red", "small"]),
            ("focus-fixture", ["focus", "left"]),
            ("help", ["--help"]),
            ("completion-hit", ["alfred-complete", "f"]),
        ):
            cases.append(
                (f"{variant}/{label}", [args.python, script, *command], variant_env)
            )
        miss_env = dict(variant_env, Y_BENCH_CACHE=str(out / variant / "miss"))
        (Path(miss_env["Y_BENCH_CACHE"]) / "alfred_commands.cache").unlink(
            missing_ok=True
        )
        cases.append(
            (
                f"{variant}/completion-miss",
                [args.python, script, "alfred-complete", "f"],
                miss_env,
            )
        )
        if args.uv:
            for label, command in (
                ("test-command", ["p-foo", "red", "small"]),
                ("completion-hit", ["alfred-complete", "f"]),
            ):
                cases.append(
                    (
                        f"{variant}/uv-{label}",
                        ["uv", "run", "--offline", "--script", script, *command],
                        variant_env,
                    )
                )
        if args.console_python:
            console_env = dict(variant_env, PYTHONPATH=str(script.parent))
            for label, command in (
                ("test-command", ["p-foo", "red", "small"]),
                ("completion-hit", ["alfred-complete", "f"]),
            ):
                cases.append(
                    (
                        f"{variant}/console-{label}",
                        [
                            args.console_python,
                            "-c",
                            "from y import app; app()",
                            *command,
                        ],
                        console_env,
                    )
                )
        for mode, extra in (
            ("importtime", ["-X", "importtime"]),
            ("cprofile", ["-m", "cProfile", "-s", "cumulative"]),
        ):
            profile = run(
                [args.python, *extra, script, "p-foo", "red", "small"], variant_env
            )
            (out / f"{variant}-{mode}.txt").write_text(
                profile.stderr if mode == "importtime" else profile.stdout
            )
    probes = [
        [args.python, HERE / "probe.py"],
        [out / "probe-rust"],
        [out / "probe-go"],
    ]
    # Check equivalent dispatch semantics, including child errors, before timing.
    fixture = out / "yabai-fixture"
    fixture.write_text(
        '#!/bin/sh\nprintf "%s\\n" "$@" > "$Y_BENCH_LOG"\nexit "${Y_BENCH_EXIT:-0}"\n'
    )
    fixture.chmod(0o700)
    for command, program_env in [*full_programs, *((p, env) for p in probes)]:
        test_env = dict(
            program_env, Y_BENCH_YABAI=str(fixture), Y_BENCH_LOG=str(out / "argv.txt")
        )
        for direction, target in {
            "left": "west",
            "right": "east",
            "up": "north",
            "down": "south",
        }.items():
            run([*command, "focus", direction], test_env)
            assert (out / "argv.txt").read_text().splitlines() == [
                "-m",
                "window",
                "--focus",
                target,
            ]
        for extra, code in ((["focus", "invalid"], 2), (["focus"], 2)):
            result = subprocess.run(
                [str(v) for v in [*command, *extra]],
                env=test_env,
                capture_output=True,
                timeout=60,
            )
            assert result.returncode == code, (command, extra, result.returncode)
        result = subprocess.run(
            [str(v) for v in [*command, "focus", "left"]],
            env=dict(test_env, Y_BENCH_EXIT="7"),
            capture_output=True,
            timeout=60,
        )
        assert result.returncode != 0
    for query in ("", "f", "focus ", "focus r", "p-foo red ", "zoom", "ter"):
        outputs = [
            json.loads(run([*cmd, "alfred-complete", query], e).stdout)
            for cmd, e in full_programs
        ]
        assert outputs[0] == outputs[1], query
    for language, command in zip(("python", "rust", "go"), probes):
        for label, extra in (("noop", ["noop"]), ("focus-fixture", ["focus", "left"])):
            cases.append((f"probe-{language}/{label}", [*command, *extra], env))
    cases.append(("process-floor", ["/usr/bin/true"], env))
    print(f"Parity passed. Warming {len(cases)} cases...", flush=True)
    for _, command, case_env in cases:
        for _ in range(args.warmups):
            run(command, case_env)
    samples = {name: [] for name, _, _ in cases}
    rng = random.Random(20260920)
    for iteration in range(args.runs):
        rng.shuffle(cases)
        for name, command, case_env in cases:
            start = time.perf_counter_ns()
            run(command, case_env)
            samples[name].append((time.perf_counter_ns() - start) / 1_000_000)
        if (iteration + 1) % 5 == 0:
            print(f"Measured {iteration + 1}/{args.runs} rounds", flush=True)
    summary = {}
    for name, values in sorted(samples.items()):
        summary[name] = {
            "median_ms": statistics.median(values),
            "p95_ms": sorted(values)[math.ceil(len(values) * 0.95) - 1],
            "min_ms": min(values),
        }
        print(
            f"{name:38} {summary[name]['median_ms']:8.2f} ms median {summary[name]['p95_ms']:8.2f} ms p95"
        )
    metadata = {
        "platform": platform.platform(),
        "python": run([args.python, "--version"]).stdout.strip(),
        "console_python": run([args.console_python, "--version"]).stdout.strip()
        if args.console_python
        else None,
        "rust": run(["rustc", "--version"]).stdout.strip(),
        "go": run(["go", "version"]).stdout.strip(),
        "uv": run(["uv", "--version"]).stdout.strip(),
        "arguments": vars(args),
        "source_sha256": hashes,
        "binary_bytes": {
            lang: (out / f"probe-{lang}").stat().st_size for lang in ("rust", "go")
        },
        "commands": {name: [str(v) for v in command] for name, command, _ in cases},
    }
    (out / "results.json").write_text(
        json.dumps(
            {"metadata": metadata, "summary": summary, "samples_ms": samples}, indent=2
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
