//! SQLite storage, FTS5 search, settings, and content-addressed blobs.

mod blob;
mod error;
pub mod legacy;
mod schema;
mod settings_store;
mod store;

#[cfg(test)]
mod test_support;

pub use blob::{generate_thumbnail_png, sha256, sha256_hex};
pub use error::{StorageError, StorageResult};
pub use schema::{APP_VERSION, configure_connection, migrate, migrate_down_to_zero, now_ms};
pub use store::{InsertOutcome, Storage};
