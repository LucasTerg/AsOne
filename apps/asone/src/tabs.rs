//! Zakładki `egui_dock`: stała lista plików i zakładki ze zdjęciami.
//!
//! Kolejność jest jedna: miniatury na liście i zakładki ze zdjęciami są zawsze w tej samej
//! kolejności – przeciągnięcie miniatury przestawia zakładki, a przeciągnięcie zakładki – listę.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::{Color32, CornerRadius, Id, Rect, Sense, Stroke, Ui, WidgetText, pos2, vec2};
use egui_dock::{DockState, Node, NodeIndex, NodePath, Tree};

use crate::canvas::{BrushSettings, Tool, photo_ui};
use crate::document::{DocId, Document, LoadState, ThumbState};
use crate::renderer::GpuShared;
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppTab {
    /// Stała, niezamykalna lista wszystkich wrzuconych zdjęć.
    FileList,
    /// Zakładka robocza jednego zdjęcia.
    Photo(DocId),
}

/// Miejsca zakładek `AppTab::Photo` w doku, w kolejności numeracji.
///
/// Drzewo przechodzimy w głąb, zawsze najpierw lewym/górnym dzieckiem, więc przy podzielonym
/// widoku numeracja idzie w kolejności czytania (lewa → prawa, góra → dół). `FileList` jest pomijana.
pub fn photo_tab_slots(dock: &DockState<AppTab>) -> Vec<(NodePath, usize, DocId)> {
    fn visit(
        tree: &Tree<AppTab>,
        surface: egui_dock::SurfaceIndex,
        node: NodeIndex,
        out: &mut Vec<(NodePath, usize, DocId)>,
    ) {
        if node.0 >= tree.len() {
            return;
        }
        match &tree[node] {
            Node::Leaf(leaf) => {
                for (i, tab) in leaf.tabs.iter().enumerate() {
                    if let AppTab::Photo(id) = tab {
                        out.push((NodePath { surface, node }, i, *id));
                    }
                }
            }
            Node::Vertical(_) | Node::Horizontal(_) => {
                visit(tree, surface, node.left(), out);
                visit(tree, surface, node.right(), out);
            }
            Node::Empty => {}
        }
    }
    let mut out = Vec::new();
    for (index, surface) in dock.iter_surfaces_indexed() {
        if let Some(tree) = surface.node_tree() {
            visit(tree, index, NodeIndex::root(), &mut out);
        }
    }
    out
}

/// Kolejność zakładek ze zdjęciami – wyznacza numerację plików i kolejność zapisu.
pub fn photo_tab_order(dock: &DockState<AppTab>) -> Vec<DocId> {
    photo_tab_slots(dock).into_iter().map(|(_, _, id)| id).collect()
}

/// Przestawia zakładki ze zdjęciami tak, by szły w kolejności `desired` (układ doku zostaje:
/// każde miejsce dostaje kolejny dokument). Aktywna zakładka w każdym panelu zostaje ta sama.
pub fn reorder_photo_tabs(dock: &mut DockState<AppTab>, desired: &[DocId]) {
    let slots = photo_tab_slots(dock);
    if slots.len() != desired.len() || slots.iter().map(|s| s.2).eq(desired.iter().copied()) {
        return;
    }
    let mut active: HashMap<NodePath, Option<AppTab>> = HashMap::new();
    for (path, _, _) in &slots {
        if let Ok(leaf) = dock.leaf(*path) {
            active.insert(*path, leaf.tabs.get(leaf.active.0).copied());
        }
    }
    for ((path, index, _), id) in slots.iter().zip(desired) {
        if let Ok(leaf) = dock.leaf_mut(*path) {
            leaf.tabs[*index] = AppTab::Photo(*id);
        }
    }
    for (path, tab) in active {
        if let (Ok(leaf), Some(tab)) = (dock.leaf_mut(path), tab)
            && let Some(i) = leaf.tabs.iter().position(|t| *t == tab)
        {
            leaf.active = egui_dock::TabIndex(i);
        }
    }
}

/// Przestawia `order` tak, by otwarte dokumenty szły w kolejności zakładek (zamknięte zostają na miejscu).
pub fn reorder_list_by_tabs(order: &mut [DocId], tabs: &[DocId]) {
    let open: HashSet<DocId> = tabs.iter().copied().collect();
    let mut next = tabs.iter().copied();
    for slot in order.iter_mut() {
        if open.contains(slot)
            && let Some(id) = next.next()
        {
            *slot = id;
        }
    }
}

