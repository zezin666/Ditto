//! Shared domain types, configuration, and errors for the Ditto rewrite.

pub mod config;
pub mod error;
pub mod types;

pub use config::{AppConfig, DataRoot, ThemeMode};
pub use error::{DittoError, Result};
pub use types::{
    ClipKind, ClipSummary, CopyBufferSlot, GroupNode, ProtocolCapability, SettingsSnapshot,
};

/// Protocol and schema versions advertised by this foundation build.
pub const PROTOCOL_VERSION: u16 = 1;
pub const SCHEMA_VERSION: u32 = 1;
