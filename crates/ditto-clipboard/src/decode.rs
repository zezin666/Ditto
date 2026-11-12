use std::io::Cursor;

use image::{DynamicImage, ImageFormat, RgbaImage};
use windows::Win32::Globalization::{
    CP_ACP, CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar, WideCharToMultiByte,
};

use crate::{ClipboardError, ClipboardResult};

const MAX_IMAGE_DIMENSION: u32 = 16_384;

pub fn decode_utf16le(bytes: &[u8]) -> String {
    let mut units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    if units.last() == Some(&0) {
        units.pop();
    }
    String::from_utf16_lossy(&units)
}

pub fn normalize_text_for_hash(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Decodes legacy `CF_TEXT` data using the process ANSI code page.
pub fn decode_ansi_text(bytes: &[u8]) -> String {
    decode_code_page(bytes, CP_ACP, true)
}

/// Decodes legacy `CF_OEMTEXT` data using the OEM code page.
pub fn decode_oem_text(bytes: &[u8]) -> String {
    decode_code_page(bytes, CP_OEMCP, true)
}

/// Decodes bytes in a Windows code page (`CP_ACP`, `CP_OEMCP`, or an explicit
/// numeric code page such as 936/1252/437). Returns empty on unsupported data.
pub fn decode_code_page(bytes: &[u8], code_page: u32, stop_at_nul: bool) -> String {
    let data = if stop_at_nul {
        bytes.split(|byte| *byte == 0).next().unwrap_or_default()
    } else {
        bytes
    };
    if data.is_empty() {
        return String::new();
    }
    let length =
        unsafe { MultiByteToWideChar(code_page, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), data, None) };
    if length <= 0 {
        return String::from_utf8_lossy(data).into_owned();
    }
    let mut wide = vec![0_u16; length as usize];
    let written = unsafe {
        MultiByteToWideChar(
            code_page,
            MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            data,
            Some(&mut wide),
        )
    };
    if written <= 0 {
        return String::from_utf8_lossy(data).into_owned();
    }
    String::from_utf16_lossy(&wide[..written as usize])
}

/// Encodes text for legacy `CF_TEXT` using the process ANSI code page.
/// Windows legacy text formats are NUL-terminated, so the returned payload
/// always ends with one.
pub fn encode_ansi_text(text: &str) -> Vec<u8> {
    encode_code_page_terminated(text, CP_ACP)
}

/// Encodes text for legacy `CF_OEMTEXT` using the OEM code page.
pub fn encode_oem_text(text: &str) -> Vec<u8> {
    encode_code_page_terminated(text, CP_OEMCP)
}

/// Encodes text into a Windows code page and appends the NUL terminator that
/// `CF_TEXT`/`CF_OEMTEXT` consumers expect.
pub fn encode_code_page_terminated(text: &str, code_page: u32) -> Vec<u8> {
    let mut bytes = encode_code_page(text, code_page);
    bytes.push(0);
    bytes
}

/// Encodes text into a Windows code page for legacy clipboard formats.
pub fn encode_code_page(text: &str, code_page: u32) -> Vec<u8> {
    let wide = text.encode_utf16().collect::<Vec<_>>();
    if wide.is_empty() {
        return Vec::new();
    }
    let length = unsafe {
        WideCharToMultiByte(
            code_page,
            0,
            &wide,
            None,
            windows::core::PCSTR::null(),
            None,
        )
    };
    if length <= 0 {
        return text.as_bytes().to_vec();
    }
    let mut bytes = vec![0_u8; length as usize];
    let written = unsafe {
        WideCharToMultiByte(
            code_page,
            0,
            &wide,
            Some(&mut bytes),
            windows::core::PCSTR::null(),
            None,
        )
    };
    if written <= 0 {
        return text.as_bytes().to_vec();
    }
    bytes.truncate(written as usize);
    bytes
}

pub fn html_fragment_bytes(bytes: &[u8]) -> Vec<u8> {
    let Some(start) = header_offset(bytes, b"StartFragment:") else {
        return bytes.to_vec();
    };
    let Some(end) = header_offset(bytes, b"EndFragment:") else {
        return bytes.to_vec();
    };
    if start <= end && end <= bytes.len() {
        bytes[start..end].to_vec()
    } else {
        bytes.to_vec()
    }
}

