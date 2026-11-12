use ditto_clipboard::{
    CaptureConfig, RawClipboardFormat, RawClipboardSnapshot, build_captured_clip,
};
use ditto_model::ClipKind;
use ditto_storage::legacy::{LegacyClipRecord, LegacyFormatRecord};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::source::{LegacyClipRow, LegacyDataRow};

const MAX_FORMAT_BYTES: usize = 256 * 1024 * 1024;
const MAX_TEXT_FOR_FTS_BYTES: usize = 1024 * 1024;

pub struct MappedClip {
    pub record: LegacyClipRecord,
    pub warnings: Vec<String>,
}

pub fn map_clip(row: &LegacyClipRow, data_rows: &[LegacyDataRow]) -> MappedClip {
    let mut warnings = Vec::new();
    let mut raw_formats = Vec::new();
    for data in data_rows {
        if data.format_name.trim().is_empty() {
            warnings.push(format!(
                "data {} has an empty format name; skipped",
                data.legacy_data_id
            ));
            continue;
        }
        if data.data.is_empty() {
            warnings.push(format!(
                "format {} on clip {} was empty; skipped",
                data.format_name, row.legacy_id
            ));
            continue;
        }
        if data.data.len() > MAX_FORMAT_BYTES {
            warnings.push(format!(
                "format {} on clip {} exceeded {} bytes; skipped",
                data.format_name, row.legacy_id, MAX_FORMAT_BYTES
            ));
            continue;
        }
        raw_formats.push(RawClipboardFormat {
            name: data.format_name.clone(),
            data: data.data.clone(),
            is_primary: false,
        });
    }

    let snapshot = RawClipboardSnapshot {
        formats: raw_formats,
        source_app: None,
        source_window_title: None,
        warnings: warnings.clone(),
    };
    let config = CaptureConfig {
        debounce_ms: 0,
        max_clip_bytes: MAX_FORMAT_BYTES,
        max_image_bytes: MAX_FORMAT_BYTES,
        ignored_formats: Vec::new(),
    };
    let captured = build_captured_clip(snapshot, &config);
    let (captured, mut warnings) = match captured {
        Ok(captured) => {
            let warnings = captured.warnings.clone();
            (captured, warnings)
        }
        Err(error) => {
            warnings.push(format!(
                "clip {} had no readable formats ({error}); inserted metadata-only clip",
                row.legacy_id
            ));
            let content_hash = legacy_empty_hash(row.legacy_id, b"content");
            let formats_hash = legacy_empty_hash(row.legacy_id, b"formats");
            return MappedClip {
                record: empty_record(row, content_hash, formats_hash, warnings.clone()),
                warnings,
            };
        }
    };

    let mut original = data_rows
        .iter()
        .filter(|data| {
            !data.format_name.trim().is_empty()
                && !data.data.is_empty()
                && data.data.len() <= MAX_FORMAT_BYTES
        })
        .map(|data| OriginalFormat {
            legacy_data_id: data.legacy_data_id,
            name: data.format_name.clone(),
            data: data.data.clone(),
            used: false,
        })
        .collect::<Vec<_>>();

    let mut formats = Vec::with_capacity(captured.formats.len());
    for format in &captured.formats {
        let legacy_data_id = original
            .iter_mut()
            .find(|original| {
                !original.used && original.name == format.name && original.data == format.data
            })
            .map(|original| {
                original.used = true;
                Some(original.legacy_data_id)
            })
            .unwrap_or(None);
        formats.push(LegacyFormatRecord {
            legacy_data_id,
            format_name: format.name.clone(),
            seq: format.seq,
            data: format.data.clone(),
            mime: format.mime.clone(),
            is_primary: format.is_primary,
        });
    }

    let created_at_ms = row.date_seconds.max(0).saturating_mul(1000);
    let sort_order = row.clip_order.unwrap_or(created_at_ms as f64);
    let group_sort_order = row.clip_group_order.unwrap_or(sort_order);
    let pin_order = row.sticky_order.unwrap_or_default();
    let is_pinned = row.sticky_order.is_some();
    let preview_text = if row.text.trim().is_empty() {
        captured.preview_text.clone()
    } else {
        row.text.clone()
    };
    let text_content = captured
        .text_content
        .clone()
        .or_else(|| (!row.text.trim().is_empty()).then(|| row.text.clone()));
    let text_truncated = text_content
        .as_ref()
        .is_some_and(|text| text.len() > MAX_TEXT_FOR_FTS_BYTES);
    let kind = if captured.kind == ClipKind::Image
        && !captured.formats.iter().any(|format| format.name == "PNG")
    {
        warnings.push(format!(
            "clip {} contains an image format that could not be converted",
            row.legacy_id
        ));
        ClipKind::Custom
    } else {
        captured.kind
    };
    if warnings.len() > 100 {
        warnings.truncate(100);
        warnings.push("additional per-clip warnings omitted".into());
    }
    let legacy_meta = json!({
        "source": "Ditto.Main",
        "legacy_parent_id": row.parent_legacy_id,
        "conversion_warnings": warnings,
    })
    .to_string();
    MappedClip {
        record: LegacyClipRecord {
            legacy_id: row.legacy_id,
            kind,
            primary_format: captured.primary_format,
            text_content,
            text_truncated,
            preview_text,
            quick_paste_text: row.quick_paste_text.clone(),
            description: row.text.clone(),
            source_app: None,
            source_window_title: None,
            content_hash: captured.content_hash.to_vec(),
            formats_hash: captured.formats_hash.to_vec(),
            hash_algo: "sha256".into(),
            size_bytes: captured.size_bytes,
            is_pinned,
            pin_order,
            group_legacy_id: row.parent_legacy_id,
            sort_order,
            group_sort_order,
            created_at_ms,
            last_pasted_at_ms: row
                .last_paste_date_seconds
                .map(|value| value.max(0).saturating_mul(1000)),
            paste_count: 0,
            dont_auto_delete: row.dont_auto_delete,
            shortcut: row.shortcut,
            global_shortcut: row.global_shortcut,
            move_to_group_shortcut: row.move_to_group_shortcut,
            global_move_to_group_shortcut: row.global_move_to_group_shortcut,
            has_image: captured.has_image,
            thumbnail_png: captured.thumbnail_png,
            legacy_crc: row.crc,
            legacy_meta: Some(legacy_meta),
            formats,
        },
        warnings,
    }
}

