#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console on Windows release

use rusty_terminal::app;


fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([480.0, 320.0])
            .with_title("Rusty Terminal"),
        ..Default::default()
    };

    eframe::run_native(
        "Rusty Terminal",
        native_options,
        Box::new(|cc| Ok(Box::new(app::RustyApp::new(cc)))),
    )
}