pub fn html_preview(bytes: &[u8]) -> String {
    let fragment = html_fragment_bytes(bytes);
    let source = decode_html(&fragment);
    strip_html(&source)
}

/// Decodes an HTML clipboard fragment. The Windows HTML clipboard format is
/// normally UTF-8, but producers may declare a legacy charset; honor it when
/// the bytes are not valid UTF-8.
pub fn decode_html(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let charset = html_charset(bytes);
    let code_page = charset
        .as_deref()
        .and_then(code_page_for_charset)
        .unwrap_or(CP_ACP);
    decode_code_page(bytes, code_page, false)
}

fn html_charset(bytes: &[u8]) -> Option<String> {
    let window = &bytes[..bytes.len().min(8 * 1024)];
    let lowered = String::from_utf8_lossy(window).to_ascii_lowercase();
    let index = lowered.find("charset=")? + "charset=".len();
    let rest = &lowered[index..];
    let rest = rest.trim_start_matches(['"', '\'']);
    let end = rest
        .find(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '\'' | ';' | '>' | '/')
        })
        .unwrap_or(rest.len());
    let label = rest[..end].trim();
    (!label.is_empty()).then(|| label.to_string())
}

fn code_page_for_charset(label: &str) -> Option<u32> {
    match label {
        "utf-8" | "utf8" | "unicode-1-1-utf-8" => None,
        "windows-1252" | "cp1252" | "iso-8859-1" | "latin1" => Some(1252),
        "gb2312" | "gbk" | "gb18030" | "cp936" => Some(936),
        "big5" | "cp950" => Some(950),
        "shift_jis" | "shift-jis" | "cp932" => Some(932),
        "cp437" | "ibm437" | "oem" => Some(437),
        _ => None,
    }
}

pub fn rtf_preview(bytes: &[u8]) -> String {
    let source = decode_code_page(bytes, 1252, false);
    let mut output = String::new();
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut pending_unicode = false;
    let mut code_page = 1252_u32;
    while index < chars.len() {
        match chars[index] {
            '\\' => {
                index += 1;
                if index >= chars.len() {
                    break;
                }
                match chars[index] {
                    '\\' | '{' | '}' => {
                        output.push(chars[index]);
                        index += 1;
                    }
                    '\'' => {
                        let mut escaped = Vec::new();
                        while index + 2 < chars.len() && chars[index] == '\'' {
                            let hex = format!("{}{}", chars[index + 1], chars[index + 2]);
                            if let Ok(value) = u8::from_str_radix(&hex, 16) {
                                escaped.push(value);
                            }
                            index += 3;
                            // The next escape starts with its own backslash:
                            // `\'d6\'d0` is one continuous byte run.
                            if index + 3 < chars.len()
                                && chars[index] == '\\'
                                && chars[index + 1] == '\''
                            {
                                index += 1;
                            }
                        }
                        output.push_str(&decode_code_page(&escaped, code_page, false));
                    }
                    'u' => {
                        index += 1;
                        let mut sign = 1_i32;
                        if chars.get(index) == Some(&'-') {
                            sign = -1;
                            index += 1;
                        }
                        let start = index;
                        while chars
                            .get(index)
                            .is_some_and(|character| character.is_ascii_digit())
                        {
                            index += 1;
                        }
                        if start < index {
                            let digits = chars[start..index].iter().collect::<String>();
                            if let Ok(value) = digits.parse::<i32>() {
                                let value = sign * value;
                                if let Some(character) =
                                    char::from_u32((value as u32).wrapping_add(65_536) % 65_536)
                                {
                                    output.push(character);
                                }
                            }
                        }
                        pending_unicode = true;
                    }
                    character if character.is_ascii_alphabetic() => {
                        let start = index;
                        while chars
                            .get(index)
                            .is_some_and(|character| character.is_ascii_alphabetic())
                        {
                            index += 1;
                        }
                        let word = chars[start..index].iter().collect::<String>();
                        let number_start = index;
                        while chars.get(index).is_some_and(|character| {
                            character.is_ascii_digit() || *character == '-'
                        }) {
                            index += 1;
                        }
                        if chars.get(index) == Some(&' ') {
                            index += 1;
                        }
                        if word == "ansicpg" && number_start < index {
                            let digits = chars[number_start..index]
                                .iter()
                                .filter(|character| character.is_ascii_digit())
                                .collect::<String>();
                            if let Ok(value) = digits.parse::<u32>() {
                                code_page = value;
                            }
                        }
                        if word == "par" || word == "line" {
                            output.push('\n');
                        }
                    }
                    _ => index += 1,
                }
            }
            '}' | '{' => index += 1,
            '\r' | '\n' => {
                index += 1;
            }
            character => {
                if pending_unicode && (character == '?' || character.is_whitespace()) {
                    pending_unicode = false;
                    index += 1;
                    continue;
                }
                pending_unicode = false;
                output.push(character);
                index += 1;
            }
        }
    }
    output
}

