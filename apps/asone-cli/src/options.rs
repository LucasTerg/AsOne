//! Parametry edycji wspólne dla CLI i MCP → `asone_core::batch::BatchOptions`.

use asone_core::batch::BatchOptions;
use asone_core::edits::{BOTTOM, EditParams, LEFT, RIGHT, TOP};
use asone_core::finish::{ExportLimits, MIB};
use schemars::JsonSchema;
use serde::Deserialize;

/// Ustawienia eksportu i edycji (wszystkie opcjonalne).
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, clap::Args)]
#[serde(deny_unknown_fields)]
pub struct EditOptions {
    /// Jakość JPEG 1–100 (domyślnie 90).
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub quality: Option<u8>,
    /// Maksymalny dłuższy bok w px (np. 1200). Bez tej opcji obraz nie jest zmniejszany.
    #[arg(long, value_name = "PX")]
    pub max_long_side: Option<u32>,
    /// Margines ze wszystkich stron w px (oryginału).
    #[arg(long, value_name = "PX")]
    pub margin: Option<u32>,
    /// Margines górny w px (nadpisuje `margin`).
    #[arg(long, value_name = "PX")]
    pub margin_top: Option<u32>,
    /// Margines prawy w px (nadpisuje `margin`).
    #[arg(long, value_name = "PX")]
    pub margin_right: Option<u32>,
    /// Margines dolny w px (nadpisuje `margin`).
    #[arg(long, value_name = "PX")]
    pub margin_bottom: Option<u32>,
    /// Margines lewy w px (nadpisuje `margin`).
    #[arg(long, value_name = "PX")]
    pub margin_left: Option<u32>,
    /// Dopasuj do kwadratu 1:1 (dodaje marginesy na krótszym boku).
    #[arg(long, num_args = 0..=1, default_missing_value = "true", value_name = "BOOL")]
    pub square: Option<bool>,
    /// Kolor tła pod marginesami i przezroczystością, `#RRGGBB` (domyślnie `#FFFFFF`).
    #[arg(long, value_name = "#RRGGBB")]
    pub background: Option<String>,
    /// Kadr w ułamkach oryginału: lewo, góra, prawo, dół (0–1), np. `0.1,0,0.9,1`.
    #[arg(long, value_name = "L,T,R,B", value_delimiter = ',', num_args = 4)]
    pub crop: Option<Vec<f32>>,
    /// Kadruj (auto): kadr do obiektu na jasnym tle.
    #[arg(long, num_args = 0..=1, default_missing_value = "true", value_name = "BOOL")]
    pub auto_crop: Option<bool>,
    /// Numer pierwszego pliku (domyślnie 1), np. 4 → `nazwa-4.jpg`, `nazwa-5.jpg`…
    #[arg(long = "start", value_name = "N")]
    pub start_number: Option<usize>,
    /// Limit szerokości w px (domyślnie 3000).
    #[arg(long, value_name = "PX")]
    pub max_width: Option<u32>,
    /// Limit wysokości w px (domyślnie 3600).
    #[arg(long, value_name = "PX")]
    pub max_height: Option<u32>,
    /// Limit rozmiaru pliku w MB (domyślnie 3).
    #[arg(long, value_name = "MB")]
    pub max_file_mb: Option<f32>,
    /// Najniższa jakość przy dopasowaniu do limitu rozmiaru (domyślnie 65).
    #[arg(long, value_name = "Q")]
    pub min_quality: Option<u8>,
    /// Najmniejszy bok w px – mniejsze zdjęcia dopełniane tłem (domyślnie 500).
    #[arg(long, value_name = "PX")]
    pub min_side: Option<u32>,
    /// Wyłącz domyślne limity (3000×3600 px, 3 MB, min. bok 500 px).
    #[arg(long, num_args = 0..=1, default_missing_value = "true", value_name = "BOOL")]
    pub no_limits: Option<bool>,
    /// Przy zmniejszaniu marginesy do 100 px zostają w pliku w pełnych pikselach, np. ramka 5 px
    /// (domyślnie tak; `false` – marginesy skalują się razem ze zdjęciem).
    #[arg(long, value_name = "BOOL")]
    pub keep_frame: Option<bool>,
}

pub fn parse_hex_color(s: &str) -> Result<[f32; 4], String> {
    let h = s.trim().trim_start_matches('#');
    let bad = || format!("Zły kolor „{s}” – oczekiwano #RRGGBB");
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map(|v| v as f32 / 255.0).map_err(|_| bad());
    Ok([c(0)?, c(2)?, c(4)?, 1.0])
}

