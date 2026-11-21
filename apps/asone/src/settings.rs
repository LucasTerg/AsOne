//! Ustawienia zapamiętywane między uruchomieniami (Ustawienia › Preferencje).

use asone_core::finish::{ExportLimits, MIB};
use serde::{Deserialize, Serialize};

use crate::theme::{Appearance, ThemeKind};

pub const STORAGE_KEY: &str = "asone-settings";

/// Dostępne skale interfejsu (%), przełączane przyciskami −/+ na pasku tytułu.
pub const UI_SCALES: [u32; 9] = [75, 85, 100, 110, 125, 150, 175, 200, 250];

/// Widok listy plików.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListView {
    /// Siatka miniatur (rozmiar suwakiem, nazwy opcjonalnie).
    #[default]
    Thumbnails,
    /// Wiersze: miniatura + pełna, zawijana nazwa.
    List,
    /// Tabela: nazwa, wymiary, rozmiar, typ.
    Details,
}

/// Ustawienia listy plików.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ListOptions {
    pub view: ListView,
    /// Bok miniatury w siatce (w liście – połowa).
    pub thumb_size: f32,
    /// Nazwy pod miniaturami.
    pub show_names: bool,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self { view: ListView::Thumbnails, thumb_size: 120.0, show_names: true }
    }
}

/// Co pokazuje zakładka zdjęcia.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabTitle {
    /// `#3 · nazwa-pliku.jpg`
    #[default]
    Full,
    /// Tylko numer kolejności: `#3` (pełna nazwa w podpowiedzi).
    NumberOnly,
}

/// Klucz API do pokazania: początek i koniec, np. `r8_A…x9Kq` (całości nie widać).
pub fn masked_key(key: &str) -> String {
    let key = key.trim();
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "•".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// Retusz LaMa (Big-LaMa w ONNX, Apache-2.0): plik modelu.
pub const LAMA_URL: &str = "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx";
pub const LAMA_PAGE: &str = "https://huggingface.co/Carve/LaMa-ONNX";
pub const LAMA_FILE: &str = "lama-fp32.onnx";

/// Plik modelu do pobrania w Preferencjach: usuwanie tła (RMBG) albo retusz (LaMa).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelFile {
    Rmbg(AiModel),
    Lama,
}

impl ModelFile {
    pub fn label(self) -> &'static str {
        match self {
            ModelFile::Rmbg(m) => m.label(),
            ModelFile::Lama => "LaMa (Big-LaMa, ~208 MB)",
        }
    }

    pub fn download_url(self) -> &'static str {
        match self {
            ModelFile::Rmbg(m) => m.download_url(),
            ModelFile::Lama => LAMA_URL,
        }
    }

    pub fn page_url(self) -> &'static str {
        match self {
            ModelFile::Rmbg(m) => m.page_url(),
            ModelFile::Lama => LAMA_PAGE,
        }
    }
}

/// Model edycji zdjęcia poleceniem na Replicate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditModel {
    /// FLUX.1 Kontext Pro – 0,04 USD za obraz (cennik Replicate z 20.11.2026).
    #[default]
    KontextPro,
    /// FLUX.1 Kontext Max – dokładniejszy, 0,08 USD za obraz.
    KontextMax,
}

impl EditModel {
    pub const ALL: [EditModel; 2] = [EditModel::KontextPro, EditModel::KontextMax];

    /// Identyfikator modelu na Replicate.
    pub fn id(self) -> &'static str {
        match self {
            EditModel::KontextPro => "black-forest-labs/flux-kontext-pro",
            EditModel::KontextMax => "black-forest-labs/flux-kontext-max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EditModel::KontextPro => "FLUX.1 Kontext Pro",
            EditModel::KontextMax => t!("FLUX.1 Kontext Max (dokładniejszy)"),
        }
    }

    /// Cena jednego obrazu (cennik Replicate z 20.11.2026).
    pub fn price(self) -> &'static str {
        match self {
            EditModel::KontextPro => "0,04 $",
            EditModel::KontextMax => "0,08 $",
        }
    }

    pub fn page_url(self) -> String {
        format!("https://replicate.com/{}", self.id())
    }
}

