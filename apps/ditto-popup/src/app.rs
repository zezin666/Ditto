use std::{
    collections::BTreeSet,
    ffi::c_void,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::AtomicIsize,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ditto_ipc::{ClientKind, Event, Response};
use ditto_model::{ClipKind, ClipSummary, CopyBufferSlot, GroupNode, SettingsSnapshot, ThemeMode};
use ditto_ui_tokens::{Mode, border, dimension, font, radius};
use ditto_win::{PipeClient, hello_request, make_popup_window, pipe_name, system_theme};
use eframe::egui::{
    self, Align, Align2, Color32, Event as EguiEvent, FontData, FontDefinitions, FontFamily,
    FontId, Id, ImeEvent, Label, Margin, Modifiers, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextStyle, Ui, UserData, ViewportCommand, WidgetInfo, WidgetType, WindowLevel,
    pos2, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, POINT};

use crate::{
    localization::{Localizer, key},
    mock::MockProvider,
    options::{CaptureOptions, CaptureState, PopupOptions},
    platform::{self, PositionMode},
    provider::{ClipProvider, GroupFilter, IpcProvider, PasteMode, ProviderResult},
    theme::{Palette, Theme},
    ui::{Icon, color_hex, html_summary, icon_button, paint_icon, truncate_text},
    worker::{
        ActionKind, ProviderAction, ProviderCommand, ProviderEvent, ProviderMetadata,
        ProviderWorker,
    },
};

const SEARCH_ID: &str = "ditto-popup-search";
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(75);
const FIRST_TEN: usize = 10;

enum PopupSignal {
    Show { received_at: Instant },
    Hide,
    Connection(bool),
    Shutdown,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeSettings {
    width: f32,
    height: f32,
    hide_on_focus_loss: bool,
    position: PositionMode,
    confirm_delete: bool,
}

impl RuntimeSettings {
    fn from_options(options: &PopupOptions) -> Self {
        Self {
            width: dimension::popup_width(Mode::Light),
            height: dimension::popup_height(Mode::Light),
            hide_on_focus_loss: options.hide_on_focus_loss,
            position: options.default_position(),
            confirm_delete: true,
        }
    }
}

#[derive(Default)]
struct Selection {
    primary: Option<usize>,
    anchor: Option<usize>,
    multi: BTreeSet<i64>,
}

impl Selection {
    fn clear(&mut self) {
        self.primary = None;
        self.anchor = None;
        self.multi.clear();
    }

    fn is_selected(&self, index: usize, clip: &ClipSummary) -> bool {
        self.primary == Some(index) || self.multi.contains(&clip.id)
    }

    fn ids(&self, items: &[ClipSummary]) -> Vec<i64> {
        if !self.multi.is_empty() {
            let mut ids: Vec<_> = self.multi.iter().copied().collect();
            if let Some(index) = self.primary
                && let Some(clip) = items.get(index)
                && !ids.contains(&clip.id)
            {
                ids.push(clip.id);
            }
            return ids;
        }
        self.primary
            .and_then(|index| items.get(index))
            .map(|clip| vec![clip.id])
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusTone {
    Info,
    Error,
}

struct StatusMessage {
    tone: StatusTone,
    text: String,
    detail: Option<String>,
}

enum CaptureStage {
    WaitingData,
    WaitingReload,
    Ready(Instant),
    Requested(Instant),
}

struct CaptureRuntime {
    options: CaptureOptions,
    stage: CaptureStage,
}

struct SoakSample {
    cycle: u32,
    phase: &'static str,
    gdi_objects: u32,
    user_objects: u32,
    private_bytes: usize,
    working_set_bytes: usize,
}

struct SoakRuntime {
    total: u32,
    log_path: PathBuf,
    samples: Vec<SoakSample>,
}

struct BenchScrollRuntime {
    frames: u32,
    offset: f32,
    log_path: PathBuf,
    rows: Vec<(u32, f32, f32, f32, usize)>,
}

struct DropdownState {
    open: bool,
    position: egui::Pos2,
}

#[derive(Debug, Clone, Copy)]
enum ListInteraction {
    Select { index: usize, modifiers: Modifiers },
    Paste { index: usize, plain_text: bool },
}

enum MenuCommand {
    Paste { index: usize, mode: PasteMode },
    TogglePin { index: usize, pinned: bool },
    Move { index: usize, group_id: Option<i64> },
    Delete { index: usize },
    AssignBuffer { index: usize, slot: u8 },
    ClearBuffer { slot: u8 },
    PasteBuffer { slot: u8, plain_text: bool },
    Preview { index: usize },
}

struct RowContext<'a> {
    locale: &'a Localizer,
    palette: Palette,
    mode: Mode,
    groups: &'a [GroupNode],
    copy_buffers: &'a [CopyBufferSlot],
    keyboard_focus: bool,
}

struct RowResult {
    interaction: Option<ListInteraction>,
    menu: Option<MenuCommand>,
    scroll_to: bool,
}

struct BadgeStyle<'a> {
    color: Color32,
    fill: Color32,
    tooltip: &'a str,
    mode: Mode,
}

pub struct PopupApp {
    options: PopupOptions,
    locale: Localizer,
    signals: Receiver<PopupSignal>,
    ctx: egui::Context,
    provider: ProviderWorker,
    theme: Theme,
    palette: Palette,
    visible: bool,
    always_on_top: bool,
    user_activated: bool,
    previous_foreground: HWND,
    last_popup_position: Option<POINT>,
    window_handle: Option<HWND>,
    window_styled: bool,
    shown_at: Option<Instant>,
    show_received_at: Option<Instant>,
    frame_logged_for_show: bool,
    perf_log: PathBuf,
    settings: RuntimeSettings,
    items: Vec<ClipSummary>,
    total: u64,
    groups: Vec<GroupNode>,
    copy_buffers: Vec<CopyBufferSlot>,
    query: String,
    loaded_query: String,
    current_group: GroupFilter,
    request_id: u64,
    loading: bool,
    search_due: Option<Instant>,
    selection: Selection,
    preview_open: bool,
    focus_target: FocusTarget,
    ime_composing: bool,
    service_error: Option<String>,
    status: Option<StatusMessage>,
    pending_delete: Option<Vec<i64>>,
    group_menu: DropdownState,
    rendered_rows: usize,
    pending_scroll: Option<usize>,
    last_frame_at: Instant,
    capture: Option<CaptureRuntime>,
    soak: Option<SoakRuntime>,
    bench_scroll: Option<BenchScrollRuntime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusTarget {
    Title,
    List,
    Search,
}

impl PopupApp {
    pub fn new(creation: &eframe::CreationContext<'_>, options: PopupOptions) -> Self {
        let theme = options.theme_override.unwrap_or_else(|| {
            Theme::from(if crate::theme::high_contrast_enabled() {
                ThemeMode::HighContrast
            } else {
                system_theme()
            })
        });
        let palette = Palette::for_theme(theme);
        let locale = Localizer::load(options.language);
        install_fonts(&creation.egui_ctx);
        apply_visuals(&creation.egui_ctx, theme, palette);

        let (sender, receiver) = mpsc::channel();
        let (window_handle, window_styled) = native_window(creation)
            .and_then(|hwnd| make_popup_window(hwnd).ok().map(|()| (Some(hwnd), true)))
            .unwrap_or((None, false));
        if let Some(hwnd) = window_handle {
            platform::configure_window(hwnd, theme, palette);
            resize_for_options(&creation.egui_ctx, hwnd, &options, theme);
        }
        if !options.mock {
            let handle = Arc::new(AtomicIsize::new(
                window_handle.map_or(0, |hwnd| hwnd.0 as isize),
            ));
            spawn_ipc_client(creation.egui_ctx.clone(), sender, handle);
        }

        let provider: Box<dyn ClipProvider> = if options.mock {
            Box::new(MockProvider::default())
        } else {
            Box::new(IpcProvider::new())
        };
        let provider = ProviderWorker::start(provider);
        provider.send(ProviderCommand::RefreshMetadata);

        let capture = options
            .capture
            .clone()
            .map(|capture_options| CaptureRuntime {
                options: capture_options,
                stage: CaptureStage::WaitingData,
            });
        let soak = options.soak_cycles.map(|total| SoakRuntime {
            total,
            log_path: options.soak_log.clone(),
            samples: Vec::new(),
        });
        let mut runtime_settings = RuntimeSettings::from_options(&options);
        if options.capture.is_some() || options.soak_cycles.is_some() || options.bench_scroll {
            runtime_settings.hide_on_focus_loss = false;
        }
        let mut app = Self {
            settings: runtime_settings,
            options: options.clone(),
            locale,
            signals: receiver,
            ctx: creation.egui_ctx.clone(),
            provider,
            theme,
            palette,
            visible: false,
            always_on_top: true,
            user_activated: false,
            previous_foreground: HWND::default(),
            last_popup_position: None,
            window_handle,
            window_styled,
            shown_at: None,
            show_received_at: None,
            frame_logged_for_show: false,
            perf_log: PathBuf::new(),
            items: Vec::new(),
            total: 0,
            groups: Vec::new(),
            copy_buffers: Vec::new(),
            query: String::new(),
            loaded_query: String::new(),
            current_group: GroupFilter::All,
            request_id: 0,
            loading: false,
            search_due: None,
            selection: Selection::default(),
            preview_open: false,
            focus_target: FocusTarget::Search,
            ime_composing: false,
            service_error: None,
            status: None,
            pending_delete: None,
            group_menu: DropdownState {
                open: false,
                position: egui::Pos2::ZERO,
            },
            rendered_rows: 0,
            pending_scroll: None,
            last_frame_at: Instant::now(),
            capture,
            soak,
            bench_scroll: options.bench_scroll.then(|| BenchScrollRuntime {
                frames: 0,
                offset: 0.0,
                log_path: options.bench_log.clone(),
                rows: Vec::new(),
            }),
        };
        app.perf_log = app.options.perf_log.clone();
        if app.options.auto_show {
            app.show_popup(Instant::now(), true);
        }
        app
    }

    fn mode(&self) -> Mode {
        self.theme.mode()
    }

    fn search_id() -> Id {
        Id::new(SEARCH_ID)
    }

    fn process_signals(&mut self) {
        while let Ok(signal) = self.signals.try_recv() {
            match signal {
                PopupSignal::Show { received_at } => {
                    self.show_popup(received_at, true);
                }
                PopupSignal::Hide => self.hide_popup(),
                PopupSignal::Connection(connected) => {
                    if connected {
                        if self.service_error.is_some() {
                            self.status = Some(StatusMessage {
                                tone: StatusTone::Info,
                                text: self.locale.text(key::STATUS_RECONNECTED).to_string(),
                                detail: None,
                            });
                        }
                        self.service_error = None;
                    } else if self.visible {
                        self.service_error =
                            Some(self.locale.text(key::STATE_DISCONNECTED).to_string());
                    }
                }
                PopupSignal::Shutdown => {
                    self.ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }

    fn show_popup(&mut self, received_at: Instant, reset_view: bool) {
        if let Some(hwnd) = self.window_handle {
            self.previous_foreground = platform::current_foreground();
            let placement = if let Some(scale) = self.options.dpi_scale_override {
                let width = (self.settings.width * scale).round() as i32;
                let height = (self.settings.height * scale).round() as i32;
                platform::center_on_primary(hwnd, width, height)
            } else {
                platform::position_and_size(
                    hwnd,
                    self.settings.width,
                    self.settings.height,
                    self.settings.position,
                    self.last_popup_position,
                )
            };
            if let Some(placement) = placement {
                self.last_popup_position = Some(POINT {
                    x: placement.x,
                    y: placement.y,
                });
            }
            if placement.is_none() {
                let dpi = platform::dpi_for_window(hwnd);
                let scale = f64::from(dpi) / 96.0;
                let _ = ditto_win::resize_popup(
                    hwnd,
                    (f64::from(self.settings.width) * scale).round() as i32,
                    (f64::from(self.settings.height) * scale).round() as i32,
                );
            }
            platform::restore_no_activate(hwnd);
            ditto_win::set_popup_visible(hwnd, true);
        }
        self.ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        self.ctx
            .send_viewport_cmd(ViewportCommand::WindowLevel(if self.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }));
        self.visible = true;
        self.user_activated = false;
        self.shown_at = Some(Instant::now());
        self.show_received_at = Some(received_at);
        self.frame_logged_for_show = false;
        self.focus_target = FocusTarget::Search;
        self.ctx.memory_mut(|memory| {
            memory.request_focus(Self::search_id());
        });
        if reset_view {
            self.query.clear();
            self.loaded_query.clear();
            self.current_group = GroupFilter::All;
            self.selection.clear();
            self.preview_open = false;
            self.pending_delete = None;
            self.group_menu.open = false;
        }
        self.request_load();
        self.request_repaint_soon();
    }

    fn hide_popup(&mut self) {
        self.visible = false;
        self.group_menu.open = false;
        if let Some(hwnd) = self.window_handle {
            ditto_win::set_popup_visible(hwnd, false);
            platform::restore_no_activate(hwnd);
        }
        if self.user_activated {
            platform::restore_foreground(self.previous_foreground);
        }
        self.user_activated = false;
        self.ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.ctx.memory_mut(|memory| memory.stop_text_input());
    }

    fn enforce_hidden_unless_requested(&self) {
        if self.visible {
            return;
        }
        if let Some(hwnd) = self.window_handle {
            ditto_win::set_popup_visible(hwnd, false);
        }
        self.ctx.send_viewport_cmd(ViewportCommand::Visible(false));
    }

    fn style_window_once(&mut self) {
        if self.window_styled {
            return;
        }
        if let Some(hwnd) = self.window_handle
            && make_popup_window(hwnd).is_ok()
        {
            self.window_styled = true;
        }
    }

    fn drain_provider(&mut self) {
        while let Some(event) = self.provider.try_recv() {
            match event {
                ProviderEvent::Loaded { request_id, result } => {
                    if request_id != self.request_id {
                        continue;
                    }
                    self.loading = false;
                    match result {
                        Ok(page) => {
                            self.service_error = None;
                            self.items = page.items;
                            self.total = page.total;
                            self.loaded_query = self.query.clone();
                            self.clamp_selection();
                            if self.selection.primary.is_none() && !self.items.is_empty() {
                                self.selection.primary = Some(0);
                                self.selection.anchor = Some(0);
                            }
                            if self.options.capture.is_some() {
                                self.advance_capture_after_load();
                            }
                        }
                        Err(error) => {
                            self.loading = false;
                            self.service_error = Some(error.to_string());
                            self.status = Some(StatusMessage {
                                tone: StatusTone::Error,
                                text: self.locale.text(key::STATUS_SEARCH_FAILED).to_string(),
                                detail: Some(error.to_string()),
                            });
                            if self.options.capture.is_some() {
                                self.advance_capture_after_load();
                            }
                        }
                    }
                }
                ProviderEvent::Metadata(result) => match result {
                    Ok(metadata) => self.apply_metadata(metadata),
                    Err(error) => {
                        if self.visible {
                            self.service_error = Some(error.to_string());
                        }
                    }
                },
                ProviderEvent::ActionDone { action, result } => {
                    self.handle_action_done(action, result);
                }
            }
        }
    }

    fn apply_metadata(&mut self, metadata: ProviderMetadata) {
        self.groups = metadata.groups;
        self.copy_buffers = metadata.copy_buffers;
        self.apply_settings_snapshot(&metadata.settings);
        self.service_error = None;
    }

    fn apply_settings_snapshot(&mut self, snapshot: &SettingsSnapshot) {
        let Some(popup) = snapshot.values.get("popup") else {
            return;
        };
        if let Some(value) = popup.get("width").and_then(serde_json::Value::as_f64) {
            self.settings.width = value as f32;
        }
        if let Some(value) = popup.get("height").and_then(serde_json::Value::as_f64) {
            self.settings.height = value as f32;
        }
        if let Some(value) = popup
            .get("hide_on_focus_loss")
            .and_then(serde_json::Value::as_bool)
        {
            self.settings.hide_on_focus_loss = value;
        }
        if self.options.capture.is_some()
            || self.options.soak_cycles.is_some()
            || self.options.bench_scroll
        {
            self.settings.hide_on_focus_loss = false;
        }
        if let Some(value) = popup.get("position").and_then(serde_json::Value::as_str) {
            self.settings.position = PositionMode::from_setting(value);
        }
        if let Some(value) = popup
            .get("confirm_delete")
            .and_then(serde_json::Value::as_bool)
        {
            self.settings.confirm_delete = value;
        }
        if self.options.theme_override.is_none()
            && let Some(value) = popup.get("theme").and_then(serde_json::Value::as_str)
        {
            let theme = match value {
                "light" => Some(Theme::Light),
                "dark" => Some(Theme::Dark),
                "hc" | "high_contrast" => Some(Theme::HighContrast),
                _ => None,
            };
            if let Some(theme) = theme {
                self.theme = theme;
                self.palette = Palette::for_theme(theme);
                apply_visuals(&self.ctx, theme, self.palette);
                if let Some(hwnd) = self.window_handle {
                    platform::configure_window(hwnd, theme, self.palette);
                }
            }
        }
    }

    fn handle_action_done(&mut self, action: ActionKind, result: ProviderResult<()>) {
        match result {
            Ok(()) => {
                let key = match action {
                    ActionKind::Paste => key::STATUS_PASTE_SENT,
                    ActionKind::Pin => key::STATUS_PINNED,
                    ActionKind::Move => key::STATUS_MOVED,
                    ActionKind::Delete => key::STATUS_DELETED,
                    ActionKind::AssignCopyBuffer => key::STATUS_BUFFER_ASSIGNED,
                    ActionKind::ClearCopyBuffer => key::STATUS_BUFFER_CLEARED,
                    ActionKind::PasteCopyBuffer => key::STATUS_BUFFER_PASTED,
                };
                self.status = Some(StatusMessage {
                    tone: StatusTone::Info,
                    text: self.locale.text(key).to_string(),
                    detail: None,
                });
                if matches!(
                    action,
                    ActionKind::Pin
                        | ActionKind::Move
                        | ActionKind::Delete
                        | ActionKind::AssignCopyBuffer
                        | ActionKind::ClearCopyBuffer
                ) {
                    self.request_load();
                    self.provider.send(ProviderCommand::RefreshMetadata);
                }
            }
            Err(error) => {
                self.service_error = error.is_disconnected().then(|| error.to_string());
                self.status = Some(StatusMessage {
                    tone: StatusTone::Error,
                    text: if error.is_unsupported() {
                        self.locale.text(key::STATUS_UNSUPPORTED).to_string()
                    } else {
                        self.locale.text(key::STATUS_FAILED).to_string()
                    },
                    detail: Some(error.to_string()),
                });
            }
        }
    }

    fn request_load(&mut self) {
        self.request_id = self.request_id.saturating_add(1);
        self.loading = true;
        self.search_due = None;
        let limit = if self.options.mock { 10_000 } else { 1_000 };
        self.provider.send(ProviderCommand::Load {
            request_id: self.request_id,
            query: self.query.clone(),
            group: self.current_group,
            limit,
        });
        self.request_repaint_soon();
    }

    fn schedule_search(&mut self) {
        self.search_due = Some(Instant::now() + SEARCH_DEBOUNCE);
        self.request_repaint_soon();
    }

    fn tick_search(&mut self) {
        if self
            .search_due
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.request_load();
        }
    }

    fn clamp_selection(&mut self) {
        if self.items.is_empty() {
            self.selection.clear();
            return;
        }
        let max = self.items.len() - 1;
        self.selection.primary = Some(self.selection.primary.unwrap_or(0).min(max));
        self.selection.anchor = self
            .selection
            .anchor
            .or(self.selection.primary)
            .map(|index| index.min(max));
        let valid_ids: BTreeSet<_> = self.items.iter().map(|clip| clip.id).collect();
        self.selection.multi.retain(|id| valid_ids.contains(id));
    }

    fn process_ime_state(&mut self) {
        let events = self.ctx.input(|input| input.events.clone());
        self.ime_composing = ime_composing_after_events(self.ime_composing, &events);
    }

    fn process_screenshot_event(&mut self) {
        let screenshot = self.ctx.input(|input| {
            input.events.iter().find_map(|event| {
                if let EguiEvent::Screenshot {
                    user_data, image, ..
                } = event
                {
                    let token = user_data
                        .data
                        .as_ref()
                        .and_then(|data| data.downcast_ref::<String>())?;
                    (token == "ditto-popup-capture").then(|| Arc::clone(image))
                } else {
                    None
                }
            })
        });
        let Some(image) = screenshot else {
            return;
        };
        let Some(capture) = self.capture.as_ref() else {
            return;
        };
        let path = capture.options.path.clone();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let result = image::save_buffer(
            &path,
            image.as_raw(),
            image.width() as u32,
            image.height() as u32,
            image::ExtendedColorType::Rgba8,
        );
        if let Err(error) = result {
            if let Some(parent) = path.parent() {
                let _ = fs::write(parent.join("capture-error.txt"), error.to_string());
            }
        }
        self.trace_capture("screenshot received and saved");
        self.ctx.send_viewport_cmd(ViewportCommand::Close);
        self.ctx.request_repaint();
    }

    fn activate_for_user_if_needed(&mut self) {
        if !self.visible || self.user_activated {
            return;
        }
        if self.ctx.input(|input| input.pointer.any_pressed())
            && let Some(hwnd) = self.window_handle
        {
            platform::activate_for_user(hwnd, self.previous_foreground);
            self.user_activated = true;
            self.ctx.memory_mut(|memory| {
                memory.request_focus(Self::search_id());
            });
        }
    }

    fn maybe_hide_after_focus_loss(&mut self) {
        if !self.visible || !self.settings.hide_on_focus_loss {
            return;
        }
        if self.user_activated {
            let focused = self
                .ctx
                .input(|input| input.viewport().focused.unwrap_or(false));
            if !focused {
                self.hide_popup();
            }
            return;
        }
        let enough_time = self
            .shown_at
            .is_none_or(|shown| shown.elapsed() > Duration::from_millis(100));
        if enough_time
            && self
                .window_handle
                .is_some_and(platform::primary_click_outside)
        {
            self.hide_popup();
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        if !self.visible {
            return;
        }
        if self.pending_delete.is_some() {
            if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Escape)) {
                self.pending_delete = None;
            }
            return;
        }
        if self.ime_composing {
            return;
        }

        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Escape)) {
            if !self.query.is_empty() {
                self.query.clear();
                self.schedule_search();
            } else if self.preview_open {
                self.preview_open = false;
            } else {
                self.hide_popup();
            }
            return;
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Tab)) {
            self.cycle_focus(false);
            return;
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::SHIFT, egui::Key::Tab)) {
            self.cycle_focus(true);
            return;
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::F3)) {
            self.preview_open = !self.preview_open;
            return;
        }

        let modifiers = ctx.input(|input| input.modifiers);
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::ArrowUp)) {
            self.move_selection(-1, modifiers.shift);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::ArrowDown)) {
            self.move_selection(1, modifiers.shift);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::PageUp)) {
            self.move_selection(-(self.page_rows() as isize), modifiers.shift);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::PageDown)) {
            self.move_selection(self.page_rows() as isize, modifiers.shift);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Home)) {
            self.set_selection_index(0, modifiers.shift);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::End)) {
            self.set_selection_index(self.items.len().saturating_sub(1), modifiers.shift);
        }
        if self.focus_target == FocusTarget::List
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Space))
        {
            self.toggle_current_selection();
        }
        if self.focus_target == FocusTarget::List
            && ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, egui::Key::A))
        {
            self.select_all_visible();
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, egui::Key::F)) {
            self.focus_target = FocusTarget::Search;
            ctx.memory_mut(|memory| memory.request_focus(Self::search_id()));
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, egui::Key::P)) {
            self.toggle_pin_selected();
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Delete)) {
            self.request_delete_selected();
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, egui::Key::Enter)) {
            self.paste_selected(PasteMode::Original);
        }
        if ctx.input_mut(|input| input.consume_key(Modifiers::SHIFT, egui::Key::Enter)) {
            self.paste_selected(PasteMode::PlainText);
        }

        let shift_control = Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        for (key, slot) in number_keys() {
            if ctx.input_mut(|input| input.consume_key(shift_control, key)) {
                self.assign_buffer(slot);
            }
        }
        let alt = Modifiers {
            alt: true,
            ..Default::default()
        };
        for (key, slot) in number_keys().into_iter().take(5) {
            if ctx.input_mut(|input| input.consume_key(alt, key)) {
                self.paste_buffer(slot, false);
            }
        }
        for (key, number) in number_keys() {
            if ctx.input_mut(|input| input.consume_key(Modifiers::CTRL, key)) {
                self.paste_first_ten(number);
            }
        }
    }

    fn cycle_focus(&mut self, reverse: bool) {
        self.focus_target = match (self.focus_target, reverse) {
            (FocusTarget::Title, false) => FocusTarget::List,
            (FocusTarget::List, false) => FocusTarget::Search,
            (FocusTarget::Search, false) => FocusTarget::Title,
            (FocusTarget::Title, true) => FocusTarget::Search,
            (FocusTarget::List, true) => FocusTarget::Title,
            (FocusTarget::Search, true) => FocusTarget::List,
        };
        match self.focus_target {
            FocusTarget::Search => self.ctx.memory_mut(|memory| {
                memory.request_focus(Self::search_id());
            }),
            FocusTarget::Title | FocusTarget::List => {
                self.ctx.memory_mut(|memory| {
                    memory.surrender_focus(Self::search_id());
                });
            }
        }
    }

    fn page_rows(&self) -> usize {
        self.rendered_rows.max(1)
    }

    fn move_selection(&mut self, delta: isize, extend: bool) {
        if self.items.is_empty() {
            return;
        }
        let current = self.selection.primary.unwrap_or(0);
        let target = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current
                .saturating_add(delta as usize)
                .min(self.items.len() - 1)
        };
        self.set_selection_index(target, extend);
    }

    fn set_selection_index(&mut self, index: usize, extend: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        if extend {
            let anchor = self
                .selection
                .anchor
                .unwrap_or(self.selection.primary.unwrap_or(index));
            self.selection.anchor = Some(anchor);
            let start = anchor.min(index);
            let end = anchor.max(index);
            self.selection.multi = self.items[start..=end].iter().map(|clip| clip.id).collect();
        } else {
            self.selection.multi.clear();
            self.selection.anchor = Some(index);
        }
        self.selection.primary = Some(index);
        self.pending_scroll = Some(index);
    }

    fn select_clicked(&mut self, index: usize, modifiers: Modifiers) {
        if modifiers.shift {
            self.set_selection_index(index, true);
            return;
        }
        if modifiers.ctrl || modifiers.command {
            if let Some(clip) = self.items.get(index) {
                if !self.selection.multi.remove(&clip.id) {
                    self.selection.multi.insert(clip.id);
                }
                self.selection.primary = Some(index);
                self.selection.anchor = Some(index);
            }
            return;
        }
        self.selection.multi.clear();
        self.selection.primary = Some(index);
        self.selection.anchor = Some(index);
    }

    fn toggle_current_selection(&mut self) {
        let Some(index) = self.selection.primary else {
            return;
        };
        let Some(clip) = self.items.get(index) else {
            return;
        };
        if !self.selection.multi.remove(&clip.id) {
            self.selection.multi.insert(clip.id);
        }
        self.selection.anchor = Some(index);
    }

    fn select_all_visible(&mut self) {
        self.selection.multi = self.items.iter().map(|clip| clip.id).collect();
    }

    fn selected_ids(&self) -> Vec<i64> {
        self.selection.ids(&self.items)
    }

    fn paste_selected(&mut self, mode: PasteMode) {
        let ids = self.selected_ids();
        for clip_id in ids {
            self.provider
                .send(ProviderCommand::Action(ProviderAction::Paste {
                    clip_id,
                    mode,
                }));
        }
    }

    fn paste_first_ten(&mut self, number: u8) {
        let index = first_ten_index(number);
        if let Some(clip) = self.items.get(index) {
            let clip_id = clip.id;
            self.provider
                .send(ProviderCommand::Action(ProviderAction::Paste {
                    clip_id,
                    mode: PasteMode::Original,
                }));
        }
    }

    fn toggle_pin_selected(&mut self) {
        let Some(index) = self.selection.primary else {
            return;
        };
        if let Some(clip) = self.items.get(index) {
            let action = ProviderAction::SetPinned {
                clip_id: clip.id,
                pinned: !clip.pinned,
            };
            self.provider.send(ProviderCommand::Action(action));
        }
    }

    fn assign_buffer(&mut self, slot: u8) {
        let Some(index) = self.selection.primary else {
            return;
        };
        if let Some(clip) = self.items.get(index) {
            self.provider
                .send(ProviderCommand::Action(ProviderAction::AssignCopyBuffer {
                    clip_id: clip.id,
                    slot,
                }));
        }
    }

    fn paste_buffer(&mut self, slot: u8, plain_text: bool) {
        self.provider
            .send(ProviderCommand::Action(ProviderAction::PasteCopyBuffer {
                slot,
                plain_text,
            }));
    }

    fn request_delete_selected(&mut self) {
        let ids = self.selected_ids();
        if ids.is_empty() {
            return;
        }
        if self.settings.confirm_delete {
            self.pending_delete = Some(ids);
        } else {
            self.provider
                .send(ProviderCommand::Action(ProviderAction::Delete {
                    clip_ids: ids,
                }));
        }
    }

    fn apply_menu_command(&mut self, command: MenuCommand) {
        match command {
            MenuCommand::Paste { index, mode } => {
                if let Some(clip) = self.items.get(index) {
                    self.provider
                        .send(ProviderCommand::Action(ProviderAction::Paste {
                            clip_id: clip.id,
                            mode,
                        }));
                }
            }
            MenuCommand::TogglePin { index, pinned } => {
                if let Some(clip) = self.items.get(index) {
                    self.provider
                        .send(ProviderCommand::Action(ProviderAction::SetPinned {
                            clip_id: clip.id,
                            pinned,
                        }));
                }
            }
            MenuCommand::Move { index, group_id } => {
                if let Some(clip) = self.items.get(index) {
                    self.provider
                        .send(ProviderCommand::Action(ProviderAction::MoveToGroup {
                            clip_id: clip.id,
                            group_id,
                        }));
                }
            }
            MenuCommand::Delete { index } => {
                if let Some(clip) = self.items.get(index) {
                    let ids = vec![clip.id];
                    if self.settings.confirm_delete {
                        self.pending_delete = Some(ids);
                    } else {
                        self.provider
                            .send(ProviderCommand::Action(ProviderAction::Delete {
                                clip_ids: ids,
                            }));
                    }
                }
            }
            MenuCommand::AssignBuffer { index, slot } => {
                if let Some(clip) = self.items.get(index) {
                    self.provider
                        .send(ProviderCommand::Action(ProviderAction::AssignCopyBuffer {
                            clip_id: clip.id,
                            slot,
                        }));
                }
            }
            MenuCommand::ClearBuffer { slot } => {
                self.provider
                    .send(ProviderCommand::Action(ProviderAction::ClearCopyBuffer {
                        slot,
                    }));
            }
            MenuCommand::PasteBuffer { slot, plain_text } => {
                self.paste_buffer(slot, plain_text);
            }
            MenuCommand::Preview { index } => {
                self.selection.primary = Some(index);
                self.selection.anchor = Some(index);
                self.preview_open = true;
            }
        }
    }

    fn draw_title(&mut self, ctx: &egui::Context) {
        let mode = self.mode();
        let mut toggle_preview = false;
        let mut toggle_always_on_top = false;
        let mut hide = false;
        let mut group_button_pos = None;
        egui::TopBottomPanel::top("popup-title")
            .exact_height(dimension::title_height(mode))
            .frame(panel_frame(self.palette.surface))
            .show(ctx, |ui| {
                ui.add_space(dimension::space_xs(mode));
                ui.horizontal_centered(|ui| {
                    ui.add_space(dimension::space_sm(mode));
                    ui.label(
                        RichText::new(self.locale.text(key::APP_NAME))
                            .size(font::body_size(mode))
                            .strong()
                            .color(self.palette.text_primary),
                    );
                    let group_response = icon_button(
                        ui,
                        dimension::control_height(mode) - dimension::space_xs(mode),
                        Icon::Folder,
                        self.palette,
                        self.group_menu.open,
                        self.locale.text(key::GROUP_CHOOSE),
                        radius::control(mode),
                    );
                    if group_response.clicked() {
                        self.group_menu.open = !self.group_menu.open;
                        self.group_menu.position = group_response.rect.left_bottom()
                            + vec2(0.0, dimension::space_xs(mode));
                        group_button_pos = Some(self.group_menu.position);
                    }
                    let path = self.group_path();
                    if !path.is_empty() {
                        ui.add(
                            Label::new(
                                RichText::new(path)
                                    .size(font::caption_size(mode))
                                    .color(self.palette.text_secondary),
                            )
                            .truncate(),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(
                            ui,
                            dimension::control_height(mode),
                            Icon::Close,
                            self.palette,
                            false,
                            self.locale.text(key::TITLE_CLOSE),
                            radius::control(mode),
                        )
                        .clicked()
                        {
                            hide = true;
                        }
                        if icon_button(
                            ui,
                            dimension::control_height(mode),
                            Icon::Eye,
                            self.palette,
                            self.preview_open,
                            self.locale.text(key::PREVIEW_TOGGLE),
                            radius::control(mode),
                        )
                        .clicked()
                        {
                            toggle_preview = true;
                        }
                        if icon_button(
                            ui,
                            dimension::control_height(mode),
                            Icon::Pin,
                            self.palette,
                            self.always_on_top,
                            self.locale.text(key::TITLE_ALWAYS_ON_TOP),
                            radius::control(mode),
                        )
                        .clicked()
                        {
                            toggle_always_on_top = true;
                        }
                        ui.add(
                            Label::new(
                                RichText::new(self.locale.text(key::SHORTCUT_HINT_COMPACT))
                                    .size(font::caption_size(mode))
                                    .color(self.palette.text_secondary),
                            )
                            .truncate(),
                        )
                        .on_hover_text(self.locale.text(key::SHORTCUT_HINT));
                    });
                });
            });
        if toggle_preview {
            self.preview_open = !self.preview_open;
        }
        if toggle_always_on_top {
            self.always_on_top = !self.always_on_top;
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }));
        }
        if hide {
            self.hide_popup();
        }
        if let Some(position) = group_button_pos {
            self.group_menu.position = position;
        }
    }

    fn group_path(&self) -> String {
        match self.current_group {
            GroupFilter::All => String::new(),
            GroupFilter::Ungrouped => self.locale.text(key::GROUP_UNGROUPED).to_string(),
            GroupFilter::Group(group_id) => {
                let mut names = Vec::new();
                let mut current = Some(group_id);
                let mut guard = 0;
                while let Some(id) = current
                    && guard < 16
                {
                    guard += 1;
                    let Some(group) = self.groups.iter().find(|group| group.id == id) else {
                        break;
                    };
                    names.push(group.name.clone());
                    current = group.parent_id;
                }
                names.reverse();
                names.join(" / ")
            }
        }
    }

    fn draw_search(&mut self, ctx: &egui::Context) {
        let mode = self.mode();
        let search_id = Self::search_id();
        let mut clear_clicked = false;
        let count_text = if self.loading && self.items.is_empty() {
            self.locale.text(key::SEARCHING).to_string()
        } else {
            self.locale.format(
                key::SEARCH_RESULT_COUNT,
                &[("count", &self.total.to_string())],
            )
        };
        egui::TopBottomPanel::bottom("popup-search")
            .exact_height(dimension::search_height(mode))
            .frame(panel_frame(self.palette.surface))
            .show(ctx, |ui| {
                ui.add_space(dimension::space_sm(mode));
                ui.horizontal_centered(|ui| {
                    ui.add_space(dimension::space_sm(mode));
                    let (icon_rect, _) = ui.allocate_exact_size(
                        vec2(dimension::space_lg(mode), dimension::space_lg(mode)),
                        Sense::hover(),
                    );
                    paint_icon(
                        ui,
                        Icon::Search,
                        icon_rect,
                        self.palette.text_secondary,
                        border::width(mode),
                    );
                    let count_width = dimension::space_lg(mode) * 5.0;
                    let clear_width = if self.query.is_empty() {
                        0.0
                    } else {
                        dimension::control_height(mode)
                    };
                    let edit_width = (ui.available_width()
                        - count_width
                        - clear_width
                        - dimension::space_sm(mode))
                    .max(dimension::space_lg(mode) * 4.0);
                    let text_edit = egui::TextEdit::singleline(&mut self.query)
                        .id(search_id)
                        .hint_text(self.locale.text(key::SEARCH_PLACEHOLDER))
                        .margin(Margin::symmetric(
                            dimension::space_sm(mode) as i8,
                            dimension::space_xs(mode) as i8,
                        ));
                    let response =
                        ui.add_sized([edit_width, dimension::control_height(mode)], text_edit);
                    if response.changed() {
                        self.schedule_search();
                    }
                    if response.has_focus() {
                        ui.painter().rect_stroke(
                            response.rect,
                            radius::control(mode),
                            Stroke::new(border::focus_width(mode), self.palette.focus),
                            StrokeKind::Outside,
                        );
                    }
                    if !self.query.is_empty()
                        && icon_button(
                            ui,
                            dimension::control_height(mode),
                            Icon::Close,
                            self.palette,
                            false,
                            self.locale.text(key::SEARCH_CLEAR),
                            radius::control(mode),
                        )
                        .clicked()
                    {
                        clear_clicked = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(dimension::space_sm(mode));
                        ui.add(
                            Label::new(
                                RichText::new(count_text)
                                    .size(font::caption_size(mode))
                                    .color(self.palette.text_secondary),
                            )
                            .truncate(),
                        );
                    });
                });
            });
        if clear_clicked {
            self.query.clear();
            self.schedule_search();
            ctx.memory_mut(|memory| memory.request_focus(search_id));
        }
    }

    fn draw_status(&mut self, ctx: &egui::Context) {
        let mode = self.mode();
        let error = self.service_error.clone();
        let status = self.status.as_ref().map(|status| StatusMessage {
            tone: status.tone,
            text: status.text.clone(),
            detail: status.detail.clone(),
        });
        if error.is_none() && status.is_none() {
            return;
        }
        let mut retry = false;
        let mut dismiss = false;
        egui::TopBottomPanel::bottom("popup-status")
            .exact_height(font::body_line_height(mode) + dimension::space_lg(mode))
            .frame(panel_frame(self.palette.surface_alt))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(dimension::space_md(mode));
                    if let Some(error) = error {
                        ui.label(
                            RichText::new(self.locale.text(key::STATE_ERROR_TITLE))
                                .size(font::caption_size(mode))
                                .strong()
                                .color(self.palette.danger),
                        )
                        .on_hover_text(error);
                        if ui.button(self.locale.text(key::STATE_RETRY)).clicked() {
                            retry = true;
                        }
                    } else if let Some(status) = status {
                        let color = if status.tone == StatusTone::Error {
                            self.palette.danger
                        } else {
                            self.palette.text_primary
                        };
                        let response = ui.label(
                            RichText::new(status.text)
                                .size(font::caption_size(mode))
                                .color(color),
                        );
                        if let Some(detail) = status.detail {
                            response.on_hover_text(detail);
                        }
                        if ui.button(self.locale.text(key::DELETE_CANCEL)).clicked() {
                            dismiss = true;
                        }
                    }
                });
            });
        if retry {
            self.provider.send(ProviderCommand::RefreshMetadata);
            self.request_load();
        }
        if dismiss {
            self.status = None;
        }
    }

    fn draw_preview(&mut self, ctx: &egui::Context) {
        if !self.preview_open {
            return;
        }
        let mode = self.mode();
        let Some(clip) = self
            .selection
            .primary
            .and_then(|index| self.items.get(index))
            .cloned()
        else {
            return;
        };
        let mut close = false;
        egui::TopBottomPanel::bottom("popup-preview")
            .exact_height(dimension::row_height(mode) * 3.0)
            .frame(
                panel_frame(self.palette.canvas)
                    .stroke(Stroke::new(border::width(mode), self.palette.border)),
            )
            .show(ctx, |ui| {
                ui.add_space(dimension::space_sm(mode));
                ui.horizontal(|ui| {
                    ui.add_space(dimension::space_md(mode));
                    ui.label(
                        RichText::new(self.locale.text(key::PREVIEW_TITLE))
                            .size(font::caption_size(mode))
                            .strong()
                            .color(self.palette.text_secondary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(dimension::space_sm(mode));
                        if icon_button(
                            ui,
                            dimension::control_height(mode),
                            Icon::Close,
                            self.palette,
                            false,
                            self.locale.text(key::PREVIEW_CLOSE),
                            radius::control(mode),
                        )
                        .clicked()
                        {
                            close = true;
                        }
                    });
                });
                ui.horizontal_top(|ui| {
                    ui.add_space(dimension::space_md(mode));
                    match clip.kind {
                        ClipKind::Image => {
                            draw_image_placeholder(ui, self.palette, mode);
                            ui.add_space(dimension::space_md(mode));
                            ui.add(
                                Label::new(
                                    RichText::new(self.locale.text(key::PREVIEW_IMAGE))
                                        .size(font::popup_body_size(mode))
                                        .color(self.palette.text_secondary),
                                )
                                .wrap(),
                            );
                        }
                        ClipKind::Files => {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(self.locale.text(key::PREVIEW_FILES))
                                        .size(font::caption_size(mode))
                                        .strong()
                                        .color(self.palette.text_secondary),
                                );
                                for line in clip.preview_text.lines().take(4) {
                                    ui.add(
                                        Label::new(
                                            RichText::new(line)
                                                .size(font::caption_size(mode))
                                                .color(self.palette.text_primary),
                                        )
                                        .truncate(),
                                    );
                                }
                            });
                        }
                        ClipKind::Rich => {
                            let summary = html_summary(&clip.preview_text);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(self.locale.text(key::PREVIEW_RICH))
                                        .size(font::caption_size(mode))
                                        .strong()
                                        .color(self.palette.text_secondary),
                                );
                                ui.add(
                                    Label::new(
                                        RichText::new(summary)
                                            .size(font::popup_body_size(mode))
                                            .color(self.palette.text_primary),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                        _ => {
                            if let Some(swatch) = color_hex(&clip.preview_text) {
                                let (rect, _) = ui.allocate_exact_size(
                                    vec2(
                                        dimension::row_height(mode) * 2.0,
                                        dimension::row_height(mode) * 2.0,
                                    ),
                                    Sense::hover(),
                                );
                                ui.painter()
                                    .rect_filled(rect, radius::list_item(mode), swatch);
                                ui.painter().rect_stroke(
                                    rect,
                                    radius::list_item(mode),
                                    Stroke::new(border::width(mode), self.palette.border_strong),
                                    StrokeKind::Inside,
                                );
                                ui.add_space(dimension::space_md(mode));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(self.locale.text(key::PREVIEW_COLOR))
                                            .size(font::caption_size(mode))
                                            .color(self.palette.text_secondary),
                                    );
                                    ui.label(
                                        RichText::new(
                                            clip.preview_text
                                                .split_whitespace()
                                                .find(|part| part.starts_with('#'))
                                                .unwrap_or(&clip.preview_text),
                                        )
                                        .size(font::popup_body_size(mode))
                                        .color(self.palette.text_primary),
                                    );
                                });
                            } else {
                                ui.add(
                                    Label::new(
                                        RichText::new(truncate_text(&clip.preview_text))
                                            .size(font::popup_body_size(mode))
                                            .color(self.palette.text_primary),
                                    )
                                    .wrap(),
                                );
                            }
                        }
                    }
                });
            });
        if close {
            self.preview_open = false;
        }
    }

    fn draw_list(&mut self, ctx: &egui::Context) {
        let mode = self.mode();
        egui::CentralPanel::default()
            .frame(panel_frame(self.palette.canvas))
            .show(ctx, |ui| {
                if self.items.is_empty() {
                    if self.loading {
                        draw_skeleton(ui, self.palette, mode);
                    } else {
                        draw_empty(ui, &self.locale, self.palette, mode, !self.query.is_empty());
                    }
                    return;
                }
                let row_height = clip_row_height(mode);
                let viewport_height = ui.available_height();
                let mut interaction = None;
                let mut menu_command = None;
                let items = &self.items;
                let selection = &self.selection;
                let context = RowContext {
                    locale: &self.locale,
                    palette: self.palette,
                    mode,
                    groups: &self.groups,
                    copy_buffers: &self.copy_buffers,
                    keyboard_focus: self.focus_target == FocusTarget::List,
                };
                let pending_scroll = self.pending_scroll.take();
                let mut rendered_rows = 0;
                let mut scroll = ScrollArea::vertical();
                if let Some(target) = pending_scroll {
                    let offset = (target as f32 * row_height - viewport_height * 0.5).max(0.0);
                    scroll = scroll.vertical_scroll_offset(offset);
                } else if let Some(bench) = &self.bench_scroll {
                    scroll = scroll.vertical_scroll_offset(bench.offset);
                }
                scroll.show_rows(ui, row_height, items.len(), |ui, range| {
                    rendered_rows = range.len();
                    for index in range {
                        let clip = &items[index];
                        let result = draw_row(
                            ui,
                            clip,
                            index,
                            row_height,
                            selection.is_selected(index, clip),
                            selection.primary == Some(index),
                            &context,
                        );
                        if result.interaction.is_some() {
                            interaction = result.interaction;
                        }
                        if result.menu.is_some() {
                            menu_command = result.menu;
                        }
                        if result.scroll_to {
                            ui.scroll_to_rect(
                                egui::Rect::from_min_size(
                                    ui.min_rect().min,
                                    vec2(ui.available_width(), row_height),
                                ),
                                Some(Align::Center),
                            );
                        }
                    }
                });
                self.rendered_rows = rendered_rows;
                if let Some(interaction) = interaction {
                    match interaction {
                        ListInteraction::Select { index, modifiers } => {
                            self.select_clicked(index, modifiers);
                        }
                        ListInteraction::Paste { index, plain_text } => {
                            self.selection.primary = Some(index);
                            self.paste_selected(if plain_text {
                                PasteMode::PlainText
                            } else {
                                PasteMode::Original
                            });
                        }
                    }
                }
                if let Some(command) = menu_command {
                    self.apply_menu_command(command);
                }
            });
    }

    fn draw_group_menu(&mut self, ctx: &egui::Context) {
        if !self.group_menu.open {
            return;
        }
        let mode = self.mode();
        let mut selected = None;
        let response = egui::Area::new(Id::new("ditto-group-menu"))
            .fixed_pos(self.group_menu.position)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(self.palette.surface)
                    .stroke(Stroke::new(border::width(mode), self.palette.border_strong))
                    .inner_margin(Margin::same(dimension::space_xs(mode) as i8))
                    .show(ui, |ui| {
                        ui.set_max_height(dimension::row_height(mode) * 8.0);
                        ScrollArea::vertical().show(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.current_group == GroupFilter::All,
                                    self.locale.text(key::GROUP_ALL),
                                )
                                .clicked()
                            {
                                selected = Some(GroupFilter::All);
                            }
                            if ui
                                .selectable_label(
                                    self.current_group == GroupFilter::Ungrouped,
                                    self.locale.text(key::GROUP_UNGROUPED),
                                )
                                .clicked()
                            {
                                selected = Some(GroupFilter::Ungrouped);
                            }
                            for group in &self.groups {
                                let filter = GroupFilter::Group(group.id);
                                if ui
                                    .selectable_label(self.current_group == filter, &group.name)
                                    .clicked()
                                {
                                    selected = Some(filter);
                                }
                            }
                        });
                    });
            });
        let popup_rect = response.response.rect;
        if ctx.input(|input| input.pointer.any_click())
            && ctx
                .input(|input| input.pointer.interact_pos())
                .is_some_and(|position| !popup_rect.contains(position))
        {
            self.group_menu.open = false;
        }
        if let Some(group) = selected {
            self.current_group = group;
            self.group_menu.open = false;
            self.selection.clear();
            self.request_load();
        }
    }

    fn draw_delete_modal(&mut self, ctx: &egui::Context) {
        let Some(ids) = self.pending_delete.clone() else {
            return;
        };
        let mode = self.mode();
        let mut confirm = false;
        let mut cancel = false;
        let response = egui::Modal::new(Id::new("ditto-delete-modal"))
            .backdrop_color(self.palette.text_primary.gamma_multiply(0.35))
            .frame(
                egui::Frame::popup(&ctx.style())
                    .fill(self.palette.surface)
                    .stroke(Stroke::new(border::width(mode), self.palette.border_strong))
                    .inner_margin(Margin::same(dimension::space_lg(mode) as i8)),
            )
            .show(ctx, |ui| {
                ui.set_max_width(dimension::popup_width(mode) * 0.8);
                ui.label(
                    RichText::new(self.locale.text(key::DELETE_TITLE))
                        .size(font::body_size(mode))
                        .strong()
                        .color(self.palette.text_primary),
                );
                ui.add_space(dimension::space_sm(mode));
                ui.label(
                    RichText::new(
                        self.locale
                            .format(key::DELETE_BODY, &[("count", &ids.len().to_string())]),
                    )
                    .size(font::popup_body_size(mode))
                    .color(self.palette.text_secondary),
                );
                ui.add_space(dimension::space_md(mode));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(self.locale.text(key::DELETE_CONFIRM)).clicked() {
                        confirm = true;
                    }
                    if ui.button(self.locale.text(key::DELETE_CANCEL)).clicked() {
                        cancel = true;
                    }
                });
            });
        if response.should_close() || cancel {
            self.pending_delete = None;
        } else if confirm {
            self.pending_delete = None;
            self.provider
                .send(ProviderCommand::Action(ProviderAction::Delete {
                    clip_ids: ids,
                }));
        }
    }

    fn advance_capture_after_load(&mut self) {
        let waiting_data = self
            .capture
            .as_ref()
            .is_some_and(|capture| matches!(capture.stage, CaptureStage::WaitingData));
        if waiting_data {
            let state = self
                .capture
                .as_ref()
                .map(|capture| capture.options.state)
                .unwrap_or(CaptureState::Default);
            let needs_reload = prepare_capture_state(self, state);
            if let Some(capture) = self.capture.as_mut() {
                capture.stage = if needs_reload {
                    CaptureStage::WaitingReload
                } else {
                    CaptureStage::Ready(Instant::now() + Duration::from_millis(250))
                };
            }
            self.trace_capture(if needs_reload {
                "capture state configured; waiting for reload"
            } else {
                "capture state configured; waiting to request screenshot"
            });
            if needs_reload {
                self.request_load();
            }
            return;
        }
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| matches!(capture.stage, CaptureStage::WaitingReload))
            && let Some(capture) = self.capture.as_mut()
        {
            capture.stage = CaptureStage::Ready(Instant::now() + Duration::from_millis(250));
            self.trace_capture("capture reload complete; waiting to request screenshot");
        }
    }

    fn tick_capture(&mut self) {
        let Some(capture) = self.capture.as_mut() else {
            return;
        };
        let request = match capture.stage {
            CaptureStage::Ready(deadline) => Instant::now() >= deadline,
            CaptureStage::Requested(sent_at) => sent_at.elapsed() > Duration::from_secs(2),
            CaptureStage::WaitingData | CaptureStage::WaitingReload => false,
        };
        if let CaptureStage::Ready(deadline) = capture.stage
            && Instant::now() < deadline
        {
            self.ctx
                .request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        if request {
            capture.stage = CaptureStage::Requested(Instant::now());
            self.trace_capture("requesting viewport screenshot");
            self.ctx
                .send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(
                    "ditto-popup-capture".to_string(),
                )));
            self.ctx.request_repaint();
        } else if matches!(capture.stage, CaptureStage::Requested(_)) {
            self.ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn tick_soak(&mut self) {
        let Some(mut soak) = self.soak.take() else {
            return;
        };
        let Some(hwnd) = self.window_handle else {
            self.soak = None;
            self.ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        };
        self.trace_soak(&soak, "begin-resource-cycle");
        ditto_win::set_popup_visible(hwnd, false);
        for cycle in 0..soak.total {
            ditto_win::set_popup_visible(hwnd, true);
            if let Some(sample) = platform::sample_process() {
                soak.samples.push(SoakSample {
                    cycle: cycle + 1,
                    phase: "shown",
                    gdi_objects: sample.gdi_objects,
                    user_objects: sample.user_objects,
                    private_bytes: sample.private_bytes,
                    working_set_bytes: sample.working_set_bytes,
                });
            }
            ditto_win::set_popup_visible(hwnd, false);
            if let Some(sample) = platform::sample_process() {
                soak.samples.push(SoakSample {
                    cycle: cycle + 1,
                    phase: "hidden",
                    gdi_objects: sample.gdi_objects,
                    user_objects: sample.user_objects,
                    private_bytes: sample.private_bytes,
                    working_set_bytes: sample.working_set_bytes,
                });
            }
            thread::sleep(Duration::from_millis(5));
        }
        let path = soak.log_path.clone();
        write_soak_log(&path, &soak.samples);
        self.trace_soak(&soak, "resource-cycle-complete; closing");
        self.ctx.send_viewport_cmd(ViewportCommand::Close);
        self.ctx.request_repaint();
    }

    fn tick_bench_scroll(&mut self, update_started: Instant) {
        let row_height = clip_row_height(self.mode());
        let item_count = self.items.len();
        let Some(bench) = self.bench_scroll.as_mut() else {
            return;
        };
        if !self.visible || self.items.is_empty() {
            return;
        }
        let dt_ms = self.ctx.input(|input| input.stable_dt).mul_add(1000.0, 0.0);
        let update_ms = update_started.elapsed().as_secs_f32() * 1000.0;
        bench.rows.push((
            bench.frames,
            dt_ms,
            update_ms,
            bench.offset,
            self.rendered_rows,
        ));
        bench.frames += 1;
        bench.offset += row_height * 8.0;
        let max_offset = (item_count as f32 * row_height).max(0.0);
        if bench.offset > max_offset {
            bench.offset = 0.0;
        }
        if bench.frames >= self.options.bench_frames {
            let path = bench.log_path.clone();
            let rows = std::mem::take(&mut bench.rows);
            write_scroll_log(&path, &rows);
            self.bench_scroll = None;
            self.ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        self.ctx.request_repaint();
    }

    fn record_first_frame(&mut self) {
        if !self.visible || self.frame_logged_for_show {
            return;
        }
        let Some(received_at) = self.show_received_at else {
            return;
        };
        self.frame_logged_for_show = true;
        let latency_us = received_at.elapsed().as_micros();
        if let Some(parent) = self.perf_log.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.perf_log)
            .and_then(|mut file| {
                writeln!(
                    file,
                    "{},{},{},{}",
                    unix_millis(),
                    latency_us,
                    self.ctx.pixels_per_point(),
                    self.provider.name()
                )
            });
        if self.options.latency_only {
            self.ctx.send_viewport_cmd(ViewportCommand::Close);
            self.ctx.request_repaint();
        }
    }

    fn request_repaint_soon(&self) {
        if let Some(deadline) = self.search_due {
            let wait = deadline.saturating_duration_since(Instant::now());
            self.ctx.request_repaint_after(wait);
        } else {
            self.ctx.request_repaint();
        }
    }

    fn trace_capture(&self, message: &str) {
        let Some(capture) = self.capture.as_ref() else {
            return;
        };
        let path = capture.options.path.with_extension("trace.log");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{} {message}", unix_millis()));
    }

    fn trace_soak(&self, soak: &SoakRuntime, message: &str) {
        let path = soak.log_path.with_extension("trace.log");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| {
                writeln!(
                    file,
                    "{} total={} phase={message}",
                    unix_millis(),
                    soak.total
                )
            });
    }
}

