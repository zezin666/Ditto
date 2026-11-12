use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{UacError, UacResult};

pub const HELPER_PROTOCOL_VERSION: u16 = 1;
pub const MAX_HELPER_MESSAGE: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperMode {
    Normal,
    Plain,
    Multi,
}

impl HelperMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Plain => "plain",
            Self::Multi => "multi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperArgs {
    pub core_pid: u32,
    pub target_pid: u32,
    pub target_window: isize,
    pub clip_id: i64,
    pub mode: HelperMode,
    pub request_id: u64,
    pub focus_delay_ms: u64,
}

impl HelperArgs {
    pub fn parse(args: impl IntoIterator<Item = String>) -> UacResult<Self> {
        let mut core_pid = None;
        let mut target_pid = None;
        let mut target_window = None;
        let mut clip_id = None;
        let mut mode = None;
        let mut request_id = None;
        let mut focus_delay_ms = Some(0_u64);
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| UacError::Arguments(format!("missing value after {argument}")))?;
            match argument.as_str() {
                "--core-pid" => core_pid = Some(parse_u32(&value, &argument)?),
                "--target-pid" => target_pid = Some(parse_u32(&value, &argument)?),
                "--target-window" => target_window = Some(parse_isize(&value, &argument)?),
                "--clip-id" => clip_id = Some(parse_i64(&value, &argument)?),
                "--request-id" => request_id = Some(parse_u64(&value, &argument)?),
                "--mode" => {
                    mode = Some(match value.as_str() {
                        "normal" => HelperMode::Normal,
                        "plain" => HelperMode::Plain,
                        "multi" => HelperMode::Multi,
                        _ => {
                            return Err(UacError::Arguments(format!(
                                "unsupported helper mode {value}"
                            )));
                        }
                    });
                }
                "--focus-delay-ms" => {
                    focus_delay_ms = Some(parse_u64(&value, &argument)?.min(300));
                }
                other => {
                    return Err(UacError::Arguments(format!(
                        "unsupported helper argument {other}"
                    )));
                }
            }
        }
        let parsed = Self {
            core_pid: core_pid.ok_or_else(|| missing("--core-pid"))?,
            target_pid: target_pid.ok_or_else(|| missing("--target-pid"))?,
            target_window: target_window.ok_or_else(|| missing("--target-window"))?,
            clip_id: clip_id.ok_or_else(|| missing("--clip-id"))?,
            mode: mode.ok_or_else(|| missing("--mode"))?,
            request_id: request_id.ok_or_else(|| missing("--request-id"))?,
            focus_delay_ms: focus_delay_ms.unwrap_or_default(),
        };
        if parsed.core_pid == 0
            || parsed.target_pid == 0
            || parsed.target_window == 0
            || parsed.clip_id <= 0
            || parsed.core_pid == parsed.target_pid
        {
            return Err(UacError::Arguments(
                "helper target/core process and window bindings are invalid".to_string(),
            ));
        }
        Ok(parsed)
    }
}

fn missing(argument: &str) -> UacError {
    UacError::Arguments(format!("missing required argument {argument}"))
}

fn parse_u32(value: &str, argument: &str) -> UacResult<u32> {
    value
        .parse()
        .map_err(|_| UacError::Arguments(format!("invalid {argument} value")))
}

fn parse_i64(value: &str, argument: &str) -> UacResult<i64> {
    value
        .parse()
        .map_err(|_| UacError::Arguments(format!("invalid {argument} value")))
}

fn parse_u64(value: &str, argument: &str) -> UacResult<u64> {
    value
        .parse()
        .map_err(|_| UacError::Arguments(format!("invalid {argument} value")))
}

fn parse_isize(value: &str, argument: &str) -> UacResult<isize> {
    value
        .parse()
        .map_err(|_| UacError::Arguments(format!("invalid {argument} value")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum HelperMessage {
    Hello {
        protocol: u16,
        request_id: u64,
        core_pid: u32,
        target_pid: u32,
        target_window: isize,
        clip_id: i64,
        mode: HelperMode,
        client_pid: u32,
        user_sid: String,
    },
    Authorize {
        request_id: u64,
        token: [u8; 32],
        expires_at_ms: u64,
        target_pid: u32,
        target_window: isize,
        focus_delay_ms: u64,
    },
    Ready {
        request_id: u64,
    },
    Done {
        request_id: u64,
        ok: bool,
        error: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct HelperHello {
    pub protocol: u16,
    pub request_id: u64,
    pub core_pid: u32,
    pub target_pid: u32,
    pub target_window: isize,
    pub clip_id: i64,
    pub mode: HelperMode,
    pub client_pid: u32,
    pub user_sid: String,
}

#[derive(Debug, Clone)]
pub struct OneShotAuthorize {
    pub request_id: u64,
    pub token: [u8; 32],
    pub expires_at_ms: u64,
    pub target_pid: u32,
    pub target_window: isize,
    pub focus_delay_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct HelperReady {
    pub request_id: u64,
}

#[derive(Debug, Clone)]
pub struct HelperDone {
    pub request_id: u64,
    pub ok: bool,
    pub error: Option<String>,
}

impl From<HelperArgs> for HelperHello {
    fn from(args: HelperArgs) -> Self {
        Self {
            protocol: HELPER_PROTOCOL_VERSION,
            request_id: args.request_id,
            core_pid: args.core_pid,
            target_pid: args.target_pid,
            target_window: args.target_window,
            clip_id: args.clip_id,
            mode: args.mode,
            client_pid: std::process::id(),
            user_sid: String::new(),
        }
    }
}

impl HelperHello {
    pub fn validate_against(&self, args: &HelperArgs, actual_pid: u32) -> UacResult<()> {
        if self.protocol != HELPER_PROTOCOL_VERSION
            || self.request_id != args.request_id
            || self.core_pid != args.core_pid
            || self.target_pid != args.target_pid
            || self.target_window != args.target_window
            || self.clip_id != args.clip_id
            || self.mode != args.mode
            || self.client_pid != actual_pid
        {
            return Err(UacError::Authentication(
                "helper hello does not match the launched request".to_string(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> Vec<String> {
        [
            "--core-pid",
            "10",
            "--target-pid",
            "20",
            "--target-window",
            "4660",
            "--clip-id",
            "7",
            "--mode",
            "plain",
            "--request-id",
            "99",
            "--focus-delay-ms",
            "80",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect()
    }

    #[test]
    fn helper_accepts_only_the_fixed_argument_set() {
        let parsed = HelperArgs::parse(args()).expect("parse");
        assert_eq!(parsed.target_pid, 20);
        assert_eq!(parsed.mode, HelperMode::Plain);
        assert_eq!(parsed.focus_delay_ms, 80);

        let mut rejected = args();
        rejected.extend(["--pipe-name".to_string(), r"\\.\pipe\evil".to_string()]);
        assert!(matches!(
            HelperArgs::parse(rejected),
            Err(UacError::Arguments(_))
        ));

        let mut self_target = args();
        self_target[3] = "10".to_string();
        assert!(matches!(
            HelperArgs::parse(self_target),
            Err(UacError::Arguments(_))
        ));
    }

    #[test]
    fn hello_must_match_every_launched_request_field() {
        let parsed = HelperArgs::parse(args()).expect("parse");
        let hello = HelperHello::from(parsed.clone());
        hello
            .validate_against(&parsed, std::process::id())
            .expect("matching hello");

        let mut changed = hello;
        changed.target_pid += 1;
        assert!(matches!(
            changed.validate_against(&parsed, std::process::id()),
            Err(UacError::Authentication(_))
        ));
    }
}
