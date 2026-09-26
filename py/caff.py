#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich"]
# ///
"""caff - a battery-aware wrapper around macOS `caffeinate`.

Modes, re-evaluated from `pmset -g batt` and `herdr agent list` every tick, and
immediately when macOS posts a power event (plug/unplug, battery percent):
  plugged in                     -> caffeinate -d -i -s  (display + system on)
  battery, above floor, agents   -> caffeinate -i        (system up, screen may
    working (or Herdr unreachable)                        sleep)
  battery, all agents idle       -> nothing, plus a scheduled wake (--wake) so
                                    the Mac comes back to re-check the agents
  battery, at/below floor        -> nothing              (normal sleep)
  pmset unreadable               -> nothing              (can't see the floor,
                                                          so don't risk it)

Scheduled wakes go through `sudo -n pmset schedule wake`, which needs the
sudoers rule in mac/caff.sudoers (install instructions inside). Without it
caff warns once and just doesn't wake.

One battery sample a minute is kept in ~/.cache/caff/history.json, which feeds
a sparkline and a measured drain rate (used for the "floor in ~Xm" estimate).

    caff          # watch forever, Ctrl-C to stop
    caff 60m      # same rule, but give up after 60 minutes
    caff 4h --wake 5m
    caff info     # what the watcher is doing, from any shell
"""

import ctypes
import ctypes.util
import json
import math
import os
import re
import select
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from collections.abc import Callable
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console, Group
from rich.live import Live
from rich.text import Text

console = Console()
app = typer.Typer(pretty_exceptions_enable=False, add_completion=False)

CACHE_DIR = Path.home() / ".cache" / "caff"
STATE_FILE = CACHE_DIR / "state.json"
HISTORY_FILE = CACHE_DIR / "history.json"
DEFAULT_FLOOR = 50
DEFAULT_WAKE = "10m"
DEFAULT_INTERVAL = 30.0  # seconds between ticks
BUSY_STATES = {"working"}  # Herdr agent_status values that keep the Mac up
PMSET_DATE = "%m/%d/%y %H:%M:%S"  # the only format `pmset schedule` accepts
WAKE_OWNER = "caff"
HISTORY_KEEP = timedelta(hours=24)  # samples older than this are dropped
GRAPH_WINDOW = timedelta(hours=3)  # how far back the sparkline looks
GRAPH_WIDTH = 60  # columns; samples are bucketed down to fit
DRAIN_WINDOW = timedelta(hours=1)  # measured drain rate uses at most this
DRAIN_MIN_SPAN = timedelta(minutes=5)  # ...and needs at least this much data
SPARK_CHARS = "▁▂▃▄▅▆▇█"
STALE_TICKS = 3  # state.json older than this many intervals = watcher is gone
STALE_MIN_SECONDS = 60.0
# notify(3) keys from IOPowerSources.h: AC <-> battery, and battery percent
POWER_NOTIFY_KEYS = (
    "com.apple.system.powersources.source",
    "com.apple.system.powersources.percent",
)
NOTIFY_REUSE = 0x1  # notify.h: register another key on an existing fd


class Mode(str, Enum):
    FULL = "full"  # display + system awake
    IDLE = "idle"  # system awake, display may sleep
    SLEEP = "sleep"  # let macOS do its thing

    @property
    def caffeinate_args(self) -> list[str] | None:
        return {
            Mode.FULL: ["-d", "-i", "-s"],
            Mode.IDLE: ["-i"],
            Mode.SLEEP: None,
        }[self]

    @property
    def label(self) -> str:
        return {
            Mode.FULL: "[green]awake[/green] (screen on)",
            Mode.IDLE: "[green]awake[/green] (screen may sleep)",
            Mode.SLEEP: "[red]sleep allowed[/red]",
        }[self]


@dataclass(frozen=True)
class Power:
    plugged_in: bool
    percent: int | None  # None on AC = desktop; None off AC = pmset unreadable
    minutes_left: int | None  # pmset's time-to-empty; None on AC / no estimate

    @property
    def unknown(self) -> bool:
        """pmset failed or made no sense: a desktop always reports AC."""
        return not self.plugged_in and self.percent is None

    def describe(self) -> str:
        if self.unknown:
            return "[red]power unknown[/red]"
        if self.percent is None:
            return "AC, no battery"
        source = "AC" if self.plugged_in else "battery"
        return f"{source} {self.percent}%"