impl eframe::App for PopupApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = Instant::now();
        let external_dt = now.saturating_duration_since(self.last_frame_at);
        self.last_frame_at = now;
        if self.window_handle.is_none() {
            self.window_handle = native_window(frame);
        }
        self.style_window_once();
        if let Some(scale) = self.options.dpi_scale_override {
            ctx.set_pixels_per_point(scale);
        }
        self.process_signals();
        self.process_screenshot_event();
        self.drain_provider();
        self.process_ime_state();
        self.tick_search();
        self.activate_for_user_if_needed();
        self.maybe_hide_after_focus_loss();
        self.handle_keyboard(ctx);
        self.tick_capture();
        self.tick_soak();
        self.tick_bench_scroll(now);
        self.enforce_hidden_unless_requested();

        apply_visuals(ctx, self.theme, self.palette);
        self.draw_title(ctx);
        self.draw_search(ctx);
        self.draw_status(ctx);
        self.draw_preview(ctx);
        self.draw_list(ctx);
        self.draw_group_menu(ctx);
        self.draw_delete_modal(ctx);
        self.record_first_frame();

        if let Some(bench) = &mut self.bench_scroll
            && !bench.rows.is_empty()
            && external_dt > Duration::from_millis(200)
        {
            let last = bench.rows.last_mut();
            if let Some(last) = last {
                last.1 = external_dt.as_secs_f32() * 1000.0;
            }
        }
        if !self.visible {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn draw_row(
    ui: &mut Ui,
    clip: &ClipSummary,
    index: usize,
    row_height: f32,
    selected: bool,
    primary: bool,
    context: &RowContext<'_>,
) -> RowResult {
    let mode = context.mode;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());
    if !ui.is_rect_visible(rect) {
        return RowResult {
            interaction: None,
            menu: None,
            scroll_to: false,
        };
    }
    let row_rect = rect.shrink2(vec2(dimension::space_xs(mode), border::width(mode) * 2.0));
    let fill = if selected {
        context.palette.selection
    } else if response.hovered() {
        context.palette.hover
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(row_rect, radius::list_item(mode), fill);
    }
    if context.keyboard_focus && primary {
        ui.painter().rect_stroke(
            row_rect,
            radius::list_item(mode),
            Stroke::new(border::focus_width(mode), context.palette.focus),
            StrokeKind::Inside,
        );
    }
    if clip.pasted_count > 0 {
        let bar = egui::Rect::from_min_max(
            row_rect.min,
            pos2(
                row_rect.left() + border::focus_width(mode),
                row_rect.bottom(),
            ),
        );
        ui.painter()
            .rect_filled(bar, radius::list_item(mode), context.palette.pasted);
    }

    let gutter = dimension::control_height(mode) - dimension::space_xs(mode);
    let gutter_rect = egui::Rect::from_min_size(
        row_rect.min + vec2(dimension::space_xs(mode), dimension::space_xs(mode)),
        vec2(
            gutter,
            (row_height - dimension::space_sm(mode)).max(dimension::space_sm(mode)),
        ),
    );
    if index < FIRST_TEN {
        ui.painter().rect_filled(
            gutter_rect,
            radius::list_item(mode),
            context.palette.surface_alt,
        );
        ui.painter().text(
            gutter_rect.center(),
            Align2::CENTER_CENTER,
            (index + 1).to_string(),
            FontId::proportional(font::caption_size(mode)),
            context.palette.text_secondary,
        );
    } else {
        paint_icon(
            ui,
            icon_for_kind(clip.kind),
            gutter_rect.shrink(dimension::space_xs(mode)),
            context.palette.text_secondary,
            border::width(mode),
        );
    }

    let badges_width = dimension::space_lg(mode) * 4.0;
    let text_left = gutter_rect.right() + dimension::space_sm(mode);
    let text_right = (row_rect.right() - badges_width).max(text_left);
    let text_rect = egui::Rect::from_min_max(
        pos2(text_left, row_rect.top() + dimension::space_xs(mode)),
        pos2(text_right, row_rect.bottom() - dimension::space_xs(mode)),
    );
    let body_font = FontId::proportional(font::popup_body_size(mode));
    let caption_font = FontId::proportional(font::caption_size(mode));
    let preview = if clip.kind == ClipKind::Rich {
        html_summary(&clip.preview_text)
    } else {
        truncate_text(&clip.preview_text)
    };
    let text_color = if selected {
        context.palette.selection_text
    } else {
        context.palette.text_primary
    };
    let clipped = ui.painter().with_clip_rect(text_rect);
    clipped.text(
        pos2(text_rect.left(), text_rect.top()),
        Align2::LEFT_TOP,
        preview,
        body_font,
        text_color,
    );
    let source = clip
        .source_app
        .as_deref()
        .unwrap_or_else(|| context.locale.text(key::SOURCE_UNKNOWN));
    let meta = format!(
        "{}  |  {}  |  {}",
        source,
        relative_time(context.locale, clip.created_at_ms),
        format_label(context.locale, clip.kind)
    );
    clipped.text(
        pos2(text_rect.left(), text_rect.bottom()),
        Align2::LEFT_BOTTOM,
        meta,
        caption_font,
        context.palette.text_secondary,
    );

    let mut badge_x = row_rect.right() - dimension::space_xs(mode);
    if let Some(slot) = clip.copy_buffer {
        badge_x = draw_badge(
            ui,
            badge_x,
            row_rect.center().y,
            &format!("B{slot}"),
            &BadgeStyle {
                color: context.palette.accent,
                fill: context.palette.surface,
                tooltip: &context
                    .locale
                    .format(key::BADGE_COPY_BUFFER, &[("slot", &slot.to_string())]),
                mode,
            },
        );
    }
    if clip.pinned {
        let icon_rect = egui::Rect::from_center_size(
            pos2(badge_x - dimension::space_sm(mode), row_rect.center().y),
            vec2(dimension::space_lg(mode), dimension::space_lg(mode)),
        );
        paint_icon(
            ui,
            Icon::Pin,
            icon_rect,
            context.palette.pinned,
            border::width(mode),
        );
        ui.interact(icon_rect, Id::new(("row-pin", clip.id)), Sense::hover())
            .on_hover_text(context.locale.text(key::BADGE_PINNED));
        badge_x = icon_rect.left() - dimension::space_xs(mode);
    }
    if clip.pasted_count > 0 {
        let icon_rect = egui::Rect::from_center_size(
            pos2(badge_x - dimension::space_sm(mode), row_rect.center().y),
            vec2(dimension::space_lg(mode), dimension::space_lg(mode)),
        );
        paint_icon(
            ui,
            Icon::Check,
            icon_rect,
            context.palette.pasted,
            border::width(mode),
        );
        ui.interact(icon_rect, Id::new(("row-pasted", clip.id)), Sense::hover())
            .on_hover_text(context.locale.text(key::BADGE_PASTED));
    }

    let modifiers = ui.input(|input| input.modifiers);
    let mut menu = None;
    response.context_menu(|ui| {
        menu = draw_context_menu(ui, clip, index, context);
    });
    let interaction = if response.double_clicked() {
        Some(ListInteraction::Paste {
            index,
            plain_text: modifiers.shift,
        })
    } else if response.clicked() {
        Some(ListInteraction::Select { index, modifiers })
    } else {
        None
    };
    response.widget_info(|| {
        let selected_label = if selected {
            format!("selected; {}", clip.preview_text)
        } else {
            clip.preview_text.clone()
        };
        WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, selected_label)
    });
    RowResult {
        interaction,
        menu,
        scroll_to: primary,
    }
}

