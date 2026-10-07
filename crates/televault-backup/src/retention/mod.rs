//! Retention policy engine and snapshot pruning subsystem for TELEVAULT.
//!
//! Provides deterministic, safe, metadata-driven pruning of local snapshots
//! and version bindings while preserving remote Telegram cloud backups.

pub mod engine;
pub mod evaluator;
pub mod policy;
pub mod types;

pub use engine::RetentionEngine;
pub use evaluator::RetentionEvaluator;
pub use policy::RetentionPolicy;
pub use types::{
    RetentionAction, RetentionCandidate, RetentionDecision, RetentionEvaluation, RetentionReason,
    RetentionResult,
};
