#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Ferrum Paint — a clean-room raster image editor built with egui and tiny-skia.

mod app;
mod pdn;
mod canvas;
mod colors;
mod curves_plus;
mod dialogs;
mod document;
mod fileio;
mod fx;
mod icons;
mod interactive;
mod paint;
mod raster;
mod text;
mod theme;
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
    // `--convert <in> <out>`: open any supported file and save it in the format of <out>
    // (.ferrum and .pdn keep layers; other formats are flattened).
    if args.first().map(|s| s == "--convert").unwrap_or(false) {
        let (Some(src), Some(dst)) = (args.get(1), args.get(2)) else {
            eprintln!("usage: ferrum-paint --convert <input> <output>");
            std::process::exit(2);
        };
        let res = fileio::load(std::path::Path::new(src)).and_then(|doc| {
            let out = std::path::Path::new(dst);
            if matches!(app::file_ext(out).as_str(), "ferrum" | "pdn") {
                fileio::save(&doc, out)
            } else {
                fileio::export_flat(&doc, out, 92)
            }
        });
        if let Err(e) = res {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    // `--register-file-type` (Windows): associate .ferrum files with this executable for the current user.
    if args.first().map(|s| s == "--register-file-type").unwrap_or(false) {
        match register_file_type() {
            Ok(()) => println!(".ferrum files now open with Ferrum Paint"),
            Err(e) => eprintln!("could not register .ferrum: {e}"),
        }
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

#[cfg(windows)]
fn register_file_type() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Prefer an .ico installed next to the exe (the exe itself has no embedded icon resource).
    let ico = exe.with_file_name("ferrum-paint.ico");
    let icon = if ico.exists() { format!("\"{}\"", ico.display()) } else { format!("\"{}\",0", exe.display()) };
    let exe = exe.to_string_lossy();
    let keys = [
        (r"HKCU\Software\Classes\.ferrum", String::from("FerrumPaint.Image")),
        (r"HKCU\Software\Classes\FerrumPaint.Image", String::from("Ferrum Paint image")),
        (r"HKCU\Software\Classes\FerrumPaint.Image\DefaultIcon", icon),
        (r"HKCU\Software\Classes\FerrumPaint.Image\shell\open\command", format!("\"{exe}\" \"%1\"")),
    ];
    for (k, v) in keys {
        let st = std::process::Command::new("reg").args(["add", k, "/ve", "/d", &v, "/f"]).output().map_err(|e| e.to_string())?;
        if !st.status.success() {
            return Err(String::from_utf8_lossy(&st.stderr).into_owned());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn register_file_type() -> Result<(), String> {
    Err("on Linux run packaging/install-linux.sh, which installs the application/x-ferrum-paint MIME type".into())
}