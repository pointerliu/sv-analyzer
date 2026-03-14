pub mod dataflow;

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::ast::ParsedFile;
use crate::types::{BlockId, SignalId};

pub use dataflow::DataflowBlockizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BlockType {
    ModInput,
    ModOutput,
    Always,
    Assign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CircuitType {
    Combinational,
    Sequential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataflowEntry {
    pub output: SignalId,
    pub inputs: HashSet<SignalId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BlockSet {
    blocks: Vec<Block>,
    signal_to_drivers: HashMap<SignalId, Vec<BlockId>>,
}

impl BlockSet {
    pub fn new(blocks: Vec<Block>) -> Result<Self> {
        let mut seen_block_ids = HashSet::new();
        let mut signal_to_drivers: HashMap<SignalId, Vec<BlockId>> = HashMap::new();

        for block in &blocks {
            if !seen_block_ids.insert(block.id) {
                bail!("duplicate block id in block set");
            }

            for signal in &block.output_signals {
                signal_to_drivers
                    .entry(signal.clone())
                    .or_default()
                    .push(block.id);
            }
        }

        Ok(Self {
            blocks,
            signal_to_drivers,
        })
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn drivers_for(&self, signal: &SignalId) -> &[BlockId] {
        self.signal_to_drivers
            .get(signal)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

impl TryFrom<Vec<Block>> for BlockSet {
    type Error = anyhow::Error;

    fn try_from(blocks: Vec<Block>) -> Result<Self> {
        Self::new(blocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Block {
    id: BlockId,
    block_type: BlockType,
    circuit_type: CircuitType,
    module_scope: String,
    source_file: String,
    line_start: usize,
    line_end: usize,
    input_signals: HashSet<SignalId>,
    output_signals: HashSet<SignalId>,
    dataflow: Vec<DataflowEntry>,
    ast_snippet: String,
}

impl Block {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: BlockId,
        block_type: BlockType,
        circuit_type: CircuitType,
        module_scope: impl Into<String>,
        source_file: impl Into<String>,
        line_start: usize,
        line_end: usize,
        dataflow: Vec<DataflowEntry>,
        ast_snippet: impl Into<String>,
    ) -> Result<Self> {
        let input_signals = dataflow
            .iter()
            .flat_map(|entry| entry.inputs.iter().cloned())
            .collect();
        let output_signals = dataflow.iter().map(|entry| entry.output.clone()).collect();

        Self::with_signals(
            id,
            block_type,
            circuit_type,
            module_scope,
            source_file,
            line_start,
            line_end,
            input_signals,
            output_signals,
            dataflow,
            ast_snippet,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_signals(
        id: BlockId,
        block_type: BlockType,
        circuit_type: CircuitType,
        module_scope: impl Into<String>,
        source_file: impl Into<String>,
        line_start: usize,
        line_end: usize,
        input_signals: HashSet<SignalId>,
        output_signals: HashSet<SignalId>,
        dataflow: Vec<DataflowEntry>,
        ast_snippet: impl Into<String>,
    ) -> Result<Self> {
        if line_start > line_end {
            bail!("block line range is invalid");
        }

        let derived_input_signals: HashSet<_> = dataflow
            .iter()
            .flat_map(|entry| entry.inputs.iter().cloned())
            .collect();
        let derived_output_signals: HashSet<_> =
            dataflow.iter().map(|entry| entry.output.clone()).collect();

        if input_signals != derived_input_signals {
            bail!("block input_signals do not match dataflow inputs");
        }

        if output_signals != derived_output_signals {
            bail!("block output_signals do not match dataflow outputs");
        }

        Ok(Self {
            id,
            block_type,
            circuit_type,
            module_scope: module_scope.into(),
            source_file: source_file.into(),
            line_start,
            line_end,
            input_signals,
            output_signals,
            dataflow,
            ast_snippet: ast_snippet.into(),
        })
    }

    pub fn id(&self) -> BlockId {
        self.id
    }

    pub fn block_type(&self) -> BlockType {
        self.block_type
    }

    pub fn circuit_type(&self) -> CircuitType {
        self.circuit_type
    }

    pub fn module_scope(&self) -> &str {
        &self.module_scope
    }

    pub fn source_file(&self) -> &str {
        &self.source_file
    }

    pub fn line_start(&self) -> usize {
        self.line_start
    }

    pub fn line_end(&self) -> usize {
        self.line_end
    }

    pub fn input_signals(&self) -> &HashSet<SignalId> {
        &self.input_signals
    }

    pub fn output_signals(&self) -> &HashSet<SignalId> {
        &self.output_signals
    }

    pub fn dataflow(&self) -> &[DataflowEntry] {
        &self.dataflow
    }

    pub fn ast_snippet(&self) -> &str {
        &self.ast_snippet
    }
}

pub trait Blockizer {
    fn blockize(&self, files: &[ParsedFile]) -> Result<BlockSet>;
}
