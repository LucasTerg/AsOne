//! Ustawienia › Preferencje (sekcje po lewej, Anuluj / Zastosuj / OK).

use egui::{RichText, Ui, vec2};

use crate::settings::{AiModel, BarStyle, CanvasColor, EditModel, HideItem, ModelFile, PrefsSection, Settings, TabTitle, UI_SCALES};
use crate::theme::{self, ThemeKind};

#[derive(Default)]
pub struct PrefsDialog {
    pub open: bool,
    pub section: PrefsSection,
    draft: Settings,
    /// Dla którego modelu jest otwarte okno „Wybierz…” pliku.
    pick_target: Option<ModelFile>,
}

/// Co okno Preferencji zleca aplikacji.
#[derive(Default)]
pub struct PrefsResult {
    /// Zastosuj (okno zostaje) lub OK (okno się zamyka).
    pub apply: Option<Settings>,
    /// „Pobierz model”: model, plik docelowy, token Hugging Face.
    pub download: Option<(ModelFile, std::path::PathBuf, String)>,
    /// „Wybierz…” plik modelu (okno systemowe).
    pub pick_model: Option<ModelFile>,
    /// Android: pole tekstowe do edycji w systemowym oknie (cel, bieżąca wartość).
    pub edit_text: Option<(crate::platform::TextTarget, String)>,
}

/// Android: pole klucza otwiera systemowe okno z klawiaturą (pola egui jej tam nie dostają).
fn native_text(ui: &Ui, field: &egui::Response, target: crate::platform::TextTarget, current: &str, result: &mut PrefsResult) {
    if cfg!(target_os = "android") && field.gained_focus() {
        result.edit_text = Some((target, current.to_string()));
        ui.memory_mut(|m| m.surrender_focus(field.id));
    }
}

/// Stan pobierania modelu pokazywany w Preferencjach.
pub struct DownloadView {
    pub model: ModelFile,
    pub done: u64,
    pub total: Option<u64>,
}

impl PrefsDialog {
    pub fn open(&mut self, section: PrefsSection, current: &Settings) {
        self.open = true;
        self.section = section;
        self.draft = current.clone();
    }

    /// Android: tekst z systemowego okna edycji trafia do szkicu ustawień.
    pub fn set_text(&mut self, target: crate::platform::TextTarget, text: String) {
        match target {
            crate::platform::TextTarget::FalKey => self.draft.fal_key = text.trim().to_string(),
            crate::platform::TextTarget::ReplicateKey => self.draft.replicate_key = text.trim().to_string(),
            _ => {}
        }
    }

