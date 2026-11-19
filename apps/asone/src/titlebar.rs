//! Pasek tytułu jak w PhotoCrafcie: znak, menu, tytuł na środku, skala interfejsu −/+ i
//! (z własnym paskiem) przyciski okna. Bez dekoracji systemowych okno przesuwa się za wolne
//! miejsce na pasku, a rozmiar zmienia się za krawędzie.

use egui::{Align2, Color32, CornerRadius, CursorIcon, FontId, Rect, Sense, Stroke, Ui, ViewportCommand, pos2, vec2};
use egui::viewport::ResizeDirection;

use crate::settings::Settings;
use crate::theme::{self, ThemeKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAction {
    AddFiles,
    AddFolder,
    Export,
    Quit,
    Preferences(crate::settings::PrefsSection),
    SetTheme(ThemeKind),
    UiZoomIn,
    UiZoomOut,
    UiZoomReset,
    FitImage,
    ActualSize,
    ToggleInspection,
    Shortcuts,
    About,
}

pub const BAR_HEIGHT: f32 = 32.0;

/// Czy rysujemy własne przyciski okna (Linux/BSD/Windows bez systemowego paska).
pub fn custom_decorations(settings: &Settings) -> bool {
    !settings.system_title_bar && cfg!(not(any(target_os = "macos", target_os = "android", target_arch = "wasm32")))
}

fn brand_mark(ui: &mut Ui) {
    let pal = theme::p();
    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), pal.accent);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "A1", FontId::proportional(9.5), pal.accent_text);
}

fn item(ui: &mut Ui, label: &str, shortcut: &str) -> bool {
    let b = egui::Button::new(label);
    let b = if shortcut.is_empty() { b } else { b.shortcut_text(shortcut) };
    let clicked = ui.add(b).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// Rysuje menu i zwraca prawą krawędź ostatniego tytułu menu.
fn menus(ui: &mut Ui, settings: &Settings, inspection: bool, out: &mut Vec<MenuAction>) -> f32 {
    use crate::settings::PrefsSection;
    egui::MenuBar::new().ui(ui, |ui| {
        let mut right = ui.min_rect().left();
        let r = ui.menu_button("Plik", |ui| {
            if crate::platform::CAN_PICK_FILES && item(ui, "Dodaj zdjęcia…", "Ctrl+O") {
                out.push(MenuAction::AddFiles);
            }
            if crate::platform::CAN_PICK_FOLDER && item(ui, "Dodaj folder…", "") {
                out.push(MenuAction::AddFolder);
            }
            ui.separator();
            if item(ui, "Zapisz z zakładek", "Ctrl+S") {
                out.push(MenuAction::Export);
            }
            if cfg!(not(target_arch = "wasm32")) {
                ui.separator();
                if item(ui, "Zakończ", "Ctrl+Q") {
                    out.push(MenuAction::Quit);
                }
            }
        });
        right = right.max(r.response.rect.right());
        let r = ui.menu_button("Ustawienia", |ui| {
            ui.menu_button("Preferencje", |ui| {
                if item(ui, "Interfejs…", "") {
                    out.push(MenuAction::Preferences(PrefsSection::Interface));
                }
                if item(ui, "Eksport i limity…", "") {
                    out.push(MenuAction::Preferences(PrefsSection::Export));
                }
            });
            ui.menu_button("Motyw", |ui| {
                for kind in ThemeKind::ALL {
                    if ui.radio(settings.theme == kind, kind.label()).clicked() {
                        out.push(MenuAction::SetTheme(kind));
                        ui.close();
                    }
                }
            });
        });
        right = right.max(r.response.rect.right());
        let r = ui.menu_button("Widok", |ui| {
            if item(ui, "Powiększ interfejs", "Ctrl++") {
                out.push(MenuAction::UiZoomIn);
            }
            if item(ui, "Pomniejsz interfejs", "Ctrl+−") {
                out.push(MenuAction::UiZoomOut);
            }
            if item(ui, "Interfejs 100 %", "") {
                out.push(MenuAction::UiZoomReset);
            }
            ui.separator();
            if item(ui, "Dopasuj zdjęcie do okna", "Ctrl+0") {
                out.push(MenuAction::FitImage);
            }
            if item(ui, "Zdjęcie 100 %", "Ctrl+1") {
                out.push(MenuAction::ActualSize);
            }
            ui.separator();
            let label = if inspection { "✔ Inspekcja tła" } else { "Inspekcja tła" };
            if item(ui, label, "I") {
                out.push(MenuAction::ToggleInspection);
            }
        });
        right = right.max(r.response.rect.right());
        let r = ui.menu_button("Pomoc", |ui| {
            if item(ui, "Skróty klawiszowe", "F1") {
                out.push(MenuAction::Shortcuts);
            }
            if item(ui, "O programie AsOne", "") {
                out.push(MenuAction::About);
            }
        });
        right = right.max(r.response.rect.right());
        // Pasek menu zajmuje całą szerokość – liczy się tylko obszar tytułów menu.
        right
    })
    .inner
}

/// Przycisk okna (minimalizuj / maksymalizuj / zamknij) rysowany jak w PhotoCrafcie.
fn caption_button(ui: &mut Ui, kind: u8, maximized: bool) -> bool {
    let pal = theme::p();
    let (rect, response) = ui.allocate_exact_size(vec2(46.0, BAR_HEIGHT), Sense::click());
    let close = kind == 2;
    if response.hovered() {
        let fill = if close { Color32::from_rgb(196, 43, 28) } else { pal.hover };
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    let color = if close && response.hovered() { Color32::WHITE } else { pal.icon };
    let stroke = Stroke::new(1.0, color);
    let c = rect.center();
    let p = ui.painter();
    match kind {
        0 => {
            p.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
        }
        1 if maximized => {
            p.rect_stroke(Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(8.0, 8.0)), 0.0, stroke, egui::StrokeKind::Middle);
            p.line_segment([c + vec2(-3.0, -3.0), c + vec2(5.0, -3.0)], stroke);
            p.line_segment([c + vec2(5.0, -3.0), c + vec2(5.0, 5.0)], stroke);
        }
        1 => {
            p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, stroke, egui::StrokeKind::Middle);
        }
        _ => {
            p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], stroke);
            p.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], stroke);
        }
    }
    let tip = ["Minimalizuj", if maximized { "Przywróć" } else { "Maksymalizuj" }, "Zamknij"][kind as usize];
    response.on_hover_text(tip).clicked()
}

