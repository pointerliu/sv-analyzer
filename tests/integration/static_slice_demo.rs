use std::collections::HashSet;

use dac26_mcp::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use dac26_mcp::slicer::{
    InstructionExecutionPath, SliceGraph, SliceRequest, StaticBlockNode, StaticSlicer,
};
use dac26_mcp::types::{BlockId, BlockNode, SignalNode, Timestamp};

#[test]
fn instruction_execution_path_uses_shared_graph_container() {
    let path: InstructionExecutionPath = SliceGraph {
        nodes: vec![BlockNode {
            block_id: BlockId(99),
            time: Timestamp(7),
        }],
        edges: Vec::new(),
        blocks: Vec::new(),
    };

    assert_eq!(path.nodes[0].time.0, 7);
}

#[test]
fn static_slice_returns_timeless_graph_for_transitive_dependencies() {
    let block_set = BlockSet::new(vec![
        Block::new(
            BlockId(1),
            BlockType::Assign,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            10,
            10,
            vec![entry(&["tmp"], &["a", "b"])],
            "assign tmp = a & b;",
        )
        .unwrap(),
        Block::new(
            BlockId(2),
            BlockType::Always,
            CircuitType::Sequential,
            "demo",
            "design.sv",
            12,
            14,
            vec![entry(&["result"], &["tmp", "c"])],
            "always_ff @(posedge clk) result <= tmp ^ c;",
        )
        .unwrap(),
        Block::new(
            BlockId(3),
            BlockType::ModOutput,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            20,
            20,
            vec![entry(&["sink_result"], &["result"])],
            "output result;",
        )
        .unwrap(),
    ])
    .unwrap();

    let graph: SliceGraph<StaticBlockNode> = StaticSlicer::new(block_set)
        .slice(&SliceRequest {
            signal: SignalNode::named("sink_result"),
            time: Timestamp(20),
            min_time: Timestamp(-5),
        })
        .unwrap();

    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.block_id.0)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        graph
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.from.block_id.0,
                    edge.to.block_id.0,
                    edge.signal.as_ref().map(|signal| signal.name.as_str()),
                )
            })
            .collect::<Vec<_>>(),
        vec![(1, 2, Some("tmp")), (2, 3, Some("result"))]
    );
    assert_eq!(
        graph
            .blocks
            .iter()
            .map(|block| (block.id.0, block.block_type.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "Assign"), (2, "Always"), (3, "ModOutput")]
    );

    let json = serde_json::to_string(&graph).unwrap();
    assert!(
        !json.contains("\"time\""),
        "static slice graph must not contain time annotations: {json}"
    );
}

fn entry(outputs: &[&str], inputs: &[&str]) -> DataflowEntry {
    DataflowEntry {
        output: outputs
            .iter()
            .map(|output| SignalNode::named(*output))
            .collect::<Vec<_>>(),
        inputs: inputs
            .iter()
            .map(|input| SignalNode::named(*input))
            .collect::<HashSet<_>>(),
    }
}
