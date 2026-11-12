use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ditto_model::{ClipKind, ClipSummary, CopyBufferSlot, GroupNode, SettingsSnapshot};

use crate::provider::{
    ClipPage, ClipProvider, GroupFilter, PasteMode, ProviderError, ProviderResult,
};

const DEFAULT_COUNT: usize = 10_000;

struct MockRecord {
    clip: ClipSummary,
    search_haystack: String,
}

pub struct MockProvider {
    records: Vec<MockRecord>,
    groups: Vec<GroupNode>,
    buffers: [Option<i64>; 5],
    settings: SettingsSnapshot,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(DEFAULT_COUNT)
    }
}

impl MockProvider {
    pub fn new(count: usize) -> Self {
        let groups = synthetic_groups();
        let now = unix_millis();
        let mut records = Vec::with_capacity(count);
        for index in 0..count {
            let id = index as i64 + 1;
            let kind = match index {
                _ if index % 20 == 0 => ClipKind::Image,
                _ if index % 31 == 0 => ClipKind::Files,
                _ if index % 14 == 0 => ClipKind::Rtf,
                _ if index % 7 == 0 => ClipKind::Html,
                _ => ClipKind::Text,
            };
            let group_id = groups.get(index % groups.len()).map(|group| group.id);
            let copy_buffer = (index < 5).then(|| index as u8 + 1);
            let preview_text = synthetic_preview(index, kind);
            let formats = synthetic_formats(kind);
            let source_app = synthetic_source(index).to_string();
            let search_haystack = format!(
                "{} {} {}",
                preview_text.to_lowercase(),
                source_app.to_lowercase(),
                formats.join(" ").to_lowercase()
            );
            records.push(MockRecord {
                clip: ClipSummary {
                    id,
                    kind,
                    preview_text,
                    source_app: Some(source_app),
                    created_at_ms: now - (index as i64 * 60_000),
                    last_pasted_at_ms: (index % 9 == 0).then_some(now - 120_000),
                    pinned: index % 50 == 0,
                    pin_order: (index % 50 == 0).then_some((count - index) as f64),
                    group_id,
                    copy_buffer,
                    formats,
                    size_bytes: 128,
                    has_image: kind == ClipKind::Image,
                    image_thumb_ref: (kind == ClipKind::Image)
                        .then(|| format!("mock://thumbnail/{id}")),
                    pasted_count: u32::from(index % 9 == 0),
                    state_rev: 1,
                },
                search_haystack,
            });
        }
        records.sort_by(|left, right| {
            right
                .clip
                .pinned
                .cmp(&left.clip.pinned)
                .then_with(|| {
                    right
                        .clip
                        .pin_order
                        .unwrap_or(0.0)
                        .total_cmp(&left.clip.pin_order.unwrap_or(0.0))
                })
                .then_with(|| right.clip.created_at_ms.cmp(&left.clip.created_at_ms))
        });
        let mut provider = Self {
            records,
            groups,
            buffers: [None; 5],
            settings: SettingsSnapshot {
                values: serde_json::json!({
                    "popup": {
                        "hide_on_focus_loss": true,
                        "position": "caret",
                        "width": 420,
                        "height": 480,
                        "language": "en-US",
                        "confirm_delete": true
                    }
                }),
                revision: 1,
            },
        };
        provider.update_buffers();
        provider
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn search_rows(&mut self, queries: &[&str], iterations: usize) -> Vec<BenchmarkRow> {
        let mut rows = Vec::new();
        for query in queries {
            for iteration in 0..iterations {
                let started = Instant::now();
                let page = self
                    .load(query, GroupFilter::All, self.records.len())
                    .expect("mock load");
                rows.push(BenchmarkRow {
                    query: (*query).to_string(),
                    iteration,
                    elapsed_us: started.elapsed().as_micros(),
                    rows: page.items.len(),
                });
            }
        }
        rows
    }

    fn filtered_indices(&self, query: &str, group: GroupFilter) -> Vec<usize> {
        let needle = query.trim().to_lowercase();
        self.records
            .iter()
            .enumerate()
            .filter(|(_, record)| {
                group.matches(&record.clip)
                    && (needle.is_empty() || record.search_haystack.contains(&needle))
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn update_buffers(&mut self) {
        for (index, slot) in self.buffers.iter_mut().enumerate() {
            *slot = self
                .records
                .iter()
                .find(|record| record.clip.copy_buffer == Some(index as u8 + 1))
                .map(|record| record.clip.id);
        }
    }
}

impl ClipProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn load(&mut self, query: &str, group: GroupFilter, limit: usize) -> ProviderResult<ClipPage> {
        let started = Instant::now();
        let indices = self.filtered_indices(query, group);
        let total = indices.len() as u64;
        let items = indices
            .into_iter()
            .take(limit)
            .map(|index| self.records[index].clip.clone())
            .collect();
        Ok(ClipPage {
            items,
            total,
            took_ms: started.elapsed().as_millis() as u64,
            next_cursor: None,
        })
    }

    fn groups(&mut self) -> ProviderResult<Vec<GroupNode>> {
        Ok(self.groups.clone())
    }

    fn settings(&mut self) -> ProviderResult<SettingsSnapshot> {
        Ok(self.settings.clone())
    }

    fn copy_buffers(&mut self) -> ProviderResult<Vec<CopyBufferSlot>> {
        Ok(self
            .buffers
            .iter()
            .enumerate()
            .map(|(index, clip_id)| CopyBufferSlot {
                slot: index as u8 + 1,
                name: format!("Buffer {}", index + 1),
                clip_id: *clip_id,
            })
            .collect())
    }

    fn paste(&mut self, clip_id: i64, mode: PasteMode) -> ProviderResult<()> {
        if matches!(mode, PasteMode::Html | PasteMode::Rtf | PasteMode::Files) {
            return Err(ProviderError::Unsupported(format!(
                "mock format paste {mode:?} is represented as a normal paste"
            )));
        }
        let record = self
            .records
            .iter_mut()
            .find(|record| record.clip.id == clip_id)
            .ok_or_else(|| ProviderError::Core(format!("clip {clip_id} not found")))?;
        record.clip.pasted_count = record.clip.pasted_count.saturating_add(1);
        record.clip.last_pasted_at_ms = Some(unix_millis());
        Ok(())
    }

    fn set_pinned(&mut self, clip_id: i64, pinned: bool) -> ProviderResult<()> {
        let record = self
            .records
            .iter_mut()
            .find(|record| record.clip.id == clip_id)
            .ok_or_else(|| ProviderError::Core(format!("clip {clip_id} not found")))?;
        record.clip.pinned = pinned;
        record.clip.pin_order = pinned.then_some(clip_id as f64);
        Ok(())
    }

    fn move_to_group(&mut self, clip_id: i64, group_id: Option<i64>) -> ProviderResult<()> {
        let record = self
            .records
            .iter_mut()
            .find(|record| record.clip.id == clip_id)
            .ok_or_else(|| ProviderError::Core(format!("clip {clip_id} not found")))?;
        record.clip.group_id = group_id;
        Ok(())
    }

    fn delete(&mut self, clip_ids: &[i64]) -> ProviderResult<()> {
        self.records
            .retain(|record| !clip_ids.contains(&record.clip.id));
        self.update_buffers();
        Ok(())
    }

    fn assign_copy_buffer(&mut self, clip_id: i64, slot: u8) -> ProviderResult<()> {
        if !(1..=5).contains(&slot) {
            return Err(ProviderError::Core(format!(
                "copy buffer slot {slot} is out of range"
            )));
        }
        let buffer_index = usize::from(slot - 1);
        for record in &mut self.records {
            if record.clip.copy_buffer == Some(slot) {
                record.clip.copy_buffer = None;
            }
            if record.clip.id == clip_id {
                record.clip.copy_buffer = Some(slot);
            }
        }
        self.buffers[buffer_index] = Some(clip_id);
        Ok(())
    }

    fn clear_copy_buffer(&mut self, slot: u8) -> ProviderResult<()> {
        if !(1..=5).contains(&slot) {
            return Err(ProviderError::Core(format!(
                "copy buffer slot {slot} is out of range"
            )));
        }
        for record in &mut self.records {
            if record.clip.copy_buffer == Some(slot) {
                record.clip.copy_buffer = None;
            }
        }
        self.buffers[usize::from(slot - 1)] = None;
        Ok(())
    }

    fn paste_copy_buffer(&mut self, slot: u8, plain_text: bool) -> ProviderResult<()> {
        let clip_id = self
            .buffers
            .get(usize::from(slot.saturating_sub(1)))
            .copied()
            .flatten()
            .ok_or_else(|| ProviderError::Unsupported(format!("copy buffer {slot} is empty")))?;
        self.paste(
            clip_id,
            if plain_text {
                PasteMode::PlainText
            } else {
                PasteMode::Original
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkRow {
    pub query: String,
    pub iteration: usize,
    pub elapsed_us: u128,
    pub rows: usize,
}

fn synthetic_groups() -> Vec<GroupNode> {
    let mut groups = Vec::with_capacity(50);
    for index in 0..10 {
        groups.push(GroupNode {
            id: index + 1,
            parent_id: None,
            name: match index {
                0 => "Work".to_string(),
                1 => "Research".to_string(),
                2 => "设计".to_string(),
                3 => "客户项目".to_string(),
                _ => format!("Group {}", index + 1),
            },
            sort_order: index as f64,
        });
    }
    for index in 10..50 {
        groups.push(GroupNode {
            id: index as i64 + 1,
            parent_id: Some((index % 10) as i64 + 1),
            name: match index {
                10 => "Project Atlas".to_string(),
                11 => "Meeting notes".to_string(),
                12 => "迭代计划".to_string(),
                _ => format!("Subgroup {}", index - 9),
            },
            sort_order: index as f64,
        });
    }
    groups
}

fn synthetic_preview(index: usize, kind: ClipKind) -> String {
    match kind {
        ClipKind::Image => format!("Screenshot capture {index:05} - 1920x1080 preview"),
        ClipKind::Files => format!(
            "C:\\Synthetic\\Project\\report-{index:05}.pdf\nC:\\Synthetic\\Project\\data-{index:05}.csv"
        ),
        ClipKind::Html => format!(
            "<html><body><b>Release note {index:05}</b><script>not executed</script><p>Safe HTML summary.</p></body></html>"
        ),
        ClipKind::Rtf => format!("{{\\rtf1\\ansi Release note {index:05}\\par Safe RTF summary.}}"),
        ClipKind::Text => match index % 5 {
            0 => format!("客户项目会议纪要 {index:05}: 搜索、分组、快捷粘贴和性能验收。"),
            1 => format!("日本語の入力テスト {index:05}: クリップボードの検索と候補ウィンドウ。"),
            2 => format!("#2563EB color token preview for clip {index:05}"),
            _ => format!(
                "Synthetic clipboard item {index:05}: fast search, keyboard navigation, safe preview, and deterministic fixtures."
            ),
        },
        ClipKind::Custom => format!("Custom clipboard format sample {index:05}"),
    }
}

fn synthetic_formats(kind: ClipKind) -> Vec<String> {
    let mut formats = vec!["CF_UNICODETEXT".to_string()];
    match kind {
        ClipKind::Image => formats.push("CF_DIB".to_string()),
        ClipKind::Files => formats.push("CF_HDROP".to_string()),
        ClipKind::Html => formats.push("HTML Format".to_string()),
        ClipKind::Rtf => formats.push("Rich Text Format".to_string()),
        ClipKind::Custom => formats.push("Ditto.Synthetic".to_string()),
        ClipKind::Text => {}
    }
    formats
}

fn synthetic_source(index: usize) -> &'static str {
    const SOURCES: [&str; 6] = [
        "Explorer",
        "Visual Studio Code",
        "Browser",
        "Word",
        "Terminal",
        "Notepad",
    ];
    SOURCES[index % SOURCES.len()]
}

fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p95(mut values: Vec<u128>) -> u128 {
        values.sort_unstable();
        let index = ((values.len() as f64 * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(values.len().saturating_sub(1));
        values[index]
    }

    #[test]
    fn fixture_has_ten_thousand_mixed_clips() {
        let mut provider = MockProvider::default();
        assert_eq!(provider.len(), 10_000);
        let page = provider
            .load("", GroupFilter::All, 10_000)
            .expect("load fixture");
        assert_eq!(page.items.len(), 10_000);
        assert!(page.items.iter().any(|clip| clip.kind == ClipKind::Image));
        assert!(page.items.iter().any(|clip| clip.kind == ClipKind::Files));
        assert!(
            page.items
                .iter()
                .any(|clip| matches!(clip.kind, ClipKind::Html | ClipKind::Rtf))
        );
        assert!(page.items.iter().any(|clip| clip.pinned));
        assert!(page.items.iter().any(|clip| clip.copy_buffer == Some(1)));
    }

    #[test]
    fn mock_search_stays_under_fifty_milliseconds_p95() {
        let mut provider = MockProvider::default();
        let rows = provider.search_rows(
            &["meeting", "客户项目", "日本語", "Synthetic clipboard"],
            30,
        );
        let elapsed = p95(rows.iter().map(|row| row.elapsed_us).collect());
        assert!(elapsed < 50_000, "mock search p95 was {elapsed} us");
    }

    #[test]
    fn copy_buffer_assignment_and_clear_round_trip() {
        let mut provider = MockProvider::default();
        provider.assign_copy_buffer(42, 3).expect("assign");
        let buffers = provider.copy_buffers().expect("buffers");
        assert_eq!(buffers[2].clip_id, Some(42));
        provider.clear_copy_buffer(3).expect("clear");
        let buffers = provider.copy_buffers().expect("buffers");
        assert_eq!(buffers[2].clip_id, None);
    }
}
