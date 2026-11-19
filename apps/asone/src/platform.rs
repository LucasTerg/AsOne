//! Różnice między platformami: zadania w tle, źródła plików, okna wyboru, zapis.
//!
//! | platforma              | zadania w tle        | wejście                      | wyjście                 |
//! |------------------------|----------------------|------------------------------|-------------------------|
//! | Linux/BSD/Windows/macOS | pula wątków `rayon` | Drag & Drop ścieżek, `rfd`   | folder na dysku         |
//! | Android                | pula wątków `rayon`  | folder wpisany w aplikacji   | `Pictures/AsOne`        |
//! | wasm (przeglądarka)    | kolejka na kolejne klatki | Drag & Drop bajtów, `rfd` | pobieranie plików     |

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use image::RgbaImage;

pub use web_time::Instant;

/// Skąd pochodzi zdjęcie.
#[derive(Clone)]
pub enum ImageSource {
    /// Plik na dysku (desktop, Android).
    Path(PathBuf),
    /// Bajty pliku (przeglądarka: Drag & Drop / wybór pliku).
    Bytes { name: String, bytes: Arc<[u8]> },
}

impl ImageSource {
    pub fn name(&self) -> String {
        match self {
            ImageSource::Path(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned()),
            ImageSource::Bytes { name, .. } => name.clone(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            ImageSource::Path(p) => Some(p),
            ImageSource::Bytes { .. } => None,
        }
    }

    pub fn size_hint(&self) -> u64 {
        match self {
            ImageSource::Path(p) => std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
            ImageSource::Bytes { bytes, .. } => bytes.len() as u64,
        }
    }

    /// Klucz do wykrywania duplikatów na liście plików.
    pub fn key(&self) -> String {
        match self {
            ImageSource::Path(p) => p.canonicalize().unwrap_or_else(|_| p.clone()).to_string_lossy().into_owned(),
            ImageSource::Bytes { name, bytes } => format!("{name}:{}", bytes.len()),
        }
    }

    pub fn decode(&self) -> Result<RgbaImage, String> {
        match self {
            ImageSource::Path(p) => asone_core::imageio::decode_path(p),
            ImageSource::Bytes { bytes, .. } => asone_core::imageio::decode_bytes(bytes),
        }
    }
}

// --- Zadania w tle -------------------------------------------------------------------------

/// Desktop/Android: pula wątków.
#[cfg(not(target_arch = "wasm32"))]
pub struct Tasks(rayon::ThreadPool);

#[cfg(not(target_arch = "wasm32"))]
impl Tasks {
    pub fn new(name: &'static str, max_threads: usize) -> Self {
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, max_threads);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(move |i| format!("asone-{name}-{i}"))
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new().num_threads(1).build().expect("pula wątków")
            });
        Self(pool)
    }

    pub fn spawn(&self, job: impl FnOnce() + Send + 'static) {
        self.0.spawn(job);
    }
}

/// Przeglądarka (bez wątków): zadania czekają w kolejce i są wykonywane między klatkami.
#[cfg(target_arch = "wasm32")]
pub struct Tasks;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static QUEUE: std::cell::RefCell<std::collections::VecDeque<Box<dyn FnOnce()>>> = Default::default();
}

#[cfg(target_arch = "wasm32")]
impl Tasks {
    pub fn new(_name: &'static str, _max_threads: usize) -> Self {
        Tasks
    }

    pub fn spawn(&self, job: impl FnOnce() + 'static) {
        QUEUE.with(|q| q.borrow_mut().push_back(Box::new(job)));
    }
}

/// Wykonuje zaległe zadania (tylko w przeglądarce) przez maks. `budget_ms`.
/// Zwraca `true`, jeśli w kolejce coś zostało.
pub fn run_pending_tasks(budget_ms: f64) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let start = Instant::now();
        loop {
            let Some(job) = QUEUE.with(|q| q.borrow_mut().pop_front()) else { return false };
            job();
            if start.elapsed().as_secs_f64() * 1000.0 >= budget_ms {
                return QUEUE.with(|q| !q.borrow().is_empty());
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = budget_ms;
        false
    }
}

