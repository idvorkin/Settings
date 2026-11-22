#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "pydantic", "pytest"]
# ///
"""Tests for install_repos.py"""

import json
import sys
from pathlib import Path

import pytest
from typer.testing import CliRunner

sys.path.insert(0, str(Path(__file__).parent))
import install_repos
from install_repos import Repo, Runner


class MockRunner(Runner):
    """Records run() calls; output() hands out `heads` in turn (rev-parse HEAD)."""

    def __init__(self, fail_on: str | None = None, heads: list[str] | None = None):
        self.calls: list[tuple[list[str], Path | None]] = []
        self.fail_on = fail_on
        self.heads = list(heads or [])

    def run(self, cmd: list[str], cwd: Path | None = None) -> bool:
        self.calls.append((cmd, cwd))
        return not (self.fail_on and any(self.fail_on in part for part in cmd))

    def output(self, cmd: list[str], cwd: Path | None = None) -> str | None:
        return self.heads.pop(0) if self.heads else "same"


PULL = ["git", "pull", "--ff-only", "--no-autostash", "--quiet"]


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


def test_existing_up_to_date_repo_is_pulled_not_reinstalled(env):
    mock, gits = env
    (gits / "tool").mkdir()
    result = cli.invoke(install_repos.app, ["install", "tool"])
    assert result.exit_code == 0
    assert mock.calls == [(PULL, gits / "tool")]
    assert "up to date" in result.output


def test_pull_that_moves_head_reinstalls(env, monkeypatch):
    mock = MockRunner(heads=["old", "new"])
    monkeypatch.setattr(install_repos, "runner", mock)
    gits = env[1]
    (gits / "tool").mkdir()
    result = cli.invoke(install_repos.app, ["install", "tool"])
    assert result.exit_code == 0
    assert mock.calls == [
        (PULL, gits / "tool"),
        (["bash", "-c", "make install"], gits / "tool"),
    ]


def test_missing_marker_reinstalls_without_new_commits(env, monkeypatch):
    mock, gits = env
    (gits / "tool").mkdir()
    monkeypatch.setattr(
        install_repos, "REPOS",
        [Repo(name="tool", install=["make install"], marker=str(gits / "nope"))],
    )  # fmt: skip
    result = cli.invoke(install_repos.app, ["install", "tool"])
    assert result.exit_code == 0
    assert (["bash", "-c", "make install"], gits / "tool") in mock.calls


def test_failed_pull_is_reported_and_run_continues(env, monkeypatch):
    mock = MockRunner(fail_on="--ff-only")
    monkeypatch.setattr(install_repos, "runner", mock)
    gits = env[1]
    (gits / "tool").mkdir()
    (gits / "app").mkdir()
    result = cli.invoke(install_repos.app, ["install"])
    assert result.exit_code == 0
    assert "tool not updated" in result.output
    assert "app not updated" in result.output
    assert "up to date" not in result.output


def test_list_json_reports_kind_and_installed(env):
    _, gits = env
    (gits / "app").mkdir()
    result = cli.invoke(install_repos.app, ["list", "--json"])
    rows = {r["name"]: r for r in json.loads(result.output)}
    assert rows["app"]["installed"] is True
    assert rows["tool"]["installed"] is False
    assert rows["tool"]["title"] == "Git Repos: OS"


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v"]))
