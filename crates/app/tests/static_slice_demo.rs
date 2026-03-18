use std::collections::HashSet;

use dac26_core::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use dac26_core::slicer::{
    InstructionExecutionPath, SliceGraph, SliceRequest, Slicer, StaticBlockNode, StaticSlicer,
};
use dac26_core::types::{BlockId, SignalNode, SignalNodeKind, TimedSliceNode, Timestamp};

#[test]
fn instruction_execution_path_uses_shared_graph_container() {
    let path: InstructionExecutionPath = SliceGraph {
        nodes: vec![TimedSliceNode::Block {
            block_id: BlockId(99),
            time: Some(Timestamp(7)),
        }],
        edges: Vec::new(),
        blocks: Vec::new(),
    };

    assert!(matches!(
        path.nodes[0],
        TimedSliceNode::Block {
            block_id: BlockId(99),
            time: Some(Timestamp(7))
        }
    ));
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
            .filter_map(|node| match node {
                StaticBlockNode::Block { block_id, .. } => Some(block_id.0),
                StaticBlockNode::Literal { .. } => None,
            })
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        graph
            .edges
            .iter()
            .map(|edge| {
                (
                    match &edge.from {
                        StaticBlockNode::Block { block_id, .. } => Some(block_id.0),
                        StaticBlockNode::Literal { .. } => None,
                    },
                    match &edge.to {
                        StaticBlockNode::Block { block_id, .. } => Some(block_id.0),
                        StaticBlockNode::Literal { .. } => None,
                    },
                    edge.signal.as_ref().map(|signal| signal.name.as_str()),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (Some(1), Some(2), Some("tmp")),
            (Some(2), Some(3), Some("result"))
        ]
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

#[test]
fn static_slicer_implements_shared_slicer_trait_with_none_time_nodes() {
    let block_set = BlockSet::new(vec![Block::new(
        BlockId(1),
        BlockType::Assign,
        CircuitType::Combinational,
        "demo",
        "design.sv",
        10,
        10,
        vec![entry(&["result"], &["a"])],
        "assign result = a;",
    )
    .unwrap()])
    .unwrap();

    let slicer = StaticSlicer::new(block_set);
    let graph = Slicer::slice(
        &slicer,
        &SliceRequest {
            signal: SignalNode::named("result"),
            time: Timestamp(7),
            min_time: Timestamp(0),
        },
    )
    .unwrap();

    assert!(graph.nodes.iter().any(|node| matches!(
        node,
        TimedSliceNode::Block {
            block_id: BlockId(1),
            time: None,
        }
    )));
}

#[test]
fn static_slice_keeps_literals_as_terminal_nodes() {
    let block_set = BlockSet::new(vec![Block::new(
        BlockId(1),
        BlockType::Always,
        CircuitType::Sequential,
        "demo",
        "design.sv",
        53,
        55,
        vec![DataflowEntry {
            output: vec![SignalNode::named("result")],
            inputs: HashSet::from([SignalNode::named("rst_n"), SignalNode::literal("8'h0")]),
        }],
        "always_ff @(posedge clk or negedge rst_n) if (!rst_n) result <= 8'h0;",
    )
    .unwrap()])
    .unwrap();

    let graph = StaticSlicer::new(block_set)
        .slice(&SliceRequest {
            signal: SignalNode::named("result"),
            time: Timestamp(20),
            min_time: Timestamp(-5),
        })
        .unwrap();

    assert!(graph.nodes.iter().any(|node| match node {
        StaticBlockNode::Literal { signal, .. } => {
            signal.kind == SignalNodeKind::Literal && signal.name == "8'h0"
        }
        _ => false,
    }));
    assert!(graph.edges.iter().any(|edge| match (&edge.from, &edge.to) {
        (StaticBlockNode::Literal { signal, .. }, StaticBlockNode::Block { block_id, .. }) => {
            signal.name == "8'h0" && block_id.0 == 1 && edge.signal.is_none()
        }
        _ => false,
    }));
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
