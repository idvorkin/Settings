"""Guard startup behavior without creating windows or taking screenshots."""

import builtins
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

import y


class StartupTests(unittest.TestCase):
    def test_normal_command_does_not_import_gui_frameworks(self):
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "import json, sys; import y; "
                "y.app(standalone_mode=False); "
                "print(json.dumps([m for m in ('Quartz', 'AppKit', 'Foundation') "
                "if m in sys.modules]))",
                "p-foo",
                "red",
                "small",
            ],
            cwd=Path(__file__).parent,
            capture_output=True,
            text=True,
            check=True,
            timeout=10,
        )
        self.assertIn("color=red, size=small", result.stdout)
        self.assertEqual(json.loads(result.stdout.splitlines()[-1]), [])

    def test_missing_gui_dependency_is_cached_and_reported_before_launch(self):
        real_import = builtins.__import__
        attempts = []

        def import_without_quartz(name, *args, **kwargs):
            if name == "Quartz":
                attempts.append(name)
                raise ImportError("unavailable in this test")
            return real_import(name, *args, **kwargs)

        with (
            patch.object(y, "AppKit", y._GUI_NOT_LOADED),
            patch.object(y, "Quartz", y._GUI_NOT_LOADED),
            patch.object(y, "CG", y._GUI_NOT_LOADED),
            patch("builtins.__import__", side_effect=import_without_quartz),
            patch.object(y.subprocess, "Popen") as popen,
            patch.object(y.typer, "echo") as echo,
        ):
            for _ in range(2):
                with self.assertRaises(y.typer.Exit) as error:
                    y._launch_window_number_overlay(5)
                self.assertEqual(error.exception.exit_code, 1)
            self.assertIsNone(y.AppKit)
            self.assertEqual(attempts, ["Quartz"])
            self.assertEqual(echo.call_count, 2)
            popen.assert_not_called()

    @unittest.skipUnless(sys.platform == "darwin", "macOS frameworks")
    def test_gui_frameworks_load_on_demand(self):
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "import y; assert y.AppKit is y._GUI_NOT_LOADED; "
                "y._load_gui_imports(); "
                "assert y.AppKit is not None; "
                "assert y.AppKit.__name__ == 'AppKit'; "
                "assert y.Quartz.__name__ == 'Quartz'; "
                "assert y.CG.__name__ == 'Quartz.CoreGraphics'; "
                "original = y.AppKit; y._load_gui_imports(); "
                "assert y.AppKit is original",
            ],
            cwd=Path(__file__).parent,
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
