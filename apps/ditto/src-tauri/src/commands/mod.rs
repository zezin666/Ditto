use std::{
    collections::HashSet,
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const MOCK_GROUP_NAMES: [&str; 8] = [
    "Work",
    "Personal",
    "Engineering",
    "Design",
    "Snippets",
    "References",
    "QA fixtures",
    "Archive",
];
const MOCK_SOURCE_APPS: [&str; 5] = [
    "Notepad",
    "Visual Studio Code",
    "Microsoft Edge",
    "Terminal",
    "Figma",
];
const MOCK_TEXTS: [&str; 6] = [
    "QA sample clipboard entry for the Fluent settings history.",
    "Synthetic release checklist: verify build, locale, and lifecycle evidence.",
    "Ditto settings mock data never reads the real clipboard history.",
    "A longer synthetic paragraph used to inspect wrapping, row density, and preview layout without exposing user data.",
    "搜索、筛选、预览和分页都在合成测试数据上运行。",
    "复制缓冲区、分组和热键状态均来自 MockBackend。",
];

static BACKEND: LazyLock<Mutex<MockBackend>> = LazyLock::new(|| Mutex::new(MockBackend::seeded()));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl CommandError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: "DITTO_E_NOT_FOUND".to_string(),
            message: message.into(),
            retryable: false,
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: "DITTO_E_CONFLICT".to_string(),
            message: message.into(),
            retryable: false,
        }
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

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                run_at_startup: false,
                start_minimized: true,
                show_tray_icon: true,
                single_instance: true,
                language: "en-US".to_string(),
                startup_message: String::new(),
                check_for_updates: false,
                log_level: "info".to_string(),
            },
            appearance: AppearanceSettings {
                theme: "system".to_string(),
                density: "comfortable".to_string(),
                popup_width: 420,
                popup_height: 480,
                lines_per_clip: 2,
                preview: "auto".to_string(),
                animations: true,
                always_on_top: false,
                font_scale: 100,
            },
            hotkeys: HotkeySettings {
                open: "Ctrl+`".to_string(),
                open_alt: String::new(),
                plain_text_paste: "Ctrl+Shift+V".to_string(),
                first_ten_enabled: true,
                first_ten_chords: (1..=10)
                    .map(|index| format!("Ctrl+{}", if index == 10 { 0 } else { index }))
                    .collect(),
                copy_buffers: (1..=5)
                    .map(|slot| CopyBufferHotkeys {
                        slot,
                        copy: format!("Ctrl+Alt+{slot}"),
                        paste: format!("Alt+{slot}"),
                        cut: String::new(),
                    })
                    .collect(),
            },
            clipboard: ClipboardSettings {
                max_entries: 500,
                expire_days: 30,
                ignore_formats: vec!["CF_DIB".to_string()],
                max_clip_bytes: 10_485_760,
                capture_files: true,
                capture_images: true,
                allow_duplicates: false,
                record_source_app: true,
            },
            groups: GroupSettings {
                default_group_id: None,
                prompt_for_name: true,
                show_grouped_in_main: true,
            },
            copy_buffers: CopyBufferSettings { play_sound: false },
            data: DataSettings {
                data_root: r"C:\Users\synthetic\AppData\Local\Ditto\v2".to_string(),
                portable: false,
                migration_completed: false,
                migration_in_progress: false,
                database_bytes: 12_582_912,
                last_backup_at: Some("2026-11-12T10:30:00Z".to_string()),
            },
        }
    }
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

struct MockBackend {
    settings: SettingsState,
    revision: u64,
    restart_required: bool,
    groups: Vec<GroupNode>,
    clips: Vec<ClipDetail>,
    buffers: Vec<CopyBuffer>,
}

