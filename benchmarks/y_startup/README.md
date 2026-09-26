Y startup experiment — September 20, 2026

Keep Python for now. Deferring macOS GUI imports removes about 90–98 ms from
ordinary Y invocations (40–44% in this run). A small dependency-free Python
dispatcher suggests a larger remaining improvement without rewriting the whole
tool. Rust and Go both provide much faster process startup, but their difference
here is only about 1 ms for a deliberately narrow command.

Worktree: `/tmp/settings-y-startup-perf`, branch `experiment/y-startup-perf`.
Baseline: `6531346`. Tracking: `settings-mfc`.

These are medians of 25 fresh processes per case, with three warmups, warm OS
file caches, and randomized case order each round. Output is captured through
pipes. The full raw samples, commands, source hashes, and toolchain versions are
in [results.json](results.json). p95 is the nearest-rank sample percentile, not a
confidence interval. This is one machine/session; small differences should not
be overinterpreted.

| Full Y operation                                            | Baseline median | Deferred GUI median | Baseline p95 | Deferred GUI p95 |
| ----------------------------------------------------------- | --------------: | ------------------: | -----------: | ---------------: |
| Installed console entry-point equivalent: test command      |       204.67 ms |           115.00 ms |    234.85 ms |        132.24 ms |
| `uv run --offline --script`: test command                   |       247.03 ms |           148.95 ms |    291.00 ms |        176.77 ms |
| Direct Python: test command                                 |       234.72 ms |           139.67 ms |    262.87 ms |        150.08 ms |
| Direct Python: focus with no-op yabai fixture               |       229.00 ms |           134.01 ms |    309.57 ms |        181.61 ms |
| Direct Python: help                                         |       258.10 ms |           164.07 ms |    287.78 ms |        184.60 ms |
| Direct Python: completion, command cache absent             |       222.85 ms |           127.35 ms |    241.97 ms |        140.67 ms |
| Direct Python: completion, command cache present            |        35.30 ms |            35.45 ms |     38.65 ms |         44.98 ms |
| Installed console entry-point equivalent: cached completion |        22.94 ms |            22.61 ms |     24.24 ms |         24.51 ms |
| `uv run --offline --script`: cached completion              |        43.61 ms |            44.14 ms |     46.77 ms |         52.16 ms |

The test command is `p-foo red small`, which prints a string. Completion uses
`alfred-complete f`. Cached completion already exits before loading the full
application, so deferred GUI imports do not improve it. “Cache absent” refers
to Y's command cache, not disk caches or a fresh dependency installation.

The machine runs macOS 27.0 on arm64. Direct-script Python is 3.14.7; the
installed console interpreter is 3.12.9. The latter runs `from y import app;
app()` with the snapshot directory on `PYTHONPATH`, matching the important
behavior of the installed entry point without changing its installation. Its
advantage mixes interpreter/environment differences and cached module bytecode;
it is not evidence that Python 3.12 itself is faster. `uv` is 0.12.13. Timings
exclude dependency setup, compilation, profiling, and correctness checks.

The production-code change in [y.py](../../py/y.py) moves Quartz, CoreGraphics,
and AppKit loading to the screenshot, overlay, and mouse operations that need
them. Optional-dependency error behavior and existing test injection of AppKit
are preserved. This does not accelerate the GUI work itself: those commands
still pay the import cost when needed. The installed Y remains unchanged.

