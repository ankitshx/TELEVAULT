//! Core domain models, errors, path management, and shared configuration for TELEVAULT.

#![deny(missing_docs)]

pub mod error;
pub mod paths;

pub use error::{AppError, Result};
pub use paths::PathManager;
