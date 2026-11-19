//! Wczytywanie zdjęć w tle: miniatury dla listy plików oraz pełne tekstury GPU dla zakładek.

use std::sync::Arc;

use asone_core::imageio::{alpha_mask, fit_within, lanczos3, resize_rgba};
use crossbeam_channel::{Receiver, Sender};
use fast_image_resize as fr;
use image::RgbaImage;

use crate::document::DocId;
use crate::platform::{ImageSource, Tasks};
use crate::renderer::{DocGpu, GpuShared, MipLevel};

const THUMB_SIZE: u32 = 256;
const AI_PROXY_SIZE: u32 = 1024;

// Wiadomości są rzadkie (raz na zdjęcie), więc rozmiar wariantu `Full` nie ma znaczenia.
#[allow(clippy::large_enum_variant)]
pub enum LoadResult {
    Thumbnail {
        id: DocId,
        dims: [u32; 2],
        image: egui::ColorImage,
    },
    Full {
        id: DocId,
        gpu: DocGpu,
        mask: Vec<u8>,
        /// Czy plik miał już przezroczystość (np. wycięty PNG) – wtedy maska jest z niej zbudowana.
        has_alpha: bool,
        downscaled_from: Option<[u32; 2]>,
        ai_proxy: Arc<RgbaImage>,
    },
    Failed {
        id: DocId,
        thumbnail: bool,
        error: String,
    },
}

pub struct Loader {
    tasks: Tasks,
    tx: Sender<LoadResult>,
    pub rx: Receiver<LoadResult>,
    ctx: egui::Context,
    gpu: Option<Arc<GpuShared>>,
}

impl Loader {
    pub fn new(ctx: egui::Context, gpu: Option<Arc<GpuShared>>) -> Self {
        // Ograniczamy równoległość: zdjęcie 24 Mpx to ~100 MB w RAM podczas dekodowania.
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tasks: Tasks::new("load", 4), tx, rx, ctx, gpu }
    }

    pub fn request_thumbnail(&self, id: DocId, source: ImageSource) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        self.tasks.spawn(move || {
            let result = match source.decode().and_then(|img| thumbnail(&img).map(|t| (img.dimensions(), t))) {
                Ok(((w, h), image)) => LoadResult::Thumbnail { id, dims: [w, h], image },
                Err(error) => LoadResult::Failed { id, thumbnail: true, error },
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    pub fn request_full(&self, id: DocId, source: ImageSource) {
        let (tx, ctx, gpu) = (self.tx.clone(), self.ctx.clone(), self.gpu.clone());
        self.tasks.spawn(move || {
            let result = match gpu {
                None => LoadResult::Failed { id, thumbnail: false, error: "Brak GPU (wgpu)".into() },
                Some(gpu) => match load_full(&gpu, &source) {
                    Ok(full) => LoadResult::Full {
                        id,
                        gpu: full.gpu,
                        mask: full.mask,
                        has_alpha: full.has_alpha,
                        downscaled_from: full.downscaled_from,
                        ai_proxy: full.ai_proxy,
                    },
                    Err(error) => LoadResult::Failed { id, thumbnail: false, error },
                },
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
}

fn thumbnail(img: &RgbaImage) -> Result<egui::ColorImage, String> {
    let (w, h) = img.dimensions();
    let size = fit_within([w, h], THUMB_SIZE);
    let small = resize_rgba(img, size, lanczos3())?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [size[0] as usize, size[1] as usize],
        small.as_raw(),
    ))
}

/// Zdjęcie gotowe do użycia przez GPU (ta sama funkcja służy eksportowi w przeglądarce,
/// dzięki czemu rozmiar i maska zgadzają się z teksturą).
pub fn decode_for_gpu(source: &ImageSource, max_texture_dim: u32) -> Result<(RgbaImage, Option<[u32; 2]>), String> {
    let img = source.decode()?;
    let original = [img.width(), img.height()];
    // Za duże dla GPU? Zmniejszamy do limitu tekstury (rzadkie: > 16k px, w przeglądarce > 4–8k).
    let limited = fit_within(original, max_texture_dim);
    if limited == original {
        Ok((img, None))
    } else {
        Ok((resize_rgba(&img, limited, lanczos3())?, Some(original)))
    }
}

struct FullLoad {
    gpu: DocGpu,
    mask: Vec<u8>,
    has_alpha: bool,
    downscaled_from: Option<[u32; 2]>,
    ai_proxy: Arc<RgbaImage>,
}

fn load_full(gpu: &GpuShared, source: &ImageSource) -> Result<FullLoad, String> {
    let (img, downscaled_from) = decode_for_gpu(source, gpu.max_texture_dim)?;
    let size = [img.width(), img.height()];

    // Maska: z kanału alfa pliku (jeśli jest przezroczystość), w przeciwnym razie 255.
    let alpha = alpha_mask(&img);
    let has_alpha = alpha.is_some();
    let mask = alpha.unwrap_or_else(|| vec![255; size[0] as usize * size[1] as usize]);

    let ai_proxy = Arc::new(resize_rgba(
        &img,
        fit_within(size, AI_PROXY_SIZE),
        fr::ResizeAlg::Convolution(fr::FilterType::Bilinear),
    )?);

    let levels = build_mips(img)?;
    let doc_gpu = DocGpu::new(gpu, &levels, &mask);
    Ok(FullLoad { gpu: doc_gpu, mask, has_alpha, downscaled_from, ai_proxy })
}

/// Łańcuch mipmap (każdy poziom o połowę mniejszy) – płynny podgląd przy oddaleniu.
fn build_mips(img: RgbaImage) -> Result<Vec<MipLevel>, String> {
    let mut levels = Vec::new();
    let (w, h) = img.dimensions();
    let mut current = img;
    let mut size = [w, h];
    loop {
        let next = [(size[0] / 2).max(1), (size[1] / 2).max(1)];
        let smaller = if size == [1, 1] {
            None
        } else {
            Some(resize_rgba(&current, next, fr::ResizeAlg::Convolution(fr::FilterType::Box))?)
        };
        levels.push(MipLevel { size, data: current.into_raw() });
        match smaller {
            Some(s) => {
                current = s;
                size = next;
            }
            None => break,
        }
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mipmapy_do_1x1() {
        let img = RgbaImage::from_pixel(5, 3, image::Rgba([10, 20, 30, 255]));
        let levels = build_mips(img).unwrap();
        let sizes: Vec<_> = levels.iter().map(|l| l.size).collect();
        assert_eq!(sizes, vec![[5, 3], [2, 1], [1, 1]]);
        assert_eq!(levels.len() as u32, crate::renderer::mip_count([5, 3]));
        assert!(levels.iter().all(|l| l.data.len() == (l.size[0] * l.size[1] * 4) as usize));
        assert_eq!(&levels[2].data, &[10, 20, 30, 255]);
    }
}