UNKNOWN_POWER = Power(plugged_in=False, percent=None, minutes_left=None)


_DURATION_RE = re.compile(
    r"^\s*(\d+(?:\.\d+)?)\s*(m|min|mins|minute|minutes|h|hr|hrs|hour|hours)?\s*$",
    re.IGNORECASE,
)


def parse_duration(text: str) -> timedelta:
    """'60m', '60 min', '4h', '1.5hr', or bare '90' (minutes)."""
    match = _DURATION_RE.match(text)
    if not match:
        raise ValueError(f"Can't parse duration {text!r}; try 60m or 4h")
    value = float(match.group(1))
    unit = (match.group(2) or "m").lower()
    if value <= 0:
        raise ValueError("Duration must be positive")
    try:
        if unit.startswith("h"):
            return timedelta(hours=value)
        return timedelta(minutes=value)
    except OverflowError:
        raise ValueError(f"Duration {text!r} is too long") from None


def parse_pmset(output: str) -> Power:
    """Parse `pmset -g batt` output."""
    plugged_in = "AC Power" in output
    pct = re.search(r"(\d+)%", output)
    percent = int(pct.group(1)) if pct else None
    minutes_left = None
    eta = re.search(r"(\d+):(\d+) remaining", output)
    if eta and "discharging" in output:
        minutes_left = int(eta.group(1)) * 60 + int(eta.group(2))
    return Power(plugged_in=plugged_in, percent=percent, minutes_left=minutes_left)


def read_power() -> Power:
    """Current power state; UNKNOWN_POWER when pmset can't be asked."""
    try:
        result = subprocess.run(
            ["pmset", "-g", "batt"], capture_output=True, text=True, timeout=5
        )
    except (OSError, subprocess.TimeoutExpired):
        return UNKNOWN_POWER
    if result.returncode != 0:
        return UNKNOWN_POWER
    return parse_pmset(result.stdout)


@dataclass(frozen=True)
class Agents:
    busy: int
    total: int

    def describe(self) -> str:
        return f"agents {self.busy}/{self.total} busy"


def parse_agents(output: str) -> Agents:
    """Parse `herdr agent list` JSON."""
    agents = json.loads(output)["result"]["agents"]
    busy = sum(a.get("agent_status") in BUSY_STATES for a in agents)
    return Agents(busy=busy, total=len(agents))


def read_agents() -> Agents | None:
    """Herdr's view of the agents, or None when Herdr can't be asked."""
    try:
        result = subprocess.run(
            ["herdr", "agent", "list"], capture_output=True, text=True, timeout=5
        )
        if result.returncode != 0:
            return None
        return parse_agents(result.stdout)
    except (
        OSError,
        subprocess.TimeoutExpired,
        ValueError,
        KeyError,
        TypeError,
        AttributeError,  # agents wasn't a list of dicts
    ):
        return None


def desired_mode(power: Power, floor: int, agents: Agents | None = None) -> Mode:
    """Unknown agents (Herdr unreachable) are treated as busy: stay awake.

    Unknown power goes the other way: we can't see the floor, so allow sleep
    rather than risk running the battery flat.
    """
    if power.plugged_in:
        return Mode.FULL
    if power.percent is None:
        return Mode.SLEEP
    if power.percent <= floor:
        return Mode.SLEEP
    if agents is not None and agents.busy == 0:
        return Mode.SLEEP
    return Mode.IDLE


_warned: set[str] = set()


def warn_once(message: str) -> None:
    if message in _warned:
        return
    _warned.add(message)
    console.print(f"[yellow]{message}[/yellow]")


def write_json(path: Path, data: object) -> None:
    """Atomic, so `caff info` never reads half a file. The cache is cosmetic:
    a full disk must not stop the watcher."""
    tmp = path.with_suffix(".tmp")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp.write_text(json.dumps(data))
        os.replace(tmp, path)
    except OSError as e:
        warn_once(f"Can't write {path}: {e}")


# --- battery history -------------------------------------------------------


