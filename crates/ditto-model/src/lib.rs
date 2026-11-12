//! Shared domain types, configuration, and errors for the Ditto rewrite.

pub mod config;
pub mod error;
pub mod settings;
pub mod types;

pub use config::{AppConfig, DataRoot, ThemeMode};
pub use error::{DittoError, Result};
pub use settings::{default_settings, legacy_setting_to_new};
pub use types::{
    ClipDetail, ClipFormatInfo, ClipKind, ClipPage, ClipQuery, ClipSort, ClipSummary,
    CopyBufferSlot, GroupNode, NewClip, NewClipFormat, ProtocolCapability, SearchMode,
    SettingsSnapshot,
};

/// Protocol and schema versions advertised by this foundation build.
pub const PROTOCOL_VERSION: u16 = 1;
pub const SCHEMA_VERSION: u32 = 1;
