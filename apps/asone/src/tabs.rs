//! Zakładki `egui_dock`: stała lista plików i zakładki ze zdjęciami.
//!
//! Kolejność jest jedna: miniatury na liście i zakładki ze zdjęciami są zawsze w tej samej
//! kolejności – przeciągnięcie miniatury przestawia zakładki, a przeciągnięcie zakładki – listę.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::{Color32, CornerRadius, Id, Rect, Sense, Stroke, Ui, WidgetText, pos2, vec2};
use egui_dock::{DockState, Node, NodeIndex, NodePath, Tree};

use crate::canvas::{BrushSettings, Tool, photo_ui};
use crate::settings::ListView;
use crate::document::{DocId, Document, LoadState, ThumbState};
use crate::renderer::GpuShared;
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppTab {
    /// Stała, niezamykalna lista wszystkich wrzuconych zdjęć (pusta – ekran startowy).
    FileList,
    /// Przyciski i widok listy plików (Dodaj zdjęcia, Otwórz wszystkie, Widok…) – osobna zakładka,
    /// którą można przeciągnąć w inne miejsce.
    ListTools,
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
    /// Polecenie z menu kontekstowego zdjęcia (płótna).
    Canvas(DocId, crate::canvas::CanvasAction),
    /// „Dodaj folder…”.
    AddFolder,
    /// Ekran startowy: wczytaj folder z „Ostatnich”.
    OpenRecent(String),
    /// Ekran startowy: usuń folder z „Ostatnich”.
    ForgetRecent(String),
}

/// Podgląd folderu z „Ostatnich” na ekranie startowym.
pub enum RecentPreview {
    Loading,
    Ready { count: usize, thumb: Option<egui::TextureHandle> },
    Missing,
}

/// „1 zdjęcie”, „3 zdjęcia”, „12 zdjęć”.
pub fn photos_count(n: usize) -> String {
    let word = match (n % 10, n % 100) {
        _ if n == 1 => "zdjęcie",
        (2..=4, r) if !(12..=14).contains(&r) => "zdjęcia",
        _ => "zdjęć",
    };
    format!("{n} {word}")
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
    /// Widok listy plików (zapisywany z powrotem do ustawień po narysowaniu).
    pub list: &'a mut crate::settings::ListOptions,
    pub gpu: Option<&'a Arc<GpuShared>>,
    pub tool: Tool,
    pub brush: &'a BrushSettings,
    pub inspection: bool,
    pub actions: &'a mut Vec<Action>,
    /// Android: folder do wczytania (zamiast systemowego okna wyboru).
    pub folder_input: &'a mut String,
    /// Ekran startowy: podglądy folderów z „Ostatnich” (`settings.recent_folders`).
    pub recent: &'a HashMap<String, RecentPreview>,
}

