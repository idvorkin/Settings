#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest", "typer", "rich"]
# ///
"""Tests for caff. Run directly: ./test_caff.py"""

import json
import os
import subprocess
import sys
from datetime import datetime, timedelta
from pathlib import Path

import pytest
from typer.testing import CliRunner

sys.path.insert(0, str(Path(__file__).parent))

import caff  # noqa: E402
from caff import (  # noqa: E402
    SPARK_CHARS,
    UNKNOWN_POWER,
    Agents,
    Caffeinator,
    History,
    Mode,
    Power,
    Sample,
    Waker,
    desired_mode,
    drain_rate,
    graph_line,
    minutes_to_floor,
    parse_agents,
    parse_duration,
    parse_pmset,
    sleep_eta,
    sparkline,
    status_line,
    update_wake,
)

BATTERY = (
    "Now drawing from 'Battery Power'\n"
    " -InternalBattery-0 (id=35389539)\t90%; discharging; 14:28 remaining present: true\n"
)
BATTERY_NO_ESTIMATE = (
    "Now drawing from 'Battery Power'\n"
    " -InternalBattery-0 (id=35389539)\t90%; discharging; (no estimate) present: true\n"
)
AC_CHARGING = (
    "Now drawing from 'AC Power'\n"
    " -InternalBattery-0 (id=35389539)\t60%; charging; 1:23 remaining present: true\n"
)
DESKTOP = "Now drawing from 'AC Power'\n"
HERDR = json.dumps(
    {
        "id": "cli:agent:list",
        "result": {
            "agents": [
                {"agent": "claude", "agent_status": "idle", "pane_id": "w1:p1"},
                {"agent": "claude", "agent_status": "working", "pane_id": "w2:p1"},
                {"agent": "claude", "agent_status": "blocked", "pane_id": "w3:p1"},
                {"agent": "codex", "agent_status": "unknown", "pane_id": "w4:p1"},
            ]
        },
    }
)


@pytest.mark.parametrize(
    "text,expected",
    [
        ("60m", timedelta(minutes=60)),
        ("60 min", timedelta(minutes=60)),
        ("90", timedelta(minutes=90)),
        ("4h", timedelta(hours=4)),
        ("4hr", timedelta(hours=4)),
        ("1.5 hours", timedelta(hours=1.5)),
        ("2H", timedelta(hours=2)),
    ],
)
def test_parse_duration(text, expected):
    assert parse_duration(text) == expected


@pytest.mark.parametrize("text", ["", "abc", "0m", "-5m", "4d", "h", "99999999999h"])
def test_parse_duration_rejects(text):
    with pytest.raises(ValueError):
        parse_duration(text)


def test_parse_pmset_battery():
    assert parse_pmset(BATTERY) == Power(False, 90, 14 * 60 + 28)


def test_parse_pmset_battery_no_estimate():
    assert parse_pmset(BATTERY_NO_ESTIMATE) == Power(False, 90, None)


def test_parse_pmset_ac_ignores_time_to_full():
    assert parse_pmset(AC_CHARGING) == Power(True, 60, None)


def test_parse_pmset_desktop():
    assert parse_pmset(DESKTOP) == Power(True, None, None)


@pytest.mark.parametrize(
    "power,expected",
    [
        (Power(True, 5, None), Mode.FULL),  # plugged in always wins
        (Power(True, None, None), Mode.FULL),  # desktop
        (Power(False, 90, 100), Mode.IDLE),
        (Power(False, 51, 100), Mode.IDLE),
        (Power(False, 50, 100), Mode.SLEEP),  # at the floor -> sleep
        (Power(False, 3, 100), Mode.SLEEP),
        (UNKNOWN_POWER, Mode.SLEEP),  # can't see the floor: don't risk the battery
    ],
)
def test_desired_mode(power, expected):
    assert desired_mode(power, floor=50) is expected


@pytest.mark.parametrize("output", ["", "pmset: error\n", "Now drawing from 'Batt"])
def test_unreadable_pmset_never_keeps_the_mac_awake(output):
    power = parse_pmset(output)
    assert power.unknown
    assert desired_mode(power, 50, Agents(3, 3)) is Mode.SLEEP
    assert "unknown" in power.describe()