impl MockBackend {
    fn seeded() -> Self {
        let groups = MOCK_GROUP_NAMES
            .iter()
            .enumerate()
            .map(|(index, name)| GroupNode {
                id: index as i64 + 1,
                name: (*name).to_string(),
                parent_id: if index == 7 { Some(3) } else { None },
                count: 0,
            })
            .collect::<Vec<_>>();

        let base_ms = 1_762_000_000_000_i64;
        let clips = (1..=620)
            .map(|id| {
                let text = format!(
                    "{} #{:03}",
                    MOCK_TEXTS[(id - 1) as usize % MOCK_TEXTS.len()],
                    id
                );
                let kind = if id % 17 == 0 {
                    "image"
                } else if id % 11 == 0 {
                    "files"
                } else {
                    "text"
                };
                ClipDetail {
                    summary: ClipSummary {
                        id,
                        kind: kind.to_string(),
                        preview_text: text.clone(),
                        source_app: MOCK_SOURCE_APPS[(id - 1) as usize % MOCK_SOURCE_APPS.len()]
                            .to_string(),
                        created_at_ms: base_ms - (id - 1) * 61_000,
                        last_pasted_at_ms: (id % 5 == 0).then_some(base_ms - 86_400_000),
                        pinned: id <= 12,
                        group_id: (id % 4 != 0).then_some((id - 1) % 8 + 1),
                        copy_buffer: (id <= 5).then_some(id as u8),
                        formats: if kind == "image" {
                            vec!["PNG".to_string(), "CF_DIB".to_string()]
                        } else {
                            vec!["CF_UNICODETEXT".to_string()]
                        },
                        size_bytes: 256 + id as u64 * 13,
                    },
                    text,
                }
            })
            .collect::<Vec<_>>();

        let buffers = (1..=5)
            .map(|slot| CopyBuffer {
                slot,
                name: String::new(),
                enabled: true,
                clip_id: Some(slot as i64),
                clip_preview: clips
                    .get((slot - 1) as usize)
                    .map(|clip| clip.summary.preview_text.clone()),
                updated_at_ms: Some(base_ms - i64::from(slot) * 1000),
            })
            .collect();

        let mut backend = Self {
            settings: SettingsState::default(),
            revision: 1,
            restart_required: false,
            groups,
            clips,
            buffers,
        };
        backend.refresh_group_counts();
        backend
    }

    fn settings_snapshot(&self) -> SettingsEnvelope {
        SettingsEnvelope {
            values: self.settings.clone(),
            revision: self.revision,
            restart_required: self.restart_required,
        }
    }

    fn set_settings(&mut self, next: SettingsState) -> SettingsEnvelope {
        self.restart_required = self.settings.general.language != next.general.language
            || self.settings.data.data_root != next.data.data_root;
        self.settings = next;
        self.revision += 1;
        self.settings_snapshot()
    }

