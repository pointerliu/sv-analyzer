use std::collections::HashSet;

use sva_core::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use sva_core::types::{BlockId, SignalNode};

#[test]
fn block_new_derives_signal_sets_from_dataflow() {
    let block = Block::new(
        BlockId(1),
        BlockType::Assign,
        CircuitType::Combinational,
        "alu",
        "design.sv",
        60,
        62,
        vec![DataflowEntry {
            output: vec![SignalNode::named("tmp")],
            inputs: HashSet::from([SignalNode::named("a"), SignalNode::named("b")]),
        }],
        "tmp = a + b;",
    )
    .unwrap();

    assert_eq!(block.output_signals().len(), 1);
    assert!(block.output_signals().contains(&SignalNode::named("tmp")));
    assert!(block.input_signals().contains(&SignalNode::named("a")));
    assert_eq!(block.dataflow().len(), 1);
}

#[test]
fn block_new_rejects_mismatched_signal_sets() {
    let result = Block::with_signals(
        BlockId(1),
        BlockType::Assign,
        CircuitType::Combinational,
        "alu",
        "design.sv",
        60,
        62,
        HashSet::from([SignalNode::named("a")]),
        HashSet::from([SignalNode::named("other")]),
        vec![DataflowEntry {
            output: vec![SignalNode::named("tmp")],
            inputs: HashSet::from([SignalNode::named("a")]),
        }],
        "tmp = a;",
    );

    assert!(result.is_err());
}

#[test]
fn block_set_tracks_signal_drivers_via_accessor() {
    let driver = Block::new(
        BlockId(7),
        BlockType::Assign,
        CircuitType::Combinational,
        "alu",
        "design.sv",
        10,
        10,
        vec![DataflowEntry {
            output: vec![SignalNode::named("sum")],
            inputs: HashSet::from([SignalNode::named("a")]),
        }],
        "assign sum = a;",
    )
    .unwrap();

    let block_set = BlockSet::new(vec![driver.clone()]).unwrap();

    assert_eq!(block_set.blocks(), &[driver]);
    assert_eq!(
        block_set.drivers_for(&SignalNode::named("sum")),
        &[BlockId(7)]
    );
    assert_eq!(block_set.drivers_for(&SignalNode::named("missing")), &[]);
}

#[test]
fn block_set_captures_multiple_drivers_without_exposing_index_mutation() {
    let left_driver = Block::new(
        BlockId(7),
        BlockType::Assign,
        CircuitType::Combinational,
        "alu",
        "design.sv",
        10,
        10,
        vec![DataflowEntry {
            output: vec![SignalNode::named("sum")],
            inputs: HashSet::from([SignalNode::named("a")]),
        }],
        "assign sum = a;",
    )
    .unwrap();
    let right_driver = Block::new(
        BlockId(8),
        BlockType::Always,
        CircuitType::Sequential,
        "alu",
        "design.sv",
        11,
        12,
        vec![DataflowEntry {
            output: vec![SignalNode::named("sum")],
            inputs: HashSet::from([SignalNode::named("b")]),
        }],
        "always_ff @(posedge clk) sum <= b;",
    )
    .unwrap();

    let block_set = BlockSet::new(vec![left_driver, right_driver]).unwrap();

    assert_eq!(
        block_set.drivers_for(&SignalNode::named("sum")),
        &[BlockId(7), BlockId(8)]
    );
    assert_eq!(block_set.blocks().len(), 2);
}

#[test]
fn block_set_rejects_duplicate_block_ids() {
    let left_driver = Block::new(
        BlockId(7),
        BlockType::Assign,
        CircuitType::Combinational,
        "alu",
        "design.sv",
        10,
        10,
        vec![DataflowEntry {
            output: vec![SignalNode::named("sum")],
            inputs: HashSet::from([SignalNode::named("a")]),
        }],
        "assign sum = a;",
    )
    .unwrap();
    let right_driver = Block::new(
        BlockId(7),
        BlockType::Always,
        CircuitType::Sequential,
        "alu",
        "design.sv",
        11,
        12,
        vec![DataflowEntry {
            output: vec![SignalNode::named("sum")],
            inputs: HashSet::from([SignalNode::named("b")]),
        }],
        "always_ff @(posedge clk) sum <= b;",
    )
    .unwrap();

    let result = BlockSet::try_from(vec![left_driver, right_driver]);

    assert!(result.is_err());
}

#[test]
fn block_set_resolves_hierarchical_alias_with_extra_intermediate_instance() {
    let canonical_signal = "TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o";
    let queried_signal = "TOP.ibex_simple_system.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o";

    let block_set = BlockSet::new(vec![Block::new(
        BlockId(42),
        BlockType::Assign,
        CircuitType::Combinational,
        "ibex_if_stage",
        "ibex_if_stage.sv",
        100,
        100,
        vec![DataflowEntry {
            output: vec![SignalNode::named(canonical_signal)],
            inputs: HashSet::from([SignalNode::named(
                "TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_if_o",
            )]),
        }],
        "assign pc_id_o = pc_if_o;",
    )
    .unwrap()])
    .unwrap();

    let resolved = block_set
        .resolve_signal_with_driver(&SignalNode::named(queried_signal))
        .unwrap();

    assert_eq!(resolved.name, canonical_signal);
    assert_eq!(block_set.drivers_for(&resolved), &[BlockId(42)]);
}
