//! Ustawienia › Preferencje (układ jak w PhotoCrafcie: sekcje po lewej, Anuluj / Zastosuj / OK).

use egui::{RichText, Ui, vec2};

use crate::settings::{AiModel, BarStyle, CanvasColor, EditModel, HideItem, PrefsSection, Settings, TabTitle, UI_SCALES};
use crate::theme::{self, ThemeKind};

#[derive(Default)]
pub struct PrefsDialog {
    pub open: bool,
    pub section: PrefsSection,
    draft: Settings,
}

/// Co okno Preferencji zleca aplikacji.
#[derive(Default)]
pub struct PrefsResult {
    /// Zastosuj (okno zostaje) lub OK (okno się zamyka).
    pub apply: Option<Settings>,
    /// „Pobierz model”: model, plik docelowy, token Hugging Face.
    pub download: Option<(AiModel, std::path::PathBuf, String)>,
    /// „Wybierz…” plik modelu (okno systemowe).
    pub pick_model: bool,
}

/// Stan pobierania modelu pokazywany w Preferencjach.
pub struct DownloadView {
    pub model: AiModel,
    pub done: u64,
    pub total: Option<u64>,
}

impl PrefsDialog {
    pub fn open(&mut self, section: PrefsSection, current: &Settings) {
        self.open = true;
        self.section = section;
        self.draft = current.clone();
    }

    /// Plik wybrany w oknie „Wybierz…” trafia do szkicu ustawień.
    pub fn set_model_path(&mut self, path: std::path::PathBuf) {
        let p = path.display().to_string();
        match self.draft.ai_model {
            AiModel::Rmbg14 => self.draft.ai_path_14 = p,
            AiModel::Rmbg20 => self.draft.ai_path_20 = p,
            AiModel::Rmbg30Fal => {}
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, download: Option<&DownloadView>) -> PrefsResult {
        let mut result = PrefsResult::default();
        if !self.open {
            return result;
        }
        let mut open = true;
        let mut close = false;
        // Stały rozmiar startowy; panele wypełniają dokładnie wnętrze okna, więc okno nie „rośnie”
        // z klatki na klatkę (jak przy liczeniu wysokości z `available_height`).
        let size = vec2(740.0, 520.0);
        egui::Window::new("Preferencje")
            .id(egui::Id::new("asone_prefs"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .movable(true)
            .constrain(true)
            .default_size(size)
            .min_size(vec2(580.0, 360.0))
            .default_pos(ctx.content_rect().center() - size / 2.0)
            .show(ctx, |ui| {
                let pal = theme::p();
                // Dolny pasek akcji: Anuluj / Zastosuj / OK
                egui::Panel::bottom("prefs_buttons").resizable(false).frame(egui::Frame::NONE).show(ui, |ui| {
                    ui.add_space(6.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ok = egui::Button::new(RichText::new("OK").color(pal.accent_text)).fill(pal.accent).min_size(vec2(84.0, 26.0));
                        if ui.add(ok).clicked() {
                            result.apply = Some(self.draft.clone());
                            close = true;
                        }
                        if ui.add(egui::Button::new("Zastosuj").min_size(vec2(84.0, 26.0))).clicked() {
                            result.apply = Some(self.draft.clone());
                        }
                        if ui.add(egui::Button::new("Anuluj").min_size(vec2(84.0, 26.0))).clicked() {
                            close = true;
                        }
                    });
                });
                // Lewa kolumna: kategorie preferencji
                egui::Panel::left("prefs_sections").exact_size(176.0).resizable(false).frame(egui::Frame::NONE).show(ui, |ui| {
                    ui.add_space(4.0);
                    for (section, label) in [
                        (PrefsSection::Interface, "Interfejs"),
                        (PrefsSection::Export, "Eksport i limity"),
                        (PrefsSection::Ai, "Usuwanie tła (AI)"),
                        (PrefsSection::Replicate, "Replicate (AI online)"),
                    ] {
                        if ui.add_sized(vec2(164.0, 26.0), egui::Button::selectable(self.section == section, label)).clicked() {
                            self.section = section;
                        }
                    }
                });
                // Prawa część: zawartość sekcji w wyśrodkowanej kolumnie (maks. 500 px)
                egui::CentralPanel::no_frame().show(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        let avail = ui.available_width();
                        let column = avail.min(500.0);
                        let left = ui.cursor().min.x + ((avail - column) / 2.0).max(0.0);
                        let rect = egui::Rect::from_min_size(egui::pos2(left, ui.cursor().min.y), vec2(column, f32::INFINITY));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                            ui.set_width(column);
                            ui.add_space(4.0);
                            match self.section {
                                PrefsSection::Interface => interface_page(ui, &mut self.draft),
                                PrefsSection::Export => export_page(ui, &mut self.draft),
                                PrefsSection::Ai => ai_page(ui, &mut self.draft, download, &mut result),
                                PrefsSection::Replicate => replicate_page(ui, &mut self.draft),
                            }
                        });
                    });
                });
            });
        if !open || close {
            self.open = false;
        }
        result
    }
}

fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.label(label);
    add(ui);
    ui.end_row();
}

fn interface_page(ui: &mut Ui, s: &mut Settings) {
    ui.heading("Interfejs");
    ui.add_space(6.0);
    egui::Grid::new("prefs_interface").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        row(ui, "Motyw", |ui| {
            egui::ComboBox::from_id_salt("theme")
                .width(220.0)
                .selected_text(s.theme.label())
                .show_ui(ui, |ui| {
                    for kind in ThemeKind::ALL {
                        ui.selectable_value(&mut s.theme, kind, kind.label());
                    }
                });
        });
        row(ui, "Kolor płótna", |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("canvas")
                    .width(150.0)
                    .selected_text(s.canvas_color.label())
                    .show_ui(ui, |ui| {
                        for c in CanvasColor::ALL {
                            ui.selectable_value(&mut s.canvas_color, c, c.label());
                        }
                    });
                if s.canvas_color == CanvasColor::Custom {
                    egui::color_picker::color_edit_button_srgb(ui, &mut s.canvas_custom);
                    let [r, g, b] = s.canvas_custom;
                    ui.monospace(format!("#{r:02X}{g:02X}{b:02X}"));
                }
            });
        });
        row(ui, "Skala interfejsu", |ui| {
            egui::ComboBox::from_id_salt("ui_scale")
                .width(120.0)
                .selected_text(format!("{} %", s.ui_scale))
                .show_ui(ui, |ui| {
                    for v in UI_SCALES {
                        ui.selectable_value(&mut s.ui_scale, v, format!("{v} %"));
                    }
                });
        });
        row(ui, "Zakładki zdjęć", |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.photo_tabs_bottom, false, "Na górze");
                ui.radio_value(&mut s.photo_tabs_bottom, true, "Na dole");
            });
        });
        row(ui, "Na zakładce", |ui| {
            ui.vertical(|ui| {
                ui.radio_value(&mut s.tab_title, TabTitle::Full, "Pokazuj pełną nazwę (#1 · nazwa)");
                ui.radio_value(&mut s.tab_title, TabTitle::NumberOnly, "Pokazuj tylko numer kolejności (#1)");
            });
        });
        row(ui, "", |ui| {
            ui.checkbox(&mut s.show_tooltips, "Pokaż podpowiedzi");
        });
        row(ui, "", |ui| {
            ui.checkbox(&mut s.animations, "Animacje (menu, przewijanie)");
        });
        row(ui, "Pasek menu", |ui| {
            ui.vertical(|ui| {
                for style in BarStyle::ALL {
                    ui.radio_value(&mut s.bar_style, style, style.label());
                }
            });
        });
        if cfg!(not(any(target_os = "macos", target_os = "android", target_arch = "wasm32"))) {
            row(ui, "", |ui| {
                ui.checkbox(&mut s.system_title_bar, "Systemowy pasek tytułu")
                    .on_hover_text("Zamiast paska AsOne z menu i przyciskami okna – np. dla menedżerów kafelkowych.");
            });
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Ukryj").strong());
    for item in HideItem::ALL {
        ui.checkbox(s.hidden_mut(item), item.label());
    }
    ui.add_space(10.0);
    if ui.button("Przywróć domyślne").clicked() {
        s.reset_section(PrefsSection::Interface);
    }
}

