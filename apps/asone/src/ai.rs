//! Usuwanie tła przez lokalne modele BRIA RMBG (ONNX Runtime przez `ort`).
//!
//! * RMBG-1.4 – publiczny, plik do pobrania bez rejestracji,
//! * RMBG-2.0 – dokładniejszy (BiRefNet); repozytorium na Hugging Face jest „gated”: do pobrania
//!   potrzebny jest darmowy token (klucz API) i akceptacja licencji na stronie modelu.
//!
//! Model dostaje pomniejszoną kopię zdjęcia (≤1024 px), rozciągniętą do 1024×1024, i zwraca maskę
//! 1024×1024. Wątek roboczy skaluje ją (Lanczos3) do rozdzielczości zdjęcia, a UI wgrywa ją do
//! `mask_cpu` / `mask_texture`. Licencja modeli BRIA: użytek niekomercyjny bez umowy z BRIA.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use asone_core::imageio::resize_gray;
use crossbeam_channel::{Receiver, Sender};
use image::{GrayImage, RgbaImage};

use crate::document::DocId;
use crate::platform::Tasks;
pub use crate::settings::AiModel;

/// Czy ta kompilacja potrafi uruchomić model lokalnie.
pub const AVAILABLE: bool = cfg!(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")));

#[derive(Debug, Clone)]
pub enum AiError {
    ModelMissing(PathBuf),
    BackendUnavailable(String),
    Inference(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::ModelMissing(p) => write!(
                f,
                "Brak pliku modelu: {} – pobierz go w Ustawienia › Preferencje › Usuwanie tła (AI)",
                p.display()
            ),
            AiError::BackendUnavailable(s) => write!(f, "Usuwanie tła AI niedostępne: {s}"),
            AiError::Inference(s) => write!(f, "Błąd modelu: {s}"),
        }
    }
}

/// Model generujący maskę alfa obiektu.
pub trait BackgroundRemover: Send + Sync {
    /// Maska 1024×1024 dla obrazu rozciągniętego do 1024×1024: 0 = tło, 255 = obiekt.
    fn predict_mask(&self, model: AiModel, path: &Path, image: &RgbaImage) -> Result<GrayImage, AiError>;
}

pub const INPUT: u32 = 1024;

/// Wejście modelu: NCHW f32, obraz rozciągnięty do 1024×1024.
/// RMBG-1.4: `x/255 − 0.5`; RMBG-2.0: normalizacja ImageNet.
pub fn preprocess(image: &RgbaImage, model: AiModel) -> Result<Vec<f32>, String> {
    let small = asone_core::imageio::resize_rgba(
        image,
        [INPUT, INPUT],
        fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear),
    )?;
    let (mean, std) = match model {
        AiModel::Rmbg14 => ([0.5f32; 3], [1.0f32; 3]),
        AiModel::Rmbg20 => ([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
    };
    let plane = (INPUT * INPUT) as usize;
    let mut data = vec![0f32; 3 * plane];
    for (i, p) in small.pixels().enumerate() {
        for c in 0..3 {
            data[c * plane + i] = (p[c] as f32 / 255.0 - mean[c]) / std[c];
        }
    }
    Ok(data)
}

/// Wyjście modelu → maska 8-bit. Logity (wartości spoza 0..1) przechodzą przez sigmoid,
/// RMBG-1.4 jest dodatkowo rozciągany min–max (jak w przykładzie BRIA).
pub fn postprocess(raw: &[f32], model: AiModel) -> GrayImage {
    let logits = raw.iter().any(|&v| !(-0.001..=1.001).contains(&v));
    let mut v: Vec<f32> = if logits { raw.iter().map(|&x| 1.0 / (1.0 + (-x).exp())).collect() } else { raw.to_vec() };
    if model == AiModel::Rmbg14 {
        let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        if hi - lo > 1e-6 {
            v.iter_mut().for_each(|x| *x = (*x - lo) / (hi - lo));
        }
    }
    let side = (v.len() as f64).sqrt() as u32;
    GrayImage::from_raw(side, side, v.iter().map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8).collect())
        .unwrap_or_else(|| GrayImage::new(1, 1))
}

