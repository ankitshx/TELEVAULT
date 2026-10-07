//! Telegram communication, session management, and storage adapter for TELEVAULT.

#![deny(missing_docs)]

pub mod contracts;
pub mod error;
pub mod provider;
pub mod reference;
pub mod transport;

pub use contracts::{TelegramChunkHeader, TelegramDocumentMessage, TelegramStorageConfig};
pub use error::{Result, TelegramError};
pub use provider::TelegramStorageProvider;
pub use reference::TelegramReference;
pub use transport::{MockTelegramTransport, TelegramTransport};

pub use televault_core as core;
pub use televault_manifest as manifest;
pub use televault_storage as storage;

/// Returns the telegram module status string.
pub fn telegram_module_status() -> &'static str {
    "initialized"
}
