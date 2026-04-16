pub mod dataflow;

pub use dataflow::{elaborate_block_set, DataflowBlockizer};

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use derive_builder::Builder;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

use crate::error::{FuzzyMatch, SignalNotFound};
use crate::types::{
    serialize_signal_driver_map, serialize_signal_name_set, BlockId, LineRange, SignalNode,
};

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
    pub output: Vec<SignalNode>,
    pub inputs: HashSet<SignalNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BlockSet {
    blocks: Vec<Block>,
    #[serde(serialize_with = "serialize_signal_driver_map")]
    signal_to_drivers: HashMap<SignalNode, Vec<BlockId>>,
}

impl BlockSet {
    pub fn new(blocks: Vec<Block>) -> Result<Self> {
        let mut seen_block_ids = HashSet::new();
        let mut signal_to_drivers: HashMap<SignalNode, Vec<BlockId>> = HashMap::new();

        for block in &blocks {
            if !seen_block_ids.insert(block.id) {
                bail!("duplicate block id in block set");
            }

            for signal in &block.output_signals {
                if !signal.is_variable() {
                    continue;
                }
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

    pub fn signal_to_drivers(&self) -> &HashMap<SignalNode, Vec<BlockId>> {
        &self.signal_to_drivers
    }

    pub fn signal_names(&self) -> impl Iterator<Item = &SignalNode> {
        self.signal_to_drivers.keys()
    }

    pub fn drivers_for(&self, signal: &SignalNode) -> &[BlockId] {
        self.signal_to_drivers
            .get(signal)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn validate_signal_has_driver(&self, signal: &SignalNode) -> Result<()> {
        self.resolve_signal_with_driver(signal).map(|_| ())
    }

    pub fn resolve_signal_with_driver(&self, signal: &SignalNode) -> Result<SignalNode> {
        if !self.drivers_for(signal).is_empty() {
            return Ok(signal.clone());
        }

        if let Some(resolved) = self.resolve_hierarchical_alias(signal) {
            return Ok(resolved);
        }

        let candidates: Vec<String> = self.signal_names().map(|s| s.name.clone()).collect();
        let suggestions = FuzzyMatch::find_top_n(signal.as_str(), &candidates);
        Err(SignalNotFound {
            signal: signal.as_str().to_string(),
            suggestions,
        }
        .into())
    }

    fn resolve_hierarchical_alias(&self, signal: &SignalNode) -> Option<SignalNode> {
        if signal.is_literal() || !signal.name.contains('.') {
            return None;
        }

        let query_segments = signal.name.split('.').collect::<Vec<_>>();
        let query_first = query_segments.first()?;
        let query_last = query_segments.last()?;

        let mut matches = self
            .signal_to_drivers
            .keys()
            .filter(|candidate| {
                candidate.kind == signal.kind
                    && candidate.name != signal.name
                    && candidate.name.contains('.')
            })
            .filter(|candidate| {
                let candidate_segments = candidate.name.split('.').collect::<Vec<_>>();
                if candidate_segments.len() <= query_segments.len() {
                    return false;
                }

                let Some(candidate_first) = candidate_segments.first() else {
                    return false;
                };
                let Some(candidate_last) = candidate_segments.last() else {
                    return false;
                };

                if candidate_first != query_first || candidate_last != query_last {
                    return false;
                }

                is_subsequence(&query_segments, &candidate_segments)
            })
            .cloned()
            .collect::<Vec<_>>();

        if matches.len() == 1 {
            return matches.pop();
        }

        None
    }
}

fn is_subsequence<'a>(query_segments: &[&'a str], candidate_segments: &[&'a str]) -> bool {
    let mut query_index = 0usize;

    for segment in candidate_segments {
        if query_index < query_segments.len() && query_segments[query_index] == *segment {
            query_index += 1;
            if query_index == query_segments.len() {
                return true;
            }
        }
    }

    query_index == query_segments.len()
}

impl TryFrom<Vec<Block>> for BlockSet {
    type Error = anyhow::Error;

    fn try_from(blocks: Vec<Block>) -> Result<Self> {
        Self::new(blocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(pattern = "owned", build_fn(name = "build_inner", private))]
pub struct Block {
    id: BlockId,
    block_type: BlockType,
    circuit_type: CircuitType,
    #[builder(setter(into))]
    module_scope: String,
    #[builder(setter(into))]
    source_file: String,
    #[builder(setter(custom))]
    lines: LineRange,
    #[builder(setter(custom))]
    ast_lines: LineRange,
    #[builder(setter(skip), default)]
    input_signals: HashSet<SignalNode>,
    #[builder(setter(skip), default)]
    output_signals: HashSet<SignalNode>,
    dataflow: Vec<DataflowEntry>,
    #[builder(setter(into))]
    code_snippet: String,
}

impl BlockBuilder {
    pub fn build(self) -> Result<Block> {
        let mut block = self.build_inner().map_err(|e| anyhow::anyhow!("{}", e))?;
        block.input_signals = block
            .dataflow
            .iter()
            .flat_map(|e| e.inputs.iter().cloned())
            .collect();
        block.output_signals = block
            .dataflow
            .iter()
            .flat_map(|e| e.output.iter().cloned())
            .collect();
        Ok(block)
    }

    pub fn lines(mut self, start: usize, end: usize) -> Result<Self, String> {
        let lines = LineRange::new(start, end)?;
        self.lines = Some(lines);
        if self.ast_lines.is_none() {
            self.ast_lines = Some(lines);
        }
        Ok(self)
    }

    pub fn ast_lines(mut self, start: usize, end: usize) -> Result<Self, String> {
        self.ast_lines = Some(LineRange::new(start, end)?);
        Ok(self)
    }
}

impl Block {
    pub fn builder() -> BlockBuilder {
        BlockBuilder::default()
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
        self.lines.start()
    }

    pub fn line_end(&self) -> usize {
        self.lines.end()
    }

    pub fn ast_line_start(&self) -> usize {
        self.ast_lines.start()
    }

    pub fn ast_line_end(&self) -> usize {
        self.ast_lines.end()
    }

    pub fn lines(&self) -> LineRange {
        self.lines
    }

    pub fn ast_lines(&self) -> LineRange {
        self.ast_lines
    }

    pub fn input_signals(&self) -> &HashSet<SignalNode> {
        &self.input_signals
    }

    pub fn output_signals(&self) -> &HashSet<SignalNode> {
        &self.output_signals
    }

    pub fn dataflow(&self) -> &[DataflowEntry] {
        &self.dataflow
    }

    pub fn code_snippet(&self) -> &str {
        &self.code_snippet
    }
}

impl Serialize for Block {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct SignalNames<'a>(
            #[serde(serialize_with = "serialize_signal_name_set")] &'a HashSet<SignalNode>,
        );

        let mut state = serializer.serialize_struct("Block", 13)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("block_type", &self.block_type)?;
        state.serialize_field("circuit_type", &self.circuit_type)?;
        state.serialize_field("module_scope", &self.module_scope)?;
        state.serialize_field("source_file", &self.source_file)?;
        state.serialize_field("line_start", &self.line_start())?;
        state.serialize_field("line_end", &self.line_end())?;
        state.serialize_field("ast_line_start", &self.ast_line_start())?;
        state.serialize_field("ast_line_end", &self.ast_line_end())?;
        state.serialize_field("input_signals", &SignalNames(&self.input_signals))?;
        state.serialize_field("output_signals", &SignalNames(&self.output_signals))?;
        state.serialize_field("dataflow", &self.dataflow)?;
        state.serialize_field("code_snippet", &self.code_snippet)?;
        state.end()
    }
}

pub trait Blockizer {
    fn blockize(&self, files: &[crate::ast::ParsedFile]) -> Result<BlockSet>;
}
