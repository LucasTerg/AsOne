//! Wczytywanie obrazów (plik albo bajty) z orientacją EXIF i skalowanie `fast_image_resize`.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use fast_image_resize as fr;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};

pub const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "avif", "tif", "tiff", "bmp"];

/// Czy to kontener AVIF (`ftyp` z marką `avif`/`avis` – główną albo zgodną).
pub fn is_avif(bytes: &[u8]) -> bool {
    if bytes.len() < 16 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let size = (u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize).clamp(16, bytes.len());
    let brand = |b: &[u8]| b == b"avif" || b == b"avis";
    brand(&bytes[8..12]) || bytes[16..size].as_chunks::<4>().0.iter().any(|b| brand(b))
}

/// Dekoduje AVIF do RGBA 8 bit (10/12 bit → 8 bit).
#[cfg(all(feature = "avif", not(target_arch = "wasm32")))]
pub fn decode_avif(bytes: &[u8]) -> Result<RgbaImage, String> {
    use avif_decode::Image;
    let err = |e: &dyn std::fmt::Display| format!("Błąd dekodowania AVIF: {e}");
    let image = avif_decode::Decoder::from_avif(bytes).and_then(|d| d.to_image()).map_err(|e| err(&e))?;
    // Obrazy 10/12-bitowe dekoder zwraca w pełnym zakresie 16 bit.
    let f16 = |v: u16| (v >> 8) as u8;
    let (w, h, px): (usize, usize, Vec<u8>) = match image {
        Image::Rgb8(i) => (i.width(), i.height(), i.pixels().flat_map(|p| [p.r, p.g, p.b, 255]).collect()),
        Image::Rgba8(i) => (i.width(), i.height(), i.pixels().flat_map(|p| [p.r, p.g, p.b, p.a]).collect()),
        Image::Rgb16(i) => (i.width(), i.height(), i.pixels().flat_map(|p| [f16(p.r), f16(p.g), f16(p.b), 255]).collect()),
        Image::Rgba16(i) => {
            (i.width(), i.height(), i.pixels().flat_map(|p| [f16(p.r), f16(p.g), f16(p.b), f16(p.a)]).collect())
        }
        Image::Gray8(i) => (i.width(), i.height(), i.pixels().flat_map(|p| { let v = p.value(); [v, v, v, 255] }).collect()),
        Image::Gray16(i) => (i.width(), i.height(), i.pixels().flat_map(|p| { let v = f16(p.value()); [v, v, v, 255] }).collect()),
    };
    RgbaImage::from_raw(w as u32, h as u32, px).ok_or_else(|| err(&"zły rozmiar obrazu"))
}

#[cfg(not(all(feature = "avif", not(target_arch = "wasm32"))))]
pub fn decode_avif(_bytes: &[u8]) -> Result<RgbaImage, String> {
    Err("AVIF nie jest obsługiwany w tej wersji (np. w przeglądarce)".into())
}

fn read_head(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut head = Vec::with_capacity(64);
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(64).read_to_end(&mut head);
    }
    head
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUPPORTED_EXTENSIONS.iter().any(|s| s.eq_ignore_ascii_case(e)))
}

/// Rozwija ścieżki: pliki obrazów bez zmian, katalogi – obrazy w nich (bez rekursji, posortowane).
pub fn expand_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(&path)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_supported(p))
                .collect();
            files.sort();
            out.extend(files);
        } else if is_supported(&path) {
            out.push(path);
        }
    }
    out
}

fn decode_reader<R: std::io::BufRead + std::io::Seek>(mut reader: ImageReader<R>) -> Result<RgbaImage, String> {
    reader.no_limits();
    let mut decoder = reader.into_decoder().map_err(|e| format!("Nieobsługiwany format: {e}"))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| format!("Błąd dekodowania: {e}"))?;
    img.apply_orientation(orientation);
    Ok(img.into_rgba8())
}

/// Dekoduje plik i obraca go zgodnie z orientacją EXIF (zdjęcia z telefonów).
pub fn decode_path(path: &Path) -> Result<RgbaImage, String> {
    if is_avif(&read_head(path)) {
        let bytes = std::fs::read(path).map_err(|e| format!("Nie można otworzyć pliku: {e}"))?;
        return decode_avif(&bytes);
    }
    let reader = ImageReader::open(path)
        .map_err(|e| format!("Nie można otworzyć pliku: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Nie można odczytać pliku: {e}"))?;
    decode_reader(reader)
}

