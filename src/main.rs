// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icon;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ResCAD NG")
            .with_app_id("rescad-ng")
            .with_inner_size([780.0, 450.0])
            .with_min_inner_size([680.0, 420.0])
            .with_icon(icon()),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "ResCAD NG",
        options,
        Box::new(|_cc| Ok(Box::<app::ResCadApp>::default())),
    )
}

fn icon() -> egui::IconData {
    const SIZE: usize = 64;
    egui::IconData {
        rgba: icon::render(SIZE),
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
