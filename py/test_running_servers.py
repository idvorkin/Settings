#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest", "typer", "rich"]
# ///
"""Tests for running_servers.

Uses a MockAdapter so tests work on any platform without touching real /proc or lsof.

Run directly with uv (no venv setup needed):
    ./test_running_servers.py
"""

import sys
from pathlib import Path

import pytest
from typer.testing import CliRunner

# Make sibling running_servers.py importable regardless of cwd.
sys.path.insert(0, str(Path(__file__).parent))

import running_servers  # noqa: E402
from running_servers import PlatformAdapter, ServerFinder, app  # noqa: E402


class MockAdapter(PlatformAdapter):
    """Configurable adapter for tests.

    servers: list of dicts with keys: port, pid, cwd, name, cmdline, ppid
    """

    def __init__(self, servers: list[dict]):
        self._by_pid = {s["pid"]: s for s in servers}
        self._ports = {s["port"]: s["pid"] for s in servers}

    def get_listening_ports(self) -> dict[int, int]:
        return dict(self._ports)

    def get_process_cwd(self, pid: int) -> Path | None:
        s = self._by_pid.get(pid)
        return Path(s["cwd"]) if s and s.get("cwd") else None

    def get_process_name(self, pid: int) -> str | None:
        s = self._by_pid.get(pid)
        return s["name"] if s else None

    def get_process_cmdline(self, pid: int) -> str | None:
        s = self._by_pid.get(pid)
        return s["cmdline"] if s else None

    def get_parent_pid(self, pid: int) -> int | None:
        s = self._by_pid.get(pid)
        return s.get("ppid") if s else None


# --- ServerFinder unit tests (using MockAdapter directly) ---


def test_find_for_directory_matches(tmp_path):
    adapter = MockAdapter(
        [
            {
                "port": 4000,
                "pid": 100,
                "cwd": str(tmp_path),
                "name": "ruby",
                "cmdline": "jekyll serve",
            },
        ]
    )
    finder = ServerFinder(adapter)
    servers = finder.find_for_directory(tmp_path)
    assert len(servers) == 1
    assert servers[0]["port"] == 4000
    assert servers[0]["cmdline"] == "jekyll serve"


def test_find_for_directory_excludes_other_dirs(tmp_path):
    other = tmp_path / "other"
    other.mkdir()
    target = tmp_path / "target"
    target.mkdir()
    adapter = MockAdapter(
        [
            {
                "port": 4000,
                "pid": 100,
                "cwd": str(other),
                "name": "ruby",
                "cmdline": "jekyll serve",
            },
        ]
    )
    finder = ServerFinder(adapter)
    assert finder.find_for_directory(target) == []


# --- CLI tests: reproduce issue #60 and lock in the fix ---


@pytest.fixture
def runner():
    return CliRunner()


def _patch_adapter(monkeypatch, servers: list[dict]):
    """Replace get_adapter so CLI commands use our MockAdapter."""
    monkeypatch.setattr(running_servers, "get_adapter", lambda: MockAdapter(servers))


def test_check_no_servers_exits_nonzero(runner, monkeypatch, tmp_path):
    """check on an empty dir must exit non-zero so shell callers can react."""
    _patch_adapter(monkeypatch, [])
    result = runner.invoke(app, ["check", str(tmp_path)])
    assert result.exit_code != 0, result.output
    assert "No servers" in result.output


def test_check_with_server_exits_zero(runner, monkeypatch, tmp_path):
    _patch_adapter(
        monkeypatch,
        [
            {
                "port": 4000,
                "pid": 100,
                "cwd": str(tmp_path),
                "name": "ruby",
                "cmdline": "jekyll serve",
            }
        ],
    )
    result = runner.invoke(app, ["check", str(tmp_path)])
    assert result.exit_code == 0, result.output
    assert "4000" in result.output


def test_check_port_filter_mismatch_exits_nonzero(runner, monkeypatch, tmp_path):
    """Issue #60 core case: stray node on :40816 should not satisfy
    `check --port 4000`."""
    _patch_adapter(
        monkeypatch,
        [
            {
                "port": 40816,
                "pid": 200,
                "cwd": str(tmp_path),
                "name": "node",
                "cmdline": "node some-dev-tool.js",
            }
        ],
    )
    result = runner.invoke(app, ["check", str(tmp_path), "--port", "4000"])
    assert result.exit_code != 0, result.output
    assert "4000" in result.output  # mentions the expected port


def test_check_port_filter_match_exits_zero(runner, monkeypatch, tmp_path):
    _patch_adapter(
        monkeypatch,
        [
            {
                "port": 4000,
                "pid": 100,
                "cwd": str(tmp_path),
                "name": "ruby",
                "cmdline": "jekyll serve",
            },
            {
                "port": 40816,
                "pid": 200,
                "cwd": str(tmp_path),
                "name": "node",
                "cmdline": "node some-dev-tool.js",
            },
        ],
    )
    result = runner.invoke(app, ["check", str(tmp_path), "--port", "4000"])
    assert result.exit_code == 0, result.output


def test_check_process_filter_mismatch_exits_nonzero(runner, monkeypatch, tmp_path):
    _patch_adapter(
        monkeypatch,
        [
            {
                "port": 40816,
                "pid": 200,
                "cwd": str(tmp_path),
                "name": "node",
                "cmdline": "node some-dev-tool.js",
            }
        ],
    )
    result = runner.invoke(app, ["check", str(tmp_path), "--process", "jekyll"])
    assert result.exit_code != 0, result.output


