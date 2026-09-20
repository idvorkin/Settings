"""The legacy y command forwards arguments to the bundled Alfred implementation."""

import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "y_launcher", Path(__file__).with_name("y.py")
)
launcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launcher)


class LauncherTests(unittest.TestCase):
    def test_forwards_arguments_without_shell_splitting(self):
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / "workflow with spaces" / "y.py"
            script.parent.mkdir()
            script.touch()
            args = ["y", "terminal", 'ghostty:title with "quotes" and $HOME']
            with (
                patch.dict(os.environ, {"IGOR_Y_SCRIPT": str(script)}),
                patch.object(sys, "argv", args),
                patch.object(launcher.shutil, "which", return_value="/bin/uv"),
                patch.object(launcher.os, "execv") as execute,
            ):
                launcher.app()
                execute.assert_called_once_with(
                    "/bin/uv",
                    ["/bin/uv", "run", "--script", str(script.resolve()), *args[1:]],
                )

    def test_missing_workflow_or_uv_has_actionable_error(self):
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / "y.py"
            with patch.dict(os.environ, {"IGOR_Y_SCRIPT": str(script)}):
                with self.assertRaises(SystemExit) as error:
                    launcher.app()
                self.assertEqual(error.exception.code, 1)
                script.touch()
                with patch.object(launcher.shutil, "which", return_value=None):
                    with self.assertRaises(SystemExit) as error:
                        launcher.app()
                    self.assertEqual(error.exception.code, 1)

    def test_cannot_forward_to_itself(self):
        with patch.dict(os.environ, {"IGOR_Y_SCRIPT": launcher.__file__}):
            with self.assertRaises(SystemExit):
                launcher.app()


if __name__ == "__main__":
    unittest.main()
