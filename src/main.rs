// En release, on cache la console Windows pour que ce soit une vraie app graphique.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::path::PathBuf;

mod app;
mod audio;
mod config;
mod download;
mod hotkey;
mod icon;
mod output;
mod transcribe;
mod tray;
mod worker;

fn main() -> eframe::Result<()> {
    init_logging();
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {}", info);
    }));
    log::info!("NyxWhisper start");

    let icon_size = 64u32;
    let icon_rgba = icon::n_grunge_rgba(icon_size);
    let window_icon = egui::IconData {
        rgba: icon_rgba,
        width: icon_size,
        height: icon_size,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NyxWhisper — Dictée vocale française")
            .with_inner_size([880.0, 560.0])
            .with_min_inner_size([720.0, 460.0])
            .with_icon(window_icon),
        ..Default::default()
    };

    let result = eframe::run_native(
        "NyxWhisper",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    );
    log::info!("NyxWhisper exit: {:?}", result.as_ref().err());
    result
}

fn init_logging() {
    let env = env_logger::Env::default().default_filter_or("info");
    let mut builder = env_logger::Builder::from_env(env);
    builder.format_timestamp_secs();

    let local_dir = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(dirs::data_local_dir);

    if let Some(local) = local_dir {
        let dir = local.join("NyxWhisper");
        if std::fs::create_dir_all(&dir).is_ok() {
            let path = dir.join("NyxWhisper.log");
            if let Ok(file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                builder.target(env_logger::Target::Pipe(Box::new(file)));
            }
        }
    }

    let _ = builder.try_init();
}