impl egui_dock::TabViewer for Viewer<'_> {
    type Tab = AppTab;

    fn id(&mut self, tab: &mut AppTab) -> Id {
        Id::new(*tab)
    }

    fn title(&mut self, tab: &mut AppTab) -> WidgetText {
        match tab {
            AppTab::FileList => "Lista plików".into(),
            AppTab::ListTools => "Opcje listy".into(),
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
            AppTab::ListTools => self.list_tools(ui),
            AppTab::Photo(id) => {
                if let Some(doc) = self.docs.get_mut(&id) {
                    let replicate = self.settings.replicate_active();
                    let out = photo_ui(ui, doc, self.gpu, self.tool, self.brush, self.inspection, replicate);
                    if let Some(t) = out.switch_tool {
                        self.actions.push(Action::SwitchTool(t));
                    }
                    self.actions.extend(out.actions.into_iter().map(|a| Action::Canvas(id, a)));
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
        !matches!(tab, AppTab::FileList | AppTab::ListTools)
    }

    fn allowed_in_windows(&self, _tab: &mut AppTab) -> bool {
        false
    }

    fn scroll_bars(&self, tab: &AppTab) -> [bool; 2] {
        [false, matches!(tab, AppTab::ListTools)]
    }

    /// Ustawienie „Zakładki zdjęć: na dole” – dotyczy liści ze zdjęciami (lista plików zostaje u góry).
    fn tab_bar_at_bottom(&self, tabs: &[AppTab]) -> bool {
        self.settings.photo_tabs_bottom && tabs.iter().any(|t| matches!(t, AppTab::Photo(_)))
    }
}


impl Viewer<'_> {
    /// Zakładka „Opcje listy”: przyciski, liczniki i widok listy plików.
    fn list_tools(&mut self, ui: &mut Ui) {
        let pal = theme::p();
        let open_count = self.numbering.len();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if crate::platform::CAN_PICK_FILES && ui.button("Dodaj zdjęcia…").clicked() {
                    self.actions.push(Action::AddFiles);
                }
                if crate::platform::CAN_PICK_FOLDER && ui.button("Dodaj folder…").clicked() {
                    self.actions.push(Action::AddFolder);
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
            self.list_options_bar(ui);
        });
    }

    fn file_list(&mut self, ui: &mut Ui) {
        let pal = theme::p();
        if self.order.is_empty() {
            self.start_screen(ui);
            return;
        }

        // Ctrl + kółko nad listą: rozmiar miniatur.
        if ui.rect_contains_pointer(ui.max_rect()) {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 && self.list.view != ListView::Details {
                self.list.thumb_size = (self.list.thumb_size * zoom).clamp(THUMB_MIN, THUMB_MAX);
            }
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.list.view {
            ListView::Thumbnails => {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                ui.horizontal_wrapped(|ui| {
                    for &id in self.order {
                        self.grid_cell(ui, id);
                    }
                });
            }
            ListView::List => {
                ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                for &id in self.order {
                    self.list_row(ui, id);
                }
            }
            ListView::Details => {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                details_header(ui);
                for (i, &id) in self.order.iter().enumerate() {
                    self.details_row(ui, id, i % 2 == 1);
                }
            }
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

    /// Ekran startowy (pusta lista): przyciski z lewej, „Ostatnie” foldery z prawej.
    fn start_screen(&mut self, ui: &mut Ui) {
        let pal = theme::p();
        let drop_hint = if cfg!(target_os = "android") {
            "Dodaj zdjęcia z Galerii albo Plików – „Dodaj zdjęcia…”"
        } else {
            "Przeciągnij tutaj zdjęcia (JPG, PNG, WebP, AVIF, TIFF, BMP) albo cały folder"
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(28.0);
            ui.horizontal_top(|ui| {
                ui.add_space(28.0);
                // Lewa kolumna: najważniejsze przyciski i odnośniki.
                ui.vertical(|ui| {
                    ui.set_width(180.0);
                    let primary = egui::Button::new(egui::RichText::new("Dodaj zdjęcia…").strong().color(pal.accent_text))
                        .fill(pal.accent)
                        .min_size(vec2(176.0, 32.0));
                    if crate::platform::CAN_PICK_FILES && ui.add(primary).clicked() {
                        self.actions.push(Action::AddFiles);
                    }
                    ui.add_space(4.0);
                    if crate::platform::CAN_PICK_FOLDER && ui.add(egui::Button::new("Dodaj folder…").min_size(vec2(176.0, 32.0))).clicked() {
                        self.actions.push(Action::AddFolder);
                    }
                    ui.add_space(24.0);
                    ui.hyperlink_to("Witryna AsOne", "https://asone.lmk.one/");
                    ui.hyperlink_to("Kod i wydania (GitHub)", "https://github.com/LucasTerg/AsOne");
                });
                ui.add_space(36.0);
                // Prawa kolumna: nazwa programu i ostatnio otwierane foldery.
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("AsOne").size(24.0).strong().color(pal.text));
                    ui.colored_label(pal.text_dim, "Zdjęcia produktowe do internetu: kadr, marginesy, białe tło, nazwy plików.");
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Ostatnie").size(17.0).strong().color(pal.text));
                    ui.add_space(6.0);
                    let folders = self.settings.recent_folders.clone();
                    if folders.is_empty() || !crate::platform::CAN_PICK_FOLDER {
                        ui.colored_label(pal.text_dim, "Tu pojawią się ostatnio otwierane foldery ze zdjęciami.");
                    } else {
                        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                        ui.horizontal_wrapped(|ui| {
                            for dir in &folders {
                                self.recent_card(ui, dir);
                            }
                        });
                    }
                    ui.add_space(18.0);
                    ui.colored_label(pal.text_dim, drop_hint);
                });
            });
        });
    }

    /// Kafelek folderu z „Ostatnich”: miniatura pierwszego zdjęcia, nazwa, ścieżka, liczba zdjęć.
    fn recent_card(&mut self, ui: &mut Ui, dir: &str) {
        let pal = theme::p();
        let size = vec2(168.0, 204.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let preview = self.recent.get(dir);
        let missing = matches!(preview, Some(RecentPreview::Missing));
        let fill = if response.hovered() && !missing { pal.hover } else { pal.card };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(pal.radius), fill);
        painter.rect_stroke(rect, CornerRadius::same(pal.radius), Stroke::new(1.0, pal.card_border), egui::StrokeKind::Inside);
        let thumb_area = Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(size.x - 16.0, 120.0));
        match preview {
            Some(RecentPreview::Ready { thumb: Some(tex), .. }) => {
                let s = tex.size_vec2();
                let k = (thumb_area.width() / s.x).min(thumb_area.height() / s.y);
                let ir = Rect::from_center_size(thumb_area.center(), s * k);
                painter.image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            Some(RecentPreview::Loading) | None => {
                painter.text(thumb_area.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(20.0), pal.text_faint);
            }
            _ => {
                painter.text(thumb_area.center(), egui::Align2::CENTER_CENTER, "📁", egui::FontId::proportional(40.0), pal.text_faint);
            }
        }
        let path = std::path::Path::new(dir);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.to_string());
        let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let info = match preview {
            Some(RecentPreview::Ready { count, .. }) => photos_count(*count),
            Some(RecentPreview::Missing) => "folder nie istnieje".to_string(),
            _ => String::new(),
        };
        let width = size.x - 16.0;
        let line = |text: String, font: egui::FontId, color: Color32, y: f32| {
            let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
            job.wrap = egui::text::TextWrapping::truncate_at_width(width);
            let galley = ui.painter().layout_job(job);
            painter.galley(pos2(rect.left() + 8.0, rect.top() + y), galley, color);
        };
        line(name, egui::FontId::proportional(14.0), if missing { pal.text_dim } else { pal.text }, 136.0);
        line(parent, egui::FontId::proportional(11.5), pal.text_dim, 158.0);
        line(info, egui::FontId::proportional(11.5), if missing { pal.danger } else { pal.text_faint }, 178.0);

        let response = response.on_hover_text(format!("{dir}\nKliknij, aby wczytać zdjęcia z tego folderu"));
        if response.clicked() {
            self.actions.push(Action::OpenRecent(dir.to_string()));
        }
        response.context_menu(|ui| {
            if ui.add_enabled(!missing, egui::Button::new("Wczytaj zdjęcia z folderu")).clicked() {
                self.actions.push(Action::OpenRecent(dir.to_string()));
                ui.close();
            }
            if ui.button("Usuń z ostatnich").clicked() {
                self.actions.push(Action::ForgetRecent(dir.to_string()));
                ui.close();
            }
        });
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

    /// Widok listy: Miniatury / Lista / Szczegóły, rozmiar miniatur, nazwy.
    fn list_options_bar(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Widok:");
            for (view, label, tip) in [
                (ListView::Thumbnails, "Miniatury", "Siatka miniatur"),
                (ListView::List, "Lista", "Miniatura i pełna, zawijana nazwa w wierszu"),
                (ListView::Details, "Szczegóły", "Tabela: nazwa, wymiary, rozmiar, typ"),
            ] {
                if ui.selectable_label(self.list.view == view, label).on_hover_text(tip).clicked() {
                    self.list.view = view;
                }
            }
            if self.list.view != ListView::Details {
                // „Rozmiar” z suwakiem razem – do nowego wiersza, gdy się nie mieszczą (wąska kolumna).
                if ui.available_rect_before_wrap().width() < 170.0 {
                    ui.end_row();
                } else {
                    ui.separator();
                }
                ui.label("Rozmiar");
                ui.add(egui::Slider::new(&mut self.list.thumb_size, THUMB_MIN..=THUMB_MAX).show_value(false))
                    .on_hover_text("Rozmiar miniatur (też Ctrl + kółko myszy nad listą)");
            }
            if self.list.view == ListView::Thumbnails {
                ui.checkbox(&mut self.list.show_names, "Nazwy");
            }
        });
    }

    /// Tło pozycji: zaznaczenie, najechanie, przeciąganie, ramka otwartej zakładki.
    fn item_background(&self, ui: &Ui, id: DocId, rect: Rect, response: &egui::Response, striped: bool) {
        let pal = theme::p();
        let selected = self.selected.contains(&id);
        let dragging_this = egui::DragAndDrop::payload::<DocId>(ui.ctx()).is_some_and(|p| *p == id);
        let bg = if selected {
            pal.row_selected
        } else if response.hovered() {
            pal.hover
        } else if striped {
            pal.field.gamma_multiply(0.6)
        } else {
            pal.field
        };
        let painter = ui.painter_at(rect.expand(2.0));
        painter.rect_filled(rect, CornerRadius::same(pal.radius), if dragging_this { bg.gamma_multiply(0.5) } else { bg });
        if self.numbering.contains_key(&id) {
            painter.rect_stroke(rect, CornerRadius::same(pal.radius), Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
        }
    }

    /// Miniatura wpisana w `area` (albo spinner / „Błąd”).
    fn paint_thumb(&self, ui: &mut Ui, doc: &Document, area: Rect) {
        let pal = theme::p();
        match &doc.thumb {
            ThumbState::Ready(tex) => {
                let s = tex.size_vec2();
                let scale = (area.width() / s.x).min(area.height() / s.y);
                let r = Rect::from_center_size(area.center(), s * scale);
                ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            ThumbState::Loading => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                    ui.centered_and_justified(|ui| ui.spinner());
                });
            }
            ThumbState::Failed(_) => {
                let font = egui::FontId::proportional((area.height() * 0.2).clamp(10.0, 13.0));
                ui.painter().text(area.center(), egui::Align2::CENTER_CENTER, "Błąd", font, pal.danger);
            }
        }
    }

    /// Plakietki `#n` i `verN` od lewego górnego rogu.
    fn paint_badges(&self, ui: &Ui, doc: &Document, at: egui::Pos2) {
        let pal = theme::p();
        let mut x = at.x;
        if let Some((n, _)) = self.numbering.get(&doc.id) {
            let text = format!("#{n}");
            let w = 12.0 + 7.0 * text.len() as f32;
            let badge = Rect::from_min_size(pos2(x, at.y), vec2(w, 18.0));
            ui.painter().rect_filled(badge, CornerRadius::same(3), pal.accent);
            ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), pal.accent_text);
            x += w + 4.0;
        }
        if let Some(v) = doc.version {
            let badge = Rect::from_min_size(pos2(x, at.y), vec2(36.0, 18.0));
            ui.painter().rect_filled(badge, CornerRadius::same(3), pal.warning);
            ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, format!("ver{v}"), egui::FontId::proportional(11.0), Color32::BLACK);
        }
    }

    fn grid_cell(&mut self, ui: &mut Ui, id: DocId) {
        let pal = theme::p();
        let Some(doc) = self.docs.get(&id) else { return };
        let s = self.list.thumb_size.round();
        let names = self.list.show_names;
        // Nazwa zawijana do 2 wierszy pod miniaturą, pod nią wymiary.
        let name_galley = names.then(|| {
            let mut job = egui::text::LayoutJob::simple(doc.file_name.clone(), egui::FontId::proportional(12.0), pal.text, s);
            job.wrap.max_rows = 2;
            job.wrap.break_anywhere = true;
            job.halign = egui::Align::Center;
            ui.painter().layout_job(job)
        });
        let text_h = name_galley.as_ref().map_or(0.0, |g| g.size().y + 20.0);
        let (rect, response) = ui.allocate_exact_size(vec2(s + 12.0, s + 12.0 + text_h), Sense::click_and_drag());
        self.item_background(ui, id, rect, &response, false);
        let img_area = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(s, s));
        self.paint_thumb(ui, doc, img_area);
        self.paint_badges(ui, doc, rect.min + vec2(4.0, 4.0));
        if matches!(doc.load, LoadState::Loading) {
            ui.painter().circle_filled(rect.right_top() + vec2(-14.0, 14.0), 5.0, pal.accent);
        }
        if let Some(g) = name_galley {
            let top = pos2(rect.center().x, img_area.bottom() + 4.0);
            ui.painter().galley(top, g.clone(), pal.text);
            if let Some([w, h]) = doc.dims {
                let y = top.y + g.size().y + 8.0;
                ui.painter().text(pos2(rect.center().x, y), egui::Align2::CENTER_CENTER, format!("{w}×{h}"), egui::FontId::proportional(11.0), pal.text_dim);
            }
        }
        self.item_behavior(ui, id, response, rect, false);
    }

    fn list_row(&mut self, ui: &mut Ui, id: DocId) {
        let pal = theme::p();
        let Some(doc) = self.docs.get(&id) else { return };
        let t = (self.list.thumb_size * 0.5).clamp(32.0, 140.0);
        let width = ui.available_width();
        let text_x = t + 18.0;
        let wrap = (width - text_x - 8.0).max(60.0);
        let has_badges = self.numbering.contains_key(&id) || doc.version.is_some();
        let name = ui.painter().layout(doc.file_name.clone(), egui::FontId::proportional(13.0), pal.text, wrap);
        let badge_h = if has_badges { 22.0 } else { 0.0 };
        let text_h = badge_h + name.size().y + 18.0;
        let (rect, response) = ui.allocate_exact_size(vec2(width, (t + 8.0).max(text_h + 8.0)), Sense::click_and_drag());
        self.item_background(ui, id, rect, &response, false);
        let img_area = Rect::from_min_size(pos2(rect.left() + 6.0, rect.center().y - t / 2.0), vec2(t, t));
        self.paint_thumb(ui, doc, img_area);
        let x = rect.left() + text_x;
        let mut y = rect.center().y - text_h / 2.0;
        if has_badges {
            self.paint_badges(ui, doc, pos2(x, y));
            y += badge_h;
        }
        ui.painter().galley(pos2(x, y), name.clone(), pal.text);
        ui.painter().text(pos2(x, y + name.size().y + 2.0), egui::Align2::LEFT_TOP, file_info(doc), egui::FontId::proportional(11.0), pal.text_dim);
        self.item_behavior(ui, id, response, rect, true);
    }

    fn details_row(&mut self, ui: &mut Ui, id: DocId, striped: bool) {
        let pal = theme::p();
        let Some(doc) = self.docs.get(&id) else { return };
        let width = ui.available_width();
        let cols = details_columns(width);
        let name = ui.painter().layout(doc.file_name.clone(), egui::FontId::proportional(13.0), pal.text, cols.name_w);
        let (rect, response) = ui.allocate_exact_size(vec2(width, (name.size().y + 10.0).max(34.0)), Sense::click_and_drag());
        self.item_background(ui, id, rect, &response, striped);
        let cy = rect.center().y;
        let at = |x: f32| pos2(rect.left() + x, cy);
        let small = egui::FontId::proportional(12.0);
        if let Some((n, _)) = self.numbering.get(&id) {
            ui.painter().text(at(cols.num), egui::Align2::LEFT_CENTER, format!("#{n}"), small.clone(), pal.accent);
        }
        self.paint_thumb(ui, doc, Rect::from_center_size(at(cols.thumb + 14.0), vec2(26.0, 26.0)));
        ui.painter().galley(pos2(rect.left() + cols.name, cy - name.size().y / 2.0), name.clone(), pal.text);
        if let Some(v) = doc.version {
            let name_end = cols.name + cols.name_w + 10.0;
            ui.painter().text(at(name_end), egui::Align2::RIGHT_CENTER, format!("ver{v}"), small.clone(), pal.warning);
        }
        if let Some(x) = cols.dims {
            let dims = doc.dims.map_or("…".to_string(), |[w, h]| format!("{w}×{h}"));
            ui.painter().text(at(x), egui::Align2::LEFT_CENTER, dims, small.clone(), pal.text);
        }
        if let Some(x) = cols.size {
            ui.painter().text(at(x), egui::Align2::LEFT_CENTER, human_size(doc.file_size), small.clone(), pal.text_dim);
        }
        if let Some(x) = cols.kind {
            ui.painter().text(at(x), egui::Align2::LEFT_CENTER, extension(doc), small, pal.text_dim);
        }
        self.item_behavior(ui, id, response, rect, true);
    }

    /// Wspólne dla wszystkich widoków: przeciąganie (kolejność), klik, podpowiedź, menu kontekstowe.
    fn item_behavior(&mut self, ui: &mut Ui, id: DocId, response: egui::Response, rect: Rect, rows: bool) {
        let pal = theme::p();
        let number = self.numbering.get(&id).map(|(n, _)| *n);
        // Przeciąganie: miniatura jest źródłem i celem upuszczenia.
        response.dnd_set_drag_payload(id);
        // Siatka: znacznik pionowy (przed / za); lista i szczegóły: poziomy (nad / pod).
        let before_of = |p: egui::Pos2| if rows { p.y < rect.center().y } else { p.x < rect.center().x };
        if let Some(payload) = response.dnd_hover_payload::<DocId>()
            && *payload != id
            && let Some(p) = ui.ctx().pointer_latest_pos()
        {
            let painter = ui.painter_at(rect.expand(5.0));
            let stroke = Stroke::new(3.0, pal.accent);
            if rows {
                let y = if before_of(p) { rect.top() - 1.0 } else { rect.bottom() + 1.0 };
                painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
            } else {
                let x = if before_of(p) { rect.left() - 4.0 } else { rect.right() + 4.0 };
                painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
            }
        }
        if let Some(payload) = response.dnd_release_payload::<DocId>() {
            let before = ui.ctx().pointer_latest_pos().is_none_or(before_of);
            self.actions.push(Action::Move { id: *payload, target: id, before });
        }

        let Some(doc) = self.docs.get(&id) else { return };
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

const THUMB_MIN: f32 = 56.0;
const THUMB_MAX: f32 = 280.0;

/// Kolumny widoku „Szczegóły” (x od lewej krawędzi wiersza); przy wąskiej liście znikają
/// kolejno: Typ, Rozmiar, Wymiary – nazwa zawsze zostaje.
struct DetailsColumns {
    num: f32,
    thumb: f32,
    name: f32,
    name_w: f32,
    dims: Option<f32>,
    size: Option<f32>,
    kind: Option<f32>,
}

fn details_columns(width: f32) -> DetailsColumns {
    let (num, thumb, name) = (8.0, 48.0, 84.0);
    const MIN_NAME: f32 = 100.0;
    const DIMS: f32 = 84.0;
    const SIZE: f32 = 70.0;
    const KIND: f32 = 44.0;
    // Najważniejsze kolumny zostają najdłużej: Wymiary, potem Rozmiar, na końcu Typ.
    let mut budget = width - name - 12.0 - MIN_NAME;
    let mut fits = |w: f32| {
        let ok = budget >= w;
        if ok {
            budget -= w;
        }
        ok
    };
    let show_dims = fits(DIMS);
    let show_size = show_dims && fits(SIZE);
    let show_kind = show_size && fits(KIND);
    // Pozycje od prawej: Typ | Rozmiar | Wymiary (lewa krawędź każdej kolumny).
    let mut right = width;
    let mut place = |show: bool, w: f32| {
        show.then(|| {
            right -= w;
            right
        })
    };
    let kind = place(show_kind, KIND);
    let size = place(show_size, SIZE);
    let dims = place(show_dims, DIMS);
    let name_end = dims.or(size).or(kind).unwrap_or(width);
    DetailsColumns { num, thumb, name, name_w: (name_end - name - 12.0).max(40.0), dims, size, kind }
}

fn details_header(ui: &mut Ui) {
    let pal = theme::p();
    let width = ui.available_width();
    let cols = details_columns(width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
    let font = egui::FontId::proportional(12.0);
    for (x, text) in [(Some(cols.num), "#"), (Some(cols.name), "Nazwa"), (cols.dims, "Wymiary"), (cols.size, "Rozmiar"), (cols.kind, "Typ")] {
        if let Some(x) = x {
            ui.painter().text(pos2(rect.left() + x, rect.center().y), egui::Align2::LEFT_CENTER, text, font.clone(), pal.text_dim);
        }
    }
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, pal.separator));
}

