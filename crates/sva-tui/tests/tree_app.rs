use sva_core::types::StableSliceGraphJson;
use sva_tui::app::ExplorerState;
use sva_tui::graph::GraphIndex;

fn graph_fixture() -> StableSliceGraphJson {
    serde_json::from_value(serde_json::json!({
        "target": "top.root",
        "nodes": [
            {"id": 1, "kind": "block", "block_id": 10, "time": 0},
            {"id": 2, "kind": "block", "block_id": 20, "time": 0},
            {"id": 3, "kind": "block", "block_id": 21, "time": 0},
            {"id": 4, "kind": "block", "block_id": 22, "time": 0}
        ],
        "edges": [
            {"from": 3, "to": 1, "signal": {"kind": "variable", "name": "top.fetch_addr", "locate": {"offset": 0, "line": 0, "ast_line": 0, "len": 14}}},
            {"from": 2, "to": 1, "signal": {"kind": "variable", "name": "top.fetch_addr", "locate": {"offset": 0, "line": 0, "ast_line": 0, "len": 14}}},
            {"from": 4, "to": 3, "signal": {"kind": "variable", "name": "top.deep", "locate": {"offset": 0, "line": 0, "ast_line": 0, "len": 8}}},
            {"from": 2, "to": 4, "signal": {"kind": "variable", "name": "top.fetch_addr", "locate": {"offset": 0, "line": 0, "ast_line": 0, "len": 14}}}
        ],
        "blocks": [
            {
                "id": 10,
                "scope": "top.root_i",
                "block_type": "Always",
                "source_file": "design.sv",
                "line_start": 1,
                "line_end": 2,
                "ast_line_start": 1,
                "ast_line_end": 2,
                "code_snippet": "always_ff @(posedge clk) begin\n  root <= child;\nend"
            },
            {
                "id": 20,
                "scope": "top.icache_i",
                "block_type": "ModOutput",
                "source_file": "design.sv",
                "line_start": 3,
                "line_end": 4,
                "ast_line_start": 3,
                "ast_line_end": 4,
                "code_snippet": "output logic [31:0] addr_o,"
            },
            {
                "id": 21,
                "scope": "top.prefetch_buffer_i",
                "block_type": "ModOutput",
                "source_file": "design.sv",
                "line_start": 5,
                "line_end": 6,
                "ast_line_start": 5,
                "ast_line_end": 6,
                "code_snippet": "output logic [31:0] addr_o,\nassign addr_o = next_addr;\nassign next_addr = base_addr;\nassign base_addr = branch_addr;"
            },
            {
                "id": 22,
                "scope": "top.fifo_i",
                "block_type": "ModOutput",
                "source_file": "design.sv",
                "line_start": 7,
                "line_end": 8,
                "ast_line_start": 7,
                "ast_line_end": 8,
                "code_snippet": "output logic [31:0] out_addr_o,"
            }
        ]
    }))
    .unwrap()
}

#[test]
fn children_are_exact_nodes_not_signal_groups() {
    let index = GraphIndex::new(graph_fixture());

    let children = index.children(1);

    assert_eq!(
        children
            .iter()
            .map(|child| child.node_id)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(
        children
            .iter()
            .map(|child| child.incoming_signal.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("top.fetch_addr"), Some("top.fetch_addr")]
    );
}

#[test]
fn visible_tree_expands_collapses_and_resets_selected_node() {
    let index = GraphIndex::new(graph_fixture());
    let mut state = ExplorerState::new(index, 1, Some("top.root".to_string()));

    assert_eq!(state.visible_rows().len(), 1);

    state.expand_selected();
    assert_eq!(
        state
            .visible_rows()
            .iter()
            .map(|row| row.entry.node_id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    state.move_selection(1);
    assert_eq!(state.selected_row().entry.node_id, 2);

    state.collapse_or_parent();
    assert_eq!(state.selected_row().entry.node_id, 1);

    state.move_selection(2);
    assert_eq!(state.selected_row().entry.node_id, 3);
    state.expand_selected();
    assert_eq!(
        state
            .visible_rows()
            .iter()
            .map(|row| row.entry.node_id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );

    state.reset();
    assert_eq!(state.selected_row().entry.node_id, 1);
    assert_eq!(state.visible_rows().len(), 1);
}

#[test]
fn rendered_rows_use_tree_connectors() {
    let index = GraphIndex::new(graph_fixture());
    let mut state = ExplorerState::new(index, 1, Some("top.root".to_string()));
    state.expand_selected();

    let lines = state
        .visible_rows()
        .iter()
        .map(|row| row.render(state.index(), false))
        .collect::<Vec<_>>();

    assert!(lines[0].starts_with("root, time=0, module=root"));
    assert!(lines[1].starts_with("|- fetch_addr, time=0, module=icache"));
    assert!(lines[2].starts_with("`- fetch_addr, time=0, module=prefetch_buffer"));
}

#[test]
fn code_scroll_clamps_and_resets_when_selection_changes() {
    let index = GraphIndex::new(graph_fixture());
    let mut state = ExplorerState::new(index, 1, Some("top.root".to_string()));
    state.expand_selected();
    state.move_selection(2);

    state.scroll_code(10, 2);
    assert_eq!(state.code_scroll(), 2);

    state.scroll_code(-1, 2);
    assert_eq!(state.code_scroll(), 1);

    state.move_selection(-1);
    assert_eq!(state.code_scroll(), 0);
}

#[test]
fn repeated_nodes_are_marked_and_not_expandable() {
    let index = GraphIndex::new(graph_fixture());
    let mut state = ExplorerState::new(index, 1, Some("top.root".to_string()));
    state.expand_selected();
    state.move_selection(2);
    state.expand_selected();
    state.move_selection(1);
    state.expand_selected();

    let rows = state.visible_rows();
    let repeated = rows.last().unwrap();

    assert_eq!(repeated.entry.node_id, 2);
    assert!(repeated.already_shown);
    assert!(repeated
        .render(state.index(), false)
        .contains("[already shown]"));
    assert!(!state.can_expand(repeated));
}

#[test]
fn selected_signal_value_query_uses_left_row_signal_and_time() {
    let index = GraphIndex::new(graph_fixture());
    let mut state = ExplorerState::new(index, 1, Some("top.root".to_string()));
    state.expand_selected();
    state.move_selection(1);

    let query = state.selected_wave_query().unwrap();

    assert_eq!(query.signal.name, "top.fetch_addr");
    assert_eq!(query.time.0, 0);
}
