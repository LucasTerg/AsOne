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
