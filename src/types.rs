use std::hash::{Hash, Hasher};

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalLocate {
    pub offset: usize,
    pub line: usize,
    pub len: usize,
}

impl SignalLocate {
    pub fn unknown(len: usize) -> Self {
        Self {
            offset: 0,
            line: 0,
            len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalNodeKind {
    Variable,
    Literal,
}

impl SignalNodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Variable => "variable",
            Self::Literal => "literal",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalNode {
    pub kind: SignalNodeKind,
    pub name: String,
    pub locate: SignalLocate,
}

impl SignalNode {
    pub fn new(kind: SignalNodeKind, name: impl Into<String>, locate: SignalLocate) -> Self {
        Self {
            kind,
            name: name.into(),
            locate,
        }
    }

    pub fn variable(name: impl Into<String>, locate: SignalLocate) -> Self {
        Self::new(SignalNodeKind::Variable, name, locate)
    }

    pub fn literal_with_locate(text: impl Into<String>, locate: SignalLocate) -> Self {
        Self::new(SignalNodeKind::Literal, text, locate)
    }

    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        Self::variable(name.clone(), SignalLocate::unknown(name.len()))
    }

    pub fn literal(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::literal_with_locate(text.clone(), SignalLocate::unknown(text.len()))
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }

    pub fn is_variable(&self) -> bool {
        self.kind == SignalNodeKind::Variable
    }

    pub fn is_literal(&self) -> bool {
        self.kind == SignalNodeKind::Literal
    }
}

impl PartialEq for SignalNode {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.name == other.name
    }
}

impl Eq for SignalNode {}

impl Hash for SignalNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.name.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimedSliceNode {
    Block {
        block_id: BlockId,
        #[serde(skip_serializing_if = "Option::is_none")]
        time: Option<Timestamp>,
    },
    Literal {
        signal: SignalNode,
        #[serde(skip_serializing_if = "Option::is_none")]
        time: Option<Timestamp>,
    },
}

pub type StaticSliceNode = TimedSliceNode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StableSliceNodeJson {
    Block {
        id: usize,
        block_id: BlockId,
        #[serde(skip_serializing_if = "Option::is_none")]
        time: Option<Timestamp>,
    },
    Literal {
        id: usize,
        signal: SignalNode,
        #[serde(skip_serializing_if = "Option::is_none")]
        time: Option<Timestamp>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableSliceEdgeJson {
    pub from: usize,
    pub to: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<SignalNode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableSliceGraphJson {
    pub nodes: Vec<StableSliceNodeJson>,
    pub edges: Vec<StableSliceEdgeJson>,
    pub blocks: Vec<BlockJson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEdgeJson {
    pub from: TimedSliceNode,
    pub to: TimedSliceNode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<SignalNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockJson {
    pub id: BlockId,
    pub scope: String,
    pub block_type: String,
    pub source_file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub code_snippet: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceGraphJson {
    pub nodes: Vec<TimedSliceNode>,
    pub edges: Vec<BlockEdgeJson>,
    pub blocks: Vec<BlockJson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StableSliceNodeKey {
    Block {
        block_id: BlockId,
        time: Option<Timestamp>,
    },
    Literal {
        signal: SignalNode,
        time: Option<Timestamp>,
    },
}

pub trait StableSliceNode {
    fn stable_key(&self) -> StableSliceNodeKey;
    fn stable_json(&self, id: usize) -> StableSliceNodeJson;
}

impl StableSliceNode for TimedSliceNode {
    fn stable_key(&self) -> StableSliceNodeKey {
        match self {
            Self::Block { block_id, time } => StableSliceNodeKey::Block {
                block_id: *block_id,
                time: *time,
            },
            Self::Literal { signal, time } => StableSliceNodeKey::Literal {
                signal: signal.clone(),
                time: *time,
            },
        }
    }

    fn stable_json(&self, id: usize) -> StableSliceNodeJson {
        match self {
            Self::Block { block_id, time } => StableSliceNodeJson::Block {
                id,
                block_id: *block_id,
                time: *time,
            },
            Self::Literal { signal, time } => StableSliceNodeJson::Literal {
                id,
                signal: signal.clone(),
                time: *time,
            },
        }
    }
}

pub fn serialize_signal_name_set<S>(
    signals: &std::collections::HashSet<SignalNode>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut names = signals
        .iter()
        .map(|signal| signal.name.clone())
        .collect::<Vec<_>>();
    names.sort();

    let mut seq = serializer.serialize_seq(Some(names.len()))?;
    for name in names {
        seq.serialize_element(&name)?;
    }
    seq.end()
}

pub fn serialize_signal_driver_map<S>(
    signal_to_drivers: &std::collections::HashMap<SignalNode, Vec<BlockId>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut entries = signal_to_drivers
        .iter()
        .map(|(signal, drivers)| (signal.name.clone(), drivers))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for (name, drivers) in entries {
        map.serialize_entry(&name, drivers)?;
    }
    map.end()
}
