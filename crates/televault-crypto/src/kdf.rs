//! Argon2id key derivation according to strict TELEVAULT specifications.

use crate::error::{CryptoError, Result};
use crate::key::{Salt, SecretKey};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};

/// Standard parameters for Argon2id key derivation in TELEVAULT:
/// - Memory: 64 MiB (65536 KiB)
/// - Iterations: 3
/// - Parallelism: 2
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KiB (default: 65536 = 64 MiB).
    pub memory_kib: u32,
    /// Time cost / iterations (default: 3).
    pub iterations: u32,
    /// Parallelism threads (default: 2).
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 2,
        }
    }
}

impl KdfParams {
    /// Validates parameter ranges.
    pub fn validate(&self) -> Result<()> {
        if self.memory_kib < 1024 {
            return Err(CryptoError::KeyDerivation(
                "memory cost must be at least 1024 KiB".into(),
            ));
        }
        if self.iterations < 1 {
            return Err(CryptoError::KeyDerivation(
                "iterations must be at least 1".into(),
            ));
        }
        if self.parallelism < 1 {
            return Err(CryptoError::KeyDerivation(
                "parallelism must be at least 1".into(),
            ));
        }
        Ok(())
    }
}

/// Derives a 256-bit [`SecretKey`] from a password and salt using Argon2id.
pub fn derive_key(password: &str, salt: &Salt, params: &KdfParams) -> Result<SecretKey> {
    params.validate()?;

    let argon2_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(32),
    )
    .map_err(|e| CryptoError::KeyDerivation(format!("invalid argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut key_bytes = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt.as_bytes(), &mut key_bytes)
        .map_err(|e| CryptoError::KeyDerivation(format!("derivation failed: {e}")))?;

    Ok(SecretKey::from_bytes(key_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_key_derivation() {
        // Fast test params for unit tests (1024 KiB, 1 iteration)
        let fast_params = KdfParams {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        };
        let salt = Salt::from_bytes([9u8; 32]);
        let pass = "televault-super-secure-pass";

        let k1 = derive_key(pass, &salt, &fast_params).unwrap();
        let k2 = derive_key(pass, &salt, &fast_params).unwrap();
        assert_eq!(k1.as_bytes(), k2.as_bytes());

        // Different password gives different key
        let k3 = derive_key("different-password", &salt, &fast_params).unwrap();
        assert_ne!(k1.as_bytes(), k3.as_bytes());

        // Different salt gives different key
        let diff_salt = Salt::from_bytes([8u8; 32]);
        let k4 = derive_key(pass, &diff_salt, &fast_params).unwrap();
        assert_ne!(k1.as_bytes(), k4.as_bytes());
    }

    #[test]
    fn test_invalid_kdf_params() {
        let invalid_mem = KdfParams {
            memory_kib: 512,
            iterations: 1,
            parallelism: 1,
        };
        assert!(invalid_mem.validate().is_err());

        let invalid_iter = KdfParams {
            memory_kib: 1024,
            iterations: 0,
            parallelism: 1,
        };
        assert!(invalid_iter.validate().is_err());
    }
}
