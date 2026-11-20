//! Retusz LaMa (Big-LaMa, ONNX, Apache-2.0): usuwa kurz, rysy, napisy i metki w zaznaczonym
//! prostokącie – lokalnie, bez wysyłania zdjęcia i bez opłat.
//!
//! Model ma stałe wejście 512×512: wokół zaznaczenia bierzemy fragment z zapasem (kontekst do
//! dorysowania), skalujemy go do 512×512 razem z maską zaznaczenia, a wynik wraca w zaznaczone
//! miejsce z miękkim brzegiem. Reszta zdjęcia zostaje bez zmian.

use std::path::Path;

use asone_core::edits::PxRect;
use asone_core::imageio::{lanczos3, resize_rgba};
use image::RgbaImage;

/// Bok wejścia modelu.
pub const SIZE: u32 = 512;

/// Fragment zdjęcia z kontekstem wokół zaznaczenia: zbliżony do kwadratu, w granicach zdjęcia.
pub fn context_rect(sel: PxRect, size: [u32; 2]) -> PxRect {
    let (w, h) = (sel.width().max(1), sel.height().max(1));
    let side = ((w.max(h) as f32 * 2.5) as u32 + 48).max(w.max(h) + 32);
    let (cx, cy) = ((sel.x0 + sel.x1) / 2, (sel.y0 + sel.y1) / 2);
    let axis = |c: u32, len: u32| -> (u32, u32) {
        let side = side.min(len);
        let start = c.saturating_sub(side / 2).min(len - side);
        (start, start + side)
    };
    let (x0, x1) = axis(cx, size[0]);
    let (y0, y1) = axis(cy, size[1]);
    PxRect { x0, y0, x1, y1 }
}

/// Zdjęcie z zaznaczonym prostokątem dorysowanym przez LaMa (rozmiar i kanał alfa bez zmian).
pub fn inpaint(model: &Path, image: &RgbaImage, sel: PxRect) -> Result<RgbaImage, String> {
    if !model.is_file() {
        return Err(format!(
            "Brak pliku modelu LaMa: {} – pobierz go w Ustawienia › Preferencje › Retusz (LaMa)",
            model.display()
        ));
    }
    let size = [image.width(), image.height()];
    let sel = PxRect { x0: sel.x0.min(size[0]), y0: sel.y0.min(size[1]), x1: sel.x1.min(size[0]), y1: sel.y1.min(size[1]) };
    if sel.is_empty() {
        return Err("Zaznacz obszar do usunięcia narzędziem M (prostokąt)".into());
    }
    let crop = context_rect(sel, size);
    let region = image::imageops::crop_imm(image, crop.x0, crop.y0, crop.width(), crop.height()).to_image();
    let small = resize_rgba(&region, [SIZE, SIZE], lanczos3())?;

    // Wejście: obraz RGB 0..1 (NCHW) i maska 1 = do dorysowania (z zapasem 3 px w skali 512).
    let plane = (SIZE * SIZE) as usize;
    let mut input = vec![0f32; 3 * plane];
    for (i, p) in small.pixels().enumerate() {
        for c in 0..3 {
            input[c * plane + i] = p[c] as f32 / 255.0;
        }
    }
    let (sx, sy) = (SIZE as f32 / crop.width() as f32, SIZE as f32 / crop.height() as f32);
    let mx0 = (((sel.x0 - crop.x0) as f32 * sx).floor() as i64 - 3).max(0) as u32;
    let my0 = (((sel.y0 - crop.y0) as f32 * sy).floor() as i64 - 3).max(0) as u32;
    let mx1 = ((((sel.x1 - crop.x0) as f32 * sx).ceil() as u32) + 3).min(SIZE);
    let my1 = ((((sel.y1 - crop.y0) as f32 * sy).ceil() as u32) + 3).min(SIZE);
    let mut mask = vec![0f32; plane];
    for y in my0..my1 {
        for x in mx0..mx1 {
            mask[(y * SIZE + x) as usize] = 1.0;
        }
    }

    let output = run_model(model, input, mask)?;
    let peak = output.iter().fold(0f32, |m, &v| m.max(v));
    let scale = if peak <= 1.5 { 255.0 } else { 1.0 };
    let mut filled = RgbaImage::new(SIZE, SIZE);
    for (i, p) in filled.pixels_mut().enumerate() {
        for c in 0..3 {
            p[c] = (output[c * plane + i] * scale).round().clamp(0.0, 255.0) as u8;
        }
        p[3] = 255;
    }
    let filled = resize_rgba(&filled, [crop.width(), crop.height()], lanczos3())?;

    // Wklejenie: w zaznaczeniu w całości, w pasie `feather` px wokół – płynne przejście.
    let feather = (sel.width().min(sel.height()) as f32 * 0.08).clamp(2.0, 12.0);
    let mut out = image.clone();
    let f = feather.ceil() as u32;
    let (ox0, oy0) = (sel.x0.saturating_sub(f).max(crop.x0), sel.y0.saturating_sub(f).max(crop.y0));
    let (ox1, oy1) = ((sel.x1 + f).min(crop.x1), (sel.y1 + f).min(crop.y1));
    for y in oy0..oy1 {
        for x in ox0..ox1 {
            let dx = (sel.x0 as f32 - x as f32 - 0.5).max(x as f32 + 0.5 - sel.x1 as f32).max(0.0);
            let dy = (sel.y0 as f32 - y as f32 - 0.5).max(y as f32 + 0.5 - sel.y1 as f32).max(0.0);
            let weight = (1.0 - dx.max(dy) / feather).clamp(0.0, 1.0);
            if weight <= 0.0 {
                continue;
            }
            let src = filled.get_pixel(x - crop.x0, y - crop.y0);
            let dst = out.get_pixel_mut(x, y);
            for c in 0..3 {
                dst[c] = (src[c] as f32 * weight + dst[c] as f32 * (1.0 - weight)).round() as u8;
            }
        }
    }
    Ok(out)
}

