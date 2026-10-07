//! Storage provider abstractions, streaming contracts, and temporary lifecycle management for TELEVAULT.

#![deny(missing_docs)]

pub mod error;
pub mod mock;
pub mod provider;
pub mod temp;
pub mod types;

pub use error::{Result, StorageError};
pub use mock::MockStorageProvider;
pub use provider::StorageProvider;
pub use temp::{TempPayloadFile, TempPayloadManager};
pub use types::*;

pub use televault_core as core;
pub use televault_manifest as manifest;

/// Returns the storage subsystem version string.
pub fn storage_version() -> &'static str {
    "0.1.0"
}
