#!/usr/bin/env python3
"""Interactively explore a Blues slice JSON upstream dependency tree."""

from __future__ import annotations

import argparse
import curses
import json
from collections import defaultdict
from pathlib import Path
from typing import Any, NamedTuple


DEFAULT_JSON = "ibex_blues_slice.json"
DEFAULT_ROOT_BLOCK_ID = 1480
DEFAULT_ROOT_TIME = 19


class ChildEntry(NamedTuple):
    node_id: int
    incoming_signal: str | None


class VisibleRow(NamedTuple):
    entry: ChildEntry
    depth: int
    parent_has_next: tuple[bool, ...]
    is_last: bool | None
    already_shown: bool
    path: tuple[tuple[Any, ...], ...]

    def render(self, index: GraphIndex, full_signal: bool) -> str:
        prefix = tree_prefix(self.parent_has_next, self.is_last)
        line, _ = index.describe(self.entry, full_signal)
        suffix = " [already shown]" if self.already_shown else ""
        return f"{prefix}{line}{suffix}"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Interactively explore a Blues slice graph as an upstream dependency tree."
    )
    parser.add_argument(
        "json_path",
        nargs="?",
        default=DEFAULT_JSON,
        help=f"slice JSON path (default: {DEFAULT_JSON})",
    )
    parser.add_argument(
        "--root-block-id",
        type=int,
        default=DEFAULT_ROOT_BLOCK_ID,
        help=f"root block id (default: {DEFAULT_ROOT_BLOCK_ID})",
    )
    parser.add_argument(
        "--root-time",
        type=int,
        default=DEFAULT_ROOT_TIME,
        help=f"root node time (default: {DEFAULT_ROOT_TIME})",
    )
    parser.add_argument(
        "--full-signal",
        action="store_true",
        help="print full hierarchical signal names instead of leaf names",
    )
    return parser.parse_args()


def signal_name(signal: dict[str, Any] | None) -> str | None:
    if not signal:
        return None
    return signal.get("name")


def display_signal(name: str | None, full_signal: bool) -> str:
    if not name:
        return "<unknown>"
    if full_signal:
        return name
    return name.rsplit(".", 1)[-1]


def display_module(scope: str) -> str:
    instance = scope.rsplit(".", 1)[-1]
    return instance.removesuffix("_i")


def tree_prefix(parent_has_next: tuple[bool, ...], is_last: bool | None) -> str:
    prefix = "".join("|  " if has_next else "   " for has_next in parent_has_next)
    if is_last is None:
        return prefix
    return f"{prefix}{'`- ' if is_last else '|- '}"