In the recorded baseline import profile, Quartz takes 97.6 ms cumulatively,
including AppKit's 54.9 ms. Those figures are nested and must not be added.
The candidate no longer imports either for ordinary commands. Its remaining
profile includes Typer (22.1 ms cumulative) and IceCream (17.4 ms cumulative),
plus Rich, Pydantic model construction, and argument dispatch. Profiling adds
overhead; use the subprocess measurements above for user-visible latency.
The harness writes both
[Python import timing](https://docs.python.org/3/using/cmdline.html#cmdoption-X)
and cProfile output under `out/`.

| Equivalent small probe                            | Python median / p95 | Rust median / p95 | Go median / p95 |
| ------------------------------------------------- | ------------------: | ----------------: | --------------: |
| Start and exit                                    |    22.50 / 24.45 ms |    3.21 / 4.06 ms |  3.59 / 3.76 ms |
| Map focus direction and run identical no-op child |    25.03 / 28.27 ms |    4.17 / 4.81 ms |  5.38 / 5.95 ms |

The measurement floor (`/usr/bin/true`) is 2.42 ms median. The probes implement
only `noop` and `focus left/right/up/down`, with argument validation and child
failure propagation. They do not implement Y's completion engine, screenshots,
overlays, process management, help system, or terminal discovery. In particular,
these results do not promise a 4 ms complete Rust Y. All three probes validate
the same focus-to-yabai argument mapping; full Y is checked against that mapping
as well. Native probe binaries are approximately 478 KB for Rust and 2.51 MB
for Go with the recorded build commands, without size tuning.

Rust uses 1.98.1 and `rustc -O`
([optimization level 3](https://doc.rust-lang.org/rustc/codegen-options/#opt-level));
Go uses 1.27.1 and `go build`. Neither probe downloads dependencies. Both call
the same subprocess fixture. A full GUI port would also need replacements for
PyObjC integration; Go's [cgo](https://pkg.go.dev/cmd/cgo) provides a native
interop mechanism, but that integration and its costs are outside this probe.

My recommendation is to adopt deferred GUI imports, then isolate high-frequency
window commands from Typer, Rich, IceCream, and Pydantic initialization. The
minimal Python focus probe is about 109 ms faster than the candidate full Y
focus path; moving from that probe to Rust saves another 21 ms, and Go saves
20 ms. These are opportunities suggested by scoped probes, not measured savings
from a complete implementation. Use the installed entry point for latency
sensitive completion when suitable; its measured cached path is already about
23 ms. If a roughly 25 ms Python hot path is still too slow, a small native
dispatcher with Python fallback is a better next experiment than a full port.
The current 1.2 ms Rust/Go focus gap is not a strong reason to pick a language.
The existing Rust tool in this repository makes Rust a reasonable fit; choose
Go if maintainability and team familiarity favor it.

Python 3.15 can simplify a future implementation, but upgrading alone does not
make Y's existing imports lazy. PEP 810 is opt-in: module-level `lazy import`
defers loading until first use. `__lazy_modules__` offers a source-compatible
transition, with eager imports on older interpreters. Imports in functions and
`try` blocks stay eager. See the
[Python 3.15 language reference](https://docs.python.org/3.15/reference/simple_stmts.html#lazy-imports).

That restriction matters here: the baseline imports its expensive dependencies
inside `load_full_imports()`, and wraps PyObjC imports in `try/except`. Merely
requiring 3.15 leaves those statements eager. The global
[`-X lazy_imports=all` option](https://docs.python.org/3.15/using/cmdline.html#cmdoption-X)
also preserves these restrictions; it might affect dependencies' eligible
imports, but does not replace Y's architectural change. This follows from
[PEP 810's scope restrictions](https://peps.python.org/pep-0810/#syntax-restrictions),
not from a measured 3.15 speedup.

A deliberate 3.15 refactor could put rarely used dependencies behind module-level
`lazy import` statements and handle import failures at first use. Y would still
need to avoid touching those names during startup: creating the Typer app,
evaluating command decorators, and defining Pydantic subclasses use their
dependencies immediately. Preserve command registration and missing-PyObjC
behavior, and benchmark each real entry point before replacing the explicit
loader. Selective opt-in makes those boundaries easier to review than changing
import behavior throughout the dependency graph.

As of September 20, 2026, 3.15 is a release candidate; the
[release schedule](https://peps.python.org/pep-0790/#release-schedule) targets
October 1 for the final release. This machine has no installed 3.15 interpreter,
so this change makes no 3.15 compatibility or performance claim and keeps the
existing Python requirement. A future minimum-version change should validate
PyObjC/Pydantic dependencies, both script and installed-console launchers,
completion cache hits/misses, GUI error handling, and first-use GUI latency.

The benchmark deliberately avoids moving windows, querying accessibility APIs,
or modifying the real Alfred cache. It snapshots baseline and candidate source,
substitutes private command-cache directories and a no-op yabai executable in
both, then runs them as separate processes. Actual yabai latency, AppleScript
terminal discovery, accessibility timeouts, screenshot capture, and visible
badge startup need separate end-to-end measurements. No first-launch-after-reboot
or dependency-install measurements are included.

Prerequisites: Git, Rust (`rustc`), and Go (`go`) on `PATH`, plus a Python
interpreter with Y's dependencies installed. Both native toolchains are required
because the harness always builds and compares their probes. uv is needed only
when `--uv` is selected.

The harness and minimal Python probe intentionally use only the standard
library, with empty PEP 723 dependency declarations. These are benchmark
fixtures rather than installed user CLI tools: adding Typer/Rich to the measured
probe would eliminate the dependency-free comparison. They are not registered
as commands in `py/pyproject.toml`; Y itself (now in the Alfred workflow) uses
Typer and Rich.

Reproduce from this worktree using an interpreter with Y's dependencies:

```sh
python3 benchmarks/y_startup/benchmark.py \
  --python /Users/idvorkin/.cache/uv/environments-v2/y-26d22e6fde54d364/bin/python \
  --console-python /Users/idvorkin/.local/share/uv/tools/idvorkin-scripts/bin/python \
  --uv --runs 25
```

The interpreter paths above describe this machine. Replace them on another
machine; `--console-python` and `--uv` are optional. The uv cases use offline
resolution and require the inline dependencies to be available in uv's cache.
Generated snapshots, binaries, private caches, and profiles live in ignored
`out/`. `results.json` preserves this run separately from future outputs.

Validation: 25 tests pass across `test_y_completion.py`,
`test_y_window_numbers.py`, and `test_y_startup.py`. The harness additionally
checks all four focus directions, missing/invalid directions, nonzero child
status, and baseline/candidate JSON equivalence for seven completion queries.
New regression tests confirm ordinary commands avoid GUI imports, frameworks
load on demand, and missing frameworks fail before launching an overlay. Ruff
lint/format, rustfmt, gofmt, native compilation, and `git diff --check` pass.
GUI imports were exercised, but actual screenshots and overlays were not.

After the Alfred migration, the candidate defaults to Igor Tools in
`~/gits/alfred` (or `IGOR_Y_SCRIPT`). Use `--candidate /path/to/y/y.py` to override.
The baseline still comes from settings Git history. Completion parity checks
cover stable queries; the complete command list may grow. Recorded results
above describe the original experiment, not the compatibility launcher's cost.