/// Gotowe polecenia edycji AI: (etykieta w bieżącym języku, polecenie dla modelu – po angielsku,
/// tak rozumie najlepiej).
pub fn ai_presets() -> [(&'static str, &'static str); 5] {
    [
        (
            t!("Białe tło i delikatny cień"),
            "Place the product on a pure white background with a soft, natural shadow under it. \
             Keep the product exactly the same: shape, colors, text and logos.",
        ),
        (
            t!("Usuń napisy i znaki wodne"),
            "Remove all watermarks, captions and overlaid text that are not printed on the product. \
             Keep everything else exactly the same.",
        ),
        (
            t!("Popraw światło i kolory"),
            "Improve the lighting to bright, even studio light with natural colors and crisp details. \
             Do not change the product's shape, text or logos.",
        ),
        (
            t!("Usuń kurz i rysy"),
            "Remove dust, scratches and small blemishes from the product surface. Keep everything else exactly the same.",
        ),
        (
            t!("Produkt we wnętrzu"),
            "Place the product in a bright, modern home interior with realistic lighting and shadows. \
             Keep the product exactly the same, including text and logos.",
        ),
    ]
}

/// Wygląd paska menu (tytułowego).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarStyle {
    /// Płaski pasek oddzielony od reszty cienką ciemną linią – delikatny.
    #[default]
    Flat,
    /// Połysk: błysk w górnej połowie, ku dołowi ciemniej.
    Gloss,
    /// Bez efektu (jednolity, bez linii).
    Plain,
}

impl BarStyle {
    pub const ALL: [BarStyle; 3] = [BarStyle::Flat, BarStyle::Gloss, BarStyle::Plain];

    pub fn label(self) -> &'static str {
        match self {
            BarStyle::Flat => t!("Płaski z linią (delikatny)"),
            BarStyle::Gloss => t!("Połysk"),
            BarStyle::Plain => t!("Bez efektu"),
        }
    }
}

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
            CanvasColor::Theme => t!("Z motywu"),
            CanvasColor::Black => t!("Czarny"),
            CanvasColor::DarkGray => t!("Ciemnoszary"),
            CanvasColor::MediumGray => t!("Szary"),
            CanvasColor::LightGray => t!("Jasnoszary"),
            CanvasColor::White => t!("Biały"),
            CanvasColor::Custom => t!("Własny"),
        }
    }
}

/// Model usuwania tła.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiModel {
    /// BRIA RMBG-1.4 – publiczny, bez rejestracji.
    #[default]
    Rmbg14,
    /// BRIA RMBG-2.0 – dokładniejszy; repozytorium wymaga tokenu Hugging Face i akceptacji licencji.
    Rmbg20,
    /// BRIA RMBG-3.0 online przez fal.ai (`bria/rmbg-3`) – zdjęcie jest wysyłane, płatne za zdjęcie.
    Rmbg30Fal,
}

impl AiModel {
    pub const ALL: [AiModel; 3] = [AiModel::Rmbg14, AiModel::Rmbg20, AiModel::Rmbg30Fal];

