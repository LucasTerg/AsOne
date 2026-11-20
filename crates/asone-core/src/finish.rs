//! Ostatni etap eksportu: zmniejszenie, limity „na sztywno” i kompresja do rozmiaru pliku.
//!
//! Limity z Ustawień działają nawet wtedy, gdy ktoś zapomni o nich przy pojedynczym zdjęciu:
//! 1. obraz nie może przekroczyć `max_width × max_height` (domyślnie 3000 × 3600) – zmniejszenie Lanczos3,
//! 2. bok mniejszy niż `min_side` (domyślnie 500 px) dopełniamy tłem – białym – bez powiększania
//!    pikseli,
//! 3. plik nie może przekroczyć `max_bytes` (domyślnie 3 MB) – najpierw obniżamy jakość
//!    (szukając najwyższej mieszczącej się, nie niżej niż `min_quality`), a gdy to nie wystarczy,
//!    zmniejszamy obraz o 10 % na krok.
//!
//! Ramka przy zmniejszaniu ([`finish_with_margins`], `keep_frame`): gdy zdjęcie z marginesami trzeba
//! zmniejszyć, płótno ma ten sam rozmiar co przy zwykłym zmniejszeniu, ale marginesy-ramki (do
//! [`FRAME_MAX`] px i do 2 % boku) zostają w pliku w pełnych pikselach (ramka 5 px to dalej 5 px) –
//! zmniejsza się samo zdjęcie. Większe marginesy (np. z „Dopasuj do kwadratu”) skalują się jak dotąd.

use crate::edits::{BOTTOM, LEFT, RIGHT, TOP};
use crate::imageio::{fit_within, resize_rgb};
use crate::jpeg::encode_jpeg;

pub const MIB: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportLimits {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_bytes: Option<u64>,
    /// Krótszy bok poniżej tej wartości jest dopełniany tłem (bez powiększania pikseli).
    pub min_side: Option<u32>,
    /// Najniższa jakość, do której wolno zejść przy dopasowaniu do `max_bytes`.
    pub min_quality: u8,
    /// Przy zmniejszaniu marginesy do [`FRAME_MAX`] px zostają w pełnych pikselach (zmniejsza się zdjęcie).
    pub keep_frame: bool,
}

impl Default for ExportLimits {
    fn default() -> Self {
        Self {
            max_width: Some(3000),
            max_height: Some(3600),
            max_bytes: Some(3 * MIB),
            min_side: Some(MIN_SIDE),
            min_quality: 65,
            keep_frame: true,
        }
    }
}

/// Najmniejszy bok zapisanego zdjęcia (px); mniejsze dopełniane są tłem.
pub const MIN_SIDE: u32 = 500;

/// Marginesy do tylu px (i do 2 % boku) to „ramka”: po zmniejszeniu zostają w pliku w pełnych pikselach.
pub const FRAME_MAX: u32 = 100;

/// Margines `m` na osi o długości `len` jest ramką (a nie np. dopełnieniem do kwadratu).
pub fn is_frame(m: u32, len: u32) -> bool {
    m > 0 && m <= FRAME_MAX && m.saturating_mul(50) <= len
}

/// Do takiej różnicy proporcji treść wypełnia środek dokładnie (niewidoczne), żeby ramka miała równe px.
const FILL_TOLERANCE: f64 = 0.0025;

/// Zmniejszony obraz (rozmiar, piksele RGB8).
pub type Rgb = ([u32; 2], Vec<u8>);

impl ExportLimits {
    pub const NONE: ExportLimits =
        ExportLimits { max_width: None, max_height: None, max_bytes: None, min_side: None, min_quality: 1, keep_frame: false };
}

/// Gotowy plik JPEG z opisem automatycznych korekt.
#[derive(Clone, Debug)]
pub struct Finished {
    pub size: [u32; 2],
    pub jpeg: Vec<u8>,
    pub quality: u8,
    /// Co zostało dopasowane automatycznie (pusty = nic).
    pub notes: Vec<String>,
}

/// Rozmiar mieszczący się w ramce `max_w × max_h` (z zachowaniem proporcji) albo `None`, gdy już się mieści.
pub fn fit_box(size: [u32; 2], max_w: Option<u32>, max_h: Option<u32>) -> Option<[u32; 2]> {
    let sx = max_w.map_or(1.0, |m| m.max(1) as f64 / size[0].max(1) as f64);
    let sy = max_h.map_or(1.0, |m| m.max(1) as f64 / size[1].max(1) as f64);
    let s = sx.min(sy);
    (s < 1.0).then(|| {
        [
            ((size[0] as f64 * s).floor() as u32).max(1),
            ((size[1] as f64 * s).floor() as u32).max(1),
        ]
    })
}

