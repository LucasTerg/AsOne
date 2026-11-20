//! Eksport wsadowy bez GUI (CLI, MCP): te same nazwy, parametry i kompresja co „Zapisz z zakładek”.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use image::RgbaImage;

use crate::autocrop::{DEFAULT_THRESHOLD, content_bbox};
use crate::compose::compose_rgb;
use crate::edits::{EditParams, WHITE};
use crate::finish::{ExportLimits, finish_with_margins};
use crate::imageio::{alpha_mask, decode_path};
use crate::naming::plan_names_from;

#[derive(Clone, Debug)]
pub struct BatchOptions {
    /// Nazwa bazowa paczki, np. `123-marka model` → `123-marka-model-1.jpg`, …
    pub base_name: String,
    /// Jakość JPEG 1..=100.
    pub quality: u8,
    /// Limit dłuższego boku w px (`None` = bez zmniejszania).
    pub max_long_side: Option<u32>,
    /// Kadr, marginesy, tło – wspólne dla wszystkich zdjęć.
    pub edits: EditParams,
    /// „Dopasuj do kwadratu 1:1” (marginesy na krótszym boku) dla każdego zdjęcia.
    pub square: bool,
    /// „Kadruj (auto)”: kadr do obiektu na jasnym tle (przed marginesami i kwadratem).
    pub auto_crop: bool,
    /// Numer pierwszego pliku („Start nr”).
    pub start_number: usize,
    /// Limity pilnowane przy każdym zapisie (domyślnie 3000×3600 i 3 MB).
    pub limits: ExportLimits,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            base_name: String::new(),
            quality: 90,
            max_long_side: None,
            edits: EditParams::default(),
            square: false,
            auto_crop: false,
            start_number: 1,
            limits: ExportLimits::default(),
        }
    }
}

pub struct ProcessedImage {
    pub source_size: [u32; 2],
    pub size: [u32; 2],
    pub jpeg: Vec<u8>,
    pub quality: u8,
    /// Automatyczne korekty (limity, auto-kadr).
    pub notes: Vec<String>,
}

