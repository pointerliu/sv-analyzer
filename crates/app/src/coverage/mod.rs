pub mod statements;
pub mod vcd;
pub use statements::{
    assignment_statement_coverage_report, StatementCoverageEntry, StatementCoverageReport,
};
pub use vcd::VcdCoverageTracker;
