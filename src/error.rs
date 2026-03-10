//! Error types for the media-lock library.

use thiserror::Error;

/// Main error type for the application.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("File is locked by another session: {0}")]
    FileLocked(String),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Authentication failed (wrong password or corrupted file)")]
    AuthenticationFailed,

    #[error("File is already encrypted")]
    AlreadyEncrypted,

    #[error("File is not encrypted")]
    NotEncrypted,

    #[error("Previous session failed, recovery required")]
    PreviousSessionFailed,

    #[error("Unsupported container format: {0}")]
    UnsupportedFormat(String),

    #[error("Invalid file structure: {0}")]
    InvalidStructure(String),

    #[error("WAL checksum mismatch")]
    WalChecksumError,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Integer overflow in offset calculation")]
    IntegerOverflow,
}

/// Result type alias for convenience.
pub type Result<T> = std::result::Result<T, AppError>;
