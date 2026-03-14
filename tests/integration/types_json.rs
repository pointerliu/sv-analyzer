use dac26_mcp::types::{
    BlockEdgeJson, BlockId, BlockJson, BlockNode, SignalNode, Timestamp, TraceGraphJson,
};
use serde_json::json;

#[test]
fn block_node_and_graph_dtos_serialize_as_expected() {
    let node = BlockNode {
        block_id: BlockId(7),
        time: Timestamp(19),
    };

    let graph = TraceGraphJson {
        nodes: vec![node.clone()],
        edges: vec![BlockEdgeJson {
            from: node.clone(),
            to: BlockNode {
                block_id: BlockId(3),
                time: Timestamp(18),
            },
            signal: Some(SignalNode::named("result")),
        }],
        blocks: vec![BlockJson {
            id: BlockId(7),
            scope: "tb.dut".into(),
            block_type: "Always".into(),
        }],
    };

    let json = serde_json::to_value(&graph).unwrap();

    assert_eq!(
        json,
        json!({
            "nodes": [
                {
                    "block_id": 7,
                    "time": 19
                }
            ],
            "edges": [
                {
                    "from": {
                        "block_id": 7,
                        "time": 19
                    },
                    "to": {
                        "block_id": 3,
                        "time": 18
                    },
                    "signal": {
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
                    "id": 7,
                    "scope": "tb.dut",
                    "block_type": "Always"
                }
            ]
        })
    );
}
