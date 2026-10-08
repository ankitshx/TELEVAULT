//! Production-grade remote backup repair, recovery, and ownership isolation.

pub mod eligibility;
pub mod engine;
pub mod pipeline;
pub mod types;

pub use eligibility::{is_issue_repairable, is_strict_barrier, RepairEligibilityChecker};
pub use engine::RepairEngine;
pub use pipeline::RepairPipeline;
pub use types::{RepairCandidate, RepairChunkResult, RepairExecutionResult, RepairPreview};