    /// Plik wybrany w oknie „Wybierz…” trafia do szkicu ustawień.
    pub fn set_model_path(&mut self, path: std::path::PathBuf) {
        let p = path.display().to_string();
        match self.pick_target.take() {
            Some(ModelFile::Lama) => self.draft.lama_path = p,
            _ => match self.draft.ai_model {
                AiModel::Rmbg14 => self.draft.ai_path_14 = p,
                AiModel::Rmbg20 => self.draft.ai_path_20 = p,
                AiModel::Rmbg30Fal => {}
            },
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
        egui::Window::new(t!("Preferencje"))
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
                        let ok = egui::Button::new(RichText::new("OK").color(pal.on_accent)).fill(pal.accent).min_size(vec2(84.0, 26.0));
                        if ui.add(ok).clicked() {
                            result.apply = Some(self.draft.clone());
                            close = true;
                        }
                        if ui.add(egui::Button::new(t!("Zastosuj")).min_size(vec2(84.0, 26.0))).clicked() {
                            result.apply = Some(self.draft.clone());
                        }
                        if ui.add(egui::Button::new(t!("Anuluj")).min_size(vec2(84.0, 26.0))).clicked() {
                            close = true;
                        }
                    });
                });
                // Lewa kolumna: kategorie preferencji
                egui::Panel::left("prefs_sections").exact_size(176.0).resizable(false).frame(egui::Frame::NONE).show(ui, |ui| {
                    ui.add_space(4.0);
                    for (section, label) in [
                        (PrefsSection::Interface, t!("Interfejs")),
                        (PrefsSection::Export, t!("Eksport i limity")),
                        (PrefsSection::Ai, t!("Usuwanie tła (AI)")),
                        (PrefsSection::Retouch, t!("Retusz (LaMa)")),
                        (PrefsSection::Upscale, t!("Powiększanie (waifu2x)")),
                        (PrefsSection::Replicate, t!("Replicate (AI online)")),
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
                                PrefsSection::Retouch => retouch_page(ui, &mut self.draft, download, &mut result),
                                PrefsSection::Upscale => upscale_page(ui, &mut self.draft),
                                PrefsSection::Replicate => replicate_page(ui, &mut self.draft, &mut result),
                            }
                        });
                    });
                });
            });
        if !open || close {
            self.open = false;
        }
        if let Some(target) = result.pick_model {
            self.pick_target = Some(target);
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
    ui.heading(t!("Interfejs"));
    ui.add_space(6.0);
    egui::Grid::new("prefs_interface").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        // Opis w obu językach – do znalezienia także przy niezrozumiałym języku.
        row(ui, "Język / Language", |ui| {
            egui::ComboBox::from_id_salt("language")
                .width(220.0)
                .selected_text(s.language.label())
                .show_ui(ui, |ui| {
                    for lang in crate::i18n::Language::ALL {
                        ui.selectable_value(&mut s.language, lang, lang.label());
                    }
                });
        });
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
        row(ui, t!("Kolor płótna"), |ui| {
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
        row(ui, t!("Skala interfejsu"), |ui| {
            egui::ComboBox::from_id_salt("ui_scale")
                .width(120.0)
                .selected_text(format!("{} %", s.ui_scale))
                .show_ui(ui, |ui| {
                    for v in UI_SCALES {
                        ui.selectable_value(&mut s.ui_scale, v, format!("{v} %"));
                    }
                });
        });
        row(ui, t!("Zakładki zdjęć"), |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.photo_tabs_bottom, false, t!("Na górze"));
                ui.radio_value(&mut s.photo_tabs_bottom, true, t!("Na dole"));
            });
        });
        row(ui, t!("Na zakładce"), |ui| {
            ui.vertical(|ui| {
                ui.radio_value(&mut s.tab_title, TabTitle::Full, t!("Pokazuj pełną nazwę (#1 · nazwa)"));
                ui.radio_value(&mut s.tab_title, TabTitle::NumberOnly, t!("Pokazuj tylko numer kolejności (#1)"));
            });
        });
        row(ui, "", |ui| {
            ui.checkbox(&mut s.show_tooltips, t!("Pokaż podpowiedzi"));
        });
        if cfg!(not(target_arch = "wasm32")) {
            row(ui, "", |ui| {
                ui.checkbox(&mut s.check_updates, t!("Sprawdzaj aktualizacje przy starcie (wersja AppImage)"))
                    .on_hover_text(t!("Pyta GitHub o najnowsze wydanie; nowa wersja pobiera się dopiero po kliknięciu „Zaktualizuj”."));
            });
        }
        row(ui, "", |ui| {
            ui.checkbox(&mut s.animations, t!("Animacje (menu, przewijanie)"));
        });
        row(ui, t!("Pasek menu"), |ui| {
            ui.vertical(|ui| {
                for style in BarStyle::ALL {
                    ui.radio_value(&mut s.bar_style, style, style.label());
                }
            });
        });
        if cfg!(not(any(target_os = "macos", target_os = "android", target_arch = "wasm32"))) {
            row(ui, "", |ui| {
                ui.checkbox(&mut s.system_title_bar, t!("Systemowy pasek tytułu"))
                    .on_hover_text(t!("Zamiast paska AsOne z menu i przyciskami okna – np. dla menedżerów kafelkowych."));
            });
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Ukryj").strong());
    for item in HideItem::ALL {
        ui.checkbox(s.hidden_mut(item), item.label());
    }
    ui.add_space(10.0);
    if ui.button(t!("Przywróć domyślne")).clicked() {
        s.reset_section(PrefsSection::Interface);
    }
}

