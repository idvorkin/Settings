import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

import y


def window(
    window_id,
    x,
    y_position,
    *,
    visible=True,
    minimized=False,
    hidden=False,
    space=1,
    subrole="AXStandardWindow",
):
    return SimpleNamespace(
        id=window_id,
        frame=SimpleNamespace(x=x, y=y_position, w=100, h=100),
        is_visible=visible,
        is_minimized=minimized,
        is_hidden=hidden,
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
            stderr=y.subprocess.DEVNULL,
            close_fds=True,
            cwd="/",
            start_new_session=True,
        )

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
    unittest.main()
