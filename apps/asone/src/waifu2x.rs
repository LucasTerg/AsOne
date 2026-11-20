//! Powiększanie waifu2x (swin_unet z projektu nunif autora waifu2x) lokalnie przez ONNX Runtime.
//!
//! Modele ONNX (ok. 20 MB każdy) pobierane przy pierwszym użyciu wariantu z unlimited.waifu2x.net.
//! Wejście `x`: RGB 0..1 (NCHW), kafelek T×T; wyjście `y`: (T·skala − 2·offset)², czyli wnętrze
//! kafelka bez pasa `offset/skala` px z każdej strony. Obraz dopełniamy z brzegów (odbicie dla
//! zdjęć, powielenie dla grafiki), tniemy na kafelki z zakładką i łączymy je płynnie (bez szwów).

use std::path::{Path, PathBuf};

use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Bok kafelka wejścia (dla swin_unet: (T − 16) dzielne przez 12 i 16).
const TILE: u32 = 256;
/// Zakładka sąsiednich kafelków (px wejścia) – płynne przejście zamiast szwu.
const BLEND: u32 = 16;
/// Największy dłuższy bok zdjęcia do powiększenia (waifu2x jest dla małych zdjęć; większe trwałyby minuty).
pub const MAX_INPUT_SIDE: u32 = 2000;

/// Rodzaj obrazu – wybiera model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Style {
    /// Zdjęcia (model „photo”).
    #[default]
    Photo,
    /// Grafika, rendery, czyste tła (model „art”).
    Art,
}

impl Style {
    pub const ALL: [Style; 2] = [Style::Photo, Style::Art];

    pub fn label(self) -> &'static str {
        match self {
            Style::Photo => "Zdjęcie",
            Style::Art => "Grafika / render",
        }
    }

    fn dir(self) -> &'static str {
        match self {
            Style::Photo => "photo",
            Style::Art => "art",
        }
    }
}

/// Poziomy odszumiania do wyboru w interfejsie.
pub const NOISE_LEVELS: [Option<u8>; 5] = [None, Some(0), Some(1), Some(2), Some(3)];

pub fn noise_label(noise: Option<u8>) -> &'static str {
    match noise {
        None => "bez odszumiania",
        Some(0) => "bardzo słabe (0)",
        Some(1) => "słabe (1)",
        Some(2) => "średnie (2)",
        _ => "mocne (3)",
    }
}

/// Wariant modelu: rodzaj obrazu, odszumianie (`None` – bez), skala 2× albo 4×.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    pub style: Style,
    pub noise: Option<u8>,
    pub scale: u32,
}

impl Variant {
    fn method(self) -> String {
        match self.noise {
            Some(n) => format!("noise{}_scale{}x", n.min(3), self.scale),
            None => format!("scale{}x", self.scale),
        }
    }

    pub fn url(self) -> String {
        format!("https://unlimited.waifu2x.net/models/swin_unet/{}/{}.onnx", self.style.dir(), self.method())
    }

    pub fn file_name(self) -> String {
        format!("waifu2x-swin_unet-{}-{}.onnx", self.style.dir(), self.method())
    }

    /// Pas na brzegu (px wyjścia), którego model nie oddaje.
    fn offset(self) -> u32 {
        8 * self.scale
    }
}

/// Katalog modeli waifu2x w danych AsOne.
pub fn models_dir() -> PathBuf {
    crate::settings::default_models_dir().join("waifu2x")
}

