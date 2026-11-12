use std::collections::BTreeMap;

use ditto_model::SettingsSnapshot;
use serde_json::{Map, Value};

use crate::{Storage, StorageError, StorageResult, schema::now_ms};

impl Storage {
    pub fn get_settings(&self) -> StorageResult<SettingsSnapshot> {
        let overrides = {
            let connection = self.reader()?;
            let mut statement =
                connection.prepare("SELECT key, value FROM settings ORDER BY key")?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut values = ditto_model::default_settings();
        let mut overrides_object = Map::new();
        for (key, value) in overrides {
            let value: Value = serde_json::from_str(&value)?;
            insert_path(&mut overrides_object, &key, value)?;
        }
        ditto_model::settings::merge_settings(&mut values, &Value::Object(overrides_object))
            .map_err(|error| StorageError::InvalidData(error.to_string()))?;
        Ok(SettingsSnapshot {
            values,
            revision: self.meta_u64("settings_rev")?,
        })
    }

    pub fn get_setting(&self, path: &str) -> StorageResult<Option<Value>> {
        let snapshot = self.get_settings()?;
        Ok(path_value(&snapshot.values, path).cloned())
    }

    pub fn set_settings(&self, values: &Value) -> StorageResult<SettingsSnapshot> {
        let object = values
            .as_object()
            .ok_or_else(|| StorageError::InvalidData("settings must be a JSON object".into()))?;
        let mut flattened = BTreeMap::new();
        flatten_paths("", &Value::Object(object.clone()), &mut flattened)?;

        let now = now_ms();
        let mut connection = self.writer.lock();
        let transaction = connection.transaction()?;
        let mut changed_keys = 0_usize;
        for (path, value) in flattened {
            let encoded = serde_json::to_string(&value)?;
            let changed = transaction.execute(
                "INSERT INTO settings(key, value, updated_at_ms, revision)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at_ms = excluded.updated_at_ms,
                   revision = settings.revision + 1
                 WHERE settings.value <> excluded.value",
                rusqlite::params![path, encoded, now],
            )?;
            if changed > 0 {
                changed_keys += 1;
            }
        }
        if changed_keys > 0 {
            transaction.execute(
                "UPDATE meta
                 SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT), updated_at_ms = ?1
                 WHERE key = 'settings_rev'",
                rusqlite::params![now],
            )?;
        }
        transaction.commit()?;
        self.get_settings()
    }

