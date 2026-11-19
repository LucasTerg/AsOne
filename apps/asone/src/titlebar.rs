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
    ToggleHidden(crate::settings::HideItem),
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

// --- Menu jak w PhotoCrafcie ----------------------------------------------------------------
//
// * tytuł otwiera menu na WCIŚNIĘCIE (nie na puszczenie); gest „wciśnij – przeciągnij – puść na
//   pozycji” od razu ją wykonuje,
// * gdy jedno menu jest otwarte, najechanie na inny tytuł przełącza menu (jak natywny pasek),
// * klawiatura: Alt+P/U/W/M otwiera menu, F10 – pierwsze, ←/→ – sąsiednie, Esc zamyka.

/// Tytuły menu i ich litery (Alt+litera).
const TOP: [(&str, egui::Key); 4] = [
    ("Plik", egui::Key::P),
    ("Ustawienia", egui::Key::U),
    ("Widok", egui::Key::W),
    ("Pomoc", egui::Key::M),
];

fn press_gesture_id() -> egui::Id {
    egui::Id::new("asone-menu-press-gesture")
}

/// Wciśnięcie na tytule: otwiera zamknięte menu (i zaczyna gest przeciągania) albo zamyka otwarte.
fn title_press(ctx: &egui::Context, title: &egui::Response) -> Option<egui::SetOpenCommand> {
    let pressed = ctx.input(|i| i.pointer.primary_pressed()) && (title.is_pointer_button_down_on() || title.clicked());
    if !pressed {
        return None;
    }
    let open = egui::Popup::is_id_open(ctx, egui::Popup::default_response_id(title));
    if !open {
        ctx.data_mut(|d| d.insert_temp(press_gesture_id(), true));
    }
    Some(egui::SetOpenCommand::Bool(!open))
}

/// Czy gest otwierający menu zakończył się puszczeniem przycisku nad tą pozycją?
fn released_on(ui: &Ui, item: &egui::Response) -> bool {
    item.enabled()
        && item.contains_pointer()
        && ui.input(|i| i.pointer.primary_released())
        && ui.ctx().data(|d| d.get_temp::<bool>(press_gesture_id())).unwrap_or(false)
}

fn item(ui: &mut Ui, label: &str, shortcut: &str) -> bool {
    let b = egui::Button::new(label);
    let b = if shortcut.is_empty() { b } else { b.shortcut_text(shortcut) };
    let r = ui.add(b);
    let hit = r.clicked() || released_on(ui, &r);
    if hit {
        ui.close();
    }
    hit
}

/// Pozycja z ptaszkiem (stan włączony/wyłączony).
fn check_item(ui: &mut Ui, checked: bool, label: &str) -> bool {
    let text = if checked { format!("✔  {label}") } else { format!("     {label}") };
    item(ui, &text, "")
}

/// Najechanie na inny tytuł, gdy menu jest otwarte, przełącza menu.
fn switch_on_hover(ctx: &egui::Context, buttons: &[egui::Response]) {
    let ids: Vec<egui::Id> = buttons.iter().map(egui::Popup::default_response_id).collect();
    let Some(open) = ids.iter().position(|id| egui::Popup::is_id_open(ctx, *id)) else { return };
    let Some(p) = ctx.pointer_hover_pos() else { return };
    if ctx.input(|i| i.pointer.delta() == egui::Vec2::ZERO) {
        return;
    }
    let reaches = |b: &egui::Response| {
        let r = b.interact_rect;
        let column = Rect::from_min_max(pos2(r.left(), f32::NEG_INFINITY), r.max);
        column.contains(p) && ctx.layer_id_at(p) == Some(b.layer_id)
    };
    if let Some(i) = buttons.iter().position(reaches)
        && i != open
    {
        egui::Popup::open_id(ctx, ids[i]);
        ctx.request_repaint();
    }
}

/// Klawiatura paska menu.
fn menu_keys(ctx: &egui::Context, buttons: &[egui::Response]) {
    let ids: Vec<egui::Id> = buttons.iter().map(egui::Popup::default_response_id).collect();
    let open = ids.iter().position(|id| egui::Popup::is_id_open(ctx, *id));
    let (alt_key, f10, left, right, esc) = ctx.input(|i| {
        let alt = TOP.iter().position(|(_, k)| i.modifiers.alt && !i.modifiers.command && i.key_pressed(*k));
        (alt, i.key_pressed(egui::Key::F10), i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.key_pressed(egui::Key::Escape))
    });
    let target = if let Some(k) = alt_key {
        Some(k)
    } else if f10 {
        Some(open.map_or(0, |_| usize::MAX))
    } else if let Some(o) = open {
        if left {
            Some((o + ids.len() - 1) % ids.len())
        } else if right {
            Some((o + 1) % ids.len())
        } else {
            None
        }
    } else {
        None
    };
    match target {
        Some(usize::MAX) => egui::Popup::close_all(ctx),
        Some(k) => egui::Popup::open_id(ctx, ids[k]),
        None => {
            if esc && open.is_some() {
                egui::Popup::close_all(ctx);
            }
        }
    }
}

