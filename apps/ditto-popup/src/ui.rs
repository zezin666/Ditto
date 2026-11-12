use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::theme::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Search,
    Folder,
    ChevronLeft,
    ChevronRight,
    Pin,
    Eye,
    Close,
    Text,
    Image,
    Files,
    Rich,
    Check,
    Delete,
    Buffer,
}

pub fn icon_button(
    ui: &mut Ui,
    size: f32,
    icon: Icon,
    palette: Palette,
    selected: bool,
    tooltip: &str,
    radius: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let fill = if selected {
        palette.selection
    } else if response.hovered() {
        palette.hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, radius, fill);
    }
    let color = if selected {
        if palette.high_contrast {
            palette.on_accent
        } else {
            palette.accent
        }
    } else {
        palette.text_secondary
    };
    paint_icon(ui, icon, rect.shrink(rect.width() * 0.25), color, 1.5);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, tooltip));
    response.on_hover_text(tooltip)
}

pub fn paint_icon(ui: &Ui, icon: Icon, rect: Rect, color: Color32, width: f32) {
    let painter = ui.painter();
    let stroke = Stroke::new(width, color);
    let center = rect.center();
    let left = rect.left();
    let right = rect.right();
    let top = rect.top();
    let bottom = rect.bottom();
    match icon {
        Icon::Search => {
            let radius = rect.width() * 0.30;
            painter.circle_stroke(center - vec2(radius * 0.18, radius * 0.18), radius, stroke);
            painter.line_segment(
                [
                    center + vec2(radius * 0.45, radius * 0.45),
                    pos2(right, bottom),
                ],
                stroke,
            );
        }
        Icon::Folder => {
            let points = vec![
                pos2(left, top + rect.height() * 0.25),
                pos2(left + rect.width() * 0.38, top + rect.height() * 0.25),
                pos2(left + rect.width() * 0.48, top),
                pos2(right, top),
                pos2(right, bottom),
                pos2(left, bottom),
            ];
            painter.add(egui::Shape::convex_polygon(
                points,
                Color32::TRANSPARENT,
                stroke,
            ));
        }
        Icon::ChevronLeft => {
            painter.line_segment([pos2(right, top), pos2(left, center.y)], stroke);
            painter.line_segment([pos2(left, center.y), pos2(right, bottom)], stroke);
        }
        Icon::ChevronRight => {
            painter.line_segment([pos2(left, top), pos2(right, center.y)], stroke);
            painter.line_segment([pos2(right, center.y), pos2(left, bottom)], stroke);
        }
        Icon::Pin => {
            painter.line_segment(
                [
                    pos2(left + rect.width() * 0.35, top),
                    pos2(right - rect.width() * 0.10, top + rect.height() * 0.45),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(left + rect.width() * 0.20, top + rect.height() * 0.58),
                    pos2(right - rect.width() * 0.40, top + rect.height() * 0.15),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x, center.y),
                    pos2(left + rect.width() * 0.20, bottom),
                ],
                stroke,
            );
        }
        Icon::Eye => {
            painter.circle_stroke(center, rect.width() * 0.42, stroke);
            painter.line_segment([pos2(left, center.y), pos2(right, center.y)], stroke);
            painter.circle_filled(center, rect.width() * 0.10, color);
        }
        Icon::Close => {
            painter.line_segment([pos2(left, top), pos2(right, bottom)], stroke);
            painter.line_segment([pos2(right, top), pos2(left, bottom)], stroke);
        }
        Icon::Text => {
            for fraction in [0.15, 0.50, 0.85] {
                painter.line_segment(
                    [
                        pos2(left, top + rect.height() * fraction),
                        pos2(right, top + rect.height() * fraction),
                    ],
                    stroke,
                );
            }
        }
        Icon::Image => {
            painter.rect_stroke(rect, 1.0, stroke, StrokeKind::Inside);
            painter.circle_filled(
                pos2(left + rect.width() * 0.30, top + rect.height() * 0.30),
                rect.width() * 0.08,
                color,
            );
            let points = vec![
                pos2(left + rect.width() * 0.12, bottom - rect.height() * 0.12),
                pos2(center.x, center.y),
                pos2(right - rect.width() * 0.12, bottom - rect.height() * 0.12),
            ];
            painter.add(egui::Shape::line(points, stroke));
        }
        Icon::Files => {
            painter.rect_stroke(
                Rect::from_min_size(
                    pos2(left + rect.width() * 0.18, top),
                    vec2(rect.width() * 0.65, rect.height()),
                ),
                1.0,
                stroke,
                StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    pos2(left, top + rect.height() * 0.30),
                    pos2(left + rect.width() * 0.18, top + rect.height() * 0.30),
                ],
                stroke,
            );
        }
        Icon::Rich => {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                "R",
                FontId::proportional(rect.height()),
                color,
            );
        }
        Icon::Check => {
            let points = vec![
                pos2(left, center.y),
                pos2(left + rect.width() * 0.35, bottom),
                pos2(right, top),
            ];
            painter.add(egui::Shape::line(points, stroke));
        }
        Icon::Delete => {
            painter.rect_stroke(
                Rect::from_min_max(
                    pos2(left + rect.width() * 0.20, top + rect.height() * 0.25),
                    pos2(right - rect.width() * 0.20, bottom),
                ),
                1.0,
                stroke,
                StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    pos2(left, top + rect.height() * 0.22),
                    pos2(right, top + rect.height() * 0.22),
                ],
                stroke,
            );
        }
        Icon::Buffer => {
            painter.rect_stroke(rect, 1.0, stroke, StrokeKind::Inside);
            painter.line_segment(
                [
                    pos2(left + rect.width() * 0.28, center.y),
                    pos2(right - rect.width() * 0.28, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x, top + rect.height() * 0.28),
                    pos2(center.x, bottom - rect.height() * 0.28),
                ],
                stroke,
            );
        }
    }
}

