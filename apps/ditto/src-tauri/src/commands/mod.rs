//! Tauri commands for the Fluent settings window.
//!
//! Every command is backed by the live `ditto-storage` database owned by the
//! core service. Operations that must replace the whole data root (import and
//! restore) are validated and staged by [`crate::pending`], then applied on the
//! next core start while no SQLite handles are open.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use ditto_ipc::{Event, PasteOutcome};
use ditto_migration::{MigrationError, MigrationSource};
use ditto_model::{
    ClipKind, ClipQuery, ClipSort, PROTOCOL_VERSION, SCHEMA_VERSION, SearchMode, SettingsSnapshot,
};
use ditto_storage::{Storage, StorageError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{AppState, pending};

const COPY_BUFFER_SLOTS: u8 = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl CommandError {
    fn new(code: &str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            retryable,
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new("DITTO_E_NOT_FOUND", message, false)
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new("DITTO_E_CONFLICT", message, false)
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("DITTO_E_INVALID", message, false)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new("DITTO_E_INTERNAL", message, true)
    }
}

impl From<StorageError> for CommandError {
    fn from(error: StorageError) -> Self {
        let (code, retryable) = match &error {
            StorageError::NotFound(_) => ("DITTO_E_NOT_FOUND", false),
            StorageError::Conflict(_) | StorageError::InvalidData(_) => ("DITTO_E_CONFLICT", false),
            StorageError::Migration(_) => ("DITTO_E_NOT_READY", true),
            StorageError::Io(_) | StorageError::Json(_) => ("DITTO_E_INTERNAL", true),
            StorageError::Sqlite(error) => {
                let message = error.to_string().to_ascii_lowercase();
                if message.contains("locked") || message.contains("busy") {
                    ("DITTO_E_BUSY", true)
                } else {
                    ("DITTO_E_DB", false)
                }
            }
        };
        Self::new(code, error.to_string(), retryable)
    }
}

impl From<MigrationError> for CommandError {
    fn from(error: MigrationError) -> Self {
        let (code, retryable) = match &error {
            MigrationError::Conflict(_) => ("DITTO_E_CONFLICT", false),
            MigrationError::Invalid(_) | MigrationError::Unsupported(_) => {
                ("DITTO_E_INVALID", false)
            }
            MigrationError::Checksum(_) => ("DITTO_E_CHECKSUM", false),
            MigrationError::Io(_) | MigrationError::Json(_) => ("DITTO_E_INTERNAL", true),
            MigrationError::Sqlite(_) | MigrationError::Storage(_) => ("DITTO_E_DB", true),
        };
        Self::new(code, error.to_string(), retryable)
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(error.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(error.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    pub general: GeneralSettings,
    pub appearance: AppearanceSettings,
    pub hotkeys: HotkeySettings,
    pub clipboard: ClipboardSettings,
    pub groups: GroupSettings,
    pub copy_buffers: CopyBufferSettings,
    pub data: DataSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub run_at_startup: bool,
    pub start_minimized: bool,
    pub show_tray_icon: bool,
    pub single_instance: bool,
    pub language: String,
    pub startup_message: String,
    pub check_for_updates: bool,
    pub log_level: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: String,
    pub density: String,
    pub popup_width: u32,
    pub popup_height: u32,
    pub lines_per_clip: u8,
    pub preview: String,
    pub animations: bool,
    pub always_on_top: bool,
    pub font_scale: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettings {
    pub open: String,
    pub open_alt: String,
    pub plain_text_paste: String,
    pub first_ten_enabled: bool,
    pub first_ten_chords: Vec<String>,
    pub copy_buffers: Vec<CopyBufferHotkeys>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyBufferHotkeys {
    pub slot: u8,
    pub copy: String,
    pub paste: String,
    pub cut: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSettings {
    pub max_entries: u32,
    pub expire_days: u32,
    pub ignore_formats: Vec<String>,
    pub max_clip_bytes: u64,
    pub capture_files: bool,
    pub capture_images: bool,
    pub allow_duplicates: bool,
    pub record_source_app: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSettings {
    pub default_group_id: Option<i64>,
    pub prompt_for_name: bool,
    pub show_grouped_in_main: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyBufferSettings {
    pub play_sound: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSettings {
    pub data_root: String,
    pub portable: bool,
    pub migration_completed: bool,
    pub migration_in_progress: bool,
    pub database_bytes: u64,
    pub last_backup_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsEnvelope {
    pub values: SettingsState,
    pub revision: u64,
    pub restart_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSummary {
    pub id: i64,
    pub kind: String,
    pub preview_text: String,
    pub source_app: String,
    pub created_at_ms: i64,
    pub last_pasted_at_ms: Option<i64>,
    pub pinned: bool,
    pub group_id: Option<i64>,
    pub copy_buffer: Option<u8>,
    pub formats: Vec<String>,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDetail {
    #[serde(flatten)]
    pub summary: ClipSummary,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPage {
    pub items: Vec<ClipSummary>,
    pub total: usize,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyBuffer {
    pub slot: u8,
    pub name: String,
    pub enabled: bool,
    pub clip_id: Option<i64>,
    pub clip_preview: Option<String>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationResult {
    pub ok: bool,
    pub affected: usize,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub ok: bool,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStatus {
    pub completed: bool,
    pub in_progress: bool,
    pub phase: String,
    pub processed: u64,
    pub total: u64,
    pub source_path: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationRun {
    pub status: String,
    pub dry_run: bool,
    pub source_path: String,
    pub clips: u64,
    pub groups: u64,
    pub formats: u64,
    pub copy_buffers: u64,
    pub pinned: u64,
    pub warnings: Vec<String>,
    pub backup_paths: Vec<String>,
    pub restart_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub build_sha: String,
    pub protocol_version: String,
    pub schema_version: String,
    pub source_url: String,
    pub license: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsInfo {
    pub os: String,
    pub architecture: String,
    pub core_pid: u32,
    pub webview_pid_count: usize,
    pub database_bytes: u64,
    pub log_path: String,
    pub settings_revision: u64,
    pub content_included: bool,
}

// ---------------------------------------------------------------------------
// Settings mapping
// ---------------------------------------------------------------------------

fn value_at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(root, |value, key| value.get(key))
}

fn value_bool(root: &Value, path: &str, default: bool) -> bool {
    value_at(root, path)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn value_u64(root: &Value, path: &str, default: u64) -> u64 {
    value_at(root, path)
        .and_then(Value::as_u64)
        .unwrap_or(default)
}

fn value_i64(root: &Value, path: &str) -> Option<i64> {
    value_at(root, path).and_then(Value::as_i64)
}

fn value_string(root: &Value, path: &str, default: &str) -> String {
    value_at(root, path)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn value_strings(root: &Value, path: &str) -> Option<Vec<String>> {
    value_at(root, path).and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect()
    })
}

fn ui_language(stored: &str) -> &'static str {
    if stored.starts_with("zh") {
        "zh-CN"
    } else {
        "en-US"
    }
}

fn stored_language(ui: &str) -> &'static str {
    if ui.starts_with("zh") { "zh-CN" } else { "en" }
}

fn default_first_ten_chords() -> Vec<String> {
    (1..=10)
        .map(|index| format!("Ctrl+{}", if index == 10 { 0 } else { index }))
        .collect()
}

fn default_copy_buffer_hotkeys() -> Vec<CopyBufferHotkeys> {
    (1..=COPY_BUFFER_SLOTS)
        .map(|slot| CopyBufferHotkeys {
            slot,
            copy: format!("Ctrl+Alt+{slot}"),
            paste: format!("Alt+{slot}"),
            cut: String::new(),
        })
        .collect()
}

fn hotkeys_from(values: &Value) -> HotkeySettings {
    let first_ten_chords = value_strings(values, "hotkeys.first_ten_chords")
        .filter(|chords| !chords.is_empty())
        .unwrap_or_else(default_first_ten_chords);
    let copy_buffers = value_at(values, "hotkeys.copy_buffers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(index, item)| CopyBufferHotkeys {
                    slot: u8::try_from(index + 1).unwrap_or(1),
                    copy: item
                        .get("copy")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    paste: item
                        .get("paste")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    cut: item
                        .get("cut")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(default_copy_buffer_hotkeys);
    HotkeySettings {
        open: value_string(values, "hotkeys.open_chord", "Ctrl+`"),
        open_alt: value_string(values, "hotkeys.open_alt_chord", ""),
        plain_text_paste: value_string(values, "hotkeys.plain_text_paste_chord", "Ctrl+Shift+V"),
        first_ten_enabled: value_bool(values, "hotkeys.first_ten_enabled", true),
        first_ten_chords,
        copy_buffers,
    }
}

fn settings_state(state: &AppState) -> Result<SettingsState, CommandError> {
    let snapshot = state.storage().get_settings()?;
    let values = &snapshot.values;
    let data_root = state.data_root();
    let last_backup_ms = value_i64(values, "data.last_backup_ms");
    Ok(SettingsState {
        general: GeneralSettings {
            run_at_startup: value_bool(values, "general.run_at_startup", false),
            start_minimized: value_bool(values, "general.start_minimized", true),
            show_tray_icon: value_bool(values, "general.show_tray_icon", true),
            single_instance: value_bool(values, "general.single_instance", true),
            language: ui_language(&value_string(values, "general.language", "en")).to_string(),
            startup_message: value_string(values, "general.startup_message", ""),
            check_for_updates: value_bool(values, "general.check_for_updates", false),
            log_level: value_string(values, "diagnostics.log_level", "info"),
        },
        appearance: AppearanceSettings {
            theme: value_string(values, "appearance.theme", "system"),
            density: value_string(values, "appearance.density", "comfortable"),
            popup_width: u32::try_from(value_u64(values, "appearance.popup_width", 420))
                .unwrap_or(420),
            popup_height: u32::try_from(value_u64(values, "appearance.popup_height", 480))
                .unwrap_or(480),
            lines_per_clip: u8::try_from(value_u64(values, "appearance.lines_per_clip", 1))
                .unwrap_or(1),
            preview: value_string(values, "appearance.preview", "auto"),
            animations: value_bool(values, "appearance.animations", true),
            always_on_top: value_bool(values, "appearance.always_on_top", true),
            font_scale: u16::try_from(value_u64(values, "appearance.font_scale", 100))
                .unwrap_or(100),
        },
        hotkeys: hotkeys_from(values),
        clipboard: ClipboardSettings {
            max_entries: u32::try_from(value_u64(values, "clipboard.max_entries", 500))
                .unwrap_or(500),
            expire_days: u32::try_from(value_u64(values, "clipboard.expire_days", 5)).unwrap_or(5),
            ignore_formats: value_strings(values, "clipboard.ignored_formats").unwrap_or_default(),
            max_clip_bytes: value_u64(values, "clipboard.max_clip_bytes", 67_108_864),
            capture_files: value_bool(values, "clipboard.capture_files", true),
            capture_images: value_bool(values, "clipboard.capture_images", true),
            allow_duplicates: value_bool(values, "clipboard.allow_duplicates", false),
            record_source_app: value_bool(values, "clipboard.record_source_app", true),
        },
        groups: GroupSettings {
            default_group_id: value_i64(values, "groups.default_group_id"),
            prompt_for_name: value_bool(values, "groups.prompt_for_name", true),
            show_grouped_in_main: value_bool(values, "groups.show_grouped_in_main", true),
        },
        copy_buffers: CopyBufferSettings {
            play_sound: value_bool(values, "copy_buffers.play_sound", false),
        },
        data: DataSettings {
            data_root: data_root.path.display().to_string(),
            portable: data_root.portable,
            migration_completed: value_bool(values, "migration.completed", false),
            migration_in_progress: pending::migration_marker(&data_root.path).is_file(),
            database_bytes: state.storage().database_size_bytes().unwrap_or_default(),
            last_backup_at: last_backup_ms.map(iso8601_from_ms),
        },
    })
}

fn settings_value(settings: &SettingsState) -> Value {
    let copy_buffer_hotkeys = settings
        .hotkeys
        .copy_buffers
        .iter()
        .map(|hotkey| {
            json!({
                "slot": hotkey.slot,
                "copy": hotkey.copy,
                "paste": hotkey.paste,
                "cut": hotkey.cut,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "general": {
            "language": stored_language(&settings.general.language),
            "run_at_startup": settings.general.run_at_startup,
            "start_minimized": settings.general.start_minimized,
            "show_tray_icon": settings.general.show_tray_icon,
            "single_instance": settings.general.single_instance,
            "startup_message": settings.general.startup_message,
            "check_for_updates": settings.general.check_for_updates,
        },
        "appearance": {
            "theme": settings.appearance.theme,
            "density": settings.appearance.density,
            "popup_width": settings.appearance.popup_width,
            "popup_height": settings.appearance.popup_height,
            "lines_per_clip": settings.appearance.lines_per_clip,
            "preview": settings.appearance.preview,
            "animations": settings.appearance.animations,
            "always_on_top": settings.appearance.always_on_top,
            "font_scale": settings.appearance.font_scale,
        },
        "hotkeys": {
            "open_chord": settings.hotkeys.open,
            "open_alt_chord": settings.hotkeys.open_alt,
            "plain_text_paste_chord": settings.hotkeys.plain_text_paste,
            "first_ten_enabled": settings.hotkeys.first_ten_enabled,
            "first_ten_chords": settings.hotkeys.first_ten_chords,
            "copy_buffers": copy_buffer_hotkeys,
        },
        "clipboard": {
            "max_entries": settings.clipboard.max_entries,
            "expire_days": settings.clipboard.expire_days,
            "ignored_formats": settings.clipboard.ignore_formats,
            "max_clip_bytes": settings.clipboard.max_clip_bytes,
            "capture_files": settings.clipboard.capture_files,
            "capture_images": settings.clipboard.capture_images,
            "allow_duplicates": settings.clipboard.allow_duplicates,
            "record_source_app": settings.clipboard.record_source_app,
        },
        "groups": {
            "default_group_id": settings.groups.default_group_id,
            "prompt_for_name": settings.groups.prompt_for_name,
            "show_grouped_in_main": settings.groups.show_grouped_in_main,
        },
        "copy_buffers": {
            "play_sound": settings.copy_buffers.play_sound,
        },
        "diagnostics": {
            "log_level": settings.general.log_level,
        },
    })
}

fn iso8601_from_ms(ms: i64) -> String {
    let seconds = ms.div_euclid(1_000);
    let millis = ms.rem_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's days-to-civil conversion.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Clip mapping and helpers
// ---------------------------------------------------------------------------

fn clip_summary(summary: ditto_model::ClipSummary) -> ClipSummary {
    ClipSummary {
        id: summary.id,
        kind: summary.kind.as_db_str().to_string(),
        preview_text: summary.preview_text,
        source_app: summary.source_app.unwrap_or_default(),
        created_at_ms: summary.created_at_ms,
        last_pasted_at_ms: summary.last_pasted_at_ms,
        pinned: summary.pinned,
        group_id: summary.group_id,
        copy_buffer: summary.copy_buffer,
        formats: summary.formats,
        size_bytes: summary.size_bytes,
    }
}

fn clip_detail(detail: ditto_model::ClipDetail) -> ClipDetail {
    let text = detail.text_content.clone().unwrap_or_default();
    ClipDetail {
        summary: clip_summary(detail.summary),
        text,
    }
}

fn build_query(
    query: &str,
    group_id: Option<i64>,
    kind: Option<&str>,
    pinned_only: bool,
    limit: u32,
) -> Result<ClipQuery, CommandError> {
    let kind = match kind.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(
            ClipKind::from_db_str(value)
                .ok_or_else(|| CommandError::invalid(format!("unknown clip kind {value}")))?,
        ),
        None => None,
    };
    Ok(ClipQuery {
        query: query.to_string(),
        group_id,
        kind,
        pinned_only,
        mode: SearchMode::Contains,
        sort: ClipSort::Recent,
        limit,
        cursor: None,
        include_preview: true,
    })
}

fn page_clips(
    storage: &Storage,
    query: &ClipQuery,
    page: u32,
    page_size: u32,
) -> Result<ClipPage, CommandError> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);
    let total = storage.count_clips(query)?;
    let mut request = query.clone();
    request.limit = page_size;
    for _ in 1..page {
        let previous = storage.search_clips(&request)?;
        match previous.next_cursor {
            Some(cursor) => request.cursor = Some(cursor),
            None => {
                return Ok(ClipPage {
                    items: Vec::new(),
                    total: usize::try_from(total).unwrap_or(usize::MAX),
                    page,
                    page_size,
                });
            }
        }
    }
    let result = storage.search_clips(&request)?;
    Ok(ClipPage {
        items: result.items.into_iter().map(clip_summary).collect(),
        total: usize::try_from(total).unwrap_or(usize::MAX),
        page,
        page_size,
    })
}

fn group_node(storage: &Storage, group: ditto_model::GroupNode) -> Result<GroupNode, CommandError> {
    let count = storage.count_clips(&ClipQuery {
        query: String::new(),
        group_id: Some(group.id),
        kind: None,
        pinned_only: false,
        mode: SearchMode::Contains,
        sort: ClipSort::Recent,
        limit: 1,
        cursor: None,
        include_preview: false,
    })?;
    Ok(GroupNode {
        id: group.id,
        name: group.name,
        parent_id: group.parent_id,
        count: usize::try_from(count).unwrap_or(usize::MAX),
    })
}

fn copy_buffers(storage: &Storage) -> Result<Vec<CopyBuffer>, CommandError> {
    let slots = storage.copy_buffers()?;
    let enabled = storage
        .get_settings()
        .ok()
        .and_then(|snapshot| {
            value_at(&snapshot.values, "copy_buffers.enabled")
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default();
    let mut buffers = Vec::with_capacity(usize::from(COPY_BUFFER_SLOTS));
    for slot in 1..=COPY_BUFFER_SLOTS {
        let row = slots.iter().find(|row| row.slot == slot);
        let clip_id = row.and_then(|row| row.clip_id);
        let clip_preview = clip_id
            .and_then(|id| storage.get_summary(id).ok())
            .map(|summary| summary.preview_text);
        buffers.push(CopyBuffer {
            slot,
            name: row.map(|row| row.name.clone()).unwrap_or_default(),
            enabled: enabled
                .get(usize::from(slot - 1))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            clip_id,
            clip_preview,
            updated_at_ms: None,
        });
    }
    Ok(buffers)
}

fn store_copy_buffer_enabled(
    storage: &Storage,
    slot: u8,
    enabled: bool,
) -> Result<(), CommandError> {
    let snapshot = storage.get_settings()?;
    let mut flags = value_at(&snapshot.values, "copy_buffers.enabled")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    while flags.len() < usize::from(COPY_BUFFER_SLOTS) {
        flags.push(Value::Bool(true));
    }
    let index = usize::from(slot - 1);
    flags[index] = Value::Bool(enabled);
    storage.set_setting("copy_buffers.enabled", Value::Array(flags))?;
    Ok(())
}

fn broadcast_clip_updated(state: &AppState, clip_id: i64, field: &str) {
    let state_rev = state.storage().state_rev().unwrap_or_default();
    state.broadcast(&Event::ClipUpdated {
        id: clip_id,
        fields: vec![field.to_string()],
        state_rev,
    });
}

/// Resolves the system Explorer with an absolute path. Never lets Windows'
/// executable search order (which includes the current directory) pick a
/// `explorer.exe` planted next to the app.
pub(crate) fn explorer_executable() -> PathBuf {
    std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("explorer.exe")
}

fn open_folder(path: &Path) -> Result<OperationResult, CommandError> {
    if !path.is_dir() {
        return Err(CommandError::not_found(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    Command::new(explorer_executable())
        .arg(path)
        .spawn()
        .map_err(|error| CommandError::internal(format!("failed to open folder: {error}")))?;
    Ok(OperationResult {
        ok: true,
        message: "opened".to_string(),
        path: Some(path.display().to_string()),
    })
}

// ---------------------------------------------------------------------------
// Settings commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsEnvelope, CommandError> {
    let snapshot: SettingsSnapshot = state.storage().get_settings()?;
    let mut values = settings_state(&state)?;
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        values.general.run_at_startup = enabled;
    }
    Ok(SettingsEnvelope {
        values,
        revision: snapshot.revision,
        restart_required: false,
    })
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: SettingsState,
) -> Result<SettingsEnvelope, CommandError> {
    if !matches!(settings.general.language.as_str(), "en-US" | "zh-CN") {
        return Err(CommandError::invalid("unsupported language"));
    }
    let previous = settings_state(&state)?;
    let restart_required = previous.general.language != settings.general.language
        || previous.general.run_at_startup != settings.general.run_at_startup
        || previous.general.single_instance != settings.general.single_instance;
    let previous_value = settings_value(&previous);
    let value = settings_value(&settings);
    // Validate the chord before touching storage so an invalid value does not
    // require a rollback.
    if previous.hotkeys.open != settings.hotkeys.open {
        crate::hotkeys::parse_chord(&settings.hotkeys.open).map_err(CommandError::invalid)?;
    }
    let keys = flatten_keys("", &value);
    let snapshot = state.storage().set_settings(&value)?;
    if let Err(error) = crate::apply_host_settings(&app, &previous_value, &value) {
        // Registration or monitor reconfiguration failed: restore the old
        // settings atomically and report the conflict to the UI.
        if let Err(restore_error) = state.storage().set_settings(&previous_value) {
            log::error!("failed to roll back settings after error: {restore_error}");
        }
        let _ = crate::apply_host_settings(&app, &value, &previous_value);
        return Err(CommandError::conflict(error));
    }
    state.broadcast(&Event::SettingsChanged {
        keys,
        revision: snapshot.revision,
    });
    if previous.general.run_at_startup != settings.general.run_at_startup {
        let manager = app.autolaunch();
        let result = if settings.general.run_at_startup {
            manager.enable()
        } else {
            manager.disable()
        };
        if let Err(error) = result {
            log::warn!("failed to update autostart registration: {error}");
        }
    }
    let mut values = settings_state(&state)?;
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        values.general.run_at_startup = enabled;
    }
    Ok(SettingsEnvelope {
        values,
        revision: snapshot.revision,
        restart_required,
    })
}

fn flatten_keys(prefix: &str, value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(object) = value.as_object() {
        for (key, child) in object {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            keys.extend(flatten_keys(&path, child));
        }
    } else if !prefix.is_empty() {
        keys.push(prefix.to_string());
    }
    keys
}

// ---------------------------------------------------------------------------
// Clip commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn list_clips(
    state: State<'_, AppState>,
    page: u32,
    page_size: u32,
    query: Option<String>,
    group_id: Option<i64>,
    kind: Option<String>,
    pinned_only: bool,
) -> Result<ClipPage, CommandError> {
    let query = build_query(
        query.as_deref().unwrap_or_default(),
        group_id,
        kind.as_deref(),
        pinned_only,
        500,
    )?;
    page_clips(state.storage(), &query, page, page_size)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn search_clips(
    state: State<'_, AppState>,
    query: String,
    page: u32,
    page_size: u32,
    group_id: Option<i64>,
    kind: Option<String>,
    pinned_only: bool,
) -> Result<ClipPage, CommandError> {
    let query = build_query(&query, group_id, kind.as_deref(), pinned_only, 500)?;
    page_clips(state.storage(), &query, page, page_size)
}

#[tauri::command]
pub fn get_clip(state: State<'_, AppState>, id: i64) -> Result<ClipDetail, CommandError> {
    Ok(clip_detail(state.storage().get_clip(id)?))
}

#[tauri::command]
pub fn delete_clips(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<MutationResult, CommandError> {
    let affected = state.storage().soft_delete_clips(&ids)?;
    if affected > 0 {
        let state_rev = state.storage().state_rev().unwrap_or_default();
        state.broadcast(&Event::ClipDeleted {
            ids: ids.clone(),
            state_rev,
        });
    }
    Ok(MutationResult {
        ok: true,
        affected: usize::try_from(affected).unwrap_or(usize::MAX),
        message: "deleted".to_string(),
    })
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<MutationResult, CommandError> {
    let affected = state.storage().clear_history()?;
    // Soft delete first, then hard-delete the just-cleared rows and GC the
    // blobs they referenced. Otherwise "clear history" would leave the
    // database and data root growing forever.
    let removed = state.storage().purge_deleted_before(now_ms())?;
    state.broadcast(&Event::ClipDeleted {
        ids: Vec::new(),
        state_rev: state.storage().state_rev().unwrap_or_default(),
    });
    Ok(MutationResult {
        ok: true,
        affected: usize::try_from(affected).unwrap_or(usize::MAX),
        message: format!("cleared {removed} clips and purged deleted blobs"),
    })
}

#[tauri::command]
pub fn set_pinned(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    pinned: bool,
) -> Result<MutationResult, CommandError> {
    let mut affected = 0_usize;
    for id in ids {
        state.storage().set_pinned(id, pinned)?;
        broadcast_clip_updated(&state, id, "pinned");
        affected += 1;
    }
    Ok(MutationResult {
        ok: true,
        affected,
        message: "updated".to_string(),
    })
}

#[tauri::command]
pub fn move_to_group(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    group_id: Option<i64>,
) -> Result<MutationResult, CommandError> {
    let moved = state.storage().move_clips_to_group(&ids, group_id)?;
    for id in &ids {
        broadcast_clip_updated(&state, *id, "group_id");
    }
    Ok(MutationResult {
        ok: true,
        affected: usize::try_from(moved).unwrap_or(usize::MAX),
        message: "moved".to_string(),
    })
}

#[tauri::command]
pub fn edit_clip(
    state: State<'_, AppState>,
    id: i64,
    text: String,
) -> Result<MutationResult, CommandError> {
    // A full payload rewrite: text formats, hashes, preview, size, and FTS are
    // updated together so Original paste and dedupe match the visible text.
    state.storage().rewrite_clip_text(id, &text)?;
    broadcast_clip_updated(&state, id, "text_content");
    Ok(MutationResult {
        ok: true,
        affected: 1,
        message: "updated".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Group commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_groups(state: State<'_, AppState>) -> Result<Vec<GroupNode>, CommandError> {
    state
        .storage()
        .list_groups()?
        .into_iter()
        .map(|group| group_node(state.storage(), group))
        .collect()
}

#[tauri::command]
pub fn create_group(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<i64>,
) -> Result<GroupNode, CommandError> {
    if name.trim().is_empty() {
        return Err(CommandError::invalid("group name is empty"));
    }
    if let Some(parent) = parent_id
        && !state
            .storage()
            .list_groups()?
            .iter()
            .any(|group| group.id == parent)
    {
        return Err(CommandError::not_found("parent group not found"));
    }
    let sort_order = state
        .storage()
        .list_groups()?
        .iter()
        .map(|group| group.sort_order)
        .fold(0.0_f64, f64::max)
        + 1.0;
    let id = state
        .storage()
        .create_group(name.trim(), parent_id, sort_order)?;
    let group = state
        .storage()
        .list_groups()?
        .into_iter()
        .find(|group| group.id == id)
        .ok_or_else(|| CommandError::internal("created group disappeared"))?;
    group_node(state.storage(), group)
}

#[tauri::command]
pub fn rename_group(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<GroupNode, CommandError> {
    state.storage().rename_group(id, &name)?;
    let group = state
        .storage()
        .list_groups()?
        .into_iter()
        .find(|group| group.id == id)
        .ok_or_else(|| CommandError::not_found("group not found"))?;
    group_node(state.storage(), group)
}

#[tauri::command]
pub fn delete_group(state: State<'_, AppState>, id: i64) -> Result<MutationResult, CommandError> {
    let groups = state.storage().list_groups()?;
    let mut removed = vec![id];
    loop {
        let mut added = Vec::new();
        for group in &groups {
            if group
                .parent_id
                .is_some_and(|parent| removed.contains(&parent))
                && !removed.contains(&group.id)
            {
                added.push(group.id);
            }
        }
        if added.is_empty() {
            break;
        }
        removed.extend(added);
    }
    state.storage().delete_group(id)?;
    state.broadcast(&Event::GroupChanged {
        state_rev: state.storage().state_rev().unwrap_or_default(),
    });
    Ok(MutationResult {
        ok: true,
        affected: removed.len(),
        message: "deleted".to_string(),
    })
}

#[tauri::command]
pub fn move_group(
    state: State<'_, AppState>,
    id: i64,
    parent_id: Option<i64>,
) -> Result<MutationResult, CommandError> {
    state.storage().move_group(id, parent_id, 0.0)?;
    state.broadcast(&Event::GroupChanged {
        state_rev: state.storage().state_rev().unwrap_or_default(),
    });
    Ok(MutationResult {
        ok: true,
        affected: 1,
        message: "moved".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Copy buffers and paste
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_copy_buffers(state: State<'_, AppState>) -> Result<Vec<CopyBuffer>, CommandError> {
    copy_buffers(state.storage())
}

#[tauri::command]
pub fn set_copy_buffer(
    state: State<'_, AppState>,
    slot: u8,
    name: Option<String>,
    enabled: Option<bool>,
    clip_id: Option<i64>,
) -> Result<CopyBuffer, CommandError> {
    if !(1..=COPY_BUFFER_SLOTS).contains(&slot) {
        return Err(CommandError::invalid("copy buffer slot must be 1..=5"));
    }
    if let Some(clip_id) = clip_id {
        state.storage().get_summary(clip_id)?;
    }
    state.storage().assign_copy_buffer(slot, clip_id)?;
    if let Some(name) = name {
        state.storage().set_copy_buffer_name(slot, &name)?;
    }
    if let Some(enabled) = enabled {
        store_copy_buffer_enabled(state.storage(), slot, enabled)?;
    }
    if let Some(clip_id) = clip_id {
        broadcast_clip_updated(&state, clip_id, "copy_buffer");
    }
    copy_buffers(state.storage())?
        .into_iter()
        .find(|buffer| buffer.slot == slot)
        .ok_or_else(|| CommandError::internal("copy buffer row disappeared"))
}

#[tauri::command]
pub fn clear_copy_buffer(
    state: State<'_, AppState>,
    slot: u8,
) -> Result<MutationResult, CommandError> {
    if !(1..=COPY_BUFFER_SLOTS).contains(&slot) {
        return Err(CommandError::invalid("copy buffer slot must be 1..=5"));
    }
    state.storage().clear_copy_buffer(slot)?;
    Ok(MutationResult {
        ok: true,
        affected: 1,
        message: "cleared".to_string(),
    })
}

#[tauri::command]
pub fn paste_clip(
    state: State<'_, AppState>,
    clip_id: i64,
    plain_text: bool,
    formats: Option<Vec<String>>,
    clip_ids: Option<Vec<i64>>,
) -> Result<PasteOutcome, CommandError> {
    let core = state.core.lock();
    let core = core
        .as_ref()
        .ok_or_else(|| CommandError::internal("core service is stopping"))?;
    core.paste(
        clip_id,
        plain_text,
        formats.as_deref().unwrap_or_default(),
        clip_ids.as_deref().unwrap_or_default(),
    )
    .map_err(|message| CommandError::new("DITTO_E_PASTE", message, false))
}

#[tauri::command]
pub fn paste_copy_buffer(
    state: State<'_, AppState>,
    slot: u8,
    plain_text: bool,
) -> Result<PasteOutcome, CommandError> {
    if !(1..=COPY_BUFFER_SLOTS).contains(&slot) {
        return Err(CommandError::invalid("copy buffer slot must be 1..=5"));
    }
    let clip_id = state
        .storage()
        .copy_buffers()?
        .iter()
        .find(|buffer| buffer.slot == slot)
        .and_then(|buffer| buffer.clip_id)
        .ok_or_else(|| CommandError::not_found(format!("copy buffer {slot} is empty")))?;
    let core = state.core.lock();
    let core = core
        .as_ref()
        .ok_or_else(|| CommandError::internal("core service is stopping"))?;
    core.paste(clip_id, plain_text, &[], &[])
        .map_err(|message| CommandError::new("DITTO_E_PASTE", message, false))
}

/// Starts a native OLE drag-out for a clip, using the calling settings window
/// as the drag source. Documented in REVIEW-GUIDE for T8.
#[tauri::command]
pub fn drag_out_clip(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    clip_id: i64,
) -> Result<OperationResult, CommandError> {
    let owner_hwnd = window
        .hwnd()
        .map(|handle| handle.0 as isize)
        .unwrap_or_default();
    let core = state.core.lock();
    let core = core
        .as_ref()
        .ok_or_else(|| CommandError::internal("core service is stopping"))?;
    let dropped = core
        .drag_out(clip_id, owner_hwnd)
        .map_err(|message| CommandError::new("DITTO_E_DRAG", message, false))?;
    Ok(OperationResult {
        ok: dropped,
        message: if dropped {
            "drag completed".to_string()
        } else {
            "drag cancelled".to_string()
        },
        path: None,
    })
}

// ---------------------------------------------------------------------------
// Data operations
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn backup_db(
    state: State<'_, AppState>,
    destination: Option<String>,
) -> Result<OperationResult, CommandError> {
    let data_root = &state.data_root().path;
    let backup_dir = destination
        .map(PathBuf::from)
        .unwrap_or_else(|| data_root.join("backups"));
    let report = ditto_migration::backup(data_root, &backup_dir, true)?;
    state
        .storage()
        .set_setting("data.last_backup_ms", json!(now_ms()))?;
    Ok(OperationResult {
        ok: true,
        message: format!(
            "backup created: {} clips, {} groups",
            report.counts.clips, report.counts.groups
        ),
        path: Some(report.package_path),
    })
}

#[tauri::command]
pub fn export_data(
    state: State<'_, AppState>,
    destination: Option<String>,
) -> Result<OperationResult, CommandError> {
    let data_root = &state.data_root().path;
    let destination = destination.map(PathBuf::from).unwrap_or_else(|| {
        data_root
            .join("exports")
            .join(format!("ditto-export-{}.zip", now_ms()))
    });
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = ditto_migration::create_export(
        data_root,
        &destination,
        &ditto_migration::ExportOptions {
            zip: true,
            replace_existing: true,
        },
    )?;
    Ok(OperationResult {
        ok: true,
        message: format!(
            "export created: {} clips, {} files verified",
            report.counts.clips, report.files_verified
        ),
        path: Some(report.package_path),
    })
}

#[tauri::command]
pub fn restore_db(
    state: State<'_, AppState>,
    source: Option<String>,
) -> Result<OperationResult, CommandError> {
    let package = match source {
        Some(source) => PathBuf::from(source),
        None => newest_backup(&state)?,
    };
    stage_package(&state, &package, "restore")
}

#[tauri::command]
pub fn import_data(
    state: State<'_, AppState>,
    source: Option<String>,
) -> Result<OperationResult, CommandError> {
    let package = match source {
        Some(source) => PathBuf::from(source),
        None => {
            let imports = state.data_root().path.join("imports");
            newest_package(&imports).ok_or_else(|| {
                CommandError::not_found(
                    "no package to import; place a Ditto package in the data root imports folder",
                )
            })?
        }
    };
    stage_package(&state, &package, "import")
}

fn stage_package(
    state: &AppState,
    package: &Path,
    operation: &str,
) -> Result<OperationResult, CommandError> {
    if !package.is_file() {
        return Err(CommandError::not_found(format!(
            "{} is not a package file",
            package.display()
        )));
    }
    let report = ditto_migration::verify_package(package)?;
    if !report.verified {
        return Err(CommandError::conflict("package verification failed"));
    }
    let data_root = &state.data_root().path;
    let staged = pending::import_package_path(data_root);
    if package != staged {
        std::fs::copy(package, &staged)?;
    }
    let pending = pending::PendingImport {
        package: staged.display().to_string(),
        requested_at_ms: now_ms(),
    };
    pending::write_marker(
        &pending::import_marker(data_root),
        &serde_json::to_vec_pretty(&pending)?,
    )?;
    Ok(OperationResult {
        ok: true,
        message: format!(
            "{operation} staged: {} clips verified; restart Ditto to apply",
            report.counts.clips
        ),
        path: Some(staged.display().to_string()),
    })
}

fn newest_backup(state: &AppState) -> Result<PathBuf, CommandError> {
    let data_root = &state.data_root().path;
    newest_package(&data_root.join("backups")).ok_or_else(|| {
        CommandError::not_found("no backup package found in the data root backups folder")
    })
}

fn newest_package(directory: &Path) -> Option<PathBuf> {
    let mut candidates = std::fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    candidates.pop()
}

#[tauri::command]
pub fn optimize_storage(state: State<'_, AppState>) -> Result<OperationResult, CommandError> {
    state.storage().optimize()?;
    let removed = state.storage().gc_orphan_blobs()?;
    let bytes = state.storage().database_size_bytes().unwrap_or_default();
    Ok(OperationResult {
        ok: true,
        message: format!("optimized: {bytes} bytes in use, {removed} orphan files removed"),
        path: None,
    })
}

#[tauri::command]
pub fn get_migration_status(state: State<'_, AppState>) -> Result<MigrationStatus, CommandError> {
    let data_root = &state.data_root().path;
    if let Some(failed) = pending::failed_marker_description(data_root) {
        return Ok(MigrationStatus {
            completed: false,
            in_progress: false,
            phase: "failed".to_string(),
            processed: 0,
            total: 0,
            source_path: None,
            started_at: None,
            completed_at: None,
            warnings: vec![format!(
                "a staged import or migration failed; details: {failed}"
            )],
        });
    }
    if pending::migration_marker(data_root).is_file() {
        let (total, warnings) = std::fs::read_to_string(pending::migration_marker(data_root))
            .ok()
            .and_then(|text| serde_json::from_str::<pending::PendingMigration>(&text).ok())
            .map(|pending| {
                (
                    pending.clips,
                    vec![
                        "legacy migration is staged and will run on the next start"
                            .to_string(),
                        "v1 migration is one-shot: progress is shown after restart and cannot be cancelled"
                            .to_string(),
                    ],
                )
            })
            .unwrap_or_else(|| (0, vec!["legacy migration is staged".to_string()]));
        return Ok(MigrationStatus {
            completed: false,
            in_progress: true,
            phase: "pending-restart".to_string(),
            processed: 0,
            total,
            source_path: None,
            started_at: None,
            completed_at: None,
            warnings,
        });
    }
    let report_path = data_root.join("migration-report.json");
    if let Ok(bytes) = std::fs::read(&report_path) {
        let report: ditto_migration::MigrationReport = serde_json::from_slice(&bytes)?;
        return Ok(MigrationStatus {
            completed: report.status == "success" && !report.dry_run,
            in_progress: report.status == "running",
            phase: report.status,
            processed: report.imported_counts.clips,
            total: report.source_counts.clips,
            source_path: Some(report.source_path),
            started_at: (report.started_at_ms > 0).then(|| iso8601_from_ms(report.started_at_ms)),
            completed_at: (report.started_at_ms > 0)
                .then(|| iso8601_from_ms(report.started_at_ms + report.duration_ms as i64)),
            warnings: report.warnings,
        });
    }
    let snapshot = state.storage().get_settings()?;
    let completed = value_bool(&snapshot.values, "migration.completed", false);
    Ok(MigrationStatus {
        completed,
        in_progress: false,
        phase: if completed { "complete" } else { "not-started" }.to_string(),
        processed: 0,
        total: 0,
        source_path: value_at(&snapshot.values, "migration.source_path")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        started_at: None,
        completed_at: None,
        warnings: Vec::new(),
    })
}

#[tauri::command]
pub fn clear_staged_failure(state: State<'_, AppState>) -> Result<OperationResult, CommandError> {
    let cleared = pending::clear_failed(&state.data_root().path);
    Ok(OperationResult {
        ok: true,
        message: if cleared.is_empty() {
            "no failed staged operation to clear".to_string()
        } else {
            format!("cleared {} failed staging marker(s)", cleared.len())
        },
        path: cleared.first().cloned(),
    })
}

#[tauri::command]
pub fn migration_dry_run(
    _state: State<'_, AppState>,
    source: String,
) -> Result<MigrationRun, CommandError> {
    let source = migration_source(&source)?;
    let report = ditto_migration::dry_run(&source)?;
    Ok(migration_run(report, true, false))
}

#[tauri::command]
pub fn migration_apply(
    state: State<'_, AppState>,
    source: String,
) -> Result<MigrationRun, CommandError> {
    let source = migration_source(&source)?;
    let datatabase = source.database.display().to_string();
    let settings = source
        .settings
        .as_ref()
        .map(|path| path.display().to_string());
    // Validate before staging so obvious errors surface immediately.
    let report = ditto_migration::dry_run(&source)?;
    let pending = pending::PendingMigration {
        source: datatabase,
        settings,
        requested_at_ms: now_ms(),
        clips: report.source_counts.clips,
        groups: report.source_counts.groups,
        formats: report.source_counts.formats,
    };
    pending::write_marker(
        &pending::migration_marker(&state.data_root().path),
        &serde_json::to_vec_pretty(&pending)?,
    )?;
    Ok(migration_run(report, false, true))
}

fn migration_source(source: &str) -> Result<MigrationSource, CommandError> {
    let database = PathBuf::from(source);
    if !database.is_file() {
        return Err(CommandError::not_found(format!(
            "legacy database {} was not found",
            database.display()
        )));
    }
    let settings = database
        .parent()
        .map(|parent| parent.join("Ditto.Settings"))
        .filter(|path| path.is_file());
    Ok(MigrationSource { database, settings })
}

fn migration_run(
    report: ditto_migration::MigrationReport,
    dry_run: bool,
    restart_required: bool,
) -> MigrationRun {
    MigrationRun {
        status: report.status,
        dry_run,
        source_path: report.source_path,
        clips: report.source_counts.clips,
        groups: report.source_counts.groups,
        formats: report.source_counts.formats,
        copy_buffers: report.source_counts.copy_buffers,
        pinned: report.imported_counts.pinned,
        warnings: report.warnings,
        backup_paths: report
            .backups
            .into_iter()
            .map(|backup| backup.path)
            .collect(),
        restart_required,
    }
}

// ---------------------------------------------------------------------------
// Diagnostics and application info
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn open_data_folder(state: State<'_, AppState>) -> Result<OperationResult, CommandError> {
    open_folder(&state.data_root().path)
}

#[tauri::command]
pub fn open_log_folder(state: State<'_, AppState>) -> Result<OperationResult, CommandError> {
    let logs = state.data_root().logs_dir();
    std::fs::create_dir_all(&logs)?;
    open_folder(&logs)
}

#[tauri::command]
pub fn get_diagnostics(state: State<'_, AppState>) -> Result<DiagnosticsInfo, CommandError> {
    let snapshot = state.storage().get_settings()?;
    Ok(DiagnosticsInfo {
        os: format!("Windows {}", std::env::consts::OS),
        architecture: std::env::consts::ARCH.to_string(),
        core_pid: std::process::id(),
        webview_pid_count: ditto_win::webview_process_count(),
        database_bytes: state.storage().database_size_bytes().unwrap_or_default(),
        log_path: state.data_root().logs_dir().display().to_string(),
        settings_revision: snapshot.revision,
        content_included: false,
    })
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> Result<AppInfo, CommandError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_sha: option_env!("DITTO_BUILD_SHA").unwrap_or("dev").to_string(),
        protocol_version: PROTOCOL_VERSION.to_string(),
        schema_version: format!("{}.{}", SCHEMA_VERSION, state.storage().user_version()?),
        source_url: "https://github.com/sabrogden/Ditto".to_string(),
        license: "GPL-3.0-only".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_mapping_round_trips_through_json() {
        let value = json!({
            "general": {"language": "zh-CN", "run_at_startup": true},
            "clipboard": {"max_entries": 42, "ignored_formats": ["CF_DIB"]},
            "copy_buffers": {"play_sound": true}
        });
        assert_eq!(ui_language("zh-CN"), "zh-CN");
        assert_eq!(stored_language("en-US"), "en");
        assert!(value_bool(&value, "general.run_at_startup", false));
        assert_eq!(value_u64(&value, "clipboard.max_entries", 0), 42);
        assert_eq!(
            value_strings(&value, "clipboard.ignored_formats"),
            Some(vec!["CF_DIB".to_string()])
        );
        let hotkeys = hotkeys_from(&Value::Null);
        assert_eq!(hotkeys.copy_buffers.len(), 5);
        assert_eq!(hotkeys.first_ten_chords.len(), 10);
    }

    #[test]
    fn explorer_executable_is_absolute_and_never_uses_path_search() {
        let explorer = explorer_executable();
        assert!(explorer.is_absolute(), "explorer path must be absolute");
        assert_eq!(
            explorer.file_name().and_then(|name| name.to_str()),
            Some("explorer.exe")
        );
    }

    #[test]
    fn iso_time_conversion_matches_known_epoch() {
        assert_eq!(iso8601_from_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            iso8601_from_ms(1_762_000_000_000),
            "2025-11-01T12:26:40.000Z"
        );
    }

    #[test]
    fn page_helpers_count_and_slice_real_storage() {
        let temp = tempfile::tempdir().expect("temp root");
        let storage = Storage::open_data_root(temp.path()).expect("storage");
        let query = ClipQuery {
            query: String::new(),
            ..ClipQuery::default()
        };
        assert_eq!(storage.count_clips(&query).expect("count"), 0);
        let page = page_clips(&storage, &query, 1, 50).expect("page");
        assert_eq!(page.total, 0);
        assert_eq!(page.page_size, 50);
    }
}