fn export_page(ui: &mut Ui, s: &mut Settings) {
    let pal = theme::p();
    ui.heading("Eksport i limity");
    ui.colored_label(
        pal.text_dim,
        "Limity działają przy każdym zapisie – także wtedy, gdy ktoś zapomni ustawić zmniejszanie na górnym pasku.",
    );
    ui.add_space(6.0);
    egui::Grid::new("prefs_export").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        row(ui, "Maks. wymiary", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.enforce_dimensions, "");
                ui.add_enabled_ui(s.enforce_dimensions, |ui| {
                    ui.add(egui::DragValue::new(&mut s.max_width).range(16..=16384).suffix(" px"));
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut s.max_height).range(16..=16384).suffix(" px"));
                    ui.colored_label(pal.text_dim, "(szer. × wys.)");
                });
            });
        });
        row(ui, "Maks. rozmiar pliku", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.enforce_file_size, "");
                ui.add_enabled(
                    s.enforce_file_size,
                    egui::DragValue::new(&mut s.max_file_mb).range(0.05..=100.0).speed(0.05).fixed_decimals(2).suffix(" MB"),
                );
            });
        });
        row(ui, "Najniższa jakość", |ui| {
            ui.add_enabled(s.enforce_file_size, egui::Slider::new(&mut s.min_quality, 30..=95).suffix(" %"))
                .on_hover_text("Do tej jakości wolno zejść, żeby zmieścić plik w limicie; dalej zdjęcie jest zmniejszane.");
        });
        row(ui, "Min. bok", |ui| {
            let floor = asone_core::finish::MIN_SIDE;
            s.min_side = s.min_side.max(floor);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.min_side).range(floor..=4000).suffix(" px"))
                    .on_hover_text(format!("Nie mniej niż {floor} px."));
                ui.colored_label(pal.text_dim, "mniejsze zdjęcia dopełniane białym tłem");
            });
        });
        row(ui, "Ramka przy zmniejszaniu", |ui| {
            ui.vertical(|ui| {
                ui.checkbox(&mut s.keep_frame, "zachowaj ramkę w pełnych pikselach")
                    .on_hover_text(format!(
                        "Gdy zdjęcie z marginesami trzeba zmniejszyć (limit wymiarów, „Max dłuższy bok”), \
                         marginesy do {} px zostają w pliku dokładnie takie, jak ustawione (np. ramka 5 px), \
                         a zmniejsza się samo zdjęcie. Większe marginesy (np. kwadrat 1:1) skalują się razem ze zdjęciem.",
                        asone_core::finish::FRAME_MAX
                    ));
                ui.colored_label(
                    pal.text_dim,
                    format!("np. ramka 5 px na zdjęciu 4000×5000 → plik 2880×3600 z ramką 5 px (ramki do {} px)", asone_core::finish::FRAME_MAX),
                );
            });
        });
        row(ui, "Domyślny folder zapisu", |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut s.out_subfolder).hint_text("web").desired_width(140.0))
                    .on_hover_text("Podfolder tworzony obok zdjęć, gdy nie wybrano innego folderu przyciskiem „Folder”.");
                ui.colored_label(pal.text_dim, "podfolder obok zdjęć (np. web)");
            });
        });
    });
    ui.add_space(10.0);
    if ui.button("Przywróć domyślne (3000 × 3600, 3 MB)").clicked() {
        s.reset_section(PrefsSection::Export);
    }
}