def test_desktop_is_not_unknown_power():
    assert not parse_pmset(DESKTOP).unknown


class FakeRun:
    """Stands in for subprocess.run: returns a result or raises."""

    def __init__(self, stdout: str = "", returncode: int = 0, error=None) -> None:
        self.stdout, self.stderr, self.returncode = stdout, "", returncode
        self.error = error

    def __call__(self, *args, **kwargs):
        if self.error is not None:
            raise self.error
        return self


@pytest.mark.parametrize(
    "run",
    [
        FakeRun(BATTERY, returncode=1),
        FakeRun(error=FileNotFoundError("pmset")),
        FakeRun(error=subprocess.TimeoutExpired("pmset", 5)),
    ],
)
def test_read_power_failures_are_unknown(monkeypatch, run):
    monkeypatch.setattr(caff.subprocess, "run", run)
    assert caff.read_power() == UNKNOWN_POWER


def test_read_power_success(monkeypatch):
    monkeypatch.setattr(caff.subprocess, "run", FakeRun(BATTERY))
    assert caff.read_power() == Power(False, 90, 14 * 60 + 28)


@pytest.mark.parametrize(
    "run",
    [
        FakeRun(HERDR, returncode=1),
        FakeRun(error=FileNotFoundError("herdr")),
        FakeRun(error=subprocess.TimeoutExpired("herdr", 5)),
        FakeRun("not json"),
        FakeRun('{"result": {}}'),
        FakeRun('{"result": {"agents": ["x"]}}'),  # entries aren't dicts
        FakeRun('{"result": {"agents": {"a": 1}}}'),
    ],
)
def test_read_agents_failures_are_unknown_not_a_crash(monkeypatch, run):
    monkeypatch.setattr(caff.subprocess, "run", run)
    assert caff.read_agents() is None


def test_parse_agents_counts_only_working_as_busy():
    assert parse_agents(HERDR) == Agents(busy=1, total=4)


def test_parse_agents_empty():
    assert parse_agents('{"result": {"agents": []}}') == Agents(0, 0)


@pytest.mark.parametrize(
    "power,agents,expected",
    [
        (Power(True, 90, None), Agents(0, 3), Mode.FULL),  # AC ignores agents
        (Power(False, 90, 100), Agents(1, 3), Mode.IDLE),  # someone is busy
        (Power(False, 90, 100), Agents(0, 3), Mode.SLEEP),  # everyone idle
        (Power(False, 90, 100), Agents(0, 0), Mode.SLEEP),  # no agents at all
        (Power(False, 90, 100), None, Mode.IDLE),  # Herdr unreachable: stay up
        (Power(False, 40, 100), Agents(2, 3), Mode.SLEEP),  # floor beats agents
    ],
)
def test_desired_mode_with_agents(power, agents, expected):
    assert desired_mode(power, 50, agents) is expected


class FakePmset:
    def __init__(self, ok: bool = True) -> None:
        self.ok = ok
        self.calls: list[list[str]] = []

    def __call__(self, args: list[str]) -> bool:
        self.calls.append(args)
        return self.ok


def test_waker_arms_once_until_the_wake_fires():
    pmset = FakePmset()
    waker = Waker(timedelta(minutes=10), run=pmset)
    now = datetime(2026, 1, 1, 12, 0)
    waker.arm(now)
    waker.arm(now + timedelta(minutes=5))  # still pending: no second call
    assert pmset.calls == [["wake", "01/01/26 12:10:00", "caff"]]
    assert waker.at == datetime(2026, 1, 1, 12, 10)
    waker.arm(now + timedelta(minutes=10))  # fired: re-arm for 10 more
    assert pmset.calls[-1] == ["wake", "01/01/26 12:20:00", "caff"]


