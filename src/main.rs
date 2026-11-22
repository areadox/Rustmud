//! rustmud – ein MUD-Client in Rust nach dem Vorbild von MUSHclient.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ansi;
mod app;
mod automation;
mod keypad;
mod logger;
mod net;
mod scripting;
mod session;
mod telnet;
mod ui_editors;
mod updater;
mod world;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(format!("rustmud {}", env!("CARGO_PKG_VERSION")))
        .with_inner_size([1100.0, 750.0])
        .with_min_inner_size([500.0, 300.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/rustmud.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("rustmud", options, Box::new(|cc| Ok(Box::new(app::MudApp::new(cc)))))
}