/// Uruchamia długą operację w osobnym wątku (desktop) albo jako zadanie w kolejce (wasm).
pub fn spawn_detached(name: &str, job: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let spawned = std::thread::Builder::new().name(format!("asone-{name}")).spawn(job);
        if let Err(e) = spawned {
            eprintln!("asone: nie udało się uruchomić wątku {name}: {e}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        Tasks.spawn(job);
    }
}

// --- Wejście / wyjście ---------------------------------------------------------------------

/// Czy platforma ma systemowe okno wyboru plików / folderów.
pub const CAN_PICK_FILES: bool = !cfg!(target_os = "android");
pub const CAN_PICK_FOLDER: bool = !cfg!(any(target_os = "android", target_arch = "wasm32"));
/// Eksport trafia do folderu (desktop/Android) czy do pobranych plików (przeglądarka).
pub const EXPORT_TO_FOLDER: bool = !cfg!(target_arch = "wasm32");

/// Domyślny folder, z którego Android proponuje wczytać zdjęcia.
pub const ANDROID_DEFAULT_INPUT: &str = "/storage/emulated/0/DCIM/Camera";
/// Domyślny folder eksportu na Androidzie.
pub const ANDROID_DEFAULT_OUTPUT: &str = "/storage/emulated/0/Pictures/AsOne";

/// Operacje na plikach z Asystenta PIM (otwórz, GIMP, kosz, ZIP, PDF) – tam, gdzie są ścieżki.
pub const FILE_OPS: bool = !cfg!(any(target_os = "android", target_arch = "wasm32"));
/// Czy można odczytać schowek przyciskiem „Wklej”.
pub const CAN_PASTE: bool = !cfg!(target_os = "android");

/// Do czego służy plik wybrany w oknie „Zapisz jako”.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    ZipSelected,
    ZipExport,
    PdfSelected,
}

pub enum PickerEvent {
    /// Wybrane / upuszczone obrazy i liczba pominiętych (nieobsługiwanych) plików.
    Files(Vec<ImageSource>, usize),
    OutputFolder(PathBuf),
    /// Tekst ze schowka (przycisk „Wklej”).
    Pasted(String),
    /// Plik modelu AI wybrany w Preferencjach.
    ModelFile(PathBuf),
    SaveTarget(SaveKind, PathBuf),
}

/// Okna wyboru plików działające bez blokowania UI (wynik przychodzi kanałem).
pub struct Pickers {
    #[cfg_attr(target_os = "android", allow(dead_code))]
    tx: Sender<PickerEvent>,
    pub rx: Receiver<PickerEvent>,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    ctx: egui::Context,
}

