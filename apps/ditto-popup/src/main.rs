use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ditto_ipc::{ClientKind, Event, Response};
use ditto_model::ThemeMode;
use ditto_ui_tokens::{Mode, color, dimension, parse_hex, radius};
use ditto_win::{PipeClient, hello_request, make_popup_window, pipe_name, system_theme};
use eframe::egui::{self, Color32, RichText, ViewportCommand};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ditto Popup")
            .with_inner_size([
                dimension::popup_width(Mode::Light),
                dimension::popup_height(Mode::Light),
            ])
            .with_min_inner_size([360.0, 320.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_native(
        "Ditto Popup",
        options,
        Box::new(|creation| Ok(Box::new(PopupApp::new(creation)))),
    )
}

enum PopupSignal {
    Show { received_at: Instant },
    Hide,
}

struct PopupApp {
    signals: Receiver<PopupSignal>,
    ctx: egui::Context,
    visible: bool,
    show_received_at: Option<Instant>,
    frame_logged_for_show: bool,
    window_styled: bool,
    window_handle: Option<HWND>,
    theme: ThemeMode,
    query: String,
    perf_log: PathBuf,
}

impl PopupApp {
    fn new(creation: &eframe::CreationContext<'_>) -> Self {
        let theme = system_theme();
        let ctx = creation.egui_ctx.clone();
        let (sender, receiver) = mpsc::channel();
        let perf_log = perf_log_path();
        let (window_handle, window_styled) = native_window(creation)
            .and_then(|hwnd| make_popup_window(hwnd).ok().map(|()| (Some(hwnd), true)))
            .unwrap_or((None, false));
        if let Some(hwnd) = window_handle {
            let scale = creation.egui_ctx.pixels_per_point();
            let width = (dimension::popup_width(Mode::Light) * scale).round() as i32;
            let height = (dimension::popup_height(Mode::Light) * scale).round() as i32;
            let _ = ditto_win::resize_popup(hwnd, width, height);
        }
        let native_handle = Arc::new(AtomicIsize::new(
            window_handle.map_or(0, |hwnd| hwnd.0 as isize),
        ));
        spawn_ipc_client(ctx.clone(), sender, Arc::clone(&native_handle));
        ctx.set_theme(match theme {
            ThemeMode::Dark | ThemeMode::System => egui::Theme::Dark,
            ThemeMode::Light | ThemeMode::HighContrast => egui::Theme::Light,
        });
        Self {
            signals: receiver,
            ctx,
            visible: false,
            show_received_at: None,
            frame_logged_for_show: false,
            window_styled,
            window_handle,
            theme,
            query: String::new(),
            perf_log,
        }
    }

    fn mode(&self) -> Mode {
        self.theme.into()
    }

    fn apply_visuals(&self, ctx: &egui::Context) {
        let mode = self.mode();
        let mut visuals = match self.theme {
            ThemeMode::Dark | ThemeMode::System => egui::Visuals::dark(),
            ThemeMode::Light | ThemeMode::HighContrast => egui::Visuals::light(),
        };
        if let Some(value) = color::canvas(mode) {
            visuals.panel_fill = to_color(value);
            visuals.window_fill = to_color(value);
        }
        if let Some(value) = color::text_primary(mode) {
            visuals.override_text_color = Some(to_color(value));
        }
        if let Some(value) = color::selection(mode) {
            visuals.selection.bg_fill = to_color(value);
        }
        if let Some(value) = color::selection_text(mode) {
            visuals.selection.stroke.color = to_color(value);
        }
        ctx.set_visuals(visuals);
    }

    fn process_signals(&mut self) {
        while let Ok(signal) = self.signals.try_recv() {
            match signal {
                PopupSignal::Show { received_at } => {
                    self.show_received_at = Some(received_at);
                    self.frame_logged_for_show = false;
                    self.visible = true;
                    if let Some(hwnd) = self.window_handle {
                        ditto_win::set_popup_visible(hwnd, true);
                    }
                    self.ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    self.ctx.request_repaint();
                }
                PopupSignal::Hide => {
                    self.visible = false;
                    if let Some(hwnd) = self.window_handle {
                        ditto_win::set_popup_visible(hwnd, false);
                    }
                    self.ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                }
            }
        }
    }

    fn enforce_hidden_unless_requested(&mut self) {
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
        if let Some(hwnd) = self.window_handle {
            if make_popup_window(hwnd).is_ok() {
                self.window_styled = true;
            }
        }
    }

    fn record_first_frame(&mut self) {
        if !self.visible || self.frame_logged_for_show {
            return;
        }
        let Some(received_at) = self.show_received_at else {
            return;
        };
        let rendered_at = Instant::now();
        self.frame_logged_for_show = true;
        let latency_us = rendered_at.duration_since(received_at).as_micros();
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
                    "{},{},{}",
                    unix_millis(),
                    latency_us,
                    self.ctx.pixels_per_point()
                )
            });
    }
}

