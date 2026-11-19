//! Binarka AsOne: desktop (Linux, BSD, Windows, macOS) albo wasm (budowana przez `trunk`).
//! Android używa `android_main` z biblioteki.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let files = std::env::args_os().skip(1).map(std::path::PathBuf::from).collect();
    asone_app::run_native(asone_app::native_options(), files)
}

#[cfg(target_arch = "wasm32")]
fn main() {
    asone_app::start_web();
}