/// „516 B”, „2.9 KB”, „1.23 MB”.
pub fn human_size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.2} MB", b as f64 / 1_048_576.0),
    }
}

fn extension(doc: &Document) -> String {
    std::path::Path::new(&doc.file_name).extension().map_or(String::new(), |e| e.to_string_lossy().to_uppercase())
}

/// „1600×1100 · 2.31 MB · JPG”
fn file_info(doc: &Document) -> String {
    let mut parts = Vec::new();
    if let Some([w, h]) = doc.dims {
        parts.push(format!("{w}×{h}"));
    }
    parts.push(human_size(doc.file_size));
    let ext = extension(doc);
    if !ext.is_empty() {
        parts.push(ext);
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liczebnik_zdjec() {
        let w: Vec<String> = [1, 2, 4, 5, 12, 13, 22, 25, 101, 104].iter().map(|&n| photos_count(n)).collect();
        assert_eq!(
            w,
            ["1 zdjęcie", "2 zdjęcia", "4 zdjęcia", "5 zdjęć", "12 zdjęć", "13 zdjęć", "22 zdjęcia", "25 zdjęć", "101 zdjęć", "104 zdjęcia"]
        );
    }
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
    fn kolumny_szczegolow_nie_nachodza() {
        for width in [200.0, 320.0, 450.0, 600.0, 1200.0] {
            let c = details_columns(width);
            assert!(c.num < c.thumb && c.thumb < c.name);
            let mut edge = c.name + c.name_w;
            for col in [c.dims, c.size, c.kind].into_iter().flatten() {
                assert!(col >= edge, "szerokość {width}: kolumna {col} nachodzi na {edge}");
                edge = col;
            }
            assert!(edge < width);
        }
        assert!(details_columns(1200.0).kind.is_some());
        let narrow = details_columns(320.0);
        assert!(narrow.dims.is_some() && narrow.kind.is_none(), "wąsko: wymiary zostają, typ znika");
        assert!(details_columns(250.0).dims.is_none());
        assert_eq!(human_size(516), "516 B");
        assert_eq!(human_size(3 * 1024), "3.0 KB");
    }
}