def test_check_process_filter_match_exits_zero(runner, monkeypatch, tmp_path):
    _patch_adapter(
        monkeypatch,
        [
            {
                "port": 4000,
                "pid": 100,
                "cwd": str(tmp_path),
                "name": "ruby",
                "cmdline": "jekyll serve",
            }
        ],
    )
    result = runner.invoke(app, ["check", str(tmp_path), "--process", "jekyll"])
    assert result.exit_code == 0, result.output


# --- plan_run unit tests (pure decision logic, no exec) ---


def _jekyll(port, pid, cwd, livereload_port=None):
    """Helper: a jekyll server record (optionally its livereload sibling)."""
    rows = [
        {
            "port": port,
            "pid": pid,
            "cwd": str(cwd),
            "name": "ruby",
            "cmdline": f"jekyll serve --port {port}",
        }
    ]
    if livereload_port is not None:
        rows.append(
            {
                "port": livereload_port,
                "pid": pid,  # same process, second socket
                "cwd": str(cwd),
                "name": "ruby",
                "cmdline": f"jekyll serve --port {port}",
            }
        )
    return rows


def test_plan_run_clear_dir_picks_requested(tmp_path):
    finder = ServerFinder(MockAdapter([]))
    plan = finder.plan_run(tmp_path, process="jekyll", http=4000, livereload=35729)
    assert plan["conflict"] == []
    assert plan["http"] == 4000
    assert plan["livereload"] == 35729


def test_plan_run_conflict_when_jekyll_here(tmp_path):
    finder = ServerFinder(MockAdapter(_jekyll(4000, 100, tmp_path)))
    plan = finder.plan_run(tmp_path, process="jekyll", http=4000)
    assert len(plan["conflict"]) == 1
    assert plan["conflict"][0]["port"] == 4000


def test_plan_run_dedupes_multi_port_server(tmp_path):
    """A jekyll holding http+livereload is ONE conflict, not two."""
    finder = ServerFinder(
        MockAdapter(_jekyll(4000, 100, tmp_path, livereload_port=35729))
    )
    plan = finder.plan_run(tmp_path, process="jekyll", http=4000)
    assert len(plan["conflict"]) == 1


def test_plan_run_other_dir_does_not_conflict_and_drifts(tmp_path):
    """blog4 on :4000 in another dir → no conflict, this repo drifts to :4001."""
    other = tmp_path / "blog4"
    other.mkdir()
    here = tmp_path / "blog6"
    here.mkdir()
    finder = ServerFinder(MockAdapter(_jekyll(4000, 100, other, livereload_port=35729)))
    plan = finder.plan_run(here, process="jekyll", http=4000, livereload=35729)
    assert plan["conflict"] == []
    assert plan["http"] == 4001
    assert plan["http_owner"] == str(other)
    assert plan["livereload"] == 35730


def test_plan_run_non_matching_process_is_not_a_conflict(tmp_path):
    """A serena/node server in this dir must not block a jekyll launch."""
    finder = ServerFinder(
        MockAdapter(
            [
                {
                    "port": 24283,
                    "pid": 200,
                    "cwd": str(tmp_path),
                    "name": "python",
                    "cmdline": "serena start-mcp-server",
                }
            ]
        )
    )
    plan = finder.plan_run(tmp_path, process="jekyll", http=4000)
    assert plan["conflict"] == []
    assert plan["http"] == 4000


# --- run CLI tests ---


def test_run_dry_run_substitutes_ports(runner, monkeypatch, tmp_path):
    _patch_adapter(monkeypatch, [])
    result = runner.invoke(
        app,
        [
            "run",
            "--dir",
            str(tmp_path),
            "--process",
            "jekyll",
            "--http",
            "4000",
            "--livereload",
            "35729",
            "--dry-run",
            "--",
            "jekyll",
            "serve",
            "--port",
            "{http}",
            "--livereload-port",
            "{livereload}",
        ],
    )
    assert result.exit_code == 0, result.output
    assert "jekyll serve --port 4000 --livereload-port 35729" in result.stdout


def test_run_fails_when_already_here(runner, monkeypatch, tmp_path):
    _patch_adapter(monkeypatch, _jekyll(4000, 100, tmp_path))
    result = runner.invoke(
        app,
        [
            "run",
            "--dir",
            str(tmp_path),
            "--process",
            "jekyll",
            "--dry-run",
            "--",
            "jekyll",
            "serve",
        ],
    )
    assert result.exit_code == 1, result.output
    assert "already running here" in result.output


def test_run_drifts_off_busy_port(runner, monkeypatch, tmp_path):
    other = tmp_path / "blog4"
    other.mkdir()
    here = tmp_path / "blog6"
    here.mkdir()
    _patch_adapter(monkeypatch, _jekyll(4000, 100, other))
    result = runner.invoke(
        app,
        [
            "run",
            "--dir",
            str(here),
            "--process",
            "jekyll",
            "--http",
            "4000",
            "--dry-run",
            "--",
            "jekyll",
            "serve",
            "--port",
            "{http}",
        ],
    )
    assert result.exit_code == 0, result.output
    assert "jekyll serve --port 4001" in result.stdout


def test_run_rejects_unfilled_livereload_placeholder(runner, monkeypatch, tmp_path):
    _patch_adapter(monkeypatch, [])
    result = runner.invoke(
        app,
        [
            "run",
            "--dir",
            str(tmp_path),
            "--dry-run",
            "--",
            "jekyll",
            "--livereload-port",
            "{livereload}",
        ],
    )
    assert result.exit_code == 2, result.output


# --- version command (used by callers to gate on capabilities) ---


def test_version_prints_bare_semver(runner):
    result = runner.invoke(app, ["version"])
    assert result.exit_code == 0, result.output
    out = result.stdout.strip()
    assert out == running_servers.__version__
    assert out.count(".") == 2  # X.Y.Z, no rich markup


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v"]))
