#!/usr/bin/env python3
"""Tests for interactive_blues_tree.py helper logic."""

from __future__ import annotations

import importlib.util
import pathlib
import unittest


SCRIPT_PATH = pathlib.Path(__file__).with_name("interactive_blues_tree.py")
SPEC = importlib.util.spec_from_file_location("interactive_blues_tree", SCRIPT_PATH)
assert SPEC is not None
assert SPEC.loader is not None
interactive_blues_tree = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(interactive_blues_tree)


def graph_fixture() -> dict:
    return {
        "target": "top.root",
        "nodes": [
            {"id": 1, "kind": "block", "block_id": 10, "time": 0},
            {"id": 2, "kind": "block", "block_id": 20, "time": 0},
            {"id": 3, "kind": "block", "block_id": 21, "time": 0},
            {"id": 4, "kind": "block", "block_id": 22, "time": 0},
        ],
        "edges": [
            {"from": 3, "to": 1, "signal": {"name": "top.fetch_addr"}},
            {"from": 2, "to": 1, "signal": {"name": "top.fetch_addr"}},
            {"from": 4, "to": 3, "signal": {"name": "top.deep"}},
            {"from": 2, "to": 4, "signal": {"name": "top.fetch_addr"}},
        ],
        "blocks": [
            {
                "id": 10,
                "scope": "top.root_i",
                "block_type": "Always",
                "line_start": 1,
                "line_end": 2,
                "code_snippet": "always_ff @(posedge clk) begin\n  root <= child;\nend",
            },
            {
                "id": 20,
                "scope": "top.icache_i",
                "block_type": "ModOutput",
                "line_start": 3,
                "line_end": 4,
                "code_snippet": "output logic [31:0] addr_o,",
            },
            {
                "id": 21,
                "scope": "top.prefetch_buffer_i",
                "block_type": "ModOutput",
                "line_start": 5,
                "line_end": 6,
                "code_snippet": "\n".join(
                    [
                        "output logic [31:0] addr_o,",
                        "assign addr_o = next_addr;",
                        "assign next_addr = base_addr;",
                        "assign base_addr = branch_addr;",
                    ]
                ),
            },
            {
                "id": 22,
                "scope": "top.fifo_i",
                "block_type": "ModOutput",
                "line_start": 7,
                "line_end": 8,
                "code_snippet": "output logic [31:0] out_addr_o,",
            },
        ],
    }


class InteractiveBluesTreeTest(unittest.TestCase):
    def test_children_are_exact_nodes_not_signal_groups(self) -> None:
        index = interactive_blues_tree.GraphIndex(graph_fixture())

        children = index.children(1)

        self.assertEqual([child.node_id for child in children], [2, 3])
        self.assertEqual([child.incoming_signal for child in children], ["top.fetch_addr"] * 2)

    def test_visible_tree_expands_collapses_and_resets_selected_node(self) -> None:
        index = interactive_blues_tree.GraphIndex(graph_fixture())
        state = interactive_blues_tree.ExplorerState(index, 1, "top.root")

        self.assertEqual(len(state.visible_rows()), 1)

        state.expand_selected()
        self.assertEqual([row.entry.node_id for row in state.visible_rows()], [1, 2, 3])

        state.move_selection(1)
        self.assertEqual(state.selected_row().entry.node_id, 2)

        state.collapse_or_parent()
        self.assertEqual(state.selected_row().entry.node_id, 1)

        state.move_selection(2)
        self.assertEqual(state.selected_row().entry.node_id, 3)
        state.expand_selected()
        self.assertEqual([row.entry.node_id for row in state.visible_rows()], [1, 2, 3, 4])

        state.reset()
        self.assertEqual([row.entry.node_id for row in state.visible_rows()], [1])
        self.assertEqual(state.selected_row().entry.node_id, 1)

    def test_rendered_rows_use_tree_connectors(self) -> None:
        index = interactive_blues_tree.GraphIndex(graph_fixture())
        state = interactive_blues_tree.ExplorerState(index, 1, "top.root")
        state.expand_selected()

        lines = [row.render(index, full_signal=False) for row in state.visible_rows()]

        self.assertTrue(lines[0].startswith("root, time=0, module=root"))
        self.assertTrue(lines[1].startswith("|- fetch_addr, time=0, module=icache"))
        self.assertTrue(lines[2].startswith("`- fetch_addr, time=0, module=prefetch_buffer"))

    def test_code_snippet_lines_are_available_for_selected_block(self) -> None:
        index = interactive_blues_tree.GraphIndex(graph_fixture())
        state = interactive_blues_tree.ExplorerState(index, 1, "top.root")
        state.expand_selected()
        state.move_selection(2)

        snippet = index.code_snippet_lines(state.selected_row().entry)

        self.assertEqual(
            snippet,
            [
                "output logic [31:0] addr_o,",
                "assign addr_o = next_addr;",
                "assign next_addr = base_addr;",
                "assign base_addr = branch_addr;",
            ],
        )

    def test_code_scroll_clamps_and_resets_when_selection_changes(self) -> None:
        index = interactive_blues_tree.GraphIndex(graph_fixture())
        state = interactive_blues_tree.ExplorerState(index, 1, "top.root")
        state.expand_selected()
        state.move_selection(2)

        state.scroll_code(10, visible_line_count=2)
        self.assertEqual(state.code_scroll, 2)

        state.scroll_code(-1, visible_line_count=2)
        self.assertEqual(state.code_scroll, 1)

        state.move_selection(-1)
        self.assertEqual(state.code_scroll, 0)

    def test_repeated_nodes_are_marked_and_not_expandable(self) -> None:
        index = interactive_blues_tree.GraphIndex(graph_fixture())
        state = interactive_blues_tree.ExplorerState(index, 1, "top.root")
        state.expand_selected()
        state.move_selection(2)
        state.expand_selected()
        state.move_selection(1)
        state.expand_selected()

        rows = state.visible_rows()
        repeated = rows[-1]

        self.assertEqual(repeated.entry.node_id, 2)
        self.assertTrue(repeated.already_shown)
        self.assertIn("[already shown]", repeated.render(index, full_signal=False))
        self.assertFalse(state.can_expand(repeated))


if __name__ == "__main__":
    unittest.main()
