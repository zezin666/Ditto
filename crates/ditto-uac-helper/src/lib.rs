//! One-shot elevation helper. The helper only restores a bound target and sends Ctrl+V.

mod error;
mod helper;
mod protocol;
mod token;
mod win32;

pub use error::{UacError, UacResult};
pub use helper::{helper_main, run_helper_from_args};
pub use protocol::{
    HELPER_PROTOCOL_VERSION, HelperArgs, HelperDone, HelperHello, HelperMode, HelperReady,
    OneShotAuthorize,
};
pub use token::{IssuedToken, TokenBinding, TokenError, TokenStore};
pub use win32::{launch_elevated_paste, send_ctrl_v};

pub const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