fn draw_context_menu(
    ui: &mut Ui,
    clip: &ClipSummary,
    index: usize,
    context: &RowContext<'_>,
) -> Option<MenuCommand> {
    let mut command = None;
    if ui.button(context.locale.text(key::MENU_PASTE)).clicked() {
        command = Some(MenuCommand::Paste {
            index,
            mode: PasteMode::Original,
        });
        ui.close();
    }
    if ui
        .button(context.locale.text(key::MENU_PASTE_PLAIN))
        .clicked()
    {
        command = Some(MenuCommand::Paste {
            index,
            mode: PasteMode::PlainText,
        });
        ui.close();
    }
    ui.menu_button(context.locale.text(key::MENU_PASTE), |ui| {
        for (label, mode, format) in [
            (key::MENU_PASTE_HTML, PasteMode::Html, "HTML Format"),
            (key::MENU_PASTE_RTF, PasteMode::Rtf, "Rich Text Format"),
            (key::MENU_PASTE_FILES, PasteMode::Files, "CF_HDROP"),
        ] {
            let available = clip.formats.iter().any(|value| value == format);
            if ui
                .add_enabled(available, egui::Button::new(context.locale.text(label)))
                .clicked()
            {
                command = Some(MenuCommand::Paste { index, mode });
                ui.close();
            }
        }
    });
    if ui.button(context.locale.text(key::PREVIEW_OPEN)).clicked() {
        command = Some(MenuCommand::Preview { index });
        ui.close();
    }
    ui.separator();
    let pin_label = if clip.pinned {
        key::MENU_UNPIN
    } else {
        key::MENU_PIN
    };
    if ui.button(context.locale.text(pin_label)).clicked() {
        command = Some(MenuCommand::TogglePin {
            index,
            pinned: !clip.pinned,
        });
        ui.close();
    }
    ui.menu_button(context.locale.text(key::MENU_MOVE), |ui| {
        if ui
            .button(context.locale.text(key::GROUP_UNGROUPED))
            .clicked()
        {
            command = Some(MenuCommand::Move {
                index,
                group_id: None,
            });
            ui.close();
        }
        for group in context.groups {
            if ui.button(&group.name).clicked() {
                command = Some(MenuCommand::Move {
                    index,
                    group_id: Some(group.id),
                });
                ui.close();
            }
        }
    });
    ui.menu_button(context.locale.text(key::MENU_COPY_TO_BUFFER), |ui| {
        for slot in 1..=5 {
            if ui
                .button(
                    context
                        .locale
                        .format(key::MENU_BUFFER_NAME, &[("slot", &slot.to_string())]),
                )
                .clicked()
            {
                command = Some(MenuCommand::AssignBuffer { index, slot });
                ui.close();
            }
        }
    });
    ui.menu_button(context.locale.text(key::MENU_PASTE_BUFFER), |ui| {
        for slot in 1..=5 {
            let buffer = context
                .copy_buffers
                .iter()
                .find(|buffer| buffer.slot == slot);
            let label = if buffer.is_some_and(|buffer| buffer.clip_id.is_some()) {
                context
                    .locale
                    .format(key::MENU_BUFFER_NAME, &[("slot", &slot.to_string())])
            } else {
                format!(
                    "{}: {}",
                    context
                        .locale
                        .format(key::MENU_BUFFER_NAME, &[("slot", &slot.to_string())],),
                    context.locale.text(key::MENU_BUFFER_EMPTY)
                )
            };
            if ui.button(label).clicked() {
                command = Some(MenuCommand::PasteBuffer {
                    slot,
                    plain_text: false,
                });
                ui.close();
            }
            if buffer.is_some_and(|buffer| buffer.clip_id.is_some())
                && ui
                    .button(context.locale.text(key::STATUS_BUFFER_CLEARED))
                    .clicked()
            {
                command = Some(MenuCommand::ClearBuffer { slot });
                ui.close();
            }
        }
    });
    ui.separator();
    if ui
        .button(RichText::new(context.locale.text(key::MENU_DELETE)).color(context.palette.danger))
        .clicked()
    {
        command = Some(MenuCommand::Delete { index });
        ui.close();
    }
    command
}