fn export_page(ui: &mut Ui, s: &mut Settings) {
    let pal = theme::p();
    ui.heading(t!("Eksport i limity"));
    ui.colored_label(
        pal.text_dim,
        t!("Limity działają przy każdym zapisie – także wtedy, gdy ktoś zapomni ustawić zmniejszanie na górnym pasku."),
    );
    ui.add_space(6.0);
    egui::Grid::new("prefs_export").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        row(ui, t!("Maks. wymiary"), |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.enforce_dimensions, "");
                ui.add_enabled_ui(s.enforce_dimensions, |ui| {
                    ui.add(egui::DragValue::new(&mut s.max_width).range(16..=16384).suffix(" px"));
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut s.max_height).range(16..=16384).suffix(" px"));
                    ui.colored_label(pal.text_dim, t!("(szer. × wys.)"));
                });
            });
        });
        row(ui, t!("Maks. rozmiar pliku"), |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.enforce_file_size, "");
                ui.add_enabled(
                    s.enforce_file_size,
                    egui::DragValue::new(&mut s.max_file_mb).range(0.05..=100.0).speed(0.05).fixed_decimals(2).suffix(" MB"),
                );
            });
        });
        row(ui, t!("Najniższa jakość"), |ui| {
            ui.add_enabled(s.enforce_file_size, egui::Slider::new(&mut s.min_quality, 30..=95).suffix(" %"))
                .on_hover_text(t!("Do tej jakości wolno zejść, żeby zmieścić plik w limicie; dalej zdjęcie jest zmniejszane."));
        });
        row(ui, t!("Min. bok"), |ui| {
            let floor = asone_core::finish::MIN_SIDE;
            s.min_side = s.min_side.max(floor);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.min_side).range(floor..=4000).suffix(" px"))
                    .on_hover_text(t!("Nie mniej niż {floor} px.", floor = floor));
                ui.colored_label(pal.text_dim, t!("mniejsze zdjęcia dopełniane białym tłem"));
            });
        });
        row(ui, t!("Ramka przy zmniejszaniu"), |ui| {
            ui.vertical(|ui| {
                ui.checkbox(&mut s.keep_frame, t!("zachowaj ramkę w pełnych pikselach"))
                    .on_hover_text(t!(
                        "Gdy zdjęcie z marginesami trzeba zmniejszyć (limit wymiarów, „Max dłuższy bok”), \
                         marginesy do {max} px zostają w pliku dokładnie takie, jak ustawione (np. ramka 5 px), \
                         a zmniejsza się samo zdjęcie. Większe marginesy (np. kwadrat 1:1) skalują się razem ze zdjęciem.",
                        max = asone_core::finish::FRAME_MAX
                    ));
                ui.colored_label(
                    pal.text_dim,
                    t!("np. ramka 5 px na zdjęciu 4000×5000 → plik 2880×3600 z ramką 5 px (ramki do {max} px)", max = asone_core::finish::FRAME_MAX),
                );
            });
        });
        row(ui, t!("Domyślny folder zapisu"), |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut s.out_subfolder).hint_text("web").desired_width(140.0))
                    .on_hover_text(t!("Podfolder tworzony obok zdjęć, gdy nie wybrano innego folderu przyciskiem „Folder”."));
                ui.colored_label(pal.text_dim, t!("podfolder obok zdjęć (np. web)"));
            });
        });
    });
    ui.add_space(10.0);
    if ui.button(t!("Przywróć domyślne (3000 × 3600, 3 MB)")).clicked() {
        s.reset_section(PrefsSection::Export);
    }
}

