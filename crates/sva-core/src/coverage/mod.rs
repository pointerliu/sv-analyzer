use anyhow::Result;

use crate::types::Timestamp;

pub trait CoverageTracker {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> Result<bool>;

    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> Result<u64>;

    fn delta_hits(&self, file: &str, line: usize, time: Timestamp) -> Result<u64>;

    fn clock_period(&self) -> Option<i64>;

    fn is_posedge_time(&self, time: i64) -> bool;

    fn is_block_elaborated(&self, file: &str, line_start: usize, line_end: usize) -> bool {
        let _ = (file, line_start, line_end);
        true
    }

    fn is_scoped_line_covered_at(
        &self,
        scope: &str,
        file: &str,
        line: usize,
        time: Timestamp,
    ) -> Result<bool> {
        let _ = scope;
        self.is_line_covered_at(file, line, time)
    }

    fn is_scope_elaborated(&self, scope: &str) -> bool {
        let _ = scope;
        true
    }
}

pub mod elaboration;
pub mod statements;
pub mod vcd;
pub use elaboration::{ElaboratedCoverageTracker, VerilatorElaborationIndex};
pub use statements::{
    assignment_statement_coverage_report, StatementCoverageEntry, StatementCoverageReport,
};
pub use vcd::VcdCoverageTracker;