impl EditOptions {
    pub fn to_batch(&self, base_name: &str) -> Result<BatchOptions, String> {
        let mut edits = EditParams::default();
        if let Some(bg) = &self.background {
            edits.bg_color = Some(parse_hex_color(bg)?);
        }
        if let Some(c) = &self.crop {
            let [l, t, r, b]: [f32; 4] = c.as_slice().try_into().map_err(|_| "crop: potrzebne 4 liczby".to_string())?;
            if !(0.0..=1.0).contains(&l) || !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&r) || !(0.0..=1.0).contains(&b) || l >= r || t >= b {
                return Err("crop: oczekiwano 0 ≤ lewo < prawo ≤ 1 oraz 0 ≤ góra < dół ≤ 1".into());
            }
            edits.crop = [l, t, r, b];
        }
        let all = self.margin.unwrap_or(0);
        edits.margins[TOP] = self.margin_top.unwrap_or(all);
        edits.margins[RIGHT] = self.margin_right.unwrap_or(all);
        edits.margins[BOTTOM] = self.margin_bottom.unwrap_or(all);
        edits.margins[LEFT] = self.margin_left.unwrap_or(all);
        if edits.margins.iter().any(|&m| m > 20_000) {
            return Err("Margines: maks. 20000 px".into());
        }
        let quality = self.quality.unwrap_or(90);
        if !(1..=100).contains(&quality) {
            return Err("quality: zakres 1–100".into());
        }
        if self.max_long_side.is_some_and(|m| !(16..=16384).contains(&m)) {
            return Err("max_long_side: zakres 16–16384".into());
        }
        if asone_core::naming::slugify(base_name).is_empty() {
            return Err("Nazwa bazowa po oczyszczeniu jest pusta (np. „Kubek ASONE Classic”)".into());
        }
        let defaults = if self.no_limits.unwrap_or(false) { ExportLimits::NONE } else { ExportLimits::default() };
        let limits = ExportLimits {
            max_width: self.max_width.or(defaults.max_width),
            max_height: self.max_height.or(defaults.max_height),
            max_bytes: self.max_file_mb.map(|mb| (mb.max(0.05) as f64 * MIB as f64) as u64).or(defaults.max_bytes),
            min_side: self.min_side.or(defaults.min_side),
            min_quality: self.min_quality.unwrap_or(ExportLimits::default().min_quality).clamp(1, 100),
            keep_frame: self.keep_frame.unwrap_or(true),
        };
        if self.start_number == Some(0) {
            return Err("start: numeracja zaczyna się od 1".into());
        }
        Ok(BatchOptions {
            base_name: base_name.to_string(),
            quality,
            max_long_side: self.max_long_side,
            edits,
            square: self.square.unwrap_or(false),
            auto_crop: self.auto_crop.unwrap_or(false),
            start_number: self.start_number.unwrap_or(1),
            limits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kolory() {
        assert_eq!(parse_hex_color("#FFFFFF").unwrap(), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(parse_hex_color("000000").unwrap(), [0.0, 0.0, 0.0, 1.0]);
        assert!(parse_hex_color("#FFF").is_err());
        assert!(parse_hex_color("#GG0000").is_err());
    }

    #[test]
    fn marginesy_i_walidacja() {
        let o = EditOptions { margin: Some(10), margin_left: Some(3), ..Default::default() };
        let b = o.to_batch("buty").unwrap();
        assert_eq!(b.edits.margins, [10, 10, 10, 3]);
        assert_eq!(b.quality, 90);
        assert!(EditOptions::default().to_batch("!!").is_err());
        let o = EditOptions { crop: Some(vec![0.5, 0.0, 0.4, 1.0]), ..Default::default() };
        assert!(o.to_batch("x").is_err());
        let o = EditOptions { max_long_side: Some(5), ..Default::default() };
        assert!(o.to_batch("x").is_err());
    }

    #[test]
    fn limity_domyslnie_wlaczone() {
        let b = EditOptions::default().to_batch("x").unwrap();
        assert_eq!((b.limits.max_width, b.limits.max_height), (Some(3000), Some(3600)));
        assert_eq!(b.limits.max_bytes, Some(3 * MIB));
        let b = EditOptions { no_limits: Some(true), max_width: Some(800), ..Default::default() }.to_batch("x").unwrap();
        assert_eq!((b.limits.max_width, b.limits.max_height, b.limits.max_bytes), (Some(800), None, None));
        let b = EditOptions { start_number: Some(4), ..Default::default() }.to_batch("x").unwrap();
        assert_eq!(b.start_number, 4);
    }
}
