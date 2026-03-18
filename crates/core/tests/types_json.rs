use dac26_core::types::{
    BlockEdgeJson, BlockId, BlockJson, SignalNode, TimedSliceNode, Timestamp, TraceGraphJson,
};
use serde_json::json;

#[test]
fn block_node_and_graph_dtos_serialize_as_expected() {
    let graph = TraceGraphJson {
        nodes: vec![TimedSliceNode::Block {
            block_id: BlockId(7),
            time: Some(Timestamp(19)),
        }],
        edges: vec![BlockEdgeJson {
            from: TimedSliceNode::Block {
                block_id: BlockId(7),
                time: Some(Timestamp(19)),
            },
            to: TimedSliceNode::Literal {
                signal: SignalNode::literal("8'h0"),
                time: Some(Timestamp(18)),
            },
            signal: None,
        }],
        blocks: vec![BlockJson {
            id: BlockId(7),
            scope: "tb.dut".into(),
            block_type: "Always".into(),
            source_file: "design.sv".into(),
            line_start: 12,
            line_end: 14,
            code_snippet: "always_ff @(posedge clk) q <= d;".into(),
        }],
    };

    let json = serde_json::to_value(&graph).unwrap();

    assert_eq!(
        json,
        json!({
            "nodes": [
                {
                    "kind": "block",
                    "block_id": 7,
                    "time": 19
                }
            ],
            "edges": [
                {
                    "from": {
                        "kind": "block",
                        "block_id": 7,
                        "time": 19
                    },
                    "to": {
                        "kind": "literal",
                        "signal": {
                            "kind": "literal",
                            "name": "8'h0",
                            "locate": {
                                "offset": 0,
                                "line": 0,
                                "len": 4
                            }
                        },
                        "time": 18
                    }
                }
            ],
            "blocks": [
                {
                    "id": 7,
                    "scope": "tb.dut",
                    "block_type": "Always",
                    "source_file": "design.sv",
                    "line_start": 12,
                    "line_end": 14,
                    "code_snippet": "always_ff @(posedge clk) q <= d;"
                }
            ]
        })
    );
}