/// Przenosi `id` przed/za `target`.
pub fn move_in_order(order: &mut Vec<DocId>, id: DocId, target: DocId, before: bool) {
    if id == target {
        return;
    }
    let Some(from) = order.iter().position(|d| *d == id) else { return };
    order.remove(from);
    let Some(t) = order.iter().position(|d| *d == target) else {
        order.insert(from.min(order.len()), id);
        return;
    };
    order.insert(if before { t } else { t + 1 }, id);
}

/// Działania zgłaszane z wnętrza doku i wykonywane po jego narysowaniu.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Open(DocId),
    CloseTab(DocId),
    Remove(Vec<DocId>),
    OpenAll,
    AddFiles,
    LoadFolder,
    SwitchTool(Tool),
    /// Przeciągnięcie miniatury na liście.
    Move { id: DocId, target: DocId, before: bool },
    OpenFile(DocId),
    OpenFolder(DocId),
    EditInGimp(DocId),
    Reload(Vec<DocId>),
    ZipSelected(Vec<DocId>),
    PdfSelected(Vec<DocId>),
    DeleteFromDisk(Vec<DocId>),
    /// „Odznacz”: zamyka wszystkie zakładki ze zdjęciami i czyści zaznaczenie.
    CloseAllTabs,
    /// „Wyczyść listę” / „Usuń wszystkie z listy” (z potwierdzeniem).
    ClearList,
    /// Dopisek wersji w nazwie wynikowej (`-ver2`…); `None` usuwa dopisek.
    SetVersion(Vec<DocId>, Option<u8>),
}

/// Podmenu „Dopisz wersję”: `-ver2` (zwykle), `-ver3`, `-ver4`, `-ver5`, „Bez dopisku”.
fn version_menu(ui: &mut Ui, targets: &[DocId], current: Option<u8>, actions: &mut Vec<Action>) {
    let suffix = if targets.len() > 1 { format!(" ({})", targets.len()) } else { String::new() };
    ui.menu_button(format!("Dopisz wersję (-ver){suffix}"), |ui| {
        ui.set_min_width(170.0);
        for v in 2u8..=5 {
            let mark = if current == Some(v) { "✔ " } else { "    " };
            let hint = if v == 2 { "  (zwykle)" } else { "" };
            if ui.button(format!("{mark}-ver{v}{hint}")).clicked() {
                actions.push(Action::SetVersion(targets.to_vec(), Some(v)));
                ui.close();
            }
        }
        ui.separator();
        if ui.add_enabled(current.is_some(), egui::Button::new("Bez dopisku")).clicked() {
            actions.push(Action::SetVersion(targets.to_vec(), None));
            ui.close();
        }
    });
}

pub struct Viewer<'a> {
    pub docs: &'a mut HashMap<DocId, Document>,
    pub order: &'a [DocId],
    /// Numer `#n` i nazwa wyjściowa dla otwartych zdjęć.
    pub numbering: &'a HashMap<DocId, (usize, Option<String>)>,
    pub selected: &'a mut HashSet<DocId>,
    pub settings: &'a crate::settings::Settings,
    pub gpu: Option<&'a Arc<GpuShared>>,
    pub tool: Tool,
    pub brush: &'a BrushSettings,
    pub inspection: bool,
    pub actions: &'a mut Vec<Action>,
    /// Android: folder do wczytania (zamiast systemowego okna wyboru).
    pub folder_input: &'a mut String,
}

