use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use ditto_clipboard::hdrop_paths;
use ditto_model::{ClipDetail, ClipFormatInfo};

use crate::{PasteError, PasteFormat, PasteOptions, PasteResult};

const MAX_HDROP_BYTES: usize = 16 * 1024 * 1024;
const MAX_HDROP_PATHS: usize = 100_000;
const MAX_FILE_TREE_BYTES: u64 = 16 * 1024 * 1024 * 1024 * 1024;
const MAX_DIB_BYTES: usize = 256 * 1024 * 1024;

pub fn build_payload(
    clip: &ClipDetail,
    data_root: &Path,
    options: &PasteOptions,
) -> PasteResult<Vec<PasteFormat>> {
    let mut formats = if options.plain_text {
        plain_text_payload(clip)?
    } else {
        materialize_formats(clip, data_root)?
    };

    if !options.plain_text && !options.formats.is_empty() {
        formats = select_formats(clip, formats, &options.formats)?;
    }
    if formats.is_empty() {
        return Err(PasteError::Unsupported(format!(
            "clip {} has no pasteable formats",
            clip.summary.id
        )));
    }
    normalize_unicode_text(&mut formats);
    validate_payload(&formats)?;
    Ok(formats)
}

fn plain_text_payload(clip: &ClipDetail) -> PasteResult<Vec<PasteFormat>> {
    let text = clip
        .text_content
        .as_deref()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            PasteError::Unsupported(format!("clip {} has no text content", clip.summary.id))
        })?;
    let mut data = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    data.extend_from_slice(&0_u16.to_le_bytes());
    Ok(vec![PasteFormat {
        name: "CF_UNICODETEXT".to_string(),
        data,
        seq: 0,
        is_primary: true,
    }])
}

fn materialize_formats(clip: &ClipDetail, data_root: &Path) -> PasteResult<Vec<PasteFormat>> {
    let mut formats = Vec::with_capacity(clip.formats.len());
    for format in &clip.formats {
        let data = format_data(format, data_root)?;
        if data.is_empty() {
            continue;
        }
        formats.push(PasteFormat {
            name: format.format_name.clone(),
            data,
            seq: format.seq,
            is_primary: format.is_primary,
        });
    }
    formats.sort_by_key(|format| format.seq);
    add_dib_fallback(&mut formats);
    Ok(formats)
}

fn add_dib_fallback(formats: &mut Vec<PasteFormat>) {
    if formats
        .iter()
        .any(|format| matches!(format.name.as_str(), "CF_DIB" | "CF_DIBV5"))
    {
        return;
    }
    let Some(png) = formats.iter().find(|format| format.name == "PNG") else {
        return;
    };
    match png_to_dib(&png.data) {
        Ok(data) => formats.push(PasteFormat {
            name: "CF_DIB".to_string(),
            data,
            seq: formats
                .iter()
                .map(|format| format.seq)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            is_primary: false,
        }),
        Err(error) => {
            tracing::debug!(%error, "PNG clipboard fallback could not be converted to DIB");
        }
    }
}

