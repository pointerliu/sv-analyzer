pub mod vcd;

use anyhow::Result;

use crate::types::Timestamp;

pub trait CoverageTracker {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> Result<bool>;

    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> Result<u64>;
}
