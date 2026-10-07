//! Cryptographic primitives, key derivation, and encryption foundation for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

/// Returns the cryptographic engine version string.
pub fn crypto_version() -> &'static str {
    "0.1.0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_version() {
        assert_eq!(crypto_version(), "0.1.0");
    }
}
