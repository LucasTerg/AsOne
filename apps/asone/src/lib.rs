//! # AsOne – edytor zdjęć produktowych do internetu
//!
//! Aplikacja okienkowa (`eframe` + `wgpu` + `egui_dock`) na desktop (Linux, BSD, Windows, macOS),
//! Androida i przeglądarkę (wasm). Logika bez GUI – nazwy, kompozycja, JPEG – jest w `asone-core`,
//! z którego korzysta też `asone-cli` (CLI + serwer MCP).

pub mod ai;
#[cfg(target_os = "android")]
pub mod android;
pub mod app;
pub mod canvas;
pub mod document;
pub mod export;
pub mod fal;
#[cfg(not(target_arch = "wasm32"))]
pub mod replicate;
pub mod history;
pub mod lama;
pub mod loader;
pub mod platform;
pub mod prefs;
pub mod renderer;
pub mod settings;
pub mod tabs;
pub mod theme;
pub mod titlebar;
#[cfg(not(target_arch = "wasm32"))]
pub mod update;
pub mod waifu2x;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub const APP_TITLE: &str = "AsOne – zdjęcia produktowe do internetu";
/// Identyfikator aplikacji (desktop, Flatpak, AppStream).
pub const APP_ID: &str = "io.github.lucasterg.AsOne";

/// Opcje okna natywnego (desktop i Android).
#[cfg(not(target_arch = "wasm32"))]
pub fn native_options() -> eframe::NativeOptions {
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            // = nazwa pliku .desktop (Wayland dopasowuje po niej ikonę w doku/pasku zadań).
            .with_app_id(APP_ID)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            // Własny pasek tytułu jak w PhotoCrafcie (Linux/BSD/Windows); macOS – systemowy.
            // Preferencje › Interfejs › Systemowy pasek tytułu przywraca dekoracje.
            .with_decorations(cfg!(any(target_os = "macos", target_os = "android"))),
        wgpu_options: renderer::wgpu_configuration(),
        ..Default::default()
    };
    // Linux/BSD pod Waylandem: okno przez XWayland (X11), bo winit 0.30 nie obsługuje na Waylandzie
    // upuszczania plików z menedżera plików. `ASONE_WAYLAND=1` – czysty Wayland (bez przeciągania).
    #[cfg(all(unix, not(any(target_os = "android", target_os = "macos"))))]
    {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
        if set("WAYLAND_DISPLAY") && set("DISPLAY") && !set("ASONE_WAYLAND") {
            options.event_loop_builder = Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }));
        }
    }
    options
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

/// Aktywność Androida (do odczytu marginesów pasków systemowych przez JNI).
#[cfg(target_os = "android")]
pub static ANDROID_APP: std::sync::OnceLock<winit::platform::android::activity::AndroidApp> = std::sync::OnceLock::new();

/// Punkt wejścia Androida (`cargo apk` / `xbuild` budują bibliotekę `cdylib`).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    let _ = ANDROID_APP.set(android_app.clone());
    let mut options = native_options();
    // eframe nie zna katalogu danych na Androidzie – bez tego ustawienia (motyw, klucze API…)
    // ginęłyby po zamknięciu aplikacji. Plik w prywatnym katalogu aplikacji.
    options.persistence_path = android_app.internal_data_path().map(|dir| dir.join("app.ron"));
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