@dataclass(frozen=True)
class Sample:
    at: datetime
    percent: int
    plugged_in: bool
    busy: int | None = None  # busy agents at the time; None = Herdr unreachable

    def to_json(self) -> dict:
        return {
            "at": self.at.isoformat(),
            "pct": self.percent,
            "ac": self.plugged_in,
            "busy": self.busy,
        }

    @classmethod
    def from_json(cls, data: dict) -> "Sample":
        return cls(
            datetime.fromisoformat(data["at"]),
            data["pct"],
            data["ac"],
            data.get("busy"),
        )


class History:
    """Battery samples, at most one per wall-clock minute, persisted to disk."""

    def __init__(
        self, path: Path | None = None, keep: timedelta = HISTORY_KEEP
    ) -> None:
        self.path = path or HISTORY_FILE
        self.keep = keep
        self.samples = self._load()

    def _load(self) -> list[Sample]:
        try:
            rows = json.loads(self.path.read_text())
        except (OSError, ValueError):
            return []
        if not isinstance(rows, list):
            return []
        samples: list[Sample] = []
        for row in rows:  # one bad row must not cost the other 24h
            try:
                samples.append(Sample.from_json(row))
            except (ValueError, KeyError, TypeError):
                continue
        return samples

    def save(self) -> None:
        write_json(self.path, [s.to_json() for s in self.samples])

    def record(self, power: Power, now: datetime, agents: Agents | None = None) -> bool:
        """Add a sample unless this minute already has one. True if added."""
        if power.percent is None:
            return False
        if self.samples and self.samples[-1].at.replace(
            second=0, microsecond=0
        ) == now.replace(second=0, microsecond=0):
            return False
        busy = agents.busy if agents is not None else None
        self.samples.append(Sample(now, power.percent, power.plugged_in, busy))
        self.samples = [s for s in self.samples if now - s.at <= self.keep]
        self.save()
        return True

    def recent(self, window: timedelta, now: datetime) -> list[Sample]:
        return [s for s in self.samples if now - s.at <= window]


def drain_rate(samples: list[Sample], now: datetime) -> float | None:
    """Measured percent-per-hour lost over the latest on-battery stretch.

    Uses at most DRAIN_WINDOW of samples and needs DRAIN_MIN_SPAN of them.
    None while plugged in or until enough data exists.
    """
    run: list[Sample] = []
    for sample in reversed(samples):
        if sample.plugged_in or now - sample.at > DRAIN_WINDOW:
            break
        run.append(sample)
    if len(run) < 2:
        return None
    newest, oldest = run[0], run[-1]
    span = newest.at - oldest.at
    if span < DRAIN_MIN_SPAN:
        return None
    return (oldest.percent - newest.percent) / (span.total_seconds() / 3600)


def sparkline(samples: list[Sample], width: int = GRAPH_WIDTH) -> Text:
    """One-line battery graph; charging stretches in green, scaled to the range."""
    if len(samples) < 2:
        return Text("collecting battery samples...", style="dim")
    per_bucket = math.ceil(len(samples) / width)
    buckets = [samples[i : i + per_bucket] for i in range(0, len(samples), per_bucket)]
    # ponytail: buckets are by sample count, so a gap (watcher off) is not visible
    lo = min(s.percent for s in samples)
    hi = max(s.percent for s in samples)
    spread = max(hi - lo, 1)
    graph = Text()
    for bucket in buckets:
        mean = sum(s.percent for s in bucket) / len(bucket)
        level = round((mean - lo) / spread * (len(SPARK_CHARS) - 1))
        graph.append(SPARK_CHARS[level], style="green" if bucket[-1].plugged_in else "")
    return graph


def graph_line(samples: list[Sample], rate: float | None) -> Text:
    """Sparkline plus a caption: `94%→82% 04:30–06:35 · draining 6.1%/h`."""
    if len(samples) < 2:
        return sparkline(samples)
    first, last = samples[0], samples[-1]
    caption = f"  {first.percent}%→{last.percent}% {first.at:%H:%M}–{last.at:%H:%M}"
    if rate is not None:
        caption += f" · {'draining' if rate >= 0 else 'gaining'} {abs(rate):.1f}%/h"
    width = max(10, min(GRAPH_WIDTH, console.width - len(caption)))  # one line
    line = sparkline(samples, width)
    line.append(caption, style="dim")
    return line


# --- estimates and display -------------------------------------------------