impl egui_dock::TabViewer for Viewer<'_> {
    type Tab = AppTab;

    fn id(&mut self, tab: &mut AppTab) -> Id {
        Id::new(*tab)
    }

    fn title(&mut self, tab: &mut AppTab) -> WidgetText {
        match tab {
            AppTab::FileList => "Lista plików".into(),
            AppTab::Photo(id) => {
                let name = self.docs.get(id).map(|d| d.file_name.as_str()).unwrap_or("?");
                let number_only = self.settings.tab_title == crate::settings::TabTitle::NumberOnly;
                match self.numbering.get(id) {
                    Some((n, _)) if number_only => format!("#{n}").into(),
                    Some((n, Some(out))) => format!("#{n} · {out}").into(),
                    Some((n, None)) => format!("#{n} · {name}").into(),
                    None => name.into(),
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut AppTab) {
        match *tab {
            AppTab::FileList => self.file_list(ui),
            AppTab::Photo(id) => {
                if let Some(doc) = self.docs.get_mut(&id) {
                    let out = photo_ui(ui, doc, self.gpu, self.tool, self.brush, self.inspection);
                    if let Some(t) = out.switch_tool {
                        self.actions.push(Action::SwitchTool(t));
                    }
                }
            }
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut AppTab, _path: egui_dock::NodePath) {
        if let AppTab::Photo(id) = *tab {
            let current = self.docs.get(&id).and_then(|d| d.version);
            version_menu(ui, &[id], current, self.actions);
        }
    }

    fn on_tab_button(&mut self, tab: &mut AppTab, response: &egui::Response) {
        if let AppTab::Photo(id) = tab
            && let Some(doc) = self.docs.get(id)
        {
            let where_from = doc.source.path().map(|p| p.display().to_string()).unwrap_or_else(|| doc.file_name.clone());
            let text = match self.numbering.get(id) {
                Some((n, out)) => format!("#{n} · {}\nOryginał: {where_from}", out.as_deref().unwrap_or(&doc.file_name)),
                None => format!("Oryginał: {where_from}"),
            };
            response.clone().on_hover_text(text);
        }
    }

    fn is_closeable(&self, tab: &AppTab) -> bool {
        !matches!(tab, AppTab::FileList)
    }

    fn allowed_in_windows(&self, _tab: &mut AppTab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &AppTab) -> [bool; 2] {
        [false, false]
    }

    /// Ustawienie „Zakładki zdjęć: na dole” – dotyczy liści ze zdjęciami (lista plików zostaje u góry).
    fn tab_bar_at_bottom(&self, tabs: &[AppTab]) -> bool {
        self.settings.photo_tabs_bottom && tabs.iter().any(|t| matches!(t, AppTab::Photo(_)))
    }
}

const CELL: egui::Vec2 = egui::Vec2::new(132.0, 166.0);

impl Viewer<'_> {
    fn file_list(&mut self, ui: &mut Ui) {
        let pal = theme::p();
        let open_count = self.numbering.len();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if crate::platform::CAN_PICK_FILES && ui.button("Dodaj zdjęcia…").clicked() {
                    self.actions.push(Action::AddFiles);
                }
                if !crate::platform::CAN_PICK_FILES {
                    ui.label("Folder:");
                    ui.add(egui::TextEdit::singleline(self.folder_input).desired_width(220.0));
                    if ui.button("Wczytaj").clicked() {
                        self.actions.push(Action::LoadFolder);
                    }
                }
                let can_open_all = self.order.len() > open_count;
                if ui.add_enabled(can_open_all, egui::Button::new("Otwórz wszystkie")).clicked() {
                    self.actions.push(Action::OpenAll);
                }
                let can_deselect = open_count > 0 || !self.selected.is_empty();
                if ui
                    .add_enabled(can_deselect, egui::Button::new("Odznacz"))
                    .on_hover_text("Zamyka wszystkie zakładki ze zdjęciami (pliki zostają na liście)")
                    .clicked()
                {
                    self.actions.push(Action::CloseAllTabs);
                }
                if ui
                    .add_enabled(!self.order.is_empty(), egui::Button::new("Wyczyść listę"))
                    .on_hover_text("Usuwa wszystkie zdjęcia z listy (pliki na dysku zostają)")
                    .clicked()
                {
                    self.actions.push(Action::ClearList);
                }
                let mut info = format!("Plików: {} · w zakładkach: {open_count}", self.order.len());
                if !self.selected.is_empty() {
                    info.push_str(&format!(" · zaznaczone: {}", self.selected.len()));
                }
                ui.colored_label(pal.text_dim, info);
            });
        });
        ui.separator();

        if self.order.is_empty() {
            ui.centered_and_justified(|ui| {
                let hint = if cfg!(target_os = "android") {
                    "Wpisz folder ze zdjęciami i naciśnij „Wczytaj”"
                } else {
                    "Przeciągnij tutaj zdjęcia (JPG, PNG, WebP, TIFF, BMP)\nlub użyj „Dodaj zdjęcia…”"
                };
                ui.colored_label(pal.text_dim, hint);
            });
            return;
        }

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            ui.horizontal_wrapped(|ui| {
                for &id in self.order {
                    self.thumbnail_cell(ui, id);
                }
            });
        });

        // Miniatura „w locie” przy kursorze podczas przeciągania.
        if let Some(dragged) = egui::DragAndDrop::payload::<DocId>(ui.ctx())
            && let (Some(pos), Some(doc)) = (ui.ctx().pointer_latest_pos(), self.docs.get(&dragged))
        {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("drag_ghost")));
            let r = Rect::from_center_size(pos + vec2(28.0, 28.0), vec2(56.0, 56.0));
            painter.rect_filled(r, CornerRadius::same(3), pal.card.gamma_multiply(0.9));
            if let ThumbState::Ready(tex) = &doc.thumb {
                let s = tex.size_vec2();
                let k = (52.0 / s.x).min(52.0 / s.y);
                let ir = Rect::from_center_size(r.center(), s * k);
                painter.image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha(220));
            }
            painter.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
        }
    }

    /// Dokumenty, na których działa menu kontekstowe: zaznaczenie albo sama kliknięta miniatura.
    fn targets(&self, id: DocId) -> Vec<DocId> {
        if self.selected.contains(&id) && self.selected.len() > 1 {
            self.order.iter().copied().filter(|d| self.selected.contains(d)).collect()
        } else {
            vec![id]
        }
    }

    fn select_click(&mut self, id: DocId, modifiers: egui::Modifiers) {
        if modifiers.command {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
        } else if modifiers.shift {
            let anchor = self.order.iter().position(|d| self.selected.contains(d));
            let here = self.order.iter().position(|d| *d == id);
            if let (Some(a), Some(b)) = (anchor, here) {
                let (lo, hi) = (a.min(b), a.max(b));
                self.selected.extend(self.order[lo..=hi].iter().copied());
            } else {
                self.selected.insert(id);
            }
        } else if self.numbering.contains_key(&id) {
            // Kliknięcie otwartego zdjęcia zamyka jego zakładkę (odznacza).
            self.selected.remove(&id);
            self.actions.push(Action::CloseTab(id));
        } else {
            self.selected.clear();
            self.selected.insert(id);
            self.actions.push(Action::Open(id));
        }
    }

    fn thumbnail_cell(&mut self, ui: &mut Ui, id: DocId) {
        let pal = theme::p();
        let Some(doc) = self.docs.get(&id) else { return };
        let (rect, response) = ui.allocate_exact_size(CELL, Sense::click_and_drag());
        let number = self.numbering.get(&id).map(|(n, _)| *n);
        let selected = self.selected.contains(&id);
        let dragging_this = egui::DragAndDrop::payload::<DocId>(ui.ctx()).is_some_and(|p| *p == id);
        let painter = ui.painter_at(rect.expand(5.0));

        let bg = if selected {
            pal.row_selected
        } else if response.hovered() {
            pal.hover
        } else {
            pal.field
        };
        painter.rect_filled(rect, CornerRadius::same(pal.radius), if dragging_this { bg.gamma_multiply(0.5) } else { bg });
        if number.is_some() {
            painter.rect_stroke(rect, CornerRadius::same(pal.radius), Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
        }

        let img_area = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(CELL.x - 12.0, CELL.x - 12.0));
        match &doc.thumb {
            ThumbState::Ready(tex) => {
                let s = tex.size_vec2();
                let scale = (img_area.width() / s.x).min(img_area.height() / s.y);
                let r = Rect::from_center_size(img_area.center(), s * scale);
                painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            ThumbState::Loading => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(img_area), |ui| {
                    ui.centered_and_justified(|ui| ui.spinner());
                });
            }
            ThumbState::Failed(_) => {
                painter.text(img_area.center(), egui::Align2::CENTER_CENTER, "Błąd", egui::FontId::proportional(13.0), pal.danger);
            }
        }

        if let Some(n) = number {
            let badge = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(30.0, 18.0));
            painter.rect_filled(badge, CornerRadius::same(3), pal.accent);
            painter.text(badge.center(), egui::Align2::CENTER_CENTER, format!("#{n}"), egui::FontId::proportional(12.0), pal.accent_text);
        }
        if let Some(v) = doc.version {
            let x = if number.is_some() { 38.0 } else { 4.0 };
            let badge = Rect::from_min_size(rect.min + vec2(x, 4.0), vec2(36.0, 18.0));
            painter.rect_filled(badge, CornerRadius::same(3), pal.warning);
            painter.text(badge.center(), egui::Align2::CENTER_CENTER, format!("ver{v}"), egui::FontId::proportional(11.0), Color32::BLACK);
        }
        if matches!(doc.load, LoadState::Loading) {
            painter.circle_filled(rect.right_top() + vec2(-14.0, 14.0), 5.0, pal.accent);
        }

        let name_pos = pos2(rect.center().x, img_area.bottom() + 12.0);
        painter.text(name_pos, egui::Align2::CENTER_CENTER, truncate(&doc.file_name, 18), egui::FontId::proportional(12.0), pal.text);
        if let Some([w, h]) = doc.dims {
            painter.text(
                name_pos + vec2(0.0, 15.0),
                egui::Align2::CENTER_CENTER,
                format!("{w}×{h}"),
                egui::FontId::proportional(11.0),
                pal.text_dim,
            );
        }

        // Przeciąganie: miniatura jest źródłem i celem upuszczenia.
        response.dnd_set_drag_payload(id);
        if let Some(payload) = response.dnd_hover_payload::<DocId>()
            && *payload != id
            && let Some(p) = ui.ctx().pointer_latest_pos()
        {
            let x = if p.x < rect.center().x { rect.left() - 4.0 } else { rect.right() + 4.0 };
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(3.0, pal.accent));
        }
        if let Some(payload) = response.dnd_release_payload::<DocId>() {
            let before = ui.ctx().pointer_latest_pos().is_none_or(|p| p.x < rect.center().x);
            self.actions.push(Action::Move { id: *payload, target: id, before });
        }

        let hover = match &doc.thumb {
            ThumbState::Failed(e) => format!("{}\n{e}", doc.file_name),
            _ => format!(
                "{}\n{:.1} MB · klik – otwórz / zamknij zakładkę · Ctrl/Shift+klik – zaznacz · przeciągnij – zmień kolejność",
                doc.file_name,
                doc.file_size as f64 / 1e6
            ),
        };
        let has_path = doc.source.path().is_some();
        let response = response.on_hover_text(hover);
        if response.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            self.select_click(id, modifiers);
        }
        let targets = self.targets(id);
        response.context_menu(|ui| {
            let many = targets.len() > 1;
            let suffix = if many { format!(" ({})", targets.len()) } else { String::new() };
            if !many && ui.button("Otwórz w zakładce").clicked() {
                self.actions.push(Action::Open(id));
                ui.close();
            }
            if !many && number.is_some() && ui.button("Zamknij zakładkę").clicked() {
                self.actions.push(Action::CloseTab(id));
                ui.close();
            }
            let current = targets.iter().map(|t| self.docs.get(t).and_then(|d| d.version)).reduce(|a, b| if a == b { a } else { None });
            version_menu(ui, &targets, current.flatten(), self.actions);
            if crate::platform::FILE_OPS && has_path {
                ui.separator();
                if !many && ui.button("Otwórz plik").clicked() {
                    self.actions.push(Action::OpenFile(id));
                    ui.close();
                }
                if !many && ui.button("Otwórz folder pliku").clicked() {
                    self.actions.push(Action::OpenFolder(id));
                    ui.close();
                }
                if !many && !self.settings.hide_gimp && ui.button("Edytuj w GIMP").clicked() {
                    self.actions.push(Action::EditInGimp(id));
                    ui.close();
                }
                if ui.button(format!("Wczytaj ponownie z dysku{suffix}")).clicked() {
                    self.actions.push(Action::Reload(targets.clone()));
                    ui.close();
                }
                ui.separator();
                if ui.button(format!("Zapisz do ZIP…{suffix}")).clicked() {
                    self.actions.push(Action::ZipSelected(targets.clone()));
                    ui.close();
                }
                if ui.button(format!("Eksportuj do PDF…{suffix}")).clicked() {
                    self.actions.push(Action::PdfSelected(targets.clone()));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button(format!("Usuń z listy{suffix}")).clicked() {
                self.actions.push(Action::Remove(targets.clone()));
                ui.close();
            }
            if ui.button(format!("Usuń wszystkie z listy ({})", self.order.len())).clicked() {
                self.actions.push(Action::ClearList);
                ui.close();
            }
            if crate::platform::FILE_OPS && has_path && !self.settings.hide_delete_from_disk {
                let danger = egui::RichText::new(format!("Usuń z dysku…{suffix}")).color(theme::p().danger);
                if ui.button(danger).clicked() {
                    self.actions.push(Action::DeleteFromDisk(targets.clone()));
                    ui.close();
                }
            }
        });
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max - 1).collect();
    format!("{head}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_dock::{SurfaceIndex, TabIndex, TabPath};

    fn ids(v: &[u64]) -> Vec<DocId> {
        v.iter().map(|&i| DocId(i)).collect()
    }

    #[test]
    fn numeracja_pomija_liste_plikow() {
        let dock = DockState::new(vec![AppTab::Photo(DocId(3)), AppTab::FileList, AppTab::Photo(DocId(1))]);
        assert_eq!(photo_tab_order(&dock), ids(&[3, 1]));
    }

    #[test]
    fn przeciagniecie_zakladki_zmienia_kolejnosc() {
        let mut dock = DockState::new(vec![
            AppTab::FileList,
            AppTab::Photo(DocId(1)),
            AppTab::Photo(DocId(2)),
            AppTab::Photo(DocId(3)),
        ]);
        let leaf = NodePath { surface: SurfaceIndex::main(), node: NodeIndex::root() };
        let tab = dock.remove_tab(TabPath::new(SurfaceIndex::main(), NodeIndex::root(), TabIndex(3))).unwrap();
        dock.leaf_mut(leaf).unwrap().insert_tab(TabIndex(1), tab);
        assert_eq!(photo_tab_order(&dock), ids(&[3, 1, 2]));
    }

    #[test]
    fn podzielony_widok_w_kolejnosci_czytania() {
        let mut dock = DockState::new(vec![AppTab::FileList]);
        let main = dock.main_surface_mut();
        let [_, right] = main.split_right(NodeIndex::root(), 0.3, vec![AppTab::Photo(DocId(1)), AppTab::Photo(DocId(2))]);
        main.split_left(right, 0.5, vec![AppTab::Photo(DocId(9))]);
        assert_eq!(photo_tab_order(&dock), ids(&[9, 1, 2]));
    }

    #[test]
    fn lista_przestawia_zakladki_i_odwrotnie() {
        let mut dock = DockState::new(vec![
            AppTab::FileList,
            AppTab::Photo(DocId(1)),
            AppTab::Photo(DocId(2)),
            AppTab::Photo(DocId(3)),
        ]);
        let leaf = NodePath { surface: SurfaceIndex::main(), node: NodeIndex::root() };
        dock.leaf_mut(leaf).unwrap().active = TabIndex(2);
        reorder_photo_tabs(&mut dock, &ids(&[3, 1, 2]));
        assert_eq!(photo_tab_order(&dock), ids(&[3, 1, 2]));
        let l = dock.leaf(leaf).unwrap();
        assert_eq!(l.tabs[l.active.0], AppTab::Photo(DocId(2)), "aktywna zakładka zostaje ta sama");

        let mut order = ids(&[5, 1, 7, 2, 3]);
        reorder_list_by_tabs(&mut order, &ids(&[3, 1, 2]));
        assert_eq!(order, ids(&[5, 3, 7, 1, 2]));
    }

    #[test]
    fn przenoszenie_na_liscie() {
        let mut order = ids(&[1, 2, 3, 4]);
        move_in_order(&mut order, DocId(4), DocId(1), true);
        assert_eq!(order, ids(&[4, 1, 2, 3]));
        move_in_order(&mut order, DocId(4), DocId(3), false);
        assert_eq!(order, ids(&[1, 2, 3, 4]));
        move_in_order(&mut order, DocId(2), DocId(2), true);
        assert_eq!(order, ids(&[1, 2, 3, 4]));
    }

    #[test]
    fn skracanie_nazw() {
        assert_eq!(truncate("krotka.jpg", 18), "krotka.jpg");
        assert_eq!(truncate("bardzo-dluga-nazwa-pliku.jpg", 10), "bardzo-dl…");
    }
}
