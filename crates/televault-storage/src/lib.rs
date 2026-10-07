//! Storage provider abstractions and local caching interfaces for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

/// Returns the storage subsystem version string.
pub fn storage_version() -> &'static str {
    "0.1.0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_version() {
        assert_eq!(storage_version(), "0.1.0");
    }
}
