//! Cryptographic hash verification and archive integrity validation for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

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
