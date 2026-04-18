use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::coverage::CoverageTracker;
use crate::types::Timestamp;

#[derive(Debug, Clone, Default)]
pub struct VerilatorElaborationIndex {
    assign_like_ranges_by_file: HashMap<String, Vec<ElaboratedLineRange>>,
    always_ranges_by_file: HashMap<String, Vec<ElaboratedLineRange>>,
    instance_names: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ElaboratedLineRange {
    start: usize,
    end: usize,
}

pub struct ElaboratedCoverageTracker {
    inner: Arc<dyn CoverageTracker + Send + Sync>,
    elaboration: VerilatorElaborationIndex,
}

impl VerilatorElaborationIndex {
    pub fn from_tree_json_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_tree_json_file_with_meta(path, Option::<&Path>::None)
    }

    pub fn from_tree_json_file_with_meta(
        path: impl AsRef<Path>,
        meta_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)
            .with_context(|| format!("failed to open Verilator tree JSON {}", path.display()))?;
        let root: Value = serde_json::from_reader(file)
            .with_context(|| format!("failed to parse Verilator tree JSON {}", path.display()))?;
        let file_map = load_file_map(path, meta_path.as_ref().map(|path| path.as_ref()))?;

        let mut index = Self::default();
        index.collect_value(&root, &file_map);
        index.normalize();
        Ok(index)
    }

    pub fn is_block_elaborated(&self, file: &str, line_start: usize, line_end: usize) -> bool {
        self.is_assign_like_elaborated(file, line_start, line_end)
            || self.is_always_elaborated(file, line_start, line_end)
    }

    pub fn is_assign_like_elaborated(
        &self,
        file: &str,
        line_start: usize,
        line_end: usize,
    ) -> bool {
        self.range_intersects(&self.assign_like_ranges_by_file, file, line_start, line_end)
    }

    pub fn is_always_elaborated(&self, file: &str, line_start: usize, line_end: usize) -> bool {
        self.range_intersects(&self.always_ranges_by_file, file, line_start, line_end)
    }

    pub fn is_scope_elaborated(&self, scope: &str) -> bool {
        if self.instance_names.is_empty() {
            return true;
        }

        let scope_parts = scope.split('.').collect::<Vec<_>>();
        if scope_parts.len() <= 2 {
            return true;
        }

        self.instance_names
            .iter()
            .any(|instance| scope_ends_with_instance(&scope_parts, instance))
    }

    fn collect_value(&mut self, value: &Value, file_map: &HashMap<String, String>) {
        match value {
            Value::Object(object) => {
                self.collect_object(object, file_map);
                for child in object.values() {
                    self.collect_value(child, file_map);
                }
            }
            Value::Array(values) => {
                for child in values {
                    self.collect_value(child, file_map);
                }
            }
            _ => {}
        }
    }

    fn collect_object(
        &mut self,
        object: &serde_json::Map<String, Value>,
        file_map: &HashMap<String, String>,
    ) {
        let Some(node_type) = object.get("type").and_then(Value::as_str) else {
            return;
        };
        let Some(loc) = object.get("loc").and_then(Value::as_str) else {
            return;
        };
        let Some((file_ref, range)) = parse_verilator_loc(loc) else {
            return;
        };
        let file = file_map
            .get(&file_ref)
            .cloned()
            .unwrap_or_else(|| normalize_file_key(&file_ref));

        if node_type == "CELL" {
            if let Some(name) = object.get("name").and_then(Value::as_str) {
                insert_instance_name(&mut self.instance_names, name);
            }
            if let Some(orig_name) = object.get("origName").and_then(Value::as_str) {
                insert_instance_name(&mut self.instance_names, orig_name);
            }
        }

        if is_assign_like_node(node_type) {
            self.assign_like_ranges_by_file
                .entry(file)
                .or_default()
                .push(range);
        } else if node_type == "ALWAYS" {
            self.always_ranges_by_file
                .entry(file)
                .or_default()
                .push(range);
        }
    }

    fn normalize(&mut self) {
        normalize_ranges(&mut self.assign_like_ranges_by_file);
        normalize_ranges(&mut self.always_ranges_by_file);
    }

    fn range_intersects(
        &self,
        ranges_by_file: &HashMap<String, Vec<ElaboratedLineRange>>,
        file: &str,
        line_start: usize,
        line_end: usize,
    ) -> bool {
        let normalized = normalize_file_key(file);
        let Some(ranges) = ranges_by_file.get(&normalized) else {
            return false;
        };

        ranges
            .iter()
            .any(|range| range.start <= line_end && range.end >= line_start)
    }
}

impl ElaboratedCoverageTracker {
    pub fn new(
        inner: Arc<dyn CoverageTracker + Send + Sync>,
        elaboration: VerilatorElaborationIndex,
    ) -> Self {
        Self { inner, elaboration }
    }
}

impl CoverageTracker for ElaboratedCoverageTracker {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> Result<bool> {
        self.inner.is_line_covered_at(file, line, time)
    }

    fn is_scoped_line_covered_at(
        &self,
        scope: &str,
        file: &str,
        line: usize,
        time: Timestamp,
    ) -> Result<bool> {
        self.inner
            .is_scoped_line_covered_at(scope, file, line, time)
    }

    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> Result<u64> {
        self.inner.hit_count_at(file, line, time)
    }