fn png_to_dib(png: &[u8]) -> Result<Vec<u8>, String> {
    let decoded = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    let width = decoded.width();
    let height = decoded.height();
    if width == 0 || height == 0 {
        return Err("PNG has an empty image".to_string());
    }
    let pixel_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "PNG dimensions overflow".to_string())?;
    if pixel_bytes > MAX_DIB_BYTES {
        return Err("PNG is too large for a DIB clipboard payload".to_string());
    }
    let pixel_bytes_u32 =
        u32::try_from(pixel_bytes).map_err(|_| "PNG pixel size exceeds DIB limits")?;
    let width_i32 = i32::try_from(width).map_err(|_| "PNG width exceeds DIB limits")?;
    let height_i32 = i32::try_from(height).map_err(|_| "PNG height exceeds DIB limits")?;
    let image = decoded.to_rgba8();
    let mut data = Vec::with_capacity(40 + pixel_bytes);
    data.extend_from_slice(&40_u32.to_le_bytes());
    data.extend_from_slice(&width_i32.to_le_bytes());
    data.extend_from_slice(&height_i32.to_le_bytes());
    data.extend_from_slice(&1_u16.to_le_bytes());
    data.extend_from_slice(&32_u16.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&pixel_bytes_u32.to_le_bytes());
    data.extend_from_slice(&0_i32.to_le_bytes());
    data.extend_from_slice(&0_i32.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&0_u32.to_le_bytes());
    for row in image.rows().rev() {
        for pixel in row {
            data.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Ok(data)
}

fn format_data(format: &ClipFormatInfo, data_root: &Path) -> PasteResult<Vec<u8>> {
    if let Some(data) = &format.data {
        return Ok(data.clone());
    }
    let relative = format
        .data_path
        .as_deref()
        .ok_or_else(|| PasteError::Clipboard("clip format has no data".to_string()))?;
    let relative = safe_relative_path(relative)?;
    let path = data_root.join(relative);
    let metadata = fs::metadata(&path).map_err(|error| {
        PasteError::Clipboard(format!(
            "cannot read stored format {}: {error}",
            format.format_name
        ))
    })?;
    if !metadata.is_file() {
        return Err(PasteError::Clipboard(format!(
            "stored format {} is not a file",
            format.format_name
        )));
    }
    fs::read(&path).map_err(|error| {
        PasteError::Clipboard(format!(
            "cannot read stored format {}: {error}",
            format.format_name
        ))
    })
}

fn safe_relative_path(value: &str) -> PasteResult<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(PasteError::Internal(
            "stored format path escapes the data root".to_string(),
        ));
    }
    Ok(path.to_path_buf())
}

fn select_formats(
    clip: &ClipDetail,
    formats: Vec<PasteFormat>,
    requested: &[String],
) -> PasteResult<Vec<PasteFormat>> {
    let mut selected = Vec::new();
    for request in requested {
        if let Some(format) = formats
            .iter()
            .find(|format| format_matches(request, &format.name))
        {
            if !selected
                .iter()
                .any(|existing: &PasteFormat| existing.name == format.name)
            {
                selected.push(format.clone());
            }
            continue;
        }
        if request_is_text(request) {
            if let Some(text) = clip.text_content.as_deref().filter(|text| !text.is_empty()) {
                let mut data = text
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>();
                data.extend_from_slice(&0_u16.to_le_bytes());
                selected.push(PasteFormat {
                    name: "CF_UNICODETEXT".to_string(),
                    data,
                    seq: selected.len() as u32,
                    is_primary: true,
                });
                continue;
            }
        }
    }
    if selected.is_empty() {
        let fallback = formats
            .iter()
            .find(|format| format.is_primary)
            .or_else(|| formats.first())
            .cloned()
            .ok_or_else(|| {
                PasteError::Unsupported(format!("clip {} has no fallback format", clip.summary.id))
            })?;
        selected.push(fallback);
    }
    Ok(selected)
}

fn request_is_text(request: &str) -> bool {
    matches!(
        request.to_ascii_lowercase().as_str(),
        "text" | "plain" | "plain_text" | "cf_unicodetext" | "cf_text"
    )
}

fn format_matches(request: &str, format_name: &str) -> bool {
    let request = request.to_ascii_lowercase();
    let format_name = format_name.to_ascii_lowercase();
    match request.as_str() {
        "text" | "plain" | "plain_text" => matches!(
            format_name.as_str(),
            "cf_unicodetext" | "cf_text" | "cf_oemtext"
        ),
        "html" => format_name == "html format",
        "rtf" | "rich_text" => format_name == "rich text format",
        "image" => matches!(format_name.as_str(), "png" | "cf_dib" | "cf_dibv5"),
        "files" | "file" => format_name == "cf_hdrop",
        _ => request == format_name,
    }
}

