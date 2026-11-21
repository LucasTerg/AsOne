//! Motywy interfejsu: domyślny AsOne, dodatkowe motywy (Preferencje › Interfejs) i Sepia.
//!
//! Aktualna paleta jest globalna ([`p`]) – korzystają z niej panele, płótno i shader.

use std::sync::RwLock;

use egui::{Color32, CornerRadius, Stroke, Theme};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeKind {
    /// Jasny albo ciemny – jak ustawiono w systemie (domyślny).
    #[default]
    System,
    AsOne,
    Pro,
    ProMedium,
    ProLightGray,
    ProLight,
    Studio,
    StudioLight,
    Classic,
    Adwaita,
    AdwaitaDark,
    SolarizedDark,
    BreezeLight,
    BreezeDark,
    SoftDark,
    SoftLight,
    Sepia,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 17] = [
        ThemeKind::System,
        ThemeKind::AsOne,
        ThemeKind::Pro,
        ThemeKind::ProMedium,
        ThemeKind::ProLightGray,
        ThemeKind::ProLight,
        ThemeKind::Studio,
        ThemeKind::StudioLight,
        ThemeKind::Classic,
        ThemeKind::Adwaita,
        ThemeKind::AdwaitaDark,
        ThemeKind::SolarizedDark,
        ThemeKind::BreezeLight,
        ThemeKind::BreezeDark,
        ThemeKind::SoftDark,
        ThemeKind::SoftLight,
        ThemeKind::Sepia,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::System => t!("Jak w systemie (jasny / ciemny)"),
            ThemeKind::AsOne => t!("AsOne (ciemny)"),
            ThemeKind::Pro => t!("Pro (ciemny)"),
            ThemeKind::ProMedium => t!("Pro (średni szary)"),
            ThemeKind::ProLightGray => t!("Pro (jasnoszary)"),
            ThemeKind::ProLight => t!("Pro (jasny)"),
            ThemeKind::Studio => t!("Studio (ciemny)"),
            ThemeKind::StudioLight => t!("Studio (jasny)"),
            ThemeKind::Classic => t!("Klasyczny"),
            ThemeKind::Adwaita => t!("Adwaita (jasny)"),
            ThemeKind::AdwaitaDark => t!("Adwaita (ciemny)"),
            ThemeKind::SolarizedDark => "Solarized Dark",
            ThemeKind::BreezeLight => t!("Breeze (jasny)"),
            ThemeKind::BreezeDark => t!("Breeze (ciemny)"),
            ThemeKind::SoftDark => t!("Soft (ciemny)"),
            ThemeKind::SoftLight => t!("Soft (jasny)"),
            ThemeKind::Sepia => "Sepia",
        }
    }
}

