//! Wersja przeglądarkowa: parametry adresu, WebGPU / WebGL2 z automatycznym przejściem na WebGL2,
//! sygnał dla karty startowej i przyjmowanie plików pobranych przez stronę (`?file=`, `?url=`, `?sample`).
//!
//! | parametr | działanie |
//! |---|---|
//! | `?backend=webgpu` | tylko WebGPU |
//! | `?backend=webgl2`, `?webgl=1` | tryb zgodności WebGL2 |
//! | (brak) | WebGPU, a gdy nie ruszy – przeładowanie z `?webgl=1` |
//! | `?theme=dark` / `light` / `sepia` | motyw (ten sam, który karta startowa pokazuje bez błysku) |

use std::cell::RefCell;

use eframe::wgpu;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::theme::ThemeKind;

pub struct UrlParams {
    pub webgl: bool,
    pub webgpu_only: bool,
    pub theme: Option<ThemeKind>,
}

fn search_params() -> Option<web_sys::UrlSearchParams> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()
}

pub fn url_params() -> UrlParams {
    let Some(p) = search_params() else {
        return UrlParams { webgl: false, webgpu_only: false, theme: None };
    };
    let backend = p.get("backend").unwrap_or_default().to_lowercase();
    let theme = p.get("theme").and_then(|t| match t.to_lowercase().as_str() {
        "dark" | "ciemny" => Some(ThemeKind::AsOne),
        "light" | "jasny" => Some(ThemeKind::ProLight),
        "sepia" => Some(ThemeKind::Sepia),
        _ => None,
    });
    UrlParams {
        webgl: p.has("webgl") || backend == "webgl2" || backend == "webgl",
        webgpu_only: backend == "webgpu",
        theme,
    }
}

fn dispatch(name: &str, detail: &str) {
    let Some(window) = web_sys::window() else { return };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(detail));
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
        let _ = window.dispatch_event(&event);
    }
}

/// Przeładowanie strony w trybie zgodności WebGL2 (zachowuje pozostałe parametry, np. `?file=`).
fn fallback_to_webgl() -> bool {
    let (Some(window), Some(params)) = (web_sys::window(), search_params()) else { return false };
    params.set("webgl", "1");
    params.delete("backend");
    let query: String = params.to_string().into();
    window.location().set_search(&query).is_ok()
}

pub fn start() {
    let params = url_params();
    let backends = if params.webgl {
        wgpu::Backends::GL
    } else if params.webgpu_only {
        wgpu::Backends::BROWSER_WEBGPU
    } else {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    };
    wasm_bindgen_futures::spawn_local(async move {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let Some(canvas) = document
            .get_element_by_id("asone_canvas")
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
            web_sys::console::error_1(&"AsOne: brak <canvas id=\"asone_canvas\">".into());
            return;
        };
        let options = eframe::WebOptions {
            wgpu_options: crate::renderer::wgpu_configuration_with(Some(backends)),
            ..Default::default()
        };
        let started = eframe::WebRunner::new()
            .start(canvas, options, Box::new(|cc| Ok(Box::new(crate::app::AsOneApp::new(cc)))))
            .await;
        match started {
            Ok(()) => dispatch("asone-ready", if params.webgl { "webgl2" } else { "webgpu" }),
            Err(e) => {
                web_sys::console::error_1(&e);
                let message = e.as_string().unwrap_or_else(|| format!("{e:?}"));
                // Automatyczny fallback: WebGPU nie ruszyło → tryb WebGL2.
                if params.webgl || !fallback_to_webgl() {
                    dispatch("asone-error", &message);
                }
            }
        }
    });
}

// --- Pliki i komunikaty ze strony (loader.mjs) ---------------------------------------------

thread_local! {
    static INCOMING: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    static NOTICES: RefCell<Vec<(String, bool)>> = const { RefCell::new(Vec::new()) };
    static CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}

/// Aplikacja rejestruje kontekst, żeby pliki ze strony od razu wywoływały odświeżenie.
pub fn register_context(ctx: &egui::Context) {
    CTX.with(|c| *c.borrow_mut() = Some(ctx.clone()));
}

fn repaint() {
    CTX.with(|c| {
        if let Some(ctx) = c.borrow().as_ref() {
            ctx.request_repaint();
        }
    });
}

/// Wywoływane ze strony: plik pobrany z `?file=` / `?url=` / `?sample` – trafia na listę i do zakładek.
#[wasm_bindgen]
pub fn asone_open_file(name: String, data: Vec<u8>) {
    INCOMING.with(|q| q.borrow_mut().push((name, data)));
    repaint();
}

/// Wywoływane ze strony: komunikat dla użytkownika (np. nieudane pobranie pliku).
#[wasm_bindgen]
pub fn asone_notify(text: String, error: bool) {
    NOTICES.with(|q| q.borrow_mut().push((text, error)));
    repaint();
}

pub fn take_incoming() -> Vec<(String, Vec<u8>)> {
    INCOMING.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

pub fn take_notices() -> Vec<(String, bool)> {
    NOTICES.with(|q| std::mem::take(&mut *q.borrow_mut()))
}
