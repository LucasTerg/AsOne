//! Eksport „Zapisz z zakładek”.
//!
//! * Desktop/Android: GPU offscreen (ten sam shader co podgląd) → pula `rayon`:
//!   Lanczos3 (`fast_image_resize`) → MozJPEG Progressive → zapis atomowy do folderu.
//! * Przeglądarka: odczyt z GPU jest tam asynchroniczny, więc wynik składa rdzeń na CPU
//!   (identyczna semantyka: `asone_core::compose`), a pliki trafiają do „Pobranych”.
//!
//! W obu przypadkach plik powstaje z czystych pikseli – bez EXIF, GPS, ICC i XMP.

use std::time::Duration;

use asone_core::batch::bg_rgb8;
use asone_core::edits::EditParams;
use asone_core::finish::{ExportLimits, Finished, finish_with_margins};
use crossbeam_channel::Receiver;

#[cfg(not(target_arch = "wasm32"))]
use crate::renderer::OffscreenSource;

#[derive(Clone, Debug)]
pub struct ExportSettings {
    /// Jakość JPEG 1..=100.
    pub quality: u8,
    /// Limit dłuższego boku w px (`None` = bez zmniejszania).
    pub max_long_side: Option<u32>,
    /// Folder docelowy (desktop/Android). W przeglądarce pliki są pobierane.
    pub out_dir: std::path::PathBuf,
    /// Limity z Preferencji (domyślnie 3000×3600 i 3 MB) – pilnowane zawsze.
    pub limits: ExportLimits,
}

/// Jeden plik do zapisania (migawka stanu dokumentu z chwili kliknięcia).
pub struct ExportItem {
    pub file_name: String,
    pub edits: EditParams,
    /// Obraz warstwy (smartfon); położenie w `edits.overlay`.
    pub overlay: Option<std::sync::Arc<image::RgbaImage>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub gpu: OffscreenSource,
    #[cfg(target_arch = "wasm32")]
    pub source: crate::platform::ImageSource,
    #[cfg(target_arch = "wasm32")]
    pub mask: Vec<u8>,
    #[cfg(target_arch = "wasm32")]
    pub max_texture_dim: u32,
}

#[derive(Debug)]
pub enum ExportMsg {
    Saved { file_name: String, bytes: u64, size: [u32; 2], quality: u8, notes: Vec<String> },
    Failed { file_name: String, error: String },
    Finished { elapsed: Duration },
}

/// Warstwa (smartfon), zmniejszenie, limity i kompresja jednego obrazu.
fn finish_image(
    size: [u32; 2],
    mut rgb: Vec<u8>,
    edits: &EditParams,
    overlay: Option<&image::RgbaImage>,
    settings: &ExportSettings,
) -> Result<Finished, String> {
    if let (Some(img), Some(rect)) = (overlay, edits.overlay) {
        asone_core::compose::draw_overlay(size, &mut rgb, img, rect)?;
    }
    // Z warstwą w marginesie ramka nie jest odtwarzana osobno (warstwa zostałaby zamalowana tłem).
    let margins = if edits.overlay.is_some() { [0; 4] } else { edits.margins };
    finish_with_margins(size, rgb, margins, settings.max_long_side, settings.quality, bg_rgb8(edits), &settings.limits)
}

fn saved(file_name: String, f: Finished) -> ExportMsg {
    ExportMsg::Saved { file_name, bytes: f.jpeg.len() as u64, size: f.size, quality: f.quality, notes: f.notes }
}

/// Zapis gotowego JPG: plik (desktop) albo Galeria (Android, MediaStore przez Kotlin).
#[cfg(not(target_arch = "wasm32"))]
fn store(target: &std::path::Path, jpeg: &[u8]) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        crate::android::save_image(&name, jpeg).map(drop)
    }
    #[cfg(not(target_os = "android"))]
    asone_core::batch::write_atomic(target, jpeg)
}