/// Retusz LaMa: opis, plik modelu, pobieranie.
fn retouch_page(ui: &mut Ui, s: &mut Settings, download: Option<&DownloadView>, result: &mut PrefsResult) {
    let pal = theme::p();
    ui.heading(t!("Retusz (LaMa)"));
    ui.colored_label(
        pal.text_dim,
        t!("Usuwa kurz, rysy, napisy, metki i drobne przedmioty: zaznacz je prostokątem (narzędzie M) i wybierz \
         „Usuń obiekt (LaMa)” w panelu albo pod prawym przyciskiem na zdjęciu. Model dorysowuje tło z otoczenia – \
         lokalnie, bez wysyłania zdjęcia i bez opłat."),
    );
    ui.add_space(4.0);
    ui.colored_label(
        pal.text_dim,
        t!("Najlepiej działa na małych obszarach na jednolitym albo powtarzalnym tle. Duże obszary i fragmenty samego \
         produktu mogą wyjść rozmyte – wtedy Cofnij nie pomoże, ale „Przywróć oryginał” wczyta plik od nowa."),
    );
    if !crate::ai::AVAILABLE {
        ui.colored_label(pal.warning, t!("Retusz LaMa działa lokalnie na Linux, Windows i macOS – w tej wersji jest niedostępny."));
    }
    ui.add_space(8.0);
    ui.checkbox(&mut s.lama_enabled, egui::RichText::new(t!("Używam retuszu LaMa")).strong())
        .on_hover_text(t!("Odznacz („nie używam”): znikają wszystkie opcje LaMa – zakładka w „Narzędziach AI”, przycisk przy zaznaczeniu i pozycja w menu zdjęcia."));
    ui.add_space(6.0);
    egui::Grid::new("prefs_lama").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        row(ui, t!("Model"), |ui| {
            ui.horizontal(|ui| {
                ui.label(t!("Big-LaMa (Apache-2.0), wejście 512 × 512, kilka sekund na procesorze"));
                ui.hyperlink_to(t!("strona modelu"), crate::settings::LAMA_PAGE);
            });
        });
        row(ui, t!("Plik modelu"), |ui| {
            ui.horizontal(|ui| {
                let default = crate::settings::default_models_dir().join(crate::settings::LAMA_FILE);
                ui.add(egui::TextEdit::singleline(&mut s.lama_path).hint_text(default.display().to_string()).desired_width(300.0));
                if crate::platform::CAN_PICK_FOLDER && ui.button(t!("Wybierz…")).clicked() {
                    result.pick_model = Some(ModelFile::Lama);
                }
            });
        });
        row(ui, "", |ui| {
            match std::fs::metadata(s.lama_model_path()) {
                Ok(m) if m.is_file() => {
                    ui.colored_label(pal.ok(), t!("✔ Plik jest ({mb} MB)", mb = format!("{:.0}", m.len() as f64 / 1_048_576.0)))
                }
                _ => ui.colored_label(pal.warning, t!("Brak pliku – pobierz model poniżej (ok. 208 MB) albo wskaż plik .onnx")),
            };
        });
        row(ui, t!("Pobieranie"), |ui| {
            ui.vertical(|ui| {
                match download {
                    Some(d) if d.model == ModelFile::Lama => {
                        let frac = d.total.map_or(0.0, |t| d.done as f32 / t.max(1) as f32);
                        let text = match d.total {
                            Some(t) => format!("{:.0} / {:.0} MB", d.done as f64 / 1_048_576.0, t as f64 / 1_048_576.0),
                            None => format!("{:.0} MB", d.done as f64 / 1_048_576.0),
                        };
                        ui.add(egui::ProgressBar::new(frac).desired_width(300.0).text(format!("LaMa · {text}")));
                    }
                    _ => {
                        if ui.add_enabled(crate::ai::AVAILABLE && download.is_none(), egui::Button::new(t!("Pobierz model"))).clicked() {
                            result.download = Some((ModelFile::Lama, s.lama_model_path(), String::new()));
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.small(t!("lub pobierz ręcznie:"));
                    ui.hyperlink_to(egui::RichText::new(crate::settings::LAMA_URL).small(), crate::settings::LAMA_URL);
                });
            });
        });
    });
    ui.add_space(10.0);
    if ui.button(t!("Przywróć domyślne")).clicked() {
        s.reset_section(PrefsSection::Retouch);
    }
}