/// ONNX Runtime: sesja tworzona przy pierwszym użyciu i trzymana dla ostatniego pliku modelu.
#[derive(Default)]
pub struct OnnxBackgroundRemover {
    #[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    session: std::sync::Mutex<Option<(PathBuf, ort::session::Session)>>,
}

impl BackgroundRemover for OnnxBackgroundRemover {
    fn predict_mask(&self, model: AiModel, path: &Path, image: &RgbaImage) -> Result<GrayImage, AiError> {
        if !AVAILABLE {
            let _ = (model, image);
            return Err(AiError::BackendUnavailable("działa w wersji desktopowej (Linux, Windows, macOS)".into()));
        }
        if !path.is_file() {
            return Err(AiError::ModelMissing(path.to_path_buf()));
        }
        #[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let input = preprocess(image, model).map_err(AiError::Inference)?;
            let err = |e: ort::Error| AiError::Inference(e.to_string());
            let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
            if guard.as_ref().is_none_or(|(p, _)| p != path) {
                let env = ort::init().with_name("asone").build().map_err(err)?;
                let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
                let session = ort::session::Session::builder(&env)
                    .map_err(err)?
                    .with_intra_threads(threads)
                    .map_err(|e| AiError::Inference(e.to_string()))?
                    .commit_from_file(path)
                    .map_err(err)?;
                *guard = Some((path.to_path_buf(), session));
            }
            let Some((_, session)) = guard.as_mut() else { unreachable!() };
            let output_name = session
                .outputs()
                .last()
                .map(|o| o.name().to_string())
                .ok_or_else(|| AiError::Inference("model bez wyjść".into()))?;
            let tensor = ort::value::Tensor::from_array(([1usize, 3, INPUT as usize, INPUT as usize], input.into_boxed_slice()))
                .map_err(err)?;
            let outputs = session.run(ort::inputs![tensor]).map_err(err)?;
            let (_, raw) = outputs[output_name.as_str()].try_extract_tensor::<f32>().map_err(err)?;
            Ok(postprocess(raw, model))
        }
        #[cfg(not(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos"))))]
        unreachable!()
    }
}

pub struct AiJob {
    pub id: DocId,
    /// Pomniejszona kopia źródła (≤1024 px).
    pub image: Arc<RgbaImage>,
    /// Rozdzielczość maski docelowej (= rozmiar tekstury źródłowej).
    pub full_size: [u32; 2],
    pub model: AiModel,
    pub path: PathBuf,
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
    let small = remover.predict_mask(job.model, &job.path, &job.image)?;
    let (w, h) = small.dimensions();
    resize_gray(small.as_raw(), [w, h], job.full_size).map_err(AiError::Inference)
}

// --- Pobieranie modelu ---------------------------------------------------------------------

pub enum DownloadMsg {
    Progress { done: u64, total: Option<u64> },
    Done(PathBuf),
    Failed(String),
}

