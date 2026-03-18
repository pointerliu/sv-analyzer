use anyhow::Result;

use crate::types::Timestamp;

pub trait CoverageTracker {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> Result<bool>;

    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> Result<u64>;

    fn delta_hits(&self, file: &str, line: usize, time: Timestamp) -> Result<u64>;

    fn clock_period(&self) -> Option<i64>;

    fn is_posedge_time(&self, time: i64) -> bool;
}