def test_waker_disarm_cancels_the_pending_wake():
    pmset = FakePmset()
    waker = Waker(timedelta(minutes=10), run=pmset)
    now = datetime(2026, 1, 1, 12, 0)
    waker.arm(now)
    waker.disarm(now)
    waker.disarm(now)  # nothing pending: no-op
    assert pmset.calls[-1] == ["cancel", "wake", "01/01/26 12:10:00", "caff"]
    assert len(pmset.calls) == 2
    assert waker.at is None


def test_waker_does_not_cancel_a_wake_that_already_fired():
    pmset = FakePmset()
    waker = Waker(timedelta(minutes=10), run=pmset)
    now = datetime(2026, 1, 1, 12, 0)
    waker.arm(now)
    waker.disarm(now + timedelta(minutes=11))
    assert pmset.calls == [["wake", "01/01/26 12:10:00", "caff"]]
    assert waker.at is None


def test_waker_survives_a_failed_cancel():
    pmset = FakePmset()
    waker = Waker(timedelta(minutes=10), run=pmset)
    now = datetime(2026, 1, 1, 12, 0)
    waker.arm(now)
    pmset.ok = False
    waker.disarm(now)
    pmset.ok = True
    waker.arm(now)
    assert not waker.disabled
    assert waker.at == datetime(2026, 1, 1, 12, 10)


def test_waker_gives_up_after_sudo_fails():
    pmset = FakePmset(ok=False)
    waker = Waker(timedelta(minutes=10), run=pmset)
    waker.arm(datetime(2026, 1, 1, 12, 0))
    waker.arm(datetime(2026, 1, 1, 12, 1))
    assert waker.at is None
    assert waker.disabled
    assert len(pmset.calls) == 1  # warned once, then stopped trying


@pytest.mark.parametrize(
    "mode,power,armed",
    [
        (Mode.SLEEP, Power(False, 80, 100), True),  # idle agents above the floor
        (Mode.SLEEP, Power(False, 50, 100), False),  # at the floor: sleep on
        (Mode.SLEEP, UNKNOWN_POWER, False),
        (Mode.IDLE, Power(False, 80, 100), False),  # agents busy
        (Mode.FULL, Power(True, 80, None), False),
    ],
)
def test_update_wake_only_arms_for_idle_agents_above_the_floor(mode, power, armed):
    waker = Waker(timedelta(minutes=10), run=FakePmset())
    update_wake(waker, mode, power, 50, NOW)
    assert (waker.at is not None) is armed


def test_update_wake_disarms_when_agents_get_busy():
    pmset = FakePmset()
    waker = Waker(timedelta(minutes=10), run=pmset)
    update_wake(waker, Mode.SLEEP, Power(False, 80, 100), 50, NOW)
    update_wake(waker, Mode.IDLE, Power(False, 80, 100), 50, NOW)
    assert pmset.calls[-1][0] == "cancel"
    update_wake(None, Mode.SLEEP, Power(False, 80, 100), 50, NOW)  # --wake off


class FakeProc:
    def __init__(self, args: list[str]) -> None:
        self.args = args
        self.alive = True
        self.stubborn = False  # ignores terminate()
        self.calls: list[str] = []

    def poll(self) -> int | None:
        return None if self.alive else 0

    def terminate(self) -> None:
        self.calls.append("terminate")
        self.alive = self.stubborn

    def kill(self) -> None:
        self.calls.append("kill")
        self.alive = False

    def wait(self, timeout: float | None = None) -> int:
        self.calls.append("wait")
        if self.alive:
            raise subprocess.TimeoutExpired(self.args, timeout or 0)
        return 0


class FakePopen:
    def __init__(self) -> None:
        self.procs: list[FakeProc] = []

    def __call__(self, args: list[str]) -> FakeProc:
        self.procs.append(FakeProc(args))
        return self.procs[-1]


def test_caffeinator_ties_the_child_to_our_pid():
    popen = FakePopen()
    Caffeinator(popen).ensure(Mode.FULL)
    assert popen.procs[0].args == [
        "caffeinate",
        "-d",
        "-i",
        "-s",
        "-w",
        str(os.getpid()),
    ]


