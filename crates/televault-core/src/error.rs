//! Unified error definitions for TELEVAULT.

use thiserror::Error;

/// Global application error type for TELEVAULT operations.
#[derive(Debug, Error)]
pub enum AppError {
    /// Standard I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Application or module configuration failure.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal runtime or logic failure.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Convenience result alias using [`AppError`].
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AppError::Config("missing setting".into());
        assert_eq!(err.to_string(), "Configuration error: missing setting");
    }
}
