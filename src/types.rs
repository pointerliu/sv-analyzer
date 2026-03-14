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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalNode {
    pub name: String,
    pub locate: SignalLocate,
}

impl SignalNode {
    pub fn new(name: impl Into<String>, locate: SignalLocate) -> Self {
        Self {
            name: name.into(),
            locate,
        }
    }

    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        Self::new(name.clone(), SignalLocate::unknown(name.len()))
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl PartialEq for SignalNode {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for SignalNode {}

impl Hash for SignalNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockNode {
    pub block_id: BlockId,
    pub time: Timestamp,
}

pub trait StableSliceNode {
    fn block_id(&self) -> BlockId;
    fn time(&self) -> Option<Timestamp>;
}

impl StableSliceNode for BlockNode {
    fn block_id(&self) -> BlockId {
        self.block_id
    }

    fn time(&self) -> Option<Timestamp> {
        Some(self.time)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableSliceNodeJson {
    pub id: usize,
    pub block_id: BlockId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Timestamp>,
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
    pub from: BlockNode,
    pub to: BlockNode,
    pub signal: Option<SignalNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockJson {
    pub id: BlockId,
    pub scope: String,
    pub block_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceGraphJson {
    pub nodes: Vec<BlockNode>,
    pub edges: Vec<BlockEdgeJson>,
    pub blocks: Vec<BlockJson>,
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
