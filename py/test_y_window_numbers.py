#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "pytest",
#     "typer",
#     "rich",
#     "icecream",
#     "pydantic",
#     "pyperclip",
#     "psutil",
#     "pyobjc-framework-Quartz; sys_platform == 'darwin'",
#     "pyobjc-framework-Cocoa; sys_platform == 'darwin'",
# ]
# ///
"""Regression tests for window numbering; run ./py/test_y_window_numbers.py."""

import json
from pathlib import Path
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

import pytest

import y


def window(
    window_id,
    x,
    y_position,
    *,
    visible=True,
    minimized=False,
    hidden=False,
    sticky=False,
    space=1,
    subrole="AXStandardWindow",
):
    return SimpleNamespace(
        id=window_id,
        frame=SimpleNamespace(x=x, y=y_position, w=100, h=100),
        is_visible=visible,
        is_minimized=minimized,
        is_hidden=hidden,
        is_sticky=sticky,
        space=space,
        subrole=subrole,
    )


class WindowNumberingTests(unittest.TestCase):
    def test_overlay_app_can_own_windows_without_appearing_in_the_dock(self):
        app_instance = SimpleNamespace(
            setActivationPolicy_=Mock(),
            finishLaunching=Mock(),
        )
        appkit = SimpleNamespace(
            NSApplication=SimpleNamespace(
                sharedApplication=Mock(return_value=app_instance)
            ),
            NSApplicationActivationPolicyAccessory="accessory",
        )

        with patch.object(y, "AppKit", appkit):
            self.assertIs(y._overlay_application(), app_instance)

        app_instance.setActivationPolicy_.assert_called_once_with("accessory")
        app_instance.finishLaunching.assert_called_once_with()

    @patch.object(y, "AppKit", new=Mock())
    @patch.object(y.subprocess, "Popen")
    def test_overlay_is_detached_from_the_invoking_app(self, popen):
        y._launch_window_number_overlay(9)

        command = [
            y.sys.executable,
            str(y.Path(y.__file__).resolve()),
            "number",
            "--overlay",
            "--seconds",
            "9",
        ]
        popen.assert_called_once_with(
            command,
            stdin=y.subprocess.DEVNULL,
            stdout=y.subprocess.DEVNULL,
            stderr=None,
            close_fds=True,
            cwd="/",
            start_new_session=True,
        )

    @patch.object(y, "AppKit", new=None)
    @patch.object(y.subprocess, "Popen")
    def test_missing_appkit_fails_before_launch(self, popen):
        with patch.object(y.typer, "echo") as echo:
            with self.assertRaises(y.typer.Exit) as error:
                y._launch_window_number_overlay(9)
        self.assertEqual(error.exception.exit_code, 1)
        echo.assert_called_once_with("Window badges require macOS and PyObjC", err=True)
        popen.assert_not_called()

    def test_numbers_windows_clockwise_from_top_left(self):
        windows = [
            window(3, 100, 100),  # bottom-right
            window(1, 0, 0),  # top-left
            window(4, 100, 0),  # top-right
            window(2, 0, 100),  # bottom-left
        ]

        ordered = y._clockwise_visible_windows(windows)

        self.assertEqual([item.id for item in ordered], [1, 4, 3, 2])

    def test_ignores_windows_that_do_not_have_a_visible_badge(self):
        windows = [
            window(1, 0, 0),
            window(2, 0, 100, visible=False),
            window(3, 100, 100, minimized=True),
            window(4, 100, 0, hidden=True),
            window(5, 200, 0, space=2),
            window(6, 200, 100, subrole="AXDialog"),
        ]

        entries = y._numbered_window_entries(windows, visible_spaces={1})

        self.assertEqual(
            entries,
            [
                {
                    "number": 1,
                    "id": 1,
                    "frame": {"x": 0, "y": 0, "w": 100, "h": 100},
                }
            ],
        )

    def test_single_row_starts_at_the_left(self):
        windows = [window(3, 200, 0), window(1, 0, 0), window(2, 100, 0)]

        ordered = y._clockwise_visible_windows(windows)

        self.assertEqual([item.id for item in ordered], [1, 2, 3])

    def test_visible_sticky_windows_bypass_assigned_space_filter(self):
        windows = [
            window(1, 0, 0),
            window(2, 100, 0, space=2, sticky=True),
            window(3, 200, 0, space=2),
            window(4, 300, 0, space=2, sticky=True, visible=False),
            window(5, 400, 0, space=2, sticky=True, minimized=True),
            window(6, 500, 0, space=2, sticky=True, hidden=True),
        ]
        entries = y._numbered_window_entries(windows, visible_spaces={1})
        self.assertEqual([entry["id"] for entry in entries], [1, 2])
        self.assertEqual([entry["number"] for entry in entries], [1, 2])

    def test_snapshot_is_private_and_expires(self):
        entries = [{"number": 1, "id": 101}]
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(y.Path, "home", return_value=Path(directory)):
                cache_path = y._window_number_cache_path()
                self.assertTrue(cache_path.is_relative_to(directory))
                self.assertIsNone(y._load_window_numbers())
                with patch.object(y.time, "time", return_value=100):
                    y._save_window_numbers(entries, seconds=5)
                    self.assertEqual(y._load_window_numbers(), entries)
                self.assertEqual(cache_path.stat().st_mode & 0o777, 0o600)
                self.assertEqual(cache_path.parent.stat().st_mode & 0o777, 0o700)
                with patch.object(y.time, "time", return_value=111):
                    self.assertIsNone(y._load_window_numbers())

    def test_snapshot_replaces_symlink_without_overwriting_target(self):
        with tempfile.TemporaryDirectory() as directory:
            cache_path = Path(directory) / "snapshot.json"
            target = Path(directory) / "important.txt"
            target.write_text("keep me")
            cache_path.symlink_to(target)
            with patch.object(y, "_window_number_cache_path", return_value=cache_path):
                y._save_window_numbers([{"number": 1, "id": 101}], seconds=5)
            self.assertEqual(target.read_text(), "keep me")
            self.assertFalse(cache_path.is_symlink())
            self.assertEqual(
                json.loads(cache_path.read_text())["windows"][0]["id"], 101
            )

    def test_failed_snapshot_write_preserves_previous_snapshot_and_cleans_up(self):
        with tempfile.TemporaryDirectory() as directory:
            cache_path = Path(directory) / "snapshot.json"
            cache_path.write_text("previous snapshot")
            with patch.object(y, "_window_number_cache_path", return_value=cache_path):
                with self.assertRaises(TypeError):
                    y._save_window_numbers([{"id": object()}], seconds=5)
            self.assertEqual(cache_path.read_text(), "previous snapshot")
            self.assertEqual(list(Path(directory).iterdir()), [cache_path])

    @patch.object(y, "call_yabai")
    @patch.object(
        y,
        "_load_window_numbers",
        return_value=[{"id": 101}, {"id": 202}, {"id": 303}],
    )
    def test_f_focuses_the_window_from_the_number_snapshot(
        self, _load_window_numbers, call_yabai
    ):
        y.f(2)

        call_yabai.assert_called_once_with("-m window --focus 202")


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v", *sys.argv[1:]]))