fn menu_contents(index: usize, ui: &mut Ui, settings: &Settings, inspection: bool, out: &mut Vec<MenuAction>) {
    use crate::settings::{HideItem, PrefsSection};
    ui.set_min_width(220.0);
    match index {
        0 => {
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
        }
        1 => {
            ui.menu_button("Preferencje", |ui| {
                ui.set_min_width(200.0);
                if item(ui, "Interfejs…", "") {
                    out.push(MenuAction::Preferences(PrefsSection::Interface));
                }
                if item(ui, "Eksport i limity…", "") {
                    out.push(MenuAction::Preferences(PrefsSection::Export));
                }
                if item(ui, "Usuwanie tła (AI)…", "") {
                    out.push(MenuAction::Preferences(PrefsSection::Ai));
                }
            });
            ui.menu_button("Motyw", |ui| {
                for kind in ThemeKind::ALL {
                    if check_item(ui, settings.theme == kind, kind.label()) {
                        out.push(MenuAction::SetTheme(kind));
                    }
                }
            });
            ui.menu_button("Ukryj", |ui| {
                ui.set_min_width(260.0);
                for hide in HideItem::ALL {
                    if check_item(ui, settings.hidden(hide), hide.label()) {
                        out.push(MenuAction::ToggleHidden(hide));
                    }
                }
            });
        }
        2 => {
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
            if !settings.hide_inspection {
                ui.separator();
                if check_item(ui, inspection, "Inspekcja tła   (I)") {
                    out.push(MenuAction::ToggleInspection);
                }
            }
        }
        _ => {
            if item(ui, "Skróty klawiszowe", "F1") {
                out.push(MenuAction::Shortcuts);
            }
            if item(ui, "O programie AsOne", "") {
                out.push(MenuAction::About);
            }
        }
    }
}

/// Rysuje menu i zwraca prawą krawędź ostatniego tytułu menu.
fn menus(ui: &mut Ui, settings: &Settings, inspection: bool, out: &mut Vec<MenuAction>) -> f32 {
    let pal = theme::p();
    let ctx = ui.ctx().clone();
    let mut right = ui.cursor().left();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let v = &mut ui.style_mut().visuals;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.spacing_mut().button_padding = vec2(7.0, 3.0);
            let mut buttons = Vec::with_capacity(TOP.len());
            for (k, (name, _)) in TOP.iter().enumerate() {
                let title = ui.add(egui::Button::new(egui::RichText::new(*name).color(pal.text_dim)));
                let bar = egui::containers::menu::MenuConfig::find(ui);
                let config = egui::containers::menu::MenuConfig::new()
                    .close_behavior(bar.close_behavior)
                    .style(bar.style.clone());
                let open = title_press(&ctx, &title);
                // Puszczenie przycisku kończące gest otwarcia nie może zamknąć menu.
                let opening = title.clicked() && ctx.data(|d| d.get_temp::<bool>(press_gesture_id())).unwrap_or(false);
                let close = if opening { egui::PopupCloseBehavior::IgnoreClicks } else { config.close_behavior };
                egui::Popup::menu(&title)
                    .open_memory(open)
                    .close_behavior(close)
                    .style(config.style.clone())
                    .info(
                        egui::UiStackInfo::new(egui::UiKind::Menu)
                            .with_tag_value(egui::containers::menu::MenuConfig::MENU_CONFIG_TAG, config),
                    )
                    .show(|ui| menu_contents(k, ui, settings, inspection, out));
                right = right.max(title.rect.right());
                buttons.push(title);
            }
            switch_on_hover(&ctx, &buttons);
            menu_keys(&ctx, &buttons);
        });
    });
    // Gest „wciśnij – przeciągnij” kończy się z puszczeniem przycisku.
    if ctx.input(|i| i.pointer.primary_released() || !i.pointer.primary_down()) {
        ctx.data_mut(|d| d.remove::<bool>(press_gesture_id()));
    }
    right
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
                ui.add_space(4.0);
                // Szybkie przełączanie motywu (jasny / ciemny)
                let is_dark = theme::p().is_dark();
                let theme_icon = if is_dark { "☀" } else { "🌙" };
                let theme_tip = if is_dark { "Przełącz motyw na jasny" } else { "Przełącz motyw na ciemny" };
                if ui.add(egui::Button::new(egui::RichText::new(theme_icon).size(14.0)).min_size(vec2(24.0, 22.0))).on_hover_text(theme_tip).clicked() {
                    let next = if is_dark { ThemeKind::ProLight } else { ThemeKind::AsOne };
                    actions.push(MenuAction::SetTheme(next));
                }
                ui.add_space(2.0);
                // Ikonka wyszukiwania / skrótów klawiszowych
                if ui.add(egui::Button::new(egui::RichText::new("🔍").size(13.0)).min_size(vec2(24.0, 22.0))).on_hover_text("Szukaj / Skróty klawiszowe").clicked() {
                    actions.push(MenuAction::Shortcuts);
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
