use thiserror::Error;

pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("migration failed: {0}")]
    Migration(String),
}

impl From<StorageError> for ditto_model::DittoError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::NotFound(message) => Self::NotFound(message),
            StorageError::Conflict(message) => Self::Conflict(message),
            StorageError::Migration(message) => Self::Migration(message),
            other => Self::Storage(other.to_string()),
        }
    }
}