fn normalize_unicode_text(formats: &mut [PasteFormat]) {
    for format in formats {
        if format.name == "CF_UNICODETEXT" {
            if format.data.len() % 2 != 0 {
                format.data.push(0);
            }
            if !format.data.ends_with(&[0, 0]) {
                format.data.extend_from_slice(&0_u16.to_le_bytes());
            }
        }
    }
}

fn validate_payload(formats: &[PasteFormat]) -> PasteResult<()> {
    for format in formats {
        if format.name == "CF_HDROP" {
            if format.data.len() > MAX_HDROP_BYTES {
                return Err(PasteError::InvalidFiles(
                    "file drop exceeds 16 MiB".to_string(),
                ));
            }
            let paths = hdrop_paths(&format.data)
                .map_err(|error| PasteError::InvalidFiles(error.to_string()))?
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            validate_file_paths(&paths)?;
        }
    }
    Ok(())
}

pub fn validate_file_paths(paths: &[PathBuf]) -> PasteResult<()> {
    if paths.is_empty() {
        return Err(PasteError::InvalidFiles(
            "file drop contains no paths".to_string(),
        ));
    }
    if paths.len() > MAX_HDROP_PATHS {
        return Err(PasteError::InvalidFiles(format!(
            "file drop exceeds {MAX_HDROP_PATHS} paths"
        )));
    }
    let mut total = 0_u64;
    for path in paths {
        let metadata = fs::metadata(path).map_err(|error| {
            PasteError::InvalidFiles(format!("file is not accessible: {error}"))
        })?;
        if metadata.is_file() {
            total = total.checked_add(metadata.len()).ok_or_else(|| {
                PasteError::InvalidFiles("file size total overflowed".to_string())
            })?;
        }
        if total > MAX_FILE_TREE_BYTES {
            return Err(PasteError::InvalidFiles(
                "file drop exceeds the size limit".to_string(),
            ));
        }
    }
    Ok(())
}