pub fn hdrop_paths(bytes: &[u8]) -> ClipboardResult<Vec<String>> {
    if bytes.len() < 20 {
        return Err(ClipboardError::InvalidData(
            "CF_HDROP header is too short".to_string(),
        ));
    }
    let offset = u32::from_le_bytes(bytes[0..4].try_into().expect("fixed offset")) as usize;
    let wide = u32::from_le_bytes(bytes[16..20].try_into().expect("fixed flag")) != 0;
    if offset >= bytes.len() {
        return Err(ClipboardError::InvalidData(
            "CF_HDROP path offset is outside the payload".to_string(),
        ));
    }
    let payload = &bytes[offset..];
    let mut paths = Vec::new();
    if wide {
        let units = payload
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        let mut start = 0;
        for (index, unit) in units.iter().enumerate() {
            if *unit == 0 {
                if index > start {
                    paths.push(String::from_utf16_lossy(&units[start..index]));
                }
                start = index + 1;
                if index + 1 >= units.len() || units[index + 1] == 0 {
                    break;
                }
                if paths.len() >= 100_000 {
                    return Err(ClipboardError::InvalidData(
                        "CF_HDROP contains too many paths".to_string(),
                    ));
                }
            }
        }
    } else {
        for path in payload.split(|byte| *byte == 0) {
            if path.is_empty() {
                if !paths.is_empty() {
                    break;
                }
                continue;
            }
            paths.push(String::from_utf8_lossy(path).into_owned());
            if paths.len() >= 100_000 {
                return Err(ClipboardError::InvalidData(
                    "CF_HDROP contains too many paths".to_string(),
                ));
            }
        }
    }
    Ok(paths)
}

pub fn dib_to_png(bytes: &[u8]) -> ClipboardResult<Vec<u8>> {
    if bytes.len() < 40 {
        return Err(ClipboardError::InvalidData(
            "DIB header is too short".to_string(),
        ));
    }
    let header_size = read_u32(bytes, 0)? as usize;
    if header_size < 40 || header_size > bytes.len() {
        return Err(ClipboardError::InvalidData(
            "DIB header size is invalid".to_string(),
        ));
    }
    let width = read_i32(bytes, 4)?;
    let signed_height = read_i32(bytes, 8)?;
    let planes = read_u16(bytes, 12)?;
    let bit_count = read_u16(bytes, 14)?;
    let compression = read_u32(bytes, 16)?;
    if planes != 1 {
        return Err(ClipboardError::InvalidData(
            "DIB must contain one color plane".to_string(),
        ));
    }
    if !matches!(bit_count, 24 | 32) {
        return Err(ClipboardError::UnsupportedFormat(format!(
            "DIB {bit_count}-bit pixels"
        )));
    }
    if !matches!(compression, 0 | 3) {
        return Err(ClipboardError::UnsupportedFormat(format!(
            "DIB compression {compression}"
        )));
    }
    let width_u32 = u32::try_from(width)
        .map_err(|_| ClipboardError::InvalidData("negative DIB width".to_string()))?;
    let height_u32 = signed_height.unsigned_abs().max(1);
    if width_u32 == 0 || width_u32 > MAX_IMAGE_DIMENSION || height_u32 > MAX_IMAGE_DIMENSION {
        return Err(ClipboardError::InvalidData(format!(
            "DIB dimensions {width_u32}x{height_u32} exceed the decode limit"
        )));
    }
    let mut pixel_offset = header_size;
    if compression == 3 && header_size == 40 {
        pixel_offset = pixel_offset.saturating_add(12);
    }
    let row_stride = ((u64::from(width_u32) * u64::from(bit_count)).div_ceil(32) * 4) as usize;
    let required = pixel_offset
        .checked_add(row_stride.saturating_mul(height_u32 as usize))
        .ok_or_else(|| ClipboardError::InvalidData("DIB size overflow".to_string()))?;
    if required > bytes.len() {
        return Err(ClipboardError::InvalidData(format!(
            "DIB payload needs {required} bytes, received {}",
            bytes.len()
        )));
    }

    let top_down = signed_height < 0;
    let bytes_per_pixel = usize::from(bit_count / 8);
    let mut rgba = vec![0_u8; width_u32 as usize * height_u32 as usize * 4];
    let mut has_alpha = false;
    for y in 0..height_u32 as usize {
        let source_y = if top_down {
            y
        } else {
            height_u32 as usize - 1 - y
        };
        let row = pixel_offset + source_y * row_stride;
        for x in 0..width_u32 as usize {
            let source = row + x * bytes_per_pixel;
            let target = (y * width_u32 as usize + x) * 4;
            rgba[target] = bytes[source + 2];
            rgba[target + 1] = bytes[source + 1];
            rgba[target + 2] = bytes[source];
            let alpha = if bytes_per_pixel == 4 {
                bytes[source + 3]
            } else {
                255
            };
            has_alpha |= alpha != 0;
            rgba[target + 3] = alpha;
        }
    }
    if !has_alpha {
        for alpha in rgba.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
    }
    let image = RgbaImage::from_raw(width_u32, height_u32, rgba)
        .ok_or_else(|| ClipboardError::InvalidData("DIB pixel buffer is invalid".to_string()))?;
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| ClipboardError::Image(error.to_string()))?;
    Ok(output.into_inner())
}

