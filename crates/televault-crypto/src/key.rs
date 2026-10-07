//! Secure key handling, zeroization, salts, and nonces.

use crate::error::{CryptoError, Result};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 256-bit symmetric encryption key with zeroization on drop.
///
/// Implements [`ZeroizeOnDrop`] to guarantee sensitive key material is securely
/// scrubbed from memory when dropped. Custom [`fmt::Debug`] implementation redacts
/// the key bytes to prevent accidental leakage in diagnostic logs.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey {
    bytes: [u8; 32],
}

impl SecretKey {
    /// Creates a [`SecretKey`] from raw 32 bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    /// Creates a [`SecretKey`] from a byte slice, validating exact 32-byte length.
    pub fn from_slice(slice: &[u8]) -> Result<Self> {
        if slice.len() != 32 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 32,
                actual: slice.len(),
            });
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(slice);
        Ok(Self { bytes })
    }

    /// Generates a new cryptographically secure random 256-bit key.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Returns a reference to the inner 32 key bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey([REDACTED])")
    }
}

/// Cryptographic salt for password-based key derivation (32 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Salt {
    bytes: [u8; 32],
}

impl Salt {
    /// Generates a new cryptographically secure random salt using [`OsRng`].
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Creates a salt from a 32-byte array.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    /// Creates a salt from a byte slice.
    pub fn from_slice(slice: &[u8]) -> Result<Self> {
        if slice.len() != 32 {
            return Err(CryptoError::MalformedPayload(format!(
                "invalid salt length: expected 32 bytes, got {}",
                slice.len()
            )));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(slice);
        Ok(Self { bytes })
    }

    /// Returns a reference to the salt bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

/// 96-bit (12-byte) nonce for AES-256-GCM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nonce {
    bytes: [u8; 12],
}

impl Nonce {
    /// Generates a fresh cryptographically secure random 96-bit nonce.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 12];
        OsRng.fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Creates a nonce from a 12-byte array.
    pub fn from_bytes(bytes: [u8; 12]) -> Self {
        Self { bytes }
    }

    /// Creates a nonce from a byte slice.
    pub fn from_slice(slice: &[u8]) -> Result<Self> {
        if slice.len() != 12 {
            return Err(CryptoError::InvalidNonceLength(slice.len()));
        }
        let mut bytes = [0u8; 12];
        bytes.copy_from_slice(slice);
        Ok(Self { bytes })
    }

    /// Returns a reference to the 12-byte nonce.
    pub fn as_bytes(&self) -> &[u8; 12] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_key_debug_redaction() {
        let key = SecretKey::from_bytes([42u8; 32]);
        let debug_str = format!("{key:?}");
        assert_eq!(debug_str, "SecretKey([REDACTED])");
        assert!(!debug_str.contains("42"));
    }

    #[test]
    fn test_secret_key_from_slice() {
        let valid = [7u8; 32];
        let key = SecretKey::from_slice(&valid).unwrap();
        assert_eq!(key.as_bytes(), &valid);

        let invalid = [7u8; 16];
        assert!(SecretKey::from_slice(&invalid).is_err());
    }

    #[test]
    fn test_nonce_generation_and_validation() {
        let n1 = Nonce::generate();
        let n2 = Nonce::generate();
        assert_ne!(n1.as_bytes(), n2.as_bytes());

        assert!(Nonce::from_slice(&[0u8; 12]).is_ok());
        assert!(Nonce::from_slice(&[0u8; 10]).is_err());
    }

    #[test]
    fn test_salt_generation_and_validation() {
        let s1 = Salt::generate();
        let s2 = Salt::generate();
        assert_ne!(s1.as_bytes(), s2.as_bytes());

        assert!(Salt::from_slice(&[0u8; 32]).is_ok());
        assert!(Salt::from_slice(&[0u8; 16]).is_err());
    }
}
