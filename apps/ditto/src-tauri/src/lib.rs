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
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

pub mod commands;
mod core_service;
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

pub fn run() {
    // Fast single-instance path: signal the running core and exit before
    // Tauri, the tray, the clipboard monitor, or WebView2 initialize.
    let listener_signals = claim_single_instance();

    let development_hotkey = choose_development_hotkey();
    let development_shortcut: Shortcut = development_hotkey
        .parse()
        .expect("development hotkey must parse");
    let shortcut_plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_shortcuts([development_shortcut])
        .expect("development hotkey must parse")
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_popup(app);
            }
        })
        .build();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|argument| argument == "--settings") {
                if let Err(error) = open_settings(app) {
                    log::error!("failed to open settings from second instance: {error}");
                }
            } else {
                toggle_popup(app);
            }
        }))
        .plugin(tauri_plugin_log::Builder::new().build())
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
            commands::migration_dry_run,
            commands::migration_apply,
            commands::open_data_folder,
            commands::open_log_folder,
            commands::get_diagnostics,
            commands::get_app_info,
        ])
        .setup(move |app| {
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
            for applied in pending::apply_staged(&data_root.path)
                .map_err(|error| format!("staged data operation failed: {error}"))?
            {
                log::info!("applied staged operation: {applied}");
            }
            let storage =
                Storage::open_data_root(&data_root.path).map_err(|error| error.to_string())?;
            let core =
                CoreService::start(&name, storage.clone()).map_err(|error| error.to_string())?;
            let pipe = core.pipe().clone();
            app.manage(AppState {
                pipe,
                storage,
                data_root,
                core: Mutex::new(Some(core)),
                popup: Mutex::new(None),
                _instance_signal: instance_signal,
            });

            if let Some(settings) = settings_listener {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    while settings.wait_forever() {
                        let dispatch = handle.clone();
                        let open = handle.clone();
                        if let Err(error) = dispatch.run_on_main_thread(move || {
                            if let Err(error) = open_settings(&open) {
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

            create_tray(app.handle())?;
            log::info!("development hotkey active: {development_hotkey}");
            spawn_popup(app.handle());
            if std::env::args().any(|argument| argument == "--settings") {
                open_settings(app.handle())?;
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

    let app = builder
        .build(tauri::generate_context!())
        .expect("failed to build Ditto");
    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &exit])?;

    TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("missing tray icon".to_string()))?,
        )
        .tooltip("Ditto rewrite")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_popup(app),
            "settings" => {
                if let Err(error) = open_settings(app) {
                    log::error!("failed to open settings: {error}");
                }
            }
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

fn choose_development_hotkey() -> String {
    if ditto_win::hotkey_available(0x0003, 0x44) {
        DEV_HOTKEY.to_string()
    } else {
        DEV_HOTKEY_FALLBACK.to_string()
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
    let Some(path) = popup_executable() else {
        log::warn!("DittoPopup.exe was not found beside the core");
        return;
    };
    let mut command = Command::new(path);
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
            *child_slot = Some(child);
            log::info!("DittoPopup started");
        }
        Err(error) => log::error!("failed to start DittoPopup: {error}"),
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

fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
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