def test_caffeinator_replaces_the_child_on_a_mode_change():
    popen = FakePopen()
    caffeinator = Caffeinator(popen)
    caffeinator.ensure(Mode.FULL)
    caffeinator.ensure(Mode.FULL)  # same mode, child alive: nothing to do
    assert len(popen.procs) == 1
    caffeinator.ensure(Mode.IDLE)
    assert not popen.procs[0].alive
    assert popen.procs[1].args[:2] == ["caffeinate", "-i"]


def test_caffeinator_sleep_runs_nothing():
    popen = FakePopen()
    caffeinator = Caffeinator(popen)
    caffeinator.ensure(Mode.IDLE)
    caffeinator.ensure(Mode.SLEEP)
    assert not popen.procs[0].alive
    assert len(popen.procs) == 1
    assert not caffeinator.running


def test_caffeinator_restarts_a_child_that_died():
    popen = FakePopen()
    caffeinator = Caffeinator(popen)
    caffeinator.ensure(Mode.IDLE)
    popen.procs[0].alive = False
    caffeinator.ensure(Mode.IDLE)
    assert len(popen.procs) == 2
    assert caffeinator.running


def test_caffeinator_kills_and_reaps_a_child_that_ignores_terminate():
    popen = FakePopen()
    caffeinator = Caffeinator(popen)
    caffeinator.ensure(Mode.IDLE)
    popen.procs[0].stubborn = True
    caffeinator.stop()
    assert popen.procs[0].calls == ["terminate", "wait", "kill", "wait"]


@pytest.fixture
def state_file(tmp_path, monkeypatch):
    path = tmp_path / "state.json"
    monkeypatch.setattr(caff, "STATE_FILE", path)
    return path


def test_state_round_trips_for_a_live_watcher(state_file):
    caff.write_state(Mode.IDLE, Power(False, 80, 100), 50, None, 30, NOW)
    state = caff.read_state(NOW + timedelta(seconds=45))
    assert state is not None
    assert (state["pid"], state["mode"], state["floor"]) == (os.getpid(), "idle", 50)
    assert not state_file.with_suffix(".tmp").exists()


def test_state_that_stopped_ticking_is_not_running(state_file):
    # SIGKILLed watcher + recycled pid: the pid is alive but nothing ticks
    caff.write_state(Mode.IDLE, Power(False, 80, 100), 50, None, 30, NOW)
    assert caff.read_state(NOW + timedelta(minutes=5)) is None


@pytest.mark.parametrize(
    "content",
    [
        "not json",
        "[]",
        '{"mode": "idle"}',
        '{"pid": "x", "updated": "", "interval": 1}',
    ],
)
def test_bad_state_file_is_not_running(state_file, content):
    state_file.write_text(content)
    assert caff.read_state(NOW) is None


def test_state_with_a_dead_pid_is_not_running(state_file):
    dead = subprocess.Popen(["true"])
    dead.wait()
    state_file.write_text(
        json.dumps({"pid": dead.pid, "updated": NOW.isoformat(), "interval": 30})
    )
    assert caff.read_state(NOW) is None


def test_state_from_a_watcher_predating_interval_still_counts(state_file):
    # an older caff is still running across the upgrade: don't start a second one
    state_file.write_text(json.dumps({"pid": os.getpid(), "updated": NOW.isoformat()}))
    assert caff.read_state(NOW + timedelta(seconds=45)) is not None


def test_unwritable_cache_does_not_stop_the_watcher(tmp_path, monkeypatch):
    blocker = tmp_path / "file"
    blocker.write_text("")
    monkeypatch.setattr(caff, "STATE_FILE", blocker / "state.json")  # parent is a file
    caff.write_state(Mode.IDLE, Power(False, 80, 100), 50, None, 30, NOW)
    history = History(blocker / "h.json")
    assert history.record(Power(False, 80, None), NOW)  # kept in memory


runner = CliRunner()


@pytest.fixture
def cli_env(state_file, monkeypatch):
    monkeypatch.setattr(caff.sys, "platform", "darwin")
    monkeypatch.setattr(caff.signal, "signal", lambda *args: None)  # keep pytest's
    monkeypatch.setattr(caff, "read_power", lambda: Power(False, 80, 100))
    monkeypatch.setattr(caff, "read_agents", lambda: Agents(1, 2))
    monkeypatch.setattr(caff, "History", lambda: History(state_file.parent / "h.json"))