pub fn thumbnail_png(png: &[u8]) -> ClipboardResult<Vec<u8>> {
    let image = image::load_from_memory_with_format(png, ImageFormat::Png)
        .map_err(|error| ClipboardError::Image(error.to_string()))?;
    let mut output = Cursor::new(Vec::new());
    image
        .thumbnail(256, 256)
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| ClipboardError::Image(error.to_string()))?;
    Ok(output.into_inner())
}

fn header_offset(bytes: &[u8], key: &[u8]) -> Option<usize> {
    let start = find_bytes(bytes, key)? + key.len();
    let rest = &bytes[start..];
    let digits = rest
        .iter()
        .skip_while(|byte| **byte == b' ')
        .take_while(|byte| byte.is_ascii_digit())
        .copied()
        .collect::<Vec<_>>();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn strip_html(source: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    let mut entity = String::new();
    for character in source.chars() {
        if in_tag {
            if character == '>' {
                in_tag = false;
            }
            continue;
        }
        if character == '<' {
            in_tag = true;
            continue;
        }
        if character == '&' {
            entity.clear();
            entity.push(character);
            continue;
        }
        if !entity.is_empty() {
            entity.push(character);
            if character == ';' {
                output.push_str(&decode_entity(&entity));
                entity.clear();
            } else if entity.len() > 12 {
                output.push_str(&entity);
                entity.clear();
            }
            continue;
        }
        output.push(character);
    }
    if !entity.is_empty() {
        output.push_str(&entity);
    }
    collapse_whitespace(&output)
}

fn decode_entity(entity: &str) -> String {
    match entity {
        "&amp;" => "&".to_string(),
        "&lt;" => "<".to_string(),
        "&gt;" => ">".to_string(),
        "&quot;" => "\"".to_string(),
        "&#39;" | "&apos;" => "'".to_string(),
        "&nbsp;" => " ".to_string(),
        value if value.starts_with("&#x") && value.ends_with(';') => {
            u32::from_str_radix(&value[3..value.len() - 1], 16)
                .ok()
                .and_then(char::from_u32)
                .map_or_else(|| value.to_string(), |character| character.to_string())
        }
        value if value.starts_with("&#") && value.ends_with(';') => value[2..value.len() - 1]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map_or_else(|| value.to_string(), |character| character.to_string()),
        value => value.to_string(),
    }
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read_u16(bytes: &[u8], offset: usize) -> ClipboardResult<u16> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| ClipboardError::InvalidData("DIB is truncated".to_string()))?;
    Ok(u16::from_le_bytes(slice.try_into().expect("fixed size")))
}