/// waifu2x: opis, „Używam”, domyślne ustawienia, katalog modeli.
fn upscale_page(ui: &mut Ui, s: &mut Settings) {
    use crate::waifu2x::{MAX_INPUT_SIDE, NOISE_LEVELS, Style, models_dir, noise_label};
    let pal = theme::p();
    ui.heading(t!("Powiększanie (waifu2x)"));
    ui.colored_label(
        pal.text_dim,
        t!("waifu2x to sieć neuronowa autora nagadomi (model swin_unet z projektu nunif): powiększa 2× albo 4× z ostrymi \
         krawędziami, bez jasnych obwódek, i usuwa artefakty JPEG. Najlepiej na małych zdjęciach i czystych tłach – \
         np. zamiast dopełniać białym tłem do 500 px. Lokalnie, bez opłat."),
    );
    ui.add_space(4.0);
    ui.colored_label(
        pal.text_dim,
        t!(
            "Modele (ok. 20 MB na wariant) pobierają się same przy pierwszym użyciu z unlimited.waifu2x.net. \
             Dłuższy bok zdjęcia najwyżej {max} px; czas: kilka–kilkadziesiąt sekund.",
            max = MAX_INPUT_SIDE
        ),
    );
    if !crate::ai::AVAILABLE {
        ui.colored_label(pal.warning, t!("waifu2x działa lokalnie na Linux, Windows i macOS – w tej wersji jest niedostępny."));
    }
    ui.add_space(8.0);
    ui.checkbox(&mut s.waifu2x_enabled, egui::RichText::new(t!("Używam waifu2x")).strong())
        .on_hover_text(t!("Odznacz („nie używam”): znikają wszystkie opcje waifu2x – zakładka w „Narzędziach AI” i pozycja w menu zdjęcia."));
    ui.add_space(6.0);
    ui.add_enabled_ui(s.waifu2x_enabled, |ui| {
        egui::Grid::new("prefs_w2x").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
            row(ui, t!("Rodzaj"), |ui| {
                ui.horizontal(|ui| {
                    for style in Style::ALL {
                        ui.radio_value(&mut s.waifu2x_style, style, style.label());
                    }
                });
            });
            row(ui, t!("Skala"), |ui| {
                ui.horizontal(|ui| {
                    for scale in [2u32, 4] {
                        ui.radio_value(&mut s.waifu2x_scale, scale, format!("{scale}×"));
                    }
                });
            });
            row(ui, t!("Odszumianie"), |ui| {
                egui::ComboBox::from_id_salt("prefs_w2x_noise").selected_text(noise_label(s.waifu2x_noise)).show_ui(ui, |ui| {
                    for n in NOISE_LEVELS {
                        ui.selectable_value(&mut s.waifu2x_noise, n, noise_label(n));
                    }
                });
            });
            row(ui, t!("Modele"), |ui| {
                ui.vertical(|ui| {
                    let dir = models_dir();
                    let count = std::fs::read_dir(&dir).map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "onnx")).count()).unwrap_or(0);
                    ui.small(t!("{dir} (pobrane warianty: {count})", dir = dir.display(), count = count));
                    ui.horizontal_wrapped(|ui| {
                        ui.hyperlink_to(egui::RichText::new("unlimited.waifu2x.net").small(), "https://unlimited.waifu2x.net/");
                        ui.small("·");
                        ui.hyperlink_to(egui::RichText::new(t!("projekt nunif (MIT)")).small(), "https://github.com/nagadomi/nunif");
                    });
                });
            });
        });
    });
    ui.add_space(10.0);
    if ui.button(t!("Przywróć domyślne")).clicked() {
        s.reset_section(PrefsSection::Upscale);
    }
}