fn draw_badge(ui: &mut Ui, right: f32, center_y: f32, text: &str, style: &BadgeStyle<'_>) -> f32 {
    let size = dimension::space_lg(style.mode);
    let rect = egui::Rect::from_center_size(pos2(right - size * 0.5, center_y), vec2(size, size));
    ui.painter()
        .rect_filled(rect, radius::list_item(style.mode), style.fill);
    ui.painter().rect_stroke(
        rect,
        radius::list_item(style.mode),
        Stroke::new(border::width(style.mode), style.color),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(font::caption_size(style.mode)),
        style.color,
    );
    ui.interact(
        rect,
        Id::new(("badge", text, rect.min.x as i32)),
        Sense::hover(),
    )
    .on_hover_text(style.tooltip);
    rect.left() - dimension::space_xs(style.mode)
}

fn draw_image_placeholder(ui: &mut Ui, palette: Palette, mode: Mode) {
    let size = dimension::row_height(mode) * 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let cells = 4;
    let cell = rect.width() / cells as f32;
    for y in 0..cells {
        for x in 0..cells {
            let fill = if (x + y) % 2 == 0 {
                palette.surface_alt
            } else {
                palette.surface
            };
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    pos2(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell),
                    vec2(cell, cell),
                ),
                0.0,
                fill,
            );
        }
    }
    paint_icon(
        ui,
        Icon::Image,
        rect.shrink(dimension::space_lg(mode)),
        palette.text_secondary,
        border::width(mode),
    );
    ui.painter().rect_stroke(
        rect,
        radius::list_item(mode),
        Stroke::new(border::width(mode), palette.border_strong),
        StrokeKind::Inside,
    );
}

