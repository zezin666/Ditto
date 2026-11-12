use std::{
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::Duration,
};

use ditto_ipc::Event;
use ditto_model::DataRoot;
use ditto_storage::Storage;
use ditto_win::{PipeServer, SingleInstanceSignal, pipe_name};
use parking_lot::Mutex;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub mod commands;
mod core_service;
pub mod hotkeys;
mod pending;

use core_service::CoreService;

const DEV_HOTKEY: &str = "Ctrl+Alt+D";
const DEV_HOTKEY_FALLBACK: &str = "Ctrl+Alt+Shift+D";

/// Shared state for the tray/hotkey host and the settings commands.
///
/// The core service owns the clipboard monitor and paste engine; the storage
/// handle is a cheap clone of the same SQLite pool, used by the settings
/// commands so they do not have to go through the named pipe.
pub struct AppState {
    pipe: PipeServer,
    storage: Storage,
    data_root: DataRoot,
    core: Mutex<Option<CoreService>>,
    popup: Mutex<Option<Child>>,
    /// Keeps the primary-instance event handle alive for the process lifetime.
    _instance_signal: Option<SingleInstanceSignal>,
}

impl AppState {
    pub(crate) fn storage(&self) -> &Storage {
        &self.storage
    }

    pub(crate) fn data_root(&self) -> &DataRoot {
        &self.data_root
    }

    pub(crate) fn broadcast(&self, event: &Event) {
        if let Err(error) = self.pipe.broadcast(event) {
            log::warn!("IPC broadcast failed: {error}");
        }
    }

    pub(crate) fn core_paused(&self) -> bool {
        self.core
            .lock()
            .as_ref()
            .is_some_and(CoreService::is_paused)
    }
}

struct InstanceSignals {
    settings: SingleInstanceSignal,
    toggle: SingleInstanceSignal,
    instance: SingleInstanceSignal,
}

fn claim_single_instance() -> Option<InstanceSignals> {
    let sid = ditto_win::current_user_sid_string().ok()?;
    let settings = SingleInstanceSignal::create(&format!("Local\\Ditto.v2.{sid}.settings")).ok()?;
    let toggle = SingleInstanceSignal::create(&format!("Local\\Ditto.v2.{sid}.toggle")).ok()?;
    let instance = SingleInstanceSignal::create(&format!("Local\\Ditto.v2.{sid}.instance")).ok()?;
    if instance.already_running() {
        if std::env::args().any(|argument| argument == "--settings") {
            settings.signal();
        } else {
            toggle.signal();
        }
        std::process::exit(0);
    }
    Some(InstanceSignals {
        settings,
        toggle,
        instance,
    })
}

/// Settings needed before Tauri starts: single-instance policy, tray policy,
/// log level, and the global open chord. A short-lived read-only storage open
/// is cheap and lets the host honor user settings from the first second.
#[derive(Debug, Clone)]
struct StartupSettings {
    show_tray_icon: bool,
    language: String,
    log_level: log::LevelFilter,
    open_chord: String,
}

fn load_startup_settings() -> StartupSettings {
    let defaults = StartupSettings {
        show_tray_icon: true,
        language: "en".to_string(),
        log_level: log::LevelFilter::Info,
        open_chord: hotkeys::DEFAULT_OPEN_CHORD.to_string(),
    };
    let Ok(executable) = std::env::current_exe() else {
        return defaults;
    };
    let Some(executable_dir) = executable.parent() else {
        return defaults;
    };
    let Ok(data_root) = core_service::resolve_data_root(executable_dir) else {
        return defaults;
    };
    let Ok(storage) = Storage::open_data_root(&data_root.path) else {
        return defaults;
    };
    let Ok(snapshot) = storage.get_settings() else {
        return defaults;
    };
    let values = &snapshot.values;
    let language = values
        .pointer("/general/language")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("en")
        .to_string();
    let open_chord = values
        .pointer("/hotkeys/open_chord")
        .and_then(serde_json::Value::as_str)
        .filter(|chord| !chord.trim().is_empty())
        .unwrap_or(hotkeys::DEFAULT_OPEN_CHORD)
        .to_string();
    let log_level = match values
        .pointer("/diagnostics/log_level")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("info")
    {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    };
    StartupSettings {
        show_tray_icon: values
            .pointer("/general/show_tray_icon")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        language,
        log_level,
        open_chord,
    }
}

