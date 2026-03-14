use std::collections::HashSet;

use dac26_mcp::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use dac26_mcp::types::{BlockId, SignalId};

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
            output: SignalId("tmp".into()),
            inputs: HashSet::from([SignalId("a".into()), SignalId("b".into())]),
        }],
        "tmp = a + b;",
    )
    .unwrap();

    assert_eq!(block.output_signals().len(), 1);
    assert!(block.output_signals().contains(&SignalId("tmp".into())));
    assert!(block.input_signals().contains(&SignalId("a".into())));
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
        HashSet::from([SignalId("a".into())]),
        HashSet::from([SignalId("other".into())]),
        vec![DataflowEntry {
            output: SignalId("tmp".into()),
            inputs: HashSet::from([SignalId("a".into())]),
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
            output: SignalId("sum".into()),
            inputs: HashSet::from([SignalId("a".into())]),
        }],
        "assign sum = a;",
    )
    .unwrap();

    let block_set = BlockSet::new(vec![driver.clone()]).unwrap();

    assert_eq!(block_set.blocks(), &[driver]);
    assert_eq!(
        block_set.drivers_for(&SignalId("sum".into())),
        &[BlockId(7)]
    );
    assert_eq!(block_set.drivers_for(&SignalId("missing".into())), &[]);
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
            output: SignalId("sum".into()),
            inputs: HashSet::from([SignalId("a".into())]),
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
            output: SignalId("sum".into()),
            inputs: HashSet::from([SignalId("b".into())]),
        }],
        "always_ff @(posedge clk) sum <= b;",
    )
    .unwrap();

    let block_set = BlockSet::new(vec![left_driver, right_driver]).unwrap();

    assert_eq!(
        block_set.drivers_for(&SignalId("sum".into())),
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
            output: SignalId("sum".into()),
            inputs: HashSet::from([SignalId("a".into())]),
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
            output: SignalId("sum".into()),
            inputs: HashSet::from([SignalId("b".into())]),
        }],
        "always_ff @(posedge clk) sum <= b;",
    )
    .unwrap();

    let result = BlockSet::try_from(vec![left_driver, right_driver]);

    assert!(result.is_err());
}