fn draw_skeleton(ui: &mut Ui, palette: Palette, mode: Mode) {
    let row_height = dimension::row_height(mode);
    let count = 8;
    for index in 0..count {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
        if index % 2 == 0 {
            ui.painter().rect_filled(
                rect.shrink2(vec2(dimension::space_xs(mode), border::width(mode) * 2.0)),
                radius::list_item(mode),
                palette.surface_alt,
            );
        }
    }
}

fn draw_empty(ui: &mut Ui, locale: &Localizer, palette: Palette, mode: Mode, searching: bool) {
    ui.vertical_centered(|ui| {
        ui.add_space(dimension::row_height(mode) * 3.0);
        let (icon_rect, _) = ui.allocate_exact_size(
            vec2(
                dimension::control_height(mode),
                dimension::control_height(mode),
            ),
            Sense::hover(),
        );
        paint_icon(
            ui,
            Icon::Search,
            icon_rect,
            palette.text_secondary,
            border::width(mode),
        );
        ui.add_space(dimension::space_sm(mode));
        ui.label(
            RichText::new(locale.text(key::STATE_EMPTY_TITLE))
                .size(font::body_size(mode))
                .strong()
                .color(palette.text_primary),
        );
        ui.label(
            RichText::new(if searching {
                locale.text(key::STATE_EMPTY_SEARCH)
            } else {
                locale.text(key::STATE_EMPTY_BODY)
            })
            .size(font::caption_size(mode))
            .color(palette.text_secondary),
        );
    });
}