@pytest.mark.parametrize("args", [["run", "4d"], ["run", "--wake", "soon"]])
def test_run_rejects_bad_durations(cli_env, args):
    assert runner.invoke(caff.app, args).exit_code == 2


def test_run_refuses_off_macos(cli_env, monkeypatch):
    monkeypatch.setattr(caff.sys, "platform", "linux")
    assert runner.invoke(caff.app, ["run"]).exit_code == 1


def test_run_refuses_a_second_watcher_and_leaves_its_state(cli_env, state_file):
    caff.write_state(Mode.IDLE, Power(False, 80, 100), 50, None, 30, datetime.now())
    result = runner.invoke(caff.app, ["run"])
    assert result.exit_code == 1
    assert "already running" in result.output
    assert state_file.exists()


def test_run_cleans_up_however_the_loop_ends(cli_env, state_file, monkeypatch):
    stopped: list[str] = []
    monkeypatch.setattr(Caffeinator, "stop", lambda self: stopped.append("caffeinate"))
    monkeypatch.setattr(Waker, "disarm", lambda self, now: stopped.append("wake"))

    def loop(*args, **kwargs):
        state_file.write_text("{}")
        raise SystemExit(0)

    monkeypatch.setattr(caff, "run_loop", loop)
    runner.invoke(caff.app, ["run"])
    assert stopped == ["caffeinate", "wake"]
    assert not state_file.exists()


def test_info_without_a_watcher(cli_env):
    result = runner.invoke(caff.app, ["info"])
    assert result.exit_code == 0
    assert "not running" in result.output
    assert "agents 1/2 busy" in result.output


def test_info_with_a_watcher(cli_env):
    caff.write_state(Mode.IDLE, Power(False, 80, 100), 50, None, 30, datetime.now())
    result = runner.invoke(caff.app, ["info"])
    assert result.exit_code == 0
    assert f"caff running (pid {os.getpid()}, floor 50%)" in result.output


@pytest.mark.parametrize(
    "argv,expected",
    [
        (["caff", "4h"], ["caff", "run", "4h"]),
        (["caff"], ["caff", "run"]),
        (["caff", "--floor", "40"], ["caff", "run", "--floor", "40"]),
        (["caff", "info"], ["caff", "info"]),
        (["caff", "--help"], ["caff", "--help"]),
    ],
)
def test_cli_defaults_to_run(monkeypatch, argv, expected):
    monkeypatch.setattr(caff.sys, "argv", list(argv))
    monkeypatch.setattr(caff, "app", lambda: None)
    caff.cli()
    assert caff.sys.argv == expected


def test_status_line_shows_agents_and_wake():
    now = datetime(2026, 1, 1, 12, 0)
    line = status_line(
        Mode.SLEEP,
        Power(False, 80, 100),
        50,
        None,
        now,
        agents=Agents(0, 4),
        wake_at=now + timedelta(minutes=10),
    ).plain
    assert "agents 0/4 busy" in line
    assert line.endswith("wake 12:10")
    assert (
        "agents unknown"
        in status_line(Mode.IDLE, Power(False, 80, 100), 50, None, now).plain
    )


def test_minutes_to_floor_scales_linearly():
    # 100 min to empty at 80%; 30 points above floor -> 3/8 of it
    assert minutes_to_floor(Power(False, 80, 100), floor=50) == 38


def test_minutes_to_floor_none_when_plugged_or_unknown():
    assert minutes_to_floor(Power(True, 80, None), 50) is None
    assert minutes_to_floor(Power(False, 80, None), 50) is None


def test_minutes_to_floor_prefers_measured_rate():
    # 30 points above floor at 10%/h -> 3h, regardless of pmset's guess
    assert minutes_to_floor(Power(False, 80, 100), 50, rate=10.0) == 180
    # a non-positive rate (battery gaining) falls back to pmset
    assert minutes_to_floor(Power(False, 80, 100), 50, rate=-1.0) == 38


