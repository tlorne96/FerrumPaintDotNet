#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Ferrum Paint — a clean-room raster image editor built with egui and tiny-skia.

mod app;
mod canvas;
mod colors;
mod dialogs;
mod document;
mod fileio;
mod icons;
mod paint;
mod raster;
mod text;
mod tools;
mod ui;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--write-icon <path.png> [size]` exports the procedurally drawn app icon (used for packaging).
    if args.first().map(|s| s == "--write-icon").unwrap_or(false) {
        let path = args.get(1).cloned().unwrap_or_else(|| "ferrum-paint.png".into());
        let size: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(256);
        let px = icons::app_icon_rgba(size);
        image::RgbaImage::from_raw(size, size, px).expect("icon").save(&path).expect("write icon");
        println!("wrote {path}");
        return Ok(());
    }
    let files: Vec<PathBuf> = args.drain(..).filter(|a| !a.starts_with("--")).map(PathBuf::from).collect();

    let icon = egui::IconData { rgba: icons::app_icon_rgba(64), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app::APP_NAME)
            .with_app_id("ferrum-paint")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([640.0, 420.0])
            .with_drag_and_drop(true)
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(app::APP_NAME, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, files)))))
}
