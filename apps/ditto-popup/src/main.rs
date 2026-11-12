use ditto_popup_app::{PopupApp, PopupOptions};
use ditto_ui_tokens::{Mode, dimension};
use eframe::egui;

fn main() -> eframe::Result {
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
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_native(
        "Ditto",
        native_options,
        Box::new(move |creation| Ok(Box::new(PopupApp::new(creation, options)))),
    )
}
