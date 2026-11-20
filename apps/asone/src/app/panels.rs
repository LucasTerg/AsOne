//! Strefy okna: górny pasek eksportu, pasek narzędzi, raport eksportu, okna dialogowe, nakładki.

use std::time::Duration;

use asone_core::naming::slugify;
use egui::{Align2, Color32, CornerRadius, RichText, Ui, vec2};

use super::{AboutTab, AsOneApp, ReportEntry};

/// Statystyki z `git log` (generuje `build.rs`).
mod git_stats {
    pub struct ModelStat {
        pub company: &'static str,
        pub model: &'static str,
        pub commits: u32,
        pub added: u64,
        pub removed: u64,
        pub first: &'static str,
        pub last: &'static str,
    }
    include!(concat!(env!("OUT_DIR"), "/git_stats.rs"));
}

/// Współtwórcy w „O programie”: (login GitHub, imię, rola / wkład).
/// Dopisujemy tylko osoby z realnym wkładem – np. przyjętym pull requestem – nigdy „na zapas”.
const CONTRIBUTORS: &[(&str, &str, &str)] = &[("LucasTerg", "Lucas", "Twórca i główny deweloper")];

/// Tekst w jednej linii (bez łamania) – kolumna i okno poszerzają się do jego długości.
fn one_line(ui: &mut Ui, text: RichText) {
    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Extend));
}

/// 12345 → „12 345”.
fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    out
}
use crate::canvas::Tool;
use crate::document::DocId;
use crate::platform;
use crate::theme;