/// Rysuje pasek tytułu i zwraca kliknięte pozycje menu.
pub fn title_bar(ui: &mut Ui, title: &str, settings: &Settings, inspection: bool) -> Vec<MenuAction> {
    let pal = theme::p();
    let custom = custom_decorations(settings);
    let mut actions = Vec::new();
    let frame = egui::Frame::NONE
        .fill(pal.chrome)
        .inner_margin(egui::Margin { left: 8, right: if custom { 0 } else { 8 }, top: 0, bottom: 0 });
    egui::Panel::top("title_bar").exact_size(BAR_HEIGHT).resizable(false).frame(frame).show(ui, |ui| {
        let full = ui.max_rect();
        let ctx = ui.ctx().clone();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        // Okno przesuwa tylko wolne miejsce między menu a przyciskami (z poprzedniej klatki), jak
        // w PhotoCrafcie: wciśnięcie na tytule menu ma otworzyć menu, a nie przesunąć okno.
        let gap_id = ui.id().with("title_gap");
        let (gap_l, gap_r) = ctx.data(|d| d.get_temp::<(f32, f32)>(gap_id)).unwrap_or((full.right(), full.right()));
        if custom && gap_r > gap_l {
            let gap = Rect::from_x_y_ranges(gap_l..=gap_r, full.y_range());
            let drag = ui.interact(gap, ui.id().with("title_drag"), Sense::click_and_drag());
            if drag.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
        }
        let mut menus_end = full.left();
        let mut controls_start = full.right();
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            brand_mark(ui);
            ui.add_space(4.0);
            menus_end = menus(ui, settings, inspection, &mut actions);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if custom {
                    if caption_button(ui, 2, maximized) {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    if caption_button(ui, 1, maximized) {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if caption_button(ui, 0, maximized) {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                    ui.add_space(8.0);
                }
                ui.spacing_mut().item_spacing.x = 2.0;
                let small = |t: &str| egui::Button::new(egui::RichText::new(t).size(15.0)).min_size(vec2(24.0, 22.0));
                if ui.add(small("+")).on_hover_text("Powiększ interfejs (Ctrl++)").clicked() {
                    actions.push(MenuAction::UiZoomIn);
                }
                let pct = egui::Button::new(format!("{} %", settings.ui_scale)).min_size(vec2(54.0, 22.0)).frame(false);
                if ui.add(pct).on_hover_text("Skala interfejsu – kliknij, aby wrócić do 100 %").clicked() {
                    actions.push(MenuAction::UiZoomReset);
                }
                if ui.add(small("−")).on_hover_text("Pomniejsz interfejs (Ctrl+−)").clicked() {
                    actions.push(MenuAction::UiZoomOut);
                }
                controls_start = ui.min_rect().left();
            });
        });
        ctx.data_mut(|d| d.insert_temp(gap_id, (menus_end + 6.0, controls_start - 6.0)));
        // Tytuł na środku, jeśli mieści się między menu a przyciskami.
        let galley = ui.painter().layout_no_wrap(title.to_string(), FontId::proportional(13.0), pal.text_dim);
        let x = full.center().x - galley.size().x / 2.0;
        if x > menus_end + 16.0 && x + galley.size().x < controls_start - 16.0 {
            ui.painter().galley(pos2(x, full.center().y - galley.size().y / 2.0), galley, pal.text_dim);
        }
    });
    actions
}

/// Zmiana rozmiaru okna bez dekoracji: kursor przy krawędzi + `BeginResize`.
pub fn resize_edges(ctx: &egui::Context, settings: &Settings) {
    if !custom_decorations(settings) {
        return;
    }
    let (maximized, fullscreen) = ctx.input(|i| (i.viewport().maximized.unwrap_or(false), i.viewport().fullscreen.unwrap_or(false)));
    if maximized || fullscreen {
        return;
    }
    let screen = ctx.content_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let m = 5.0;
    let (l, r) = (pos.x - screen.left() < m, screen.right() - pos.x < m);
    let (t, b) = (pos.y - screen.top() < m, screen.bottom() - pos.y < m);
    let dir = match (l, r, t, b) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return,
    };
    ctx.set_cursor_icon(match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}
