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
    /// „Spłaszcz” / edycja AI: nowy obraz jako zawartość zakładki (`ai` – wynik z Replicate).
    Flattened {
        id: DocId,
        gpu: DocGpu,
        mask: Vec<u8>,
        ai_proxy: Arc<RgbaImage>,
        content: Arc<RgbaImage>,
        thumb: egui::ColorImage,
        ai: bool,
    },
    FlattenFailed {
        id: DocId,
        error: String,
    },
    /// Retusz LaMa: nowe piksele zdjęcia (ten sam rozmiar – kadr, marginesy i maska zostają).
    Repainted {
        id: DocId,
        gpu: DocGpu,
        ai_proxy: Arc<RgbaImage>,
        content: Arc<RgbaImage>,
        thumb: egui::ColorImage,
        seconds: f32,
    },
    /// waifu2x: powiększone zdjęcie i maska (rozmiar × `scale`).
    Upscaled {
        id: DocId,
        gpu: DocGpu,
        ai_proxy: Arc<RgbaImage>,
        content: Arc<RgbaImage>,
        thumb: egui::ColorImage,
        mask: Vec<u8>,
        scale: u32,
        seconds: f32,
    },
    /// Ekran startowy: folder z „Ostatnich” – czy istnieje, ile ma zdjęć, miniatura pierwszego.
    FolderPreview {
        dir: String,
        exists: bool,
        count: usize,
        image: Option<egui::ColorImage>,
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

    /// „Spłaszcz”: składa obraz wynikowy na CPU (ta sama semantyka co eksport) i wgrywa go
    /// jako nową teksturę zakładki – potem kadr, marginesy i gumka działają na całości.
    pub fn request_flatten(
        &self,
        id: DocId,
        source: ImageSource,
        mask: Vec<u8>,
        edits: asone_core::edits::EditParams,
        overlay: Option<Arc<RgbaImage>>,
    ) {
        let (tx, ctx, gpu) = (self.tx.clone(), self.ctx.clone(), self.gpu.clone());
        self.tasks.spawn(move || {
            let result = (|| -> Result<LoadResult, String> {
                let gpu = gpu.ok_or("Brak GPU (wgpu)")?;
                let image = compose_output(&source, &mask, &edits, overlay.as_deref(), gpu.max_texture_dim)?;
                new_content(&gpu, id, image, false)
            })();
            let _ = tx.send(result.unwrap_or_else(|e| LoadResult::FlattenFailed { id, error: format!("Spłaszczanie: {e}") }));
            ctx.request_repaint();
        });
    }

    /// Edycja AI (Replicate): obraz wynikowy + polecenie → model → nowa zawartość zakładki.
    #[allow(clippy::too_many_arguments)]
    pub fn request_ai_edit(
        &self,
        id: DocId,
        source: ImageSource,
        mask: Vec<u8>,
        edits: asone_core::edits::EditParams,
        overlay: Option<Arc<RgbaImage>>,
        prompt: String,
        model: &'static str,
        key: String,
    ) {
        let (tx, ctx, gpu) = (self.tx.clone(), self.ctx.clone(), self.gpu.clone());
        self.tasks.spawn(move || {
            let result = (|| -> Result<LoadResult, String> {
                let gpu = gpu.ok_or("Brak GPU (wgpu)")?;
                let input = compose_output(&source, &mask, &edits, overlay.as_deref(), gpu.max_texture_dim)?;
                #[cfg(not(target_arch = "wasm32"))]
                let image = crate::replicate::edit_image(&input, &prompt, model, &key);
                #[cfg(target_arch = "wasm32")]
                let image: Result<RgbaImage, String> = {
                    let _ = (&input, &prompt, model, &key);
                    Err("Edycja przez Replicate nie działa w przeglądarce (Replicate blokuje takie zapytania)".to_string())
                };
                new_content(&gpu, id, image?, true)
            })();
            let _ = tx.send(result.unwrap_or_else(|error| LoadResult::FlattenFailed { id, error }));
            ctx.request_repaint();
        });
    }

    /// Retusz LaMa: zaznaczony prostokąt dorysowany z otoczenia; maska zakładki zostaje.
    pub fn request_inpaint(&self, id: DocId, source: ImageSource, mask: Vec<u8>, rect: asone_core::edits::PxRect, model: std::path::PathBuf) {
        let (tx, ctx, gpu) = (self.tx.clone(), self.ctx.clone(), self.gpu.clone());
        self.tasks.spawn(move || {
            let started = crate::platform::Instant::now();
            let result = (|| -> Result<LoadResult, String> {
                let gpu = gpu.ok_or("Brak GPU (wgpu)")?;
                let (img, _) = decode_for_gpu(&source, gpu.max_texture_dim)?;
                let out = crate::lama::inpaint(&model, &img, rect)?;
                drop(img);
                let thumb = thumbnail(&out)?;
                let content = Arc::new(out);
                let full = full_from_image(&gpu, (*content).clone(), None, Some(mask))?;
                let seconds = started.elapsed().as_secs_f32();
                Ok(LoadResult::Repainted { id, gpu: full.gpu, ai_proxy: full.ai_proxy, content, thumb, seconds })
            })();
            let _ = tx.send(result.unwrap_or_else(|e| LoadResult::FlattenFailed { id, error: format!("Retusz LaMa: {e}") }));
            ctx.request_repaint();
        });
    }

    /// waifu2x: zdjęcie i maska powiększone (model pobierany przy pierwszym użyciu wariantu).
    pub fn request_upscale(
        &self,
        id: DocId,
        source: ImageSource,
        mask: Vec<u8>,
        variant: crate::waifu2x::Variant,
        progress: Arc<std::sync::atomic::AtomicU32>,
    ) {
        let (tx, ctx, gpu) = (self.tx.clone(), self.ctx.clone(), self.gpu.clone());
        self.tasks.spawn(move || {
            let started = crate::platform::Instant::now();
            let result = (|| -> Result<LoadResult, String> {
                let gpu = gpu.ok_or("Brak GPU (wgpu)")?;
                let (img, _) = decode_for_gpu(&source, gpu.max_texture_dim)?;
                let size = [img.width(), img.height()];
                let new = [size[0] * variant.scale, size[1] * variant.scale];
                if new[0].max(new[1]) > gpu.max_texture_dim {
                    return Err(format!("wynik {}×{} px przekracza limit karty graficznej ({} px)", new[0], new[1], gpu.max_texture_dim));
                }
                let out = crate::waifu2x::upscale(&img, variant, Some(&progress))?;
                drop(img);
                let mask = if mask.len() == size[0] as usize * size[1] as usize {
                    asone_core::imageio::resize_gray(&mask, size, new)?
                } else {
                    vec![255; new[0] as usize * new[1] as usize]
                };
                let thumb = thumbnail(&out)?;
                let content = Arc::new(out);
                let full = full_from_image(&gpu, (*content).clone(), None, Some(mask.clone()))?;
                let seconds = started.elapsed().as_secs_f32();
                Ok(LoadResult::Upscaled { id, gpu: full.gpu, ai_proxy: full.ai_proxy, content, thumb, mask, scale: variant.scale, seconds })
            })();
            let _ = tx.send(result.unwrap_or_else(|e| LoadResult::FlattenFailed { id, error: format!("waifu2x: {e}") }));
            ctx.request_repaint();
        });
    }

    /// Podgląd folderu z „Ostatnich” (liczba zdjęć i miniatura pierwszego).
    pub fn request_folder_preview(&self, dir: String) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        self.tasks.spawn(move || {
            let path = std::path::PathBuf::from(&dir);
            let exists = path.is_dir();
            let files = if exists { asone_core::imageio::expand_paths([path]) } else { Vec::new() };
            let image = files.first().and_then(|f| asone_core::imageio::decode_path(f).ok()).and_then(|img| thumbnail(&img).ok());
            let _ = tx.send(LoadResult::FolderPreview { dir, exists, count: files.len(), image });
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
    full_from_image(gpu, img, downscaled_from, None)
}

/// Obraz wynikowy (kadr, marginesy, maska na tle, warstwa) jako RGBA – na CPU, jak eksport.
fn compose_output(
    source: &ImageSource,
    mask: &[u8],
    edits: &asone_core::edits::EditParams,
    overlay: Option<&RgbaImage>,
    max_texture_dim: u32,
) -> Result<RgbaImage, String> {
    let (img, _) = decode_for_gpu(source, max_texture_dim)?;
    let (size, mut rgb) = asone_core::compose::compose_rgb(&img, Some(mask), edits);
    drop(img);
    if let (Some(layer), Some(rect)) = (overlay, edits.overlay) {
        asone_core::compose::draw_overlay(size, &mut rgb, layer, rect)?;
    }
    let rgba: Vec<u8> = rgb.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    RgbaImage::from_raw(size[0], size[1], rgba).ok_or_else(|| "Błąd bufora".to_string())
}

/// Nowa zawartość zakładki z gotowego obrazu (w limicie tekstury GPU).
fn new_content(gpu: &GpuShared, id: DocId, image: RgbaImage, ai: bool) -> Result<LoadResult, String> {
    let size = [image.width(), image.height()];
    let limited = fit_within(size, gpu.max_texture_dim);
    let image = if limited == size { image } else { resize_rgba(&image, limited, lanczos3())? };
    let thumb = thumbnail(&image)?;
    let content = Arc::new(image);
    let full = full_from_image(gpu, (*content).clone(), None, None)?;
    Ok(LoadResult::Flattened { id, gpu: full.gpu, mask: full.mask, ai_proxy: full.ai_proxy, content, thumb, ai })
}

/// Tekstura z mipmapami, maska (z kanału alfa) i kopia dla AI z gotowego obrazu.
fn full_from_image(
    gpu: &GpuShared,
    img: RgbaImage,
    downscaled_from: Option<[u32; 2]>,
    mask: Option<Vec<u8>>,
) -> Result<FullLoad, String> {
    let size = [img.width(), img.height()];

    // Maska: podana (retusz – bez zmian), z kanału alfa pliku (przezroczystość) albo 255.
    let (mask, has_alpha) = match mask.filter(|m| m.len() == size[0] as usize * size[1] as usize) {
        Some(m) => (m, false),
        None => {
            let alpha = alpha_mask(&img);
            let has_alpha = alpha.is_some();
            (alpha.unwrap_or_else(|| vec![255; size[0] as usize * size[1] as usize]), has_alpha)
        }
    };

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