/// Kolory jednego motywu (podzbiór tokenów motywu).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Paski: tytułowy, górny, narzędzi, stanu.
    pub chrome: Color32,
    /// Tło wokół zdjęcia na płótnie.
    pub canvas: Color32,
    pub dock: Color32,
    /// Panele (inspektor, lista plików).
    pub card: Color32,
    pub card_border: Color32,
    pub field: Color32,
    pub field_border: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub icon: Color32,
    pub accent: Color32,
    /// Napisy i ikony na tle w kolorze akcentu (główne przyciski, zaznaczenie, numery) – zawsze czytelne.
    pub on_accent: Color32,
    pub separator: Color32,
    pub tab_strip: Color32,
    pub row_selected: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub radius: u8,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Domyślny wygląd AsOne (paleta z promptu: #1E1E1E / #262626 / #383838 / #2680EB).
const ASONE: Palette = Palette {
    chrome: rgb(0x26, 0x26, 0x26),
    canvas: rgb(0x1E, 0x1E, 0x1E),
    dock: rgb(0x1E, 0x1E, 0x1E),
    card: rgb(0x26, 0x26, 0x26),
    card_border: rgb(0x1A, 0x1A, 0x1A),
    field: rgb(0x38, 0x38, 0x38),
    field_border: rgb(0x46, 0x46, 0x46),
    hover: rgb(0x42, 0x42, 0x42),
    pressed: rgb(0x4A, 0x4A, 0x4A),
    text: rgb(0xD6, 0xD6, 0xD6),
    text_dim: rgb(0x96, 0x96, 0x96),
    text_faint: rgb(0x70, 0x70, 0x70),
    icon: rgb(0xC8, 0xC8, 0xC8),
    accent: rgb(0x26, 0x80, 0xEB),
    on_accent: rgb(255, 255, 255),
    separator: rgb(0x1A, 0x1A, 0x1A),
    tab_strip: rgb(0x2B, 0x2B, 0x2B),
    row_selected: rgb(0x3A, 0x3A, 0x3A),
    danger: rgb(0xF0, 0x6A, 0x5A),
    warning: rgb(0xE8, 0xB0, 0x46),
    radius: 3,
};

/// Sepia: ciepły, jasny motyw do długiej pracy (kremowe panele, brązowy tekst, bursztynowy akcent).
const SEPIA: Palette = Palette {
    chrome: rgb(232, 220, 196),
    canvas: rgb(205, 192, 166),
    dock: rgb(222, 208, 180),
    card: rgb(240, 230, 210),
    card_border: rgb(196, 178, 146),
    field: rgb(250, 243, 228),
    field_border: rgb(180, 160, 125),
    hover: rgb(228, 214, 186),
    pressed: rgb(214, 196, 162),
    text: rgb(67, 52, 34),
    text_dim: rgb(105, 86, 62),
    text_faint: rgb(150, 130, 100),
    icon: rgb(90, 70, 45),
    accent: rgb(160, 98, 40),
    on_accent: rgb(255, 255, 255),
    separator: rgb(196, 178, 146),
    tab_strip: rgb(226, 212, 184),
    row_selected: rgb(218, 196, 152),
    danger: rgb(178, 58, 48),
    warning: rgb(190, 120, 30),
    radius: 4,
};

impl Palette {
    /// Kolory wygenerowane z tokenów motywów (`Tokens::for_kind`, z rozwiniętym dziedziczeniem).
    pub fn for_kind(kind: ThemeKind) -> Palette {
        match kind {
            // Rozstrzygany wcześniej (`ThemeKind::resolve`); tu – ciemny jak dotąd.
            ThemeKind::System | ThemeKind::AsOne => ASONE,
            ThemeKind::Sepia => SEPIA,
        ThemeKind::Pro => Palette {
            chrome: rgb(50, 50, 50),
            canvas: rgb(40, 40, 40),
            dock: rgb(30, 30, 30),
            card: rgb(50, 50, 50),
            card_border: rgb(30, 30, 30),
            field: rgb(36, 36, 36),
            field_border: rgb(74, 74, 74),
            hover: rgb(66, 66, 66),
            pressed: rgb(78, 78, 78),
            text: rgb(222, 222, 222),
            text_dim: rgb(178, 178, 178),
            text_faint: rgb(128, 128, 128),
            icon: rgb(200, 200, 200),
            accent: rgb(55, 142, 240),
            on_accent: rgb(255, 255, 255),
            separator: rgb(30, 30, 30),
            tab_strip: rgb(38, 38, 38),
            row_selected: rgb(82, 82, 82),
            danger: rgb(236, 91, 98),
            warning: rgb(232, 176, 70),
            radius: 4,
        },
        ThemeKind::ProMedium => Palette {
            chrome: rgb(83, 83, 83),
            canvas: rgb(40, 40, 40),
            dock: rgb(66, 66, 66),
            card: rgb(83, 83, 83),
            card_border: rgb(66, 66, 66),
            field: rgb(69, 69, 69),
            field_border: rgb(104, 104, 104),
            hover: rgb(98, 98, 98),
            pressed: rgb(112, 112, 112),
            text: rgb(238, 238, 238),
            text_dim: rgb(212, 212, 212),
            text_faint: rgb(160, 160, 160),
            icon: rgb(226, 226, 226),
            accent: rgb(55, 142, 240),
            on_accent: rgb(255, 255, 255),
            separator: rgb(62, 62, 62),
            tab_strip: rgb(74, 74, 74),
            row_selected: rgb(107, 107, 107),
            danger: rgb(236, 91, 98),
            warning: rgb(232, 176, 70),
            radius: 4,
        },
        ThemeKind::ProLightGray => Palette {
            chrome: rgb(184, 184, 184),
            canvas: rgb(160, 160, 160),
            dock: rgb(168, 168, 168),
            card: rgb(184, 184, 184),
            card_border: rgb(160, 160, 160),
            field: rgb(214, 214, 214),
            field_border: rgb(140, 140, 140),
            hover: rgb(198, 198, 198),
            pressed: rgb(170, 170, 170),
            text: rgb(20, 20, 20),
            text_dim: rgb(52, 52, 52),
            text_faint: rgb(96, 96, 96),
            icon: rgb(38, 38, 38),
            accent: rgb(55, 142, 240),
            on_accent: rgb(255, 255, 255),
            separator: rgb(158, 158, 158),
            tab_strip: rgb(170, 170, 170),
            row_selected: rgb(212, 212, 212),
            danger: rgb(236, 91, 98),
            warning: rgb(232, 176, 70),
            radius: 4,
        },
        ThemeKind::ProLight => Palette {
            chrome: rgb(240, 240, 240),
            canvas: rgb(214, 214, 214),
            dock: rgb(226, 226, 226),
            card: rgb(240, 240, 240),
            card_border: rgb(214, 214, 214),
            field: rgb(255, 255, 255),
            field_border: rgb(186, 186, 186),
            hover: rgb(226, 226, 226),
            pressed: rgb(206, 206, 206),
            text: rgb(20, 20, 20),
            text_dim: rgb(64, 64, 64),
            text_faint: rgb(120, 120, 120),
            icon: rgb(46, 46, 46),
            accent: rgb(55, 142, 240),
            on_accent: rgb(255, 255, 255),
            separator: rgb(214, 214, 214),
            tab_strip: rgb(226, 226, 226),
            row_selected: rgb(204, 204, 204),
            danger: rgb(236, 91, 98),
            warning: rgb(232, 176, 70),
            radius: 4,
        },
        ThemeKind::Studio => Palette {
            chrome: rgb(20, 20, 21),
            canvas: rgb(14, 14, 15),
            dock: rgb(17, 17, 18),
            card: rgb(26, 26, 28),
            card_border: rgb(40, 40, 44),
            field: rgb(35, 35, 38),
            field_border: rgb(52, 52, 57),
            hover: rgb(44, 44, 48),
            pressed: rgb(56, 56, 62),
            text: rgb(236, 236, 240),
            text_dim: rgb(150, 150, 158),
            text_faint: rgb(96, 96, 104),
            icon: rgb(196, 196, 204),
            accent: rgb(139, 124, 246),
            on_accent: rgb(255, 255, 255),
            separator: rgb(38, 38, 42),
            tab_strip: rgb(17, 17, 18),
            row_selected: rgb(56, 56, 62),
            danger: rgb(240, 96, 96),
            warning: rgb(240, 190, 90),
            radius: 8,
        },
        ThemeKind::StudioLight => Palette {
            chrome: rgb(246, 246, 248),
            canvas: rgb(226, 226, 230),
            dock: rgb(240, 240, 243),
            card: rgb(252, 252, 253),
            card_border: rgb(222, 222, 228),
            field: rgb(242, 242, 245),
            field_border: rgb(214, 214, 220),
            hover: rgb(232, 232, 237),
            pressed: rgb(220, 220, 226),
            text: rgb(24, 24, 28),
            text_dim: rgb(96, 96, 106),
            text_faint: rgb(150, 150, 160),
            icon: rgb(60, 60, 68),
            accent: rgb(108, 92, 231),
            on_accent: rgb(255, 255, 255),
            separator: rgb(226, 226, 232),
            tab_strip: rgb(240, 240, 243),
            row_selected: rgb(220, 220, 226),
            danger: rgb(210, 60, 60),
            warning: rgb(190, 130, 20),
            radius: 8,
        },
        ThemeKind::Classic => Palette {
            chrome: rgb(212, 208, 200),
            canvas: rgb(128, 128, 128),
            dock: rgb(212, 208, 200),
            card: rgb(212, 208, 200),
            card_border: rgb(128, 128, 128),
            field: rgb(255, 255, 255),
            field_border: rgb(128, 128, 128),
            hover: rgb(226, 222, 214),
            pressed: rgb(190, 186, 178),
            text: rgb(0, 0, 0),
            text_dim: rgb(64, 64, 64),
            text_faint: rgb(128, 128, 128),
            icon: rgb(0, 0, 0),
            accent: rgb(10, 36, 106),
            on_accent: rgb(255, 255, 255),
            separator: rgb(128, 128, 128),
            tab_strip: rgb(212, 208, 200),
            row_selected: rgb(10, 36, 106),
            danger: rgb(160, 0, 0),
            warning: rgb(128, 96, 0),
            radius: 3,
        },
        ThemeKind::Adwaita => Palette {
            chrome: rgb(255, 255, 255),
            canvas: rgb(222, 222, 223),
            dock: rgb(235, 235, 237),
            card: rgb(255, 255, 255),
            card_border: rgb(218, 218, 221),
            field: rgb(235, 235, 237),
            field_border: rgb(218, 218, 221),
            hover: rgb(226, 226, 229),
            pressed: rgb(212, 212, 216),
            text: rgb(50, 50, 54),
            text_dim: rgb(100, 100, 104),
            text_faint: rgb(155, 155, 160),
            icon: rgb(56, 56, 60),
            accent: rgb(53, 132, 228),
            on_accent: rgb(255, 255, 255),
            separator: rgb(222, 222, 225),
            tab_strip: rgb(235, 235, 237),
            row_selected: rgb(212, 212, 216),
            danger: rgb(192, 28, 40),
            warning: rgb(156, 110, 3),
            radius: 9,
        },
        ThemeKind::AdwaitaDark => Palette {
            chrome: rgb(46, 46, 50),
            canvas: rgb(29, 29, 32),
            dock: rgb(34, 34, 38),
            card: rgb(54, 54, 58),
            card_border: rgb(64, 64, 68),
            field: rgb(64, 64, 68),
            field_border: rgb(76, 76, 80),
            hover: rgb(72, 72, 76),
            pressed: rgb(86, 86, 90),
            text: rgb(255, 255, 255),
            text_dim: rgb(192, 192, 194),
            text_faint: rgb(145, 145, 148),
            icon: rgb(235, 235, 237),
            accent: rgb(53, 132, 228),
            on_accent: rgb(255, 255, 255),
            separator: rgb(24, 24, 27),
            tab_strip: rgb(34, 34, 38),
            row_selected: rgb(86, 86, 90),
            danger: rgb(255, 123, 99),
            warning: rgb(248, 228, 92),
            radius: 9,
        },
        ThemeKind::SolarizedDark => Palette {
            chrome: rgb(7, 54, 66),
            canvas: rgb(0, 43, 54),
            dock: rgb(0, 37, 47),
            card: rgb(7, 54, 66),
            card_border: rgb(0, 37, 47),
            field: rgb(0, 37, 47),
            field_border: rgb(21, 80, 95),
            hover: rgb(14, 74, 90),
            pressed: rgb(18, 89, 107),
            text: rgb(131, 148, 150),
            text_dim: rgb(101, 123, 131),
            text_faint: rgb(88, 110, 117),
            icon: rgb(147, 161, 161),
            accent: rgb(38, 139, 210),
            on_accent: rgb(255, 255, 255),
            separator: rgb(0, 37, 47),
            tab_strip: rgb(0, 37, 47),
            row_selected: rgb(18, 89, 107),
            danger: rgb(220, 50, 47),
            warning: rgb(181, 137, 0),
            radius: 8,
        },
        ThemeKind::BreezeLight => Palette {
            chrome: rgb(219, 222, 226),
            canvas: rgb(219, 222, 226),
            dock: rgb(224, 227, 231),
            card: rgb(236, 238, 241),
            card_border: rgb(203, 208, 213),
            field: rgb(249, 250, 252),
            field_border: rgb(184, 191, 198),
            hover: rgb(226, 230, 235),
            pressed: rgb(208, 214, 221),
            text: rgb(33, 37, 42),
            text_dim: rgb(54, 63, 72),
            text_faint: rgb(81, 90, 100),
            icon: rgb(52, 58, 64),
            accent: rgb(58, 172, 232),
            on_accent: rgb(255, 255, 255),
            separator: rgb(214, 218, 222),
            tab_strip: rgb(224, 227, 231),
            row_selected: rgb(208, 214, 221),
            danger: rgb(170, 50, 64),
            warning: rgb(153, 72, 0),
            radius: 5,
        },
        ThemeKind::BreezeDark => Palette {
            chrome: rgb(38, 42, 48),
            canvas: rgb(17, 20, 24),
            dock: rgb(26, 29, 34),
            card: rgb(29, 33, 38),
            card_border: rgb(52, 56, 61),
            field: rgb(38, 42, 48),
            field_border: rgb(68, 73, 79),
            hover: rgb(50, 54, 59),
            pressed: rgb(58, 63, 69),
            text: rgb(250, 251, 253),
            text_dim: rgb(200, 206, 213),
            text_faint: rgb(164, 170, 177),
            icon: rgb(236, 238, 240),
            accent: rgb(58, 172, 232),
            on_accent: rgb(255, 255, 255),
            separator: rgb(46, 50, 54),
            tab_strip: rgb(26, 29, 34),
            row_selected: rgb(58, 63, 69),
            danger: rgb(227, 111, 123),
            warning: rgb(244, 120, 6),
            radius: 5,
        },
        ThemeKind::SoftDark => Palette {
            chrome: rgb(58, 58, 68),
            canvas: rgb(40, 40, 48),
            dock: rgb(47, 47, 56),
            card: rgb(54, 54, 64),
            card_border: rgb(70, 70, 82),
            field: rgb(64, 64, 75),
            field_border: rgb(84, 84, 98),
            hover: rgb(68, 68, 80),
            pressed: rgb(74, 74, 88),
            text: rgb(238, 238, 246),
            text_dim: rgb(212, 212, 224),
            text_faint: rgb(190, 190, 206),
            icon: rgb(214, 214, 226),
            accent: rgb(138, 154, 232),
            on_accent: rgb(255, 255, 255),
            separator: rgb(36, 36, 44),
            tab_strip: rgb(47, 47, 56),
            row_selected: rgb(74, 74, 88),
            danger: rgb(250, 160, 172),
            warning: rgb(240, 200, 130),
            radius: 10,
        },
        ThemeKind::SoftLight => Palette {
            chrome: rgb(232, 230, 236),
            canvas: rgb(218, 216, 224),
            dock: rgb(226, 224, 231),
            card: rgb(240, 238, 243),
            card_border: rgb(208, 205, 215),
            field: rgb(250, 249, 252),
            field_border: rgb(190, 187, 201),
            hover: rgb(228, 226, 234),
            pressed: rgb(212, 209, 221),
            text: rgb(44, 42, 56),
            text_dim: rgb(62, 60, 76),
            text_faint: rgb(80, 78, 96),
            icon: rgb(64, 62, 80),
            accent: rgb(110, 128, 216),
            on_accent: rgb(255, 255, 255),
            separator: rgb(210, 207, 218),
            tab_strip: rgb(226, 224, 231),
            row_selected: rgb(212, 209, 221),
            danger: rgb(166, 44, 62),
            warning: rgb(140, 72, 0),
            radius: 10,
        },
        }
    }

    /// Linia pod płaskim paskiem: wyraźnie ciemniejsza od paska (1 piksel ekranu).
    pub fn paint_bar_line(&self, painter: &egui::Painter, rect: egui::Rect) {
        let c = self.chrome;
        let k = if self.is_dark() { 0.6 } else { 0.82 };
        let line = Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8);
        let px = 1.0 / painter.ctx().pixels_per_point();
        let y = rect.bottom() - px * 0.5;
        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], egui::Stroke::new(px, line));
    }

    /// Połysk paska: jasna linia i błysk w górnej połowie, wyraźne przejście w połowie,
    /// ku dołowi ciemniej (jak lakierowany pasek). Rysowany na tle paska, pod jego treścią.
    pub fn paint_gloss(&self, painter: &egui::Painter, rect: egui::Rect) {
        let dark = self.is_dark();
        let (shine_top, shine_mid, shade_bottom, top_line) =
            if dark { (58u8, 24u8, 110u8, 120u8) } else { (160, 70, 42, 210) };
        let w = |a: u8| Color32::from_white_alpha(a);
        let b = |a: u8| Color32::from_black_alpha(a);
        let mid = rect.top() + rect.height() * 0.5;
        let mut mesh = egui::Mesh::default();
        let mut band = |y0: f32, y1: f32, c0: Color32, c1: Color32| {
            let i = mesh.vertices.len() as u32;
            mesh.colored_vertex(egui::pos2(rect.left(), y0), c0);
            mesh.colored_vertex(egui::pos2(rect.right(), y0), c0);
            mesh.colored_vertex(egui::pos2(rect.left(), y1), c1);
            mesh.colored_vertex(egui::pos2(rect.right(), y1), c1);
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i + 1, i + 3, i + 2);
        };
        // Błysk: od jasnego u góry do lekkiego w połowie, potem cień narastający ku dołowi.
        band(rect.top(), mid, w(shine_top), w(shine_mid));
        band(mid, rect.bottom(), b(0), b(shade_bottom));
        painter.add(egui::Shape::mesh(mesh));
        // Ostra jasna linia na samej górze i ciemniejsza na dole.
        let px = 1.0 / painter.ctx().pixels_per_point();
        painter.line_segment(
            [egui::pos2(rect.left(), rect.top() + px * 0.5), egui::pos2(rect.right(), rect.top() + px * 0.5)],
            egui::Stroke::new(px, w(top_line)),
        );
        painter.line_segment(
            [egui::pos2(rect.left(), rect.bottom() - px * 0.5), egui::pos2(rect.right(), rect.bottom() - px * 0.5)],
            egui::Stroke::new(px, b(if dark { 90 } else { 40 })),
        );
    }

    pub fn is_dark(&self) -> bool {
        let c = self.chrome;
        (c.r() as u32 * 299 + c.g() as u32 * 587 + c.b() as u32 * 114) / 1000 < 128
    }

    /// Kolor „sukces” czytelny na tle motywu.
    pub fn ok(&self) -> Color32 {
        if self.is_dark() { rgb(0x6C, 0xC6, 0x7A) } else { rgb(0x2E, 0x8B, 0x3E) }
    }

    pub fn canvas_rgba(&self) -> [f32; 4] {
        let c = self.canvas;
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }
}