fn choose_initial_hotkey(configured: &str) -> (String, Shortcut) {
    for chord in [
        configured,
        hotkeys::DEFAULT_OPEN_CHORD,
        DEV_HOTKEY,
        DEV_HOTKEY_FALLBACK,
    ] {
        let Ok(shortcut) = hotkeys::parse_chord(chord) else {
            continue;
        };
        if hotkeys::chord_available(chord) {
            return (chord.to_string(), shortcut);
        }
    }
    let chord = hotkeys::DEFAULT_OPEN_CHORD.to_string();
    let shortcut = chord.parse().expect("default chord must parse");
    (chord, shortcut)
}

fn startup_trace(message: &str) {
    if let Some(path) =
        std::env::var_os("DITTO_HOST_TRACE").or_else(|| std::env::var_os("DITTO_POPUP_TRACE"))
    {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{} host:{message}", ditto_win::unix_millis());
        }
    }
}

pub fn run() {
    startup_trace("run-enter");
    let startup = load_startup_settings();
    startup_trace("startup-settings-loaded");
    log::set_max_level(startup.log_level);
    // Fast single-instance path: signal the running core and exit before
    // Tauri, the tray, the clipboard monitor, or WebView2 initialize. The
    // per-user pipe only supports one core, so this is always enforced; the
    // Settings toggle is disabled instead of pretending otherwise.
    let listener_signals = claim_single_instance();

    let (initial_hotkey, initial_shortcut) = choose_initial_hotkey(&startup.open_chord);
    let shortcut_plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_shortcuts([initial_shortcut])
        .expect("initial hotkey must parse")
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_popup(app);
            }
        })
        .build();

    // Start the popup before Tauri/WebView2 initialization so loader and
    // Defender startup time overlaps the host's own startup.
    let early_popup = spawn_popup_process(&startup.language);
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|argument| argument == "--settings") {
                if let Err(error) = open_settings(app, None) {
                    log::error!("failed to open settings from second instance: {error}");
                }
            } else {
                toggle_popup(app);
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(startup.log_level)
                .build(),
        )
        .plugin(shortcut_plugin)
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::set_settings,
            commands::list_clips,
            commands::search_clips,
            commands::get_clip,
            commands::delete_clips,
            commands::clear_history,
            commands::set_pinned,
            commands::move_to_group,
            commands::edit_clip,
            commands::get_groups,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::move_group,
            commands::get_copy_buffers,
            commands::set_copy_buffer,
            commands::clear_copy_buffer,
            commands::paste_clip,
            commands::paste_copy_buffer,
            commands::drag_out_clip,
            commands::backup_db,
            commands::restore_db,
            commands::export_data,
            commands::import_data,
            commands::optimize_storage,
            commands::get_migration_status,
            commands::clear_staged_failure,
            commands::migration_dry_run,
            commands::migration_apply,
            commands::open_data_folder,
            commands::open_log_folder,
            commands::get_diagnostics,
            commands::get_app_info,
        ])
        .setup(move |app| {
            startup_trace("setup-enter");
            let mut settings_listener = None;
            let mut toggle_listener = None;
            let mut instance_signal = None;
            if let Some(signals) = listener_signals {
                let InstanceSignals {
                    settings,
                    toggle,
                    instance,
                } = signals;
                settings_listener = Some(settings);
                toggle_listener = Some(toggle);
                instance_signal = Some(instance);
            }
            let name = pipe_name().map_err(|error| error.to_string())?;
            let executable = std::env::current_exe()?;
            let executable_dir = executable
                .parent()
                .ok_or_else(|| "Ditto executable has no parent directory".to_string())?;
            let data_root = core_service::resolve_data_root(executable_dir)
                .map_err(|error| error.to_string())?;
            startup_trace("data-root-and-staging");
            for applied in pending::apply_staged(&data_root.path) {
                log::info!("staged data operation: {applied}");
            }
            let storage =
                Storage::open_data_root(&data_root.path).map_err(|error| error.to_string())?;
            startup_trace("storage-open");
            let core =
                CoreService::start(&name, storage.clone()).map_err(|error| error.to_string())?;
            startup_trace("core-started");
            // If the configured chord was unavailable at startup, persist the
            // fallback that is actually registered so Settings never shows a
            // chord the process does not own.
            if initial_hotkey != startup.open_chord
                && let Err(error) =
                    storage.set_setting("hotkeys.open_chord", serde_json::json!(initial_hotkey))
            {
                log::warn!("failed to persist fallback hotkey: {error}");
            }
            let pipe = core.pipe().clone();
            app.manage(AppState {
                pipe,
                storage,
                data_root,
                core: Mutex::new(Some(core)),
                popup: Mutex::new(early_popup),
                _instance_signal: instance_signal,
            });

            if let Some(settings) = settings_listener {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    while settings.wait_forever() {
                        let dispatch = handle.clone();
                        let open = handle.clone();
                        if let Err(error) = dispatch.run_on_main_thread(move || {
                            if let Err(error) = open_settings(&open, None) {
                                log::error!(
                                    "failed to open settings from instance signal: {error}"
                                );
                            }
                        }) {
                            log::error!("failed to dispatch settings signal: {error}");
                        }
                    }
                });
            }
            if let Some(toggle) = toggle_listener {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    while toggle.wait_forever() {
                        toggle_popup(&handle);
                    }
                });
            }

            if startup.show_tray_icon {
                create_tray(app.handle(), &startup.language)?;
            }
            startup_trace("tray-ready");
            log::info!("global hotkey active: {initial_hotkey}");
            spawn_popup(app.handle());
            startup_trace("popup-spawned");
            if std::env::args().any(|argument| argument == "--settings")
                && let Err(error) = open_settings(app.handle(), None)
            {
                log::error!("failed to open settings from command line: {error}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "settings" {
                    let _ = window.destroy();
                }
            }
        });

    startup_trace("builder-ready");
    let app = builder
        .build(tauri::generate_context!())
        .expect("failed to build Ditto");
    startup_trace("tauri-built");
    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

