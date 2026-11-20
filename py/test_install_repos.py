#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "pydantic", "pytest"]
# ///
"""Tests for install_repos.py"""

import sys
from pathlib import Path

import pytest
from typer.testing import CliRunner

sys.path.insert(0, str(Path(__file__).parent))
import install_repos
from install_repos import Repo, Runner


class MockRunner(Runner):
    def __init__(self, fail_on: str | None = None):
        self.calls: list[tuple[list[str], Path | None]] = []
        self.fail_on = fail_on

    def run(self, cmd: list[str], cwd: Path | None = None) -> bool:
        self.calls.append((cmd, cwd))
        return not (self.fail_on and any(self.fail_on in part for part in cmd))


@pytest.fixture
def env(tmp_path, monkeypatch):
    mock = MockRunner()
    monkeypatch.setattr(install_repos, "runner", mock)
    monkeypatch.setattr(install_repos, "GITS", tmp_path)
    monkeypatch.setattr(
        install_repos,
        "REPOS",
        [
            Repo(name="tool", install=["make install"], note="grant access"),
            Repo(name="app"),
        ],
    )
    return mock, tmp_path


cli = CliRunner()


def test_clone_skips_existing_checkout(env):
    mock, gits = env
    (gits / "tool").mkdir()
    result = cli.invoke(install_repos.app, ["clone"])
    assert result.exit_code == 0
    assert mock.calls == [(["gh", "repo", "clone", "idvorkin/app"], gits)]


def test_install_runs_steps_in_checkout_and_prints_notes(env):
    mock, gits = env
    result = cli.invoke(install_repos.app, ["install", "tool"])
    assert result.exit_code == 0
    assert mock.calls == [
        (["gh", "repo", "clone", "idvorkin/tool"], gits),
        (["bash", "-c", "make install"], gits / "tool"),
    ]
    assert "grant access" in result.output


def test_install_reports_failure_and_continues(env, monkeypatch):
    mock = MockRunner(fail_on="make install")
    monkeypatch.setattr(install_repos, "runner", mock)
    result = cli.invoke(install_repos.app, ["install"])
    assert result.exit_code == 1
    assert "Failed: tool" in result.output
    assert any(cmd[-1] == "idvorkin/app" for cmd, _ in mock.calls)


def test_clone_uses_repo_owner(env, monkeypatch):
    mock, gits = env
    monkeypatch.setattr(
        install_repos, "REPOS", [Repo(name="tool", owner="someone-else")]
    )
    result = cli.invoke(install_repos.app, ["clone"])
    assert result.exit_code == 0
    assert mock.calls == [(["gh", "repo", "clone", "someone-else/tool"], gits)]


def test_unknown_repo_exits(env):
    mock, _ = env
    result = cli.invoke(install_repos.app, ["install", "nope"])
    assert result.exit_code == 1
    assert "Unknown repos: nope" in result.output
    assert mock.calls == []


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v"]))