/// Replicate: jeden przełącznik dla wszystkich płatnych funkcji, klucz i model edycji.
fn replicate_page(ui: &mut Ui, s: &mut Settings, result: &mut PrefsResult) {
    let pal = theme::p();
    ui.heading(t!("Replicate (edycja AI online)"));
    ui.colored_label(
        pal.text_dim,
        t!("Edycja zdjęcia poleceniem (np. białe tło z cieniem, usunięcie napisów). Płatne za każdy obraz z kredytu \
         konta Replicate; zdjęcie jest wysyłane na serwery Replicate (dane zapytań kasowane po godzinie)."),
    );
    ui.add_space(8.0);
    ui.checkbox(&mut s.replicate_enabled, egui::RichText::new(t!("Włącz funkcje Replicate")).strong())
        .on_hover_text(t!("Wyłączone: znikają wszystkie płatne przyciski Replicate (panel „Edycja AI” i menu zdjęcia)."));
    if !crate::ai::REPLICATE_AVAILABLE {
        ui.colored_label(
            pal.warning,
            t!("W przeglądarce Replicate nie działa – blokuje zapytania ze stron WWW. Użyj wersji na komputer albo Androida."),
        );
    }
    ui.add_space(6.0);
    ui.add_enabled_ui(s.replicate_enabled, |ui| {
        egui::Grid::new("prefs_replicate").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
            row(ui, t!("Klucz API"), |ui| {
                ui.vertical(|ui| {
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut s.replicate_key)
                            .password(true)
                            .hint_text("r8_…")
                            .desired_width(300.0),
                    );
                    native_text(ui, &field, crate::platform::TextTarget::ReplicateKey, &s.replicate_key, result);
                    ui.horizontal_wrapped(|ui| {
                        ui.hyperlink_to(egui::RichText::new(t!("utwórz klucz")).small(), "https://replicate.com/account/api-tokens");
                        ui.small("·");
                        ui.hyperlink_to(egui::RichText::new(t!("kredyt i rozliczenia")).small(), "https://replicate.com/account/billing");
                    });
                    let key = s.replicate_api_key();
                    if key.is_empty() {
                        ui.colored_label(pal.warning, egui::RichText::new(t!("brak klucza – funkcje Replicate są ukryte")).small());
                    } else if s.replicate_key.trim().is_empty() {
                        ui.colored_label(
                            pal.ok(),
                            egui::RichText::new(t!(
                                "✔ klucz {key} – z REPLICATE_API_TOKEN albo ~/.config/asone/replicate-token",
                                key = crate::settings::masked_key(&key)
                            ))
                            .small(),
                        );
                    } else {
                        ui.colored_label(pal.ok(), egui::RichText::new(t!("✔ zapisany klucz {key}", key = crate::settings::masked_key(&key))).small());
                    }
                    ui.small(t!("Klucz wpisany tutaj jest zapisywany tylko lokalnie, w ustawieniach AsOne."));
                });
            });
            row(ui, t!("Model"), |ui| {
                ui.vertical(|ui| {
                    for model in EditModel::ALL {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut s.replicate_model, model, t!("{model} – {price} za obraz", model = model.label(), price = model.price()));
                            ui.hyperlink_to(egui::RichText::new("strona").small(), model.page_url());
                        });
                    }
                    ui.small(t!("Ceny z cennika Replicate z 20.11.2026; wyniki wolno używać komercyjnie (strona modelu)."));
                });
            });
            row(ui, t!("Wysyłane"), |ui| {
                ui.vertical(|ui| {
                    ui.small(t!("obraz z zakładki (kadr, marginesy, smartfon) jako JPEG do 1536 px"));
                    ui.small(t!("wynik (ok. 1 Mpx) zastępuje zdjęcie w zakładce – „Przywróć oryginał” wraca do pliku"));
                });
            });
        });
    });
    ui.add_space(10.0);
    if ui.button(t!("Przywróć domyślne")).clicked() {
        s.reset_section(PrefsSection::Replicate);
    }
}