/// Replicate: jeden przełącznik dla wszystkich płatnych funkcji, klucz i model edycji.
fn replicate_page(ui: &mut Ui, s: &mut Settings) {
    let pal = theme::p();
    ui.heading("Replicate (edycja AI online)");
    ui.colored_label(
        pal.text_dim,
        "Edycja zdjęcia poleceniem (np. białe tło z cieniem, usunięcie napisów). Płatne za każdy obraz z kredytu \
         konta Replicate; zdjęcie jest wysyłane na serwery Replicate (dane zapytań kasowane po godzinie).",
    );
    ui.add_space(8.0);
    ui.checkbox(&mut s.replicate_enabled, egui::RichText::new("Włącz funkcje Replicate").strong())
        .on_hover_text("Wyłączone: znikają wszystkie płatne przyciski Replicate (panel „Edycja AI” i menu zdjęcia).");
    if !crate::ai::AVAILABLE {
        ui.colored_label(pal.warning, "W tej wersji (przeglądarka, Android, FreeBSD) edycja AI jest niedostępna.");
    }
    ui.add_space(6.0);
    ui.add_enabled_ui(s.replicate_enabled, |ui| {
        egui::Grid::new("prefs_replicate").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
            row(ui, "Klucz API", |ui| {
                ui.vertical(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut s.replicate_key)
                            .password(true)
                            .hint_text("r8_…")
                            .desired_width(300.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.hyperlink_to(egui::RichText::new("utwórz klucz").small(), "https://replicate.com/account/api-tokens");
                        ui.small("·");
                        ui.hyperlink_to(egui::RichText::new("kredyt i rozliczenia").small(), "https://replicate.com/account/billing");
                    });
                    if s.replicate_key.trim().is_empty() {
                        if s.replicate_api_key().is_empty() {
                            ui.colored_label(pal.warning, egui::RichText::new("brak klucza – funkcje Replicate są ukryte").small());
                        } else {
                            ui.small("używany klucz z REPLICATE_API_TOKEN albo ~/.config/asone/replicate-token");
                        }
                    }
                    ui.small("Klucz wpisany tutaj jest zapisywany tylko lokalnie, w ustawieniach AsOne.");
                });
            });
            row(ui, "Model", |ui| {
                ui.vertical(|ui| {
                    for model in EditModel::ALL {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut s.replicate_model, model, format!("{} – {} za obraz", model.label(), model.price()));
                            ui.hyperlink_to(egui::RichText::new("strona").small(), model.page_url());
                        });
                    }
                    ui.small("Ceny z cennika Replicate z 20.11.2026; wyniki wolno używać komercyjnie (strona modelu).");
                });
            });
            row(ui, "Wysyłane", |ui| {
                ui.vertical(|ui| {
                    ui.small("obraz z zakładki (kadr, marginesy, smartfon) jako JPEG do 1536 px");
                    ui.small("wynik (ok. 1 Mpx) zastępuje zdjęcie w zakładce – „Przywróć oryginał” wraca do pliku");
                });
            });
        });
    });
    ui.add_space(10.0);
    if ui.button("Przywróć domyślne").clicked() {
        s.reset_section(PrefsSection::Replicate);
    }
}