impl Pickers {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx, ctx }
    }

    /// Okno „Dodaj zdjęcia…”.
    pub fn pick_images(&self) {
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            run_dialog(move || {
                let files = rfd::FileDialog::new()
                    .set_title("Dodaj zdjęcia")
                    .add_filter("Obrazy", asone_core::imageio::SUPPORTED_EXTENSIONS)
                    .pick_files();
                if let Some(files) = files {
                    let _ = tx.send(PickerEvent::Files(files.into_iter().map(ImageSource::Path).collect(), 0));
                    ctx.request_repaint();
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let picked = rfd::AsyncFileDialog::new()
                    .add_filter("Obrazy", asone_core::imageio::SUPPORTED_EXTENSIONS)
                    .pick_files()
                    .await;
                if let Some(files) = picked {
                    let mut out = Vec::with_capacity(files.len());
                    for f in files {
                        let bytes = f.read().await;
                        out.push(ImageSource::Bytes { name: f.file_name(), bytes: bytes.into() });
                    }
                    let _ = tx.send(PickerEvent::Files(out, 0));
                    ctx.request_repaint();
                }
            });
        }
    }

    /// Przeglądarka: czyta zawartość upuszczonych plików (asynchronicznie).
    #[cfg(target_arch = "wasm32")]
    pub fn read_dropped(&self, files: Vec<egui::DroppedFileHandle>) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let mut out = Vec::new();
            let mut skipped = 0;
            for f in files {
                if !asone_core::imageio::is_supported(f.path()) {
                    skipped += 1;
                    continue;
                }
                let name = f.path().to_string_lossy().into_owned();
                match f.bytes_async().await {
                    Ok(bytes) => out.push(ImageSource::Bytes { name, bytes: bytes.into() }),
                    Err(_) => skipped += 1,
                }
            }
            let _ = tx.send(PickerEvent::Files(out, skipped));
            ctx.request_repaint();
        });
    }

    /// Okno wyboru folderu z obrazami (Plik › Dodaj folder…).
    pub fn pick_image_folder(&self) {
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            run_dialog(move || {
                if let Some(dir) = rfd::FileDialog::new().set_title("Dodaj folder ze zdjęciami").pick_folder() {
                    let files = asone_core::imageio::expand_paths([dir]);
                    let _ = tx.send(PickerEvent::Files(files.into_iter().map(ImageSource::Path).collect(), 0));
                    ctx.request_repaint();
                }
            });
        }
    }

    /// Okno wyboru pliku modelu ONNX (Preferencje › Usuwanie tła).
    pub fn pick_model_file(&self, start: Option<PathBuf>) {
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            run_dialog(move || {
                let mut dialog = rfd::FileDialog::new().set_title("Plik modelu ONNX").add_filter("Model ONNX", &["onnx"]);
                if let Some(dir) = start.filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(path) = dialog.pick_file() {
                    let _ = tx.send(PickerEvent::ModelFile(path));
                    ctx.request_repaint();
                }
            });
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let _ = start;
    }

    /// Okno „Zapisz jako” (ZIP / PDF).
    pub fn pick_save_target(&self, kind: SaveKind, default_name: String, start: Option<PathBuf>) {
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            run_dialog(move || {
                let (label, ext) = match kind {
                    SaveKind::ZipSelected | SaveKind::ZipExport => ("Archiwum ZIP", "zip"),
                    SaveKind::PdfSelected => ("Dokument PDF", "pdf"),
                };
                let mut dialog = rfd::FileDialog::new().set_file_name(default_name).add_filter(label, &[ext]);
                if let Some(dir) = start.filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(mut path) = dialog.save_file() {
                    if path.extension().is_none() {
                        path.set_extension(ext);
                    }
                    let _ = tx.send(PickerEvent::SaveTarget(kind, path));
                    ctx.request_repaint();
                }
            });
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let _ = (kind, default_name, start);
    }

    /// Odczyt tekstu ze schowka systemowego (desktop: arboard, także Wayland; przeglądarka: Clipboard API).
    pub fn paste_text(&self) {
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            let text = arboard::Clipboard::new().and_then(|mut c| c.get_text());
            match text {
                Ok(t) => {
                    let _ = self.tx.send(PickerEvent::Pasted(t));
                }
                Err(e) => eprintln!("asone: schowek: {e}"),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let Some(window) = web_sys::window() else { return };
                let promise = window.navigator().clipboard().read_text();
                if let Ok(v) = wasm_bindgen_futures::JsFuture::from(promise).await {
                    if let Some(t) = v.as_string() {
                        let _ = tx.send(PickerEvent::Pasted(t));
                        ctx.request_repaint();
                    }
                }
            });
        }
    }

    /// Okno wyboru folderu docelowego eksportu (desktop).
    pub fn pick_output_folder(&self, start: Option<PathBuf>) {
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            run_dialog(move || {
                let mut dialog = rfd::FileDialog::new().set_title("Folder docelowy eksportu");
                if let Some(dir) = start.filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(dir) = dialog.pick_folder() {
                    let _ = tx.send(PickerEvent::OutputFolder(dir));
                    ctx.request_repaint();
                }
            });
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let _ = start;
    }
}

