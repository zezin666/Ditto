use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Text,
    Html,
    Rtf,
    Image,
    Files,
    Custom,
}

impl ClipKind {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Html => "html",
            Self::Rtf => "rtf",
            Self::Image => "image",
            Self::Files => "files",
            Self::Custom => "custom",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "html" => Some(Self::Html),
            "rtf" => Some(Self::Rtf),
            "image" => Some(Self::Image),
            "files" => Some(Self::Files),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
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
    pub pin_order: Option<f64>,
    pub group_id: Option<i64>,
    pub copy_buffer: Option<u8>,
    pub formats: Vec<String>,
    pub size_bytes: u64,
    pub has_image: bool,
    pub image_thumb_ref: Option<String>,
    pub pasted_count: u32,
    pub state_rev: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipFormatInfo {
    pub format_name: String,
    pub seq: u32,
    pub size_bytes: u64,
    pub mime: Option<String>,
    pub sha256: Vec<u8>,
    pub data: Option<Vec<u8>>,
    pub data_path: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipDetail {
    pub summary: ClipSummary,
    pub text_content: Option<String>,
    pub quick_paste_text: String,
    pub description: String,
    pub source_window_title: Option<String>,
    pub formats: Vec<ClipFormatInfo>,
    pub dont_auto_delete: bool,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewClipFormat {
    pub format_name: String,
    pub seq: u32,
    pub data: Vec<u8>,
    pub mime: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewClip {
    pub kind: ClipKind,
    pub primary_format: Option<String>,
    pub text_content: Option<String>,
    pub text_truncated: bool,
    pub preview_text: String,
    pub quick_paste_text: String,
    pub description: String,
    pub source_app: Option<String>,
    pub source_window_title: Option<String>,
    pub content_hash: Vec<u8>,
    pub formats_hash: Vec<u8>,
    pub hash_algo: String,
    pub size_bytes: u64,
    pub is_pinned: bool,
    pub pin_order: f64,
    pub group_id: Option<i64>,
    pub sort_order: f64,
    pub group_sort_order: f64,
    pub created_at_ms: i64,
    pub dont_auto_delete: bool,
    pub has_image: bool,
    pub thumbnail_png: Option<Vec<u8>>,
    pub formats: Vec<NewClipFormat>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipSort {
    #[default]
    Recent,
    Oldest,
    PinnedFirst,
    GroupOrder,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    #[default]
    Contains,
    Fulltext,
    QuickPaste,
    Wildcard,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipQuery {
    pub query: String,
    pub group_id: Option<i64>,
    #[serde(default)]
    pub kind: Option<ClipKind>,
    #[serde(default)]
    pub pinned_only: bool,
    #[serde(default)]
    pub mode: SearchMode,
    #[serde(default)]
    pub sort: ClipSort,
    pub limit: u32,
    pub cursor: Option<String>,
    #[serde(default = "default_true")]
    pub include_preview: bool,
}

impl Default for ClipQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            group_id: None,
            kind: None,
            pinned_only: false,
            mode: SearchMode::Contains,
            sort: ClipSort::Recent,
            limit: 200,
            cursor: None,
            include_preview: true,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPage {
    pub items: Vec<ClipSummary>,
    pub total: u64,
    pub total_is_estimate: bool,
    pub took_ms: u64,
    pub next_cursor: Option<String>,
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
