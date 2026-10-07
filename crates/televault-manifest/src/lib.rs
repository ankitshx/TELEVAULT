//! Manifest schemas, serialization, and encrypted metadata contracts for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

/// Returns the manifest specification version.
pub fn manifest_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_version() {
        assert_eq!(manifest_version(), 1);
    }
}