/// Linux/BSD: okno w osobnym wątku (portal XDG nie blokuje wtedy pętli zdarzeń).
/// Windows/macOS: okna systemowe muszą powstać w wątku głównym – są modalne.
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn run_dialog(f: impl FnOnce() + Send + 'static) {
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        f();
    } else {
        spawn_detached("dialog", f);
    }
}

/// Przeglądarka: zapisuje bajty jako pobierany plik.
#[cfg(target_arch = "wasm32")]
pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("brak window")?;
    let document = window.document().ok_or("brak document")?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(js)?
        .dyn_into()
        .map_err(|_| "a".to_string())?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    // URL zostaje do końca sesji: natychmiastowe zwolnienie potrafi przerwać pobieranie.
    Ok(())
}

/// Otwiera plik w domyślnym programie (desktop).
pub fn open_path(path: &Path) {
    open_in_file_manager(path);
}

/// Otwiera plik w GIMP-ie (zwykła instalacja albo Flatpak).
pub fn open_in_gimp(path: &Path) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    {
        use std::process::Command;
        let in_flatpak = Path::new("/.flatpak-info").exists();
        let tries: Vec<(&str, Vec<std::ffi::OsString>)> = if cfg!(target_os = "macos") {
            vec![("open", vec!["-a".into(), "GIMP".into(), path.into()])]
        } else if in_flatpak {
            // Piaskownica Flatpaka nie uruchamia programów hosta (bez dodatkowego uprawnienia).
            return Err("W wersji Flatpak otwórz zdjęcie w GIMP-ie ręcznie (Otwórz folder pliku)".into());
        } else {
            vec![
                ("gimp", vec![path.into()]),
                ("flatpak", vec!["run".into(), "org.gimp.GIMP".into(), path.into()]),
            ]
        };
        for (program, args) in tries {
            if Command::new(program).args(&args).spawn().is_ok() {
                return Ok(());
            }
        }
        Err("Nie znaleziono GIMP-a (gimp ani flatpak org.gimp.GIMP)".into())
    }
    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    {
        let _ = path;
        Err("Niedostępne na tej platformie".into())
    }
}

/// Przenosi pliki do systemowego kosza (desktop).
pub fn move_to_trash(paths: &[PathBuf]) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    {
        trash::delete_all(paths).map_err(|e| format!("Kosz: {e}"))
    }
    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    {
        let _ = paths;
        Err("Kosz niedostępny na tej platformie".into())
    }
}

/// Otwiera folder w systemowym menedżerze plików (desktop).
pub fn open_in_file_manager(dir: &Path) {
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    {
        let in_flatpak = Path::new("/.flatpak-info").exists();
        if in_flatpak {
            let _ = std::process::Command::new("flatpak-spawn")
                .args(["--host", "xdg-open"])
                .arg(dir)
                .spawn();
            return;
        }
        let program = if cfg!(target_os = "windows") {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        if let Err(e) = std::process::Command::new(program).arg(dir).spawn() {
            eprintln!("asone: nie można otworzyć {}: {e}", dir.display());
        }
    }
    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    let _ = dir;
}

/// Otwiera URL w domyślnej przeglądarce internetowej (desktop/flatpak/wasm).
pub fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let in_flatpak = Path::new("/.flatpak-info").exists();
        if in_flatpak {
            if std::process::Command::new("flatpak-spawn")
                .args(["--host", "xdg-open", url])
                .spawn()
                .is_ok()
            {
                return;
            }
        }
        let program = if cfg!(target_os = "windows") {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        if let Err(e) = std::process::Command::new(program).arg(url).spawn() {
            eprintln!("asone: nie można otworzyć URL {url}: {e}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(win) = web_sys::window() {
            let _ = win.open_with_url_and_target(url, "_blank");
        }
    }
}