static CURRENT: RwLock<Palette> = RwLock::new(ASONE);

/// Aktualna paleta.
pub fn p() -> Palette {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

impl ThemeKind {
    /// Motyw do narysowania: „Jak w systemie” → AsOne (ciemny) albo Pro (jasny).
    pub fn resolve(self, system_dark: bool) -> ThemeKind {
        match self {
            ThemeKind::System if system_dark => ThemeKind::AsOne,
            ThemeKind::System => ThemeKind::ProLight,
            kind => kind,
        }
    }
}

/// Wygląd pulpitu z portalu: 0 – nieznany, 1 – ciemny, 2 – jasny albo bez preferencji.
static DESKTOP_SCHEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Czy system prosi o ciemny wygląd: portal pulpitu (Linux/BSD), inaczej informacja od okna
/// (Windows, macOS, przeglądarka). Bez żadnej informacji – jasny.
pub fn system_dark(ctx: &egui::Context) -> bool {
    match DESKTOP_SCHEME.load(std::sync::atomic::Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => ctx.system_theme() == Some(egui::Theme::Dark),
    }
}

/// Linux/BSD: odczyt wyglądu pulpitu w tle co 5 s (zmiana w systemie przełącza motyw
/// „Jak w systemie”). Czeka do 300 ms na pierwszą odpowiedź – pierwsza klatka ma już właściwy motyw.
pub fn watch_system_scheme(ctx: &egui::Context) {
    #[cfg(all(unix, not(any(target_os = "android", target_os = "macos"))))]
    {
        use std::sync::atomic::Ordering;
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = ctx.clone();
            let _ = std::thread::Builder::new().name("system-theme".into()).spawn(move || {
                let mut first = Some(tx);
                loop {
                    let scheme = match crate::platform::desktop_prefers_dark() {
                        Some(true) => 1,
                        Some(false) => 2,
                        None => 0,
                    };
                    if DESKTOP_SCHEME.swap(scheme, Ordering::Relaxed) != scheme {
                        ctx.request_repaint();
                    }
                    if let Some(tx) = first.take() {
                        let _ = tx.send(());
                    }
                    if scheme == 0 {
                        break; // brak portalu – zostaje informacja od okna
                    }
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });
            let _ = rx.recv_timeout(std::time::Duration::from_millis(300));
        });
    }
    #[cfg(not(all(unix, not(any(target_os = "android", target_os = "macos")))))]
    let _ = ctx;
}

