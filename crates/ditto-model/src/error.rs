use thiserror::Error;

pub type Result<T> = std::result::Result<T, DittoError>;

#[derive(Debug, Error)]
pub enum DittoError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("IPC error: {0}")]
    Ipc(String),
    #[error("Windows API error: {0}")]
    Windows(String),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