/// RMBG-3.0 online (fal.ai): koszt, klucz API, co jest wysyłane.
fn online_rows(ui: &mut Ui, s: &mut Settings, result: &mut PrefsResult) {
    let pal = theme::p();
    row(ui, t!("Wymagania"), |ui| {
        ui.vertical(|ui| {
            ui.colored_label(pal.warning, t!("internet · konto fal.ai z doładowanymi kredytami"));
            ui.horizontal_wrapped(|ui| {
                ui.small(t!("0,018 USD za zdjęcie (cennik fal.ai z 20.11.2026) –"));
                ui.hyperlink_to(egui::RichText::new(t!("aktualna cena")).small(), s.ai_model.page_url());
            });
            ui.small(t!("kilka–kilkanaście sekund na zdjęcie (wysyłanie + model na serwerze)"));
        });
    });
    row(ui, t!("Klucz fal.ai"), |ui| {
        ui.vertical(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut s.fal_key)
                    .password(true)
                    .hint_text(t!("klucz API fal.ai (albo zmienna FAL_KEY)"))
                    .desired_width(300.0),
            );
            native_text(ui, &field, crate::platform::TextTarget::FalKey, &s.fal_key, result);
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to(egui::RichText::new(t!("utwórz klucz")).small(), "https://fal.ai/dashboard/keys");
                ui.small("·");
                ui.hyperlink_to(egui::RichText::new(t!("saldo i zużycie")).small(), "https://fal.ai/dashboard/usage-billing");
                ui.small("·");
                ui.hyperlink_to(egui::RichText::new(t!("historia zapytań")).small(), "https://fal.ai/dashboard/requests");
            });
            ui.small(t!("Klucz jest zapisywany tylko lokalnie, w ustawieniach AsOne."));
            let key = s.fal_api_key();
            if !key.is_empty() {
                let from = if s.fal_key.trim().is_empty() { t!(" – ze zmiennej FAL_KEY") } else { "" };
                ui.colored_label(pal.ok(), egui::RichText::new(format!("{}{from}", t!("✔ zapisany klucz {key}", key = crate::settings::masked_key(&key)))).small());
            }
        });
    });
    row(ui, t!("Licencja"), |ui| {
        ui.vertical(|ui| {
            ui.colored_label(pal.ok(), t!("do zastosowań komercyjnych"));
            ui.horizontal_wrapped(|ui| {
                ui.small(t!("fal.ai: „Commercial use” – model trenowany wyłącznie na licencjonowanych danych –"));
                ui.hyperlink_to(egui::RichText::new(t!("warunki na stronie modelu")).small(), s.ai_model.page_url());
            });
        });
    });
    row(ui, t!("Wysyłane"), |ui| {
        ui.vertical(|ui| {
            ui.small(t!(
                "zdjęcie jako JPEG (dłuższy bok do {side} px, bez metadanych) do fal.ai, model bria/rmbg-3",
                side = crate::ai::FAL_UPLOAD_SIDE
            ));
            ui.small(t!("wynik: maska przezroczystości w rozmiarze zdjęcia – dalej jak przy modelach lokalnych"));
        });
    });
}