/// Jak [`decode_path`], ale z bajtów w pamięci (przeglądarka, MCP).
pub fn decode_bytes(bytes: &[u8]) -> Result<RgbaImage, String> {
    if is_avif(bytes) {
        return decode_avif(bytes);
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Nie można odczytać danych: {e}"))?;
    decode_reader(reader)
}

/// Informacje o pliku obrazu (bez modyfikacji).
#[derive(Clone, Debug)]
pub struct ImageInfo {
    pub format: String,
    /// Wymiary po uwzględnieniu orientacji EXIF.
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// Czy plik zawiera metadane, które eksport usuwa.
    pub has_exif: bool,
    pub has_icc: bool,
    pub bytes: u64,
}

/// Odczytuje informacje o obrazie (pełne dekodowanie – sprawdza też realną przezroczystość).
pub fn probe_path(path: &Path) -> Result<ImageInfo, String> {
    let bytes = std::fs::metadata(path).map(|m| m.len()).map_err(|e| format!("{e}"))?;
    if is_avif(&read_head(path)) {
        let data = std::fs::read(path).map_err(|e| format!("Nie można otworzyć pliku: {e}"))?;
        let img = decode_avif(&data)?;
        let has_alpha = alpha_mask(&img).is_some();
        let has_exif = crate::metadata::has_exif(&data);
        return Ok(ImageInfo { format: "avif".into(), width: img.width(), height: img.height(), has_alpha, has_exif, has_icc: false, bytes });
    }
    let reader = ImageReader::open(path)
        .map_err(|e| format!("Nie można otworzyć pliku: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Nie można odczytać pliku: {e}"))?;
    let format = reader.format().map_or("?".to_string(), |f| format!("{f:?}").to_lowercase());
    let mut decoder = reader.into_decoder().map_err(|e| format!("Nieobsługiwany format: {e}"))?;
    let has_exif = decoder.exif_metadata().ok().flatten().is_some_and(|e| !e.is_empty());
    let has_icc = decoder.icc_profile().ok().flatten().is_some_and(|e| !e.is_empty());
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| format!("Błąd dekodowania: {e}"))?;
    img.apply_orientation(orientation);
    let has_alpha = img.color().has_alpha() && alpha_mask(&img.to_rgba8()).is_some();
    Ok(ImageInfo { format, width: img.width(), height: img.height(), has_alpha, has_exif, has_icc, bytes })
}

/// Maska z kanału alfa, jeśli obraz ma jakąkolwiek przezroczystość (np. wycięty PNG).
pub fn alpha_mask(img: &RgbaImage) -> Option<Vec<u8>> {
    img.pixels().any(|p| p[3] < 255).then(|| img.pixels().map(|p| p[3]).collect())
}

/// Rozmiar mieszczący się w `max` na dłuższym boku, z zachowaniem proporcji.
pub fn fit_within(size: [u32; 2], max: u32) -> [u32; 2] {
    let long = size[0].max(size[1]);
    if long <= max {
        return size;
    }
    let s = max as f64 / long as f64;
    [
        ((size[0] as f64 * s).round() as u32).max(1),
        ((size[1] as f64 * s).round() as u32).max(1),
    ]
}

fn resize_raw(
    src: &[u8],
    size: [u32; 2],
    new_size: [u32; 2],
    pixel: fr::PixelType,
    alg: fr::ResizeAlg,
) -> Result<Vec<u8>, String> {
    let src = fr::images::ImageRef::new(size[0], size[1], src, pixel).map_err(|e| e.to_string())?;
    let mut dst = fr::images::Image::new(new_size[0], new_size[1], pixel);
    fr::Resizer::new()
        .resize(&src, &mut dst, &fr::ResizeOptions::new().resize_alg(alg).use_alpha(false))
        .map_err(|e| format!("Błąd skalowania: {e}"))?;
    Ok(dst.into_vec())
}

pub fn lanczos3() -> fr::ResizeAlg {
    fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3)
}