/// Desktop/Android: renderowanie na GPU w wątku eksportu, kompresja równolegle w puli `rayon`.
#[cfg(not(target_arch = "wasm32"))]
pub fn start_export(
    shared: std::sync::Arc<crate::renderer::GpuShared>,
    items: Vec<ExportItem>,
    settings: ExportSettings,
    sources: Vec<std::path::PathBuf>,
    ctx: egui::Context,
) -> Receiver<ExportMsg> {
    use asone_core::batch::check_target;

    let (tx, rx) = crossbeam_channel::unbounded();
    crate::platform::spawn_detached("export", move || {
        let started = crate::platform::Instant::now();
        let send = |msg| {
            let _ = tx.send(msg);
            ctx.request_repaint();
        };
        // Android: zapis przez powłokę Kotlin do Galerii (MediaStore), bez folderu na dysku.
        if !cfg!(target_os = "android")
            && let Err(e) = std::fs::create_dir_all(&settings.out_dir)
        {
            for item in &items {
                send(ExportMsg::Failed {
                    file_name: item.file_name.clone(),
                    error: format!("Nie można utworzyć folderu: {e}"),
                });
            }
            send(ExportMsg::Finished { elapsed: started.elapsed() });
            return;
        }

        let pool = crate::platform::Tasks::new("encode", 4);
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 4);
        // Ogranicza liczbę pełnych bitmap w pamięci naraz.
        let (slot_tx, slot_rx) = crossbeam_channel::bounded::<()>(workers);
        let (done_tx, done_rx) = crossbeam_channel::unbounded::<()>();
        let mut spawned = 0usize;

        for item in items {
            let target = settings.out_dir.join(&item.file_name);
            if !cfg!(target_os = "android")
                && let Err(error) = check_target(&target, &sources)
            {
                send(ExportMsg::Failed { file_name: item.file_name, error });
                continue;
            }
            let _ = slot_tx.send(());
            // Renderowanie na GPU odbywa się sekwencyjnie w tym wątku…
            match crate::renderer::render_offscreen(&shared, &item.gpu, &item.edits) {
                Ok((size, rgb)) => {
                    // …a skalowanie + kompresja równolegle w puli rayon.
                    let (tx, ctx, slot_rx, done_tx) = (tx.clone(), ctx.clone(), slot_rx.clone(), done_tx.clone());
                    let settings = settings.clone();
                    spawned += 1;
                    pool.spawn(move || {
                        let msg = match finish_image(size, rgb, &item.edits, item.overlay.as_deref(), &settings)
                            .and_then(|f| store(&target, &f.jpeg).map(|()| f))
                        {
                            Ok(f) => saved(item.file_name, f),
                            Err(error) => ExportMsg::Failed { file_name: item.file_name, error },
                        };
                        let _ = slot_rx.recv();
                        let _ = tx.send(msg);
                        ctx.request_repaint();
                        let _ = done_tx.send(());
                    });
                }
                Err(error) => {
                    let _ = slot_rx.recv();
                    send(ExportMsg::Failed { file_name: item.file_name, error });
                }
            }
        }
        for _ in 0..spawned {
            let _ = done_rx.recv();
        }
        send(ExportMsg::Finished { elapsed: started.elapsed() });
    });
    rx
}

/// Przeglądarka: każde zdjęcie to osobne zadanie między klatkami (UI pokazuje postęp).
#[cfg(target_arch = "wasm32")]
pub fn start_export(items: Vec<ExportItem>, settings: ExportSettings, ctx: egui::Context) -> Receiver<ExportMsg> {
    use crate::platform::{Instant, Tasks};

    let (tx, rx) = crossbeam_channel::unbounded();
    let started = Instant::now();
    for item in items {
        let (tx, ctx, settings) = (tx.clone(), ctx.clone(), settings.clone());
        Tasks.spawn(move || {
            let result = crate::loader::decode_for_gpu(&item.source, item.max_texture_dim).and_then(|(img, _)| {
                let (size, rgb) = asone_core::compose::compose_rgb(&img, Some(&item.mask), &item.edits);
                let f = finish_image(size, rgb, &item.edits, item.overlay.as_deref(), &settings)?;
                crate::platform::download(&item.file_name, &f.jpeg, "image/jpeg")?;
                Ok(f)
            });
            let msg = match result {
                Ok(f) => saved(item.file_name, f),
                Err(error) => ExportMsg::Failed { file_name: item.file_name, error },
            };
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }
    Tasks.spawn(move || {
        let _ = tx.send(ExportMsg::Finished { elapsed: started.elapsed() });
        ctx.request_repaint();
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Warstwa (smartfon) trafia do zapisanego JPEG w swoim miejscu; bez obrazu warstwy – samo tło.
    #[test]
    fn warstwa_w_zapisanym_pliku() {
        let size = [600, 500];
        let rgb = vec![255u8; 600 * 500 * 3];
        let edits = EditParams { margins: [0, 0, 0, 100], overlay: Some([20.0, 100.0, 60.0, 200.0]), ..Default::default() };
        let phone = image::RgbaImage::from_pixel(30, 100, image::Rgba([0, 0, 200, 255]));
        let settings = ExportSettings {
            quality: 95,
            max_long_side: None,
            out_dir: std::path::PathBuf::new(),
            limits: ExportLimits::default(),
        };
        let f = finish_image(size, rgb.clone(), &edits, Some(&phone), &settings).unwrap();
        let img = image::load_from_memory(&f.jpeg).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (600, 500));
        let p = img.get_pixel(50, 200).0;
        assert!(p[2] > 150 && p[0] < 60, "smartfon: {p:?}");
        assert!(img.get_pixel(50, 50).0.iter().all(|&c| c > 240), "tło nad smartfonem");
        let f = finish_image(size, rgb, &edits, None, &settings).unwrap();
        let img = image::load_from_memory(&f.jpeg).unwrap().to_rgb8();
        assert!(img.get_pixel(50, 200).0.iter().all(|&c| c > 240));
    }
}