    pub fn label(self) -> &'static str {
        match self {
            AiModel::Rmbg14 => "RMBG-1.4 (BRIA, ~176 MB)",
            AiModel::Rmbg20 => "RMBG-2.0 (BRIA, ~1 GB)",
            AiModel::Rmbg30Fal => "RMBG-3.0 online (fal.ai)",
        }
    }

    /// Model działa na serwerze (zdjęcie wysyłane przez internet), nie lokalnie.
    pub fn is_online(self) -> bool {
        self == AiModel::Rmbg30Fal
    }

    /// Strona modelu (licencja, akceptacja warunków, cena).
    pub fn page_url(self) -> &'static str {
        match self {
            AiModel::Rmbg14 => "https://huggingface.co/briaai/RMBG-1.4",
            AiModel::Rmbg20 => "https://huggingface.co/briaai/RMBG-2.0",
            AiModel::Rmbg30Fal => "https://fal.ai/models/bria/rmbg-3",
        }
    }

    /// Bezpośredni link do pliku ONNX (pusty dla modelu online).
    pub fn download_url(self) -> &'static str {
        match self {
            AiModel::Rmbg14 => "https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx",
            AiModel::Rmbg20 => "https://huggingface.co/briaai/RMBG-2.0/resolve/main/onnx/model.onnx",
            AiModel::Rmbg30Fal => "",
        }
    }

    pub fn needs_token(self) -> bool {
        self == AiModel::Rmbg20
    }

    /// Nazwa pliku ONNX (pusta dla modelu online).
    pub fn file_name(self) -> &'static str {
        match self {
            AiModel::Rmbg14 => "rmbg-1.4.onnx",
            AiModel::Rmbg20 => "rmbg-2.0.onnx",
            AiModel::Rmbg30Fal => "",
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
    /// Język interfejsu (domyślnie jak w systemie).
    pub language: crate::i18n::Language,
    /// Wygląd paska menu.
    pub bar_style: BarStyle,
    /// AppImage: sprawdzanie nowej wersji przy starcie (GitHub).
    pub check_updates: bool,
    /// Ostatnio otwierane foldery (ekran startowy), najnowsze pierwsze.
    pub recent_folders: Vec<String>,

    // --- Replicate (edycja AI online, płatna) ---
    /// Jeden przełącznik dla wszystkich funkcji Replicate (np. gdy brak środków na koncie).
    pub replicate_enabled: bool,
    /// Klucz API Replicate (`r8_…`, zapisywany lokalnie). Pusty = zmienna `REPLICATE_API_TOKEN`
    /// albo plik `~/.config/asone/replicate-token`.
    pub replicate_key: String,
    pub replicate_model: EditModel,

    // --- Retusz LaMa (lokalnie) ---
    /// Retusz LaMa włączony (wyłączony – „nie używam”: wszystkie opcje LaMa znikają).
    pub lama_enabled: bool,
    /// Ścieżka pliku modelu LaMa (pusta = domyślna w katalogu danych AsOne).
    pub lama_path: String,

    // --- Powiększanie waifu2x (lokalnie) ---
    /// waifu2x włączony (wyłączony – „nie używam”: wszystkie opcje waifu2x znikają).
    pub waifu2x_enabled: bool,
    pub waifu2x_style: crate::waifu2x::Style,
    /// Odszumianie: `None` – bez, 0–3 – coraz mocniejsze.
    pub waifu2x_noise: Option<u8>,
    /// Skala: 2 albo 4.
    pub waifu2x_scale: u32,
    pub animations: bool,
    /// Systemowy pasek tytułu zamiast własnego (Linux/Windows).
    pub system_title_bar: bool,
    /// Pasek zakładek zdjęć pod zdjęciem zamiast nad nim.
    pub photo_tabs_bottom: bool,
    /// Pełna nazwa na zakładce albo tylko numer `#n`.
    pub tab_title: TabTitle,
    /// Lista plików: widok, rozmiar miniatur, nazwy.
    pub list: ListOptions,

    // --- Ustawienia › Ukryj ---
    pub hide_gimp: bool,
    pub hide_delete_from_disk: bool,
    pub hide_inspection: bool,
    pub hide_mask_toggle: bool,

    // --- AI (usuwanie tła) ---
    pub ai_model: AiModel,
    /// Ścieżka pliku RMBG-1.4 (pusta = domyślna w katalogu danych AsOne).
    pub ai_path_14: String,
    /// Ścieżka pliku RMBG-2.0 (pusta = domyślna w katalogu danych AsOne).
    pub ai_path_20: String,
    /// Token Hugging Face (tylko do pobrania RMBG-2.0; zapisywany lokalnie).
    pub hf_token: String,
    /// Klucz API fal.ai (RMBG-3.0 online; zapisywany lokalnie). Pusty = zmienna środowiskowa `FAL_KEY`.
    pub fal_key: String,

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
    pub min_side: u32,
    /// Przy zmniejszaniu dużych zdjęć ramka / marginesy do 100 px zostają w pełnych pikselach.
    pub keep_frame: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeKind::System,
            canvas_color: CanvasColor::Theme,
            canvas_custom: [0x28, 0x28, 0x28],
            ui_scale: 100,
            show_tooltips: true,
            language: crate::i18n::Language::Auto,
            bar_style: BarStyle::Flat,
            check_updates: true,
            recent_folders: Vec::new(),
            replicate_enabled: true,
            replicate_key: String::new(),
            replicate_model: EditModel::KontextPro,
            lama_enabled: true,
            lama_path: String::new(),
            waifu2x_enabled: true,
            waifu2x_style: crate::waifu2x::Style::Photo,
            waifu2x_noise: Some(1),
            waifu2x_scale: 2,
            animations: true,
            system_title_bar: false,
            photo_tabs_bottom: false,
            tab_title: TabTitle::Full,
            list: ListOptions::default(),
            hide_gimp: false,
            hide_delete_from_disk: false,
            hide_inspection: false,
            hide_mask_toggle: false,
            ai_model: AiModel::Rmbg14,
            ai_path_14: String::new(),
            ai_path_20: String::new(),
            hf_token: String::new(),
            fal_key: String::new(),
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
            min_side: 500,
            keep_frame: true,
        }
    }
}