pub fn truncate_text(text: &str) -> String {
    let normalized = text.replace(['\r', '\n'], " ");
    if normalized.chars().count() <= 512 {
        normalized
    } else {
        normalized.chars().take(512).collect()
    }
}

pub fn html_summary(input: &str) -> String {
    let mut output = String::with_capacity(input.len().min(512));
    let mut inside_tag = false;
    let mut inside_script = false;
    let lowered = input.to_ascii_lowercase();
    let mut index = 0;
    while index < input.len() {
        let rest = &lowered[index..];
        if rest.starts_with("<script") {
            inside_script = true;
            inside_tag = true;
            index += "<script".len();
            continue;
        }
        if rest.starts_with("</script") {
            inside_script = false;
            inside_tag = true;
            index += "</script".len();
            continue;
        }
        let Some(ch) = input[index..].chars().next() else {
            break;
        };
        index += ch.len_utf8();
        if ch == '<' {
            inside_tag = true;
        } else if ch == '>' {
            inside_tag = false;
        } else if !inside_tag && !inside_script {
            output.push(ch);
        }
        if output.chars().count() >= 512 {
            break;
        }
    }
    truncate_text(output.trim())
}

pub fn color_hex(text: &str) -> Option<Color32> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len().saturating_sub(6) {
        if bytes[start] != b'#' {
            continue;
        }
        let candidate = &text[start..start + 7];
        if candidate[1..].bytes().all(|byte| byte.is_ascii_hexdigit()) {
            let red = u8::from_str_radix(&candidate[1..3], 16).ok()?;
            let green = u8::from_str_radix(&candidate[3..5], 16).ok()?;
            let blue = u8::from_str_radix(&candidate[5..7], 16).ok()?;
            return Some(Color32::from_rgb(red, green, blue));
        }
    }
    None
}

pub fn clipped_rect(rect: Rect, clip: Rect) -> Rect {
    let min = Pos2::new(rect.min.x.max(clip.min.x), rect.min.y.max(clip.min.y));
    let max = Pos2::new(rect.max.x.min(clip.max.x), rect.max.y.min(clip.max.y));
    Rect::from_min_max(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_summary_never_exposes_script_content() {
        let summary = html_summary(
            "<html><body><b>Visible</b><script>secret()</script><p>safe</p></body></html>",
        );
        assert!(summary.contains("Visible"));
        assert!(summary.contains("safe"));
        assert!(!summary.contains("secret"));
    }

    #[test]
    fn color_hex_finds_embedded_token() {
        assert_eq!(
            color_hex("accent #2563EB value"),
            Some(Color32::from_rgb(0x25, 0x63, 0xEB))
        );
    }
}
