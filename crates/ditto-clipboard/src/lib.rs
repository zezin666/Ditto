//! Clipboard capture, format decoding, image conversion, and self-write suppression.

mod decode;
mod error;
mod windows;

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ditto_model::{ClipKind, NewClip, NewClipFormat};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

pub use decode::{
    decode_ansi_text, decode_utf16le, dib_to_png, hdrop_paths, html_fragment_bytes, html_preview,
    normalize_text_for_hash, rtf_preview, thumbnail_png,
};
pub use error::{ClipboardError, ClipboardResult};
pub use windows::{ClipboardMonitor, WindowsClipboardSource};

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub debounce_ms: u64,
    pub max_clip_bytes: usize,
    pub max_image_bytes: usize,
    pub ignored_formats: Vec<String>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 150,
            max_clip_bytes: 64 * 1024 * 1024,
            max_image_bytes: 32 * 1024 * 1024,
            ignored_formats: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawClipboardFormat {
    pub name: String,
    pub data: Vec<u8>,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct RawClipboardSnapshot {
    pub formats: Vec<RawClipboardFormat>,
    pub source_app: Option<String>,
    pub source_window_title: Option<String>,
    pub warnings: Vec<String>,
}

pub trait ClipboardSource: Send {
    fn read(&self) -> ClipboardResult<Option<RawClipboardSnapshot>>;
}

pub trait ClipboardCapture {
    fn capture(&self) -> ClipboardResult<Option<CapturedClip>>;
}

pub trait ClipboardWriter {
    fn write_text(&self, text: &str) -> ditto_model::Result<()>;
}

#[derive(Debug, Clone)]
pub struct CapturedFormat {
    pub name: String,
    pub seq: u32,
    pub data: Vec<u8>,
    pub mime: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct CapturedClip {
    pub kind: ClipKind,
    pub primary_format: Option<String>,
    pub text_content: Option<String>,
    pub preview_text: String,
    pub quick_paste_text: String,
    pub description: String,
    pub source_app: Option<String>,
    pub source_window_title: Option<String>,
    pub content_hash: [u8; 32],
    pub formats_hash: [u8; 32],
    pub size_bytes: u64,
    pub has_image: bool,
    pub thumbnail_png: Option<Vec<u8>>,
    pub created_at_ms: i64,
    pub formats: Vec<CapturedFormat>,
    pub warnings: Vec<String>,
}

impl CapturedClip {
    pub fn into_new_clip(self) -> NewClip {
        let sort_order = self.created_at_ms as f64;
        NewClip {
            kind: self.kind,
            primary_format: self.primary_format,
            text_content: self.text_content,
            text_truncated: false,
            preview_text: self.preview_text,
            quick_paste_text: self.quick_paste_text,
            description: self.description,
            source_app: self.source_app,
            source_window_title: self.source_window_title,
            content_hash: self.content_hash.to_vec(),
            formats_hash: self.formats_hash.to_vec(),
            hash_algo: "sha256".to_string(),
            size_bytes: self.size_bytes,
            is_pinned: false,
            pin_order: 0.0,
            group_id: None,
            sort_order,
            group_sort_order: sort_order,
            created_at_ms: self.created_at_ms,
            dont_auto_delete: false,
            has_image: self.has_image,
            thumbnail_png: self.thumbnail_png,
            formats: self
                .formats
                .into_iter()
                .map(|format| NewClipFormat {
                    format_name: format.name,
                    seq: format.seq,
                    data: format.data,
                    mime: format.mime,
                    is_primary: format.is_primary,
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
pub enum CaptureDisposition {
    Captured(Box<CapturedClip>),
    Empty,
    Suppressed,
}

#[derive(Default)]
pub struct WriteSuppression {
    expected: Mutex<Option<[u8; 32]>>,
}

impl WriteSuppression {
    pub fn remember(&self, content_hash: [u8; 32]) {
        *self.expected.lock() = Some(content_hash);
    }

    fn consume_if_matches(&self, content_hash: [u8; 32]) -> bool {
        let expected = self.expected.lock().take();
        expected == Some(content_hash)
    }
}

pub struct CaptureEngine<S> {
    source: S,
    config: CaptureConfig,
    suppression: Arc<WriteSuppression>,
}

impl<S: ClipboardSource> CaptureEngine<S> {
    pub fn new(source: S, config: CaptureConfig, suppression: Arc<WriteSuppression>) -> Self {
        Self {
            source,
            config,
            suppression,
        }
    }

    pub fn capture_once(&self) -> ClipboardResult<CaptureDisposition> {
        let Some(snapshot) = self.source.read()? else {
            return Ok(CaptureDisposition::Empty);
        };
        let clip = build_captured_clip(snapshot, &self.config)?;
        if self.suppression.consume_if_matches(clip.content_hash) {
            return Ok(CaptureDisposition::Suppressed);
        }
        Ok(CaptureDisposition::Captured(Box::new(clip)))
    }

    pub fn suppression(&self) -> &Arc<WriteSuppression> {
        &self.suppression
    }
}

impl<S: ClipboardSource> ClipboardCapture for CaptureEngine<S> {
    fn capture(&self) -> ClipboardResult<Option<CapturedClip>> {
        match self.capture_once()? {
            CaptureDisposition::Captured(clip) => Ok(Some(*clip)),
            CaptureDisposition::Empty | CaptureDisposition::Suppressed => Ok(None),
        }
    }
}

pub fn build_captured_clip(
    snapshot: RawClipboardSnapshot,
    config: &CaptureConfig,
) -> ClipboardResult<CapturedClip> {
    let mut formats = Vec::new();
    let mut warnings = snapshot.warnings;
    let mut seq = 0_u32;
    let mut image_png = None;
    let mut has_dib = false;

    for raw in snapshot.formats {
        if raw.data.is_empty() {
            warnings.push(format!("{} was empty", raw.name));
            continue;
        }
        let image = is_image_format(&raw.name);
        let limit = if image {
            config.max_image_bytes
        } else {
            config.max_clip_bytes
        };
        if raw.data.len() > limit {
            warnings.push(format!("{} exceeded the {} byte limit", raw.name, limit));
            continue;
        }
        if raw.name == "PNG" {
            image_png.get_or_insert_with(|| raw.data.clone());
        }
        if matches!(raw.name.as_str(), "CF_DIB" | "CF_DIBV5") {
            has_dib = true;
            if image_png.is_none() {
                match dib_to_png(&raw.data) {
                    Ok(png) => image_png = Some(png),
                    Err(error) => warnings.push(format!("{} conversion failed: {error}", raw.name)),
                }
            }
        }
        formats.push(CapturedFormat {
            mime: mime_for_format(&raw.name),
            is_primary: raw.is_primary,
            name: raw.name,
            seq,
            data: raw.data,
        });
        seq += 1;
    }

    if image_png.is_none() {
        image_png = formats
            .iter()
            .find(|format| format.name == "PNG")
            .map(|format| format.data.clone());
    }
    if has_dib && !formats.iter().any(|format| format.name == "PNG") {
        if let Some(png) = image_png.as_ref() {
            formats.push(CapturedFormat {
                name: "PNG".to_string(),
                seq,
                data: png.clone(),
                mime: Some("image/png".to_string()),
                is_primary: false,
            });
        }
    }
    if formats.is_empty() {
        return Err(ClipboardError::ReadFailed(
            "clipboard contained no readable formats".to_string(),
        ));
    }

    let primary_index = primary_format_index(&formats);
    for (index, format) in formats.iter_mut().enumerate() {
        format.is_primary = index == primary_index;
    }
    let primary = &formats[primary_index];
    let content_hash = content_hash(primary);
    let formats_hash = formats_hash(&formats);
    let text_content = derive_text_content(&formats);
    let preview_text = text_content
        .as_deref()
        .map(truncate_chars)
        .unwrap_or_default();
    let kind = derive_kind(&formats);
    let has_image = formats.iter().any(|format| is_image_format(&format.name));
    let thumbnail_png = image_png
        .as_deref()
        .and_then(|png| match thumbnail_png(png) {
            Ok(thumbnail) => Some(thumbnail),
            Err(error) => {
                warnings.push(format!("thumbnail generation failed: {error}"));
                None
            }
        });
    let size_bytes = formats.iter().map(|format| format.data.len() as u64).sum();
    let description = preview_text.clone();
    Ok(CapturedClip {
        kind,
        primary_format: Some(primary.name.clone()),
        text_content,
        preview_text,
        quick_paste_text: String::new(),
        description,
        source_app: snapshot.source_app,
        source_window_title: snapshot.source_window_title,
        content_hash,
        formats_hash,
        size_bytes,
        has_image,
        thumbnail_png,
        created_at_ms: now_ms(),
        formats,
        warnings,
    })
}

fn primary_format_index(formats: &[CapturedFormat]) -> usize {
    for preferred in [
        "CF_UNICODETEXT",
        "HTML Format",
        "Rich Text Format",
        "PNG",
        "CF_DIB",
        "CF_DIBV5",
        "CF_HDROP",
    ] {
        if let Some(index) = formats.iter().position(|format| format.name == preferred) {
            return index;
        }
    }
    formats
        .iter()
        .position(|format| format.is_primary)
        .unwrap_or(0)
}

fn content_hash(format: &CapturedFormat) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(format.name.as_bytes());
    hasher.update([0]);
    match format.name.as_str() {
        "CF_UNICODETEXT" => {
            let text = decode_utf16le(&format.data);
            let normalized = normalize_text_for_hash(&text)
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            hasher.update(normalized);
        }
        "CF_TEXT" | "CF_OEMTEXT" | "HTML Format" | "Rich Text Format" | "CF_HDROP" => {
            let text = decode_ansi_text(&format.data);
            hasher.update(normalize_text_for_hash(&text).as_bytes());
        }
        _ => hasher.update(&format.data),
    }
    hasher.finalize().into()
}

fn formats_hash(formats: &[CapturedFormat]) -> [u8; 32] {
    let mut pairs = formats
        .iter()
        .map(|format| (format.name.clone(), sha256(&format.data)))
        .collect::<Vec<_>>();
    pairs.sort();
    let mut hasher = Sha256::new();
    for (name, hash) in pairs {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(hash);
    }
    hasher.finalize().into()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn derive_text_content(formats: &[CapturedFormat]) -> Option<String> {
    if let Some(format) = formats
        .iter()
        .find(|format| format.name == "CF_UNICODETEXT")
    {
        return Some(decode_utf16le(&format.data));
    }
    if let Some(format) = formats
        .iter()
        .find(|format| matches!(format.name.as_str(), "CF_TEXT" | "CF_OEMTEXT"))
    {
        return Some(decode_ansi_text(&format.data));
    }
    if let Some(format) = formats.iter().find(|format| format.name == "HTML Format") {
        return Some(html_preview(&format.data));
    }
    if let Some(format) = formats
        .iter()
        .find(|format| format.name == "Rich Text Format")
    {
        return Some(rtf_preview(&format.data));
    }
    if let Some(format) = formats.iter().find(|format| format.name == "CF_HDROP") {
        return hdrop_paths(&format.data)
            .ok()
            .map(|paths| paths.join("\r\n"));
    }
    None
}

fn derive_kind(formats: &[CapturedFormat]) -> ClipKind {
    if formats.iter().any(|format| format.name == "CF_UNICODETEXT") {
        ClipKind::Text
    } else if formats.iter().any(|format| format.name == "HTML Format") {
        ClipKind::Html
    } else if formats
        .iter()
        .any(|format| format.name == "Rich Text Format")
    {
        ClipKind::Rtf
    } else if formats.iter().any(|format| is_image_format(&format.name)) {
        ClipKind::Image
    } else if formats.iter().any(|format| format.name == "CF_HDROP") {
        ClipKind::Files
    } else {
        ClipKind::Custom
    }
}

fn mime_for_format(name: &str) -> Option<String> {
    match name {
        "CF_UNICODETEXT" | "CF_TEXT" | "CF_OEMTEXT" => Some("text/plain".to_string()),
        "HTML Format" => Some("text/html".to_string()),
        "Rich Text Format" => Some("application/rtf".to_string()),
        "PNG" => Some("image/png".to_string()),
        "CF_DIB" | "CF_DIBV5" => Some("image/bmp".to_string()),
        "CF_HDROP" => Some("application/x-windows-hdrop".to_string()),
        _ => None,
    }
}

fn is_image_format(name: &str) -> bool {
    matches!(name, "PNG" | "CF_DIB" | "CF_DIBV5")
}

fn truncate_chars(value: &str) -> String {
    value.chars().take(512).collect()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemorySource {
        snapshot: Mutex<Option<RawClipboardSnapshot>>,
    }

    impl ClipboardSource for MemorySource {
        fn read(&self) -> ClipboardResult<Option<RawClipboardSnapshot>> {
            Ok(self.snapshot.lock().take())
        }
    }

    fn snapshot(text: &str) -> RawClipboardSnapshot {
        RawClipboardSnapshot {
            formats: vec![
                RawClipboardFormat {
                    name: "CF_UNICODETEXT".to_string(),
                    data: text
                        .encode_utf16()
                        .chain(Some(0))
                        .flat_map(u16::to_le_bytes)
                        .collect(),
                    is_primary: false,
                },
                RawClipboardFormat {
                    name: "HTML Format".to_string(),
                    data:
                        b"Version:0.9\r\nStartFragment:0000000045\r\nEndFragment:0000000060\r\n<b>QA sample</b>"
                            .to_vec(),
                    is_primary: false,
                },
            ],
            source_app: Some("synthetic.exe".to_string()),
            source_window_title: Some("Synthetic fixture".to_string()),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn synthetic_multi_format_event_becomes_one_captured_clip() {
        let source = MemorySource {
            snapshot: Mutex::new(Some(snapshot("QA sample 001"))),
        };
        let engine = CaptureEngine::new(
            source,
            CaptureConfig::default(),
            Arc::new(WriteSuppression::default()),
        );
        let CaptureDisposition::Captured(clip) = engine.capture_once().expect("capture") else {
            panic!("expected capture");
        };
        assert_eq!(clip.formats.len(), 2);
        assert_eq!(clip.kind, ClipKind::Text);
        assert_eq!(clip.source_app.as_deref(), Some("synthetic.exe"));
        assert_eq!(clip.text_content.as_deref(), Some("QA sample 001"));
        let new = clip.into_new_clip();
        assert_eq!(new.formats.len(), 2);
        assert_eq!(new.content_hash.len(), 32);
    }

    #[test]
    fn self_write_suppression_consumes_the_next_matching_hash() {
        let source = MemorySource {
            snapshot: Mutex::new(Some(snapshot("loop guard"))),
        };
        let suppression = Arc::new(WriteSuppression::default());
        let engine = CaptureEngine::new(source, CaptureConfig::default(), Arc::clone(&suppression));
        let CaptureDisposition::Captured(clip) = engine.capture_once().expect("first capture")
        else {
            panic!("expected capture");
        };
        suppression.remember(clip.content_hash);
        *engine.source.snapshot.lock() = Some(snapshot("loop guard"));
        assert!(matches!(
            engine.capture_once().expect("suppressed"),
            CaptureDisposition::Suppressed
        ));
    }

    #[test]
    fn oversized_and_ignored_formats_do_not_block_other_formats() {
        let mut sample = snapshot("kept");
        sample.formats.push(RawClipboardFormat {
            name: "Custom Large".to_string(),
            data: vec![1; 20],
            is_primary: false,
        });
        let config = CaptureConfig {
            max_clip_bytes: 10,
            ..CaptureConfig::default()
        };
        let clip = build_captured_clip(sample, &config).expect("capture");
        assert!(
            clip.formats
                .iter()
                .all(|format| format.name != "Custom Large")
        );
        assert_eq!(clip.text_content.as_deref(), Some("kept"));
    }

    #[test]
    fn text_hash_normalizes_line_endings_for_dedupe() {
        let first =
            build_captured_clip(snapshot("line one\r\nline two"), &CaptureConfig::default())
                .expect("first");
        let second = build_captured_clip(snapshot("line one\nline two"), &CaptureConfig::default())
            .expect("second");
        assert_eq!(first.content_hash, second.content_hash);
    }
}