/// Jeden przebieg modelu (sesja tworzona na czas zadania – retusz jest okazjonalny).
#[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn run_model(model: &Path, image: Vec<f32>, mask: Vec<f32>) -> Result<Vec<f32>, String> {
    let err = |e: ort::Error| format!("LaMa: {e}");
    let e2 = |e: ort::Error<ort::session::builder::SessionBuilder>| format!("LaMa: {e}");
    let env = ort::init().with_name("asone").build().map_err(err)?;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut session = ort::session::Session::builder(&env)
        .map_err(err)?
        .with_intra_threads(threads)
        .map_err(e2)?
        .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
        .map_err(e2)?
        .with_memory_pattern(false)
        .map_err(e2)?
        .commit_from_file(model)
        .map_err(err)?;
    // Kolejność wejść z modelu: „image” i „mask” (gdyby maska była pierwsza – zamiana).
    let mask_first = session.inputs().first().is_some_and(|i| i.name().to_lowercase().contains("mask"));
    let n = SIZE as usize;
    let image = ort::value::Tensor::from_array(([1usize, 3, n, n], image.into_boxed_slice())).map_err(err)?;
    let mask = ort::value::Tensor::from_array(([1usize, 1, n, n], mask.into_boxed_slice())).map_err(err)?;
    let output_name = session.outputs().first().map(|o| o.name().to_string()).ok_or("LaMa: model bez wyjść")?;
    let outputs = if mask_first {
        session.run(ort::inputs![mask, image]).map_err(err)?
    } else {
        session.run(ort::inputs![image, mask]).map_err(err)?
    };
    let (_, raw) = outputs[output_name.as_str()].try_extract_tensor::<f32>().map_err(err)?;
    if raw.len() != 3 * n * n {
        return Err(format!("LaMa: nieoczekiwany rozmiar wyniku ({} wartości)", raw.len()));
    }
    Ok(raw.to_vec())
}

#[cfg(not(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos"))))]
fn run_model(_model: &Path, _image: Vec<f32>, _mask: Vec<f32>) -> Result<Vec<f32>, String> {
    Err("Retusz LaMa działa lokalnie w wersji na Linux, Windows i macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kontekst_w_granicach_zdjecia() {
        let r = context_rect(PxRect { x0: 10, y0: 10, x1: 30, y1: 20 }, [1000, 800]);
        assert!(r.x0 <= 10 && r.y0 <= 10 && r.x1 >= 30 && r.y1 >= 20);
        assert!(r.x1 <= 1000 && r.y1 <= 800);
        // Zdjęcie mniejsze niż kontekst (10 px × 2,5 + 48 = 73 px) – cały bok.
        let r = context_rect(PxRect { x0: 20, y0: 20, x1: 30, y1: 30 }, [60, 50]);
        assert_eq!((r.x0, r.x1, r.y0, r.y1), (0, 60, 0, 50));
    }

    /// Prawdziwy model: czarny kwadrat na jasnym tle ma zniknąć.
    /// `ASONE_LAMA=…/lama-fp32.onnx cargo test -p asone lama_na_zywo -- --ignored --nocapture`
    #[test]
    #[ignore = "wymaga pliku modelu LaMa (~208 MB)"]
    fn lama_na_zywo() {
        let model = std::path::PathBuf::from(std::env::var("ASONE_LAMA").expect("ASONE_LAMA"));
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sample/produkt-1.jpg");
        let mut img = asone_core::imageio::decode_path(&sample).unwrap();
        let sel = PxRect { x0: 1150, y0: 200, x1: 1300, y1: 300 };
        let before = *img.get_pixel(1225, 250);
        for y in sel.y0..sel.y1 {
            for x in sel.x0..sel.x1 {
                img.put_pixel(x, y, image::Rgba([20, 20, 20, 255]));
            }
        }
        let t = std::time::Instant::now();
        let out = inpaint(&model, &img, sel).unwrap();
        eprintln!("LaMa: {:?}", t.elapsed());
        if let Ok(p) = std::env::var("ASONE_LAMA_OUT") {
            out.save(p).unwrap();
        }
        let after = out.get_pixel(1225, 250);
        eprintln!("piksel przed {:?}, z czarnym 20, po {:?}", before, after);
        assert!(after[0] > 150, "kwadrat nie został zamalowany tłem: {after:?}");
        assert_eq!(out.get_pixel(5, 5), img.get_pixel(5, 5), "poza zaznaczeniem bez zmian");
    }
}
