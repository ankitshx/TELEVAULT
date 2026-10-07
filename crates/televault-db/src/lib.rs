//! Embedded database, migrations, and local catalog persistence for TELEVAULT.

#![deny(missing_docs)]

pub mod db;
pub mod error;
pub mod migrations;
pub mod models;

pub use db::Database;
pub use error::{DbError, Result};
pub use models::*;
pub use televault_core as core;
pub use televault_manifest as manifest;

/// Returns the current database schema version.
pub fn schema_version() -> u32 {
    1
}