/// Dopełnia obraz tłem do `new` (wyśrodkowany).
pub fn pad_rgb(size: [u32; 2], rgb: &[u8], new: [u32; 2], bg: [u8; 3]) -> Vec<u8> {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let (nw, nh) = (new[0] as usize, new[1] as usize);
    let mut out: Vec<u8> = std::iter::repeat_n(bg, nw * nh).flatten().collect();
    let (ox, oy) = ((nw - w) / 2, (nh - h) / 2);
    for y in 0..h {
        let src = &rgb[y * w * 3..(y + 1) * w * 3];
        let start = ((oy + y) * nw + ox) * 3;
        out[start..start + w * 3].copy_from_slice(src);
    }
    out
}

/// Rozmiar po zmniejszeniu (dłuższy bok, potem limity szer. × wys.) albo `None`, gdy się mieści.
fn shrunk_size(size: [u32; 2], max_long_side: Option<u32>, limits: &ExportLimits) -> Option<[u32; 2]> {
    let s = max_long_side.map_or(size, |max| fit_within(size, max));
    let s = fit_box(s, limits.max_width, limits.max_height).unwrap_or(s);
    (s != size).then_some(s)
}

/// Zmniejszenie z zachowaniem ramki: płótno jak przy zwykłym zmniejszeniu, marginesy-ramki
/// ([`is_frame`]) w pełnych pikselach, pozostałe skalowane; treść (bez marginesów) wypełnia resztę.
/// `None` – nic do zmniejszenia, brak ramki albo marginesy nie mieszczą się w płótnie.
pub fn shrink_keep_frame(
    size: [u32; 2],
    rgb: &[u8],
    margins: [u32; 4],
    max_long_side: Option<u32>,
    limits: &ExportLimits,
    bg: [u8; 3],
) -> Result<Option<Rgb>, String> {
    let any_frame = [(LEFT, 0), (RIGHT, 0), (TOP, 1), (BOTTOM, 1)].iter().any(|&(i, axis)| is_frame(margins[i], size[axis]));
    if !any_frame {
        return Ok(None);
    }
    let Some(canvas) = shrunk_size(size, max_long_side, limits) else { return Ok(None) };
    let [ml, mr, mt, mb] = [margins[LEFT], margins[RIGHT], margins[TOP], margins[BOTTOM]];
    if ml + mr >= size[0] || mt + mb >= size[1] {
        return Ok(None);
    }
    let (kx, ky) = (canvas[0] as f64 / size[0] as f64, canvas[1] as f64 / size[1] as f64);
    let keep = |m: u32, k: f64, len: u32| if is_frame(m, len) { m } else { (m as f64 * k).round() as u32 };
    let [fl, fr, ft, fb] = [keep(ml, kx, size[0]), keep(mr, kx, size[0]), keep(mt, ky, size[1]), keep(mb, ky, size[1])];
    if fl + fr >= canvas[0] || ft + fb >= canvas[1] {
        return Ok(None);
    }
    let content = [size[0] - ml - mr, size[1] - mt - mb];
    let inner = [canvas[0] - fl - fr, canvas[1] - ft - fb];
    let (cx, cy) = (inner[0] as f64 / content[0] as f64, inner[1] as f64 / content[1] as f64);
    let target = if (cx - cy).abs() <= FILL_TOLERANCE * cx.max(cy) {
        inner
    } else {
        let c = cx.min(cy).min(1.0);
        [
            ((content[0] as f64 * c).round() as u32).clamp(1, inner[0]),
            ((content[1] as f64 * c).round() as u32).clamp(1, inner[1]),
        ]
    };
    let (w, cw) = (size[0] as usize, content[0] as usize);
    let mut cropped = Vec::with_capacity(cw * content[1] as usize * 3);
    for y in mt as usize..(mt + content[1]) as usize {
        let row = (y * w + ml as usize) * 3;
        cropped.extend_from_slice(&rgb[row..row + cw * 3]);
    }
    let (_, scaled) = resize_rgb(content, cropped, target)?;
    let mut out: Vec<u8> = std::iter::repeat_n(bg, canvas[0] as usize * canvas[1] as usize).flatten().collect();
    let ox = (fl + (inner[0] - target[0]) / 2) as usize;
    let oy = (ft + (inner[1] - target[1]) / 2) as usize;
    let (tw, nw) = (target[0] as usize, canvas[0] as usize);
    for y in 0..target[1] as usize {
        let dst = ((oy + y) * nw + ox) * 3;
        out[dst..dst + tw * 3].copy_from_slice(&scaled[y * tw * 3..(y + 1) * tw * 3]);
    }
    Ok(Some((canvas, out)))
}