impl AsOneApp {
    pub(super) fn top_bar(&mut self, ui: &mut Ui, open: &[DocId]) {
        let pal = theme::p();
        let frame = egui::Frame::new()
            .fill(pal.chrome)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .stroke(egui::Stroke::new(1.0, pal.separator));
        egui::Panel::top("top_bar").frame(frame).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Wklej nazwę:");
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.base_name)
                        .hint_text("np. 123-marka model")
                        .desired_width(210.0),
                );
                // Android: klawiatura systemowa w oknie edycji (GameActivity nie podaje tekstu do egui).
                #[cfg(target_os = "android")]
                if field.gained_focus() {
                    crate::android::edit_text("Wklej nazwę", &self.base_name);
                    ui.memory_mut(|m| m.surrender_focus(field.id));
                }
                field.context_menu(|ui| {
                    if platform::CAN_PASTE && ui.button("Wklej").clicked() {
                        self.pickers.paste_text();
                        ui.close();
                    }
                    if ui.button("Kopiuj").clicked() {
                        ui.ctx().copy_text(self.base_name.clone());
                        ui.close();
                    }
                    if ui.button("Wytnij").clicked() {
                        ui.ctx().copy_text(std::mem::take(&mut self.base_name));
                        ui.close();
                    }
                    if ui.button("Wyczyść").clicked() {
                        self.base_name.clear();
                        ui.close();
                    }
                });
                if platform::CAN_PASTE
                    && ui.button("Wklej").on_hover_text("Wkleja nazwę ze schowka (zastępuje obecną)").clicked()
                {
                    self.pickers.paste_text();
                }
                if !self.base_name.is_empty() && ui.small_button("×").on_hover_text("Wyczyść").clicked() {
                    self.base_name.clear();
                }
                ui.label("Start nr:");
                ui.add(egui::DragValue::new(&mut self.settings.start_number).range(1..=99_999))
                    .on_hover_text("Numer pierwszego zdjęcia w paczce");
                let slug = slugify(&self.base_name);
                let first = self.settings.start_number.max(1);
                let preview = if slug.is_empty() { "» …".to_string() } else { format!("» {slug}-{first}.jpg") };
                ui.colored_label(pal.text_dim, preview);
                ui.separator();

                ui.label("Jakość");
                ui.add(egui::Slider::new(&mut self.settings.quality, 1..=100).suffix(" %"));
                ui.checkbox(&mut self.settings.limit_long_side, "Max dłuższy bok");
                ui.add_enabled(
                    self.settings.limit_long_side,
                    egui::DragValue::new(&mut self.settings.max_long_side).range(16..=16384).suffix(" px"),
                );

                // Folder + „Zmień tylko nazwę” + „Zapisz z zakładek” to jedna grupa przy prawej krawędzi;
                // gdy nie mieści się w bieżącym wierszu, cała przechodzi do następnego (zamiast nachodzić).
                let group_id = ui.id().with("top_actions_width");
                let need = ui.ctx().data(|d| d.get_temp::<f32>(group_id)).unwrap_or(460.0);
                // W układzie zawijanym available_width() to cały wiersz – liczy się reszta bieżącego wiersza.
                if ui.available_rect_before_wrap().width() < need + 12.0 {
                    ui.end_row();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(job) = &self.export {
                        let done = job.entries.len();
                        ui.add(
                            egui::ProgressBar::new(done as f32 / job.total.max(1) as f32)
                                .desired_width(170.0)
                                .text(format!("Zapisywanie {done}/{}", job.total)),
                        );
                    } else {
                        let blocker = self.export_blocker(open);
                        let button = egui::Button::new(
                            RichText::new(format!("Zapisz z zakładek ({})", open.len())).strong().color(pal.accent_text),
                        )
                        .fill(pal.accent)
                        .min_size(vec2(170.0, 26.0));
                        let response = ui.add_enabled(blocker.is_none(), button);
                        let limits = self.settings.limits();
                        let limit_info = match (limits.max_width, limits.max_bytes) {
                            (Some(w), Some(b)) => format!(
                                "\nLimity: {w}×{} px, {:.1} MB (Ustawienia › Preferencje › Eksport)",
                                limits.max_height.unwrap_or(w),
                                b as f64 / 1_048_576.0
                            ),
                            _ => String::new(),
                        };
                        let response = match blocker {
                            Some(reason) => response.on_disabled_hover_text(reason),
                            None => response.on_hover_text(format!(
                                "Eksport {} zdjęć w kolejności zakładek ({}, Progressive JPEG, bez metadanych) · Ctrl+S{limit_info}",
                                open.len(),
                                asone_core::jpeg::ENCODER_NAME
                            )),
                        };
                        if response.clicked() {
                            let ctx = ui.ctx().clone();
                            self.start_export(&ctx, open);
                        }
                        if platform::FILE_OPS {
                            let blocker = self.rename_blocker(open);
                            let response = ui.add_enabled(blocker.is_none(), egui::Button::new("Zmień tylko nazwę").min_size(vec2(0.0, 26.0)));
                            let response = match blocker {
                                Some(reason) => response.on_disabled_hover_text(reason),
                                None => response.on_hover_text(format!(
                                    "Zmienia nazwy {} plików na dysku (w ich folderach) według wklejonej nazwy i kolejności zakładek – \
                                     bez zapisu nowych JPG. Najpierw pokaże listę zmian.",
                                    open.len()
                                )),
                            };
                            if response.clicked() {
                                self.prepare_rename(open);
                            }
                        }
                    }
                    if platform::CAN_PICK_FOLDER {
                        // Ikonka obok „Folder:” – powrót do domyślnego podfolderu obok zdjęć.
                        let sub = self.settings.out_subfolder_name();
                        let image = egui::Image::new(egui::include_image!("../../assets/icons/folder-default.svg"))
                            .fit_to_exact_size(vec2(16.0, 16.0))
                            .tint(pal.icon);
                        let reset = egui::Button::image(image).min_size(vec2(26.0, 26.0));
                        let custom = self.out_dir.is_some();
                        let response = ui.add_enabled(custom, reset);
                        let response = if custom {
                            response.on_hover_text(format!("Przywróć domyślny folder: „{sub}” obok zdjęć"))
                        } else {
                            response.on_disabled_hover_text(format!(
                                "Zapis do domyślnego folderu „{sub}” obok zdjęć (nazwa: Ustawienia › Preferencje › Eksport)"
                            ))
                        };
                        if response.clicked() {
                            self.out_dir = None;
                        }
                        let dir = self.default_out_dir(open);
                        let label = dir
                            .as_ref()
                            .and_then(|d| d.file_name())
                            .map(|n| format!("Folder: {}", n.to_string_lossy()))
                            .unwrap_or_else(|| "Folder: …".into());
                        let hover = dir
                            .as_ref()
                            .map(|d| format!("Zapis do: {}\nKliknij, aby zmienić", d.display()))
                            .unwrap_or_else(|| "Domyślnie podfolder obok zdjęć".into());
                        if ui.button(label).on_hover_text(hover).clicked() {
                            self.pickers.pick_output_folder(dir);
                        }
                    } else if cfg!(target_os = "android") {
                        ui.colored_label(pal.text_dim, "Zapis: Galeria › Pictures/AsOne");
                    } else if platform::EXPORT_TO_FOLDER {
                        let dir = self.out_dir.clone().unwrap_or_default();
                        ui.colored_label(pal.text_dim, format!("Zapis: {}", dir.display()));
                    } else {
                        ui.colored_label(pal.text_dim, "Zapis: pobierane pliki");
                    }
                    let width = ui.min_rect().width();
                    ui.ctx().data_mut(|d| d.insert_temp(group_id, width));
                });
            });
        });
    }

    pub(super) fn tool_bar(&mut self, ui: &mut Ui) {
        let pal = theme::p();
        let frame = egui::Frame::new().fill(pal.chrome).inner_margin(egui::Margin::symmetric(5, 8));
        egui::Panel::left("tool_bar").exact_size(46.0).resizable(false).frame(frame).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                for tool in Tool::ALL {
                    let icon = match tool {
                        Tool::Move => egui::include_image!("../../assets/icons/move.svg"),
                        Tool::Crop => egui::include_image!("../../assets/icons/crop.svg"),
                        Tool::Eraser => egui::include_image!("../../assets/icons/eraser.svg"),
                        Tool::RectCut => egui::include_image!("../../assets/icons/marquee.svg"),
                    };
                    let selected = self.tool == tool;
                    let image = egui::Image::new(icon)
                        .fit_to_exact_size(vec2(20.0, 20.0))
                        .tint(if selected { pal.accent_text } else { pal.icon });
                    let button = egui::Button::image(image)
                        .selected(selected)
                        .min_size(vec2(34.0, 34.0))
                        .corner_radius(CornerRadius::same(pal.radius));
                    let hint = format!("{} ({})", tool.label(), tool.key().name());
                    if ui.add(button).on_hover_text(hint).clicked() {
                        self.tool = tool;
                    }
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                let history = self.active_doc.and_then(|id| self.docs.get(&id)).map(|d| &d.history);
                let (can_undo, can_redo) = (history.is_some_and(|h| h.can_undo()), history.is_some_and(|h| h.can_redo()));
                for (redo, icon, enabled, hint) in [
                    (false, egui::include_image!("../../assets/icons/undo.svg"), can_undo, "Cofnij (Ctrl+Z)"),
                    (true, egui::include_image!("../../assets/icons/redo.svg"), can_redo, "Ponów (Ctrl+Shift+Z)"),
                ] {
                    let image = egui::Image::new(icon).fit_to_exact_size(vec2(20.0, 20.0)).tint(pal.icon);
                    let button = egui::Button::image(image).min_size(vec2(34.0, 34.0)).corner_radius(CornerRadius::same(pal.radius));
                    if ui.add_enabled(enabled, button).on_hover_text(hint).clicked()
                        && let Some(doc) = self.active_doc.and_then(|id| self.docs.get_mut(&id))
                    {
                        doc.step_history(redo);
                    }
                }
            });
        });
    }

    pub(super) fn report_window(&mut self, ctx: &egui::Context) {
        let Some(report) = &self.report else { return };
        let pal = theme::p();
        let mut open = true;
        let (mut close, mut zip) = (false, false);
        egui::Window::new("Eksport zakończony")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .movable(true)
            .default_pos(ctx.content_rect().center() - vec2(260.0, 200.0))
            .default_width(520.0)
            .show(ctx, |ui| {
                let saved = report.entries.iter().filter(|e| matches!(e, ReportEntry::Saved { .. })).count();
                let total_bytes: u64 = report
                    .entries
                    .iter()
                    .map(|e| if let ReportEntry::Saved { bytes, .. } = e { *bytes } else { 0 })
                    .sum();
                ui.label(format!(
                    "Zapisano {saved} z {} plików · {:.0} KB · {:.1} s",
                    report.entries.len(),
                    total_bytes as f64 / 1024.0,
                    report.elapsed.as_secs_f64()
                ));
                if let Some(dir) = &report.out_dir {
                    ui.colored_label(pal.text_dim, dir.display().to_string());
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    egui::Grid::new("report").num_columns(3).striped(true).show(ui, |ui| {
                        for e in &report.entries {
                            match e {
                                ReportEntry::Saved { name, bytes, size, quality, notes } => {
                                    ui.colored_label(pal.ok(), "✔");
                                    ui.vertical(|ui| {
                                        one_line(ui, RichText::new(name));
                                        for n in notes {
                                            one_line(ui, RichText::new(format!("↳ {n}")).color(pal.warning));
                                        }
                                    });
                                    one_line(
                                        ui,
                                        RichText::new(format!(
                                            "{}×{} · {:.0} KB · jakość {quality}",
                                            size[0],
                                            size[1],
                                            *bytes as f64 / 1024.0
                                        )),
                                    );
                                }
                                ReportEntry::Failed { name, error } => {
                                    ui.colored_label(pal.danger, "✖");
                                    one_line(ui, RichText::new(name));
                                    one_line(ui, RichText::new(error).color(pal.danger));
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if let Some(dir) = report.out_dir.as_ref().filter(|_| platform::CAN_PICK_FOLDER) {
                        if ui.button("Otwórz folder").clicked() {
                            platform::open_in_file_manager(dir);
                        }
                        if saved > 0 && ui.button("Spakuj do ZIP…").clicked() {
                            zip = true;
                        }
                    }
                    if ui.button("Zamknij").clicked() {
                        close = true;
                    }
                });
            });
        if zip {
            self.zip_last_export();
        }
        if !open || close {
            self.report = None;
        }
    }

    pub(super) fn dialogs_ui(&mut self, ctx: &egui::Context) {
        let pal = theme::p();
        if let Some(ids) = self.dialogs.delete.clone() {
            let mut open = true;
            let mut choice = None;
            egui::Window::new("Usuń z dysku")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .movable(true)
                .default_pos(ctx.content_rect().center() - vec2(220.0, 150.0))
                .show(ctx, |ui| {
                    ui.colored_label(pal.danger, RichText::new("Ostrzeżenie").strong());
                    ui.label(format!("Pliki ({}) zostaną usunięte z dysku i z listy:", ids.len()));
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for id in &ids {
                            if let Some(p) = self.docs.get(id).and_then(|d| d.source.path()) {
                                ui.monospace(p.display().to_string());
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Przenieś do kosza").clicked() {
                            choice = Some(true);
                        }
                        let danger = egui::Button::new(RichText::new("Usuń trwale (nieodwracalne)").color(Color32::WHITE))
                            .fill(pal.danger);
                        if ui.add(danger).clicked() {
                            choice = Some(false);
                        }
                        if ui.button("Anuluj").clicked() {
                            self.dialogs.delete = None;
                        }
                    });
                });
            if let Some(trash) = choice {
                self.dialogs.delete = None;
                self.delete_from_disk(&ids, trash);
            }
            if !open {
                self.dialogs.delete = None;
            }
        }

        if let Some(plans) = self.dialogs.rename.clone() {
            let mut open = true;
            let mut confirm = false;
            let changes = plans.iter().filter(|(_, p)| !p.is_noop()).count();
            egui::Window::new("Zmień tylko nazwę")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(560.0)
                .default_pos(ctx.content_rect().center() - vec2(280.0, 180.0))
                .show(ctx, |ui| {
                    ui.label(format!("Pliki na dysku dostaną nowe nazwy ({changes} z {}). Zawartość i folder się nie zmieniają.", plans.len()));
                    ui.colored_label(pal.text_dim, "Istniejące pliki o tych nazwach nie zostaną nadpisane.");
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        egui::Grid::new("rename_preview").num_columns(3).striped(true).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                            for (_, p) in &plans {
                                let name = |path: &std::path::Path| path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                                ui.monospace(name(&p.from));
                                ui.label("»");
                                if p.is_noop() {
                                    ui.colored_label(pal.text_dim, "bez zmian");
                                } else {
                                    ui.colored_label(pal.accent, RichText::new(name(&p.to)).monospace());
                                }
                                ui.end_row();
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let ok = egui::Button::new(RichText::new(format!("Zmień nazwy ({changes})")).color(pal.accent_text)).fill(pal.accent);
                        if ui.add_enabled(changes > 0, ok).clicked() {
                            confirm = true;
                        }
                        if ui.button("Anuluj").clicked() {
                            self.dialogs.rename = None;
                        }
                    });
                });
            if confirm {
                self.dialogs.rename = None;
                self.apply_rename(plans);
            }
            if !open {
                self.dialogs.rename = None;
            }
        }

        if self.dialogs.clear_list {
            let mut open = true;
            let mut confirm = false;
            let n = self.order.len();
            egui::Window::new("Wyczyść listę")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .movable(true)
                .default_pos(ctx.content_rect().center() - vec2(200.0, 120.0))
                .show(ctx, |ui| {
                    ui.label(format!("Usunąć z listy wszystkie zdjęcia ({n})?"));
                    ui.colored_label(pal.text_dim, "Pliki na dysku zostają; tracisz tylko ustawienia kadru, marginesów i maski.");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Wyczyść listę").clicked() {
                            confirm = true;
                        }
                        if ui.button("Anuluj").clicked() {
                            self.dialogs.clear_list = false;
                        }
                    });
                });
            if confirm {
                self.dialogs.clear_list = false;
                self.clear_list();
            }
            if !open {
                self.dialogs.clear_list = false;
            }
        }

        if self.dialogs.shortcuts {
            let mut open = true;
            egui::Window::new("Skróty klawiszowe")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .movable(true)
                .default_pos(ctx.content_rect().center() - vec2(280.0, 200.0))
                .show(ctx, |ui| {
                    egui::Grid::new("shortcuts").num_columns(2).striped(true).show(ui, |ui| {
                        for (k, v) in [
                            ("H / C / E / M", "Przesuwanie / Kadrowanie / Gumka / Wycinanie prostokątem"),
                            ("I", "Inspekcja tła"),
                            ("Kółko myszy", "Zoom zdjęcia w punkt kursora"),
                            ("Spacja + LPM, środkowy", "Przesuwanie widoku"),
                            ("Ctrl+0 / Ctrl+1", "Dopasuj zdjęcie / 100 %"),
                            ("Ctrl++ / Ctrl+−", "Skala interfejsu"),
                            ("[ ]", "Rozmiar pędzla"),
                            ("LPM / PPM (Alt+LPM)", "Gumka: usuń / przywróć"),
                            ("Delete / Shift+Delete / Esc", "Wytnij / przywróć zaznaczenie, odznacz"),
                            ("Enter, dwuklik", "Zatwierdź kadr"),
                            ("Ctrl/Shift+klik", "Zaznaczanie na liście plików"),
                            ("Ctrl+O / Ctrl+S / Ctrl+Q", "Dodaj zdjęcia / Zapisz z zakładek / Zakończ"),
                        ] {
                            ui.monospace(k);
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                });
            self.dialogs.shortcuts = open;
        }

        if self.dialogs.about {
            let mut open = true;
            egui::Window::new("O programie AsOne")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .movable(true)
                .default_size(vec2(660.0, 420.0))
                .min_size(vec2(540.0, 320.0))
                .default_pos(ctx.content_rect().center() - vec2(330.0, 210.0))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        // Pasek zakładek okna
                        ui.horizontal(|ui| {
                            for (tab, label) in [
                                (AboutTab::About, "O programie"),
                                (AboutTab::Contributors, "Współtwórcy"),
                                (AboutTab::Models, "Modele"),
                            ] {
                                if ui.add_sized(vec2(120.0, 26.0), egui::Button::selectable(self.dialogs.about_tab == tab, label)).clicked() {
                                    self.dialogs.about_tab = tab;
                                }
                            }
                        });
                        ui.separator();
                        ui.add_space(4.0);

                        match self.dialogs.about_tab {
                            AboutTab::About => {
                                ui.heading(format!("AsOne {}", env!("CARGO_PKG_VERSION")));
                                ui.add_space(4.0);
                                ui.label("Zdjęcia produktowe do internetu: kadr, marginesy, białe tło, limity, usuwanie tła przez modele AI (BRIA RMBG) i Progressive JPEG bez metadanych.");
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Wersja:").strong());
                                    ui.monospace(env!("CARGO_PKG_VERSION"));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Kod źródłowy:").strong());
                                    if ui.link("https://github.com/LucasTerg/AsOne").on_hover_text("Otwórz repozytorium GitHub").clicked() {
                                        crate::platform::open_url("https://github.com/LucasTerg/AsOne");
                                    }
                                });
                                ui.add_space(6.0);
                                ui.separator();
                                ui.add_space(4.0);
                                ui.label(RichText::new("Komponenty systemowe:").strong());
                                ui.colored_label(pal.text_dim, format!("• Kompresja: {}", asone_core::jpeg::ENCODER_NAME));
                                ui.colored_label(pal.text_dim, "• Silnik AI: ONNX Runtime (ort) – RMBG 1.4 & 2.0 BiRefNet");
                                if let Some(g) = &self.gpu {
                                    ui.colored_label(pal.text_dim, format!("• Akceleracja graficzna: wgpu (maks. tekstura GPU: {} px)", g.max_texture_dim));
                                }
                            }
                            AboutTab::Contributors => {
                                ui.heading("Współtwórcy");
                                ui.add_space(4.0);
                                ui.colored_label(
                                    pal.text_dim,
                                    "Twórca projektu i osoby, które wniosły do AsOne realny wkład – np. przyjętym pull requestem:",
                                );
                                ui.add_space(8.0);
                                egui::Grid::new("about_contributors").striped(true).num_columns(3).spacing(vec2(20.0, 10.0)).show(ui, |ui| {
                                    ui.label(RichText::new("Użytkownik").strong());
                                    ui.label(RichText::new("Imię").strong());
                                    ui.label(RichText::new("Rola / Wkład").strong());
                                    ui.end_row();
                                    for (login, name, role) in CONTRIBUTORS {
                                        let url = format!("https://github.com/{login}");
                                        if ui.link(format!("@{login}")).on_hover_text(format!("GitHub: {login}")).clicked() {
                                            crate::platform::open_url(&url);
                                        }
                                        ui.label(*name);
                                        ui.label(*role);
                                        ui.end_row();
                                    }
                                });
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.label("Chcesz pomóc?");
                                    if ui.link("Pull requesty na GitHubie").clicked() {
                                        crate::platform::open_url("https://github.com/LucasTerg/AsOne/pulls");
                                    }
                                });
                            }
                            AboutTab::Models => {
                                use git_stats::{MODEL_STATS, STATS_HEAD, TOTAL_COMMITS};
                                ui.heading("Modele AI");
                                ui.add_space(4.0);
                                ui.colored_label(pal.text_dim, "Udział modeli AI w kodzie – wyliczone z git log przy budowaniu programu:");
                                ui.add_space(8.0);
                                if MODEL_STATS.is_empty() {
                                    ui.colored_label(pal.warning, "Brak danych (program zbudowany bez repozytorium git).");
                                } else {
                                    egui::ScrollArea::horizontal().show(ui, |ui| {
                                        egui::Grid::new("about_models").striped(true).num_columns(6).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
                                            for h in ["Firma", "Model", "Commity", "% commitów", "Wiersze +/−", "Okres"] {
                                                ui.label(RichText::new(h).strong());
                                            }
                                            ui.end_row();
                                            for m in MODEL_STATS {
                                                ui.label(m.company);
                                                ui.label(m.model);
                                                ui.label(m.commits.to_string());
                                                ui.label(format!("{:.0} %", 100.0 * m.commits as f64 / TOTAL_COMMITS.max(1) as f64));
                                                ui.colored_label(pal.ok(), format!("+{} / −{}", group(m.added), group(m.removed)));
                                                ui.label(if m.first == m.last { m.first.to_string() } else { format!("{} – {}", m.first, m.last) });
                                                ui.end_row();
                                            }
                                        });
                                    });
                                    ui.add_space(8.0);
                                    ui.small(format!(
                                        "Wszystkich commitów: {TOTAL_COMMITS} · ostatni: {STATS_HEAD}. Model = stopka „Co-Authored-By” commita \
                                         lub wpis w ai-coauthors.txt; wiersze bez vendor/ i Cargo.lock."
                                    ));
                                }
                            }
                        }
                    });
                });
            self.dialogs.about = open;
        }
    }

    pub(super) fn overlays(&mut self, ctx: &egui::Context) {
        let pal = theme::p();
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            let rect = ctx.content_rect();
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(170));
            painter.rect_stroke(rect.shrink(12.0), CornerRadius::same(6), egui::Stroke::new(2.0, pal.accent), egui::StrokeKind::Inside);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Upuść, aby dodać do listy plików",
                egui::FontId::proportional(22.0),
                Color32::WHITE,
            );
        }
        if let Some((text, error, at)) = &self.toast {
            if at.elapsed() > Duration::from_secs(5) {
                self.toast = None;
            } else {
                let color = if *error { pal.danger } else { pal.text };
                egui::Area::new(egui::Id::new("toast"))
                    .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -16.0))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).fill(pal.card).show(ui, |ui| {
                            ui.colored_label(color, text.as_str());
                        });
                    });
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
    }
}
