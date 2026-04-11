"""Tests for running_servers.

Uses a MockAdapter so tests work on any platform without touching real /proc or lsof.
"""

from pathlib import Path

import pytest
from typer.testing import CliRunner

import running_servers
from running_servers import PlatformAdapter, ServerFinder, app


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
