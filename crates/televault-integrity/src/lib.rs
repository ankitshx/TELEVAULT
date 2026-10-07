//! Cryptographic hash verification, archive integrity validation, and ownership isolation for TELEVAULT.

#![deny(missing_docs)]

pub mod error;
pub mod hasher;
pub mod types;

pub use televault_core as core;

pub use error::{IntegrityError, Result};
pub use hasher::{HashWriter, NullHashWriter, StreamHasher, STREAM_BUFFER_SIZE};
pub use types::{
    RestoreImpact, VerificationFinding, VerificationIssueCode, VerificationLevel,
    VerificationOptions, VerificationResult, VerificationSeverity, VerificationStatus,
    VerificationSummary,
};

/// Returns the integrity subsystem status string.
pub fn integrity_status() -> &'static str {
    "active"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrity_status() {
        assert_eq!(integrity_status(), "active");
    }
}
