//! Ustawienia › Preferencje (układ jak w PhotoCrafcie: sekcje po lewej, Anuluj / Zastosuj / OK).

use egui::{RichText, Ui, vec2};

use crate::settings::{AiModel, CanvasColor, HideItem, PrefsSection, Settings, UI_SCALES};
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
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, download: Option<&DownloadView>) -> PrefsResult {
        let mut result = PrefsResult::default();
        if !self.open {
            return result;
        }
        let mut open = true;
        let mut close = false;
        egui::Window::new("Preferencje")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(vec2(720.0, 440.0))
            .min_size(vec2(560.0, 360.0))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                let pal = theme::p();
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
                egui::Panel::left("prefs_sections").exact_size(170.0).resizable(false).frame(egui::Frame::NONE).show(ui, |ui| {
                    for (section, label) in [
                        (PrefsSection::Interface, "Interfejs"),
                        (PrefsSection::Export, "Eksport i limity"),
                        (PrefsSection::Ai, "Usuwanie tła (AI)"),
                    ] {
                        if ui.add_sized(vec2(160.0, 24.0), egui::Button::selectable(self.section == section, label)).clicked() {
                            self.section = section;
                        }
                    }
                });
                egui::CentralPanel::no_frame().show(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.section {
                        PrefsSection::Interface => interface_page(ui, &mut self.draft),
                        PrefsSection::Export => export_page(ui, &mut self.draft),
                        PrefsSection::Ai => ai_page(ui, &mut self.draft, download, &mut result),
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
        row(ui, "", |ui| {
            ui.checkbox(&mut s.show_tooltips, "Pokaż podpowiedzi");
        });
        row(ui, "", |ui| {
            ui.checkbox(&mut s.animations, "Animacje (menu, przewijanie)");
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
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.enforce_min_side, "");
                ui.add_enabled(s.enforce_min_side, egui::DragValue::new(&mut s.min_side).range(16..=4000).suffix(" px"));
                ui.colored_label(pal.text_dim, "mniejsze zdjęcia dopełnij tłem");
            });
        });
        row(ui, "Podfolder eksportu", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.out_subfolder).desired_width(140.0))
                .on_hover_text("Używany, gdy nie wybrano folderu: tworzony obok pierwszego zdjęcia.");
        });
    });
    ui.add_space(10.0);
    if ui.button("Przywróć domyślne (3000 × 3600, 3 MB)").clicked() {
        s.reset_section(PrefsSection::Export);
    }
}

fn ai_page(ui: &mut Ui, s: &mut Settings, download: Option<&DownloadView>, result: &mut PrefsResult) {
    let pal = theme::p();
    ui.heading("Usuwanie tła (AI)");
    ui.colored_label(pal.text_dim, "Modele BRIA RMBG działają lokalnie – zdjęcia nie są nigdzie wysyłane.");
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
        row(ui, "Plik modelu", |ui| {
            ui.horizontal(|ui| {
                let default = crate::settings::default_models_dir().join(s.ai_model.file_name());
                let field = match s.ai_model {
                    AiModel::Rmbg14 => &mut s.ai_path_14,
                    AiModel::Rmbg20 => &mut s.ai_path_20,
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
    ui.colored_label(
        pal.text_dim,
        egui::RichText::new("Modele BRIA RMBG są darmowe do użytku niekomercyjnego; do użytku komercyjnego potrzebna jest umowa z BRIA.").small(),
    );
    ui.add_space(6.0);
    if ui.button("Przywróć domyślne").clicked() {
        s.reset_section(PrefsSection::Ai);
    }
}