/// RMBG-3.0 online (fal.ai): koszt, klucz API, co jest wysyłane.
fn online_rows(ui: &mut Ui, s: &mut Settings) {
    let pal = theme::p();
    row(ui, "Wymagania", |ui| {
        ui.vertical(|ui| {
            ui.colored_label(pal.warning, "internet · konto fal.ai z doładowanymi kredytami");
            ui.horizontal_wrapped(|ui| {
                ui.small("0,018 USD za zdjęcie (cennik fal.ai z 20.11.2026) –");
                ui.hyperlink_to(egui::RichText::new("aktualna cena").small(), s.ai_model.page_url());
            });
            ui.small("kilka–kilkanaście sekund na zdjęcie (wysyłanie + model na serwerze)");
        });
    });
    row(ui, "Klucz fal.ai", |ui| {
        ui.vertical(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut s.fal_key)
                    .password(true)
                    .hint_text("klucz API fal.ai (albo zmienna FAL_KEY)")
                    .desired_width(300.0),
            );
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to(egui::RichText::new("utwórz klucz").small(), "https://fal.ai/dashboard/keys");
                ui.small("·");
                ui.hyperlink_to(egui::RichText::new("saldo i zużycie").small(), "https://fal.ai/dashboard/usage-billing");
                ui.small("·");
                ui.hyperlink_to(egui::RichText::new("historia zapytań").small(), "https://fal.ai/dashboard/requests");
            });
            ui.small("Klucz jest zapisywany tylko lokalnie, w ustawieniach AsOne.");
            if s.fal_key.trim().is_empty() && !s.fal_api_key().is_empty() {
                ui.small("używany klucz ze zmiennej środowiskowej FAL_KEY");
            }
        });
    });
    row(ui, "Licencja", |ui| {
        ui.vertical(|ui| {
            ui.colored_label(pal.ok(), "do zastosowań komercyjnych");
            ui.horizontal_wrapped(|ui| {
                ui.small("fal.ai: „Commercial use” – model trenowany wyłącznie na licencjonowanych danych –");
                ui.hyperlink_to(egui::RichText::new("warunki na stronie modelu").small(), s.ai_model.page_url());
            });
        });
    });
    row(ui, "Wysyłane", |ui| {
        ui.vertical(|ui| {
            ui.small(format!(
                "zdjęcie jako JPEG (dłuższy bok do {} px, bez metadanych) do fal.ai, model bria/rmbg-3",
                crate::ai::FAL_UPLOAD_SIDE
            ));
            ui.small("wynik: maska przezroczystości w rozmiarze zdjęcia – dalej jak przy modelach lokalnych");
        });
    });
}

