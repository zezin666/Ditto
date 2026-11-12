use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{StorageError, StorageResult};

pub const INLINE_FORMAT_LIMIT: usize = 64 * 1024;

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256(bytes))
}

pub fn format_extension(format_name: &str, mime: Option<&str>) -> &'static str {
    match format_name {
        "PNG" => "png",
        "CF_DIB" | "CF_DIBV5" => "dib",
        "HTML Format" => "html",
        "Rich Text Format" => "rtf",
        "CF_UNICODETEXT" | "CF_TEXT" | "CF_OEMTEXT" => "txt",
        _ => match mime {
            Some("image/png") => "png",
            Some("text/html") => "html",
            Some("application/rtf") => "rtf",
            Some("text/plain") => "txt",
            _ => "bin",
        },
    }
}

pub fn should_store_blob(format_name: &str, mime: Option<&str>, len: usize) -> bool {
    len > INLINE_FORMAT_LIMIT
        || matches!(format_name, "PNG" | "CF_DIB" | "CF_DIBV5")
        || mime.is_some_and(|value| value.starts_with("image/"))
}

pub fn write_blob(
    data_root: &Path,
    bytes: &[u8],
    format_name: &str,
    mime: Option<&str>,
) -> StorageResult<String> {
    let hash = sha256_hex(bytes);
    let extension = format_extension(format_name, mime);
    let relative = format!(
        "blobs/{}/{}/{}.{}",
        &hash[0..2],
        &hash[2..4],
        hash,
        extension
    );
    write_content_addressed(data_root, &relative, bytes)?;
    Ok(relative)
}

pub fn write_thumbnail(data_root: &Path, clip_hash: &[u8], png: &[u8]) -> StorageResult<String> {
    let hash = hex::encode(clip_hash);
    let relative = format!("thumbs/{hash}-256.png");
    write_content_addressed(data_root, &relative, png)?;
    Ok(relative)
}

fn write_content_addressed(data_root: &Path, relative: &str, bytes: &[u8]) -> StorageResult<()> {
    let target = data_root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    if target.is_file() {
        return Ok(());
    }
    let parent = target
        .parent()
        .ok_or_else(|| StorageError::InvalidData("blob path has no parent".to_string()))?;
    fs::create_dir_all(parent)?;
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| StorageError::InvalidData("blob path has no UTF-8 name".to_string()))?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if let Err(error) = fs::rename(&temporary, &target) {
        if target.is_file() {
            let _ = fs::remove_file(&temporary);
        } else {
            return Err(error.into());
        }
    }
    Ok(())
}

pub fn generate_thumbnail_png(png_bytes: &[u8]) -> StorageResult<Vec<u8>> {
    let image = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .map_err(|error| StorageError::InvalidData(format!("PNG decode failed: {error}")))?;
    let thumbnail = image.thumbnail(256, 256);
    let mut output = std::io::Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| StorageError::InvalidData(format!("thumbnail encode failed: {error}")))?;
    Ok(output.into_inner())
}

pub fn collect_files_recursive(root: &Path) -> StorageResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.is_dir() {
        return Ok(files);
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    Ok(files)
}
