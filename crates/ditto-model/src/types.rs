use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Text,
    Image,
    Files,
    Rich,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipSummary {
    pub id: i64,
    pub kind: ClipKind,
    pub preview_text: String,
    pub source_app: Option<String>,
    pub created_at_ms: i64,
    pub last_pasted_at_ms: Option<i64>,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub group_id: Option<i64>,
    pub copy_buffer: Option<u8>,
    pub formats: Vec<String>,
    pub has_image: bool,
    pub image_thumb_ref: Option<String>,
    pub pasted_count: u32,
    pub state_rev: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyBufferSlot {
    pub slot: u8,
    pub name: String,
    pub clip_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupNode {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub sort_order: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolCapability {
    Clipboard,
    Groups,
    CopyBuffers,
    Preview,
    UacPaste,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub values: serde_json::Value,
    pub revision: u64,
}