/// Jak [`finish`], ale z marginesami obrazu: przy `limits.keep_frame` zmniejsza z zachowaniem ramki.
pub fn finish_with_margins(
    size: [u32; 2],
    rgb: Vec<u8>,
    margins: [u32; 4],
    max_long_side: Option<u32>,
    quality: u8,
    bg: [u8; 3],
    limits: &ExportLimits,
) -> Result<Finished, String> {
    if limits.keep_frame
        && let Some((canvas, out)) = shrink_keep_frame(size, &rgb, margins, max_long_side, limits, bg)?
    {
        let mut f = finish(canvas, out, None, quality, bg, limits)?;
        f.notes.insert(
            0,
            format!("zmniejszono {}×{} → {}×{}, ramka zachowana w pełnych pikselach", size[0], size[1], canvas[0], canvas[1]),
        );
        return Ok(f);
    }
    finish(size, rgb, max_long_side, quality, bg, limits)
}

fn mb(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / MIB as f64)
}

/// Zmniejszenie, limity i kompresja. `bg` – kolor dopełnienia przy `min_side`.
pub fn finish(
    size: [u32; 2],
    rgb: Vec<u8>,
    max_long_side: Option<u32>,
    quality: u8,
    bg: [u8; 3],
    limits: &ExportLimits,
) -> Result<Finished, String> {
    let mut notes = Vec::new();
    let (mut size, mut rgb) = (size, rgb);

    if let Some(max) = max_long_side {
        (size, rgb) = resize_rgb(size, rgb, fit_within(size, max))?;
    }
    if let Some(target) = fit_box(size, limits.max_width, limits.max_height) {
        notes.push(format!(
            "zmniejszono {}×{} → {}×{} (limit {}×{})",
            size[0],
            size[1],
            target[0],
            target[1],
            limits.max_width.map_or("∞".into(), |v| v.to_string()),
            limits.max_height.map_or("∞".into(), |v| v.to_string()),
        ));
        (size, rgb) = resize_rgb(size, rgb, target)?;
    }
    if let Some(min) = limits.min_side.filter(|&m| size[0] < m || size[1] < m) {
        let new = [size[0].max(min), size[1].max(min)];
        notes.push(format!("dopełniono tłem {}×{} → {}×{} (min. {min} px)", size[0], size[1], new[0], new[1]));
        rgb = pad_rgb(size, &rgb, new, bg);
        size = new;
    }

    let quality = quality.clamp(1, 100);
    let jpeg = encode_jpeg(&rgb, size, quality)?;
    let Some(max_bytes) = limits.max_bytes.filter(|&m| jpeg.len() as u64 > m) else {
        return Ok(Finished { size, jpeg, quality, notes });
    };

    // Najwyższa jakość w [min_quality, quality - 1], przy której plik się mieści.
    let min_q = limits.min_quality.clamp(1, quality);
    let (mut lo, mut hi) = (min_q as i32, quality as i32 - 1);
    let mut best: Option<(u8, Vec<u8>)> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let j = encode_jpeg(&rgb, size, mid as u8)?;
        if j.len() as u64 <= max_bytes {
            best = Some((mid as u8, j));
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    if let Some((q, j)) = best {
        notes.push(format!("jakość {quality} → {q} (limit {})", mb(max_bytes)));
        return Ok(Finished { size, jpeg: j, quality: q, notes });
    }

    // Nawet minimalna jakość to za dużo: zmniejszamy obraz krokami po 10 %.
    let mut scale = 0.9f64;
    let mut last = None;
    loop {
        let target = [
            ((size[0] as f64 * scale) as u32).max(1),
            ((size[1] as f64 * scale) as u32).max(1),
        ];
        if target[0] < 100 || target[1] < 100 {
            break;
        }
        let (s, r) = resize_rgb(size, rgb.clone(), target)?;
        let j = encode_jpeg(&r, s, min_q)?;
        let fits = j.len() as u64 <= max_bytes;
        last = Some((s, j));
        if fits {
            break;
        }
        scale *= 0.9;
    }
    match last {
        Some((s, j)) => {
            let fits = j.len() as u64 <= max_bytes;
            notes.push(format!(
                "jakość {quality} → {min_q} i zmniejszono {}×{} → {}×{} (limit {}){}",
                size[0],
                size[1],
                s[0],
                s[1],
                mb(max_bytes),
                if fits { "" } else { " – nadal za duży" }
            ));
            Ok(Finished { size: s, jpeg: j, quality: min_q, notes })
        }
        None => {
            let j = encode_jpeg(&rgb, size, min_q)?;
            notes.push(format!("jakość {quality} → {min_q}; nie udało się zejść poniżej {}", mb(max_bytes)));
            Ok(Finished { size, jpeg: j, quality: min_q, notes })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Szum – trudny do skompresowania, żeby limit rozmiaru zadziałał.
    fn noise(size: [u32; 2]) -> Vec<u8> {
        let mut x = 0x1234_5678u32;
        (0..size[0] * size[1] * 3)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x >> 24) as u8
            })
            .collect()
    }

    /// Obraz T×T z ramką `m` (biała) wokół czarnej treści.
    fn framed(size: [u32; 2], m: [u32; 4]) -> Vec<u8> {
        let mut rgb = vec![255u8; (size[0] * size[1] * 3) as usize];
        for y in m[TOP]..size[1] - m[BOTTOM] {
            for x in m[LEFT]..size[0] - m[RIGHT] {
                let i = ((y * size[0] + x) * 3) as usize;
                rgb[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        rgb
    }

    /// Szerokość białego pasa od krawędzi do pierwszego ciemnego piksela (wiersz/kolumna środkowa).
    fn white_run(size: [u32; 2], rgb: &[u8], side: usize) -> u32 {
        let px = |x: u32, y: u32| rgb[((y * size[0] + x) * 3) as usize];
        let (cx, cy) = (size[0] / 2, size[1] / 2);
        let mut n = 0;
        loop {
            let v = match side {
                TOP => px(cx, n),
                BOTTOM => px(cx, size[1] - 1 - n),
                LEFT => px(n, cy),
                _ => px(size[0] - 1 - n, cy),
            };
            if v < 128 {
                return n;
            }
            n += 1;
        }
    }

    #[test]
    fn ramka_5px_zostaje_po_zmniejszeniu() {
        // 4010×5010 z ramką 5 px → limit 3000×3600: płótno jak dotąd, ramka nadal 5 px.
        let size = [4010, 5010];
        let m = [5; 4];
        let limits = ExportLimits::default();
        let (canvas, out) = shrink_keep_frame(size, &framed(size, m), m, None, &limits, [255; 3]).unwrap().unwrap();
        assert_eq!(Some(canvas), fit_box(size, Some(3000), Some(3600)));
        for side in [TOP, RIGHT, BOTTOM, LEFT] {
            assert_eq!(white_run(canvas, &out, side), 5, "bok {side}");
        }
    }

    #[test]
    fn duze_marginesy_skaluja_sie_kwadrat_zostaje() {
        // Treść 400×500 + kwadrat (50 z boków) + ramka 5 px: 510×510 → limit 300 px.
        let size = [510, 510];
        let m = [5, 55, 5, 55];
        let limits = ExportLimits { max_width: Some(300), max_height: Some(300), keep_frame: true, ..ExportLimits::NONE };
        let (canvas, out) = shrink_keep_frame(size, &framed(size, m), m, None, &limits, [255; 3]).unwrap().unwrap();
        assert_eq!(canvas, [300, 300], "kwadrat zostaje");
        assert_eq!(white_run(canvas, &out, TOP), 5);
        assert_eq!(white_run(canvas, &out, BOTTOM), 5);
        // 55 px to ponad 2 % boku – dopełnienie do kwadratu, skaluje się (≈ 32 px).
        assert!((30..=40).contains(&white_run(canvas, &out, LEFT)), "{}", white_run(canvas, &out, LEFT));
        // Same duże marginesy (bez ramki) – jak dotąd.
        let m = [0, 55, 0, 55];
        assert!(shrink_keep_frame(size, &framed(size, m), m, None, &limits, [255; 3]).unwrap().is_none());
    }

    #[test]
    fn bez_zmniejszenia_albo_bez_marginesow_nic() {
        let limits = ExportLimits::default();
        assert!(shrink_keep_frame([800, 600], &framed([800, 600], [5; 4]), [5; 4], None, &limits, [255; 3]).unwrap().is_none());
        assert!(shrink_keep_frame([4000, 5000], &vec![0; 4000 * 5000 * 3], [0; 4], None, &limits, [255; 3]).unwrap().is_none());
        // „Max dłuższy bok” też zmniejsza – ramka zostaje.
        let size = [1010, 810];
        let (canvas, out) = shrink_keep_frame(size, &framed(size, [5; 4]), [5; 4], Some(500), &limits, [255; 3]).unwrap().unwrap();
        assert_eq!(canvas, fit_within(size, 500));
        assert_eq!(white_run(canvas, &out, LEFT), 5);
    }

    #[test]
    fn finish_z_marginesami_wylaczone_jak_dotad() {
        let size = [1010, 810];
        let rgb = framed(size, [5; 4]);
        let off = ExportLimits { keep_frame: false, ..ExportLimits::default() };
        let a = finish_with_margins(size, rgb.clone(), [5; 4], Some(500), 95, [255; 3], &off).unwrap();
        let b = finish(size, rgb.clone(), Some(500), 95, [255; 3], &off).unwrap();
        assert_eq!(a.jpeg, b.jpeg);
        let on = finish_with_margins(size, rgb, [5; 4], Some(500), 95, [255; 3], &ExportLimits::default()).unwrap();
        assert!(on.notes[0].contains("ramka zachowana"), "{:?}", on.notes);
    }

    #[test]
    fn ramka_limitu() {
        assert_eq!(fit_box([6000, 4000], Some(3000), Some(3600)), Some([3000, 2000]));
        assert_eq!(fit_box([3000, 4800], Some(3000), Some(3600)), Some([2250, 3600]));
        assert_eq!(fit_box([2000, 2000], Some(3000), Some(3600)), None);
        assert_eq!(fit_box([5000, 10], None, Some(3600)), None);
    }

    #[test]
    fn limit_wymiarow_dziala_bez_ustawienia_przez_uzytkownika() {
        let limits = ExportLimits { max_width: Some(300), max_height: Some(360), max_bytes: None, min_side: None, ..Default::default() };
        let f = finish([600, 400], vec![200; 600 * 400 * 3], None, 90, [255; 3], &limits).unwrap();
        assert_eq!(f.size, [300, 200]);
        assert_eq!(f.notes.len(), 1);
    }

    #[test]
    fn domyslnie_min_500_px_bialym_tlem() {
        // 300×200 (np. po „Max dłuższy bok”) → 500×500, środek zostaje, brzegi białe.
        let f = finish([300, 200], vec![10; 300 * 200 * 3], None, 95, [255; 3], &ExportLimits::default()).unwrap();
        assert_eq!(f.size, [500, 500]);
        assert!(f.notes.iter().any(|n| n.contains("min. 500 px")), "{:?}", f.notes);
        let img = image::load_from_memory(&f.jpeg).unwrap().to_rgb8();
        assert!(img.get_pixel(5, 5).0.iter().all(|&c| c > 245), "róg biały");
        assert!(img.get_pixel(250, 250).0.iter().all(|&c| c < 30), "środek ze zdjęcia");
    }

    #[test]
    fn dopelnienie_do_minimum() {
        let limits = ExportLimits { min_side: Some(50), ..ExportLimits::NONE };
        let f = finish([20, 60], vec![0; 20 * 60 * 3], None, 90, [255; 3], &limits).unwrap();
        assert_eq!(f.size, [50, 60]);
        let out = pad_rgb([1, 1], &[9, 9, 9], [3, 1], [255, 255, 255]);
        assert_eq!(out, vec![255, 255, 255, 9, 9, 9, 255, 255, 255]);
    }

    #[test]
    fn limit_rozmiaru_pliku() {
        let size = [256, 256];
        let full = encode_jpeg(&noise(size), size, 95).unwrap().len() as u64;
        // Limit między jakością 95 a minimalną: wystarczy obniżyć jakość.
        let limits = ExportLimits { max_bytes: Some(full * 2 / 3), min_quality: 30, ..ExportLimits::NONE };
        let f = finish(size, noise(size), None, 95, [255; 3], &limits).unwrap();
        assert!(f.jpeg.len() as u64 <= full * 2 / 3);
        assert!(f.quality < 95 && f.quality >= 30, "jakość {}", f.quality);
        assert_eq!(f.size, size);
        // Limit nie do osiągnięcia samą jakością: obraz musi się zmniejszyć.
        let limits = ExportLimits { max_bytes: Some(4000), min_quality: 90, ..ExportLimits::NONE };
        let f = finish(size, noise(size), None, 95, [255; 3], &limits).unwrap();
        assert!(f.size[0] < 256);
        assert_eq!(f.quality, 90);
        assert!(f.jpeg.len() <= 4000 || f.notes[0].contains("nadal"));
    }
}
