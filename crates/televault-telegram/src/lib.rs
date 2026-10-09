//! Telegram communication, session management, and storage adapter for TELEVAULT.

#![deny(missing_docs)]

pub mod contracts;
pub mod credentials;
pub mod error;
pub mod http;
pub mod mtproto;
pub mod provider;
pub mod reference;
pub mod transport;

pub use contracts::{TelegramChunkHeader, TelegramDocumentMessage, TelegramStorageConfig};
pub use credentials::{TelegramApiCredentials, TelegramCredentials};
pub use error::{Result, TelegramError};
pub use http::HttpTelegramTransport;
pub use mtproto::{
    AuthState, GrammersMtprotoDriver, MockMtprotoDriver, MtprotoAuthManager, MtprotoDriver,
    PersistedTelegramSession, TelegramAccountInfo, TelegramAuthStatus, TelegramChannelInfo,
};
pub use provider::TelegramStorageProvider;
pub use reference::TelegramReference;
pub use transport::{MockTelegramTransport, TelegramConnectionInfo, TelegramTransport};

pub use televault_core as core;
pub use televault_manifest as manifest;
pub use televault_storage as storage;

/// Returns the telegram module status string.
pub fn telegram_module_status() -> &'static str {
    "initialized"
}
