use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteErrorCode {
    NotFound,
    Busy,
    Clipboard,
    Elevation,
    NoTarget,
    TargetChanged,
    SecureDesktop,
    InvalidFiles,
    Unsupported,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PasteError {
    #[error("clip {0} was not found")]
    NotFound(i64),
    #[error("clipboard is busy")]
    Busy,
    #[error("clipboard operation failed: {0}")]
    Clipboard(String),
    #[error("elevation failed: {0}")]
    Elevation(String),
    #[error("there is no paste target")]
    NoTarget,
    #[error("the target window changed")]
    TargetChanged,
    #[error("the secure desktop cannot receive a paste")]
    SecureDesktop,
    #[error("invalid file drop: {0}")]
    InvalidFiles(String),
    #[error("unsupported paste payload: {0}")]
    Unsupported(String),
    #[error("paste was cancelled")]
    Cancelled,
    #[error("internal paste error: {0}")]
    Internal(String),
}

impl PasteError {
    pub const fn code(&self) -> PasteErrorCode {
        match self {
            Self::NotFound(_) => PasteErrorCode::NotFound,
            Self::Busy => PasteErrorCode::Busy,
            Self::Clipboard(_) => PasteErrorCode::Clipboard,
            Self::Elevation(_) => PasteErrorCode::Elevation,
            Self::NoTarget => PasteErrorCode::NoTarget,
            Self::TargetChanged => PasteErrorCode::TargetChanged,
            Self::SecureDesktop => PasteErrorCode::SecureDesktop,
            Self::InvalidFiles(_) => PasteErrorCode::InvalidFiles,
            Self::Unsupported(_) => PasteErrorCode::Unsupported,
            Self::Cancelled => PasteErrorCode::Cancelled,
            Self::Internal(_) => PasteErrorCode::Internal,
        }
    }
}

pub type PasteResult<T> = std::result::Result<T, PasteError>;
