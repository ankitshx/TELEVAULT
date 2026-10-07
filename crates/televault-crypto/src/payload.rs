//! Versioned encrypted payload container and metadata.

use crate::error::{CryptoError, Result};
use crate::kdf::KdfParams;
use crate::key::{Nonce, Salt};
use serde::{Deserialize, Serialize};
use televault_core::models::EncryptionAlgorithm;

/// Versioned container for encrypted data and associated cryptographic metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// Format specification version (currently 1).
    pub format_version: u32,
    /// Symmetric cipher algorithm used.
    pub algorithm: EncryptionAlgorithm,
    /// Cryptographically random 96-bit nonce.
    pub nonce: Nonce,
    /// Ciphertext containing encrypted data and appended 128-bit authentication tag.
    pub ciphertext: Vec<u8>,
    /// Optional KDF parameters and salt if key was derived from a user password.
    pub kdf_metadata: Option<KdfMetadata>,
}

/// Metadata describing password key derivation parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfMetadata {
    /// Salt used during key derivation.
    pub salt: Salt,
    /// Argon2id parameters.
    pub params: KdfParams,
}

impl EncryptedPayload {
    /// Creates a new [`EncryptedPayload`].
    pub fn new(nonce: Nonce, ciphertext: Vec<u8>, kdf_metadata: Option<KdfMetadata>) -> Self {
        Self {
            format_version: 1,
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            nonce,
            ciphertext,
            kdf_metadata,
        }
    }

    /// Validates payload headers before attempting decryption.
    pub fn validate_header(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(CryptoError::UnsupportedVersion(self.format_version));
        }
        if self.ciphertext.len() < 16 {
            return Err(CryptoError::MalformedPayload(
                "ciphertext too short (must be at least 16 bytes for auth tag)".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_header_validation() {
        let valid = EncryptedPayload::new(Nonce::generate(), vec![0u8; 32], None);
        assert!(valid.validate_header().is_ok());

        // Ciphertext < 16 bytes tag
        let too_short = EncryptedPayload::new(Nonce::generate(), vec![0u8; 15], None);
        assert!(too_short.validate_header().is_err());

        // Unsupported version
        let mut bad_version = valid.clone();
        bad_version.format_version = 2;
        assert!(bad_version.validate_header().is_err());
    }

    #[test]
    fn test_payload_serialization_roundtrip() {
        let payload = EncryptedPayload::new(
            Nonce::generate(),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            Some(KdfMetadata {
                salt: Salt::generate(),
                params: KdfParams::default(),
            }),
        );

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: EncryptedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, payload);
    }
}
