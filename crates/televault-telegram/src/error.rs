//! Strongly-typed errors for the Telegram storage adapter and transport boundary.

use thiserror::Error;

/// Error conditions occurring within Telegram transport, reference parsing, or API calls.
#[derive(Debug, Error)]
pub enum TelegramError {
    /// Low-level transport or connection failure.
    #[error("Telegram transport error: {0}")]
    Transport(String),

    /// Telegram API returned an error response.
    #[error("Telegram API error {error_code}: {description}")]
    Api {
        /// Telegram error code (e.g. 400, 429, 500).
        error_code: i32,
        /// Explanation provided by the Telegram server.
        description: String,
        /// Recommended retry backoff in seconds, if specified.
        retry_after_secs: Option<u64>,
    },

    /// Invalid or malformed Telegram storage reference.
    #[error("Invalid Telegram storage reference: {0}")]
    InvalidReference(String),

    /// Client exceeded Telegram rate limits (FloodWait / 429).
    #[error("Telegram rate limited, retry after {retry_after_secs}s")]
    RateLimited {
        /// Mandatory cooldown before retrying.
        retry_after_secs: u64,
    },

    /// Message not found in the designated Telegram chat/channel.
    #[error("Telegram message {message_id} not found in chat {chat_id}")]
    MessageNotFound {
        /// Target chat ID.
        chat_id: i64,
        /// Target message ID.
        message_id: i64,
    },

    /// Message exists but contains no document attachment.
    #[error("Telegram message {message_id} in chat {chat_id} does not contain a document")]
    DocumentNotFound {
        /// Target chat ID.
        chat_id: i64,
        /// Target message ID.
        message_id: i64,
    },

    /// Underlying I/O error during document streaming.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Mapped storage error from the storage abstraction layer.
    #[error("Storage error: {0}")]
    Storage(#[from] televault_storage::StorageError),
}

impl From<TelegramError> for televault_storage::StorageError {
    fn from(err: TelegramError) -> Self {
        match err {
            TelegramError::Storage(s) => s,
            TelegramError::Io(e) => televault_storage::StorageError::Io(e),
            TelegramError::InvalidReference(msg) => {
                televault_storage::StorageError::InvalidReference(msg)
            }
            TelegramError::MessageNotFound {
                chat_id,
                message_id,
            } => televault_storage::StorageError::NotFound(format!(
                "Telegram message {message_id} not found in chat {chat_id}"
            )),
            TelegramError::DocumentNotFound {
                chat_id,
                message_id,
            } => televault_storage::StorageError::NotFound(format!(
                "No document found in message {message_id} (chat {chat_id})"
            )),
            TelegramError::RateLimited { retry_after_secs } => {
                televault_storage::StorageError::UploadFailed {
                    reason: format!("Telegram rate limited (retry after {retry_after_secs}s)"),
                    retryable: true,
                }
            }
            TelegramError::Api {
                error_code,
                description,
                retry_after_secs,
            } => {
                let retryable = error_code >= 500 || error_code == 429;
                televault_storage::StorageError::UploadFailed {
                    reason: format!(
                        "Telegram API error {error_code}: {description} (retry_after={retry_after_secs:?})"
                    ),
                    retryable,
                }
            }
            TelegramError::Transport(msg) => {
                televault_storage::StorageError::ProviderUnavailable(msg)
            }
        }
    }
}

impl From<TelegramError> for televault_core::AppError {
    fn from(err: TelegramError) -> Self {
        let storage_err: televault_storage::StorageError = err.into();
        storage_err.into()
    }
}

/// Convenience result alias for Telegram storage operations.
pub type Result<T> = std::result::Result<T, TelegramError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_error_conversions() {
        let not_found = TelegramError::MessageNotFound {
            chat_id: -100123,
            message_id: 42,
        };
        let storage_err: televault_storage::StorageError = not_found.into();
        let app_err: televault_core::AppError = storage_err.into();
        assert_eq!(app_err.error_code(), "NOT_FOUND");
    }
}
