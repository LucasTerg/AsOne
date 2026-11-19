//! Ustawienia zapamiętywane między uruchomieniami (Ustawienia › Preferencje).

use asone_core::finish::{ExportLimits, MIB};
use serde::{Deserialize, Serialize};

use crate::theme::{Appearance, ThemeKind};

pub const STORAGE_KEY: &str = "asone-settings";

/// Dostępne skale interfejsu (%), przełączane przyciskami −/+ na pasku tytułu.
pub const UI_SCALES: [u32; 9] = [75, 85, 100, 110, 125, 150, 175, 200, 250];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasColor {
    /// Z motywu.
    #[default]
    Theme,
    Black,
    DarkGray,
    MediumGray,
    LightGray,
    White,
    Custom,
}

impl CanvasColor {
    pub const ALL: [CanvasColor; 7] = [
        CanvasColor::Theme,
        CanvasColor::Black,
        CanvasColor::DarkGray,
        CanvasColor::MediumGray,
        CanvasColor::LightGray,
        CanvasColor::White,
        CanvasColor::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CanvasColor::Theme => "Z motywu",
            CanvasColor::Black => "Czarny",
            CanvasColor::DarkGray => "Ciemnoszary",
            CanvasColor::MediumGray => "Szary",
            CanvasColor::LightGray => "Jasnoszary",
            CanvasColor::White => "Biały",
            CanvasColor::Custom => "Własny",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // --- Interfejs ---
    pub theme: ThemeKind,
    pub canvas_color: CanvasColor,
    pub canvas_custom: [u8; 3],
    /// Skala całego interfejsu w %.
    pub ui_scale: u32,
    pub show_tooltips: bool,
    pub animations: bool,
    /// Systemowy pasek tytułu zamiast własnego (Linux/Windows).
    pub system_title_bar: bool,

    // --- Eksport (górny pasek) ---
    pub quality: u8,
    pub limit_long_side: bool,
    pub max_long_side: u32,
    /// Numer pierwszego pliku w paczce („Start nr”).
    pub start_number: u32,
    /// Podfolder obok zdjęć, gdy nie wybrano folderu.
    pub out_subfolder: String,

    // --- Limity „na sztywno” (pilnowane przy każdym eksporcie) ---
    pub enforce_dimensions: bool,
    pub max_width: u32,
    pub max_height: u32,
    pub enforce_file_size: bool,
    pub max_file_mb: f32,
    pub min_quality: u8,
    pub enforce_min_side: bool,
    pub min_side: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeKind::AsOne,
            canvas_color: CanvasColor::Theme,
            canvas_custom: [0x28, 0x28, 0x28],
            ui_scale: 100,
            show_tooltips: true,
            animations: true,
            system_title_bar: false,
            quality: 90,
            limit_long_side: false,
            max_long_side: 1200,
            start_number: 1,
            out_subfolder: "web".into(),
            enforce_dimensions: true,
            max_width: 3000,
            max_height: 3600,
            enforce_file_size: true,
            max_file_mb: 3.0,
            min_quality: 65,
            enforce_min_side: false,
            min_side: 500,
        }
    }
}

impl Settings {
    pub fn limits(&self) -> ExportLimits {
        ExportLimits {
            max_width: self.enforce_dimensions.then_some(self.max_width.max(16)),
            max_height: self.enforce_dimensions.then_some(self.max_height.max(16)),
            max_bytes: self.enforce_file_size.then(|| (self.max_file_mb.max(0.05) as f64 * MIB as f64) as u64),
            min_side: self.enforce_min_side.then_some(self.min_side.max(1)),
            min_quality: self.min_quality.clamp(1, 100),
        }
    }

    pub fn appearance(&self) -> Appearance {
        let gray = |v: u8| Some(egui::Color32::from_gray(v));
        let canvas = match self.canvas_color {
            CanvasColor::Theme => None,
            CanvasColor::Black => gray(0),
            CanvasColor::DarkGray => gray(0x28),
            CanvasColor::MediumGray => gray(0x50),
            CanvasColor::LightGray => gray(0xA0),
            CanvasColor::White => gray(0xFF),
            CanvasColor::Custom => {
                let [r, g, b] = self.canvas_custom;
                Some(egui::Color32::from_rgb(r, g, b))
            }
        };
        Appearance { kind: self.theme, canvas, show_tooltips: self.show_tooltips, animations: self.animations }
    }

    /// Następna/poprzednia skala z listy (przyciski −/+).
    pub fn step_ui_scale(&mut self, up: bool) {
        let cur = self.ui_scale;
        self.ui_scale = if up {
            UI_SCALES.iter().copied().find(|&s| s > cur).unwrap_or(*UI_SCALES.last().unwrap_or(&cur))
        } else {
            UI_SCALES.iter().rev().copied().find(|&s| s < cur).unwrap_or(UI_SCALES[0])
        };
    }

    /// Przywraca domyślne wartości jednej sekcji Preferencji.
    pub fn reset_section(&mut self, section: PrefsSection) {
        let d = Settings::default();
        match section {
            PrefsSection::Interface => {
                self.theme = d.theme;
                self.canvas_color = d.canvas_color;
                self.canvas_custom = d.canvas_custom;
                self.ui_scale = d.ui_scale;
                self.show_tooltips = d.show_tooltips;
                self.animations = d.animations;
                self.system_title_bar = d.system_title_bar;
            }
            PrefsSection::Export => {
                self.enforce_dimensions = d.enforce_dimensions;
                self.max_width = d.max_width;
                self.max_height = d.max_height;
                self.enforce_file_size = d.enforce_file_size;
                self.max_file_mb = d.max_file_mb;
                self.min_quality = d.min_quality;
                self.enforce_min_side = d.enforce_min_side;
                self.min_side = d.min_side;
                self.out_subfolder = d.out_subfolder;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PrefsSection {
    #[default]
    Interface,
    Export,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domyslne_limity() {
        let l = Settings::default().limits();
        assert_eq!((l.max_width, l.max_height), (Some(3000), Some(3600)));
        assert_eq!(l.max_bytes, Some(3 * MIB));
        assert_eq!(l.min_side, None);
    }

    #[test]
    fn skala_interfejsu_krokami() {
        let mut s = Settings::default();
        s.step_ui_scale(true);
        assert_eq!(s.ui_scale, 110);
        s.ui_scale = 75;
        s.step_ui_scale(false);
        assert_eq!(s.ui_scale, 75);
        s.ui_scale = 105;
        s.step_ui_scale(false);
        assert_eq!(s.ui_scale, 100);
    }

    #[test]
    fn stare_ustawienia_bez_nowych_pol() {
        // Brakujące pola biorą wartości domyślne (#[serde(default)]).
        let s: Settings = serde_json_like("{\"quality\":77}");
        assert_eq!(s.quality, 77);
        assert_eq!(s.max_width, 3000);
    }

    fn serde_json_like(text: &str) -> Settings {
        // Ręczna mapa klucz → u8 podana do serde (jak zapisane wcześniej ustawienia z mniejszą liczbą pól).
        let map: std::collections::BTreeMap<String, u8> = text
            .trim_matches(|c| c == '{' || c == '}')
            .split(',')
            .filter_map(|kv| kv.split_once(':'))
            .map(|(k, v)| (k.trim_matches('"').to_string(), v.parse().unwrap()))
            .collect();
        Settings::deserialize(serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(map.into_iter())).unwrap()
    }
}
