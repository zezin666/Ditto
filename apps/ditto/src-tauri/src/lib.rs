use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use ditto_ipc::Event;
use ditto_win::{PipeServer, pipe_name};
use parking_lot::Mutex;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

pub mod commands;

const DEV_HOTKEY: &str = "Ctrl+Alt+D";
const DEV_HOTKEY_FALLBACK: &str = "Ctrl+Alt+Shift+D";

struct AppState {
    pipe: PipeServer,
    popup_visible: AtomicBool,
    popup: Mutex<Option<Child>>,
}

pub fn run() {
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
            commands::backup_db,
            commands::restore_db,
            commands::export_data,
            commands::import_data,
            commands::optimize_storage,
            commands::get_migration_status,
            commands::open_data_folder,
            commands::open_log_folder,
            commands::get_diagnostics,
            commands::get_app_info,
        ])
        .setup(move |app| {
            let name = pipe_name().map_err(|error| error.to_string())?;
            let pipe = PipeServer::start(&name).map_err(|error| error.to_string())?;
            app.manage(AppState {
                pipe,
                popup_visible: AtomicBool::new(false),
                popup: Mutex::new(None),
            });

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
    let show = !state.popup_visible.fetch_xor(true, Ordering::SeqCst);
    let event = if show {
        Event::ShowPopup {
            at_ms: ditto_win::unix_millis(),
            monitor: None,
        }
    } else {
        Event::HidePopup
    };
    log::info!(
        "toggle popup: show={show}, ipc_clients={}",
        state.pipe.client_count()
    );
    if let Err(error) = state.pipe.broadcast(&event) {
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
    app.exit(0);
}
