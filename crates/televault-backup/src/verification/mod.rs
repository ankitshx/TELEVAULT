//! Production-grade remote backup verification, integrity audit, and ownership isolation.

pub mod engine;
pub mod ownership;

pub use engine::VerificationEngine;
pub use ownership::OwnershipValidator;
pub use televault_integrity::hasher::{HashWriter, NullHashWriter, StreamHasher};
pub use televault_integrity::types::{
    RestoreImpact, VerificationFinding, VerificationIssueCode, VerificationLevel,
    VerificationOptions, VerificationResult, VerificationSeverity, VerificationStatus,
    VerificationSummary,
};