fn icon_for_kind(kind: ClipKind) -> Icon {
    match kind {
        ClipKind::Text => Icon::Text,
        ClipKind::Image => Icon::Image,
        ClipKind::Files => Icon::Files,
        ClipKind::Rich => Icon::Rich,
        ClipKind::Custom => Icon::Buffer,
    }
}

fn format_label(locale: &Localizer, kind: ClipKind) -> &str {
    match kind {
        ClipKind::Text => locale.text(key::FORMAT_TEXT),
        ClipKind::Image => locale.text(key::FORMAT_IMAGE),
        ClipKind::Files => locale.text(key::FORMAT_FILES),
        ClipKind::Rich => locale.text(key::FORMAT_HTML),
        ClipKind::Custom => locale.text(key::FORMAT_CUSTOM),
    }
}

fn relative_time(locale: &Localizer, timestamp_ms: i64) -> String {
    let now = unix_millis() as i64;
    let delta_minutes = ((now - timestamp_ms).max(0) / 60_000) as u64;
    if delta_minutes == 0 {
        locale.text(key::TIME_NOW).to_string()
    } else if delta_minutes < 60 {
        locale.format(key::TIME_MINUTES, &[("n", &delta_minutes.to_string())])
    } else if delta_minutes < 24 * 60 {
        locale.format(key::TIME_HOURS, &[("n", &(delta_minutes / 60).to_string())])
    } else {
        locale.format(
            key::TIME_DAYS,
            &[("n", &(delta_minutes / (24 * 60)).to_string())],
        )
    }
}

