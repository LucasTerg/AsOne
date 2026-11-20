//! Dokument = jedno wrzucone zdjęcie wraz z niedestrukcyjnymi ustawieniami edycji.
//!
//! Oryginał pliku nigdy nie jest modyfikowany: na GPU leży niezmieniona tekstura
//! źródłowa (`source_texture`) i osobna maska (`mask_texture`), a kadr, marginesy
//! i tło są tylko parametrami (`EditParams`) nakładanymi w shaderze.

use std::sync::Arc;

pub use asone_core::edits::*;

use crate::canvas::CanvasState;
use crate::platform::ImageSource;
use crate::renderer::DocGpu;

/// Stabilny identyfikator dokumentu (nie zmienia się przy zmianie kolejności zakładek).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocId(pub u64);

/// Kamera 2D podglądu: `screen = canvas.min + pan + src_px * zoom` (w punktach egui).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub zoom: f32,
    pub pan: egui::Vec2,
    /// `true` = przy następnej klatce dopasuj widok do okna.
    pub needs_fit: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            needs_fit: true,
        }
    }
}

#[derive(Debug)]
pub enum LoadState {
    NotLoaded,
    Loading,
    Ready,
    Failed(String),
}

pub enum ThumbState {
    Loading,
    Ready(egui::TextureHandle),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiState {
    Idle,
    Running,
    Failed(String),
}

pub struct Document {
    pub id: DocId,
    /// Plik na dysku (desktop, Android) albo bajty upuszczone w przeglądarce.
    pub source: ImageSource,
    pub file_name: String,
    pub file_size: u64,
    /// Wymiary po uwzględnieniu orientacji EXIF (znane po miniaturze lub pełnym wczytaniu).
    pub dims: Option<[u32; 2]>,
    /// Jeśli oryginał przekraczał limit tekstury GPU: jego pierwotny rozmiar.
    pub downscaled_from: Option<[u32; 2]>,
    pub thumb: ThumbState,
    pub load: LoadState,
    /// Zasoby GPU: `source_texture`, `mask_texture` (R8Unorm), bufor parametrów, bind group.
    pub gpu: Option<DocGpu>,
    /// Kopia maski w RAM (1 bajt/piksel, 255 = widoczne) do szybkiej edycji gumką.
    pub mask_cpu: Vec<u8>,
    /// Obszar maski zmieniony od ostatniego wysłania na GPU.
    pub mask_dirty: Option<PxRect>,
    /// Maska coś zmienia (PNG z przezroczystością, gumka, prostokąt, AI) – wtedy pokazujemy „Maska aktywna”.
    pub mask_touched: bool,
    pub edits: EditParams,
    /// Kłódka „Spnij wszystkie boki”.
    pub margins_linked: bool,
    pub camera: Camera,
    /// Zaznaczenie narzędzia prostokąta (px źródła).
    pub selection: Option<PxRect>,
    pub ai: AiState,
    /// Pomniejszona kopia źródła (≤1024 px) jako wejście dla modelu AI.
    pub ai_proxy: Option<Arc<image::RgbaImage>>,
    pub canvas: CanvasState,
    /// Cofnij / Ponów.
    pub history: crate::history::History,
    /// Dopisek wersji w nazwie wynikowej: `Some(2)` → `123-marka-2-ver2.jpg`.
    pub version: Option<u8>,
    /// Obraz warstwy (np. wklejony smartfon); położenie w `edits.overlay`.
    pub overlay_image: Option<Arc<image::RgbaImage>>,
    /// Podgląd warstwy na płótnie (pomniejszony do [`OVERLAY_PREVIEW`] px).
    pub overlay_texture: Option<egui::TextureHandle>,
    /// Zawartość po „Spłaszcz” – zastępuje plik źródłowy (Wczytaj ponownie wraca do pliku).
    pub content: Option<Arc<image::RgbaImage>>,
    /// Praca w tle na zdjęciu (LaMa, waifu2x, edycja AI, spłaszczanie) – karta na środku płótna.
    pub busy: Option<Busy>,
}

/// Zadanie w tle na zdjęciu: zdjęcie zostaje widoczne (przyciemnione), edycja czeka na wynik.
pub struct Busy {
    pub label: &'static str,
    pub started: crate::platform::Instant,
    /// Postęp 0–1000, gdy zadanie go zna (np. kafelki waifu2x).
    pub progress: Option<Arc<std::sync::atomic::AtomicU32>>,
}

impl Busy {
    pub fn new(label: &'static str) -> Self {
        Self { label, started: crate::platform::Instant::now(), progress: None }
    }

    /// Z licznikiem postępu dla zadania (0–1000).
    pub fn with_progress(label: &'static str) -> (Self, Arc<std::sync::atomic::AtomicU32>) {
        let progress = Arc::new(std::sync::atomic::AtomicU32::new(0));
        (Self { progress: Some(progress.clone()), ..Self::new(label) }, progress)
    }

