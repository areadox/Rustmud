//! rustmud – ein MUD-Client in Rust nach dem Vorbild von MUSHclient.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ansi;
mod app;
mod automation;
mod logger;
mod net;
mod scripting;
mod session;
mod telnet;
mod ui_editors;
mod world;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rustmud")
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([500.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native("rustmud", options, Box::new(|cc| Ok(Box::new(app::MudApp::new(cc)))))
}