/// Opcje wyglądu z Preferencji › Interfejs.
pub struct Appearance {
    pub kind: ThemeKind,
    /// System prosi o ciemny wygląd (dla motywu „Jak w systemie”).
    pub system_dark: bool,
    pub canvas: Option<Color32>,
    pub show_tooltips: bool,
    pub animations: bool,
}

pub fn apply(ctx: &egui::Context, a: &Appearance) {
    let mut pal = Palette::for_kind(a.kind.resolve(a.system_dark));
    if let Some(c) = a.canvas {
        pal.canvas = c;
    }
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = pal;
    let theme = if pal.is_dark() { Theme::Dark } else { Theme::Light };
    ctx.set_theme(match theme {
        Theme::Dark => egui::ThemePreference::Dark,
        Theme::Light => egui::ThemePreference::Light,
    });
    let radius = CornerRadius::same(pal.radius);
    let (tooltips, animations) = (a.show_tooltips, a.animations);
    ctx.style_mut_of(theme, |style| {
        let v = &mut style.visuals;
        v.dark_mode = pal.is_dark();
        v.panel_fill = pal.card;
        v.window_fill = pal.card;
        v.extreme_bg_color = pal.field;
        v.text_edit_bg_color = Some(pal.field);
        v.faint_bg_color = pal.hover;
        v.code_bg_color = pal.field;
        v.window_stroke = Stroke::new(1.0, pal.card_border);
        v.window_corner_radius = CornerRadius::same(pal.radius.saturating_add(2));
        v.menu_corner_radius = radius;
        v.selection.bg_fill = pal.accent;
        v.selection.stroke = Stroke::new(1.0, pal.on_accent);
        v.hyperlink_color = pal.accent;
        v.warn_fg_color = pal.warning;
        v.error_fg_color = pal.danger;
        v.weak_text_color = Some(pal.text_faint);
        v.override_text_color = None;

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = pal.card;
        w.noninteractive.weak_bg_fill = pal.card;
        w.noninteractive.bg_stroke = Stroke::new(1.0, pal.separator);
        w.noninteractive.fg_stroke = Stroke::new(1.0, pal.text);
        for (wv, fill, stroke, fg) in [
            (&mut w.inactive, pal.field, pal.field_border, pal.text),
            (&mut w.hovered, pal.hover, pal.field_border, pal.text),
            (&mut w.active, pal.pressed, pal.accent, pal.text),
            (&mut w.open, pal.pressed, pal.field_border, pal.text),
        ] {
            wv.bg_fill = fill;
            wv.weak_bg_fill = fill;
            wv.bg_stroke = Stroke::new(1.0, stroke);
            wv.fg_stroke = Stroke::new(1.0, fg);
        }
        w.inactive.bg_stroke = Stroke::new(if pal.is_dark() { 0.0 } else { 1.0 }, pal.field_border);
        for wv in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            wv.corner_radius = radius;
        }

        style.spacing.item_spacing = egui::vec2(6.0, 5.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.interact_size.y = 22.0;
        // Podpowiedzi szersze – opisy mieszczą się w 1–2 liniach zamiast wąskiej kolumny.
        style.spacing.tooltip_width = 720.0;
        style.animation_time = if animations { 1.0 / 12.0 } else { 0.0 };
        style.interaction.tooltip_delay = if tooltips { 0.4 } else { 1.0e9 };
    });
}