fn prepare_capture_state(app: &mut PopupApp, state: CaptureState) -> bool {
    match state {
        CaptureState::Default => {
            app.query.clear();
            app.current_group = GroupFilter::All;
            app.preview_open = false;
            app.selection.primary = Some(0);
            app.selection.anchor = Some(0);
            false
        }
        CaptureState::Empty => {
            app.query = "zzzz-no-such-clip".to_string();
            app.current_group = GroupFilter::All;
            app.preview_open = false;
            true
        }
        CaptureState::Search => {
            app.query = "Synthetic".to_string();
            app.current_group = GroupFilter::All;
            app.preview_open = false;
            true
        }
        CaptureState::Preview => {
            app.query.clear();
            app.current_group = GroupFilter::All;
            app.preview_open = true;
            app.selection.primary = Some(0);
            app.selection.anchor = Some(0);
            false
        }
        CaptureState::Groups => {
            app.query.clear();
            app.current_group = app
                .groups
                .first()
                .map_or(GroupFilter::All, |group| GroupFilter::Group(group.id));
            app.preview_open = false;
            true
        }
        CaptureState::Multi => {
            app.query.clear();
            app.current_group = GroupFilter::All;
            app.preview_open = false;
            app.selection.primary = Some(0);
            app.selection.anchor = Some(0);
            app.selection.multi = app.items.iter().take(3).map(|clip| clip.id).collect();
            false
        }
    }
}

