//! Centralized error definitions and result types for TELEVAULT.

use thiserror::Error;

/// Structured, strongly-typed application error for TELEVAULT operations.
///
/// Designed to be safe for propagation across internal layers and eventual
/// serialization to the Tauri IPC boundary without leaking sensitive information.
#[derive(Debug, Error)]
pub enum AppError {
    /// Standard I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Application or module configuration error.
    #[error("Configuration error in '{field}': {message}")]
    Config {
        /// Configuration field or section.
        field: &'static str,
        /// Detail message.
        message: String,
    },

    /// Validation failure for domain invariants.
    #[error("Validation error for '{field}': {message}")]
    Validation {
        /// Name of the field or invariant that failed validation.
        field: &'static str,
        /// Explanation of validation failure.
        message: String,
    },

    /// Invalid or malformed domain identifier.
    #[error("Invalid {entity} identifier: {reason}")]
    InvalidId {
        /// Entity name (e.g., "Profile", "Snapshot").
        entity: &'static str,
        /// Reason for rejection.
        reason: String,
    },

    /// Requested domain entity was not found.
    #[error("{entity} not found: {id}")]
    NotFound {
        /// Entity type name.
        entity: &'static str,
        /// Identifier of missing entity.
        id: String,
    },

    /// Path access or traversal safety violation.
    #[error("Path error: {0}")]
    Path(String),

    /// Conflict when attempting a state mutation or entity creation.
    #[error("Conflict on {entity}: {reason}")]
    Conflict {
        /// Entity or operation name.
        entity: &'static str,
        /// Details of conflict.
        reason: String,
    },

    /// Invalid state transition for domain lifecycle.
    #[error("Invalid state transition: current state '{current}', expected '{expected}'")]
    InvalidState {
        /// The current state.
        current: String,
        /// The expected state.
        expected: String,
    },

    /// Internal runtime or logic failure.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Returns a standardized error code string suitable for IPC and telemetry.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO_ERROR",
            Self::Config { .. } => "CONFIG_ERROR",
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::InvalidId { .. } => "INVALID_IDENTIFIER",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::Path(_) => "PATH_ERROR",
            Self::Conflict { .. } => "CONFLICT",
            Self::InvalidState { .. } => "INVALID_STATE",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Convenience constructor for configuration errors.
    pub fn config(field: &'static str, message: impl Into<String>) -> Self {
        Self::Config {
            field,
            message: message.into(),
        }
    }

    /// Convenience constructor for validation errors.
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            field,
            message: message.into(),
        }
    }

    /// Convenience constructor for not-found errors.
    pub fn not_found(entity: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity,
            id: id.into(),
        }
    }
}

/// Convenience result alias using [`AppError`].
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_formatting() {
        let config_err = AppError::config("storage.cache", "must be positive");
        assert_eq!(
            config_err.to_string(),
            "Configuration error in 'storage.cache': must be positive"
        );
        assert_eq!(config_err.error_code(), "CONFIG_ERROR");

        let val_err = AppError::validation("name", "cannot be blank");
        assert_eq!(
            val_err.to_string(),
            "Validation error for 'name': cannot be blank"
        );
        assert_eq!(val_err.error_code(), "VALIDATION_ERROR");

        let id_err = AppError::InvalidId {
            entity: "Profile",
            reason: "empty string".into(),
        };
        assert_eq!(
            id_err.to_string(),
            "Invalid Profile identifier: empty string"
        );
        assert_eq!(id_err.error_code(), "INVALID_IDENTIFIER");

        let not_found_err = AppError::not_found("Snapshot", "snap-123");
        assert_eq!(not_found_err.to_string(), "Snapshot not found: snap-123");
        assert_eq!(not_found_err.error_code(), "NOT_FOUND");

        let path_err = AppError::Path("access denied".into());
        assert_eq!(path_err.to_string(), "Path error: access denied");
        assert_eq!(path_err.error_code(), "PATH_ERROR");

        let conflict_err = AppError::Conflict {
            entity: "Job",
            reason: "already running".into(),
        };
        assert_eq!(conflict_err.to_string(), "Conflict on Job: already running");
        assert_eq!(conflict_err.error_code(), "CONFLICT");

        let state_err = AppError::InvalidState {
            current: "Paused".into(),
            expected: "Active".into(),
        };
        assert_eq!(
            state_err.to_string(),
            "Invalid state transition: current state 'Paused', expected 'Active'"
        );
        assert_eq!(state_err.error_code(), "INVALID_STATE");

        let internal_err = AppError::Internal("unexpected panic".into());
        assert_eq!(internal_err.to_string(), "Internal error: unexpected panic");
        assert_eq!(internal_err.error_code(), "INTERNAL_ERROR");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
        assert_eq!(app_err.error_code(), "IO_ERROR");
    }
}
