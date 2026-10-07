//! Cryptographic primitives, key derivation, and encryption foundation for TELEVAULT.

#![deny(missing_docs)]

pub mod aad;
pub mod cipher;
pub mod error;
pub mod kdf;
pub mod key;
pub mod payload;
pub mod policy;

pub use aad::ChunkAad;
pub use cipher::{decrypt_chunk, decrypt_raw, encrypt_chunk, encrypt_raw};
pub use error::{CryptoError, Result};
pub use kdf::{derive_key, KdfParams};
pub use key::{Nonce, Salt, SecretKey};
pub use payload::{EncryptedPayload, KdfMetadata};
pub use policy::EncryptionPolicy;

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
