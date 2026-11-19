//! Kompresja pod internet: zawsze Progressive JPEG z optymalizacją, bez żadnych metadanych.
//!
//! Domyślnie MozJPEG (jak w Squoosh). Na wasm albo bez funkcji `mozjpeg` – `jpeg-encoder`
//! (czysty Rust). W obu przypadkach plik zawiera tylko nagłówek JFIF: zero EXIF/GPS/ICC/XMP.

#[cfg(all(feature = "mozjpeg", not(target_arch = "wasm32")))]
pub const ENCODER_NAME: &str = "MozJPEG";
#[cfg(not(all(feature = "mozjpeg", not(target_arch = "wasm32"))))]
pub const ENCODER_NAME: &str = "jpeg-encoder";

/// Koduje ciasno upakowane piksele RGB8. `quality` 1..=100.
pub fn encode_jpeg(rgb: &[u8], size: [u32; 2], quality: u8) -> Result<Vec<u8>, String> {
    let expected = size[0] as usize * size[1] as usize * 3;
    if rgb.len() != expected || size[0] == 0 || size[1] == 0 {
        return Err(format!("JPEG: zły rozmiar bufora ({} zamiast {expected})", rgb.len()));
    }
    let quality = quality.clamp(1, 100);
    #[cfg(all(feature = "mozjpeg", not(target_arch = "wasm32")))]
    return encode_mozjpeg(rgb, size, quality);
    #[cfg(not(all(feature = "mozjpeg", not(target_arch = "wasm32"))))]
    return encode_pure(rgb, size, quality);
}

#[cfg(all(feature = "mozjpeg", not(target_arch = "wasm32")))]
fn encode_mozjpeg(rgb: &[u8], size: [u32; 2], quality: u8) -> Result<Vec<u8>, String> {
    // mozjpeg zgłasza błędy biblioteki C przez panikę – łapiemy ją.
    std::panic::catch_unwind(|| -> std::io::Result<Vec<u8>> {
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        comp.set_size(size[0] as usize, size[1] as usize);
        comp.set_quality(quality as f32);
        comp.set_progressive_mode();
        comp.set_optimize_scans(true);
        comp.set_optimize_coding(true);
        let mut started = comp.start_compress(Vec::new())?;
        started.write_scanlines(rgb)?;
        started.finish()
    })
    .map_err(|_| "MozJPEG: błąd kodowania".to_string())?
    .map_err(|e| format!("MozJPEG: {e}"))
}

#[cfg_attr(all(feature = "mozjpeg", not(target_arch = "wasm32")), allow(dead_code))]
fn encode_pure(rgb: &[u8], size: [u32; 2], quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (
        u16::try_from(size[0]).map_err(|_| "JPEG: maks. 65535 px na bok".to_string())?,
        u16::try_from(size[1]).map_err(|_| "JPEG: maks. 65535 px na bok".to_string())?,
    );
    let mut out = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality);
    enc.set_progressive(true);
    enc.set_optimized_huffman_tables(true);
    enc.encode(rgb, w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| format!("JPEG: {e}"))?;
    Ok(out)
}

/// JPEG baseline (bez trybu progresywnego) – dla stron PDF, gdzie część czytników
/// gorzej znosi progresywne strumienie DCT.
pub fn encode_jpeg_baseline(rgb: &[u8], size: [u32; 2], quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (
        u16::try_from(size[0]).map_err(|_| "JPEG: maks. 65535 px na bok".to_string())?,
        u16::try_from(size[1]).map_err(|_| "JPEG: maks. 65535 px na bok".to_string())?,
    );
    let mut out = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100));
    enc.set_optimized_huffman_tables(true);
    enc.encode(rgb, w, h, jpeg_encoder::ColorType::Rgb).map_err(|e| format!("JPEG: {e}"))?;
    Ok(out)
}

/// Markery JPEG (bajt po 0xFF) aż do początku danych pierwszego skanu.
pub fn jpeg_markers(jpeg: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 2; // po SOI
    while i + 4 <= jpeg.len() && jpeg[i] == 0xFF {
        let m = jpeg[i + 1];
        out.push(m);
        if m == 0xDA {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        i += 2 + len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ([u32; 2], Vec<u8>) {
        let (w, h) = (64u32, 48u32);
        let rgb = (0..w * h).flat_map(|i| [(i % 251) as u8, (i % 7 * 30) as u8, 200]).collect();
        ([w, h], rgb)
    }

    fn check(jpeg: &[u8], size: [u32; 2]) {
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let m = jpeg_markers(jpeg);
        assert!(m.contains(&0xC2), "brak SOF2 (progressive): {m:x?}");
        assert!(!m.contains(&0xE1), "znaleziono APP1 (EXIF/XMP): {m:x?}");
        assert!(!m.contains(&0xE2), "znaleziono APP2 (ICC): {m:x?}");
        let decoded = image::load_from_memory(jpeg).unwrap();
        assert_eq!([decoded.width(), decoded.height()], size);
    }

    #[test]
    fn jpeg_progresywny_bez_metadanych() {
        let (size, rgb) = sample();
        check(&encode_jpeg(&rgb, size, 90).unwrap(), size);
    }

    #[test]
    fn czysty_rust_tez_progresywny() {
        let (size, rgb) = sample();
        check(&encode_pure(&rgb, size, 90).unwrap(), size);
    }

    #[test]
    fn zly_bufor() {
        assert!(encode_jpeg(&[0; 5], [2, 2], 90).is_err());
    }
}
