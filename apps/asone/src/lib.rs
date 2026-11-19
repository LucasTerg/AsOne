//! # AsOne – edytor zdjęć produktowych do internetu
//!
//! Aplikacja okienkowa (`eframe` + `wgpu` + `egui_dock`) na desktop (Linux, BSD, Windows, macOS),
//! Androida i przeglądarkę (wasm). Logika bez GUI – nazwy, kompozycja, JPEG – jest w `asone-core`,
//! z którego korzysta też `asone-cli` (CLI + serwer MCP).

pub mod ai;
pub mod app;
pub mod canvas;
pub mod document;
pub mod export;
pub mod loader;
pub mod platform;
pub mod prefs;
pub mod renderer;
pub mod settings;
pub mod tabs;
pub mod theme;
pub mod titlebar;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub const APP_TITLE: &str = "AsOne – zdjęcia produktowe do internetu";

/// Opcje okna natywnego (desktop i Android).
#[cfg(not(target_arch = "wasm32"))]
pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_app_id("asone")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            // Własny pasek tytułu jak w PhotoCrafcie (Linux/BSD/Windows); macOS – systemowy.
            // Preferencje › Interfejs › Systemowy pasek tytułu przywraca dekoracje.
            .with_decorations(cfg!(any(target_os = "macos", target_os = "android"))),
        wgpu_options: renderer::wgpu_configuration(),
        ..Default::default()
    }
}

/// Uruchamia aplikację natywną; `files` od razu trafiają do zakładek.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native(options: eframe::NativeOptions, files: Vec<std::path::PathBuf>) -> eframe::Result {
    eframe::run_native(
        "asone",
        options,
        Box::new(|cc| Ok(Box::new(app::AsOneApp::new(cc).with_files(files)))),
    )
}

/// Punkt wejścia Androida (`cargo apk` / `xbuild` budują bibliotekę `cdylib`).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    let mut options = native_options();
    options.android_app = Some(android_app);
    if let Err(e) = run_native(options, Vec::new()) {
        eprintln!("asone: {e}");
    }
}

/// Uruchamia aplikację w przeglądarce na `<canvas id="asone_canvas">`.
#[cfg(target_arch = "wasm32")]
pub fn start_web() {
    web::start();
}