fn ai_page(ui: &mut Ui, s: &mut Settings, download: Option<&DownloadView>, result: &mut PrefsResult) {
    let pal = theme::p();
    ui.heading("Usuwanie tła (AI)");
    if s.ai_model.is_online() {
        ui.colored_label(pal.warning, "RMBG-3.0 online: zdjęcie jest wysyłane do fal.ai, każde usunięcie tła jest płatne.");
    } else {
        ui.colored_label(pal.text_dim, "Modele BRIA RMBG-1.4 i 2.0 działają lokalnie – zdjęcia nie są nigdzie wysyłane.");
    }
    if !crate::ai::AVAILABLE {
        ui.colored_label(pal.warning, "W tej wersji (przeglądarka, Android, FreeBSD) usuwanie tła AI jest niedostępne.");
    }
    ui.add_space(6.0);
    egui::Grid::new("prefs_ai").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        row(ui, "Model", |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("ai_model")
                    .width(220.0)
                    .selected_text(s.ai_model.label())
                    .show_ui(ui, |ui| {
                        for m in AiModel::ALL {
                            ui.selectable_value(&mut s.ai_model, m, m.label());
                        }
                    });
                ui.hyperlink_to("strona modelu", s.ai_model.page_url());
            });
        });
        if s.ai_model.is_online() {
            online_rows(ui, s);
            return;
        }
        row(ui, "Wymagania", |ui| {
            let (weights, run) = crate::ai::memory_need_gb(s.ai_model);
            let (text, color) = match s.ai_model {
                AiModel::Rmbg14 => (format!("ok. {:.0} GB RAM · ok. 1 s na zdjęcie", weights + run), pal.text_dim),
                AiModel::Rmbg20 => (
                    format!("ok. {:.0} GB wolnej pamięci RAM · 15–20 s na zdjęcie (procesor)", (weights + run).ceil()),
                    pal.warning,
                ),
                AiModel::Rmbg30Fal => (String::new(), pal.text_dim),
            };
            ui.vertical(|ui| {
                ui.colored_label(color, text);
                if let Some(free) = crate::ai::free_memory_bytes() {
                    ui.small(format!("teraz wolne: {:.1} GB (RAM + swap)", free as f64 / (1u64 << 30) as f64));
                }
            });
        });
        row(ui, "Plik modelu", |ui| {
            ui.horizontal(|ui| {
                let default = crate::settings::default_models_dir().join(s.ai_model.file_name());
                let field = match s.ai_model {
                    AiModel::Rmbg14 => &mut s.ai_path_14,
                    AiModel::Rmbg20 | AiModel::Rmbg30Fal => &mut s.ai_path_20,
                };
                ui.add(
                    egui::TextEdit::singleline(field)
                        .hint_text(default.display().to_string())
                        .desired_width(300.0),
                );
                if crate::platform::CAN_PICK_FOLDER && ui.button("Wybierz…").clicked() {
                    result.pick_model = true;
                }
            });
        });
        row(ui, "", |ui| {
            let path = s.ai_model_path();
            match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => {
                    ui.colored_label(pal.ok(), format!("✔ Plik jest ({:.0} MB)", m.len() as f64 / 1_048_576.0))
                }
                _ => ui.colored_label(pal.warning, "Brak pliku – pobierz model poniżej albo wskaż istniejący plik .onnx"),
            };
        });
        row(ui, "Pobieranie", |ui| {
            ui.vertical(|ui| {
                match download {
                    Some(d) => {
                        let frac = d.total.map_or(0.0, |t| d.done as f32 / t.max(1) as f32);
                        let text = match d.total {
                            Some(t) => format!("{:.0} / {:.0} MB", d.done as f64 / 1_048_576.0, t as f64 / 1_048_576.0),
                            None => format!("{:.0} MB", d.done as f64 / 1_048_576.0),
                        };
                        ui.add(egui::ProgressBar::new(frac).desired_width(300.0).text(format!("{} · {text}", d.model.label())));
                    }
                    None => {
                        let needs_token = s.ai_model.needs_token() && s.hf_token.trim().is_empty();
                        let button = ui.add_enabled(crate::ai::AVAILABLE && !needs_token, egui::Button::new("Pobierz model"));
                        let button = if needs_token {
                            button.on_disabled_hover_text("RMBG-2.0 wymaga tokenu Hugging Face (poniżej)")
                        } else {
                            button
                        };
                        if button.clicked() {
                            result.download = Some((s.ai_model, s.ai_model_path(), s.hf_token.clone()));
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.small("lub pobierz ręcznie:");
                    ui.hyperlink_to(egui::RichText::new(s.ai_model.download_url()).small(), s.ai_model.download_url());
                });
            });
        });
        row(ui, "Token Hugging Face", |ui| {
            ui.vertical(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut s.hf_token)
                        .password(true)
                        .hint_text("hf_…  (potrzebny tylko dla RMBG-2.0)")
                        .desired_width(300.0),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.small("klucz (token) po rejestracji na huggingface.co jest darmowy –");
                    ui.hyperlink_to(egui::RichText::new("utwórz token (Read)").small(), "https://huggingface.co/settings/tokens");
                });
                ui.small("dla RMBG-2.0 zaakceptuj też licencję na stronie modelu. Token jest zapisywany tylko lokalnie.");
            });
        });
    });
    ui.add_space(8.0);
    let note = if s.ai_model.is_online() {
        "RMBG-3.0 przez fal.ai: także do zastosowań komercyjnych; zapytania rozlicza fal.ai (przedpłacone kredyty)."
    } else {
        "Lokalne modele BRIA RMBG-1.4 i 2.0 są darmowe do użytku niekomercyjnego; do komercyjnego potrzebna jest umowa \
         z BRIA – albo RMBG-3.0 przez fal.ai."
    };
    ui.colored_label(pal.text_dim, egui::RichText::new(note).small());
    ui.add_space(6.0);
    if ui.button("Przywróć domyślne").clicked() {
        s.reset_section(PrefsSection::Ai);
    }
}
