//! Minimal elevation helper policy. The token and SendInput path belong to T4.

pub const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElevationRequest {
    pub request_id: u64,
    pub target_window: isize,
}

pub fn validate(_request: &ElevationRequest) -> ditto_model::Result<()> {
    Err(ditto_model::DittoError::NotImplemented(
        "UAC one-shot token validation",
    ))
}