def load_graph(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as file:
        graph = json.load(file)

    for key in ("nodes", "edges", "blocks", "target"):
        if key not in graph:
            raise ValueError(f"{path} is missing required key {key!r}")
    return graph


class GraphIndex:
    def __init__(self, graph: dict[str, Any]) -> None:
        self.graph = graph
        self.nodes_by_id = {node["id"]: node for node in graph["nodes"]}
        self.blocks_by_id = {block["id"]: block for block in graph["blocks"]}
        self.edges_by_to: dict[int, list[dict[str, Any]]] = defaultdict(list)

        for edge in graph["edges"]:
            self.edges_by_to[edge["to"]].append(edge)

    def find_root_node(self, root_block_id: int, root_time: int) -> dict[str, Any]:
        for node in self.graph["nodes"]:
            if (
                node.get("kind") == "block"
                and node.get("block_id") == root_block_id
                and node.get("time") == root_time
            ):
                return node

        raise ValueError(
            f"root block {root_block_id} at time {root_time} was not found"
        )

    def children(self, node_id: int) -> list[ChildEntry]:
        return [
            ChildEntry(edge["from"], signal_name(edge.get("signal")))
            for edge in sorted(
                self.edges_by_to.get(node_id, []), key=self._incoming_sort_key
            )
        ]

    def has_children(self, entry: ChildEntry) -> bool:
        return bool(self.edges_by_to.get(entry.node_id))

    def describe(
        self, entry: ChildEntry, full_signal: bool
    ) -> tuple[str, tuple[Any, ...]]:
        node = self.nodes_by_id[entry.node_id]
        signal = entry.incoming_signal

        if node.get("kind") == "block":
            block_id = node["block_id"]
            block = self.blocks_by_id.get(block_id)
            if block is None:
                return (
                    f"{display_signal(signal, full_signal)}, time={node.get('time')}, "
                    f"bid={block_id}, block=<missing>",
                    ("block", block_id, node.get("time"), signal),
                )

            line = (
                f"{display_signal(signal, full_signal)}, time={node.get('time')}, "
                f"module={display_module(block['scope'])}, bid={block['id']}, "
                f"type={block['block_type']}, lines={block['line_start']}-{block['line_end']}"
            )
            return line, ("block", block_id, node.get("time"), signal)

        literal = node.get("signal", {}).get("name", "<literal>")
        line = (
            f"{display_signal(signal, full_signal)}, time={node.get('time')}, "
            f"literal={display_signal(literal, full_signal)}"
        )
        return line, ("literal", literal, node.get("time"), signal)

    def identity(self, entry: ChildEntry) -> tuple[Any, ...]:
        _, key = self.describe(entry, full_signal=True)
        return key

    def code_snippet_lines(self, entry: ChildEntry) -> list[str]:
        node = self.nodes_by_id[entry.node_id]
        if node.get("kind") != "block":
            return ["<literal>"]

        block = self.blocks_by_id.get(node["block_id"])
        if block is None:
            return ["<missing block metadata>"]

        snippet = block.get("code_snippet") or "<no code snippet>"
        return snippet.splitlines() or ["<no code snippet>"]

    def code_snippet_highlight_indices(self, entry: ChildEntry) -> set[int]:
        node = self.nodes_by_id[entry.node_id]
        if node.get("kind") != "block":
            return set()

        block = self.blocks_by_id.get(node["block_id"])
        if block is None:
            return set()

        signal = entry.incoming_signal
        if not signal:
            return set()

        leaf_name = signal.rsplit(".", 1)[-1]
        snippet = block.get("code_snippet") or ""
        lines = snippet.splitlines() or []

        highlighted = set()
        for i, line in enumerate(lines):
            if leaf_name in line:
                highlighted.add(i)
        return highlighted

    def _incoming_sort_key(self, edge: dict[str, Any]) -> tuple[Any, ...]:
        from_node = self.nodes_by_id[edge["from"]]
        return (
            signal_name(edge.get("signal")) or "",
            from_node.get("block_id", -1),
            from_node.get("time", -1),
            edge["from"],
        )


class ExplorerState:
    def __init__(
        self, index: GraphIndex, root_id: int, root_signal: str | None
    ) -> None:
        self.index = index
        self.root = ChildEntry(root_id, root_signal)
        self.expanded_paths: set[tuple[tuple[Any, ...], ...]] = set()
        self.selected_index = 0
        self.code_scroll = 0

    def visible_rows(self) -> list[VisibleRow]:
        rows: list[VisibleRow] = []
        seen: set[tuple[Any, ...]] = set()

        def walk(
            entry: ChildEntry,
            depth: int,
            parent_has_next: tuple[bool, ...],
            is_last: bool | None,
            path: tuple[tuple[Any, ...], ...],
        ) -> None:
            identity = self.index.identity(entry)
            already_shown = identity in seen
            row_path = path + (identity,)
            row = VisibleRow(
                entry, depth, parent_has_next, is_last, already_shown, row_path
            )
            rows.append(row)

            if already_shown:
                return

            seen.add(identity)
            if row_path not in self.expanded_paths:
                return

            child_parent_has_next = parent_has_next
            if is_last is not None:
                child_parent_has_next += (not is_last,)

            children = self.index.children(entry.node_id)
            for index, child in enumerate(children):
                walk(
                    child,
                    depth + 1,
                    child_parent_has_next,
                    index == len(children) - 1,
                    row_path,
                )

        walk(self.root, 0, (), None, ())
        if self.selected_index >= len(rows):
            self.selected_index = max(0, len(rows) - 1)
        return rows

    def selected_row(self) -> VisibleRow:
        return self.visible_rows()[self.selected_index]

    def move_selection(self, delta: int) -> None:
        rows = self.visible_rows()
        next_index = min(max(self.selected_index + delta, 0), len(rows) - 1)
        if next_index != self.selected_index:
            self.code_scroll = 0
        self.selected_index = next_index

    def can_expand(self, row: VisibleRow) -> bool:
        return (
            not row.already_shown
            and row.path not in self.expanded_paths
            and self.index.has_children(row.entry)
        )

    def expand_selected(self) -> None:
        row = self.selected_row()
        if self.can_expand(row):
            self.expanded_paths.add(row.path)

    def collapse_or_parent(self) -> None:
        row = self.selected_row()
        if row.path in self.expanded_paths:
            self.expanded_paths.remove(row.path)
            return

        if len(row.path) <= 1:
            return

        parent_path = row.path[:-1]
        for index, candidate in enumerate(self.visible_rows()):
            if candidate.path == parent_path:
                self.selected_index = index
                self.code_scroll = 0
                return

    def scroll_code(self, delta: int, visible_line_count: int) -> None:
        max_scroll = self.max_code_scroll(visible_line_count)
        self.code_scroll = min(max(self.code_scroll + delta, 0), max_scroll)

    def clamp_code_scroll(self, visible_line_count: int) -> None:
        self.code_scroll = min(
            self.code_scroll, self.max_code_scroll(visible_line_count)
        )

    def max_code_scroll(self, visible_line_count: int) -> int:
        line_count = len(self.index.code_snippet_lines(self.selected_row().entry))
        return max(0, line_count - max(1, visible_line_count))

    def reset(self) -> None:
        self.expanded_paths.clear()
        self.selected_index = 0
        self.code_scroll = 0


def run_curses(stdscr: Any, state: ExplorerState, full_signal: bool) -> None:
    curses.curs_set(0)
    stdscr.keypad(True)
    curses.use_default_colors()
    curses.init_pair(1, curses.COLOR_GREEN, -1)
    top_scroll = 0

    while True:
        rows = state.visible_rows()
        selected = state.selected_index
        height, width = stdscr.getmaxyx()
        content_height = max(1, height - 3)
        code_visible_lines = max(1, content_height - 1)
        state.clamp_code_scroll(code_visible_lines)

        if selected < top_scroll:
            top_scroll = selected
        elif selected >= top_scroll + content_height - 1:
            top_scroll = selected - content_height + 2

        draw_screen(stdscr, state, rows, top_scroll, content_height, width, full_signal)

        key = stdscr.getch()
        if key in (ord("q"), ord("Q")):
            return
        if key in (curses.KEY_UP, ord("k"), ord("K")):
            state.move_selection(-1)
        elif key in (curses.KEY_DOWN, ord("j"), ord("J")):
            state.move_selection(1)
        elif key in (curses.KEY_ENTER, 10, 13, curses.KEY_RIGHT, ord("l"), ord("L")):
            state.expand_selected()
        elif key in (curses.KEY_LEFT, curses.KEY_BACKSPACE, 8, 127, ord("h"), ord("H")):
            state.collapse_or_parent()
        elif key in (ord("r"), ord("R")):
            state.reset()
        elif key in (curses.KEY_NPAGE, ord("]")):
            state.scroll_code(code_visible_lines, code_visible_lines)
        elif key in (curses.KEY_PPAGE, ord("[")):
            state.scroll_code(-code_visible_lines, code_visible_lines)


def draw_screen(
    stdscr: Any,
    state: ExplorerState,
    rows: list[VisibleRow],
    top_scroll: int,
    content_height: int,
    width: int,
    full_signal: bool,
) -> None:
    stdscr.erase()
    selected_row = rows[state.selected_index]
    left_width = max(30, width // 2)
    left_width = min(left_width, max(1, width - 30))
    right_start = left_width + 1
    right_width = max(1, width - right_start)

    title = "Current tree (Up/Down select, Enter/Right expand, Left collapse/parent)"
    add_line(stdscr, 0, 0, title, left_width + 1, curses.A_BOLD)
    add_line(stdscr, 0, right_start, "Code context", width, curses.A_BOLD)
    draw_vertical_rule(stdscr, 0, content_height, left_width)

    for screen_row, row in enumerate(
        rows[top_scroll : top_scroll + content_height - 1], start=1
    ):
        row_index = top_scroll + screen_row - 1
        attr = (
            curses.A_REVERSE if row_index == state.selected_index else curses.A_NORMAL
        )
        add_line(
            stdscr,
            screen_row,
            0,
            row.render(state.index, full_signal),
            left_width + 1,
            attr,
        )

    snippet_lines = state.index.code_snippet_lines(selected_row.entry)
    highlight_indices = state.index.code_snippet_highlight_indices(selected_row.entry)
    code_visible_lines = max(1, content_height - 1)
    code_start = state.code_scroll
    code_end = code_start + code_visible_lines
    max_scroll = state.max_code_scroll(code_visible_lines)
    scroll_text = f"lines {code_start + 1}-{min(code_end, len(snippet_lines))}/{len(snippet_lines)}"
    if highlight_indices:
        scroll_text += f" ({len(highlight_indices)} line{'s' if len(highlight_indices) != 1 else ''} driving signal)"
    if max_scroll:
        scroll_text += " PgUp/PgDn or [/] scroll"
    add_line(
        stdscr,
        0,
        right_start + max(0, right_width - len(scroll_text) - 1),
        scroll_text,
        width,
    )
    for offset, line in enumerate(snippet_lines[code_start:code_end], start=1):
        line_index = code_start + offset - 1
        if line_index in highlight_indices:
            add_line(stdscr, offset, right_start, line, width, curses.color_pair(1))
        else:
            add_line(stdscr, offset, right_start, line, width)

    bottom_row = content_height
    add_line(stdscr, bottom_row, 0, "-" * max(1, width - 1), width)
    status = node_status(state, selected_row)
    selected_text = selected_row.render(state.index, full_signal)
    add_line(
        stdscr,
        bottom_row + 1,
        0,
        f"[{status}] {selected_text}",
        width,
    )
    help_text = "q quit | r reset | Enter/Right expand | Left/Backspace collapse or parent | PgUp/PgDn code"
    add_line(stdscr, bottom_row + 2, 0, help_text, width, curses.A_BOLD)
    stdscr.refresh()


def node_status(state: ExplorerState, row: VisibleRow) -> str:
    if row.already_shown:
        return "already shown"
    if row.path in state.expanded_paths:
        return "expanded"
    if state.index.has_children(row.entry):
        return "expandable"
    return "leaf"


def add_line(
    stdscr: Any,
    row: int,
    col: int,
    text: str,
    width: int,
    attr: int = curses.A_NORMAL,
) -> None:
    try:
        stdscr.addnstr(row, col, text, max(0, width - col - 1), attr)
    except curses.error:
        pass


def draw_vertical_rule(stdscr: Any, start_row: int, end_row: int, col: int) -> None:
    for row in range(start_row, end_row):
        try:
            stdscr.addch(row, col, "|")
        except curses.error:
            pass


def main() -> int:
    args = parse_args()
    graph = load_graph(Path(args.json_path))
    index = GraphIndex(graph)
    root = index.find_root_node(args.root_block_id, args.root_time)
    state = ExplorerState(index, root["id"], graph["target"])
    curses.wrapper(run_curses, state, args.full_signal)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