/// Elementy, które można ukryć (Ustawienia › Ukryj).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HideItem {
    Gimp,
    DeleteFromDisk,
    Inspection,
    MaskToggle,
}

impl HideItem {
    pub const ALL: [HideItem; 4] = [HideItem::Gimp, HideItem::DeleteFromDisk, HideItem::Inspection, HideItem::MaskToggle];

    pub fn label(self) -> &'static str {
        match self {
            HideItem::Gimp => t!("Edytuj w GIMP"),
            HideItem::DeleteFromDisk => t!("Usuń z dysku"),
            HideItem::Inspection => t!("Inspekcja tła"),
            HideItem::MaskToggle => t!("Maska aktywna (usunięte tło, PNG)"),
        }
    }
}

impl Settings {
    /// Ile folderów pamięta ekran startowy.
    pub const MAX_RECENT: usize = 12;

    /// Dopisuje foldery na początek „Ostatnich” (bez powtórzeń, najwyżej [`Self::MAX_RECENT`]).
    pub fn remember_folders(&mut self, dirs: &[String]) {
        for dir in dirs.iter().rev() {
            self.recent_folders.retain(|d| d != dir);
            self.recent_folders.insert(0, dir.clone());
        }
        self.recent_folders.truncate(Self::MAX_RECENT);
    }

    pub fn hidden_mut(&mut self, item: HideItem) -> &mut bool {
        match item {
            HideItem::Gimp => &mut self.hide_gimp,
            HideItem::DeleteFromDisk => &mut self.hide_delete_from_disk,
            HideItem::Inspection => &mut self.hide_inspection,
            HideItem::MaskToggle => &mut self.hide_mask_toggle,
        }
    }

    pub fn hidden(&self, item: HideItem) -> bool {
        match item {
            HideItem::Gimp => self.hide_gimp,
            HideItem::DeleteFromDisk => self.hide_delete_from_disk,
            HideItem::Inspection => self.hide_inspection,
            HideItem::MaskToggle => self.hide_mask_toggle,
        }
    }

    /// Nazwa domyślnego podfolderu zapisu (obok zdjęć); pusta lub same znaki `/` `\\` → „web”.
    pub fn out_subfolder_name(&self) -> String {
        let name = self.out_subfolder.trim().trim_matches(['/', '\\']).trim();
        if name.is_empty() { "web".into() } else { name.to_string() }
    }

    /// waifu2x widoczny: włączony i ta wersja uruchamia modele lokalnie.
    pub fn waifu2x_active(&self) -> bool {
        self.waifu2x_enabled && crate::ai::AVAILABLE
    }

