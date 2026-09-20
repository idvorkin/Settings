"""Exercise completion and focus without importing GUI dependencies or moving windows."""

import ast
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch


def load_completion_functions():
    tree = ast.parse(Path(__file__).with_name("y.py").read_text())
    names = {"PARAM_COMPLETIONS", "DYNAMIC_COMPLETION_COMMANDS"}
    functions = {
        "_make_item",
        "fast_alfred_complete",
        "focus",
        "_focus_iterm_tab",
        "_get_ghostty_tabs",
    }
    nodes = []
    for node in tree.body:
        if (
            isinstance(node, ast.AnnAssign)
            and getattr(node.target, "id", None) in names
        ):
            nodes.append(node)
        elif isinstance(node, ast.Assign) and any(
            getattr(t, "id", None) in names for t in node.targets
        ):
            nodes.append(node)
        elif isinstance(node, ast.FunctionDef) and node.name in functions:
            node.decorator_list = []
            nodes.append(node)
    module = ast.Module(body=nodes, type_ignores=[])
    namespace = {"json": json, "typer": Mock(BadParameter=ValueError)}
    # Defer type annotations, so testing the pure logic needs only the stdlib.
    import __future__

    exec(
        compile(module, "y.py", "exec", flags=__future__.annotations.compiler_flag),
        namespace,
    )
    return namespace


class CompletionTests(unittest.TestCase):
    def setUp(self):
        self.code = load_completion_functions()
        self.commands = [
            ("fleft", "left"),
            ("fright", "right"),
            ("focus", "direction"),
            ("p-foo", "test"),
            ("zoom", "zoom"),
            ("terminal", "terminal tabs"),
        ]
        self.code["get_cached_commands_list"] = lambda: self.commands

    def complete(self, query):
        return json.loads(self.code["fast_alfred_complete"](query))["items"]

    def test_tab_from_f_opens_directions(self):
        focus = self.complete("f")[0]
        self.assertEqual(focus["title"], "focus")
        self.assertFalse(focus["valid"])
        directions = self.complete(focus["autocomplete"])
        self.assertEqual(
            [i["title"] for i in directions], ["right", "left", "up", "down"]
        )
        self.assertEqual(directions[0]["arg"], ["focus", "right"])

    def test_partial_direction_and_tab(self):
        item = self.complete("focus l")[0]
        self.assertEqual(item["arg"], ["focus", "left"])
        self.assertEqual(self.complete(item["autocomplete"])[0]["arg"], item["arg"])

    def test_cold_and_cached_results_agree(self):
        for query in ("", "f", "focus ", "focus r", "p-foo red "):
            cached = self.code["fast_alfred_complete"](query)
            cold = self.code["fast_alfred_complete"](query, commands=self.commands)
            self.assertEqual(cached, cold)

    def test_existing_multi_parameter_completion(self):
        red = self.complete("p-foo r")[0]
        self.assertEqual(red["autocomplete"], "p-foo red ")
        self.assertEqual(
            self.complete(red["autocomplete"])[0]["arg"], ["p-foo", "red", "small"]
        )
        self.assertEqual(self.complete("zoom")[0]["arg"], "zoom")

    def test_numbered_actions_work_without_a_command_cache(self):
        self.code["get_cached_commands_list"] = Mock(
            side_effect=AssertionError("cache queried")
        )
        for query in ("3", "3 "):
            items = self.complete(query)
            self.assertEqual([i["title"] for i in items], ["focus", "close"])
            self.assertTrue(items[0].get("valid", True))
            self.assertEqual(
                [i["arg"] for i in items], [["3", "focus"], ["3", "close"]]
            )
            self.assertEqual(items[1]["autocomplete"], "3 close")
        for query in ("3 c", "3 close", "3 close "):
            self.assertEqual(self.complete(query)[0]["arg"], ["3", "close"])
        for query in ("0", "3 invalid", "3 close extra"):
            self.assertEqual(self.complete(query), [])

    def test_focus_dispatch_and_invalid_input(self):
        call = self.code["call_yabai"] = Mock()
        for direction, target in (
            ("right", "east"),
            ("left", "west"),
            ("up", "north"),
            ("down", "south"),
        ):
            self.code["focus"](direction)
            call.assert_called_with(f"-m window --focus {target}")
        call.reset_mock()
        with self.assertRaises(ValueError):
            self.code["focus"]("wrong; echo bad")
        call.assert_not_called()

    def test_terminal_selection_preserves_target_as_one_argument(self):
        title = 'project, "quoted" \\ path $HOME `literal`'
        identifier = "ghostty:" + title
        self.code["_get_terminal_windows_for_completion"] = lambda: [
            (title, "Ghostty tab", identifier),
            ("shell", "iTerm2 tab", "iterm:2:3:1"),
        ]
        items = self.complete("terminal ")
        self.assertEqual(items[0]["arg"], ["terminal", identifier])
        self.assertEqual(items[1]["arg"], ["terminal", "iterm:2:3:1"])
        # Mirror Alfred's run-script argv forwarding without activating a tab.
        result = subprocess.run(
            [
                "/bin/zsh",
                "-c",
                'exec "$@"',
                "alfred",
                sys.executable,
                "-c",
                "import json,sys; print(json.dumps(sys.argv[1:]))",
                *items[0]["arg"],
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertEqual(json.loads(result.stdout), ["terminal", identifier])

    def test_terminal_filter_uses_whole_query_even_after_space(self):
        self.code["_get_terminal_windows_for_completion"] = lambda: [
            ("project server", "Ghostty tab", "ghostty:project server"),
            ("project client", "Ghostty tab", "ghostty:project client"),
        ]
        for query in ("terminal project serve", "terminal project serve "):
            self.assertEqual(
                [i["title"] for i in self.complete(query)], ["project server"]
            )
        self.assertFalse(self.complete("terminal missing")[0]["valid"])

    def test_iterm_rejects_invalid_coordinates_and_reports_script_failure(self):
        runner = self.code["subprocess"] = Mock()
        for target in ("1:2", "1:0:1", "1:2:bad", "1:2:1\nactivate"):
            self.assertFalse(self.code["_focus_iterm_tab"](target))
        runner.run.assert_not_called()
        runner.run.return_value.returncode = 1
        self.assertFalse(self.code["_focus_iterm_tab"]("1:2:1"))

    def test_typing_terminal_does_not_scan_until_tab_or_space(self):
        scan = self.code["_get_terminal_windows_for_completion"] = Mock(return_value=[])
        item = self.complete("terminal")[0]
        self.assertEqual(item["autocomplete"], "terminal ")
        scan.assert_not_called()
        self.complete(item["autocomplete"])
        scan.assert_called_once()

    def test_closed_ghostty_skips_accessibility_query(self):
        with patch("subprocess.run", return_value=Mock(returncode=1)) as run:
            self.assertEqual(self.code["_get_ghostty_tabs"](), [])
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.args[0], ["/usr/bin/pgrep", "-x", "Ghostty"])

    def test_terminal_name_and_legacy_alias(self):
        scan = self.code["_get_terminal_windows_for_completion"] = Mock(
            return_value=[("shell", "iTerm2 tab", "iterm:1:2:1")]
        )
        result = self.complete("t")[0]
        self.assertEqual(result["title"], "terminal")
        self.assertEqual(result["autocomplete"], "terminal ")
        self.assertFalse(result["valid"])
        scan.assert_not_called()
        self.assertEqual(self.complete("ter "), self.complete("terminal "))
        self.assertEqual(
            self.complete("terminal ")[0]["arg"], ["terminal", "iterm:1:2:1"]
        )


if __name__ == "__main__":
    unittest.main()