def minutes_to_floor(power: Power, floor: int, rate: float | None = None) -> int | None:
    """Minutes until the battery hits the floor. None when not discharging.

    Prefers a measured drain `rate` (%/h); falls back to a linear scaling of
    pmset's time-to-empty.
    """
    if power.plugged_in or power.percent is None:
        return None
    if power.percent <= floor:
        return 0
    if rate is not None and rate > 0:
        return round((power.percent - floor) / rate * 60)
    if power.minutes_left is None:
        return None
    return round(power.minutes_left * (power.percent - floor) / power.percent)


def fmt_minutes(minutes: int) -> str:
    hours, mins = divmod(minutes, 60)
    return f"{hours}h{mins:02d}m" if hours else f"{mins}m"


def sleep_eta(
    power: Power,
    floor: int,
    deadline: datetime | None,
    now: datetime,
    rate: float | None = None,
) -> str:
    """Human line for 'when will we let the Mac sleep'."""
    parts: list[str] = []
    to_floor = minutes_to_floor(power, floor, rate)
    if to_floor is not None:
        parts.append(f"floor {floor}% in ~{fmt_minutes(to_floor)}")
    if deadline is not None:
        left = max(0, int((deadline - now).total_seconds() // 60))
        parts.append(f"timer ends {deadline:%H:%M} ({fmt_minutes(left)})")
    if not parts:
        return "no sleep scheduled" if power.plugged_in else "no battery estimate yet"
    return "sleep in " + ", ".join(parts)


def _register_notify_fd(keys: tuple[str, ...]) -> int | None:
    """One notify(3) file descriptor that becomes readable when any key posts.

    None when libSystem or a registration fails.
    """
    try:
        libsystem = ctypes.CDLL(ctypes.util.find_library("System"))
        register = libsystem.notify_register_file_descriptor
    except (OSError, AttributeError):
        return None
    fd, token = ctypes.c_int(-1), ctypes.c_int()
    for i, key in enumerate(keys):
        flags = NOTIFY_REUSE if i else 0  # later keys share the first key's fd
        if register(key.encode(), ctypes.byref(fd), flags, ctypes.byref(token)):
            return None
    return fd.value


class PowerEvents:
    """Wakes the watcher the moment the power source or battery percent changes.

    Without a notify fd (registration failed) it degrades to a plain sleep, so
    the watcher still ticks every interval.
    """

    def __init__(
        self,
        keys: tuple[str, ...] = POWER_NOTIFY_KEYS,
        register: Callable[[tuple[str, ...]], int | None] = _register_notify_fd,
    ) -> None:
        self.fd = register(keys)
        if self.fd is None:
            warn_once("Can't subscribe to power events - polling only")

    def wait(self, timeout: float) -> bool:
        """Sleep up to `timeout` seconds; True if a power event cut it short."""
        if self.fd is None:
            time.sleep(timeout)
            return False
        ready, _, _ = select.select([self.fd], [], [], timeout)
        if not ready:
            return False
        os.read(self.fd, 4096)  # drain the queued tokens; one tick covers them all
        return True


class Caffeinator:
    """Owns the single caffeinate child process."""

    def __init__(
        self, popen: Callable[[list[str]], subprocess.Popen] = subprocess.Popen
    ) -> None:
        self.popen = popen
        self.proc: subprocess.Popen | None = None
        self.mode = Mode.SLEEP

    @property
    def running(self) -> bool:
        return self.proc is not None and self.proc.poll() is None

    def ensure(self, mode: Mode) -> None:
        if mode == self.mode and (mode == Mode.SLEEP or self.running):
            return
        if mode == self.mode:
            console.print("[yellow]caffeinate exited on its own[/yellow] - restarting")
        self.stop()
        args = mode.caffeinate_args
        if args is not None:
            # -w: caffeinate quits when we do, even if we're SIGKILLed and
            # never reach stop() - otherwise it holds the Mac awake ownerless
            self.proc = self.popen(["caffeinate", *args, "-w", str(os.getpid())])
        self.mode = mode

    def stop(self) -> None:
        if self.running:
            assert self.proc is not None
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self.proc = None
        self.mode = Mode.SLEEP


class Waker:
    """A one-shot `pmset schedule wake`, re-armed while the Mac is allowed to sleep.

    Needs root, so the calls go through `sudo -n` and the mac/caff.sudoers rule.
    A failed `wake` disables waking for the rest of the run (warned once); a
    failed `cancel` doesn't, since a stray wake is harmless.
    """

    def __init__(
        self, every: timedelta, run: Callable[[list[str]], bool] | None = None
    ) -> None:
        self.every = every
        self.run = run or self._sudo_pmset
        self.at: datetime | None = None
        self.disabled = False

    @staticmethod
    def _sudo_pmset(args: list[str]) -> bool:
        result = subprocess.run(
            ["sudo", "-n", "pmset", "schedule", *args], capture_output=True, text=True
        )
        if result.returncode != 0:
            console.print(f"[dim]pmset schedule: {result.stderr.strip()}[/dim]")
        return result.returncode == 0

    def arm(self, now: datetime) -> None:
        """Make sure a wake is pending; re-arms after a fired wake."""
        if self.disabled or (self.at is not None and self.at > now):
            return
        when = now + self.every
        if self.run(["wake", when.strftime(PMSET_DATE), WAKE_OWNER]):
            self.at = when
            return
        self.disabled = True
        console.print(
            "[yellow]Can't schedule wakes[/yellow] - is mac/caff.sudoers "
            "installed? (see caff --help). Continuing without them."
        )

    def disarm(self, now: datetime) -> None:
        if self.at is None:
            return
        if self.at > now:  # a wake that already fired has nothing to cancel
            self.run(["cancel", "wake", self.at.strftime(PMSET_DATE), WAKE_OWNER])
        self.at = None


def write_state(
    mode: Mode,
    power: Power,
    floor: int,
    deadline: datetime | None,
    interval: float,
    now: datetime,
) -> None:
    write_json(
        STATE_FILE,
        {
            "pid": os.getpid(),
            "mode": mode.value,
            "floor": floor,
            "deadline": deadline.isoformat() if deadline else None,
            "updated": now.isoformat(),
            "interval": interval,
            "power": power.describe(),
        },
    )


def read_state(now: datetime) -> dict | None:
    """The watcher's last tick, or None if it isn't running.

    A live pid isn't enough: after a SIGKILL the file stays behind and the pid
    gets reused, so the tick must also be recent.
    """
    try:
        state = json.loads(STATE_FILE.read_text())
        os.kill(state["pid"], 0)
        age = now - datetime.fromisoformat(state["updated"])
        # .get: a watcher from before `interval` was recorded is still a watcher
        interval = state.get("interval", DEFAULT_INTERVAL)
        stale_after = max(STALE_TICKS * interval, STALE_MIN_SECONDS)
    except (OSError, ValueError, KeyError, TypeError):
        return None
    if age.total_seconds() > stale_after:
        return None
    return state


def status_line(
    mode: Mode,
    power: Power,
    floor: int,
    deadline: datetime | None,
    now: datetime,
    rate: float | None = None,
    agents: Agents | None = None,
    wake_at: datetime | None = None,
) -> Text:
    parts = [
        f"{now:%H:%M} {mode.label}",
        power.describe(),
        agents.describe() if agents is not None else "agents unknown",
        sleep_eta(power, floor, deadline, now, rate),
    ]
    if wake_at is not None:
        parts.append(f"wake {wake_at:%H:%M}")
    return Text.from_markup(" · ".join(parts))


def render(
    mode: Mode,
    power: Power,
    floor: int,
    deadline: datetime | None,
    now: datetime,
    history: History,
    agents: Agents | None = None,
    wake_at: datetime | None = None,
) -> tuple[Text, Text]:
    """(status line, graph line) for the current tick."""
    rate = drain_rate(history.samples, now)
    status = status_line(mode, power, floor, deadline, now, rate, agents, wake_at)
    graph = graph_line(history.recent(GRAPH_WINDOW, now), rate)
    return status, graph


def update_wake(
    waker: Waker | None, mode: Mode, power: Power, floor: int, now: datetime
) -> None:
    """Only wake to re-check agents; at/below the floor let the Mac sleep on."""
    if waker is None:
        return
    if mode == Mode.SLEEP and power.percent is not None and power.percent > floor:
        waker.arm(now)
        return
    waker.disarm(now)


def run_loop(
    floor: int,
    interval: float,
    deadline: datetime | None,
    caff: Caffeinator,
    history: History,
    waker: Waker | None = None,
    events: PowerEvents | None = None,
) -> None:
    wait = events.wait if events is not None else time.sleep
    last_mode: Mode | None = None
    with Live(console=console, transient=True) as live:
        while True:
            now = datetime.now()
            if deadline is not None and now >= deadline:
                console.print("[yellow]Time's up[/yellow] - letting the Mac sleep")
                return
            power = read_power()
            agents = read_agents()
            mode = desired_mode(power, floor, agents)
            caff.ensure(mode)
            update_wake(waker, mode, power, floor, now)
            history.record(power, now, agents)
            write_state(mode, power, floor, deadline, interval, now)
            wake_at = waker.at if waker is not None else None
            status, graph = render(
                mode, power, floor, deadline, now, history, agents, wake_at
            )
            if mode != last_mode:
                console.print(status)  # permanent log line on every transition
                last_mode = mode
            live.update(Group(status, graph))
            wait(interval)


@app.command()
def run(
    duration: Annotated[
        str | None,
        typer.Argument(
            help="Stop after this long, e.g. 60m or 4h. Omit to run forever."
        ),
    ] = None,
    floor: Annotated[
        int, typer.Option(help="On battery, allow sleep at or below this percent")
    ] = DEFAULT_FLOOR,
    interval: Annotated[
        float, typer.Option(help="Seconds between power checks")
    ] = DEFAULT_INTERVAL,
    wake: Annotated[
        str | None,
        typer.Option(
            help="When asleep with idle agents, wake this often to re-check "
            "(needs mac/caff.sudoers). 'off' to disable."
        ),
    ] = DEFAULT_WAKE,
) -> None:
    """Keep the Mac awake while plugged in, or on battery while Herdr agents work."""
    if sys.platform != "darwin":
        console.print("[red]caff only works on macOS[/red]")
        raise typer.Exit(1)
    if (state := read_state(datetime.now())) is not None:
        console.print(f"[red]caff is already running[/red] (pid {state['pid']})")
        raise typer.Exit(1)

    deadline: datetime | None = None
    waker: Waker | None = None
    try:
        if duration is not None:
            deadline = datetime.now() + parse_duration(duration)
        if wake is not None and wake.lower() != "off":
            waker = Waker(parse_duration(wake))
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise typer.Exit(2)
    until = f"until {deadline:%H:%M}" if deadline else "forever"
    console.print(
        f"Caffeinating {until}: plugged in, or on battery above {floor}% "
        f"while agents are busy. Ctrl-C to stop."
    )

    caff = Caffeinator()

    def shutdown(signum, frame):  # noqa: ARG001
        raise SystemExit(0)

    signal.signal(signal.SIGINT, shutdown)
    signal.signal(signal.SIGTERM, shutdown)

    try:
        run_loop(floor, interval, deadline, caff, History(), waker, PowerEvents())
    finally:
        caff.stop()
        if waker is not None:
            waker.disarm(datetime.now())
        STATE_FILE.unlink(missing_ok=True)
        console.print("Stopped - letting the Mac sleep")


@app.command()
def info() -> None:
    """Show battery state, drain history, and what the watcher is doing."""
    power = read_power()
    agents = read_agents()
    now = datetime.now()
    history = History()
    state = read_state(now)
    if state is None:
        console.print("[yellow]caff is not running[/yellow]")
        mode = desired_mode(power, DEFAULT_FLOOR, agents)
        who = agents.describe() if agents is not None else "agents unknown"
        console.print(
            f"{power.describe()} · {who} · would be {mode.label} "
            f"at floor {DEFAULT_FLOOR}%"
        )
        rate = drain_rate(history.samples, now)
        console.print(sleep_eta(power, DEFAULT_FLOOR, None, now, rate))
        console.print(graph_line(history.recent(GRAPH_WINDOW, now), rate))
        return
    until = state.get("deadline")
    deadline = datetime.fromisoformat(until) if until else None
    mode = Mode(state["mode"])
    console.print(f"caff running (pid {state['pid']}, floor {state['floor']}%)")
    status, graph = render(mode, power, state["floor"], deadline, now, history, agents)
    console.print(status)
    console.print(graph)


SUBCOMMANDS = {"run", "info"}


def cli() -> None:
    """`caff 4h` / `caff --floor 40` mean `caff run ...`; keep `caff info` as is."""
    args = sys.argv[1:]
    if not args or (args[0] not in SUBCOMMANDS and args[0] != "--help"):
        sys.argv.insert(1, "run")
    app()


if __name__ == "__main__":
    cli()