/// Skalowanie RGBA8 (bez premultiplikacji alfy – maskę trzymamy osobno).
pub fn resize_rgba(img: &RgbaImage, size: [u32; 2], alg: fr::ResizeAlg) -> Result<RgbaImage, String> {
    let (w, h) = img.dimensions();
    if [w, h] == size {
        return Ok(img.clone());
    }
    let data = resize_raw(img.as_raw(), [w, h], size, fr::PixelType::U8x4, alg)?;
    RgbaImage::from_raw(size[0], size[1], data).ok_or_else(|| "Błąd bufora".to_string())
}

/// Skalowanie RGB8 (eksport) filtrem Lanczos3.
pub fn resize_rgb(size: [u32; 2], rgb: Vec<u8>, new_size: [u32; 2]) -> Result<([u32; 2], Vec<u8>), String> {
    if new_size == size {
        return Ok((size, rgb));
    }
    let data = resize_raw(&rgb, size, new_size, fr::PixelType::U8x3, lanczos3())?;
    Ok((new_size, data))
}

/// Skalowanie maski (1 bajt/piksel) filtrem Lanczos3.
pub fn resize_gray(src: &[u8], size: [u32; 2], new_size: [u32; 2]) -> Result<Vec<u8>, String> {
    if new_size == size {
        return Ok(src.to_vec());
    }
    resize_raw(src, size, new_size, fr::PixelType::U8, lanczos3())
}

#[cfg(test)]
mod tests {

    #[cfg(all(feature = "avif", not(target_arch = "wasm32")))]
    #[test]
    fn avif_8_10_bit_i_alfa() {
        let data = |n: &str| std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(n)).unwrap();
        for name in ["test-8bit.avif", "test-10bit.avif"] {
            let bytes = data(name);
            assert!(is_avif(&bytes), "{name}");
            let img = decode_bytes(&bytes).unwrap();
            assert_eq!(img.dimensions(), (64, 48));
            // Lewa połowa czerwona (200,40,30), prawa niebieska (30,60,220) – kompresja stratna.
            let (l, r) = (img.get_pixel(8, 24).0, img.get_pixel(56, 24).0);
            assert!(l[0] > 170 && l[2] < 70 && l[3] == 255, "{name} lewa {l:?}");
            assert!(r[2] > 190 && r[0] < 70, "{name} prawa {r:?}");
        }
        let img = decode_bytes(&data("test-alpha.avif")).unwrap();
        assert!(img.get_pixel(8, 24).0[3] > 240 && img.get_pixel(56, 24).0[3] < 15);
        assert!(alpha_mask(&img).is_some());
        assert!(!is_avif(b"\x89PNG\r\n\x1a\n...."));
    }
    use super::*;

    #[test]
    fn rozszerzenia() {
        assert!(is_supported(Path::new("/a/b/IMG_0001.JPG")));
        assert!(is_supported(Path::new("x.webp")));
        assert!(!is_supported(Path::new("x.gif")));
        assert!(!is_supported(Path::new("bez_rozszerzenia")));
    }

    #[test]
    fn dopasowanie_rozmiaru() {
        assert_eq!(fit_within([6000, 4000], 1200), [1200, 800]);
        assert_eq!(fit_within([4000, 6000], 1200), [800, 1200]);
        assert_eq!(fit_within([800, 600], 1200), [800, 600]);
        assert_eq!(fit_within([10000, 1], 100), [100, 1]);
    }

    #[test]
    fn zmniejszanie_lanczos() {
        let (size, out) = resize_rgb([300, 200], vec![128u8; 300 * 200 * 3], [150, 100]).unwrap();
        assert_eq!(size, [150, 100]);
        assert_eq!(out.len(), 150 * 100 * 3);
        assert!(out.iter().all(|&v| v.abs_diff(128) <= 1));
        let m = resize_gray(&[200u8; 100], [10, 10], [40, 30]).unwrap();
        assert_eq!(m.len(), 1200);
        assert!(m.iter().all(|&v| v.abs_diff(200) <= 1));
    }

    #[test]
    fn dekodowanie_z_bajtow_i_alfa() {
        let mut img = RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let back = decode_bytes(&png).unwrap();
        assert_eq!(back.dimensions(), (3, 2));
        assert!(alpha_mask(&back).is_none());
        img.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        let mask = alpha_mask(&img).unwrap();
        assert_eq!(mask, vec![0, 255, 255, 255, 255, 255]);
    }
}