/// Styl egui dla aktualnego motywu (jasny/ciemny wg palety).
pub fn current_style(ctx: &egui::Context) -> std::sync::Arc<egui::Style> {
    ctx.style_of(if p().is_dark() { Theme::Dark } else { Theme::Light })
}

/// Styl doku zakładek zgodny z aktualną paletą.
pub fn dock_style(style: &egui::Style) -> egui_dock::Style {
    let pal = p();
    let mut s = egui_dock::Style::from_egui(style);
    s.main_surface_border_stroke = Stroke::NONE;
    s.tab_bar.bg_fill = pal.tab_strip;
    s.tab_bar.hline_color = pal.separator;
    s.tab_bar.height = 26.0;
    s.tab.tab_body.bg_fill = pal.card;
    s.tab.tab_body.stroke = Stroke::NONE;
    s.tab.hline_below_active_tab_name = false;
    for (t, fill, text) in [
        (&mut s.tab.active, pal.card, pal.text),
        (&mut s.tab.focused, pal.card, pal.text),
        (&mut s.tab.active_with_kb_focus, pal.card, pal.text),
        (&mut s.tab.focused_with_kb_focus, pal.card, pal.text),
        (&mut s.tab.inactive, pal.tab_strip, pal.text_dim),
        (&mut s.tab.inactive_with_kb_focus, pal.tab_strip, pal.text_dim),
        (&mut s.tab.hovered, pal.hover, pal.text),
    ] {
        t.bg_fill = fill;
        t.text_color = text;
        t.outline_color = pal.separator;
        t.corner_radius = CornerRadius { nw: pal.radius, ne: pal.radius, sw: 0, se: 0 };
    }
    s.tab.focused.outline_color = pal.accent;
    s.buttons.close_tab_color = pal.text_dim;
    s.buttons.close_tab_active_color = pal.text;
    s.buttons.close_tab_bg_fill = pal.hover;
    s.separator.color_idle = pal.separator;
    s.separator.color_hovered = pal.accent;
    s.separator.color_dragged = pal.accent;
    s.overlay.selection_color = pal.accent.gamma_multiply(0.35);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wszystkie_motywy_maja_czytelny_tekst() {
        for kind in ThemeKind::ALL {
            let p = Palette::for_kind(kind);
            let l = |c: Color32| (c.r() as i32 * 299 + c.g() as i32 * 587 + c.b() as i32 * 114) / 1000;
            assert!((l(p.text) - l(p.card)).abs() > 90, "{kind:?}: tekst {:?} na {:?}", p.text, p.card);
        }
        assert!(!Palette::for_kind(ThemeKind::Sepia).is_dark());
        assert!(Palette::for_kind(ThemeKind::AsOne).is_dark());
    }

    /// Wygląd pulpitu z portalu na tym komputerze: `cargo test -p asone wyglad_pulpitu -- --ignored --nocapture`
    #[test]
    #[ignore = "odpytuje portal pulpitu (sesja graficzna)"]
    fn wyglad_pulpitu_na_zywo() {
        #[cfg(all(unix, not(any(target_os = "android", target_os = "macos"))))]
        eprintln!("portal: ciemny = {:?}", crate::platform::desktop_prefers_dark());
    }

    #[test]
    fn motyw_jak_w_systemie() {
        assert_eq!(ThemeKind::System.resolve(true), ThemeKind::AsOne);
        assert_eq!(ThemeKind::System.resolve(false), ThemeKind::ProLight);
        assert_eq!(ThemeKind::Sepia.resolve(true), ThemeKind::Sepia);
        assert_eq!(crate::settings::Settings::default().theme, ThemeKind::System);
    }
}
