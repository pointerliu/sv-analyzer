#!/usr/bin/env python3
"""Regression tests for print_blues_tree.py."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import pathlib
import unittest


SCRIPT_PATH = pathlib.Path(__file__).with_name("print_blues_tree.py")
SPEC = importlib.util.spec_from_file_location("print_blues_tree", SCRIPT_PATH)
assert SPEC is not None
assert SPEC.loader is not None
print_blues_tree = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(print_blues_tree)


class PrintBluesTreeTest(unittest.TestCase):
    def test_same_block_reached_by_different_signals_is_printed_per_signal(self) -> None:
        graph = {
            "target": "top.root",
            "nodes": [
                {"id": 1, "kind": "block", "block_id": 10, "time": 0},
                {"id": 2, "kind": "block", "block_id": 20, "time": 0},
            ],
            "edges": [
                {"from": 2, "to": 1, "signal": {"name": "top.a"}},
                {"from": 2, "to": 1, "signal": {"name": "top.b"}},
            ],
            "blocks": [
                {
                    "id": 10,
                    "scope": "top.root_i",
                    "block_type": "Always",
                    "line_start": 1,
                    "line_end": 2,
                },
                {
                    "id": 20,
                    "scope": "top.child_i",
                    "block_type": "Assign",
                    "line_start": 3,
                    "line_end": 4,
                },
            ],
        }
        output = io.StringIO()

        with contextlib.redirect_stdout(output):
            print_blues_tree.print_tree(
                graph,
                root_block_id=10,
                root_time=0,
                max_depth=1,
                full_signal=False,
            )

        self.assertIn("|- a, time=0, module=child", output.getvalue())
        self.assertIn("`- b, time=0, module=child", output.getvalue())
        self.assertNotIn("[already shown]", output.getvalue())


if __name__ == "__main__":
    unittest.main()
