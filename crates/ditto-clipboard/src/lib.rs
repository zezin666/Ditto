//! Clipboard capture and write boundary. Implementation belongs to T2.

use ditto_model::ClipKind;

#[derive(Debug, Clone)]
pub struct CapturedClip {
    pub kind: ClipKind,
    pub formats: Vec<String>,
}

pub trait ClipboardCapture {
    fn capture(&self) -> ditto_model::Result<Option<CapturedClip>>;
}

pub trait ClipboardWriter {
    fn write_text(&self, text: &str) -> ditto_model::Result<()>;
}