fn number_keys() -> [(egui::Key, u8); 10] {
    [
        (egui::Key::Num1, 1),
        (egui::Key::Num2, 2),
        (egui::Key::Num3, 3),
        (egui::Key::Num4, 4),
        (egui::Key::Num5, 5),
        (egui::Key::Num6, 6),
        (egui::Key::Num7, 7),
        (egui::Key::Num8, 8),
        (egui::Key::Num9, 9),
        (egui::Key::Num0, 0),
    ]
}

fn first_ten_index(number: u8) -> usize {
    if number == 0 {
        9
    } else {
        usize::from(number - 1)
    }
}

fn ime_composing_after_events(initial: bool, events: &[EguiEvent]) -> bool {
    let mut composing = initial;
    for event in events {
        if let EguiEvent::Ime(ime) = event {
            match ime {
                ImeEvent::Preedit(text) => composing = !text.is_empty(),
                ImeEvent::Commit(_) | ImeEvent::Enabled | ImeEvent::Disabled => {
                    composing = false;
                }
            }
        }
    }
    composing
}

fn panel_frame(fill: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill)
}

fn clip_row_height(mode: Mode) -> f32 {
    dimension::row_height(mode) * 1.5
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = [
        PathBuf::from(r"C:\Windows\Fonts\msyh.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\msyhbd.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\YuGothR.ttc"),
        PathBuf::from(r"C:\Windows\Fonts\meiryo.ttc"),
    ];
    for path in candidates {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        fonts.font_data.insert(
            "ditto-cjk".to_string(),
            Arc::new(FontData::from_owned(bytes)),
        );
        if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
            proportional.insert(0, "ditto-cjk".to_string());
        }
        if let Some(monospace) = fonts.families.get_mut(&FontFamily::Monospace) {
            monospace.push("ditto-cjk".to_string());
        }
        break;
    }
    ctx.set_fonts(fonts);
}

fn apply_visuals(ctx: &egui::Context, theme: Theme, palette: Palette) {
    let mode = theme.mode();
    let mut visuals = if theme.dark() {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.canvas;
    visuals.window_fill = palette.surface;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface_alt;
    visuals.override_text_color = Some(palette.text_primary);
    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(border::width(mode), palette.selection_text);
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(border::width(mode), palette.text_primary);
    visuals.widgets.inactive.bg_fill = palette.surface_alt;
    visuals.widgets.inactive.fg_stroke = Stroke::new(border::width(mode), palette.text_primary);
    visuals.widgets.hovered.bg_fill = palette.hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(border::width(mode), palette.text_primary);
    visuals.widgets.active.bg_fill = palette.selection;
    visuals.widgets.active.fg_stroke = Stroke::new(border::width(mode), palette.selection_text);
    ctx.set_theme(if theme.dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(font::body_size(mode)));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::proportional(font::popup_body_size(mode)),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::proportional(font::caption_size(mode)),
        );
        style.spacing.item_spacing = vec2(dimension::space_sm(mode), dimension::space_xs(mode));
        style.spacing.button_padding = vec2(dimension::space_sm(mode), dimension::space_xs(mode));
    });
}

fn resize_for_options(ctx: &egui::Context, hwnd: HWND, options: &PopupOptions, theme: Theme) {
    let mode = theme.mode();
    let scale = options
        .dpi_scale_override
        .unwrap_or_else(|| f32::from(platform::dpi_for_window(hwnd) as u16) / 96.0);
    let width = (dimension::popup_width(mode) * scale).round() as i32;
    let height = (dimension::popup_height(mode) * scale).round() as i32;
    let _ = ditto_win::resize_popup(hwnd, width, height);
    if options.dpi_scale_override.is_some() {
        ctx.set_pixels_per_point(scale);
        let _ = platform::center_on_primary(hwnd, width, height);
    }
}

fn spawn_ipc_client(
    ctx: egui::Context,
    sender: Sender<PopupSignal>,
    native_handle: Arc<AtomicIsize>,
) {
    thread::Builder::new()
        .name("ditto-popup-ipc".to_string())
        .spawn(move || {
            let mut backoff = Duration::from_millis(250);
            loop {
                match connect_and_read(ctx.clone(), sender.clone(), Arc::clone(&native_handle)) {
                    Ok(()) => backoff = Duration::from_millis(250),
                    Err(_) => {
                        let _ = sender.send(PopupSignal::Connection(false));
                        ctx.request_repaint();
                        thread::sleep(backoff);
                        backoff = (backoff * 2).min(Duration::from_secs(5));
                    }
                }
            }
        })
        .expect("popup IPC thread");
}

fn connect_and_read(
    ctx: egui::Context,
    sender: Sender<PopupSignal>,
    _native_handle: Arc<AtomicIsize>,
) -> ditto_model::Result<()> {
    let name = pipe_name()?;
    let client = PipeClient::connect(&name)?;
    client.send(1, &hello_request(ClientKind::Popup))?;
    match client.read_response()? {
        Response::HelloAck { .. } => {
            let _ = sender.send(PopupSignal::Connection(true));
            ctx.request_repaint();
        }
        Response::Error { message, .. } => {
            return Err(ditto_model::DittoError::Ipc(message));
        }
        _ => {
            return Err(ditto_model::DittoError::Ipc(
                "unexpected response to Hello".to_string(),
            ));
        }
    }
    loop {
        match client.read_event()? {
            Event::ShowPopup { .. } => {
                let _ = sender.send(PopupSignal::Show {
                    received_at: Instant::now(),
                });
                ctx.request_repaint();
            }
            Event::HidePopup => {
                let _ = sender.send(PopupSignal::Hide);
                ctx.request_repaint();
            }
            Event::Shutdown { .. } => {
                let _ = sender.send(PopupSignal::Shutdown);
                ctx.request_repaint();
                return Ok(());
            }
            _ => {}
        }
    }
}

fn native_window<T: HasWindowHandle>(handle_source: &T) -> Option<HWND> {
    let handle = handle_source.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(HWND(win32.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

fn write_soak_log(path: &Path, samples: &[SoakSample]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        path,
        samples
            .iter()
            .map(|sample| {
                format!(
                    "{},{},{},{},{},{}\n",
                    sample.cycle,
                    sample.phase,
                    sample.gdi_objects,
                    sample.user_objects,
                    sample.private_bytes,
                    sample.working_set_bytes
                )
            })
            .collect::<String>(),
    );
}

fn write_scroll_log(path: &Path, rows: &[(u32, f32, f32, f32, usize)]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut output = String::from("frame,frame_ms,update_ms,scroll_offset,rendered_rows\n");
    for (frame, ms, update_ms, offset, rendered_rows) in rows {
        output.push_str(&format!(
            "{frame},{ms:.3},{update_ms:.3},{offset:.1},{rendered_rows}\n"
        ));
    }
    let _ = fs::write(path, output);
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_selection_becomes_single_item_when_multi_is_empty() {
        let items = vec![sample_clip(1), sample_clip(2)];
        let selection = Selection {
            primary: Some(1),
            anchor: Some(1),
            multi: BTreeSet::new(),
        };
        assert_eq!(selection.ids(&items), vec![2]);
    }

    #[test]
    fn multi_selection_includes_primary_once() {
        let items = vec![sample_clip(1), sample_clip(2), sample_clip(3)];
        let mut selection = Selection {
            primary: Some(1),
            anchor: Some(1),
            multi: BTreeSet::from([2, 3]),
        };
        assert_eq!(
            selection.ids(&items).into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([2, 3])
        );
        selection.multi.remove(&2);
        assert_eq!(
            selection.ids(&items).into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([2, 3])
        );
    }

    #[test]
    fn first_ten_shortcuts_map_one_through_zero() {
        assert_eq!(first_ten_index(1), 0);
        assert_eq!(first_ten_index(9), 8);
        assert_eq!(first_ten_index(0), 9);
    }

    #[test]
    fn ime_preedit_blocks_shortcuts_until_commit() {
        let preedit = [EguiEvent::Ime(ImeEvent::Preedit("ni".to_string()))];
        assert!(ime_composing_after_events(false, &preedit));
        let commit = [EguiEvent::Ime(ImeEvent::Commit("你好".to_string()))];
        assert!(!ime_composing_after_events(true, &commit));
    }

    fn sample_clip(id: i64) -> ClipSummary {
        ClipSummary {
            id,
            kind: ClipKind::Text,
            preview_text: format!("clip {id}"),
            source_app: None,
            created_at_ms: 0,
            last_pasted_at_ms: None,
            pinned: false,
            pin_order: None,
            group_id: None,
            copy_buffer: None,
            formats: vec!["CF_UNICODETEXT".to_string()],
            has_image: false,
            image_thumb_ref: None,
            pasted_count: 0,
            state_rev: 1,
        }
    }
}