pub fn build_file_drop(paths: &[PathBuf]) -> PasteResult<Vec<u8>> {
    validate_file_paths(paths)?;
    let mut data = Vec::new();
    data.extend_from_slice(&(20_u32).to_le_bytes());
    data.extend_from_slice(&0_i32.to_le_bytes());
    data.extend_from_slice(&0_i32.to_le_bytes());
    data.extend_from_slice(&0_i32.to_le_bytes());
    data.extend_from_slice(&1_i32.to_le_bytes());
    for path in paths {
        use std::os::windows::ffi::OsStrExt;
        for unit in path.as_os_str().encode_wide() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        data.extend_from_slice(&0_u16.to_le_bytes());
    }
    data.extend_from_slice(&0_u16.to_le_bytes());
    if data.len() > MAX_HDROP_BYTES {
        return Err(PasteError::InvalidFiles(
            "file drop exceeds 16 MiB".to_string(),
        ));
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use ditto_model::{ClipDetail, ClipFormatInfo, ClipKind, ClipSummary};

    use super::*;

    fn clip(formats: Vec<ClipFormatInfo>) -> ClipDetail {
        ClipDetail {
            summary: ClipSummary {
                id: 9,
                kind: ClipKind::Text,
                preview_text: "QA sample".to_string(),
                source_app: None,
                created_at_ms: 0,
                last_pasted_at_ms: None,
                pinned: false,
                pin_order: None,
                group_id: None,
                copy_buffer: None,
                formats: formats
                    .iter()
                    .map(|format| format.format_name.clone())
                    .collect(),
                size_bytes: 9,
                has_image: false,
                image_thumb_ref: None,
                pasted_count: 0,
                state_rev: 1,
            },
            text_content: Some("QA sample".to_string()),
            quick_paste_text: String::new(),
            description: String::new(),
            source_window_title: None,
            formats,
            dont_auto_delete: false,
            updated_at_ms: 0,
        }
    }

    fn format(name: &str, data: Vec<u8>, seq: u32) -> ClipFormatInfo {
        ClipFormatInfo {
            format_name: name.to_string(),
            seq,
            size_bytes: data.len() as u64,
            mime: None,
            sha256: Vec::new(),
            data: Some(data),
            data_path: None,
            is_primary: seq == 0,
        }
    }

    #[test]
    fn plain_text_prefers_text_content_and_has_a_terminator() {
        let detail = clip(vec![
            format("CF_UNICODETEXT", b"wrong".to_vec(), 0),
            format("Rich Text Format", b"{\\rtf1 QA}".to_vec(), 1),
        ]);
        let formats = build_payload(
            &detail,
            Path::new("."),
            &PasteOptions {
                plain_text: true,
                ..PasteOptions::default()
            },
        )
        .expect("plain payload");
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].name, "CF_UNICODETEXT");
        assert!(formats[0].data.ends_with(&[0, 0]));
    }

    #[test]
    fn requested_missing_text_format_falls_back_to_text_content() {
        let detail = clip(vec![format("Rich Text Format", b"{\\rtf1 QA}".to_vec(), 0)]);
        let formats = build_payload(
            &detail,
            Path::new("."),
            &PasteOptions {
                formats: vec!["text".to_string()],
                ..PasteOptions::default()
            },
        )
        .expect("fallback payload");
        assert_eq!(formats[0].name, "CF_UNICODETEXT");
    }

    #[test]
    fn requested_unknown_format_falls_back_to_primary() {
        let detail = clip(vec![format("CF_UNICODETEXT", vec![0, 0], 0)]);
        let formats = build_payload(
            &detail,
            Path::new("."),
            &PasteOptions {
                formats: vec!["application/missing".to_string()],
                ..PasteOptions::default()
            },
        )
        .expect("fallback");
        assert_eq!(formats[0].name, "CF_UNICODETEXT");
    }

    #[test]
    fn format_aliases_cover_text_html_rtf_image_and_files() {
        assert!(format_matches("text", "CF_UNICODETEXT"));
        assert!(format_matches("html", "HTML Format"));
        assert!(format_matches("rtf", "Rich Text Format"));
        assert!(format_matches("image", "PNG"));
        assert!(format_matches("image", "CF_DIBV5"));
        assert!(format_matches("files", "CF_HDROP"));
        assert!(!format_matches("files", "CF_UNICODETEXT"));
    }

    #[test]
    fn png_only_payload_gets_a_dib_fallback() {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("encode PNG");
        let detail = clip(vec![format("PNG", png.into_inner(), 0)]);
        let formats = build_payload(&detail, Path::new("."), &PasteOptions::default())
            .expect("image payload");
        assert!(formats.iter().any(|format| format.name == "PNG"));
        assert!(formats.iter().any(|format| format.name == "CF_DIB"));
    }

    #[test]
    fn stored_blob_paths_cannot_escape_the_data_root() {
        assert!(matches!(
            safe_relative_path(r"..\outside.bin"),
            Err(PasteError::Internal(_))
        ));
        assert!(matches!(
            safe_relative_path(r"C:\outside.bin"),
            Err(PasteError::Internal(_))
        ));
        assert!(safe_relative_path(r"blobs\aa\bb\payload.bin").is_ok());
    }

    #[test]
    fn file_drop_round_trips_synthetic_path_and_rejects_missing_path() {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("ditto-t4-{stamp}.txt"));
        fs::write(&path, b"QA sample 001").expect("fixture");
        let bytes = build_file_drop(std::slice::from_ref(&path)).expect("build");
        let parsed = hdrop_paths(&bytes).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            PathBuf::from(&parsed[0]),
            path,
            "the synthetic path should round-trip"
        );
        let _ = fs::remove_file(&path);
        assert!(matches!(
            build_file_drop(std::slice::from_ref(&path)),
            Err(PasteError::InvalidFiles(_))
        ));
    }
}
