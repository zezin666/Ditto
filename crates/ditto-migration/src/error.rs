use ditto_storage::StorageError;
use thiserror::Error;

pub type MigrationResult<T> = std::result::Result<T, MigrationError>;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid migration input: {0}")]
    Invalid(String),
    #[error("checksum mismatch: {0}")]
    Checksum(String),
    #[error("unsupported format: {0}")]
    Unsupported(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

impl From<ditto_clipboard::ClipboardError> for MigrationError {
    fn from(error: ditto_clipboard::ClipboardError) -> Self {
        Self::Invalid(error.to_string())
    }
}
