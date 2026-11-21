//! Prawy inspektor aktywnego zdjęcia: tło, kadr, marginesy (−/+), szybkie operacje.

use std::collections::HashMap;

use asone_core::imageio::fit_within;
use egui::{RichText, Ui, vec2};

use super::{AsOneApp, InspectorTab};
use crate::ai::AiJob;
use crate::canvas::CanvasAction;
use crate::canvas::Tool;
use crate::document::{AiState, BOTTOM, DocId, LEFT, LoadState, PxRect, RIGHT, TOP, WHITE, fill_rect, union_opt};
use crate::theme;

/// Podpowiedź przycisku „Spłaszcz zdjęcie”.
fn flatten_hint() -> &'static str {
    t!("Scala zdjęcie, marginesy, smartfon i usunięte tło (na tle) w jeden obraz – \
        potem kadr, marginesy i gumka działają na całości. Bez cofania: „Przywróć oryginał” wczytuje plik od nowa.")
}

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
                for (tab, label) in [(InspectorTab::Tools, t!("Narzędzia")), (InspectorTab::Metadata, t!("Metadane"))] {
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
            ui.colored_label(pal.text_dim, t!("Kliknij zdjęcie na liście plików, aby otworzyć je w zakładce i edytować."));
            return;
        };
        let max_long = self.settings.limit_long_side.then_some(self.settings.max_long_side);
        let limits = self.settings.limits();
        let tool = self.tool;
        let ai_model = self.settings.ai_model;
        let ai_path = self.settings.ai_model_path();
        let hide_mask = self.settings.hide_mask_toggle;
        let replicate = self.settings.replicate_active();
        let lama = self.settings.lama_active();
        let lama_ready = lama && self.settings.lama_model_path().is_file();
        let w2x = self.settings.waifu2x_active();
        // Zakładki „Narzędzi AI”: tylko włączone narzędzia; wybrana musi być jedną z nich.
        let ai_tools: Vec<super::AiTool> =
            [(super::AiTool::Lama, lama), (super::AiTool::Waifu2x, w2x), (super::AiTool::Replicate, replicate)]
            .into_iter()
            .filter_map(|(t, on)| on.then_some(t))
            .collect();
        if !ai_tools.contains(&self.ai_tool)
            && let Some(first) = ai_tools.first()
        {
            self.ai_tool = *first;
        }
        let mut open_retouch_prefs = false;
        let edit_model = self.settings.replicate_model;
        let hide_inspection = self.settings.hide_inspection;
        let mut open_ai_prefs = false;
        let mut switch_tool = None;
        let mut remove_bg = false;
        let mut phone = None;
        let mut apply_all = false;
        let mut auto_crop = false;
        {
            let Some(doc) = self.docs.get_mut(&id) else { return };
            ui.label(RichText::new(&doc.file_name).strong().size(14.0));
            if let Some((n, name)) = numbering.get(&id) {
                ui.colored_label(pal.accent, format!("#{n} » {}", name.as_deref().unwrap_or(t!("(wklej nazwę)"))));
            }
            let Some(src) = doc.src_size() else {
                match &doc.load {
                    LoadState::Failed(e) => ui.colored_label(pal.danger, e),
                    _ => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t!("Wczytywanie…"));
                        })
                        .response
                    }
                };
                return;
            };
            let out = doc.edits.output_size(src);
            egui::Grid::new("info").num_columns(2).spacing(vec2(10.0, 2.0)).show(ui, |ui| {
                ui.colored_label(pal.text_dim, t!("Źródło"));
                ui.label(format!("{} × {} px", src[0], src[1]));
                ui.end_row();
                if let Some([w, h]) = doc.downscaled_from {
                    ui.colored_label(pal.text_dim, t!("Oryginał"));
                    ui.label(t!("{w} × {h} (limit GPU)", w = w, h = h));
                    ui.end_row();
                }
                ui.colored_label(pal.text_dim, t!("Wynik"));
                ui.label(format!("{} × {} px", out[0], out[1]));
                ui.end_row();
                let mut saved = max_long.map_or(out, |m| fit_within(out, m));
                if let Some(t) = asone_core::finish::fit_box(saved, limits.max_width, limits.max_height) {
                    saved = t;
                }
                if let Some(min) = limits.min_side {
                    saved = [saved[0].max(min), saved[1].max(min)];
                }
                ui.colored_label(pal.text_dim, t!("Zapis"));
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

            section(ui, t!("Tło"));
            match doc.ai.clone() {
                AiState::Running => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(if ai_model.is_online() { t!("Usuwanie tła (fal.ai)…") } else { t!("Usuwanie tła…") });
                    });
                }
                state => {
                    ui.horizontal(|ui| {
                        let tip = if ai_model.is_online() {
                            t!("{model} – zdjęcie zostanie wysłane do fal.ai (płatne za zdjęcie)", model = ai_model.label())
                        } else {
                            format!("{} · {}", ai_model.label(), ai_path.display())
                        };
                        if ui.button(t!("Usuń tło")).on_hover_text(tip).clicked() {
                            remove_bg = true;
                        }
                        if ui.small_button("⚙").on_hover_text(t!("Model i plik: Ustawienia › Preferencje › Usuwanie tła (AI)")).clicked() {
                            open_ai_prefs = true;
                        }
                    });
                    if let AiState::Failed(e) = state {
                        ui.colored_label(pal.danger, &e);
                        if crate::ai::AVAILABLE && !ai_model.is_online() && !ai_path.is_file() && ui.button(t!("Pobierz model…")).clicked() {
                            open_ai_prefs = true;
                        }
                    }
                }
            }
            let mut white = doc.edits.bg_color.is_some();
            if ui.checkbox(&mut white, t!("Białe tło")).changed() {
                doc.edits.bg_color = white.then_some(WHITE);
            }
            // „Maska aktywna” tylko tam, gdzie maska coś zmienia (PNG z przezroczystością, gumka, AI).
            let show_mask = !hide_mask && doc.mask_touched;
            if show_mask {
                ui.checkbox(&mut doc.edits.use_bg_removal, t!("Maska aktywna (usunięte tło)"));
            }
            if !white {
                ui.colored_label(pal.text_dim, t!("JPEG nie ma przezroczystości – w pliku tło będzie białe."));
            }
            if show_mask || !hide_inspection {
                ui.horizontal(|ui| {
                    if show_mask && ui.button(t!("Resetuj maskę")).clicked() {
                        doc.begin_mask_change();
                        doc.mask_cpu.fill(255);
                        doc.mask_dirty = Some(PxRect { x0: 0, y0: 0, x1: src[0], y1: src[1] });
                        doc.edits.use_bg_removal = false;
                        doc.mask_touched = false;
                    }
                    if !hide_inspection {
                        ui.toggle_value(&mut self.inspection, t!("Inspekcja tła (I)"))
                            .on_hover_text(t!("Ekstremalny kontrast przy bieli: czyste białe zostaje białe, każdy szary piksel tła (cień, kurz, obwódka) od razu widać. Tylko podgląd."));
                    }
                });
            }
            ui.separator();

            section(ui, t!("Kadrowanie"));
            let c = doc.edits.crop_px(src);
            ui.label(t!("Kadr: {w} × {h} px (od {x}, {y})", w = c.width(), h = c.height(), x = c.x0, y = c.y0));
            if doc.edits.overlay.is_some() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(pal.text_dim, t!("Kadr obejmuje samo zdjęcie –"));
                    if ui.small_button(t!("Spłaszcz")).on_hover_text(flatten_hint()).clicked() {
                        phone = Some(CanvasAction::Flatten);
                    }
                    ui.colored_label(pal.text_dim, t!("aby kadrować razem ze smartfonem."));
                });
            }
            if doc.content.is_some() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(pal.text_dim, t!("Zdjęcie zmienione (spłaszczenie, retusz albo AI)."));
                    if ui.small_button(t!("Przywróć oryginał")).on_hover_text(t!("Wczytuje plik od nowa (bez spłaszczenia i zmian)")).clicked() {
                        phone = Some(CanvasAction::RestoreOriginal);
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(tool != Tool::Crop, egui::Button::new(t!("Kadruj (C)"))).clicked() {
                    switch_tool = Some(Tool::Crop);
                }
                if ui.button(t!("Kadruj (auto)")).on_hover_text(t!("Kadr do obiektu na jasnym tle (lub do maski, jeśli tło usunięte)")).clicked() {
                    auto_crop = true;
                }
                if ui.add_enabled(!doc.edits.crop_is_full(src), egui::Button::new(t!("Resetuj"))).clicked() {
                    doc.edits.crop = [0.0, 0.0, 1.0, 1.0];
                }
            });
            ui.separator();

            section(ui, t!("Marginesy (px)"));
            let shift = ui.input(|i| i.modifiers.shift);
            let step = if shift { 1 } else { MARGIN_STEP };
            let mut m = doc.edits.margins;
            let mut changed_side = None;
            egui::Grid::new("margins").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                for (label, side) in [(t!("Góra"), TOP), (t!("Dół"), BOTTOM), (t!("Lewo"), LEFT), (t!("Prawo"), RIGHT)] {
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
            let lock = if doc.margins_linked { t!("🔒 Spnij wszystkie boki") } else { t!("🔓 Spnij wszystkie boki") };
            if ui.toggle_value(&mut doc.margins_linked, lock).changed() && doc.margins_linked {
                doc.edits.margins = [doc.edits.margins[TOP]; 4];
            }
            ui.horizontal(|ui| {
                if ui.button(t!("Dopasuj do kwadratu 1:1")).clicked() {
                    doc.edits.fit_square(src);
                    let m = doc.edits.margins;
                    if m.iter().any(|&v| v != m[0]) {
                        doc.margins_linked = false;
                    }
                }
                if ui.button(t!("Zeruj")).clicked() {
                    doc.edits.margins = [0; 4];
                }
            });
            ui.separator();

            section(ui, t!("Szybkie operacje"));
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
                if ui.button(t!("Ramka 5 px")).on_hover_text(t!("+5 px marginesu z każdej strony")).clicked() {
                    add(&[TOP, RIGHT, BOTTOM, LEFT], 5);
                }
                if ui.button(t!("Paski 5 px L+P")).clicked() {
                    add(&[LEFT, RIGHT], 5);
                }
                if ui.button(t!("Paski 5 px G+D")).clicked() {
                    add(&[TOP, BOTTOM], 5);
                }
                if ui
                    .button("Min. 500 px")
                    .on_hover_text(t!("Dopełnia tłem do co najmniej 500 × 500 px (bez powiększania pikseli)"))
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
                if doc.edits.overlay.is_none() {
                    if ui
                        .add_enabled(crate::platform::CAN_PASTE_IMAGE, egui::Button::new(t!("Wklej smartfon")))
                        .on_hover_text(t!(
                            "Dokłada {px} px białego tła z lewej i wkleja tam zdjęcie smartfona ze schowka \
                             (do przesunięcia i skalowania na zdjęciu)",
                            px = crate::document::PHONE_STRIP
                        ))
                        .clicked()
                    {
                        phone = Some(CanvasAction::PastePhone);
                    }
                    if ui.add_enabled(crate::platform::CAN_PICK_OVERLAY, egui::Button::new(t!("Smartfon z pliku…"))).clicked() {
                        phone = Some(CanvasAction::PickPhone);
                    }
                }
            });
            ui.separator();

            if doc.overlay_image.is_some()
                && let Some([x, y, w, h]) = doc.edits.overlay
            {
                section(ui, t!("Smartfon (warstwa)"));
                let aspect = w / h.max(1.0);
                let (mut nx, mut ny, mut nw) = (x, y, w);
                egui::Grid::new("phone_layer").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                    ui.label(t!("Szerokość"));
                    ui.add(egui::DragValue::new(&mut nw).range(16.0..=20000.0).speed(1.0).max_decimals(0).suffix(" px"));
                    ui.end_row();
                    ui.label(t!("Od lewej"));
                    ui.add(egui::DragValue::new(&mut nx).speed(1.0).max_decimals(0).suffix(" px"));
                    ui.end_row();
                    ui.label(t!("Od góry"));
                    ui.add(egui::DragValue::new(&mut ny).speed(1.0).max_decimals(0).suffix(" px"));
                    ui.end_row();
                });
                if nw != w {
                    // Rozmiar zmienia się względem środka warstwy.
                    let nh = nw / aspect.max(1e-3);
                    doc.edits.overlay = Some([x + (w - nw) / 2.0, y + (h - nh) / 2.0, nw, nh]);
                } else if (nx, ny) != (x, y) {
                    doc.edits.overlay = Some([nx, ny, w, h]);
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(crate::platform::CAN_PASTE_IMAGE, egui::Button::new(t!("Podmień ze schowka"))).clicked() {
                        phone = Some(CanvasAction::PastePhone);
                    }
                    if ui.add_enabled(crate::platform::CAN_PICK_OVERLAY, egui::Button::new(t!("Z pliku…"))).clicked() {
                        phone = Some(CanvasAction::PickPhone);
                    }
                    if ui.button(t!("Dopasuj do pasa")).clicked() {
                        doc.fit_overlay_to_strip();
                    }
                    if ui.button(t!("Usuń smartfon")).on_hover_text(t!("Usuwa warstwę i pas {px} px", px = crate::document::PHONE_STRIP)).clicked() {
                        doc.remove_overlay();
                    }
                });
                if ui
                    .button(t!("Spłaszcz zdjęcie"))
                    .on_hover_text(flatten_hint())
                    .clicked()
                {
                    phone = Some(CanvasAction::Flatten);
                }
                ui.colored_label(pal.text_dim, t!("Na zdjęciu (Przesuwanie): przeciągnij, aby przesunąć; rogi – rozmiar."));
                ui.separator();
            }

            if !ai_tools.is_empty() {
                section(ui, t!("Narzędzia AI"));
                ui.horizontal(|ui| {
                    for tool in &ai_tools {
                        let label = match tool {
                            super::AiTool::Lama => "LaMa",
                            super::AiTool::Waifu2x => "waifu2x",
                            super::AiTool::Replicate => "Replicate",
                        };
                        ui.selectable_value(&mut self.ai_tool, *tool, label);
                    }
                });
                ui.add_space(2.0);
            }
            if lama && self.ai_tool == super::AiTool::Lama {
                ui.colored_label(pal.text_dim, t!("Usuwa kurz, rysy, napisy i metki – lokalnie, bez opłat."));
                if !lama_ready {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(pal.warning, t!("Brak modelu (ok. 208 MB) –"));
                        if ui.small_button(t!("Pobierz…")).clicked() {
                            open_retouch_prefs = true;
                        }
                    });
                }
                let busy = doc.busy.is_some();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(tool != Tool::RectCut, egui::Button::new(t!("Zaznacz (M)")))
                        .on_hover_text(t!("Narzędzie prostokąta: przeciągnij po kurzu, napisie albo metce"))
                        .clicked()
                    {
                        switch_tool = Some(Tool::RectCut);
                    }
                    let can = lama_ready && doc.selection.is_some() && !busy;
                    let run = ui.add_enabled(can, egui::Button::new(t!("Usuń obiekt (LaMa)")));
                    let run = if doc.selection.is_none() {
                        run.on_disabled_hover_text(t!("Najpierw zaznacz obszar narzędziem M"))
                    } else {
                        run.on_hover_text(t!("Dorysowuje zaznaczony fragment z otoczenia. Cofnięcie: „Przywróć oryginał”."))
                    };
                    if run.clicked() {
                        phone = Some(CanvasAction::Inpaint);
                    }
                    if busy {
                        ui.spinner();
                    }
                });
                ui.separator();
            }
            if w2x && self.ai_tool == super::AiTool::Waifu2x {
                use crate::waifu2x::{MAX_INPUT_SIDE, NOISE_LEVELS, Style, noise_label};
                ui.colored_label(pal.text_dim, t!("Powiększa z ostrymi krawędziami i usuwa artefakty JPEG – lokalnie, bez opłat."));
                egui::Grid::new("w2x").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
                    ui.label(t!("Rodzaj"));
                    ui.horizontal(|ui| {
                        for style in Style::ALL {
                            ui.selectable_value(&mut self.settings.waifu2x_style, style, style.label());
                        }
                    });
                    ui.end_row();
                    ui.label(t!("Skala"));
                    ui.horizontal(|ui| {
                        for scale in [2u32, 4] {
                            ui.selectable_value(&mut self.settings.waifu2x_scale, scale, format!("{scale}×"));
                        }
                    });
                    ui.end_row();
                    ui.label(t!("Odszumianie"));
                    egui::ComboBox::from_id_salt("w2x_noise")
                        .selected_text(noise_label(self.settings.waifu2x_noise))
                        .show_ui(ui, |ui| {
                            for n in NOISE_LEVELS {
                                ui.selectable_value(&mut self.settings.waifu2x_noise, n, noise_label(n));
                            }
                        });
                    ui.end_row();
                });
                let variant = self.settings.waifu2x_variant();
                let new = [src[0] * variant.scale, src[1] * variant.scale];
                let too_big = src[0].max(src[1]) > MAX_INPUT_SIDE;
                if src[0].min(src[1]) < asone_core::finish::MIN_SIDE && !too_big {
                    ui.colored_label(pal.accent, t!("Zdjęcie ma mniej niż 500 px – powiększ zamiast dopełniać białym tłem."));
                }
                let busy = doc.busy.is_some();
                ui.horizontal_wrapped(|ui| {
                    let run = ui.add_enabled(!busy && !too_big, egui::Button::new(t!("Powiększ → {w} × {h} px", w = new[0], h = new[1])));
                    let run = if too_big {
                        run.on_disabled_hover_text(t!("waifu2x jest dla mniejszych zdjęć – dłuższy bok najwyżej {max} px", max = MAX_INPUT_SIDE))
                    } else {
                        run.on_hover_text(t!("Zastępuje zdjęcie powiększonym; kadr, marginesy i maska skalują się razem z nim."))
                    };
                    if run.clicked() {
                        phone = Some(CanvasAction::Upscale);
                    }
                    if busy {
                        ui.spinner();
                    }
                });
                if !crate::waifu2x::models_dir().join(variant.file_name()).is_file() {
                    ui.colored_label(pal.text_dim, egui::RichText::new(t!("Pierwsze użycie tego wariantu: pobranie modelu (ok. 20 MB).")).small());
                }
                ui.separator();
            }
            if replicate && self.ai_tool == super::AiTool::Replicate {
                let field = ui.add(
                    egui::TextEdit::multiline(&mut self.ai_prompt)
                        .hint_text(t!("Co zrobić ze zdjęciem – najlepiej po angielsku, np. „place the product on a white background”"))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                // Android: klawiatura systemowa w oknie edycji (GameActivity nie podaje tekstu do egui).
                #[cfg(target_os = "android")]
                if field.gained_focus() {
                    self.text_target = crate::platform::TextTarget::AiPrompt;
                    crate::android::edit_text(t!("Polecenie dla AI"), &self.ai_prompt, false);
                    ui.memory_mut(|m| m.surrender_focus(field.id));
                }
                #[cfg(not(target_os = "android"))]
                let _ = field;
                let busy = doc.busy.is_some();
                ui.horizontal_wrapped(|ui| {
                    ui.menu_button(t!("Gotowe polecenia"), |ui| {
                        ui.set_min_width(220.0);
                        for (label, prompt) in crate::settings::ai_presets() {
                            if ui.button(label).on_hover_text(prompt).clicked() {
                                self.ai_prompt = prompt.to_string();
                                ui.close();
                            }
                        }
                    });
                    let run = ui
                        .add_enabled(!busy && !self.ai_prompt.trim().is_empty(), egui::Button::new(t!("Wykonaj ({price})", price = edit_model.price())))
                        .on_hover_text(t!(
                            "{model} na Replicate: zdjęcie z zakładki (z kadrem, marginesami i smartfonem) zostanie wysłane, \
                             a wynik (ok. 1 Mpx) zastąpi zdjęcie. Płatne: {price} za obraz.",
                            model = edit_model.label(),
                            price = edit_model.price()
                        ));
                    if run.clicked() {
                        phone = Some(CanvasAction::AiEdit);
                    }
                    if busy {
                        ui.spinner();
                    }
                });
                ui.colored_label(pal.text_dim, t!("Wynik zastępuje zdjęcie; „Przywróć oryginał” wraca do pliku."));
                ui.separator();
            }

            match tool {
                Tool::Eraser => {
                    section(ui, t!("Gumka"));
                    ui.add(egui::Slider::new(&mut self.brush.radius, 1.0..=1000.0).logarithmic(true).text(t!("promień px")));
                    ui.colored_label(pal.text_dim, t!("LPM usuwa, PPM / Alt+LPM przywraca · [ ] albo Ctrl+kółko – rozmiar"));
                    ui.separator();
                }
                Tool::RectCut => {
                    section(ui, t!("Zaznaczenie"));
                    match doc.selection {
                        Some(sel) => {
                            ui.label(t!("{w} × {h} px (od {x}, {y})", w = sel.width(), h = sel.height(), x = sel.x0, y = sel.y0));
                            ui.horizontal(|ui| {
                                for (label, value) in [(t!("Wytnij (Delete)"), 0u8), (t!("Przywróć"), 255u8)] {
                                    if ui.button(label).clicked() {
                                        doc.begin_mask_change();
                                        let dirty = fill_rect(&mut doc.mask_cpu, src, sel, value);
                                        doc.mask_dirty = union_opt(doc.mask_dirty, dirty);
                                        doc.edits.use_bg_removal = true;
                                        doc.mask_touched = true;
                                    }
                                }
                            });
                            if lama {
                                let busy = doc.busy.is_some();
                                if ui
                                    .add_enabled(!busy, egui::Button::new(t!("Usuń obiekt (LaMa)")))
                                    .on_hover_text(t!(
                                        "Dorysowuje zaznaczony fragment z otoczenia: kurz, rysy, napisy, metki. \
                                         Lokalnie, bez opłat. Cofnięcie: „Przywróć oryginał”."
                                    ))
                                    .clicked()
                                {
                                    phone = Some(CanvasAction::Inpaint);
                                }
                            }
                            if ui.button(t!("Odznacz (Esc)")).clicked() {
                                doc.selection = None;
                            }
                        }
                        None => {
                            ui.colored_label(pal.text_dim, t!("Przeciągnij po zdjęciu, aby zaznaczyć obszar."));
                        }
                    }
                    ui.separator();
                }
                _ => {}
            }

            section(ui, t!("Wszystkie otwarte"));
            if ui
                .add_enabled(open.len() > 1, egui::Button::new(t!("Zastosuj ustawienia do wszystkich otwartych")))
                .on_hover_text(t!("Kopiuje marginesy, kłódkę i białe tło na pozostałe zakładki"))
                .clicked()
            {
                apply_all = true;
            }
            ui.checkbox(&mut self.copy_crop_too, t!("razem z kadrowaniem"));
        }

        if remove_bg {
            self.start_remove_background(id);
        }
        if let Some(action) = phone {
            self.canvas_action(id, action);
        }
        if open_retouch_prefs {
            self.prefs.open(crate::settings::PrefsSection::Retouch, &self.settings);
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
            self.toast(t!("Skopiowano ustawienia na {n} zdjęć", n = open.len() - 1), false);
        }
    }

    /// „Usuń tło”: zadanie dla modelu z Ustawień (lokalnie albo online).
    pub(super) fn start_remove_background(&mut self, id: DocId) {
        let model = self.settings.ai_model;
        let path = self.settings.ai_model_path();
        let fal_key = if model.is_online() { self.settings.fal_api_key() } else { String::new() };
        let Some(doc) = self.docs.get_mut(&id) else { return };
        let (Some(image), Some(full_size)) = (doc.ai_proxy.clone(), doc.src_size()) else { return };
        if matches!(doc.ai, AiState::Running) {
            return;
        }
        doc.ai = AiState::Running;
        let job = AiJob { id, image, full_size, model, path, source: doc.pixel_source(), fal_key };
        self.ai.submit(job);
    }

    /// „Spłaszcz zdjęcie”: obraz wynikowy (z warstwą) staje się nowym zdjęciem zakładki.
    pub(super) fn flatten(&mut self, id: DocId) {
        let Some(doc) = self.docs.get_mut(&id) else { return };
        if !matches!(doc.load, LoadState::Ready) || doc.gpu.is_none() || doc.busy.is_some() {
            return;
        }
        doc.commit_history();
        let overlay = doc.overlay_image.clone().filter(|_| doc.edits.overlay.is_some());
        self.loader.request_flatten(id, doc.pixel_source(), doc.mask_cpu.clone(), doc.edits.clone(), overlay);
        doc.busy = Some(crate::document::Busy::new(t!("Spłaszczanie…")));
    }

    /// waifu2x: powiększenie zdjęcia (wariant z panelu), lokalnie.
    pub(super) fn start_upscale(&mut self, id: DocId) {
        let variant = self.settings.waifu2x_variant();
        let Some(doc) = self.docs.get_mut(&id) else { return };
        if !matches!(doc.load, LoadState::Ready) || doc.gpu.is_none() || doc.busy.is_some() {
            return;
        }
        doc.commit_history();
        let (busy, progress) = crate::document::Busy::with_progress(t!("Powiększanie (waifu2x)…"));
        self.loader.request_upscale(id, doc.pixel_source(), doc.mask_cpu.clone(), variant, progress);
        doc.busy = Some(busy);
        let first = !crate::waifu2x::models_dir().join(variant.file_name()).is_file();
        self.toast(
            if first { t!("waifu2x – pobieram model (ok. 20 MB) i powiększam…") } else { t!("waifu2x – powiększanie…") },
            false,
        );
    }

    /// Retusz LaMa: zaznaczony prostokąt (narzędzie M) dorysowany z otoczenia, lokalnie.
    pub(super) fn start_inpaint(&mut self, id: DocId) {
        let model = self.settings.lama_model_path();
        if !model.is_file() {
            self.toast(t!("Brak modelu LaMa – pobierz go w Ustawienia › Preferencje › Retusz (LaMa)"), true);
            self.prefs.open(crate::settings::PrefsSection::Retouch, &self.settings);
            return;
        }
        let Some(doc) = self.docs.get_mut(&id) else { return };
        let Some(rect) = doc.selection else {
            self.toast(t!("Zaznacz obszar do usunięcia narzędziem M (prostokąt)"), true);
            return;
        };
        if !matches!(doc.load, LoadState::Ready) || doc.gpu.is_none() || doc.busy.is_some() {
            return;
        }
        doc.commit_history();
        self.loader.request_inpaint(id, doc.pixel_source(), doc.mask_cpu.clone(), rect, model);
        doc.busy = Some(crate::document::Busy::new(t!("Usuwanie obiektu (LaMa)…")));
        self.toast(t!("Retusz LaMa – kilka sekund…"), false);
    }

    /// Edycja AI (Replicate): obraz z zakładki + polecenie; wynik zastąpi zdjęcie.
    pub(super) fn start_ai_edit(&mut self, id: DocId, prompt: String) {
        if !self.settings.replicate_active() {
            self.toast(t!("Funkcje Replicate są wyłączone albo brak klucza – Ustawienia › Preferencje › Replicate"), true);
            return;
        }
        if prompt.trim().is_empty() {
            self.toast(t!("Wpisz polecenie dla AI (co zrobić ze zdjęciem)"), true);
            return;
        }
        let (key, model) = (self.settings.replicate_api_key(), self.settings.replicate_model);
        let Some(doc) = self.docs.get_mut(&id) else { return };
        if !matches!(doc.load, LoadState::Ready) || doc.gpu.is_none() || doc.busy.is_some() {
            return;
        }
        doc.commit_history();
        let overlay = doc.overlay_image.clone().filter(|_| doc.edits.overlay.is_some());
        self.loader.request_ai_edit(id, doc.pixel_source(), doc.mask_cpu.clone(), doc.edits.clone(), overlay, prompt, model.id(), key);
        doc.busy = Some(crate::document::Busy::new(t!("Edycja AI (Replicate)…")));
        self.toast(t!("Edycja AI ({model}, {price}) – zwykle kilkanaście sekund…", model = model.label(), price = model.price()), false);
    }

    /// Polecenia z menu kontekstowego zdjęcia i przycisków „Smartfon”.
    pub(super) fn canvas_action(&mut self, id: DocId, action: CanvasAction) {
        match action {
            CanvasAction::Inpaint => self.start_inpaint(id),
            CanvasAction::Upscale => self.start_upscale(id),
            CanvasAction::AiEdit => self.start_ai_edit(id, self.ai_prompt.clone()),
            CanvasAction::AiPreset(i) => {
                if let Some((_, prompt)) = crate::settings::ai_presets().get(i as usize) {
                    self.ai_prompt = prompt.to_string();
                    self.start_ai_edit(id, prompt.to_string());
                }
            }
            CanvasAction::Flatten => self.flatten(id),
            CanvasAction::RestoreOriginal => self.reload(&[id]),
            CanvasAction::PastePhone => self.pickers.paste_image(id),
            CanvasAction::PickPhone => self.pickers.pick_overlay_image(id),
            CanvasAction::RemoveBackground => self.start_remove_background(id),
            CanvasAction::AutoCrop => self.auto_crop(id),
            _ => {
                let Some(doc) = self.docs.get_mut(&id) else { return };
                let Some(src) = doc.src_size() else { return };
                match action {
                    CanvasAction::FitPhone => doc.fit_overlay_to_strip(),
                    CanvasAction::RemovePhone => doc.remove_overlay(),
                    CanvasAction::FitSquare => doc.edits.fit_square(src),
                    CanvasAction::Frame5 => doc.edits.margins.iter_mut().for_each(|m| *m += 5),
                    CanvasAction::ResetMargins => doc.edits.margins = [0; 4],
                    CanvasAction::FitView => doc.camera.needs_fit = true,
                    CanvasAction::Undo => {
                        doc.step_history(false);
                    }
                    CanvasAction::Redo => {
                        doc.step_history(true);
                    }
                    _ => {}
                }
                let m = doc.edits.margins;
                if m.iter().any(|&v| v != m[0]) {
                    doc.margins_linked = false;
                }
            }
        }
    }
}