fn locale_text(language: &str, key: &str) -> String {
    const EN: &str = include_str!("../../ui/locales/en-US.json");
    const ZH: &str = include_str!("../../ui/locales/zh-CN.json");
    let raw = if language.starts_with("zh") { ZH } else { EN };
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| key.to_string())
}

fn current_language(app: &AppHandle) -> String {
    app.state::<AppState>()
        .storage()
        .get_settings()
        .ok()
        .and_then(|snapshot| {
            snapshot
                .values
                .pointer("/general/language")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "en".to_string())
}

fn create_tray(app: &AppHandle, language: &str) -> tauri::Result<()> {
    let paused = app.state::<AppState>().core_paused();
    let t = |key: &str| locale_text(language, key);
    let open = MenuItem::with_id(app, "show", t("tray.open"), true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", t("tray.settings"), true, None::<&str>)?;
    let groups = MenuItem::with_id(app, "groups", t("tray.groups"), true, None::<&str>)?;
    let copy_buffers = MenuItem::with_id(
        app,
        "copyBuffers",
        t("tray.copyBuffers"),
        true,
        None::<&str>,
    )?;
    let connect = MenuItem::with_id(app, "connect", t("tray.resume"), paused, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, "disconnect", t("tray.pause"), !paused, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", t("tray.exit"), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &settings,
            &groups,
            &copy_buffers,
            &separator,
            &connect,
            &disconnect,
            &separator2,
            &exit,
        ],
    )?;

    TrayIconBuilder::with_id("main")
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("missing tray icon".to_string()))?,
        )
        .tooltip(t("tray.tooltip"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_popup(app),
            "settings" => {
                if let Err(error) = open_settings(app, None) {
                    log::error!("failed to open settings: {error}");
                }
            }
            "groups" => {
                if let Err(error) = open_settings(app, Some("groups")) {
                    log::error!("failed to open groups: {error}");
                }
            }
            "copyBuffers" => {
                if let Err(error) = open_settings(app, Some("copyBuffers")) {
                    log::error!("failed to open copy buffers: {error}");
                }
            }
            "connect" => set_paused(app, false),
            "disconnect" => set_paused(app, true),
            "exit" => shutdown_and_exit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popup(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn refresh_tray(app: &AppHandle) {
    app.remove_tray_by_id("main");
    let show = app
        .state::<AppState>()
        .storage()
        .get_settings()
        .ok()
        .and_then(|snapshot| {
            snapshot
                .values
                .pointer("/general/show_tray_icon")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true);
    if show {
        let language = current_language(app);
        if let Err(error) = create_tray(app, &language) {
            log::error!("failed to refresh tray icon: {error}");
        }
    }
}

fn set_paused(app: &AppHandle, paused: bool) {
    let state = app.state::<AppState>();
    let result = match state.core.lock().as_ref() {
        Some(core) => core.set_paused(paused),
        None => Err("core service is stopping".to_string()),
    };
    match result {
        Ok(()) => {
            state.broadcast(&Event::ClipboardState { connected: !paused });
            refresh_tray(app);
        }
        Err(error) => log::error!("failed to change clipboard capture state: {error}"),
    }
}

fn toggle_popup(app: &AppHandle) {
    let state = app.state::<AppState>();
    ensure_popup_running(app);
    let show = !state.pipe.popup_visible();
    log::info!(
        "toggle popup: show={show}, ipc_clients={}",
        state.pipe.client_count()
    );
    if show {
        // Capture the user's foreground window before the popup can be
        // activated. Paste later uses this snapshot, never DittoPopup itself.
        if let Some(core) = state.core.lock().as_ref() {
            core.capture_paste_target();
        }
    } else if let Some(core) = state.core.lock().as_ref() {
        core.clear_paste_target();
    }
    if let Err(error) = state.pipe.set_popup_visible(show) {
        log::error!("failed to broadcast popup event: {error}");
    }
}

fn spawn_popup(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut child_slot = state.popup.lock();
    if child_slot
        .as_mut()
        .is_some_and(|child| child.try_wait().ok().flatten().is_none())
    {
        return;
    }
    let language = state
        .storage()
        .get_settings()
        .ok()
        .and_then(|snapshot| {
            snapshot
                .values
                .pointer("/general/language")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "en".to_string());
    if let Some(child) = spawn_popup_process(&language) {
        *child_slot = Some(child);
        log::info!("DittoPopup started");
    }
}

/// Builds and starts `DittoPopup.exe`. Called before Tauri initialization on
/// the first launch and lazily when the resident popup has exited.
fn spawn_popup_process(language: &str) -> Option<Child> {
    let Some(path) = popup_executable() else {
        log::warn!("DittoPopup.exe was not found beside the core");
        startup_trace("popup-missing");
        return None;
    };
    startup_trace(&format!("popup-path={}", path.display()));
    let mut command = Command::new(path);
    command.arg(format!(
        "--lang={}",
        if language.starts_with("zh") {
            "zh-CN"
        } else {
            "en-US"
        }
    ));
    if let Some(perf_log) = std::env::var_os("DITTO_PERF_LOG") {
        command.arg(format!("--perf-log={}", PathBuf::from(perf_log).display()));
    }
    match command.spawn() {
        Ok(child) => {
            startup_trace(&format!("popup-pid={}", child.id()));
            Some(child)
        }
        Err(error) => {
            log::error!("failed to start DittoPopup: {error}");
            startup_trace(&format!("popup-spawn-error={error}"));
            None
        }
    }
}

fn ensure_popup_running(app: &AppHandle) {
    spawn_popup(app);
}

fn popup_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DITTO_POPUP_PATH") {
        return Some(PathBuf::from(path));
    }
    let current = std::env::current_exe().ok()?;
    let candidate = current.parent()?.join("DittoPopup.exe");
    candidate.is_file().then_some(candidate)
}

fn webview2_missing_message(language: &str) -> (String, String) {
    (
        locale_text(language, "webview2.missingTitle"),
        locale_text(language, "webview2.missingBody"),
    )
}

fn open_settings(app: &AppHandle, page: Option<&str>) -> tauri::Result<()> {
    if !ditto_win::webview2_available() {
        let language = current_language(app);
        let (title, body) = webview2_missing_message(&language);
        ditto_win::show_message_box(&title, &body, true);
        return Err(tauri::Error::AssetNotFound(
            "Microsoft Edge WebView2 Runtime is not installed".to_string(),
        ));
    }
    if let Some(window) = app.get_webview_window("settings") {
        window.show()?;
        window.set_focus()?;
        if let Some(page) = page {
            let script = format!("window.location.hash = {:?};", page);
            let _ = window.eval(&script);
        }
        return Ok(());
    }

    // Capture the app the user was working in before Settings takes focus, so
    // "paste" from the history manager still targets that app.
    if let Some(core) = app.state::<AppState>().core.lock().as_ref() {
        core.capture_paste_target();
    }
    let url = match page {
        Some(page) => format!("index.html#{page}"),
        None => "index.html".to_string(),
    };
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App(url.into()))
        .title("Ditto Settings")
        .inner_size(900.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .center()
        .build()?;
    Ok(())
}

fn shutdown_and_exit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = state.pipe.broadcast(&Event::Shutdown {
        reason: "tray exit".to_string(),
    });
    thread::sleep(Duration::from_millis(100));
    if let Some(mut child) = state.popup.lock().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some(mut core) = state.core.lock().take() {
        core.stop();
    }
    app.exit(0);
}

/// Applies host-side settings that are not owned by storage: tray visibility,
/// log level, and the global open chord. Called from `set_settings` after the
/// new values are persisted.
pub(crate) fn apply_host_settings(
    app: &AppHandle,
    previous: &serde_json::Value,
    values: &serde_json::Value,
) -> Result<(), String> {
    let previous_tray = previous
        .pointer("/general/show_tray_icon")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let next_tray = values
        .pointer("/general/show_tray_icon")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if previous_tray != next_tray {
        refresh_tray(app);
    }
    let previous_level = previous
        .pointer("/diagnostics/log_level")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("info");
    let next_level = values
        .pointer("/diagnostics/log_level")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("info");
    if previous_level != next_level {
        log::set_max_level(match next_level {
            "error" => log::LevelFilter::Error,
            "warn" => log::LevelFilter::Warn,
            "debug" => log::LevelFilter::Debug,
            "trace" => log::LevelFilter::Trace,
            _ => log::LevelFilter::Info,
        });
    }
    let previous_chord = previous
        .pointer("/hotkeys/open_chord")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(hotkeys::DEFAULT_OPEN_CHORD);
    let next_chord = values
        .pointer("/hotkeys/open_chord")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(hotkeys::DEFAULT_OPEN_CHORD);
    if previous_chord != next_chord && !next_chord.trim().is_empty() {
        let previous_shortcut = hotkeys::parse_chord(previous_chord).ok();
        let next_shortcut = hotkeys::parse_chord(next_chord)?;
        let mut registrar = TauriRegistrar(app.clone());
        hotkeys::swap(
            &mut registrar,
            previous_shortcut.as_ref(),
            Some(&next_shortcut),
        )?;
        log::info!("global hotkey changed to {next_chord}");
    }
    if let Some(core) = app.state::<AppState>().core.lock().as_ref() {
        core.apply_settings(values)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct TauriRegistrar(AppHandle);

impl hotkeys::ShortcutRegistrar for TauriRegistrar {
    fn register(&mut self, shortcut: &Shortcut) -> Result<(), String> {
        self.0
            .global_shortcut()
            .register(*shortcut)
            .map_err(|error| error.to_string())
    }

    fn unregister(&mut self, shortcut: &Shortcut) -> Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(*shortcut)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_and_webview_strings_exist_in_both_languages() {
        for key in [
            "tray.open",
            "tray.settings",
            "tray.groups",
            "tray.copyBuffers",
            "tray.pause",
            "tray.resume",
            "tray.exit",
            "webview2.missingTitle",
            "webview2.missingBody",
        ] {
            let english = locale_text("en", key);
            let chinese = locale_text("zh-CN", key);
            assert_ne!(english, key, "missing EN locale key {key}");
            assert_ne!(chinese, key, "missing ZH locale key {key}");
            assert_ne!(english, chinese, "locale key {key} is not localized");
        }
    }

    #[test]
    fn webview2_detection_is_a_boolean_without_panicking() {
        let _ = ditto_win::webview2_available();
        let _ = ditto_win::webview2_version();
    }
}
