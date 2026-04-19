use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use sva_core::types::{
    BlockId, BlockJson, SignalNode, StableSliceEdgeJson, StableSliceGraphJson, StableSliceNodeJson,
    Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildEntry {
    pub node_id: usize,
    pub incoming_signal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeIdentity {
    Block {
        block_id: BlockId,
        time: Option<Timestamp>,
        incoming_signal: Option<String>,
    },
    Literal {
        signal: SignalNode,
        time: Option<Timestamp>,
        incoming_signal: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    pub entry: ChildEntry,
    pub depth: usize,
    pub parent_has_next: Vec<bool>,
    pub is_last: Option<bool>,
    pub already_shown: bool,
    pub path: Vec<NodeIdentity>,
}

impl VisibleRow {
    pub fn render(&self, index: &GraphIndex, full_signal: bool) -> String {
        let prefix = tree_prefix(&self.parent_has_next, self.is_last);
        let (line, _) = index.describe(&self.entry, full_signal);
        let suffix = if self.already_shown {
            " [already shown]"
        } else {
            ""
        };
        format!("{prefix}{line}{suffix}")
    }
}

pub struct GraphIndex {
    graph: StableSliceGraphJson,
    nodes_by_id: HashMap<usize, StableSliceNodeJson>,
    blocks_by_id: HashMap<BlockId, BlockJson>,
    edges_by_to: HashMap<usize, Vec<StableSliceEdgeJson>>,
}

impl GraphIndex {
    pub fn new(graph: StableSliceGraphJson) -> Self {
        let nodes_by_id = graph
            .nodes
            .iter()
            .cloned()
            .map(|node| (node_id(&node), node))
            .collect();
        let blocks_by_id = graph
            .blocks
            .iter()
            .cloned()
            .map(|block| (block.id, block))
            .collect();
        let mut edges_by_to: HashMap<usize, Vec<StableSliceEdgeJson>> = HashMap::new();

        for edge in &graph.edges {
            edges_by_to.entry(edge.to).or_default().push(edge.clone());
        }

        Self {
            graph,
            nodes_by_id,
            blocks_by_id,
            edges_by_to,
        }
    }

    pub fn target(&self) -> &str {
        &self.graph.target
    }

    pub fn find_root_node(&self, root_block_id: u64, root_time: i64) -> Result<usize> {
        for node in &self.graph.nodes {
            if let StableSliceNodeJson::Block { id, block_id, time } = node {
                if block_id.0 == root_block_id && time.map(|time| time.0) == Some(root_time) {
                    return Ok(*id);
                }
            }
        }

        bail!("root block {root_block_id} at time {root_time} was not found")
    }

    pub fn children(&self, node_id: usize) -> Vec<ChildEntry> {
        let mut edges = self.edges_by_to.get(&node_id).cloned().unwrap_or_default();
        edges.sort_by_key(|edge| self.incoming_sort_key(edge));
        edges
            .into_iter()
            .map(|edge| ChildEntry {
                node_id: edge.from,
                incoming_signal: edge.signal.map(|signal| signal.name),
            })
            .collect()
    }

    pub fn has_children(&self, entry: &ChildEntry) -> bool {
        self.edges_by_to
            .get(&entry.node_id)
            .is_some_and(|edges| !edges.is_empty())
    }

    pub fn describe(&self, entry: &ChildEntry, full_signal: bool) -> (String, NodeIdentity) {
        match self.nodes_by_id.get(&entry.node_id) {
            Some(StableSliceNodeJson::Block { block_id, time, .. }) => {
                let Some(block) = self.blocks_by_id.get(block_id) else {
                    return (
                        format!(
                            "{}, time={}, bid={}, block=<missing>",
                            display_signal(entry.incoming_signal.as_deref(), full_signal),
                            display_time(*time),
                            block_id.0
                        ),
                        NodeIdentity::Block {
                            block_id: *block_id,
                            time: *time,
                            incoming_signal: entry.incoming_signal.clone(),
                        },
                    );
                };

                (
                    format!(
                        "{}, time={}, module={}, bid={}, type={}, lines={}-{}",
                        display_signal(entry.incoming_signal.as_deref(), full_signal),
                        display_time(*time),
                        display_module(&block.scope),
                        block.id.0,
                        block.block_type,
                        block.line_start,
                        block.line_end
                    ),
                    NodeIdentity::Block {
                        block_id: *block_id,
                        time: *time,
                        incoming_signal: entry.incoming_signal.clone(),
                    },
                )
            }
            Some(StableSliceNodeJson::Literal { signal, time, .. }) => (
                format!(
                    "{}, time={}, literal={}",
                    display_signal(entry.incoming_signal.as_deref(), full_signal),
                    display_time(*time),
                    display_signal(Some(&signal.name), full_signal)
                ),
                NodeIdentity::Literal {
                    signal: signal.clone(),
                    time: *time,
                    incoming_signal: entry.incoming_signal.clone(),
                },
            ),
            None => (
                format!(
                    "{}, node=<missing {}>",
                    display_signal(entry.incoming_signal.as_deref(), full_signal),
                    entry.node_id
                ),
                NodeIdentity::Block {
                    block_id: BlockId(0),
                    time: None,
                    incoming_signal: entry.incoming_signal.clone(),
                },
            ),
        }
    }

    pub fn identity(&self, entry: &ChildEntry) -> NodeIdentity {
        let (_, identity) = self.describe(entry, true);
        identity
    }

    pub fn node_time(&self, node_id: usize) -> Option<Timestamp> {
        match self.nodes_by_id.get(&node_id) {
            Some(StableSliceNodeJson::Block { time, .. })
            | Some(StableSliceNodeJson::Literal { time, .. }) => *time,
            None => None,
        }
    }

    pub fn code_snippet_lines(&self, entry: &ChildEntry) -> Vec<String> {
        let Some(StableSliceNodeJson::Block { block_id, .. }) =
            self.nodes_by_id.get(&entry.node_id)
        else {
            return vec!["<literal>".to_string()];
        };

        let Some(block) = self.blocks_by_id.get(block_id) else {
            return vec!["<missing block metadata>".to_string()];
        };

        let snippet = if block.code_snippet.is_empty() {
            "<no code snippet>"
        } else {
            &block.code_snippet
        };
        snippet.lines().map(str::to_string).collect()
    }

    pub fn code_snippet_highlight_indices(&self, entry: &ChildEntry) -> HashSet<usize> {
        let Some(StableSliceNodeJson::Block { block_id, .. }) =
            self.nodes_by_id.get(&entry.node_id)
        else {
            return HashSet::new();
        };
        let Some(block) = self.blocks_by_id.get(block_id) else {
            return HashSet::new();
        };
        let Some(signal) = entry.incoming_signal.as_deref() else {
            return HashSet::new();
        };

        let leaf_name = signal.rsplit('.').next().unwrap_or(signal);
        block
            .code_snippet
            .lines()
            .enumerate()
            .filter_map(|(index, line)| line.contains(leaf_name).then_some(index))
            .collect()
    }

    fn incoming_sort_key(&self, edge: &StableSliceEdgeJson) -> (String, u64, i64, usize) {
        let (block_id, time) = match self.nodes_by_id.get(&edge.from) {
            Some(StableSliceNodeJson::Block { block_id, time, .. }) => {
                (block_id.0, time.map(|time| time.0).unwrap_or(-1))
            }
            Some(StableSliceNodeJson::Literal { time, .. }) => {
                (0, time.map(|time| time.0).unwrap_or(-1))
            }
            None => (0, -1),
        };
        (
            edge.signal
                .as_ref()
                .map(|signal| signal.name.clone())
                .unwrap_or_default(),
            block_id,
            time,
            edge.from,
        )
    }
}

fn node_id(node: &StableSliceNodeJson) -> usize {
    match node {
        StableSliceNodeJson::Block { id, .. } | StableSliceNodeJson::Literal { id, .. } => *id,
    }
}

fn display_time(time: Option<Timestamp>) -> String {
    time.map(|time| time.0.to_string())
        .unwrap_or_else(|| "<none>".to_string())
}

fn display_signal(name: Option<&str>, full_signal: bool) -> String {
    let Some(name) = name else {
        return "<unknown>".to_string();
    };
    if full_signal {
        name.to_string()
    } else {
        name.rsplit('.').next().unwrap_or(name).to_string()
    }
}

fn display_module(scope: &str) -> String {
    scope
        .rsplit('.')
        .next()
        .unwrap_or(scope)
        .strip_suffix("_i")
        .unwrap_or_else(|| scope.rsplit('.').next().unwrap_or(scope))
        .to_string()
}

fn tree_prefix(parent_has_next: &[bool], is_last: Option<bool>) -> String {
    let mut prefix = parent_has_next
        .iter()
        .map(|has_next| if *has_next { "|  " } else { "   " })
        .collect::<String>();
    if let Some(is_last) = is_last {
        prefix.push_str(if is_last { "`- " } else { "|- " });
    }
    prefix
}