/// Pobiera model do `dest` (przez plik `.part`), z tokenem Hugging Face, jeśli podany.
#[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn download_model(model: AiModel, dest: PathBuf, token: String, ctx: egui::Context) -> Receiver<DownloadMsg> {
    let (tx, rx) = crossbeam_channel::unbounded();
    crate::platform::spawn_detached("download", move || {
        let send = |m| {
            let _ = tx.send(m);
            ctx.request_repaint();
        };
        let result = (|| -> Result<PathBuf, String> {
            use std::io::{Read, Write};
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let mut req = ureq::get(model.download_url());
            let token = token.trim();
            if !token.is_empty() {
                req = req.header("Authorization", &format!("Bearer {token}"));
            }
            let resp = req.call().map_err(|e| match e {
                ureq::Error::StatusCode(401) => {
                    "Hugging Face: 401 – brak lub błędny token (klucz API)".to_string()
                }
                ureq::Error::StatusCode(403) => format!(
                    "Hugging Face: 403 – zaakceptuj licencję modelu na {} (zalogowany tym samym kontem co token)",
                    model.page_url()
                ),
                other => format!("Pobieranie: {other}"),
            })?;
            let total = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let mut reader = resp.into_body().into_reader();
            let part = dest.with_extension("onnx.part");
            let mut file = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
            let mut buf = vec![0u8; 256 * 1024];
            let mut done = 0u64;
            let mut last = 0u64;
            loop {
                let n = reader.read(&mut buf).map_err(|e| format!("Pobieranie: {e}"))?;
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).map_err(|e| format!("Zapis: {e}"))?;
                done += n as u64;
                if done - last > 2 * 1024 * 1024 {
                    last = done;
                    send(DownloadMsg::Progress { done, total });
                }
            }
            file.flush().map_err(|e| format!("Zapis: {e}"))?;
            drop(file);
            if total.is_some_and(|t| t != done) {
                let _ = std::fs::remove_file(&part);
                return Err("Pobieranie przerwane (niepełny plik)".into());
            }
            std::fs::rename(&part, &dest).map_err(|e| format!("Zapis: {e}"))?;
            Ok(dest.clone())
        })();
        match result {
            Ok(p) => send(DownloadMsg::Done(p)),
            Err(e) => send(DownloadMsg::Failed(e)),
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brak_pliku_modelu() {
        let r = OnnxBackgroundRemover::default();
        let img = RgbaImage::new(4, 4);
        let e = r.predict_mask(AiModel::Rmbg14, Path::new("/nie/istnieje.onnx"), &img);
        if AVAILABLE {
            assert!(matches!(e, Err(AiError::ModelMissing(_))));
        } else {
            assert!(matches!(e, Err(AiError::BackendUnavailable(_))));
        }
    }

    #[test]
    fn przygotowanie_wejscia() {
        let img = RgbaImage::from_pixel(10, 20, image::Rgba([255, 0, 128, 255]));
        let v = preprocess(&img, AiModel::Rmbg14).unwrap();
        assert_eq!(v.len(), 3 * 1024 * 1024);
        assert!((v[0] - 0.5).abs() < 1e-3); // R = 1.0 − 0.5
        assert!((v[1024 * 1024] + 0.5).abs() < 1e-3); // G = 0 − 0.5
        let v = preprocess(&img, AiModel::Rmbg20).unwrap();
        assert!((v[0] - (1.0 - 0.485) / 0.229).abs() < 1e-3);
    }

    /// Prawdziwy model na przykładowym zdjęciu (kubek na jasnym tle).
    /// `cargo test -p asone rmbg14 -- --ignored` (plik w katalogu danych AsOne albo w ASONE_RMBG14).
    #[test]
    #[ignore = "wymaga pliku modelu RMBG-1.4"]
    fn rmbg14_na_przykladowym_zdjeciu() {
        let path = std::env::var("ASONE_RMBG14")
            .map(PathBuf::from)
            .unwrap_or_else(|_| crate::settings::default_models_dir().join(AiModel::Rmbg14.file_name()));
        assert!(path.is_file(), "brak modelu: {}", path.display());
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sample/produkt-1.jpg");
        let img = asone_core::imageio::decode_path(&sample).unwrap();
        let t = std::time::Instant::now();
        let mask = OnnxBackgroundRemover::default().predict_mask(AiModel::Rmbg14, &path, &img).unwrap();
        eprintln!("RMBG-1.4: {:?}", t.elapsed());
        if let Ok(out) = std::env::var("ASONE_MASK_OUT") {
            mask.save(out).unwrap();
        }
        assert_eq!(mask.dimensions(), (INPUT, INPUT));
        let at = |fx: f32, fy: f32| mask.get_pixel((fx * 1023.0) as u32, (fy * 1023.0) as u32)[0];
        // Korpus kubka (środek zdjęcia) = obiekt, rogi = tło.
        assert!(at(0.5, 0.5) > 200, "środek {}", at(0.5, 0.5));
        assert!(at(0.03, 0.03) < 40 && at(0.97, 0.97) < 40, "rogi {} {}", at(0.03, 0.03), at(0.97, 0.97));
    }

    #[test]
    fn maska_z_wyjscia() {
        // RMBG-1.4: rozciągnięcie min–max.
        let m = postprocess(&[0.25, 0.25, 0.75, 0.5], AiModel::Rmbg14);
        assert_eq!(m.dimensions(), (2, 2));
        assert_eq!(m.as_raw(), &vec![0, 0, 255, 128]);
        // Logity → sigmoid.
        let m = postprocess(&[-10.0, 10.0, 0.0, 0.0], AiModel::Rmbg20);
        assert_eq!(m.as_raw()[..2], [0, 255]);
        assert_eq!(m.as_raw()[2], 128);
    }
}