    fn delta_hits(&self, file: &str, line: usize, time: Timestamp) -> Result<u64> {
        self.inner.delta_hits(file, line, time)
    }

    fn clock_period(&self) -> Option<i64> {
        self.inner.clock_period()
    }

    fn is_posedge_time(&self, time: i64) -> bool {
        self.inner.is_posedge_time(time)
    }

    fn is_block_elaborated(&self, file: &str, line_start: usize, line_end: usize) -> bool {
        self.elaboration
            .is_block_elaborated(file, line_start, line_end)
    }

    fn is_scope_elaborated(&self, scope: &str) -> bool {
        self.elaboration.is_scope_elaborated(scope)
    }
}

fn is_assign_like_node(node_type: &str) -> bool {
    matches!(node_type, "ASSIGN" | "ASSIGNW" | "ASSIGNDLY")
}

fn insert_instance_name(instance_names: &mut HashSet<String>, name: &str) {
    instance_names.insert(name.to_string());
    if let Some(leaf) = name.rsplit('.').next() {
        instance_names.insert(leaf.to_string());
    }
}

fn scope_ends_with_instance(scope_parts: &[&str], instance: &str) -> bool {
    let instance_parts = instance.split('.').collect::<Vec<_>>();
    if instance_parts.is_empty() || instance_parts.len() > scope_parts.len() {
        return false;
    }

    scope_parts
        .windows(instance_parts.len())
        .last()
        .is_some_and(|suffix| suffix == instance_parts.as_slice())
}

fn parse_verilator_loc(loc: &str) -> Option<(String, ElaboratedLineRange)> {
    let (file, rest) = loc.split_once(',')?;
    if file.starts_with('<') {
        return None;
    }

    let (start, end) = rest.split_once(',')?;
    let start_line = parse_loc_line(start)?;
    let end_line = parse_loc_line(end)?;
    if start_line == 0 || end_line == 0 {
        return None;
    }

    Some((
        file.to_string(),
        ElaboratedLineRange {
            start: start_line.min(end_line),
            end: start_line.max(end_line),
        },
    ))
}

fn load_file_map(
    tree_path: &Path,
    explicit_meta_path: Option<&Path>,
) -> Result<HashMap<String, String>> {
    let Some(meta_path) = explicit_meta_path
        .map(Path::to_path_buf)
        .or_else(|| discover_tree_meta_path(tree_path))
    else {
        return Ok(HashMap::new());
    };

    let file = File::open(&meta_path).with_context(|| {
        format!(
            "failed to open Verilator tree metadata JSON {}",
            meta_path.display()
        )
    })?;
    let root: Value = serde_json::from_reader(file).with_context(|| {
        format!(
            "failed to parse Verilator tree metadata JSON {}",
            meta_path.display()
        )
    })?;

    Ok(collect_file_map(&root))
}

fn discover_tree_meta_path(tree_path: &Path) -> Option<PathBuf> {
    let file_name = tree_path.file_name()?.to_str()?;

    if let Some(prefix) = file_name.strip_suffix(".tree.json") {
        let candidate = tree_path.with_file_name(format!("{prefix}.tree.meta.json"));
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let parent = tree_path.parent()?;
    let mut candidates = std::fs::read_dir(parent)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".tree.meta.json"))
        })
        .collect::<Vec<_>>();
    candidates.sort();

    match candidates.as_slice() {
        [single] => Some(single.clone()),
        _ => None,
    }
}

fn collect_file_map(root: &Value) -> HashMap<String, String> {
    let mut file_map = HashMap::new();

    match root.get("files") {
        Some(Value::Object(files)) => {
            for (id, file) in files {
                if let Some(path) = file_path_from_meta_entry(file) {
                    file_map.insert(id.clone(), normalize_file_key(path));
                }
            }
        }
        Some(Value::Array(files)) => {
            for file in files {
                let Some(id) = file
                    .get("id")
                    .or_else(|| file.get("file_id"))
                    .or_else(|| file.get("key"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                if let Some(path) = file_path_from_meta_entry(file) {
                    file_map.insert(id.to_string(), normalize_file_key(path));
                }
            }
        }
        _ => {}
    }

    file_map
}

fn file_path_from_meta_entry(entry: &Value) -> Option<&str> {
    match entry {
        Value::String(path) => Some(path),
        Value::Object(object) => object
            .get("realpath")
            .or_else(|| object.get("filename"))
            .or_else(|| object.get("path"))
            .and_then(Value::as_str),
        _ => None,
    }
}

fn parse_loc_line(loc_part: &str) -> Option<usize> {
    loc_part.split_once(':')?.0.parse().ok()
}

fn normalize_ranges(ranges_by_file: &mut HashMap<String, Vec<ElaboratedLineRange>>) {
    for ranges in ranges_by_file.values_mut() {
        ranges.sort_by_key(|range| (range.start, range.end));
        ranges.dedup();
    }
}

pub(crate) fn normalize_file_key(file: &str) -> String {
    let normalized = file.replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
    basename
        .strip_suffix(".sv")
        .or_else(|| basename.strip_suffix(".v"))
        .unwrap_or(basename)
        .to_string()
}
