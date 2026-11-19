//! Usuwanie tła przez lokalny model AI – interfejs gotowy do podpięcia `ort` (ONNX Runtime).
//!
//! Aplikacja zna tylko trait [`BackgroundRemover`]. [`AiWorker`] (zadanie w tle) przyjmuje
//! pomniejszoną kopię zdjęcia, woła `predict_mask`, skaluje maskę do pełnej rozdzielczości
//! (Lanczos3) i odsyła ją do UI, które wgrywa ją do `mask_cpu` / `mask_texture`.
//!
//! Aby podpiąć prawdziwy model (np. `rmbg-1.4.onnx`):
//! 1. dodać do `Cargo.toml` zależność `ort` (i ewentualnie `ndarray`),
//! 2. w [`OnnxBackgroundRemover::predict_mask`] zbudować sesję z `self.model_path`,
//!    przeskalować wejście do 1024×1024, znormalizować (RMBG-1.4: `x / 255 - 0.5`),
//!    uruchomić model i zwrócić wyjście jako `GrayImage` (0 = tło, 255 = obiekt).
//!
//! Reszta potoku (wątek, skalowanie maski, GPU, UI z paskiem postępu) już działa.

use std::path::PathBuf;
use std::sync::Arc;

use asone_core::imageio::resize_gray;
use crossbeam_channel::{Receiver, Sender};
use image::{GrayImage, RgbaImage};

use crate::document::DocId;
use crate::platform::Tasks;

pub const DEFAULT_MODEL_PATH: &str = "models/rmbg-1.4.onnx";

#[derive(Debug, Clone)]
pub enum AiError {
    ModelMissing(PathBuf),
    BackendUnavailable(String),
    Inference(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::ModelMissing(p) => write!(f, "Brak pliku modelu: {}", p.display()),
            AiError::BackendUnavailable(s) => write!(f, "Moduł AI niedostępny: {s}"),
            AiError::Inference(s) => write!(f, "Błąd modelu: {s}"),
        }
    }
}

/// Model generujący maskę alfa obiektu.
pub trait BackgroundRemover: Send + Sync {
    fn name(&self) -> &str;
    /// Zwraca maskę w rozdzielczości wejścia: 0 = tło, 255 = obiekt.
    fn predict_mask(&self, image: &RgbaImage) -> Result<GrayImage, AiError>;
}

/// Zaślepka modelu ONNX – miejsce na integrację z `ort`.
pub struct OnnxBackgroundRemover {
    pub model_path: PathBuf,
}

impl BackgroundRemover for OnnxBackgroundRemover {
    fn name(&self) -> &str {
        "RMBG-1.4 (ONNX)"
    }

    fn predict_mask(&self, _image: &RgbaImage) -> Result<GrayImage, AiError> {
        if !self.model_path.exists() {
            return Err(AiError::ModelMissing(self.model_path.clone()));
        }
        Err(AiError::BackendUnavailable(
            "ONNX Runtime (ort) nie jest jeszcze wkompilowany".into(),
        ))
    }
}

pub struct AiJob {
    pub id: DocId,
    /// Pomniejszona kopia źródła (≤1024 px).
    pub image: Arc<RgbaImage>,
    /// Rozdzielczość maski docelowej (= rozmiar tekstury źródłowej).
    pub full_size: [u32; 2],
}

pub enum AiResult {
    Done { id: DocId, mask: Vec<u8> },
    Failed { id: DocId, error: String },
}

pub struct AiWorker {
    tasks: Tasks,
    remover: Arc<dyn BackgroundRemover>,
    tx: Sender<AiResult>,
    pub rx: Receiver<AiResult>,
    ctx: egui::Context,
}

impl AiWorker {
    pub fn new(remover: Arc<dyn BackgroundRemover>, ctx: egui::Context) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        // Jeden wątek: model AI zajmuje dużo pamięci, zadania idą po kolei.
        Self { tasks: Tasks::new("ai", 1), remover, tx, rx, ctx }
    }

    pub fn submit(&self, job: AiJob) {
        let (remover, tx, ctx) = (self.remover.clone(), self.tx.clone(), self.ctx.clone());
        self.tasks.spawn(move || {
            let result = match run_job(remover.as_ref(), &job) {
                Ok(mask) => AiResult::Done { id: job.id, mask },
                Err(e) => AiResult::Failed { id: job.id, error: e.to_string() },
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
}

fn run_job(remover: &dyn BackgroundRemover, job: &AiJob) -> Result<Vec<u8>, AiError> {
    let small = remover.predict_mask(&job.image)?;
    let (w, h) = small.dimensions();
    resize_gray(small.as_raw(), [w, h], job.full_size).map_err(AiError::Inference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaslepka_zglasza_brak_modelu() {
        let r = OnnxBackgroundRemover { model_path: "/nie/istnieje.onnx".into() };
        let img = RgbaImage::new(4, 4);
        assert!(matches!(r.predict_mask(&img), Err(AiError::ModelMissing(_))));
    }
}
