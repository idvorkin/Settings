"""Exercise numbered CLI actions without focusing or closing real windows."""

from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from typer.testing import CliRunner

import y


class NumberActionTests(unittest.TestCase):
    def setUp(self):
        self.runner = CliRunner()
        self.entries = [{"number": 3, "id": 303}, {"number": 1, "id": 101}]

    def test_default_focus_and_explicit_actions_use_displayed_numbers(self):
        with (
            patch.object(y, "_load_window_numbers", return_value=self.entries),
            patch.object(y, "call_yabai") as call,
        ):
            for args, expected in (
                (["3"], "-m window --focus 303"),
                (["1"], "-m window --focus 101"),
                (["3", "focus"], "-m window --focus 303"),
                (["1", "close"], "-m window 101 --close"),
            ):
                result = self.runner.invoke(y.app, args)
                self.assertEqual(result.exit_code, 0, result.output)
                call.assert_called_once_with(expected)
                call.reset_mock()

    def test_missing_or_expired_snapshot_never_recomputes_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(
                    y,
                    "_window_number_cache_path",
                    return_value=Path(directory) / "numbers.json",
                ),
                patch.object(y, "get_windows") as query,
                patch.object(y, "call_yabai") as call,
            ):
                for expired in (False, True):
                    if expired:
                        with patch.object(y.time, "time", return_value=0):
                            y._save_window_numbers(self.entries, seconds=5)
                    for suffix in ([], ["focus"], ["close"]):
                        result = self.runner.invoke(y.app, ["3", *suffix])
                        self.assertEqual(result.exit_code, 1, result.output)
                        self.assertIn("Run `y number` again", result.output)
                query.assert_not_called()
                call.assert_not_called()

    def test_bad_number_action_and_extra_arguments_do_not_touch_windows(self):
        with (
            patch.object(y, "_load_window_numbers", return_value=self.entries),
            patch.object(y, "call_yabai") as call,
        ):
            for args in (
                ["0", "close"],
                ["0"],
                ["2"],
                ["2", "focus"],
                ["-1", "close"],
                ["3", "delete"],
                ["3", "close", "extra"],
            ):
                result = self.runner.invoke(y.app, args)
                self.assertNotEqual(result.exit_code, 0, (args, result.output))
            call.assert_not_called()

    def test_closed_window_failure_does_not_fall_back_to_focused_window(self):
        with (
            patch.object(y, "_load_window_numbers", return_value=self.entries),
            patch.object(
                y, "call_yabai", side_effect=subprocess.CalledProcessError(1, "yabai")
            ) as call,
        ):
            result = self.runner.invoke(y.app, ["3", "close"])
            self.assertNotEqual(result.exit_code, 0)
            call.assert_called_once_with("-m window 303 --close")

    def test_existing_focus_and_close_commands_still_work(self):
        with patch.object(y, "call_yabai") as call:
            for args, expected in (
                (["focus", "left"], "-m window --focus west"),
                (["close"], "-m window --close"),
            ):
                result = self.runner.invoke(y.app, args)
                self.assertEqual(result.exit_code, 0, result.output)
                call.assert_called_once_with(expected)
                call.reset_mock()

    def test_number_help_lists_supported_actions(self):
        with patch.object(y, "call_yabai") as call:
            result = self.runner.invoke(y.app, ["3", "--help"])
            call.assert_not_called()
        self.assertEqual(result.exit_code, 0, result.output)
        self.assertIn("focus", result.output)
        self.assertIn("close", result.output)


if __name__ == "__main__":
    unittest.main()