    pub fn set_setting(&self, path: &str, value: Value) -> StorageResult<SettingsSnapshot> {
        let mut object = Map::new();
        let mut current = &mut object;
        let parts = path.split('.').collect::<Vec<_>>();
        for (index, part) in parts.iter().enumerate() {
            if index + 1 == parts.len() {
                current.insert((*part).to_string(), value.clone());
            } else {
                let next = current
                    .entry((*part).to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                current = next.as_object_mut().ok_or_else(|| {
                    StorageError::Conflict(format!("setting path conflict at {part}"))
                })?;
            }
        }
        self.set_settings(&Value::Object(object))
    }

    pub fn legacy_settings_json(entries: &[(String, String)]) -> StorageResult<Value> {
        let mut output = Map::new();
        for (key, raw) in entries {
            let Some(mapped) = map_legacy_key(key) else {
                continue;
            };
            let Some(value) = convert_legacy_value(&mapped, raw) else {
                continue;
            };
            insert_path(&mut output, &mapped, value)?;
        }
        Ok(Value::Object(output))
    }
}

fn map_legacy_key(key: &str) -> Option<String> {
    if let Some(slot) = key.strip_prefix("CopyBufferCopyHotKey_") {
        if let Ok(index) = slot.parse::<u8>() {
            if (0..5).contains(&index) {
                return Some(format!("hotkeys.copy_buffer.{}.copy", index + 1));
            }
        }
    }
    if let Some(slot) = key.strip_prefix("CopyBufferPasteHotKey_") {
        if let Ok(index) = slot.parse::<u8>() {
            if (0..5).contains(&index) {
                return Some(format!("hotkeys.copy_buffer.{}.paste", index + 1));
            }
        }
    }
    if let Some(slot) = key.strip_prefix("CopyBufferCutHotKey_") {
        if let Ok(index) = slot.parse::<u8>() {
            if (0..5).contains(&index) {
                return Some(format!("hotkeys.copy_buffer.{}.cut", index + 1));
            }
        }
    }
    ditto_model::legacy_setting_to_new(key).map(ToString::to_string)
}

fn convert_legacy_value(path: &str, raw: &str) -> Option<Value> {
    let raw = raw.trim();
    match path {
        "appearance.theme" => {
            let lowered = raw.to_ascii_lowercase();
            let value = if lowered.contains("dark") || lowered.contains("monolight") {
                "dark"
            } else if lowered.is_empty() {
                "system"
            } else {
                "light"
            };
            Some(Value::String(value.to_string()))
        }
        "general.language" => Some(Value::String(
            if raw.eq_ignore_ascii_case("Chinese Simplified") {
                "zh-CN"
            } else {
                "en"
            }
            .to_string(),
        )),
        "appearance.popup_position" => Some(Value::String(
            match raw {
                "2" => "cursor",
                "3" => "previous",
                _ => "caret",
            }
            .to_string(),
        )),
        "search.mode" => Some(Value::String(
            if raw.eq_ignore_ascii_case("true") {
                "contains"
            } else {
                "wildcard"
            }
            .to_string(),
        )),
        "diagnostics.log_level" => Some(Value::String(
            if parse_bool(raw) { "debug" } else { "info" }.to_string(),
        )),
        "clipboard.multi_paste_separator" => Some(Value::String(raw.replace("[LF]", "\r\n"))),
        "clipboard.include_apps" | "clipboard.exclude_apps" => Some(Value::Array(
            raw.split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| Value::String(value.to_string()))
                .collect(),
        )),
        path if path.ends_with("_enabled")
            || path.ends_with("_expire")
            || path.ends_with("_max_entries")
            || path.ends_with("_duplicates")
            || path.ends_with("_on_paste")
            || path.ends_with("_tray_icon")
            || path.ends_with("_show_text")
            || path.ends_with("_send_paste")
            || path.ends_with("_paste_as_admin")
            || path.ends_with("_description")
            || path.ends_with("_fulltext")
            || path.ends_with("_quickpaste")
            || path == "groups.show_grouped_in_main"
            || path == "groups.prompt_for_name" =>
        {
            Some(Value::Bool(parse_bool(raw)))
        }
        path if path.ends_with("_ms")
            || path.ends_with("_count")
            || path.ends_with("_days")
            || path.ends_with(".open") =>
        {
            Some(Value::Number(raw.parse::<i64>().ok()?.into()))
        }
        _ => {
            if let Ok(value) = raw.parse::<i64>() {
                Some(Value::Number(value.into()))
            } else {
                Some(Value::String(raw.to_string()))
            }
        }
    }
}

fn parse_bool(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

fn flatten_paths(
    prefix: &str,
    value: &Value,
    output: &mut BTreeMap<String, Value>,
) -> StorageResult<()> {
    if let Some(object) = value.as_object() {
        for (key, child) in object {
            if key.is_empty()
                || key.contains('.')
                || key
                    .chars()
                    .any(|character| !(character.is_ascii_alphanumeric() || character == '_'))
            {
                return Err(StorageError::InvalidData(format!(
                    "invalid setting key {key:?}"
                )));
            }
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            flatten_paths(&path, child, output)?;
        }
    } else if !prefix.is_empty() {
        output.insert(prefix.to_string(), value.clone());
    }
    Ok(())
}

fn insert_path(object: &mut Map<String, Value>, path: &str, value: Value) -> StorageResult<()> {
    let parts = path.split('.').collect::<Vec<_>>();
    let Some((last, parents)) = parts.split_last() else {
        return Err(StorageError::InvalidData("empty setting path".to_string()));
    };
    let mut current = object;
    for part in parents {
        let next = current
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        current = next
            .as_object_mut()
            .ok_or_else(|| StorageError::Conflict(format!("setting path conflict at {part}")))?;
    }
    current.insert((*last).to_string(), value);
    Ok(())
}

fn path_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |current, part| current.get(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_conversion_handles_rich_types_and_copy_buffers() {
        let value = Storage::legacy_settings_json(&[
            ("MaxEntries".to_string(), "77".to_string()),
            ("LanguageFile".to_string(), "Chinese Simplified".to_string()),
            ("CopyBufferPasteHotKey_4".to_string(), "99".to_string()),
            ("Password".to_string(), "ignored".to_string()),
        ])
        .expect("map legacy settings");
        assert_eq!(value["clipboard"]["max_entries"], 77);
        assert_eq!(value["general"]["language"], "zh-CN");
        assert_eq!(value["hotkeys"]["copy_buffer"]["5"]["paste"], 99);
        assert!(value.get("Password").is_none());
    }
}
