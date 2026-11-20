//! Prawy inspektor aktywnego zdjęcia: tło, kadr, marginesy (−/+), szybkie operacje.

use std::collections::HashMap;

use asone_core::imageio::fit_within;
use egui::{RichText, Ui, vec2};

use super::{AsOneApp, InspectorTab};
use crate::ai::AiJob;
use crate::canvas::Tool;
use crate::document::{AiState, BOTTOM, DocId, LEFT, LoadState, PxRect, RIGHT, TOP, WHITE, fill_rect, union_opt};
use crate::theme;

const MARGIN_STEP: u32 = 10;

impl AsOneApp {
    pub(super) fn inspector(&mut self, ui: &mut Ui, open: &[DocId], numbering: &HashMap<DocId, (usize, Option<String>)>) {
        let pal = theme::p();
        let frame = egui::Frame::new()
            .fill(pal.card)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .stroke(egui::Stroke::new(1.0, pal.separator));
        egui::Panel::right("inspector").exact_size(280.0).resizable(false).frame(frame).show(ui, |ui| {
            // Zakładki panelu: narzędzia edycji albo podgląd metadanych pliku.
            ui.horizontal(|ui| {
                let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                for (tab, label) in [(InspectorTab::Tools, "Narzędzia"), (InspectorTab::Metadata, "Metadane")] {
                    let button = egui::Button::selectable(self.inspector_tab == tab, label).min_size(vec2(w, 24.0));
                    if ui.add(button).clicked() {
                        self.inspector_tab = tab;
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.inspector_tab {
                InspectorTab::Tools => self.inspector_contents(ui, open, numbering),
                InspectorTab::Metadata => self.metadata_panel(ui),
            });
        });
    }

    fn inspector_contents(&mut self, ui: &mut Ui, open: &[DocId], numbering: &HashMap<DocId, (usize, Option<String>)>) {
        let pal = theme::p();
        let section = |ui: &mut Ui, title: &str| {
            ui.add_space(6.0);
            ui.label(RichText::new(title).strong().color(pal.text));
            ui.add_space(2.0);
        };
        let Some(id) = self.active_doc.filter(|id| self.docs.contains_key(id)) else {
            ui.colored_label(pal.text_dim, "Kliknij zdjęcie na liście plików, aby otworzyć je w zakładce i edytować.");
            return;
        };
        let max_long = self.settings.limit_long_side.then_some(self.settings.max_long_side);
        let limits = self.settings.limits();
        let tool = self.tool;
        let ai_model = self.settings.ai_model;
        let ai_path = self.settings.ai_model_path();
        let hide_mask = self.settings.hide_mask_toggle;
        let hide_inspection = self.settings.hide_inspection;
        let mut open_ai_prefs = false;
        let mut switch_tool = None;
        let mut ai_job = None;
        let mut apply_all = false;
        let mut auto_crop = false;
        {
            let Some(doc) = self.docs.get_mut(&id) else { return };
            ui.label(RichText::new(&doc.file_name).strong().size(14.0));
            if let Some((n, name)) = numbering.get(&id) {
                ui.colored_label(pal.accent, format!("#{n} » {}", name.as_deref().unwrap_or("(wklej nazwę)")));
            }
            let Some(src) = doc.src_size() else {
                match &doc.load {
                    LoadState::Failed(e) => ui.colored_label(pal.danger, e),
                    _ => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Wczytywanie…");
                        })
                        .response
                    }
                };
                return;
            };
            let out = doc.edits.output_size(src);
            egui::Grid::new("info").num_columns(2).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                ui.colored_label(pal.text_dim, "Źródło");
                ui.label(format!("{} × {} px", src[0], src[1]));
                ui.end_row();
                if let Some([w, h]) = doc.downscaled_from {
                    ui.colored_label(pal.text_dim, "Oryginał");
                    ui.label(format!("{w} × {h} (limit GPU)"));
                    ui.end_row();
                }
                ui.colored_label(pal.text_dim, "Wynik");
                ui.label(format!("{} × {} px", out[0], out[1]));
                ui.end_row();
                let mut saved = max_long.map_or(out, |m| fit_within(out, m));
                if let Some(t) = asone_core::finish::fit_box(saved, limits.max_width, limits.max_height) {
                    saved = t;
                }
                if let Some(min) = limits.min_side {
                    saved = [saved[0].max(min), saved[1].max(min)];
                }
                ui.colored_label(pal.text_dim, "Zapis");
                let capped = saved != out && max_long.is_none_or(|m| fit_within(out, m) != saved);
                let text = format!("{} × {} px", saved[0], saved[1]);
                if capped {
                    ui.colored_label(pal.warning, format!("{text} (limit)"));
                } else {
                    ui.label(text);
                }
                ui.end_row();
            });
            ui.separator();

