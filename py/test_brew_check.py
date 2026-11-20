#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["typer", "rich", "questionary", "pytest"]
# ///
"""Tests for brew_check.py"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import brew_check

MAS_LIST_OUTPUT = """\
497799835   Xcode          (27.2)
1542143627  OmniFocus 4    (4.6)
310633997   WhatsApp       (25.1)
"""


def test_parse_mas_list_reads_ids():
    assert brew_check.parse_mas_list(MAS_LIST_OUTPUT) == {
        497799835,
        1542143627,
        310633997,
    }


def test_parse_mas_list_ignores_blank_and_non_id_lines():
    assert brew_check.parse_mas_list("\nNo installed apps found\n") == set()


def test_missing_mas_apps_skips_installed():
    installed = brew_check.parse_mas_list(MAS_LIST_OUTPUT)
    missing = {name for name, _ in brew_check.missing_mas_apps(installed)}
    assert "Xcode" not in missing
    assert "Goodnotes" in missing


def test_mas_app_ids_are_unique():
    ids = [app_id for _, app_id in brew_check.MAS_APPS]
    assert len(ids) == len(set(ids))


def test_calibre_is_a_tracked_cask():
    casks = [c for _, pkgs in brew_check.CASK_CATEGORIES for c in pkgs]
    assert "calibre" in casks


def test_parse_npm_globals_reads_package_names():
    out = '{"dependencies": {"@xai-official/grok": {"version": "1.0.77"}, "npm": {}}}'
    assert brew_check.parse_npm_globals(out) == {"@xai-official/grok", "npm"}


def test_parse_npm_globals_handles_empty_output():
    assert brew_check.parse_npm_globals("") == set()


def test_grok_is_a_tracked_npm_global():
    assert "@xai-official/grok" in brew_check.NPM_GLOBALS


def test_claude_code_not_installed_via_brew():
    formulae = [f for _, pkgs in brew_check.FORMULA_CATEGORIES for f in pkgs]
    assert "claude-code" not in formulae


def test_only_idvorkin_and_yabai_taps():
    assert brew_check.MAC_TAPS == ["asmvik/formulae", "idvorkin/tap"]


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v"]))
