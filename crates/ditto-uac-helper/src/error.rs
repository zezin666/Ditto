use thiserror::Error;

#[derive(Debug, Error)]
pub enum UacError {
    #[error("elevation request was cancelled")]
    Cancelled,
    #[error("the elevation helper was not found")]
    HelperNotFound,
    #[error("helper authentication failed: {0}")]
    Authentication(String),
    #[error("helper validation failed: {0}")]
    Validation(String),
    #[error("helper timed out: {0}")]
    Timeout(String),
    #[error("Windows API error: {0}")]
    Windows(String),
    #[error("IPC error: {0}")]
    Ipc(String),
    #[error("invalid helper arguments: {0}")]
    Arguments(String),
    #[error("SendInput failed")]
    SendInput,
    #[error("secure desktop or target window is unavailable")]
    TargetUnavailable,
}

impl From<windows::core::Error> for UacError {
    fn from(error: windows::core::Error) -> Self {
        Self::Windows(error.to_string())
    }
}

impl From<serde_json::Error> for UacError {
    fn from(error: serde_json::Error) -> Self {
        Self::Ipc(error.to_string())
    }
}

impl From<crate::TokenError> for UacError {
    fn from(error: crate::TokenError) -> Self {
        Self::Authentication(error.to_string())
    }
}

pub type UacResult<T> = std::result::Result<T, UacError>;