struct OriginalFormat {
    legacy_data_id: i64,
    name: String,
    data: Vec<u8>,
    used: bool,
}

fn empty_record(
    row: &LegacyClipRow,
    content_hash: [u8; 32],
    formats_hash: [u8; 32],
    warnings: Vec<String>,
) -> LegacyClipRecord {
    let created_at_ms = row.date_seconds.max(0).saturating_mul(1000);
    let sort_order = row.clip_order.unwrap_or(created_at_ms as f64);
    LegacyClipRecord {
        legacy_id: row.legacy_id,
        kind: ClipKind::Custom,
        primary_format: None,
        text_content: None,
        text_truncated: false,
        preview_text: if row.text.trim().is_empty() {
            row.quick_paste_text.clone()
        } else {
            row.text.clone()
        },
        quick_paste_text: row.quick_paste_text.clone(),
        description: row.text.clone(),
        source_app: None,
        source_window_title: None,
        content_hash: content_hash.to_vec(),
        formats_hash: formats_hash.to_vec(),
        hash_algo: "sha256".into(),
        size_bytes: 0,
        is_pinned: row.sticky_order.is_some(),
        pin_order: row.sticky_order.unwrap_or_default(),
        group_legacy_id: row.parent_legacy_id,
        sort_order,
        group_sort_order: row.clip_group_order.unwrap_or(sort_order),
        created_at_ms,
        last_pasted_at_ms: row
            .last_paste_date_seconds
            .map(|value| value.max(0).saturating_mul(1000)),
        paste_count: 0,
        dont_auto_delete: row.dont_auto_delete,
        shortcut: row.shortcut,
        global_shortcut: row.global_shortcut,
        move_to_group_shortcut: row.move_to_group_shortcut,
        global_move_to_group_shortcut: row.global_move_to_group_shortcut,
        has_image: false,
        thumbnail_png: None,
        legacy_crc: row.crc,
        legacy_meta: Some(
            json!({
                "source": "Ditto.Main",
                "legacy_parent_id": row.parent_legacy_id,
                "conversion_warnings": warnings,
            })
            .to_string(),
        ),
        formats: Vec::new(),
    }
}

fn legacy_empty_hash(legacy_id: i64, domain: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"Ditto.Legacy.Empty");
    hasher.update([0]);
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(legacy_id.to_le_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> LegacyClipRow {
        LegacyClipRow {
            legacy_id: 7,
            date_seconds: 1_700_000_000,
            text: "preview".into(),
            shortcut: Some(42),
            dont_auto_delete: true,
            crc: Some(123),
            parent_legacy_id: None,
            quick_paste_text: "qp".into(),
            clip_order: Some(11.0),
            clip_group_order: Some(12.0),
            global_shortcut: true,
            last_paste_date_seconds: Some(1_700_000_001),
            sticky_order: Some(9.0),
            move_to_group_shortcut: Some(55),
            global_move_to_group_shortcut: true,
        }
    }

    #[test]
    fn unicode_text_and_legacy_fields_map_together() {
        let data = "中文 sample"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let mapped = map_clip(
            &row(),
            &[LegacyDataRow {
                legacy_data_id: 90,
                parent_legacy_id: 7,
                format_name: "CF_UNICODETEXT".into(),
                data,
            }],
        );
        assert_eq!(mapped.record.legacy_id, 7);
        assert!(mapped.record.is_pinned);
        assert_eq!(mapped.record.pin_order, 9.0);
        assert_eq!(mapped.record.shortcut, Some(42));
        assert_eq!(mapped.record.legacy_crc, Some(123));
        assert_eq!(mapped.record.last_pasted_at_ms, Some(1_700_000_001_000));
        assert_eq!(mapped.record.formats[0].legacy_data_id, Some(90));
        assert!(mapped.record.content_hash.len() == 32);
    }

    #[test]
    fn empty_data_still_produces_a_metadata_clip() {
        let mapped = map_clip(
            &row(),
            &[LegacyDataRow {
                legacy_data_id: 91,
                parent_legacy_id: 7,
                format_name: "CF_TEXT".into(),
                data: Vec::new(),
            }],
        );
        assert!(mapped.record.formats.is_empty());
        assert_eq!(
            mapped.record.kind,
            ClipKind::Custom,
            "empty input should not invent a content format"
        );
        assert!(
            mapped
                .warnings
                .iter()
                .any(|warning| warning.contains("empty"))
        );
    }
}
