use thiserror::Error;

pub type ClipboardResult<T> = std::result::Result<T, ClipboardError>;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("clipboard is occupied by another process")]
    Occupied,
    #[error("clipboard read failed: {0}")]
    ReadFailed(String),
    #[error("unsupported clipboard format: {0}")]
    UnsupportedFormat(String),
    #[error("invalid clipboard data: {0}")]
    InvalidData(String),
    #[error("{format} is {size} bytes, exceeding the {limit} byte capture budget")]
    TooLarge {
        format: String,
        size: usize,
        limit: usize,
    },
    #[error("image conversion failed: {0}")]
    Image(String),
    #[error("Windows API error: {0}")]
    Windows(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ClipboardError> for ditto_model::DittoError {
    fn from(error: ClipboardError) -> Self {
        Self::Clipboard(error.to_string())
    }
}