fn read_u32(bytes: &[u8], offset: usize) -> ClipboardResult<u32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ClipboardError::InvalidData("DIB is truncated".to_string()))?;
    Ok(u32::from_le_bytes(slice.try_into().expect("fixed size")))
}

fn read_i32(bytes: &[u8], offset: usize) -> ClipboardResult<i32> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ClipboardError::InvalidData("DIB is truncated".to_string()))?;
    Ok(i32::from_le_bytes(slice.try_into().expect("fixed size")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_fragment_and_preview_are_extracted_without_active_content() {
        let fragment = b"<b>Hello &amp; world</b>";
        let header = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";
        let start = header.len();
        let end = start + fragment.len();
        let header = format!(
            "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{start:010}\r\nEndFragment:{end:010}\r\n",
            start, end
        );
        assert_eq!(header.len(), start);
        let mut raw = header.into_bytes();
        raw.extend_from_slice(fragment);
        let extracted = html_fragment_bytes(&raw);
        assert!(extracted.starts_with(b"<b>"));
        assert_eq!(html_preview(&raw), "Hello & world");
    }

    #[test]
    fn rtf_and_hdrop_decoders_preserve_expected_text() {
        assert_eq!(rtf_preview(br"{\rtf1 Hello\par World}"), "Hello\nWorld");
        let mut drop = vec![0_u8; 20];
        drop[0..4].copy_from_slice(&20_u32.to_le_bytes());
        drop[16..20].copy_from_slice(&1_u32.to_le_bytes());
        let paths = "C:\\QA\\one.txt\0C:\\QA\\two.txt\0\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        drop.extend(paths);
        assert_eq!(
            hdrop_paths(&drop).expect("paths"),
            vec!["C:\\QA\\one.txt", "C:\\QA\\two.txt"]
        );
    }

    #[test]
    fn ansi_text_uses_windows_code_pages_instead_of_utf8_lossy() {
        assert_eq!(
            decode_code_page(&[0xD6, 0xD0, 0xCE, 0xC4], 936, true),
            "中文"
        );
        assert_eq!(
            decode_code_page(&[0x43, 0x61, 0x66, 0xE9], 1252, true),
            "Café"
        );
        assert_eq!(decode_code_page(&[0x82], 437, true), "é");
    }

    #[test]
    fn legacy_text_payloads_are_nul_terminated_and_round_trip() {
        for (code_page, text) in [(936, "中文"), (1252, "Café"), (437, "Café")] {
            let encoded = encode_code_page_terminated(text, code_page);
            assert_eq!(encoded.last(), Some(&0), "code page {code_page} terminator");
            assert_eq!(decode_code_page(&encoded, code_page, true), text);
        }
        for bytes in [
            encode_ansi_text("plain Ascii"),
            encode_oem_text("plain Ascii"),
        ] {
            assert_eq!(bytes.last(), Some(&0));
            assert!(!bytes[..bytes.len() - 1].contains(&0));
        }
        assert_eq!(encode_ansi_text(""), vec![0]);
        assert_eq!(encode_oem_text(""), vec![0]);
    }

    #[test]
    fn html_and_rtf_honor_declared_code_pages() {
        let html = b"<meta charset=\"gbk\"><b>\xD6\xD0\xCE\xC4</b>";
        assert_eq!(html_preview(html), "中文");
        assert_eq!(rtf_preview(br"{\rtf1\ansicpg936 \'d6\'d0\'ce\'c4}"), "中文");
    }

    #[test]
    fn two_by_two_dib_becomes_png_and_thumbnail_is_bounded() {
        let mut dib = vec![0_u8; 40 + 16];
        dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
        dib[4..8].copy_from_slice(&2_i32.to_le_bytes());
        dib[8..12].copy_from_slice(&2_i32.to_le_bytes());
        dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32_u16.to_le_bytes());
        for offset in [40, 48] {
            dib[offset..offset + 4].copy_from_slice(&[10, 20, 30, 255]);
            dib[offset + 4..offset + 8].copy_from_slice(&[40, 50, 60, 255]);
        }
        let png = dib_to_png(&dib).expect("DIB conversion");
        assert!(png.starts_with(b"\x89PNG"));
        let thumb = thumbnail_png(&png).expect("thumbnail");
        assert!(thumb.starts_with(b"\x89PNG"));
    }
}