/// Kolor tła jako RGB8.
pub fn bg_rgb8(edits: &EditParams) -> [u8; 3] {
    let c = edits.bg_color.unwrap_or(WHITE);
    [0, 1, 2].map(|i| (c[i].clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Przetwarza jeden obraz w pamięci: (auto-kadr) → kompozycja → zmniejszenie i limity → JPEG.
/// Przezroczystość pliku (np. wycięty PNG) staje się maską, a pod nią jest tło.
pub fn process_image(img: &RgbaImage, opts: &BatchOptions) -> Result<ProcessedImage, String> {
    let source_size = [img.width(), img.height()];
    let mut edits = opts.edits.clone();
    let mask = alpha_mask(img);
    if mask.is_some() {
        edits.use_bg_removal = true;
    }
    let mut notes = Vec::new();
    if opts.auto_crop {
        match content_bbox(img, mask.as_deref(), DEFAULT_THRESHOLD) {
            Some(r) => {
                edits.set_crop_px(r, source_size);
                notes.push(format!("auto-kadr {}×{}", r.width(), r.height()));
            }
            None => notes.push("auto-kadr: nie znaleziono obiektu".into()),
        }
    }
    if opts.square {
        edits.fit_square(source_size);
    }
    let (size, rgb) = compose_rgb(img, mask.as_deref(), &edits);
    let f = finish_with_margins(size, rgb, edits.margins, opts.max_long_side, opts.quality, bg_rgb8(&edits), &opts.limits)?;
    notes.extend(f.notes);
    Ok(ProcessedImage { source_size, size: f.size, jpeg: f.jpeg, quality: f.quality, notes })
}

/// Nazwy plików wynikowych dla `count` zdjęć (numeracja od 1).
pub fn plan_names(base_name: &str, count: usize) -> Vec<String> {
    plan_names_from(base_name, 1, count)
}

/// Czy zapis pod `target` nadpisałby któryś z plików źródłowych?
pub fn would_overwrite_source(target: &Path, sources: &[PathBuf]) -> bool {
    let canon = |p: &Path| -> PathBuf {
        if let Ok(c) = p.canonicalize() {
            return c;
        }
        // Plik jeszcze nie istnieje: porównujemy kanoniczny katalog + nazwę.
        match (p.parent().and_then(|d| d.canonicalize().ok()), p.file_name()) {
            (Some(dir), Some(name)) => dir.join(name),
            _ => p.to_path_buf(),
        }
    };
    let target = canon(target);
    sources.iter().any(|s| canon(s) == target)
}

/// Sprawdza, czy wolno zapisać `target`: nie oryginał, nie dowiązanie, nie katalog.
/// Istniejący zwykły plik `.jpg` (wcześniejszy eksport) może zostać zastąpiony.
pub fn check_target(target: &Path, sources: &[PathBuf]) -> Result<(), String> {
    if would_overwrite_source(target, sources) {
        return Err("Pominięto: nadpisałby oryginalne zdjęcie".into());
    }
    match std::fs::symlink_metadata(target) {
        Ok(m) if m.file_type().is_symlink() => Err("Pominięto: cel jest dowiązaniem symbolicznym".into()),
        Ok(m) if !m.is_file() => Err("Pominięto: cel nie jest zwykłym plikiem".into()),
        _ => Ok(()),
    }
}

/// Zapis atomowy: plik tymczasowy w tym samym katalogu, potem zmiana nazwy.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = target.with_file_name(format!(".{name}.part"));
    std::fs::write(&tmp, bytes).map_err(|e| format!("Błąd zapisu: {e}"))?;
    std::fs::rename(&tmp, target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Błąd zapisu: {e}")
    })
}

/// Zapisany plik.
#[derive(Debug, Clone)]
pub struct Saved {
    pub size: [u32; 2],
    pub bytes: u64,
    pub quality: u8,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BatchEntry {
    pub index: usize,
    pub source: PathBuf,
    pub file_name: String,
    pub output: PathBuf,
    pub result: Result<Saved, String>,
}

/// Zapisuje `inputs` w podanej kolejności jako `<slug>-1.jpg`, `<slug>-2.jpg`, … w `out_dir`.
/// Przetwarza równolegle w `threads` wątkach; `on_done` jest wołane po każdym pliku.
pub fn export_batch(
    inputs: &[PathBuf],
    out_dir: &Path,
    opts: &BatchOptions,
    threads: usize,
    on_done: impl Fn(&BatchEntry) + Sync,
) -> Vec<BatchEntry> {
    let names = plan_names_from(&opts.base_name, opts.start_number, inputs.len());
    let dir_error = std::fs::create_dir_all(out_dir).err().map(|e| format!("Nie można utworzyć folderu: {e}"));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<BatchEntry>>> = Mutex::new(vec![None; inputs.len()]);

    let work = || {
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= inputs.len() {
                break;
            }
            let output = out_dir.join(&names[i]);
            let result = match &dir_error {
                Some(e) => Err(e.clone()),
                None => check_target(&output, inputs)
                    .and_then(|()| decode_path(&inputs[i]))
                    .and_then(|img| process_image(&img, opts))
                    .and_then(|p| {
                        write_atomic(&output, &p.jpeg).map(|()| Saved {
                            size: p.size,
                            bytes: p.jpeg.len() as u64,
                            quality: p.quality,
                            notes: p.notes,
                        })
                    }),
            };
            let entry = BatchEntry {
                index: opts.start_number + i,
                source: inputs[i].clone(),
                file_name: names[i].clone(),
                output,
                result,
            };
            on_done(&entry);
            results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(entry);
        }
    };
    std::thread::scope(|s| {
        for _ in 1..threads.clamp(1, inputs.len().max(1)) {
            s.spawn(work);
        }
        work();
    });
    results
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("asone-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn ochrona_oryginalow() {
        let dir = temp_dir("guard");
        let src = dir.join("123-marka-model-1.jpg");
        std::fs::write(&src, b"x").unwrap();
        assert!(would_overwrite_source(&dir.join("123-marka-model-1.jpg"), std::slice::from_ref(&src)));
        assert!(would_overwrite_source(&dir.join(".").join("123-marka-model-1.jpg"), std::slice::from_ref(&src)));
        assert!(!would_overwrite_source(&dir.join("123-marka-model-2.jpg"), std::slice::from_ref(&src)));
        assert!(!would_overwrite_source(&dir.join("web").join("123-marka-model-1.jpg"), std::slice::from_ref(&src)));
        assert!(check_target(&dir, &[]).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn eksport_wsadowy_w_kolejnosci() {
        let dir = temp_dir("batch");
        let mut inputs = Vec::new();
        for (i, (w, h)) in [(300u32, 200u32), (100, 400)].into_iter().enumerate() {
            let p = dir.join(format!("src{i}.png"));
            RgbaImage::from_pixel(w, h, image::Rgba([200, 10, 10, 255])).save(&p).unwrap();
            inputs.push(p);
        }
        let opts = BatchOptions {
            base_name: "Żółte buty".into(),
            max_long_side: Some(100),
            square: true,
            ..Default::default()
        };
        let out = dir.join("web");
        let entries = export_batch(&inputs, &out, &opts, 4, |_| {});
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file_name, "Zolte-buty-1.jpg");
        assert_eq!(entries[1].file_name, "Zolte-buty-2.jpg");
        for e in &entries {
            let saved = e.result.clone().unwrap();
            // 100×100 (dłuższy bok + kwadrat) → domyślnie dopełnione białym tłem do min. 500 px.
            assert_eq!(saved.size, [500, 500]);
            assert_eq!(std::fs::metadata(&e.output).unwrap().len(), saved.bytes);
        }

        // Start nr + auto-kadr + limit wymiarów.
        let opts = BatchOptions {
            base_name: "buty".into(),
            start_number: 4,
            auto_crop: true,
            limits: ExportLimits { max_width: Some(50), max_height: Some(50), ..ExportLimits::NONE },
            ..Default::default()
        };
        let entries = export_batch(&inputs[..1], &out, &opts, 1, |_| {});
        assert_eq!(entries[0].file_name, "buty-4.jpg");
        let saved = entries[0].result.clone().unwrap();
        assert_eq!(saved.size, [50, 33]);
        assert!(saved.notes.iter().any(|n| n.contains("auto-kadr")), "{:?}", saved.notes);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
