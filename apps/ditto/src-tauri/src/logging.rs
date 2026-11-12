//! Runtime-reloadable log filter.
//!
//! `tauri-plugin-log` builds a `fern` dispatch once at startup; changing
//! `log::set_max_level` afterwards is not enough because fern's own level
//! filter has already discarded the record. The plugin's builder-level
//! `.filter(..)` runs for every record, so this module owns the real decision
//! in an atomic value that both startup and Settings changes update.

use std::sync::atomic::{AtomicU8, Ordering};

use log::{Level, LevelFilter, Metadata};

const ERROR: u8 = 1;
const WARN: u8 = 2;
const INFO: u8 = 3;
const DEBUG: u8 = 4;
const TRACE: u8 = 5;

static RUNTIME_LEVEL: AtomicU8 = AtomicU8::new(INFO);

pub fn level_from_setting(value: &str) -> LevelFilter {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

fn code(level: LevelFilter) -> u8 {
    match level {
        LevelFilter::Off => 0,
        LevelFilter::Error => ERROR,
        LevelFilter::Warn => WARN,
        LevelFilter::Info => INFO,
        LevelFilter::Debug => DEBUG,
        LevelFilter::Trace => TRACE,
    }
}

fn level_code(level: Level) -> u8 {
    match level {
        Level::Error => ERROR,
        Level::Warn => WARN,
        Level::Info => INFO,
        Level::Debug => DEBUG,
        Level::Trace => TRACE,
    }
}

/// Updates the filter used by the installed logger. Safe to call from any
/// thread; the next log record observes the new level.
pub fn set_runtime_level(level: LevelFilter) {
    RUNTIME_LEVEL.store(code(level), Ordering::SeqCst);
    // The plugin installs a Trace max level; keep the global gate open so
    // raising the level at runtime can produce previously filtered records.
    log::set_max_level(LevelFilter::Trace);
}

pub fn set_runtime_level_from_setting(value: &str) {
    set_runtime_level(level_from_setting(value));
}

pub fn runtime_level() -> LevelFilter {
    match RUNTIME_LEVEL.load(Ordering::SeqCst) {
        0 => LevelFilter::Off,
        ERROR => LevelFilter::Error,
        WARN => LevelFilter::Warn,
        INFO => LevelFilter::Info,
        DEBUG => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

/// Predicate passed to `tauri_plugin_log::Builder::filter`.
pub fn enabled(metadata: &Metadata<'_>) -> bool {
    level_code(metadata.level()) <= RUNTIME_LEVEL.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_names_map_to_filters() {
        assert_eq!(level_from_setting("error"), LevelFilter::Error);
        assert_eq!(level_from_setting("WARN"), LevelFilter::Warn);
        assert_eq!(level_from_setting("debug"), LevelFilter::Debug);
        assert_eq!(level_from_setting("trace"), LevelFilter::Trace);
        assert_eq!(level_from_setting("unknown"), LevelFilter::Info);
    }

    #[test]
    fn runtime_level_changes_the_predicate() {
        let debug = Metadata::builder()
            .level(Level::Debug)
            .target("ditto-test")
            .build();
        let info = Metadata::builder()
            .level(Level::Info)
            .target("ditto-test")
            .build();
        set_runtime_level(LevelFilter::Info);
        assert!(!enabled(&debug));
        assert!(enabled(&info));
        set_runtime_level(LevelFilter::Debug);
        assert!(enabled(&debug));
        set_runtime_level(LevelFilter::Error);
        assert!(!enabled(&info));
        set_runtime_level(LevelFilter::Info);
    }
}