            section(ui, "Tło");
            match doc.ai.clone() {
                AiState::Running => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Usuwanie tła…");
                    });
                }
                state => {
                    ui.horizontal(|ui| {
                        let tip = format!("{} · {}", ai_model.label(), ai_path.display());
                        if ui.button("Usuń tło").on_hover_text(tip).clicked()
                            && let Some(image) = doc.ai_proxy.clone()
                        {
                            ai_job = Some(AiJob { id, image, full_size: src, model: ai_model, path: ai_path.clone() });
                            doc.ai = AiState::Running;
                        }
                        if ui.small_button("⚙").on_hover_text("Model i plik: Ustawienia › Preferencje › Usuwanie tła (AI)").clicked() {
                            open_ai_prefs = true;
                        }
                    });
                    if let AiState::Failed(e) = state {
                        ui.colored_label(pal.danger, &e);
                        if crate::ai::AVAILABLE && !ai_path.is_file() && ui.button("Pobierz model…").clicked() {
                            open_ai_prefs = true;
                        }
                    }
                }
            }
            let mut white = doc.edits.bg_color.is_some();
            if ui.checkbox(&mut white, "Białe tło").changed() {
                doc.edits.bg_color = white.then_some(WHITE);
            }
            // „Maska aktywna” tylko tam, gdzie maska coś zmienia (PNG z przezroczystością, gumka, AI).
            let show_mask = !hide_mask && doc.mask_touched;
            if show_mask {
                ui.checkbox(&mut doc.edits.use_bg_removal, "Maska aktywna (usunięte tło)");
            }
            if !white {
                ui.colored_label(pal.text_dim, "JPEG nie ma przezroczystości – w pliku tło będzie białe.");
            }
            if show_mask || !hide_inspection {
                ui.horizontal(|ui| {
                    if show_mask && ui.button("Resetuj maskę").clicked() {
                        doc.begin_mask_change();
                        doc.mask_cpu.fill(255);
                        doc.mask_dirty = Some(PxRect { x0: 0, y0: 0, x1: src[0], y1: src[1] });
                        doc.edits.use_bg_removal = false;
                        doc.mask_touched = false;
                    }
                    if !hide_inspection {
                        ui.toggle_value(&mut self.inspection, "Inspekcja tła (I)")
                            .on_hover_text("Ekstremalny kontrast przy bieli: czyste białe zostaje białe, każdy szary piksel tła (cień, kurz, obwódka) od razu widać. Tylko podgląd.");
                    }
                });
            }
            ui.separator();

            section(ui, "Kadrowanie");
            let c = doc.edits.crop_px(src);
            ui.label(format!("Kadr: {} × {} px (od {}, {})", c.width(), c.height(), c.x0, c.y0));
            ui.horizontal(|ui| {
                if ui.add_enabled(tool != Tool::Crop, egui::Button::new("Kadruj (C)")).clicked() {
                    switch_tool = Some(Tool::Crop);
                }
                if ui.button("Kadruj (auto)").on_hover_text("Kadr do obiektu na jasnym tle (lub do maski, jeśli tło usunięte)").clicked() {
                    auto_crop = true;
                }
                if ui.add_enabled(!doc.edits.crop_is_full(src), egui::Button::new("Resetuj")).clicked() {
                    doc.edits.crop = [0.0, 0.0, 1.0, 1.0];
                }
            });
            ui.separator();

            section(ui, "Marginesy (px)");
            let shift = ui.input(|i| i.modifiers.shift);
            let step = if shift { 1 } else { MARGIN_STEP };
            let mut m = doc.edits.margins;
            let mut changed_side = None;
            egui::Grid::new("margins").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                for (label, side) in [("Góra", TOP), ("Dół", BOTTOM), ("Lewo", LEFT), ("Prawo", RIGHT)] {
                    ui.label(label);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        let tip = format!("−{MARGIN_STEP} px (Shift: −1 px)");
                        if ui.add(egui::Button::new("−").min_size(vec2(24.0, 22.0))).on_hover_text(tip).clicked() {
                            m[side] = m[side].saturating_sub(step);
                            changed_side = Some(side);
                        }
                        if ui.add(egui::DragValue::new(&mut m[side]).range(0..=20000).speed(1.0).suffix(" px")).changed() {
                            changed_side = Some(side);
                        }
                        let tip = format!("+{MARGIN_STEP} px (Shift: +1 px)");
                        if ui.add(egui::Button::new("+").min_size(vec2(24.0, 22.0))).on_hover_text(tip).clicked() {
                            m[side] = (m[side] + step).min(20000);
                            changed_side = Some(side);
                        }
                    });
                    ui.end_row();
                }
            });
            if let Some(side) = changed_side
                && doc.margins_linked
            {
                m = [m[side]; 4];
            }
            doc.edits.margins = m;
            let lock = if doc.margins_linked { "🔒 Spnij wszystkie boki" } else { "🔓 Spnij wszystkie boki" };
            if ui.toggle_value(&mut doc.margins_linked, lock).changed() && doc.margins_linked {
                doc.edits.margins = [doc.edits.margins[TOP]; 4];
            }
            ui.horizontal(|ui| {
                if ui.button("Dopasuj do kwadratu 1:1").clicked() {
                    doc.edits.fit_square(src);
                    let m = doc.edits.margins;
                    if m.iter().any(|&v| v != m[0]) {
                        doc.margins_linked = false;
                    }
                }
                if ui.button("Zeruj").clicked() {
                    doc.edits.margins = [0; 4];
                }
            });
            ui.separator();

            section(ui, "Szybkie operacje");
            ui.horizontal_wrapped(|ui| {
                let mut add = |sides: &[usize], px: u32| {
                    for s in sides {
                        doc.edits.margins[*s] += px;
                    }
                    let m = doc.edits.margins;
                    if m.iter().any(|&v| v != m[0]) {
                        doc.margins_linked = false;
                    }
                };
                if ui.button("Ramka 5 px").on_hover_text("+5 px marginesu z każdej strony").clicked() {
                    add(&[TOP, RIGHT, BOTTOM, LEFT], 5);
                }
                if ui.button("Paski 5 px L+P").clicked() {
                    add(&[LEFT, RIGHT], 5);
                }
                if ui.button("Paski 5 px G+D").clicked() {
                    add(&[TOP, BOTTOM], 5);
                }
                if ui
                    .button("Min. 500 px")
                    .on_hover_text("Dopełnia tłem do co najmniej 500 × 500 px (bez powiększania pikseli)")
                    .clicked()
                {
                    let [w, h] = doc.edits.output_size(src);
                    let (dw, dh) = (500u32.saturating_sub(w), 500u32.saturating_sub(h));
                    doc.edits.margins[LEFT] += dw / 2;
                    doc.edits.margins[RIGHT] += dw - dw / 2;
                    doc.edits.margins[TOP] += dh / 2;
                    doc.edits.margins[BOTTOM] += dh - dh / 2;
                    let m = doc.edits.margins;
                    if m.iter().any(|&v| v != m[0]) {
                        doc.margins_linked = false;
                    }
                }
            });
            ui.separator();

            match tool {
                Tool::Eraser => {
                    section(ui, "Gumka");
                    ui.add(egui::Slider::new(&mut self.brush.radius, 1.0..=1000.0).logarithmic(true).text("promień px"));
                    ui.colored_label(pal.text_dim, "LPM usuwa, PPM / Alt+LPM przywraca · [ ] zmienia rozmiar");
                    ui.separator();
                }
                Tool::RectCut => {
                    section(ui, "Zaznaczenie");
                    match doc.selection {
                        Some(sel) => {
                            ui.label(format!("{} × {} px (od {}, {})", sel.width(), sel.height(), sel.x0, sel.y0));
                            ui.horizontal(|ui| {
                                for (label, value) in [("Wytnij (Delete)", 0u8), ("Przywróć", 255u8)] {
                                    if ui.button(label).clicked() {
                                        doc.begin_mask_change();
                                        let dirty = fill_rect(&mut doc.mask_cpu, src, sel, value);
                                        doc.mask_dirty = union_opt(doc.mask_dirty, dirty);
                                        doc.edits.use_bg_removal = true;
                                        doc.mask_touched = true;
                                    }
                                }
                            });
                            if ui.button("Odznacz (Esc)").clicked() {
                                doc.selection = None;
                            }
                        }
                        None => {
                            ui.colored_label(pal.text_dim, "Przeciągnij po zdjęciu, aby zaznaczyć obszar.");
                        }
                    }
                    ui.separator();
                }
                _ => {}
            }

            section(ui, "Wszystkie otwarte");
            if ui
                .add_enabled(open.len() > 1, egui::Button::new("Zastosuj ustawienia do wszystkich otwartych"))
                .on_hover_text("Kopiuje marginesy, kłódkę i białe tło na pozostałe zakładki")
                .clicked()
            {
                apply_all = true;
            }
            ui.checkbox(&mut self.copy_crop_too, "razem z kadrowaniem");
        }

        if let Some(job) = ai_job {
            self.ai.submit(job);
        }
        if open_ai_prefs {
            self.prefs.open(crate::settings::PrefsSection::Ai, &self.settings);
        }
        if let Some(t) = switch_tool {
            self.tool = t;
        }
        if auto_crop {
            self.auto_crop(id);
        }
        if apply_all && let Some(src_doc) = self.docs.get(&id) {
            let (edits, linked) = (src_doc.edits.clone(), src_doc.margins_linked);
            let with_crop = self.copy_crop_too;
            for other in open.iter().filter(|o| **o != id) {
                if let Some(d) = self.docs.get_mut(other) {
                    d.edits.margins = edits.margins;
                    d.edits.bg_color = edits.bg_color;
                    d.margins_linked = linked;
                    if with_crop {
                        d.edits.crop = edits.crop;
                    }
                }
            }
            self.toast(format!("Skopiowano ustawienia na {} zdjęć", open.len() - 1), false);
        }
    }
}