/// Powiększony obraz (rozmiar × skala); alfa skalowana zwykłym filtrem.
pub fn upscale(image: &RgbaImage, variant: Variant) -> Result<RgbaImage, String> {
    let (w, h) = image.dimensions();
    if w.max(h) > MAX_INPUT_SIDE {
        return Err(format!("waifu2x jest dla mniejszych zdjęć – dłuższy bok najwyżej {MAX_INPUT_SIDE} px"));
    }
    let model = ensure_model(variant)?;
    let s = variant.scale;
    let pad = variant.offset() / s;
    let step = TILE - 2 * pad - BLEND;
    let (nx, ny) = (w.div_ceil(step).max(1), h.div_ceil(step).max(1));
    // Piksele spoza zdjęcia: odbicie (zdjęcia) albo powielenie brzegu (grafika).
    let reflect = variant.style == Style::Photo;
    let src = |x: i64, y: i64| -> [f32; 3] {
        let fold = |v: i64, n: i64| -> u32 {
            if reflect && n > 1 {
                let period = 2 * (n - 1);
                let m = v.rem_euclid(period);
                (if m >= n { period - m } else { m }) as u32
            } else {
                v.clamp(0, n - 1) as u32
            }
        };
        let p = image.get_pixel(fold(x, w as i64), fold(y, h as i64));
        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]
    };

    let (ow, oh) = (w * s, h * s);
    let mut acc = vec![0f32; (ow * oh * 3) as usize];
    let mut weight = vec![0f32; (ow * oh) as usize];
    let out_tile = TILE * s - 2 * variant.offset();
    let ramp = (BLEND * s) as f32;
    let mut session = Session::open(&model)?;
    for ty in 0..ny {
        for tx in 0..nx {
            // Kafelek wejścia: od (tx·step − pad, ty·step − pad), T×T.
            let (x0, y0) = ((tx * step) as i64 - pad as i64, (ty * step) as i64 - pad as i64);
            let plane = (TILE * TILE) as usize;
            let mut input = vec![0f32; 3 * plane];
            for y in 0..TILE {
                for x in 0..TILE {
                    let p = src(x0 + x as i64, y0 + y as i64);
                    let i = (y * TILE + x) as usize;
                    input[i] = p[0];
                    input[plane + i] = p[1];
                    input[2 * plane + i] = p[2];
                }
            }
            let y_out = session.run(input, TILE)?;
            let oplane = (out_tile * out_tile) as usize;
            if y_out.len() != 3 * oplane {
                return Err(format!("waifu2x: nieoczekiwany rozmiar wyniku ({} wartości)", y_out.len()));
            }
            // Wynik odpowiada wejściu od (tx·step, ty·step) – w skali wyjścia.
            let (bx, by) = (tx * step * s, ty * step * s);
            for y in 0..out_tile {
                let gy = by + y;
                if gy >= oh {
                    break;
                }
                // Waga: narasta na zakładce z sąsiednim kafelkiem (brzegi obrazu – pełna).
                let wy = edge_weight(y, out_tile, ramp, ty > 0, ty + 1 < ny);
                for x in 0..out_tile {
                    let gx = bx + x;
                    if gx >= ow {
                        break;
                    }
                    let wgt = wy * edge_weight(x, out_tile, ramp, tx > 0, tx + 1 < nx);
                    let o = (y * out_tile + x) as usize;
                    let g = (gy * ow + gx) as usize;
                    for c in 0..3 {
                        acc[g * 3 + c] += y_out[c * oplane + o].clamp(0.0, 1.0) * wgt;
                    }
                    weight[g] += wgt;
                }
            }
        }
    }

    // Alfa: zwykłe skalowanie (zdjęcia produktów rzadko mają przezroczystość w samych pikselach).
    let alpha = asone_core::imageio::resize_rgba(image, [ow, oh], asone_core::imageio::lanczos3())?;
    let mut out = RgbaImage::new(ow, oh);
    for (i, p) in out.pixels_mut().enumerate() {
        let wgt = weight[i].max(1e-6);
        for c in 0..3 {
            p[c] = (acc[i * 3 + c] / wgt * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
        p[3] = alpha.as_raw()[i * 4 + 3];
    }
    Ok(out)
}

/// Waga piksela kafelka: 1 w środku, liniowo w dół na zakładce z sąsiadem (`before`/`after`).
fn edge_weight(i: u32, len: u32, ramp: f32, before: bool, after: bool) -> f32 {
    let mut w = 1.0f32;
    if before {
        w = w.min((i as f32 + 0.5) / ramp);
    }
    if after {
        w = w.min((len as f32 - i as f32 - 0.5) / ramp);
    }
    w.clamp(1e-3, 1.0)
}

/// Plik modelu wariantu – pobierany przy pierwszym użyciu (ok. 20 MB).
fn ensure_model(variant: Variant) -> Result<PathBuf, String> {
    let path = models_dir().join(variant.file_name());
    if path.is_file() {
        return Ok(path);
    }
    download(&variant.url(), &path)?;
    Ok(path)
}

#[cfg(not(target_arch = "wasm32"))]
fn download(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut resp = ureq::get(url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        .build()
        .call()
        .map_err(|e| format!("waifu2x: pobieranie modelu ({e})"))?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(200 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("waifu2x: pobieranie modelu ({e})"))?;
    let part = dest.with_extension("onnx.part");
    std::fs::write(&part, &bytes).map_err(|e| format!("Zapis: {e}"))?;
    std::fs::rename(&part, dest).map_err(|e| format!("Zapis: {e}"))
}

#[cfg(target_arch = "wasm32")]
fn download(_url: &str, _dest: &Path) -> Result<(), String> {
    Err("waifu2x działa lokalnie na Linux, Windows i macOS".into())
}

/// Sesja ONNX na czas jednego powiększenia.
struct Session {
    #[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    inner: ort::session::Session,
}

impl Session {
    #[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    fn open(model: &Path) -> Result<Self, String> {
        let err = |e: ort::Error| format!("waifu2x: {e}");
        let e2 = |e: ort::Error<ort::session::builder::SessionBuilder>| format!("waifu2x: {e}");
        let env = ort::init().with_name("asone").build().map_err(err)?;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let inner = ort::session::Session::builder(&env)
            .map_err(err)?
            .with_intra_threads(threads)
            .map_err(e2)?
            .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
            .map_err(e2)?
            .with_memory_pattern(false)
            .map_err(e2)?
            .commit_from_file(model)
            .map_err(err)?;
        Ok(Self { inner })
    }

    #[cfg(not(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos"))))]
    fn open(_model: &Path) -> Result<Self, String> {
        Err("waifu2x działa lokalnie na Linux, Windows i macOS".into())
    }

    #[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    fn run(&mut self, input: Vec<f32>, tile: u32) -> Result<Vec<f32>, String> {
        let err = |e: ort::Error| format!("waifu2x: {e}");
        let n = tile as usize;
        let x = ort::value::Tensor::from_array(([1usize, 3, n, n], input.into_boxed_slice())).map_err(err)?;
        let name = self.inner.outputs().first().map(|o| o.name().to_string()).ok_or("waifu2x: model bez wyjść")?;
        let outputs = self.inner.run(ort::inputs![x]).map_err(err)?;
        let (_, raw) = outputs[name.as_str()].try_extract_tensor::<f32>().map_err(err)?;
        Ok(raw.to_vec())
    }

    #[cfg(not(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos"))))]
    fn run(&mut self, _input: Vec<f32>, _tile: u32) -> Result<Vec<f32>, String> {
        Err("waifu2x działa lokalnie na Linux, Windows i macOS".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nazwy_i_adresy_modeli() {
        let v = Variant { style: Style::Photo, noise: Some(1), scale: 2 };
        assert_eq!(v.file_name(), "waifu2x-swin_unet-photo-noise1_scale2x.onnx");
        assert_eq!(v.url(), "https://unlimited.waifu2x.net/models/swin_unet/photo/noise1_scale2x.onnx");
        let v = Variant { style: Style::Art, noise: None, scale: 4 };
        assert_eq!(v.url(), "https://unlimited.waifu2x.net/models/swin_unet/art/scale4x.onnx");
        assert_eq!(v.offset(), 32);
        // Kafelek spełnia warunek swin_unet.
        const { assert!((TILE - 16).is_multiple_of(12) && (TILE - 16).is_multiple_of(16)) };
    }

    #[test]
    fn wagi_zakladki() {
        assert_eq!(edge_weight(100, 480, 32.0, false, false), 1.0);
        assert!(edge_weight(0, 480, 32.0, true, false) < 0.05);
        assert!(edge_weight(479, 480, 32.0, false, true) < 0.05);
    }

    /// Prawdziwy model (pobiera ok. 20 MB): `cargo test -p asone waifu2x_na_zywo -- --ignored --nocapture`.
    #[test]
    #[ignore = "pobiera model waifu2x i liczy na CPU"]
    fn waifu2x_na_zywo() {
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sample/produkt-1.jpg");
        let img = asone_core::imageio::decode_path(&sample).unwrap();
        let small = asone_core::imageio::resize_rgba(&img, [400, 300], asone_core::imageio::lanczos3()).unwrap();
        let t = std::time::Instant::now();
        let out = upscale(&small, Variant { style: Style::Photo, noise: Some(1), scale: 2 }).unwrap();
        eprintln!("waifu2x: {:?}, {:?} → {:?}", t.elapsed(), small.dimensions(), out.dimensions());
        if let Ok(p) = std::env::var("ASONE_W2X_OUT") {
            out.save(p).unwrap();
        }
        assert_eq!(out.dimensions(), (800, 600));
        // Kolor tła i kubka zachowany (bez przesunięcia kafelków).
        let (a, b) = (small.get_pixel(200, 150), out.get_pixel(400, 300));
        for c in 0..3 {
            assert!((a[c] as i32 - b[c] as i32).abs() < 25, "{a:?} vs {b:?}");
        }
    }
}
