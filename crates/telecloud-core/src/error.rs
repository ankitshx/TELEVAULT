use thiserror::Error;

#[derive(Debug, Error)]
pub enum TeleCloudError {
    #[error("Configuration error in {operation}: {message}")]
    Config {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Authentication error in {operation}: {message}")]
    Auth {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Storage error in {operation}: {message}")]
    Storage {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Database error in {operation}: {message}")]
    Database {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Cryptography error in {operation}: {message}")]
    Crypto {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Integrity error in {operation}: {message}")]
    Integrity {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Manifest error in {operation}: {message}")]
    Manifest {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("Transfer error in {operation}: {message}")]
    Transfer {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("I/O error in {operation}: {message}")]
    Io {
        operation: String,
        message: String,
        recoverable: bool,
    },

    #[error("System error in {operation}: {message}")]
    Internal {
        operation: String,
        message: String,
        recoverable: bool,
    },
}

impl TeleCloudError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Config { recoverable, .. }
            | Self::Auth { recoverable, .. }
            | Self::Storage { recoverable, .. }
            | Self::Database { recoverable, .. }
            | Self::Crypto { recoverable, .. }
            | Self::Integrity { recoverable, .. }
            | Self::Manifest { recoverable, .. }
            | Self::Transfer { recoverable, .. }
            | Self::Io { recoverable, .. }
            | Self::Internal { recoverable, .. } => *recoverable,
        }
    }

    pub fn operation(&self) -> &str {
        match self {
            Self::Config { operation, .. }
            | Self::Auth { operation, .. }
            | Self::Storage { operation, .. }
            | Self::Database { operation, .. }
            | Self::Crypto { operation, .. }
            | Self::Integrity { operation, .. }
            | Self::Manifest { operation, .. }
            | Self::Transfer { operation, .. }
            | Self::Io { operation, .. }
            | Self::Internal { operation, .. } => operation,
        }
    }
}

pub type Result<T> = std::result::Result<T, TeleCloudError>;