impl eframe::App for PopupApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.window_handle.is_none() {
            self.window_handle = native_window(frame);
        }
        self.style_window_once();
        self.process_signals();
        self.enforce_hidden_unless_requested();
        self.apply_visuals(ctx);
        let mode = self.mode();

        egui::TopBottomPanel::top("popup_title")
            .exact_height(dimension::title_height(mode))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(dimension::space_md(mode));
                    ui.label(
                        RichText::new("Ditto")
                            .size(14.0)
                            .strong()
                            .color(color::text_primary(mode).map_or(Color32::WHITE, to_color)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Ctrl+Alt+D")
                                .size(11.0)
                                .color(color::text_secondary(mode).map_or(Color32::GRAY, to_color)),
                        );
                        ui.add_space(dimension::space_md(mode));
                    });
                });
            });

        egui::TopBottomPanel::bottom("popup_search")
            .exact_height(dimension::search_height(mode))
            .show(ctx, |ui| {
                ui.add_space(dimension::space_xs(mode));
                let response = ui.add_sized(
                    [ui.available_width() - 16.0, dimension::control_height(mode)],
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Search history")
                        .margin(egui::Margin::symmetric(8, 6)),
                );
                if !self.visible {
                    response.surrender_focus();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.add(
                    egui::Label::new(
                        RichText::new("Ditto rewrite popup placeholder")
                            .size(14.0)
                            .strong(),
                    )
                    .wrap(),
                );
                ui.add_space(dimension::space_sm(mode));
                ui.add(
                    egui::Label::new(
                        RichText::new("Preview, groups, and clip navigation arrive in T3.")
                            .size(12.0)
                            .color(color::text_secondary(mode).map_or(Color32::GRAY, to_color)),
                    )
                    .wrap(),
                );
                ui.add_space(dimension::space_lg(mode));
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_stroke(
                    rect.shrink(12.0),
                    radius::list_item(mode),
                    egui::Stroke::new(
                        1.0_f32,
                        color::border(mode).map_or(Color32::DARK_GRAY, to_color),
                    ),
                    egui::StrokeKind::Inside,
                );
            });
        });

        self.record_first_frame();
        if !self.visible {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
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
                    Ok(()) | Err(_) => {
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
    native_handle: Arc<AtomicIsize>,
) -> ditto_model::Result<()> {
    let name = pipe_name()?;
    let client = PipeClient::connect(&name)?;
    client.send(1, &hello_request(ClientKind::Popup))?;
    match client.read_response()? {
        Response::HelloAck { .. } => {}
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
                show_native_popup(&native_handle, true);
                let _ = sender.send(PopupSignal::Show {
                    received_at: Instant::now(),
                });
                ctx.request_repaint();
            }
            Event::HidePopup => {
                show_native_popup(&native_handle, false);
                let _ = sender.send(PopupSignal::Hide);
                ctx.request_repaint();
            }
            Event::Shutdown { .. } => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
                return Ok(());
            }
            _ => {}
        }
    }
}

fn perf_log_path() -> PathBuf {
    if let Some(path) = std::env::args()
        .find_map(|argument| argument.strip_prefix("--perf-log=").map(PathBuf::from))
    {
        return path;
    }
    std::env::var_os("DITTO_PERF_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/rewrite/after/perf/popup-latency.csv"))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn show_native_popup(native_handle: &AtomicIsize, visible: bool) {
    let handle = native_handle.load(Ordering::Relaxed);
    if handle != 0 {
        ditto_win::set_popup_visible(HWND(handle as *mut std::ffi::c_void), visible);
    }
}

fn to_color(value: &str) -> Color32 {
    let [red, green, blue, alpha] = parse_hex(value);
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn native_window<T: HasWindowHandle>(handle_source: &T) -> Option<HWND> {
    let handle = handle_source.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(HWND(win32.hwnd.get() as *mut std::ffi::c_void)),
        _ => None,
    }
}
