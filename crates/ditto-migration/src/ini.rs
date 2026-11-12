use std::{collections::BTreeMap, fs, path::Path};

use ditto_storage::Storage;
use serde_json::{Map, Value, json};

use crate::{MigrationError, MigrationResult, source::LegacyCatalog};

pub struct MappedSettings {
    pub values: Value,
    pub unknown: BTreeMap<String, String>,
    pub warnings: Vec<String>,
    pub mapped_count: u64,
}

pub fn read_settings(
    path: Option<&Path>,
) -> MigrationResult<(BTreeMap<String, String>, Vec<String>)> {
    let Some(path) = path else {
        return Ok((BTreeMap::new(), Vec::new()));
    };
    if !path.is_file() {
        return Ok((
            BTreeMap::new(),
            vec![format!("settings file {} was not found", path.display())],
        ));
    }
    let bytes = fs::read(path)?;
    let text = decode_ini(&bytes)?;
    let mut entries = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut section = String::new();
    for (line_number, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!(
                "settings line {} is not key=value; ignored",
                line_number + 1
            ));
            continue;
        };
        if !section.is_empty() && section != "ditto" {
            continue;
        }
        let key = key.trim();
        if !key.is_empty() {
            entries.insert(key.to_string(), value.trim().to_string());
        }
    }
    Ok((entries, warnings))
}

pub fn map_settings(
    path: Option<&Path>,
    catalog: &LegacyCatalog,
) -> MigrationResult<MappedSettings> {
    let (entries, mut warnings) = read_settings(path)?;
    let mut supported_entries = Vec::new();
    let mut unknown = BTreeMap::new();
    for (key, value) in &entries {
        if is_sensitive_key(key) {
            warnings.push(format!("unsupported secret setting {key} was not migrated"));
            continue;
        }
        if is_supported_key(key) && key != "QuickPasteSize" && key != "PlaySoundOnCopy" {
            supported_entries.push((key.clone(), value.clone()));
        } else {
            unknown.insert(key.clone(), value.clone());
        }
    }
    let mut values = Storage::legacy_settings_json(&supported_entries)?;
    let object = values
        .as_object_mut()
        .ok_or_else(|| MigrationError::Invalid("mapped settings are not an object".into()))?;
    if entries.contains_key("QuickPasteSize") {
        if let Some((width, height)) = entries
            .get("QuickPasteSize")
            .and_then(|value| parse_popup_size(value))
        {
            insert_path(object, "appearance.popup_width", json!(width));
            insert_path(object, "appearance.popup_height", json!(height));
        } else {
            warnings.push("QuickPasteSize was malformed; defaults retained".into());
        }
    }
    if entries
        .get("PlaySoundOnCopy")
        .is_some_and(|value| parse_bool(value))
        || entries.iter().any(|(key, value)| {
            key.strip_prefix("CopyBufferPlaySound_")
                .and_then(|slot| slot.parse::<u8>().ok())
                .is_some_and(|slot| slot <= 4 && parse_bool(value))
        })
    {
        insert_path(object, "copy_buffers.play_sound", Value::Bool(true));
    }
    if entries.contains_key("LinesPerRow") {
        if let Some(value) = entries
            .get("LinesPerRow")
            .and_then(|value| value.parse::<i64>().ok())
        {
            insert_path(
                object,
                "appearance.lines_per_clip",
                json!(value.clamp(1, 5)),
            );
        }
    }
    if !catalog.types.is_empty() {
        insert_path(
            object,
            "clipboard.supported_types",
            Value::Array(
                catalog
                    .types
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
        );
    }
    let mapped_count = count_leaf_values(&values);
    Ok(MappedSettings {
        values,
        unknown,
        warnings,
        mapped_count,
    })
}

fn decode_ini(bytes: &[u8]) -> MigrationResult<String> {
    if bytes.starts_with(&[0xff, 0xfe]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        return decode_utf16(&bytes[2..], false);
    }
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8(bytes[3..].to_vec())
            .map_err(|error| MigrationError::Invalid(format!("invalid UTF-8 settings: {error}")));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|error| MigrationError::Invalid(format!("invalid settings encoding: {error}")))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> MigrationResult<String> {
    if bytes.len() % 2 != 0 {
        return Err(MigrationError::Invalid(
            "UTF-16 settings contain an odd byte count".into(),
        ));
    }
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&words)
        .map_err(|error| MigrationError::Invalid(format!("invalid UTF-16 settings: {error}")))
}

fn is_supported_key(key: &str) -> bool {
    ditto_model::legacy_setting_to_new(key).is_some()
        || key == "QuickPasteSize"
        || key == "PlaySoundOnCopy"
        || key == "CopyBufferPlaySound_0"
        || key
            .strip_prefix("CopyBufferPlaySound_")
            .and_then(|slot| slot.parse::<u8>().ok())
            .is_some_and(|slot| slot <= 4)
        || key
            .strip_prefix("CopyBufferCopyHotKey_")
            .and_then(|slot| slot.parse::<u8>().ok())
            .is_some_and(|slot| slot <= 4)
        || key
            .strip_prefix("CopyBufferPasteHotKey_")
            .and_then(|slot| slot.parse::<u8>().ok())
            .is_some_and(|slot| slot <= 4)
        || key
            .strip_prefix("CopyBufferCutHotKey_")
            .and_then(|slot| slot.parse::<u8>().ok())
            .is_some_and(|slot| slot <= 4)
}

fn is_sensitive_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered.contains("password")
        || lowered.contains("secret")
        || lowered.contains("token")
        || lowered.contains("privatekey")
        || lowered.contains("private_key")
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

fn parse_popup_size(value: &str) -> Option<(i64, i64)> {
    let inner = value
        .trim()
        .strip_prefix('(')?
        .strip_suffix(')')?
        .split(',')
        .map(str::trim)
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if inner.len() != 2 {
        return None;
    }
    Some((inner[0].clamp(240, 1400), inner[1].clamp(200, 1200)))
}

fn insert_path(object: &mut Map<String, Value>, path: &str, value: Value) {
    let parts = path.split('.').collect::<Vec<_>>();
    let Some((last, parents)) = parts.split_last() else {
        return;
    };
    let mut current = object;
    for parent in parents {
        current = current
            .entry((*parent).to_string())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("settings path parent is an object");
    }
    current.insert((*last).to_string(), value);
}

fn count_leaf_values(value: &Value) -> u64 {
    match value {
        Value::Object(object) => object.values().map(count_leaf_values).sum(),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reads_utf16le_ini_and_redacts_passwords() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("Ditto.Settings");
        let content = "[Ditto]\r\nMaxEntries=88\r\nPassword=secret\r\nUnknownThing=value\r\n";
        let mut bytes = vec![0xff, 0xfe];
        for word in content.encode_utf16() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        fs::write(&path, bytes).expect("write");
        let (entries, warnings) = read_settings(Some(&path)).expect("parse");
        assert_eq!(entries["MaxEntries"], "88");
        assert!(warnings.is_empty());
    }

    #[test]
    fn malformed_popup_size_is_reported() {
        assert_eq!(parse_popup_size("oops"), None);
        assert_eq!(parse_popup_size("(500,600)"), Some((500, 600)));
    }
}
