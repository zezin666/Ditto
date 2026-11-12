//! Versioned JSON control frames for Ditto's current-user named pipe.

use std::io::{self, Read, Write};

use ditto_model::{ClipSummary, GroupNode, PROTOCOL_VERSION, SettingsSnapshot};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAGIC: [u8; 4] = *b"DTP1";
pub const HEADER_LEN: usize = 24;
pub const MAX_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum FrameType {
    Request = 1,
    Response = 2,
    Event = 3,
    BlobChunk = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Popup,
    Settings,
    Cli,
    Uac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadFrame,
    Protocol,
    Auth,
    NotReady,
    Busy,
    NotFound,
    Conflict,
    Database,
    Clipboard,
    Elevation,
    Cancelled,
    NotImplemented,
    Internal,
}

impl ErrorCode {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::BadFrame => "DITTO_E_BAD_FRAME",
            Self::Protocol => "DITTO_E_PROTO",
            Self::Auth => "DITTO_E_AUTH",
            Self::NotReady => "DITTO_E_NOT_READY",
            Self::Busy => "DITTO_E_BUSY",
            Self::NotFound => "DITTO_E_NOT_FOUND",
            Self::Conflict => "DITTO_E_CONFLICT",
            Self::Database => "DITTO_E_DB",
            Self::Clipboard => "DITTO_E_CLIPBOARD",
            Self::Elevation => "DITTO_E_ELEVATION",
            Self::Cancelled => "DITTO_E_CANCELLED",
            Self::NotImplemented => "DITTO_E_NOT_IMPLEMENTED",
            Self::Internal => "DITTO_E_INTERNAL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum Request {
    Hello {
        #[serde(rename = "proto_min")]
        proto_min: u16,
        #[serde(rename = "proto_max")]
        proto_max: u16,
        client: ClientKind,
        pid: u32,
        nonce: String,
        #[serde(default)]
        subscribe: bool,
    },
    ShowPopup,
    HidePopup,
    GetClips {
        query: String,
        limit: u32,
    },
    Search {
        query: String,
        limit: u32,
    },
    Paste {
        clip_id: i64,
        plain_text: bool,
    },
    SetPinned {
        clip_id: i64,
        pinned: bool,
    },
    MoveToGroup {
        clip_id: i64,
        group_id: Option<i64>,
    },
    Delete {
        clip_ids: Vec<i64>,
    },
    GetGroups,
    GetSettings,
    SetSettings {
        values: serde_json::Value,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum Response {
    HelloAck {
        proto: u16,
        core_pid: u32,
        session: String,
        schema_version: u32,
        capabilities: Vec<String>,
        state_rev: u64,
        theme_rev: u64,
    },
    PopupVisibility {
        visible: bool,
    },
    Clips {
        items: Vec<ClipSummary>,
        total: u64,
        took_ms: u64,
        next_cursor: Option<String>,
    },
    Groups {
        groups: Vec<GroupNode>,
    },
    Settings {
        values: SettingsSnapshot,
    },
    Ok,
    Error {
        code: ErrorCode,
        message: String,
        retryable: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum Event {
    ClipAdded {
        clip: ClipSummary,
    },
    ClipUpdated {
        id: i64,
        fields: Vec<String>,
        state_rev: u64,
    },
    ClipDeleted {
        ids: Vec<i64>,
        state_rev: u64,
    },
    SettingsChanged {
        keys: Vec<String>,
        revision: u64,
    },
    ShowPopup {
        at_ms: u64,
        monitor: Option<String>,
    },
    HidePopup,
    Shutdown {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Payload {
    Request(Request),
    Response(Response),
    Event(Event),
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub frame_type: FrameType,
    pub flags: u16,
    pub request_id: u64,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn json<T: Serialize>(
        frame_type: FrameType,
        request_id: u64,
        value: &T,
    ) -> Result<Self, IpcError> {
        let payload = serde_json::to_vec(value)?;
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(IpcError::PayloadTooLarge(payload.len()));
        }
        Ok(Self {
            frame_type,
            flags: 0,
            request_id,
            payload,
        })
    }

    pub fn decode_request(&self) -> Result<Request, IpcError> {
        Ok(serde_json::from_slice(&self.payload)?)
    }

    pub fn decode_response(&self) -> Result<Response, IpcError> {
        Ok(serde_json::from_slice(&self.payload)?)
    }

    pub fn decode_event(&self) -> Result<Event, IpcError> {
        Ok(serde_json::from_slice(&self.payload)?)
    }
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid frame magic")]
    InvalidMagic,
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("invalid frame type {0}")]
    InvalidFrameType(u16),
    #[error("payload is too large: {0} bytes")]
    PayloadTooLarge(usize),
    #[error("reserved header field must be zero")]
    ReservedNotZero,
}

pub fn write_frame<W: Write>(writer: &mut W, frame: &Frame) -> Result<(), IpcError> {
    if frame.payload.len() > MAX_PAYLOAD_LEN {
        return Err(IpcError::PayloadTooLarge(frame.payload.len()));
    }

    let mut header = [0_u8; HEADER_LEN];
    header[0..4].copy_from_slice(&MAGIC);
    header[4..6].copy_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    header[6..8].copy_from_slice(&(frame.frame_type as u16).to_le_bytes());
    header[8..10].copy_from_slice(&frame.flags.to_le_bytes());
    header[12..16].copy_from_slice(&(frame.payload.len() as u32).to_le_bytes());
    header[16..24].copy_from_slice(&frame.request_id.to_le_bytes());

    let mut message = Vec::with_capacity(HEADER_LEN + frame.payload.len());
    message.extend_from_slice(&header);
    message.extend_from_slice(&frame.payload);
    writer.write_all(&message)?;
    writer.flush()?;
    Ok(())
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<Frame, IpcError> {
    let mut header = [0_u8; HEADER_LEN];
    reader.read_exact(&mut header)?;
    if header[0..4] != MAGIC {
        return Err(IpcError::InvalidMagic);
    }
    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != PROTOCOL_VERSION {
        return Err(IpcError::UnsupportedVersion(version));
    }
    let frame_type = match u16::from_le_bytes([header[6], header[7]]) {
        1 => FrameType::Request,
        2 => FrameType::Response,
        3 => FrameType::Event,
        4 => FrameType::BlobChunk,
        other => return Err(IpcError::InvalidFrameType(other)),
    };
    let flags = u16::from_le_bytes([header[8], header[9]]);
    if header[10] != 0 || header[11] != 0 {
        return Err(IpcError::ReservedNotZero);
    }
    let payload_len = u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(IpcError::PayloadTooLarge(payload_len));
    }
    let request_id = u64::from_le_bytes(header[16..24].try_into().expect("fixed header size"));
    let mut payload = vec![0_u8; payload_len];
    reader.read_exact(&mut payload)?;
    Ok(Frame {
        frame_type,
        flags,
        request_id,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trip_uses_json_and_preserves_request_id() {
        let request = Request::Hello {
            proto_min: 1,
            proto_max: 1,
            client: ClientKind::Popup,
            pid: 42,
            nonce: "test".to_string(),
            subscribe: true,
        };
        let frame = Frame::json(FrameType::Request, 17, &request).expect("encode");
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &frame).expect("write");
        let decoded = read_frame(&mut bytes.as_slice()).expect("read");
        assert_eq!(decoded.request_id, 17);
        assert!(matches!(
            decoded.decode_request().expect("decode"),
            Request::Hello { .. }
        ));
    }

    #[test]
    fn oversized_payload_is_rejected_before_write() {
        let frame = Frame {
            frame_type: FrameType::Request,
            flags: 0,
            request_id: 1,
            payload: vec![0; MAX_PAYLOAD_LEN + 1],
        };
        assert!(matches!(
            write_frame(&mut Vec::new(), &frame),
            Err(IpcError::PayloadTooLarge(_))
        ));
    }

    #[test]
    fn invalid_magic_is_rejected() {
        let error = read_frame(&mut b"nope".as_slice()).expect_err("must fail");
        assert!(matches!(error, IpcError::Io(_)));
    }
}
