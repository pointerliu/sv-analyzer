use dac26_mcp::slicer::{
    InstructionExecutionPath, SliceBlock, SliceEdge, SliceGraph, StaticBlockNode, TimedSliceNode,
};
use dac26_mcp::types::{BlockId, SignalNode, Timestamp};
use serde_json::json;

#[test]
fn instruction_execution_path_serializes_as_stable_json_graph() {
    let path: InstructionExecutionPath = SliceGraph {
        nodes: vec![
            TimedSliceNode::Block {
                block_id: BlockId(17),
                time: Some(Timestamp(19)),
            },
            TimedSliceNode::Block {
                block_id: BlockId(23),
                time: Some(Timestamp(18)),
            },
        ],
        edges: vec![SliceEdge {
            from: TimedSliceNode::Block {
                block_id: BlockId(23),
                time: Some(Timestamp(18)),
            },
            to: TimedSliceNode::Block {
                block_id: BlockId(17),
                time: Some(Timestamp(19)),
            },
            signal: Some(SignalNode::named("result")),
        }],
        blocks: vec![SliceBlock {
            id: BlockId(17),
            scope: "tb.dut".into(),
            block_type: "Always".into(),
            source_file: "design.sv".into(),
            line_start: 10,
            line_end: 20,
            code_snippet: "always_ff @(posedge clk) result <= tmp;".into(),
        }],
    };

    let json = serde_json::to_value(&path).unwrap();

    assert_eq!(
        json,
        json!({
            "nodes": [
                {
                    "kind": "block",
                    "id": 0,
                    "block_id": 17,
                    "time": 19
                },
                {
                    "kind": "block",
                    "id": 1,
                    "block_id": 23,
                    "time": 18
                }
            ],
            "edges": [
                {
                    "from": 1,
                    "to": 0,
                    "signal": {
                        "kind": "variable",
                        "name": "result",
                        "locate": {
                            "offset": 0,
                            "line": 0,
                            "len": 6
                        }
                    }
                }
            ],
            "blocks": [
                {
                    "id": 17,
                    "scope": "tb.dut",
                    "block_type": "Always",
                    "source_file": "design.sv",
                    "line_start": 10,
                    "line_end": 20,
                    "code_snippet": "always_ff @(posedge clk) result <= tmp;"
                }
            ]
        })
    );
}

#[test]
fn static_slice_graph_serializes_without_time_annotations() {
    let graph: SliceGraph<StaticBlockNode> = SliceGraph {
        nodes: vec![StaticBlockNode::Block {
            block_id: BlockId(5),
            time: None,
        }],
        edges: Vec::new(),
        blocks: vec![SliceBlock {
            id: BlockId(5),
            scope: "tb.dut".into(),
            block_type: "Assign".into(),
            source_file: "design.sv".into(),
            line_start: 5,
            line_end: 5,
            code_snippet: "assign result = a & b;".into(),
        }],
    };

    let json = serde_json::to_value(&graph).unwrap();

    assert_eq!(
        json,
        json!({
            "nodes": [
                {
                    "kind": "block",
                    "id": 0,
                    "block_id": 5
                }
            ],
            "edges": [],
            "blocks": [
                {
                    "id": 5,
                    "scope": "tb.dut",
                    "block_type": "Assign",
                    "source_file": "design.sv",
                    "line_start": 5,
                    "line_end": 5,
                    "code_snippet": "assign result = a & b;"
                }
            ]
        })
    );
}

#[test]
fn stable_export_rejects_duplicate_dynamic_nodes() {
    let graph: InstructionExecutionPath = SliceGraph {
        nodes: vec![
            TimedSliceNode::Block {
                block_id: BlockId(17),
                time: Some(Timestamp(19)),
            },
            TimedSliceNode::Block {
                block_id: BlockId(17),
                time: Some(Timestamp(19)),
            },
        ],
        edges: Vec::new(),
        blocks: Vec::new(),
    };

    let error = graph.stable_json_graph().unwrap_err();

    assert!(
        error
            .to_string()
            .contains("duplicate slice node for block_id=17 time=19"),
        "unexpected error: {error}"
    );
}

#[test]
fn stable_export_rejects_duplicate_static_nodes() {
    let graph: SliceGraph<StaticBlockNode> = SliceGraph {
        nodes: vec![
            StaticBlockNode::Block {
                block_id: BlockId(5),
                time: None,
            },
            StaticBlockNode::Block {
                block_id: BlockId(5),
                time: None,
            },
        ],
        edges: Vec::new(),
        blocks: Vec::new(),
    };

    let error = graph.stable_json_graph().unwrap_err();

    assert!(
        error
            .to_string()
            .contains("duplicate slice node for block_id=5"),
        "unexpected error: {error}"
    );
}