    /// Wybrany wariant waifu2x (skala tylko 2× albo 4×).
    pub fn waifu2x_variant(&self) -> crate::waifu2x::Variant {
        crate::waifu2x::Variant {
            style: self.waifu2x_style,
            noise: self.waifu2x_noise.map(|n| n.min(3)),
            scale: if self.waifu2x_scale >= 4 { 4 } else { 2 },
        }
    }

    /// Retusz LaMa widoczny: włączony i ta wersja uruchamia modele lokalnie.
    pub fn lama_active(&self) -> bool {
        self.lama_enabled && crate::ai::AVAILABLE
    }

    /// Plik modelu LaMa: wpisany w Ustawieniach (jeśli istnieje) albo domyślny w katalogu danych.
    pub fn lama_model_path(&self) -> std::path::PathBuf {
        let custom = std::path::PathBuf::from(self.lama_path.trim());
        if !self.lama_path.trim().is_empty() && custom.is_file() {
            return custom;
        }
        default_models_dir().join(LAMA_FILE)
    }

    /// Klucz Replicate: wpisany w Ustawieniach, zmienna `REPLICATE_API_TOKEN` albo plik
    /// `~/.config/asone/replicate-token` (desktop).
    pub fn replicate_api_key(&self) -> String {
        let key = self.replicate_key.trim();
        if !key.is_empty() {
            return key.to_string();
        }
        if let Ok(k) = std::env::var("REPLICATE_API_TOKEN")
            && !k.trim().is_empty()
        {
            return k.trim().to_string();
        }
        // Plik czytany raz na uruchomienie (funkcja jest wołana co klatkę).
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            static FILE_KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            FILE_KEY
                .get_or_init(|| {
                    std::env::var_os("HOME")
                        .or_else(|| std::env::var_os("USERPROFILE"))
                        .and_then(|home| std::fs::read_to_string(std::path::PathBuf::from(home).join(".config/asone/replicate-token")).ok())
                        .map(|k| k.trim().to_string())
                        .unwrap_or_default()
                })
                .clone()
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        String::new()
    }

    /// Funkcje Replicate widoczne: włączone, ta wersja je obsługuje i jest klucz.
    pub fn replicate_active(&self) -> bool {
        self.replicate_enabled && crate::ai::REPLICATE_AVAILABLE && !self.replicate_api_key().is_empty()
    }

    /// Klucz fal.ai: wpisany w Ustawieniach albo ze zmiennej środowiskowej `FAL_KEY`.
    pub fn fal_api_key(&self) -> String {
        let key = self.fal_key.trim();
        if key.is_empty() { std::env::var("FAL_KEY").unwrap_or_default().trim().to_string() } else { key.to_string() }
    }

