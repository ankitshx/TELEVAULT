//! Concrete Telegram storage reference modeling remote cloud coordinates.

use crate::error::{Result, TelegramError};
use serde::{Deserialize, Serialize};
use televault_manifest::StorageReference;

/// Structured reference identifying an uploaded chunk or file in Telegram Cloud.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramReference {
    /// Telegram chat or channel ID where the message is stored.
    pub chat_id: i64,
    /// Message ID containing the document attachment.
    pub message_id: i64,
    /// Telegram document file_id string.
    pub file_id: String,
}

impl TelegramReference {
    /// Creates and validates a new [`TelegramReference`].
    pub fn new(chat_id: i64, message_id: i64, file_id: impl Into<String>) -> Result<Self> {
        let file_id = file_id.into();
        if chat_id == 0 {
            return Err(TelegramError::InvalidReference(
                "Telegram chat_id cannot be 0".into(),
            ));
        }
        if message_id <= 0 {
            return Err(TelegramError::InvalidReference(
                "Telegram message_id must be positive".into(),
            ));
        }
        if file_id.trim().is_empty() {
            return Err(TelegramError::InvalidReference(
                "Telegram file_id cannot be empty".into(),
            ));
        }

        Ok(Self {
            chat_id,
            message_id,
            file_id,
        })
    }

    /// Converts this reference into the standard manifest [`StorageReference`].
    pub fn to_storage_reference(&self) -> StorageReference {
        StorageReference::Telegram {
            chat_id: self.chat_id,
            message_id: self.message_id,
            file_id: self.file_id.clone(),
        }
    }

    /// Attempts to parse and validate a manifest [`StorageReference`] as a [`TelegramReference`].
    pub fn from_storage_reference(reference: &StorageReference) -> Result<Self> {
        match reference {
            StorageReference::Telegram {
                chat_id,
                message_id,
                file_id,
            } => Self::new(*chat_id, *message_id, file_id.clone()),
            StorageReference::LocalStaging { .. } => Err(TelegramError::InvalidReference(
                "Cannot parse local staging reference as Telegram storage coordinate".into(),
            )),
            StorageReference::Pending => Err(TelegramError::InvalidReference(
                "Cannot parse pending reference as Telegram storage coordinate".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_reference_validation() {
        let valid = TelegramReference::new(-1001234567890, 42, "BAADBAAD...").expect("valid");
        assert_eq!(valid.chat_id, -1001234567890);
        assert_eq!(valid.message_id, 42);
        assert_eq!(valid.file_id, "BAADBAAD...");

        // Invalid chat_id
        assert!(TelegramReference::new(0, 42, "file").is_err());
        // Invalid message_id
        assert!(TelegramReference::new(-100, 0, "file").is_err());
        assert!(TelegramReference::new(-100, -1, "file").is_err());
        // Empty file_id
        assert!(TelegramReference::new(-100, 1, "   ").is_err());
    }

    #[test]
    fn test_storage_reference_conversion() {
        let ref_original = TelegramReference::new(-100999, 101, "file_doc_101").unwrap();
        let manifest_ref = ref_original.to_storage_reference();

        let parsed = TelegramReference::from_storage_reference(&manifest_ref).expect("parse");
        assert_eq!(parsed, ref_original);
    }
}
