use std::collections::HashSet;

use sva_core::types::{SignalNode, Timestamp};

use crate::graph::{ChildEntry, GraphIndex, NodeIdentity, VisibleRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveQuery {
    pub signal: SignalNode,
    pub time: Timestamp,
}

pub struct ExplorerState {
    index: GraphIndex,
    root: ChildEntry,
    expanded_paths: HashSet<Vec<NodeIdentity>>,
    selected_index: usize,
    code_scroll: usize,
}

struct WalkCursor {
    entry: ChildEntry,
    depth: usize,
    parent_has_next: Vec<bool>,
    is_last: Option<bool>,
    path: Vec<NodeIdentity>,
}

impl ExplorerState {
    pub fn new(index: GraphIndex, root_id: usize, root_signal: Option<String>) -> Self {
        Self {
            index,
            root: ChildEntry {
                node_id: root_id,
                incoming_signal: root_signal,
            },
            expanded_paths: HashSet::new(),
            selected_index: 0,
            code_scroll: 0,
        }
    }

    pub fn index(&self) -> &GraphIndex {
        &self.index
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn code_scroll(&self) -> usize {
        self.code_scroll
    }

    pub fn visible_rows(&mut self) -> Vec<VisibleRow> {
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        self.walk_visible(
            WalkCursor {
                entry: self.root.clone(),
                depth: 0,
                parent_has_next: Vec::new(),
                is_last: None,
                path: Vec::new(),
            },
            &mut seen,
            &mut rows,
        );

        if self.selected_index >= rows.len() {
            self.selected_index = rows.len().saturating_sub(1);
        }

        rows
    }

    pub fn selected_row(&mut self) -> VisibleRow {
        let rows = self.visible_rows();
        rows[self.selected_index].clone()
    }

    pub fn move_selection(&mut self, delta: isize) {
        let rows = self.visible_rows();
        let last_index = rows.len().saturating_sub(1) as isize;
        let next = (self.selected_index as isize + delta).clamp(0, last_index) as usize;
        if next != self.selected_index {
            self.code_scroll = 0;
        }
        self.selected_index = next;
    }

    pub fn can_expand(&self, row: &VisibleRow) -> bool {
        !row.already_shown
            && !self.expanded_paths.contains(&row.path)
            && self.index.has_children(&row.entry)
    }

    pub fn expand_selected(&mut self) {
        let row = self.selected_row();
        if self.can_expand(&row) {
            self.expanded_paths.insert(row.path);
        }
    }

    pub fn collapse_or_parent(&mut self) {
        let row = self.selected_row();
        if self.expanded_paths.remove(&row.path) {
            return;
        }

        if row.path.len() <= 1 {
            return;
        }

        let parent_path = &row.path[..row.path.len() - 1];
        for (index, candidate) in self.visible_rows().iter().enumerate() {
            if candidate.path == parent_path {
                self.selected_index = index;
                self.code_scroll = 0;
                return;
            }
        }
    }

    pub fn scroll_code(&mut self, delta: isize, visible_line_count: usize) {
        let max_scroll = self.max_code_scroll(visible_line_count);
        self.code_scroll =
            (self.code_scroll as isize + delta).clamp(0, max_scroll as isize) as usize;
    }

    pub fn clamp_code_scroll(&mut self, visible_line_count: usize) {
        self.code_scroll = self
            .code_scroll
            .min(self.max_code_scroll(visible_line_count));
    }

    pub fn max_code_scroll(&mut self, visible_line_count: usize) -> usize {
        let selected = self.selected_row();
        let line_count = self.index.code_snippet_lines(&selected.entry).len();
        line_count.saturating_sub(visible_line_count.max(1))
    }

    pub fn reset(&mut self) {
        self.expanded_paths.clear();
        self.selected_index = 0;
        self.code_scroll = 0;
    }

    pub fn selected_wave_query(&mut self) -> Option<WaveQuery> {
        let row = self.selected_row();
        let signal = row.entry.incoming_signal?;
        let time = self.index.node_time(row.entry.node_id)?;
        Some(WaveQuery {
            signal: SignalNode::named(signal),
            time,
        })
    }

    fn walk_visible(
        &self,
        cursor: WalkCursor,
        seen: &mut HashSet<NodeIdentity>,
        rows: &mut Vec<VisibleRow>,
    ) {
        let identity = self.index.identity(&cursor.entry);
        let already_shown = seen.contains(&identity);
        let mut row_path = cursor.path;
        row_path.push(identity.clone());

        rows.push(VisibleRow {
            entry: cursor.entry.clone(),
            depth: cursor.depth,
            parent_has_next: cursor.parent_has_next.clone(),
            is_last: cursor.is_last,
            already_shown,
            path: row_path.clone(),
        });

        if already_shown {
            return;
        }

        seen.insert(identity);
        if !self.expanded_paths.contains(&row_path) {
            return;
        }

        let mut child_parent_has_next = cursor.parent_has_next;
        if let Some(is_last) = cursor.is_last {
            child_parent_has_next.push(!is_last);
        }

        let children = self.index.children(cursor.entry.node_id);
        let child_count = children.len();
        for (index, child) in children.into_iter().enumerate() {
            self.walk_visible(
                WalkCursor {
                    entry: child,
                    depth: cursor.depth + 1,
                    parent_has_next: child_parent_has_next.clone(),
                    is_last: Some(index == child_count - 1),
                    path: row_path.clone(),
                },
                seen,
                rows,
            );
        }
    }
}