fn ai_page(ui: &mut Ui, s: &mut Settings, download: Option<&DownloadView>, result: &mut PrefsResult) {
    let pal = theme::p();
    ui.heading(t!("Usuwanie tła (AI)"));
    if s.ai_model.is_online() {
        ui.colored_label(pal.warning, t!("RMBG-3.0 online: zdjęcie jest wysyłane do fal.ai, każde usunięcie tła jest płatne."));
    } else {
        ui.colored_label(pal.text_dim, t!("Modele BRIA RMBG-1.4 i 2.0 działają lokalnie – zdjęcia nie są nigdzie wysyłane."));
    }
    if !crate::ai::AVAILABLE {
        ui.colored_label(
            pal.warning,
            t!("Na tej platformie działa model online RMBG-3.0 (fal.ai); modele lokalne – tylko na Linux, Windows i macOS."),
        );
        // Model lokalny i tak by tu nie zadziałał – w szkicu od razu model online (zapis dopiero po OK).
        if !s.ai_model.is_online() {
            s.ai_model = AiModel::Rmbg30Fal;
        }
    }
    ui.add_space(6.0);
    egui::Grid::new("prefs_ai").num_columns(2).spacing(vec2(14.0, 8.0)).show(ui, |ui| {
        row(ui, t!("Model"), |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("ai_model")
                    .width(220.0)
                    .selected_text(s.ai_model.label())
                    .show_ui(ui, |ui| {
                        for m in AiModel::ALL.into_iter().filter(|m| m.is_online() || crate::ai::AVAILABLE) {
                            ui.selectable_value(&mut s.ai_model, m, m.label());
                        }
                    });
                ui.hyperlink_to(t!("strona modelu"), s.ai_model.page_url());
            });
        });
        if s.ai_model.is_online() {
            online_rows(ui, s, result);
            return;
        }
        row(ui, t!("Wymagania"), |ui| {
            let (weights, run) = crate::ai::memory_need_gb(s.ai_model);
            let (text, color) = match s.ai_model {
                AiModel::Rmbg14 => (t!("ok. {gb} GB RAM · ok. 1 s na zdjęcie", gb = format!("{:.0}", weights + run)), pal.text_dim),
                AiModel::Rmbg20 => (
                    t!("ok. {gb} GB wolnej pamięci RAM · 15–20 s na zdjęcie (procesor)", gb = format!("{:.0}", (weights + run).ceil())),
                    pal.warning,
                ),
                AiModel::Rmbg30Fal => (String::new(), pal.text_dim),
            };
            ui.vertical(|ui| {
                ui.colored_label(color, text);
                if let Some(free) = crate::ai::free_memory_bytes() {
                    ui.small(t!("teraz wolne: {gb} GB (RAM + swap)", gb = format!("{:.1}", free as f64 / (1u64 << 30) as f64)));
                }
            });
        });
        row(ui, t!("Plik modelu"), |ui| {
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
                if crate::platform::CAN_PICK_FOLDER && ui.button(t!("Wybierz…")).clicked() {
                    result.pick_model = Some(ModelFile::Rmbg(s.ai_model));
                }
            });
        });
        row(ui, "", |ui| {
            let path = s.ai_model_path();
            match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => {
                    ui.colored_label(pal.ok(), t!("✔ Plik jest ({mb} MB)", mb = format!("{:.0}", m.len() as f64 / 1_048_576.0)))
                }
                _ => ui.colored_label(pal.warning, t!("Brak pliku – pobierz model poniżej albo wskaż istniejący plik .onnx")),
            };
        });
        row(ui, t!("Pobieranie"), |ui| {
            ui.vertical(|ui| {
                match download {
                    Some(d) if matches!(d.model, ModelFile::Rmbg(_)) => {
                        let frac = d.total.map_or(0.0, |t| d.done as f32 / t.max(1) as f32);
                        let text = match d.total {
                            Some(t) => format!("{:.0} / {:.0} MB", d.done as f64 / 1_048_576.0, t as f64 / 1_048_576.0),
                            None => format!("{:.0} MB", d.done as f64 / 1_048_576.0),
                        };
                        ui.add(egui::ProgressBar::new(frac).desired_width(300.0).text(format!("{} · {text}", d.model.label())));
                    }
                    _ => {
                        let needs_token = s.ai_model.needs_token() && s.hf_token.trim().is_empty();
                        let button = ui.add_enabled(crate::ai::AVAILABLE && !needs_token, egui::Button::new(t!("Pobierz model")));
                        let button = if needs_token {
                            button.on_disabled_hover_text(t!("RMBG-2.0 wymaga tokenu Hugging Face (poniżej)"))
                        } else {
                            button
                        };
                        if button.clicked() {
                            result.download = Some((ModelFile::Rmbg(s.ai_model), s.ai_model_path(), s.hf_token.clone()));
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.small(t!("lub pobierz ręcznie:"));
                    ui.hyperlink_to(egui::RichText::new(s.ai_model.download_url()).small(), s.ai_model.download_url());
                });
            });
        });
        row(ui, t!("Token Hugging Face"), |ui| {
            ui.vertical(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut s.hf_token)
                        .password(true)
                        .hint_text(t!("hf_…  (potrzebny tylko dla RMBG-2.0)"))
                        .desired_width(300.0),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.small(t!("klucz (token) po rejestracji na huggingface.co jest darmowy –"));
                    ui.hyperlink_to(egui::RichText::new(t!("utwórz token (Read)")).small(), "https://huggingface.co/settings/tokens");
                });
                ui.small(t!("dla RMBG-2.0 zaakceptuj też licencję na stronie modelu. Token jest zapisywany tylko lokalnie."));
            });
        });
    });
    ui.add_space(8.0);
    let note = if s.ai_model.is_online() {
        t!("RMBG-3.0 przez fal.ai: także do zastosowań komercyjnych; zapytania rozlicza fal.ai (przedpłacone kredyty).")
    } else {
        t!("Lokalne modele BRIA RMBG-1.4 i 2.0 są darmowe do użytku niekomercyjnego; do komercyjnego potrzebna jest umowa \
         z BRIA – albo RMBG-3.0 przez fal.ai.")
    };
    ui.colored_label(pal.text_dim, egui::RichText::new(note).small());
    ui.add_space(6.0);
    if ui.button(t!("Przywróć domyślne")).clicked() {
        s.reset_section(PrefsSection::Ai);
    }
}