    /// Postęp 0..1, gdy znany.
    pub fn fraction(&self) -> Option<f32> {
        self.progress.as_ref().map(|p| p.load(std::sync::atomic::Ordering::Relaxed).min(1000) as f32 / 1000.0)
    }
}

/// Dłuższy bok tekstury podglądu warstwy.
pub const OVERLAY_PREVIEW: u32 = 2048;

/// „Wklej smartfon”: tyle px białego tła dokładane z lewej strony zdjęcia (miejsce na warstwę).
pub const PHONE_STRIP: u32 = 500;

/// Prostokąt warstwy o proporcjach `aspect` (szer./wys.) wyśrodkowany w pasie `strip` × `height`
/// (od lewej krawędzi obrazu wynikowego), z odstępem 6 % szerokości pasa.
pub fn fit_in_strip(strip: u32, height: u32, aspect: f32) -> [f32; 4] {
    let pad = strip as f32 * 0.06;
    let (bw, bh) = ((strip as f32 - 2.0 * pad).max(1.0), (height as f32 - 2.0 * pad).max(1.0));
    let (w, h) = if bw / bh > aspect { (bh * aspect, bh) } else { (bw, bw / aspect) };
    [(strip as f32 - w) / 2.0, (height as f32 - h) / 2.0, w, h]
}

impl Document {
    pub fn new(id: DocId, source: ImageSource) -> Self {
        let file_name = source.name();
        let file_size = source.size_hint();
        Self {
            id,
            source,
            file_name,
            file_size,
            dims: None,
            downscaled_from: None,
            thumb: ThumbState::Loading,
            load: LoadState::NotLoaded,
            gpu: None,
            mask_cpu: Vec::new(),
            mask_dirty: None,
            mask_touched: false,
            edits: EditParams::default(),
            margins_linked: false,
            camera: Camera::default(),
            selection: None,
            ai: AiState::Idle,
            ai_proxy: None,
            canvas: CanvasState::default(),
            history: crate::history::History::default(),
            version: None,
            overlay_image: None,
            overlay_texture: None,
            content: None,
            busy: None,
        }
    }

    /// Wołać przed każdą zmianą `mask_cpu` (do Cofnij / Ponów).
    pub fn begin_mask_change(&mut self) {
        self.history.begin_mask_change(&self.mask_cpu);
    }

    /// Zatwierdza zmiany od ostatniego kroku historii (gdy nic nie jest przeciągane).
    pub fn commit_history(&mut self) -> bool {
        let width = self.src_size().map_or(0, |s| s[0]);
        self.history.commit(&self.edits, self.mask_touched, &self.mask_cpu, width)
    }

    /// Nowa historia (po wczytaniu lub przeładowaniu pliku).
    pub fn reset_history(&mut self) {
        self.history = crate::history::History::default();
        self.commit_history(); // stan bazowy
    }

    /// Cofnij (`redo = false`) lub Ponów (`redo = true`); `true`, gdy coś się zmieniło.
    pub fn step_history(&mut self, redo: bool) -> bool {
        self.commit_history();
        let width = self.src_size().map_or(0, |s| s[0]);
        let mut history = std::mem::take(&mut self.history);
        let target = crate::history::Target {
            edits: &mut self.edits,
            mask_touched: &mut self.mask_touched,
            mask: &mut self.mask_cpu,
            width,
        };
        let result = if redo { history.redo(target) } else { history.undo(target) };
        self.history = history;
        match result {
            Some(dirty) => {
                self.mask_dirty = union_opt(self.mask_dirty, dirty);
                true
            }
            None => false,
        }
    }

    /// „Wklej smartfon”: pierwsza warstwa dokłada [`PHONE_STRIP`] px białego tła z lewej strony
    /// i mieści się w tym pasie; kolejna podmienia obraz w tym samym miejscu (ta sama wysokość).
    pub fn set_overlay(&mut self, image: image::RgbaImage) {
        let Some(src) = self.src_size() else { return };
        let aspect = image.width().max(1) as f32 / image.height().max(1) as f32;
        self.edits.overlay = Some(match self.edits.overlay {
            Some([x, y, w, h]) => {
                let nw = h * aspect;
                [x + (w - nw) / 2.0, y, nw, h]
            }
            None => {
                self.edits.margins[LEFT] += PHONE_STRIP;
                self.margins_linked = false;
                fit_in_strip(PHONE_STRIP, self.edits.output_size(src)[1], aspect)
            }
        });
        self.overlay_image = Some(Arc::new(image));
        self.overlay_texture = None;
    }

    /// Warstwa z powrotem w lewym pasie (cała szerokość lewego marginesu), wyśrodkowana.
    pub fn fit_overlay_to_strip(&mut self) {
        let (Some(src), Some(img), Some(_)) = (self.src_size(), self.overlay_image.as_ref(), self.edits.overlay) else {
            return;
        };
        let aspect = img.width().max(1) as f32 / img.height().max(1) as f32;
        let strip = self.edits.margins[LEFT].max(1);
        self.edits.overlay = Some(fit_in_strip(strip, self.edits.output_size(src)[1], aspect));
    }

    /// Usuwa warstwę razem z dołożonym pasem (obraz zostaje w dokumencie dla „Cofnij”).
    pub fn remove_overlay(&mut self) {
        if self.edits.overlay.take().is_some() {
            self.edits.margins[LEFT] = self.edits.margins[LEFT].saturating_sub(PHONE_STRIP);
        }
    }

    /// Piksele zdjęcia: po „Spłaszcz” – spłaszczony obraz, w przeciwnym razie plik źródłowy.
    pub fn pixel_source(&self) -> ImageSource {
        match &self.content {
            Some(image) => ImageSource::Pixels { name: self.file_name.clone(), image: image.clone() },
            None => self.source.clone(),
        }
    }

    /// Rozmiar tekstury źródłowej (do niego odnoszą się kadr, marginesy i maska).
    pub fn src_size(&self) -> Option<[u32; 2]> {
        self.gpu.as_ref().map(|g| g.size)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.load, LoadState::Ready) && self.gpu.is_some()
    }
}
