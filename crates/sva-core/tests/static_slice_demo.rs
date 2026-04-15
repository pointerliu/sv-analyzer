use std::collections::HashSet;

use sva_core::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use sva_core::slicer::{
    InstructionExecutionPath, SliceGraph, SliceRequest, Slicer, StaticBlockNode, StaticSlicer,
};
use sva_core::types::{BlockId, SignalNode, SignalNodeKind, TimedSliceNode, Timestamp};

#[test]
fn instruction_execution_path_uses_shared_graph_container() {
    let path: InstructionExecutionPath = SliceGraph {
        target: "x".into(),
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
        Block::builder()
            .id(BlockId(1))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(10, 10)
            .dataflow(vec![entry(&["tmp"], &["a", "b"])])
            .code_snippet("assign tmp = a & b;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(2))
            .block_type(BlockType::Always)
            .circuit_type(CircuitType::Sequential)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(12, 14)
            .dataflow(vec![entry(&["result"], &["tmp", "c"])])
            .code_snippet("always_ff @(posedge clk) result <= tmp ^ c;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(3))
            .block_type(BlockType::ModOutput)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(20, 20)
            .dataflow(vec![entry(&["sink_result"], &["result"])])
            .code_snippet("output result;")
            .build()
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
    let block_set = BlockSet::new(vec![Block::builder()
        .id(BlockId(1))
        .block_type(BlockType::Assign)
        .circuit_type(CircuitType::Combinational)
        .module_scope("demo")
        .source_file("design.sv")
        .lines(10, 10)
        .dataflow(vec![entry(&["result"], &["a"])])
        .code_snippet("assign result = a;")
        .build()
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
fn static_slice_resolves_signal_with_omitted_intermediate_instance() {
    let canonical_signal = "TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o";
    let query_signal = "TOP.ibex_simple_system.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o";

    let block_set = BlockSet::new(vec![Block::builder()
        .id(BlockId(1))
        .block_type(BlockType::Assign)
        .circuit_type(CircuitType::Combinational)
        .module_scope("demo")
        .source_file("design.sv")
        .lines(10, 10)
        .dataflow(vec![entry(
            &[canonical_signal],
            &["TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_if_o"],
        )])
        .code_snippet("assign pc_id_o = pc_if_o;")
        .build()
        .unwrap()])
    .unwrap();

    let graph = StaticSlicer::new(block_set)
        .slice(&SliceRequest {
            signal: SignalNode::named(query_signal),
            time: Timestamp(0),
            min_time: Timestamp(0),
        })
        .unwrap();

    assert_eq!(graph.blocks.len(), 1);
    assert_eq!(graph.blocks[0].id.0, 1);
}

#[test]
fn static_slice_keeps_literals_as_terminal_nodes() {
    let block_set = BlockSet::new(vec![Block::builder()
        .id(BlockId(1))
        .block_type(BlockType::Always)
        .circuit_type(CircuitType::Sequential)
        .module_scope("demo")
        .source_file("design.sv")
        .lines(53, 55)
        .dataflow(vec![DataflowEntry {
            output: vec![SignalNode::named("result")],
            inputs: HashSet::from([SignalNode::named("rst_n"), SignalNode::literal("8'h0")]),
        }])
        .code_snippet("always_ff @(posedge clk or negedge rst_n) if (!rst_n) result <= 8'h0;")
        .build()
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