    /// Ścieżka pliku modelu: wpisana w Ustawieniach albo domyślna w katalogu danych AsOne.
    pub fn ai_model_path(&self) -> std::path::PathBuf {
        let custom = match self.ai_model {
            AiModel::Rmbg14 => self.ai_path_14.trim(),
            AiModel::Rmbg20 => self.ai_path_20.trim(),
            AiModel::Rmbg30Fal => return std::path::PathBuf::new(),
        };
        if !custom.is_empty() {
            let p = std::path::PathBuf::from(custom);
            if p.is_file() {
                return p;
            }
        }
        let fname = self.ai_model.file_name();
        let default = default_models_dir().join(fname);
        if default.is_file() {
            return default;
        }
        // Sprawdź inne standardowe lokalizacje (np. uruchomienie w Flatpaku, dev lub home)
        if let Some(user_home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
            for sub in [
                ".local/share/asone/models",
                ".local/share/io.github.lucasterg.asone/models",
                ".cache/asone/models",
            ] {
                let candidate = user_home.join(sub).join(fname);
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
        default
    }

    pub fn limits(&self) -> ExportLimits {
        ExportLimits {
            max_width: self.enforce_dimensions.then_some(self.max_width.max(16)),
            max_height: self.enforce_dimensions.then_some(self.max_height.max(16)),
            max_bytes: self.enforce_file_size.then(|| (self.max_file_mb.max(0.05) as f64 * MIB as f64) as u64),
            // Zawsze: zapisane zdjęcie ma co najmniej 500 px na każdym boku (dopełnienie białym tłem).
            min_side: Some(self.min_side.max(asone_core::finish::MIN_SIDE)),
            min_quality: self.min_quality.clamp(1, 100),
            keep_frame: self.keep_frame,
        }
    }

    /// `system_dark` – system prosi o ciemny wygląd (motyw „Jak w systemie”).
    pub fn appearance(&self, system_dark: bool) -> Appearance {
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
        Appearance { kind: self.theme, system_dark, canvas, show_tooltips: self.show_tooltips, animations: self.animations }
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
                self.language = d.language;
                self.bar_style = d.bar_style;
                self.check_updates = d.check_updates;
                self.animations = d.animations;
                self.system_title_bar = d.system_title_bar;
                self.photo_tabs_bottom = d.photo_tabs_bottom;
                self.tab_title = d.tab_title;
                self.hide_gimp = d.hide_gimp;
                self.hide_delete_from_disk = d.hide_delete_from_disk;
                self.hide_inspection = d.hide_inspection;
                self.hide_mask_toggle = d.hide_mask_toggle;
            }
            PrefsSection::Ai => {
                self.ai_model = d.ai_model;
                self.ai_path_14 = d.ai_path_14;
                self.ai_path_20 = d.ai_path_20;
            }
            PrefsSection::Retouch => {
                self.lama_enabled = d.lama_enabled;
                self.lama_path = d.lama_path;
            }
            PrefsSection::Upscale => {
                self.waifu2x_enabled = d.waifu2x_enabled;
                self.waifu2x_style = d.waifu2x_style;
                self.waifu2x_noise = d.waifu2x_noise;
                self.waifu2x_scale = d.waifu2x_scale;
            }
            PrefsSection::Replicate => {
                // Klucz zostaje – „Przywróć domyślne” nie kasuje danych logowania.
                self.replicate_enabled = d.replicate_enabled;
                self.replicate_model = d.replicate_model;
            }
            PrefsSection::Export => {
                self.enforce_dimensions = d.enforce_dimensions;
                self.max_width = d.max_width;
                self.max_height = d.max_height;
                self.enforce_file_size = d.enforce_file_size;
                self.max_file_mb = d.max_file_mb;
                self.min_quality = d.min_quality;
                self.min_side = d.min_side;
                self.keep_frame = d.keep_frame;
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
    Ai,
    Retouch,
    Upscale,
    Replicate,
}

/// Katalog modeli AI w danych aplikacji (np. `~/.local/share/asone/models`).
pub fn default_models_dir() -> std::path::PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        eframe::storage_dir("asone").unwrap_or_else(|| std::path::PathBuf::from(".")).join("models")
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::path::PathBuf::from("models")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domyslne_limity() {
        let l = Settings::default().limits();
        assert_eq!((l.max_width, l.max_height), (Some(3000), Some(3600)));
        assert_eq!(l.max_bytes, Some(3 * MIB));
        assert_eq!(l.min_side, Some(500));
        // Niższej wartości z ustawień nie da się wymusić.
        let low = Settings { min_side: 100, ..Settings::default() };
        assert_eq!(low.limits().min_side, Some(500));
    }

    #[test]
    fn klucz_zamaskowany() {
        assert_eq!(masked_key("r8_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ab"), "r8_A…89ab");
        assert_eq!(masked_key("krotki"), "••••••");
    }

    #[test]
    fn ostatnie_foldery() {
        let mut s = Settings::default();
        s.remember_folders(&["/a".into(), "/b".into()]);
        s.remember_folders(&["/c".into(), "/a".into()]);
        assert_eq!(s.recent_folders, ["/c", "/a", "/b"]);
        let many: Vec<String> = (0..20).map(|i| format!("/d{i}")).collect();
        s.remember_folders(&many);
        assert_eq!(s.recent_folders.len(), Settings::MAX_RECENT);
        assert_eq!(s.recent_folders[0], "/d0");
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
