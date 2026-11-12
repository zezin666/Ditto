//! Versioned JSON control frames for Ditto's current-user named pipe.

use std::io::{self, Read, Write};

use ditto_model::{
    ClipDetail, ClipSort, ClipSummary, CopyBufferSlot, GroupNode, PROTOCOL_VERSION, SearchMode,
    SettingsSnapshot,
};
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
    GetPopupVisibility,
    GetClips {
        #[serde(default)]
        query: String,
        #[serde(default)]
        group_id: Option<i64>,
        #[serde(default = "default_limit")]
        limit: u32,
        #[serde(default)]
        cursor: Option<String>,
        #[serde(default)]
        sort: ClipSort,
    },
    Search {
        #[serde(default)]
        query: String,
        #[serde(default)]
        group_id: Option<i64>,
        #[serde(default)]
        mode: SearchMode,
        #[serde(default)]
        sort: ClipSort,
        #[serde(default = "default_limit")]
        limit: u32,
        #[serde(default)]
        cursor: Option<String>,
    },
    GetClip {
        clip_id: i64,
    },
    Paste {
        #[serde(default)]
        clip_id: i64,
        #[serde(default)]
        plain_text: bool,
        #[serde(default)]
        formats: Vec<String>,
        #[serde(default)]
        clip_ids: Vec<i64>,
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
    CreateGroup {
        name: String,
        parent_id: Option<i64>,
        sort_order: f64,
    },
    RenameGroup {
        group_id: i64,
        name: String,
    },
    DeleteGroup {
        group_id: i64,
    },
    MoveGroup {
        group_id: i64,
        parent_id: Option<i64>,
        sort_order: f64,
    },
    GetCopyBuffers,
    CopyBufferAssign {
        slot: u8,
        clip_id: Option<i64>,
    },
    CopyBufferClear {
        slot: u8,
    },
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
        #[serde(default)]
        total_is_estimate: bool,
        took_ms: u64,
        next_cursor: Option<String>,
    },
    Clip {
        clip: Box<ClipDetail>,
    },
    Groups {
        groups: Vec<GroupNode>,
    },
    CopyBuffers {
        slots: Vec<CopyBufferSlot>,
    },
    Settings {
        #[serde(flatten)]
        snapshot: SettingsSnapshot,
        #[serde(default)]
        restart_required: bool,
    },
    PasteResult {
        outcome: PasteOutcome,
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
    GroupChanged {
        state_rev: u64,
    },
    ClipboardState {
        connected: bool,
    },
    SettingsChanged {
        keys: Vec<String>,
        revision: u64,
    },
    PasteResult {
        outcome: PasteOutcome,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PasteOutcome {
    pub ok: bool,
    pub mode: String,
    pub elevated: bool,
    pub target_pid: Option<u32>,
    pub pasted_clip_ids: Vec<i64>,
    pub failed_clip_id: Option<i64>,
    pub error_code: Option<ErrorCode>,
    pub message: String,
}

const fn default_limit() -> u32 {
    200
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

    #[test]
    fn settings_response_flattens_snapshot_to_documented_shape() {
        let response = Response::Settings {
            snapshot: ditto_model::SettingsSnapshot {
                values: serde_json::json!({"general": {"language": "en"}}),
                revision: 7,
            },
            restart_required: true,
        };
        let value = serde_json::to_value(response).expect("serialize settings");
        assert_eq!(value["values"]["general"]["language"], "en");
        assert_eq!(value["revision"], 7);
        assert_eq!(value["restart_required"], true);
    }

    #[test]
    fn legacy_paste_request_defaults_new_fields() {
        let request: Request =
            serde_json::from_str(r#"{"type":"Paste","clip_id":7,"plain_text":true}"#)
                .expect("decode legacy paste");
        assert!(matches!(
            request,
            Request::Paste {
                clip_id: 7,
                plain_text: true,
                formats,
                clip_ids
            } if formats.is_empty() && clip_ids.is_empty()
        ));
    }

    #[test]
    fn paste_result_round_trips_without_content() {
        let outcome = PasteOutcome {
            ok: false,
            mode: "multi".to_string(),
            elevated: false,
            target_pid: Some(42),
            pasted_clip_ids: vec![1],
            failed_clip_id: Some(2),
            error_code: Some(ErrorCode::Clipboard),
            message: "clipboard unavailable".to_string(),
        };
        let encoded = serde_json::to_string(&Response::PasteResult {
            outcome: outcome.clone(),
        })
        .expect("encode");
        assert!(!encoded.contains("sensitive clip"));
        let decoded: Response = serde_json::from_str(&encoded).expect("decode");
        assert!(matches!(
            decoded,
            Response::PasteResult { outcome: decoded } if decoded == outcome
        ));
    }
}
