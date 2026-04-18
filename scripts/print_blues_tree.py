#!/usr/bin/env python3
"""Print a Blues slice JSON as an upstream dependency tree."""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path
from typing import Any


DEFAULT_JSON = "ibex_blues_slice.json"
DEFAULT_ROOT_BLOCK_ID = 1480
DEFAULT_ROOT_TIME = 19
DEFAULT_DEPTH = 3


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Print a Blues slice graph as an indented upstream dependency tree."
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
        "--depth",
        type=int,
        default=DEFAULT_DEPTH,
        help=f"maximum upstream depth to print (default: {DEFAULT_DEPTH})",
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


def load_graph(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as file:
        graph = json.load(file)

    for key in ("nodes", "edges", "blocks", "target"):
        if key not in graph:
            raise ValueError(f"{path} is missing required key {key!r}")
    return graph


def find_root_node(
    nodes: list[dict[str, Any]], root_block_id: int, root_time: int
) -> dict[str, Any]:
    for node in nodes:
        if (
            node.get("kind") == "block"
            and node.get("block_id") == root_block_id
            and node.get("time") == root_time
        ):
            return node

    raise ValueError(f"root block {root_block_id} at time {root_time} was not found")


def describe_node(
    node: dict[str, Any],
    blocks_by_id: dict[int, dict[str, Any]],
    signal: str | None,
    full_signal: bool,
) -> tuple[str, tuple[Any, ...]]:
    if node.get("kind") == "block":
        block_id = node["block_id"]
        block = blocks_by_id.get(block_id)
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


def print_tree(
    graph: dict[str, Any],
    root_block_id: int,
    root_time: int,
    max_depth: int,
    full_signal: bool,
) -> None:
    if max_depth < 0:
        raise ValueError("--depth must be non-negative")

    nodes_by_id = {node["id"]: node for node in graph["nodes"]}
    blocks_by_id = {block["id"]: block for block in graph["blocks"]}
    edges_by_to: dict[int, list[dict[str, Any]]] = defaultdict(list)

    for edge in graph["edges"]:
        edges_by_to[edge["to"]].append(edge)

    root = find_root_node(graph["nodes"], root_block_id, root_time)

    def incoming_sort_key(edge: dict[str, Any]) -> tuple[Any, ...]:
        from_node = nodes_by_id[edge["from"]]
        return (
            signal_name(edge.get("signal")) or "",
            from_node.get("block_id", -1),
            from_node.get("time", -1),
            edge["from"],
        )

    def tree_prefix(parent_has_next: tuple[bool, ...], is_last: bool | None) -> str:
        prefix = "".join("|  " if has_next else "   " for has_next in parent_has_next)
        if is_last is None:
            return prefix
        return f"{prefix}{'`- ' if is_last else '|- '}"

    def walk(
        node_id: int,
        incoming_signal: str | None,
        depth: int,
        seen: set[tuple[Any, ...]],
        parent_has_next: tuple[bool, ...],
        is_last: bool | None,
    ) -> None:
        node = nodes_by_id[node_id]
        line, key = describe_node(node, blocks_by_id, incoming_signal, full_signal)
        prefix = tree_prefix(parent_has_next, is_last)

        if key in seen:
            print(f"{prefix}{line} [already shown]")
            return

        print(f"{prefix}{line}")
        seen.add(key)

        incoming_edges = sorted(edges_by_to.get(node_id, []), key=incoming_sort_key)
        child_parent_has_next = parent_has_next
        if is_last is not None:
            child_parent_has_next += (not is_last,)

        if depth >= max_depth:
            if incoming_edges:
                limit_prefix = tree_prefix(child_parent_has_next, True)
                print(f"{limit_prefix}... depth limit reached")
            return

        for index, edge in enumerate(incoming_edges):
            walk(
                edge["from"],
                signal_name(edge.get("signal")),
                depth + 1,
                seen,
                child_parent_has_next,
                index == len(incoming_edges) - 1,
            )

    walk(root["id"], graph["target"], 0, set(), (), None)


def main() -> int:
    args = parse_args()
    graph = load_graph(Path(args.json_path))
    print_tree(
        graph,
        root_block_id=args.root_block_id,
        root_time=args.root_time,
        max_depth=args.depth,
        full_signal=args.full_signal,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
