//! Typed database errors and results for TELEVAULT.

use thiserror::Error;

/// Structured error type for database and persistence operations.
#[derive(Debug, Error)]
pub enum DbError {
    /// Failed to open or establish database connection.
    #[error("Database connection error: {0}")]
    Connection(String),

    /// Schema migration failure.
    #[error("Database migration error: {0}")]
    Migration(String),

    /// Underlying SQLite execution or syntax error.
    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),

    /// JSON serialization or deserialization failure.
    #[error("Database serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Manifest parsing, serialization, or validation error.
    #[error("Manifest error: {0}")]
    Manifest(#[from] televault_manifest::error::ManifestError),

    /// Database constraint violation.
    #[error("Database constraint violation: {0}")]
    Constraint(String),

    /// Entity not found in database.
    #[error("Database entity '{entity}' not found: {id}")]
    NotFound {
        /// Type of entity.
        entity: &'static str,
        /// Identifier of missing entity.
        id: String,
    },

    /// Corrupted, unexpected, or invalid data encountered in database.
    #[error("Invalid database data: {0}")]
    InvalidData(String),

    /// Transaction lifecycle or commit failure.
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Internal mutex lock poisoned.
    #[error("Database lock poisoned: {0}")]
    LockPoisoned(String),
}

impl From<DbError> for televault_core::AppError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::NotFound { entity, id } => televault_core::AppError::NotFound { entity, id },
            DbError::Constraint(msg) => televault_core::AppError::Conflict {
                entity: "Database",
                reason: msg,
            },
            DbError::InvalidData(msg) => {
                televault_core::AppError::validation("database_record", msg)
            }
            DbError::Sql(ref sqlite_err) => {
                if let rusqlite::Error::SqliteFailure(err, _) = sqlite_err {
                    if err.code == rusqlite::ErrorCode::ConstraintViolation {
                        return televault_core::AppError::Conflict {
                            entity: "Database",
                            reason: sqlite_err.to_string(),
                        };
                    }
                }
                televault_core::AppError::Internal(err.to_string())
            }
            DbError::Connection(msg) => televault_core::AppError::Internal(msg),
            DbError::Migration(msg) => televault_core::AppError::Internal(msg),
            DbError::Serialization(e) => televault_core::AppError::Internal(e.to_string()),
            DbError::Manifest(e) => e.into(),
            DbError::Transaction(msg) => televault_core::AppError::Internal(msg),
            DbError::LockPoisoned(msg) => televault_core::AppError::Internal(msg),
        }
    }
}

/// Convenience result alias for database operations.
pub type Result<T> = std::result::Result<T, DbError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_mapping_to_app_error() {
        let not_found = DbError::NotFound {
            entity: "Profile",
            id: "prof-1".into(),
        };
        let app_err: televault_core::AppError = not_found.into();
        assert_eq!(app_err.error_code(), "NOT_FOUND");

        let constraint = DbError::Constraint("duplicate key".into());
        let app_err: televault_core::AppError = constraint.into();
        assert_eq!(app_err.error_code(), "CONFLICT");

        let invalid = DbError::InvalidData("bad size".into());
        let app_err: televault_core::AppError = invalid.into();
        assert_eq!(app_err.error_code(), "VALIDATION_ERROR");
    }
}