    fn list_clips(
        &self,
        query: &str,
        page: u32,
        page_size: u32,
        group_id: Option<i64>,
        kind: Option<&str>,
        pinned_only: bool,
    ) -> ClipPage {
        let needle = query.trim().to_lowercase();
        let mut clips = self
            .clips
            .iter()
            .filter(|clip| {
                needle.is_empty()
                    || clip.summary.preview_text.to_lowercase().contains(&needle)
                    || clip.summary.source_app.to_lowercase().contains(&needle)
                    || clip
                        .summary
                        .formats
                        .iter()
                        .any(|format| format.to_lowercase().contains(&needle))
            })
            .filter(|clip| group_id.is_none_or(|id| clip.summary.group_id == Some(id)))
            .filter(|clip| kind.is_none_or(|value| clip.summary.kind == value))
            .filter(|clip| !pinned_only || clip.summary.pinned)
            .map(|clip| clip.summary.clone())
            .collect::<Vec<_>>();

        clips.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.created_at_ms.cmp(&left.created_at_ms))
                .then_with(|| right.id.cmp(&left.id))
        });

        let total = clips.len();
        let page = page.max(1);
        let page_size = page_size.clamp(1, 500);
        let start = ((page - 1) * page_size) as usize;
        let items = clips
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .collect();
        ClipPage {
            items,
            total,
            page,
            page_size,
        }
    }

    fn get_clip(&self, id: i64) -> Result<ClipDetail, CommandError> {
        self.clips
            .iter()
            .find(|clip| clip.summary.id == id)
            .cloned()
            .ok_or_else(|| CommandError::not_found(format!("Clip {id} not found")))
    }

    fn delete_clips(&mut self, ids: &[i64]) -> MutationResult {
        let ids = ids.iter().copied().collect::<HashSet<_>>();
        let before = self.clips.len();
        self.clips.retain(|clip| !ids.contains(&clip.summary.id));
        let affected = before - self.clips.len();
        for buffer in &mut self.buffers {
            if buffer.clip_id.is_some_and(|id| ids.contains(&id)) {
                buffer.clip_id = None;
                buffer.clip_preview = None;
            }
        }
        self.refresh_group_counts();
        MutationResult {
            ok: true,
            affected,
            message: "deleted".to_string(),
        }
    }

    fn set_pinned(&mut self, ids: &[i64], pinned: bool) -> MutationResult {
        let ids = ids.iter().copied().collect::<HashSet<_>>();
        let mut affected = 0;
        for clip in &mut self.clips {
            if ids.contains(&clip.summary.id) {
                clip.summary.pinned = pinned;
                affected += 1;
            }
        }
        MutationResult {
            ok: true,
            affected,
            message: "updated".to_string(),
        }
    }

    fn move_to_group(
        &mut self,
        ids: &[i64],
        group_id: Option<i64>,
    ) -> Result<MutationResult, CommandError> {
        if group_id.is_some_and(|id| !self.groups.iter().any(|group| group.id == id)) {
            return Err(CommandError::not_found("Target group not found"));
        }
        let ids = ids.iter().copied().collect::<HashSet<_>>();
        let mut affected = 0;
        for clip in &mut self.clips {
            if ids.contains(&clip.summary.id) {
                clip.summary.group_id = group_id;
                affected += 1;
            }
        }
        self.refresh_group_counts();
        Ok(MutationResult {
            ok: true,
            affected,
            message: "moved".to_string(),
        })
    }

    fn edit_clip(&mut self, id: i64, text: String) -> Result<MutationResult, CommandError> {
        let clip = self
            .clips
            .iter_mut()
            .find(|clip| clip.summary.id == id)
            .ok_or_else(|| CommandError::not_found(format!("Clip {id} not found")))?;
        clip.text = text;
        clip.summary.preview_text = clip.text.chars().take(512).collect();
        clip.summary.size_bytes = clip.text.len() as u64;
        for buffer in &mut self.buffers {
            if buffer.clip_id == Some(id) {
                buffer.clip_preview = Some(clip.summary.preview_text.clone());
            }
        }
        Ok(MutationResult {
            ok: true,
            affected: 1,
            message: "updated".to_string(),
        })
    }

    fn groups(&self) -> Vec<GroupNode> {
        self.groups.clone()
    }

    fn create_group(
        &mut self,
        name: String,
        parent_id: Option<i64>,
    ) -> Result<GroupNode, CommandError> {
        if parent_id.is_some_and(|id| !self.groups.iter().any(|group| group.id == id)) {
            return Err(CommandError::not_found("Parent group not found"));
        }
        let id = self.groups.iter().map(|group| group.id).max().unwrap_or(0) + 1;
        let group = GroupNode {
            id,
            name,
            parent_id,
            count: 0,
        };
        self.groups.push(group.clone());
        Ok(group)
    }

    fn rename_group(&mut self, id: i64, name: String) -> Result<GroupNode, CommandError> {
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or_else(|| CommandError::not_found(format!("Group {id} not found")))?;
        group.name = name;
        Ok(group.clone())
    }

    fn delete_group(&mut self, id: i64) -> MutationResult {
        let mut removed = HashSet::from([id]);
        loop {
            let before = removed.len();
            for group in &self.groups {
                if group
                    .parent_id
                    .is_some_and(|parent| removed.contains(&parent))
                {
                    removed.insert(group.id);
                }
            }
            if removed.len() == before {
                break;
            }
        }
        self.groups.retain(|group| !removed.contains(&group.id));
        for clip in &mut self.clips {
            if clip
                .summary
                .group_id
                .is_some_and(|group| removed.contains(&group))
            {
                clip.summary.group_id = None;
            }
        }
        if self
            .settings
            .groups
            .default_group_id
            .is_some_and(|group| removed.contains(&group))
        {
            self.settings.groups.default_group_id = None;
        }
        self.refresh_group_counts();
        MutationResult {
            ok: true,
            affected: removed.len(),
            message: "deleted".to_string(),
        }
    }

    fn move_group(
        &mut self,
        id: i64,
        parent_id: Option<i64>,
    ) -> Result<MutationResult, CommandError> {
        if parent_id == Some(id) {
            return Err(CommandError::conflict("A group cannot be its own parent"));
        }
        if parent_id.is_some_and(|parent| !self.groups.iter().any(|group| group.id == parent)) {
            return Err(CommandError::not_found("Parent group not found"));
        }
        let mut ancestor = parent_id;
        let mut visited = HashSet::new();
        while let Some(parent) = ancestor {
            if parent == id || !visited.insert(parent) {
                return Err(CommandError::conflict("Group move would create a cycle"));
            }
            ancestor = self
                .groups
                .iter()
                .find(|group| group.id == parent)
                .and_then(|group| group.parent_id);
        }
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or_else(|| CommandError::not_found(format!("Group {id} not found")))?;
        group.parent_id = parent_id;
        Ok(MutationResult {
            ok: true,
            affected: 1,
            message: "moved".to_string(),
        })
    }

    fn copy_buffers(&self) -> Vec<CopyBuffer> {
        self.buffers.clone()
    }

    fn set_copy_buffer(
        &mut self,
        slot: u8,
        name: Option<String>,
        enabled: Option<bool>,
        clip_id: Option<i64>,
    ) -> Result<CopyBuffer, CommandError> {
        if !(1..=5).contains(&slot) {
            return Err(CommandError::conflict("Copy buffer slot must be 1..=5"));
        }
        if clip_id.is_some_and(|id| !self.clips.iter().any(|clip| clip.summary.id == id)) {
            return Err(CommandError::not_found("Clip not found"));
        }
        if let Some(clip_id) = clip_id {
            for buffer in &mut self.buffers {
                if buffer.slot != slot && buffer.clip_id == Some(clip_id) {
                    buffer.clip_id = None;
                    buffer.clip_preview = None;
                }
            }
        }
        let clip_preview = clip_id.and_then(|id| {
            self.clips
                .iter()
                .find(|clip| clip.summary.id == id)
                .map(|clip| clip.summary.preview_text.clone())
        });
        let updated_buffer = {
            let buffer = self
                .buffers
                .iter_mut()
                .find(|buffer| buffer.slot == slot)
                .expect("seeded copy buffers contain all five slots");
            if let Some(name) = name {
                buffer.name = name;
            }
            if let Some(enabled) = enabled {
                buffer.enabled = enabled;
            }
            buffer.clip_id = clip_id;
            buffer.clip_preview = clip_preview;
            buffer.updated_at_ms = Some(now_ms());
            buffer.clone()
        };
        for clip in &mut self.clips {
            clip.summary.copy_buffer = self
                .buffers
                .iter()
                .find(|buffer| buffer.clip_id == Some(clip.summary.id))
                .map(|buffer| buffer.slot);
        }
        Ok(updated_buffer)
    }

    fn refresh_group_counts(&mut self) {
        for group in &mut self.groups {
            group.count = self
                .clips
                .iter()
                .filter(|clip| clip.summary.group_id == Some(group.id))
                .count();
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn backend() -> &'static Mutex<MockBackend> {
    // TODO(T7): replace MockBackend access with the ditto-storage/core command bridge.
    &BACKEND
}

#[tauri::command]
pub fn get_settings() -> Result<SettingsEnvelope, CommandError> {
    Ok(backend().lock().settings_snapshot())
}

#[tauri::command]
pub fn set_settings(settings: SettingsState) -> Result<SettingsEnvelope, CommandError> {
    Ok(backend().lock().set_settings(settings))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn list_clips(
    page: u32,
    page_size: u32,
    query: Option<String>,
    group_id: Option<i64>,
    kind: Option<String>,
    pinned_only: bool,
) -> Result<ClipPage, CommandError> {
    Ok(backend().lock().list_clips(
        query.as_deref().unwrap_or_default(),
        page,
        page_size,
        group_id,
        kind.as_deref(),
        pinned_only,
    ))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn search_clips(
    query: String,
    page: u32,
    page_size: u32,
    group_id: Option<i64>,
    kind: Option<String>,
    pinned_only: bool,
) -> Result<ClipPage, CommandError> {
    Ok(backend().lock().list_clips(
        &query,
        page,
        page_size,
        group_id,
        kind.as_deref(),
        pinned_only,
    ))
}

#[tauri::command]
pub fn get_clip(id: i64) -> Result<ClipDetail, CommandError> {
    backend().lock().get_clip(id)
}

#[tauri::command]
pub fn delete_clips(ids: Vec<i64>) -> Result<MutationResult, CommandError> {
    Ok(backend().lock().delete_clips(&ids))
}

#[tauri::command]
pub fn set_pinned(ids: Vec<i64>, pinned: bool) -> Result<MutationResult, CommandError> {
    Ok(backend().lock().set_pinned(&ids, pinned))
}

#[tauri::command]
pub fn move_to_group(ids: Vec<i64>, group_id: Option<i64>) -> Result<MutationResult, CommandError> {
    backend().lock().move_to_group(&ids, group_id)
}

#[tauri::command]
pub fn edit_clip(id: i64, text: String) -> Result<MutationResult, CommandError> {
    backend().lock().edit_clip(id, text)
}

#[tauri::command]
pub fn get_groups() -> Result<Vec<GroupNode>, CommandError> {
    Ok(backend().lock().groups())
}

#[tauri::command]
pub fn create_group(name: String, parent_id: Option<i64>) -> Result<GroupNode, CommandError> {
    backend().lock().create_group(name, parent_id)
}

#[tauri::command]
pub fn rename_group(id: i64, name: String) -> Result<GroupNode, CommandError> {
    backend().lock().rename_group(id, name)
}

#[tauri::command]
pub fn delete_group(id: i64) -> Result<MutationResult, CommandError> {
    Ok(backend().lock().delete_group(id))
}

#[tauri::command]
pub fn move_group(id: i64, parent_id: Option<i64>) -> Result<MutationResult, CommandError> {
    backend().lock().move_group(id, parent_id)
}

#[tauri::command]
pub fn get_copy_buffers() -> Result<Vec<CopyBuffer>, CommandError> {
    Ok(backend().lock().copy_buffers())
}

#[tauri::command]
pub fn set_copy_buffer(
    slot: u8,
    name: Option<String>,
    enabled: Option<bool>,
    clip_id: Option<i64>,
) -> Result<CopyBuffer, CommandError> {
    backend()
        .lock()
        .set_copy_buffer(slot, name, enabled, clip_id)
}

#[tauri::command]
pub fn backup_db(destination: Option<String>) -> Result<OperationResult, CommandError> {
    // TODO(T7): call the storage backup API and verify the resulting SQLite file.
    Ok(OperationResult {
        ok: true,
        message: "backup".to_string(),
        path: destination.or_else(|| {
            Some(r"C:\Users\synthetic\AppData\Local\Ditto\v2\backups\mock-backup.db".to_string())
        }),
    })
}

#[tauri::command]
pub fn restore_db(source: Option<String>) -> Result<OperationResult, CommandError> {
    // TODO(T7): validate source, require restart, and call the storage restore API.
    Ok(OperationResult {
        ok: true,
        message: "restore".to_string(),
        path: source,
    })
}

#[tauri::command]
pub fn export_data(destination: Option<String>) -> Result<OperationResult, CommandError> {
    // TODO(T7): use a native dialog token and the T6 export implementation.
    Ok(OperationResult {
        ok: true,
        message: "export".to_string(),
        path: destination,
    })
}

#[tauri::command]
pub fn import_data(source: Option<String>) -> Result<OperationResult, CommandError> {
    // TODO(T7): validate manifest/hashes through the T6 import implementation.
    Ok(OperationResult {
        ok: true,
        message: "import".to_string(),
        path: source,
    })
}

#[tauri::command]
pub fn optimize_storage() -> Result<OperationResult, CommandError> {
    // TODO(T7): invoke PRAGMA optimize / incremental vacuum in the storage owner.
    Ok(OperationResult {
        ok: true,
        message: "optimize".to_string(),
        path: None,
    })
}

#[tauri::command]
pub fn get_migration_status() -> Result<MigrationStatus, CommandError> {
    Ok(MigrationStatus {
        completed: false,
        in_progress: false,
        phase: "mock-ready".to_string(),
        processed: 0,
        total: 0,
        source_path: None,
        started_at: None,
        completed_at: None,
        warnings: vec!["MockBackend: no real legacy data was read.".to_string()],
    })
}

#[tauri::command]
pub fn open_data_folder() -> Result<OperationResult, CommandError> {
    // TODO(T7): open the resolved data root through a narrow approved command.
    Ok(OperationResult {
        ok: true,
        message: "mock-open".to_string(),
        path: Some(r"C:\Users\synthetic\AppData\Local\Ditto\v2".to_string()),
    })
}

#[tauri::command]
pub fn open_log_folder() -> Result<OperationResult, CommandError> {
    // TODO(T7): open the resolved log directory through a narrow approved command.
    Ok(OperationResult {
        ok: true,
        message: "mock-open".to_string(),
        path: Some(r"C:\Users\synthetic\AppData\Local\Ditto\v2\logs".to_string()),
    })
}

#[tauri::command]
pub fn get_diagnostics() -> Result<DiagnosticsInfo, CommandError> {
    let backend = backend().lock();
    Ok(DiagnosticsInfo {
        os: "Windows 11".to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        core_pid: std::process::id(),
        webview_pid_count: 6,
        database_bytes: backend.settings.data.database_bytes,
        log_path: r"C:\Users\synthetic\AppData\Local\Ditto\v2\logs".to_string(),
        settings_revision: backend.revision,
        content_included: false,
    })
}

#[tauri::command]
pub fn get_app_info() -> Result<AppInfo, CommandError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_sha: option_env!("DITTO_BUILD_SHA")
            .unwrap_or("mock-t5")
            .to_string(),
        protocol_version: "1".to_string(),
        schema_version: "mock".to_string(),
        source_url: "https://github.com/sabrogden/Ditto".to_string(),
        license: "GPL-3.0-only".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_search_and_pagination_are_deterministic() {
        let backend = MockBackend::seeded();
        let page = backend.list_clips("synthetic", 1, 25, None, None, false);
        assert_eq!(page.page, 1);
        assert_eq!(page.page_size, 25);
        assert!(page.items.len() <= 25);
        assert!(
            page.items
                .windows(2)
                .all(|pair| pair[0].pinned || !pair[1].pinned)
        );
        assert!(page.total > 25);
    }

    #[test]
    fn edit_and_delete_keep_copy_buffers_consistent() {
        let mut backend = MockBackend::seeded();
        backend
            .edit_clip(1, "updated synthetic text".to_string())
            .unwrap();
        assert_eq!(backend.get_clip(1).unwrap().text, "updated synthetic text");
        backend.delete_clips(&[1]);
        assert!(backend.get_clip(1).is_err());
        assert_eq!(backend.copy_buffers()[0].clip_id, None);
    }

    #[test]
    fn group_move_rejects_cycles() {
        let mut backend = MockBackend::seeded();
        let child = backend.create_group("Child".to_string(), Some(3)).unwrap();
        assert!(backend.move_group(3, Some(child.id)).is_err());
        assert!(backend.move_group(child.id, Some(3)).is_ok());
    }
}
