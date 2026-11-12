use ditto_popup_app::{PopupApp, PopupOptions, prewarm_ipc};
use ditto_ui_tokens::{Mode, dimension};
use eframe::egui;

fn main() -> eframe::Result {
    if let Ok(path) = std::env::var("DITTO_POPUP_TRACE") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{} popup:process-start", ditto_win::unix_millis());
        }
    }
    let options = PopupOptions::parse();
    let width = options
        .dpi_scale_override
        .map_or(dimension::popup_width(Mode::Light), |scale| {
            dimension::popup_width(Mode::Light) * scale
        });
    let height = options
        .dpi_scale_override
        .map_or(dimension::popup_height(Mode::Light), |scale| {
            dimension::popup_height(Mode::Light) * scale
        });
    // Establish IPC before the renderer/window initializes. The core's popup
    // "ready" signal then depends on the pipe, not on GPU/font cold-start.
    let preconnected = if options.mock { None } else { prewarm_ipc() };
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ditto")
            .with_inner_size([width, height])
            .with_min_inner_size([
                dimension::popup_width(Mode::Light) * 0.75,
                dimension::popup_height(Mode::Light) * 0.66,
            ])
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            // winit suppresses redraws for hidden windows, which would stop
            // egui from processing IPC Show events. Create the window
            // "visible" off-screen instead and hide it natively on the first
            // update, so repaint requests keep flowing while it is resident.
            .with_position([-32_000.0, -32_000.0])
            .with_visible(true),
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        "Ditto",
        native_options,
        Box::new(move |creation| Ok(Box::new(PopupApp::new(creation, options, preconnected)))),
    )
}