NOW = datetime(2026, 1, 1, 12, 0)


def samples(*points: tuple[int, int], plugged_in: bool = False) -> list[Sample]:
    """(minutes before NOW, percent) pairs -> Samples, oldest first."""
    return [
        Sample(NOW - timedelta(minutes=ago), pct, plugged_in)
        for ago, pct in sorted(points, reverse=True)
    ]


def test_history_one_sample_per_minute(tmp_path):
    history = History(tmp_path / "h.json")
    assert history.record(Power(False, 80, None), NOW)
    assert not history.record(Power(False, 79, None), NOW + timedelta(seconds=30))
    assert history.record(Power(False, 79, None), NOW + timedelta(seconds=60))
    assert [s.percent for s in history.samples] == [80, 79]


def test_history_skips_desktops_and_persists(tmp_path):
    path = tmp_path / "h.json"
    history = History(path)
    assert not history.record(Power(True, None, None), NOW)
    history.record(Power(True, 55, None), NOW)
    reloaded = History(path)
    assert reloaded.samples == [Sample(NOW, 55, True)]


def test_history_prunes_old_samples(tmp_path):
    history = History(tmp_path / "h.json", keep=timedelta(hours=1))
    history.record(Power(False, 90, None), NOW - timedelta(hours=2))
    history.record(Power(False, 80, None), NOW)
    assert [s.percent for s in history.samples] == [80]


def test_history_tolerates_corrupt_file(tmp_path):
    path = tmp_path / "h.json"
    path.write_text("not json")
    assert History(path).samples == []


def test_drain_rate_from_recent_battery_samples():
    # 90% -> 85% over 30 minutes = 10%/h
    assert drain_rate(samples((30, 90), (15, 88), (0, 85)), NOW) == 10.0


def test_drain_rate_ignores_samples_before_last_charge():
    plugged = samples((40, 50), plugged_in=True)
    battery = samples((30, 90), (0, 85))
    assert drain_rate(plugged + battery, NOW) == 10.0


def test_drain_rate_needs_enough_data():
    assert drain_rate(samples((2, 90), (0, 89)), NOW) is None
    assert drain_rate(samples((0, 90)), NOW) is None
    assert drain_rate(samples((30, 90), (0, 85), plugged_in=True), NOW) is None


def test_drain_rate_caps_window_to_an_hour():
    # the 3h-old sample is out of the window; 60% -> 55% over 60 min
    assert drain_rate(samples((180, 90), (60, 60), (0, 55)), NOW) == 5.0


def test_sparkline_scales_to_range_and_colors_charging():
    battery = samples((3, 80), (2, 75), (1, 60))
    charging = samples((0, 65), plugged_in=True)
    line = sparkline(battery + charging)
    assert (
        line.plain == SPARK_CHARS[7] + SPARK_CHARS[5] + SPARK_CHARS[0] + SPARK_CHARS[2]
    )
    assert [span.style for span in line.spans if span.style] == ["green"]


def test_sparkline_buckets_down_to_width():
    line = sparkline(samples(*((i, 100 - i) for i in range(120))), width=60)
    assert len(line.plain) == 60
    assert line.plain[0] == SPARK_CHARS[0] and line.plain[-1] == SPARK_CHARS[7]


def test_sparkline_waits_for_two_samples():
    assert "collecting" in sparkline(samples((0, 80))).plain


def test_graph_line_caption():
    line = graph_line(samples((30, 90), (0, 85)), rate=10.0)
    assert line.plain.endswith("90%→85% 11:30–12:00 · draining 10.0%/h")


def test_sleep_eta_mentions_floor_and_timer():
    now = datetime(2026, 1, 1, 12, 0)
    line = sleep_eta(Power(False, 80, 100), 50, now + timedelta(hours=2), now)
    assert "floor 50% in ~38m" in line
    assert "timer ends 14:00 (2h00m)" in line


def test_sleep_eta_plugged_in_no_timer():
    assert sleep_eta(Power(True, 80, None), 50, None, datetime.now()) == (
        "no sleep scheduled"
    )


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v"]))
