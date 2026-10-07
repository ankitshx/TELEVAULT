//! Explicit encryption policy and optional encryption contracts.

use crate::key::SecretKey;

/// Explicit application-level encryption policy.
///
/// Encryption in TELEVAULT is strictly opt-in and must never be silently forced.
/// Callers explicitly choose whether a given file, folder, or backup profile
/// undergoes application-level encryption.
#[derive(Debug, Clone)]
pub enum EncryptionPolicy {
    /// Application-level encryption is enabled with the provided secret key.
    Enabled(SecretKey),
    /// Application-level encryption is disabled. Data flows through unencrypted.
    Disabled,
}

impl EncryptionPolicy {
    /// Returns true if application-level encryption is enabled.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }

    /// Returns a reference to the secret key if enabled, or None if disabled.
    pub fn key(&self) -> Option<&SecretKey> {
        match self {
            Self::Enabled(key) => Some(key),
            Self::Disabled => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_policy_states() {
        let disabled = EncryptionPolicy::Disabled;
        assert!(!disabled.is_enabled());
        assert!(disabled.key().is_none());

        let key = SecretKey::generate();
        let enabled = EncryptionPolicy::Enabled(key.clone());
        assert!(enabled.is_enabled());
        assert!(enabled.key().is_some());
        assert_eq!(enabled.key().unwrap().as_bytes(), key.as_bytes());
    }
}
