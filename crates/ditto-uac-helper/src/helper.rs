use crate::{UacError, UacResult, protocol::HelperArgs, win32::run_helper};

pub fn helper_main() -> i32 {
    match run_helper_from_args(std::env::args().skip(1)) {
        Ok(()) => 0,
        Err(error) => {
            tracing::warn!(%error, "UAC paste helper failed");
            match error {
                UacError::Arguments(_) => 2,
                UacError::Cancelled => 3,
                UacError::Authentication(_) | UacError::Validation(_) => 4,
                UacError::Timeout(_) => 5,
                UacError::TargetUnavailable => 6,
                UacError::SendInput => 7,
                UacError::HelperNotFound => 8,
                UacError::Windows(_) | UacError::Ipc(_) => 9,
            }
        }
    }
}

pub fn run_helper_from_args(args: impl IntoIterator<Item = String>) -> UacResult<()> {
    let args = HelperArgs::parse(args)?;
    run_helper(&args)
}
