//! Paste target tracking, clipboard materialization, SendInput, and OLE drag-out.

mod clipboard;
mod drag;
mod elevation;
mod engine;
mod error;
mod input;
mod model;
mod payload;
mod target;

pub use clipboard::WindowsClipboardBackend;
pub use drag::{drag_out, parse_file_drop};
pub use engine::{
    ClipboardBackend, ElevationBackend, InputBackend, PasteEngine, PasteFailure, PasteReport,
    TargetBackend, WindowsPasteEngine,
};
pub use error::{PasteError, PasteErrorCode, PasteResult};
pub use input::SendInputBackend;
pub use model::{BatchMode, IntegrityLevel, PasteFormat, PasteOptions, TargetWindow};
pub use payload::{build_file_drop, build_payload, validate_file_paths};
pub use target::WindowsTargetBackend;
