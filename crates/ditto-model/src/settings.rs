//! Stable setting keys, defaults, and legacy key translation.

use serde_json::{Map, Value, json};

pub fn default_settings() -> Value {
    json!({
        "general": {
            "language": "en",
            "run_at_startup": false,
            "start_minimized": true,
            "show_tray_icon": true,
            "default_paste_mode": "normal"
        },
        "appearance": {
            "theme": "system",
            "accent": "system",
            "accent_value": null,
            "density": "comfortable",
            "popup_width": 420,
            "popup_height": 480,
            "popup_position": "caret",
            "lines_per_clip": 1,
            "font_scale": 100,
            "preview": "auto",
            "animations": true,
            "always_on_top": true,
            "hide_on_paste": true,
            "zebra_rows": false,
            "show_grouped_in_main": true
        },
        "hotkeys": {
            "open": 704,
            "open_alt": [],
            "plain_text_paste": 0,
            "first_ten_enabled": true,
            "first_ten_start": 1,
            "first_ten_show_text": true,
            "first_ten_send_paste": true
        },
        "clipboard": {
            "capture_formats": [],
            "max_clip_bytes": 67108864,
            "max_image_bytes": 33554432,
            "max_entries": 500,
            "expire_days": 5,
            "enforce_max_entries": true,
            "enforce_expire": true,
            "allow_duplicates": false,
            "allow_back_to_back_duplicates": false,
            "update_time_on_paste": true,
            "ignore_cf_dib": false,
            "include_apps": [],
            "exclude_apps": [],
            "ignored_formats": [],
            "save_delay_ms": 150,
            "process_delay_ms": 150,
            "paste_delay_ms": 0,
            "send_paste_after_selection": true,
            "restore_clipboard": false,
            "restore_delay_ms": 0,
            "paste_as_admin": true,
            "multi_paste_separator": "\r\n",
            "supported_types": []
        },
        "groups": {
            "show_grouped_in_main": true,
            "prompt_for_name": true,
            "default_group_id": null
        },
        "copy_buffers": {
            "play_sound": false,
            "slot_names": ["", "", "", "", ""]
        },
        "search": {
            "mode": "contains",
            "description": true,
            "fulltext": true,
            "quickpaste": true,
            "regex_case_insensitive": true
        },
        "data": {
            "backup_count": 3,
            "backup_interval_hours": 24,
            "last_backup_ms": null,
            "deleted_retention_days": 7
        },
        "diagnostics": {
            "log_level": "info"
        },
        "migration": {
            "completed": false,
            "source_path": null,
            "source_sha256": null,
            "completed_at_ms": null,
            "dry_run": false
        }
    })
}

pub fn merge_settings(base: &mut Value, overrides: &Value) -> crate::Result<()> {
    let base = base.as_object_mut().ok_or_else(|| {
        crate::DittoError::Configuration("default settings are not an object".into())
    })?;
    let overrides = overrides
        .as_object()
        .ok_or_else(|| crate::DittoError::Configuration("settings must be a JSON object".into()))?;
    merge_object(base, overrides)?;
    Ok(())
}

fn merge_object(
    base: &mut Map<String, Value>,
    overrides: &Map<String, Value>,
) -> crate::Result<()> {
    for (key, value) in overrides {
        if value.is_object() {
            let target = base
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            let target = target.as_object_mut().ok_or_else(|| {
                crate::DittoError::Conflict(format!("setting namespace {key} is not an object"))
            })?;
            merge_object(target, value.as_object().expect("checked object"))?;
        } else {
            base.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

/// Maps the legacy `[Ditto]` key names to stable v2 setting paths.
///
/// The caller parses INI/registry values; this function only owns the mapping.
pub fn legacy_setting_to_new(key: &str) -> Option<&'static str> {
    let key = key.trim();
    Some(match key {
        "DittoHotKey" => "hotkeys.open",
        "DittoHotKey2" | "DittoHotKey3" => "hotkeys.open_alt",
        "Theme" => "appearance.theme",
        "LanguageFile" => "general.language",
        "MaxEntries" => "clipboard.max_entries",
        "ExpiredEntries" => "clipboard.expire_days",
        "CheckForMaxEntries" => "clipboard.enforce_max_entries",
        "CheckForExpiredEntries" => "clipboard.enforce_expire",
        "AllowDuplicates" => "clipboard.allow_duplicates",
        "UpdateTimeOnPaste" => "clipboard.update_time_on_paste",
        "MaxClipSizeInBytes" => "clipboard.max_clip_bytes",
        "SaveClipDelay" => "clipboard.save_delay_ms",
        "ProcessDrawClipboardDelay" => "clipboard.process_delay_ms",
        "PasteAsAdmin" => "clipboard.paste_as_admin",
        "MultiPasteSeparator" => "clipboard.multi_paste_separator",
        "SendPasteAfterSelection" => "clipboard.send_paste_after_selection",
        "RestoreClipboardDelay" | "DittoRestoreClipboardDelay" => "clipboard.restore_delay_ms",
        "HideDittoOnPaste" => "appearance.hide_on_paste",
        "ShowIconInSystemTray" => "general.show_tray_icon",
        "LinesPerRow" => "appearance.lines_per_clip",
        "QuickPastePosition" => "appearance.popup_position",
        "QuickPasteSize" => "appearance.popup_size",
        "UseCtrlNumForFirstTenHotKeys" => "hotkeys.first_ten_enabled",
        "FirstTenHotKeysStart" => "hotkeys.first_ten_start",
        "ShowTextForFirstTenHotKeys" => "hotkeys.first_ten_show_text",
        "SendPasteOnFirstTenHotKeys" => "hotkeys.first_ten_send_paste",
        "SearchDescription" => "search.description",
        "SearchFullText" => "search.fulltext",
        "SearchQuickPaste" => "search.quickpaste",
        "SimpleTextSearch" | "RegExTextSearch" => "search.mode",
        "ShowAllClipsInMainList" | "ShowGroupsInMainList" => "groups.show_grouped_in_main",
        "PromptForNewGroupName" => "groups.prompt_for_name",
        "EnableDebugLogging" => "diagnostics.log_level",
        "IncludeApps" => "clipboard.include_apps",
        "ExcludeApps" => "clipboard.exclude_apps",
        "NoFormatsRetryDelay" => "clipboard.no_formats_retry_ms",
        _ if key.starts_with("CopyBufferCopyHotKey_") => "hotkeys.copy_buffer.copy",
        _ if key.starts_with("CopyBufferPasteHotKey_") => "hotkeys.copy_buffer.paste",
        _ if key.starts_with("CopyBufferCutHotKey_") => "hotkeys.copy_buffer.cut",
        _ if key == "CopyBufferPlaySound_0" => "copy_buffers.play_sound",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_namespaced_and_merge_recursively() {
        let mut value = default_settings();
        merge_settings(
            &mut value,
            &json!({"clipboard": {"max_entries": 42}, "general": {"language": "zh-CN"}}),
        )
        .expect("merge settings");
        assert_eq!(value["clipboard"]["max_entries"], 42);
        assert_eq!(value["clipboard"]["expire_days"], 5);
        assert_eq!(value["general"]["language"], "zh-CN");
    }

    #[test]
    fn legacy_dynamic_copy_buffer_keys_are_mapped() {
        assert_eq!(
            legacy_setting_to_new("CopyBufferPasteHotKey_4"),
            Some("hotkeys.copy_buffer.paste")
        );
        assert_eq!(legacy_setting_to_new("Password"), None);
    }
}
