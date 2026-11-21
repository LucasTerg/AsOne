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

/// Zgodność ze starszym glibc (Ubuntu 22.04, Debian 12): gotowa biblioteka ONNX Runtime (ort, usuwanie
/// tła i retusz) jest skompilowana z nagłówkami glibc ≥ 2.38 i woła `__isoc23_strto*` – wersje
/// `strtol`/`strtoll`/`strtoull` z C23 (różnią się tylko przedrostkiem „0b”, którego ONNX Runtime
/// nie używa). Zamienniki przekazują wywołanie do zwykłych funkcji, więc program nie wymaga glibc 2.38.
#[cfg(all(feature = "ai", target_os = "linux", target_env = "gnu"))]
mod glibc_compat {
    use std::ffi::{c_char, c_int, c_long, c_longlong, c_ulonglong};

    unsafe extern "C" {
        fn strtol(s: *const c_char, end: *mut *mut c_char, base: c_int) -> c_long;
        fn strtoll(s: *const c_char, end: *mut *mut c_char, base: c_int) -> c_longlong;
        fn strtoull(s: *const c_char, end: *mut *mut c_char, base: c_int) -> c_ulonglong;
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __isoc23_strtol(s: *const c_char, end: *mut *mut c_char, base: c_int) -> c_long {
        unsafe { strtol(s, end, base) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __isoc23_strtoll(s: *const c_char, end: *mut *mut c_char, base: c_int) -> c_longlong {
        unsafe { strtoll(s, end, base) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn __isoc23_strtoull(s: *const c_char, end: *mut *mut c_char, base: c_int) -> c_ulonglong {
        unsafe { strtoull(s, end, base) }
    }
}
