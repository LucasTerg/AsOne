//! Pliki: dodawanie, zakładki, kolejność, operacje z Asystenta PIM (ZIP, PDF, kosz, GIMP) i eksport.

use std::collections::HashSet;
use std::path::PathBuf;

use asone_core::archive::{PdfPage, images_to_pdf, zip_files};
use asone_core::autocrop::{DEFAULT_THRESHOLD, content_bbox};
use asone_core::imageio::fit_within;
use asone_core::naming::{build_filename_ver, slugify};
use egui_dock::{Node, NodeIndex, NodePath, Split, SurfaceIndex};

use super::{AsOneApp, ExportJob, OpsMsg};
use crate::document::{DocId, Document, LoadState, ThumbState};
use crate::export::{ExportItem, ExportSettings};
use crate::platform::{self, ImageSource, SaveKind};
use crate::tabs::{Action, AppTab, move_in_order, photo_tab_order, reorder_photo_tabs};

impl AsOneApp {
    pub(super) fn add_sources(&mut self, sources: Vec<ImageSource>, skipped: usize) {
        let mut known: HashSet<String> = self.docs.values().map(|d| d.source.key()).collect();
        let (mut added, mut dupes) = (0, 0);
        for source in sources {
            if !known.insert(source.key()) {
                dupes += 1;
                continue;
            }
            let id = DocId(self.next_id);
            self.next_id += 1;
            self.loader.request_thumbnail(id, source.clone());
            self.docs.insert(id, Document::new(id, source));
            self.order.push(id);
            added += 1;
        }
        let mut msg = format!("Dodano zdjęć: {added}");
        if dupes > 0 {
            msg.push_str(&format!(" · pominięto duplikaty: {dupes}"));
        }
        if skipped > 0 {
            msg.push_str(&format!(" · nieobsługiwane pliki: {skipped}"));
        }
        self.toast(msg, added == 0);
    }

    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        // Przeglądarka: zawartość plików czyta się asynchronicznie, wynik przyjdzie kanałem.
        #[cfg(target_arch = "wasm32")]
        self.pickers.read_dropped(dropped);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let paths: Vec<PathBuf> = dropped.iter().map(|f| f.path().to_path_buf()).collect();
            let dropped_files = paths.iter().filter(|p| !p.is_dir()).count();
            let expanded = asone_core::imageio::expand_paths(paths.clone());
            let from_files = expanded.iter().filter(|p| paths.contains(p)).count();
            self.add_sources(
                expanded.into_iter().map(ImageSource::Path).collect(),
                dropped_files.saturating_sub(from_files),
            );
        }
    }

    fn load_folder(&mut self) {
        let dir = PathBuf::from(self.folder_input.trim());
        let files = asone_core::imageio::expand_paths([dir.clone()]);
        if files.is_empty() {
            self.toast(format!("Brak zdjęć w {} (lub brak uprawnień)", dir.display()), true);
        } else {
            self.add_sources(files.into_iter().map(ImageSource::Path).collect(), 0);
        }
    }

    // --- Zakładki --------------------------------------------------------------------------

    pub(super) fn open_doc(&mut self, id: DocId) {
        let tab = AppTab::Photo(id);
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            self.dock.set_focused_node_and_surface(NodePath { surface: path.surface, node: path.node });
        } else {
            let order = photo_tab_order(&self.dock);
            let last_photo = order.last().and_then(|l| self.dock.find_tab(&AppTab::Photo(*l)));
            if let Some(last) = last_photo {
                let node = NodePath { surface: last.surface, node: last.node };
                if let Ok(leaf) = self.dock.leaf_mut(node) {
                    leaf.append_tab(tab);
                }
                self.dock.set_focused_node_and_surface(node);
            } else {
                // Pierwsze zdjęcie: lista plików zostaje po lewej, zdjęcia obok.
                let list = self
                    .dock
                    .find_tab(&AppTab::FileList)
                    .map(|p| NodePath { surface: p.surface, node: p.node })
                    .unwrap_or(NodePath { surface: SurfaceIndex::main(), node: NodeIndex::root() });
                let [_, new] = self.dock.split(list, Split::Right, 0.28, Node::leaf(tab));
                self.dock.set_focused_node_and_surface(NodePath { surface: list.surface, node: new });
            }
            // Zakładka trafia na miejsce wynikające z kolejności listy.
            self.sync_order();
            if let Some(path) = self.dock.find_tab(&tab) {
                let _ = self.dock.set_active_tab(path);
            }
        }
        if let Some(doc) = self.docs.get_mut(&id)
            && matches!(doc.load, LoadState::NotLoaded | LoadState::Failed(_))
        {
            doc.load = LoadState::Loading;
            self.loader.request_full(id, doc.source.clone());
        }
        self.active_doc = Some(id);
    }

    fn close_tab(&mut self, id: DocId) {
        if let Some(path) = self.dock.find_tab(&AppTab::Photo(id)) {
            self.dock.remove_tab(path);
        }
    }

    pub(super) fn clear_list(&mut self) {
        let all = self.order.clone();
        let n = all.len();
        self.remove_docs(&all);
        self.toast(format!("Wyczyszczono listę ({n} zdjęć) – pliki na dysku zostały"), false);
    }

    fn remove_docs(&mut self, ids: &[DocId]) {
        for id in ids {
            self.close_tab(*id);
            self.docs.remove(id);
            self.forget_metadata(*id);
            self.selected.remove(id);
            if self.active_doc == Some(*id) {
                self.active_doc = None;
            }
        }
        self.order.retain(|d| !ids.contains(d));
    }

    fn reload(&mut self, ids: &[DocId]) {
        for id in ids {
            self.forget_metadata(*id);
            let Some(doc) = self.docs.get_mut(id) else { continue };
            doc.thumb = ThumbState::Loading;
            doc.file_size = doc.source.size_hint();
            self.loader.request_thumbnail(*id, doc.source.clone());
            if !matches!(doc.load, LoadState::NotLoaded) {
                doc.load = LoadState::Loading;
                self.loader.request_full(*id, doc.source.clone());
            }
        }
    }

    pub(super) fn apply_actions(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::Open(id) => self.open_doc(id),
                Action::CloseTab(id) => self.close_tab(id),
                Action::Remove(ids) => self.remove_docs(&ids),
                Action::OpenAll => {
                    for id in self.order.clone() {
                        self.open_doc(id);
                    }
                }
                Action::AddFiles => self.pickers.pick_images(),
                Action::LoadFolder => self.load_folder(),
                Action::SwitchTool(t) => self.tool = t,
                Action::Move { id, target, before } => {
                    move_in_order(&mut self.order, id, target, before);
                    let open: HashSet<DocId> = photo_tab_order(&self.dock).into_iter().collect();
                    let desired: Vec<DocId> = self.order.iter().copied().filter(|d| open.contains(d)).collect();
                    reorder_photo_tabs(&mut self.dock, &desired);
                    self.last_tabs = photo_tab_order(&self.dock);
                }
                Action::OpenFile(id) => {
                    if let Some(p) = self.docs.get(&id).and_then(|d| d.source.path()) {
                        platform::open_path(p);
                    }
                }
                Action::OpenFolder(id) => {
                    if let Some(dir) = self.docs.get(&id).and_then(|d| d.source.path()?.parent()) {
                        platform::open_in_file_manager(dir);
                    }
                }
                Action::EditInGimp(id) => {
                    if let Some(p) = self.docs.get(&id).and_then(|d| d.source.path()).map(|p| p.to_path_buf()) {
                        match platform::open_in_gimp(&p) {
                            Ok(()) => self.toast("Otwarto w GIMP – po zapisaniu użyj „Wczytaj ponownie z dysku”", false),
                            Err(e) => self.toast(e, true),
                        }
                    }
                }
                Action::Reload(ids) => self.reload(&ids),
                Action::ZipSelected(ids) => {
                    self.pending_save = self.paths_of(&ids);
                    let name = self.archive_name("zip");
                    let start = self.pending_save.first().and_then(|p| p.parent()).map(|p| p.to_path_buf());
                    self.pickers.pick_save_target(SaveKind::ZipSelected, name, start);
                }
                Action::PdfSelected(ids) => {
                    self.pending_pdf = ids;
                    let name = self.archive_name("pdf");
                    let start = self.pending_pdf.first().and_then(|id| self.docs.get(id)?.source.path()?.parent().map(|p| p.to_path_buf()));
                    self.pickers.pick_save_target(SaveKind::PdfSelected, name, start);
                }
                Action::DeleteFromDisk(ids) => self.dialogs.delete = Some(ids),
                Action::CloseAllTabs => {
                    for id in photo_tab_order(&self.dock) {
                        self.close_tab(id);
                    }
                    self.selected.clear();
                }
                Action::ClearList => self.dialogs.clear_list = true,
                Action::SetVersion(ids, version) => {
                    for id in ids {
                        if let Some(doc) = self.docs.get_mut(&id) {
                            doc.version = version;
                        }
                    }
                }
            }
        }
    }

    fn paths_of(&self, ids: &[DocId]) -> Vec<PathBuf> {
        ids.iter().filter_map(|id| self.docs.get(id)?.source.path().map(|p| p.to_path_buf())).collect()
    }

    fn archive_name(&self, ext: &str) -> String {
        let slug = slugify(&self.base_name);
        format!("{}.{ext}", if slug.is_empty() { "zdjecia".to_string() } else { slug })
    }

    // --- ZIP / PDF / kosz ------------------------------------------------------------------

    /// Wybrano miejsce zapisu ZIP/PDF: praca idzie w tle.
    pub(super) fn save_archive(&mut self, kind: SaveKind, target: PathBuf) {
        let tx = self.ops_tx.clone();
        match kind {
            SaveKind::ZipSelected | SaveKind::ZipExport => {
                let files = std::mem::take(&mut self.pending_save);
                self.ops.spawn(move || {
                    let msg = match zip_files(&files, &target) {
                        Ok(n) => OpsMsg::Toast(format!("ZIP: {n} plików → {}", target.display()), false),
                        Err(e) => OpsMsg::Toast(format!("ZIP: {e}"), true),
                    };
                    let _ = tx.send(msg);
                });
            }
            SaveKind::PdfSelected => {
                let max_dim = self.gpu.as_ref().map_or(8192, |g| g.max_texture_dim);
                let jobs: Vec<_> = std::mem::take(&mut self.pending_pdf)
                    .iter()
                    .filter_map(|id| {
                        let d = self.docs.get(id)?;
                        let mask = (d.edits.use_bg_removal && !d.mask_cpu.is_empty()).then(|| d.mask_cpu.clone());
                        Some((d.source.clone(), d.edits.clone(), mask))
                    })
                    .collect();
                self.ops.spawn(move || {
                    let pages: Result<Vec<PdfPage>, String> = jobs
                        .into_iter()
                        .map(|(source, edits, mask)| {
                            let (img, _) = crate::loader::decode_for_gpu(&source, max_dim)?;
                            let (size, rgb) = asone_core::compose::compose_rgb(&img, mask.as_deref(), &edits);
                            let (size, rgb) = asone_core::imageio::resize_rgb(size, rgb, fit_within(size, 2000))?;
                            let jpeg = asone_core::jpeg::encode_jpeg_baseline(&rgb, size, 85)?;
                            Ok(PdfPage { size, jpeg })
                        })
                        .collect();
                    let msg = match pages.and_then(|p| {
                        let n = p.len();
                        std::fs::write(&target, images_to_pdf(&p)).map(|()| n).map_err(|e| e.to_string())
                    }) {
                        Ok(n) => OpsMsg::Toast(format!("PDF: {n} stron → {}", target.display()), false),
                        Err(e) => OpsMsg::Toast(format!("PDF: {e}"), true),
                    };
                    let _ = tx.send(msg);
                });
            }
        }
    }

    /// „Usuń z dysku”: `trash` – do kosza, inaczej trwale.
    pub(super) fn delete_from_disk(&mut self, ids: &[DocId], trash: bool) {
        let paths = self.paths_of(ids);
        let result = if trash {
            platform::move_to_trash(&paths)
        } else {
            paths.iter().try_for_each(|p| std::fs::remove_file(p).map_err(|e| format!("{}: {e}", p.display())))
        };
        match result {
            Ok(()) => {
                self.remove_docs(ids);
                let how = if trash { "przeniesiono do kosza" } else { "usunięto trwale" };
                self.toast(format!("{} plików {how}", paths.len()), false);
            }
            Err(e) => self.toast(e, true),
        }
    }

    pub(super) fn auto_crop(&mut self, id: DocId) {
        let Some(doc) = self.docs.get(&id) else { return };
        let max_dim = self.gpu.as_ref().map_or(8192, |g| g.max_texture_dim);
        let source = doc.source.clone();
        let mask = (doc.edits.use_bg_removal && doc.mask_cpu.iter().any(|&v| v < 255)).then(|| doc.mask_cpu.clone());
        let tx = self.ops_tx.clone();
        self.ops.spawn(move || {
            let rect = crate::loader::decode_for_gpu(&source, max_dim)
                .ok()
                .and_then(|(img, _)| content_bbox(&img, mask.as_deref(), DEFAULT_THRESHOLD));
            let _ = tx.send(OpsMsg::AutoCrop { id, rect });
        });
    }

    // --- Eksport ---------------------------------------------------------------------------

    pub(super) fn default_out_dir(&self, open: &[DocId]) -> Option<PathBuf> {
        if let Some(dir) = &self.out_dir {
            return Some(dir.clone());
        }
        let sub = if self.settings.out_subfolder.trim().is_empty() { "web" } else { self.settings.out_subfolder.trim() };
        open.iter()
            .chain(self.order.iter())
            .find_map(|id| self.docs.get(id)?.source.path()?.parent().map(|p| p.join(sub)))
    }

    pub(super) fn export_blocker(&self, open: &[DocId]) -> Option<&'static str> {
        if self.export.is_some() {
            Some("Trwa eksport")
        } else if open.is_empty() {
            Some("Otwórz zdjęcia w zakładkach (kliknij miniaturę)")
        } else if slugify(&self.base_name).is_empty() {
            Some("Wklej lub wpisz nazwę")
        } else if self.gpu.is_none() {
            Some("Brak GPU (wgpu)")
        } else if open.iter().any(|id| self.docs.get(id).is_none_or(|d| !d.is_ready())) {
            Some("Poczekaj, aż wszystkie otwarte zdjęcia się wczytają")
        } else if platform::EXPORT_TO_FOLDER && self.default_out_dir(open).is_none() {
            Some("Wybierz folder docelowy")
        } else {
            None
        }
    }

    /// Dlaczego „Zmień tylko nazwę” jest nieaktywne (`None` = można).
    pub(super) fn rename_blocker(&self, open: &[DocId]) -> Option<&'static str> {
        if !platform::FILE_OPS {
            Some("Zmiana nazw plików działa w wersji desktopowej")
        } else if self.export.is_some() {
            Some("Trwa eksport")
        } else if open.is_empty() {
            Some("Otwórz zdjęcia w zakładkach (kliknij miniaturę)")
        } else if slugify(&self.base_name).is_empty() {
            Some("Wklej lub wpisz nazwę")
        } else if open.iter().any(|id| self.docs.get(id).is_none_or(|d| d.source.path().is_none())) {
            Some("Zdjęcia muszą być plikami na dysku")
        } else {
            None
        }
    }

    /// „Zmień tylko nazwę”: przygotowuje podgląd (okno z potwierdzeniem).
    pub(super) fn prepare_rename(&mut self, open: &[DocId]) {
        if self.rename_blocker(open).is_some() {
            return;
        }
        let start = self.settings.start_number.max(1) as usize;
        let files: Vec<(PathBuf, Option<u8>)> = open
            .iter()
            .filter_map(|id| self.docs.get(id).and_then(|d| Some((d.source.path()?.to_path_buf(), d.version))))
            .collect();
        let plans = asone_core::rename::plan_rename(&self.base_name, start, &files);
        self.dialogs.rename = Some(open.iter().copied().zip(plans).collect());
    }

    /// Wykonuje zatwierdzoną zmianę nazw i podmienia ścieżki w dokumentach.
    pub(super) fn apply_rename(&mut self, plans: Vec<(DocId, asone_core::rename::RenamePlan)>) {
        let only: Vec<_> = plans.iter().map(|(_, p)| p.clone()).collect();
        let results = asone_core::rename::rename_files(&only);
        let (mut changed, mut errors) = (0, Vec::new());
        for ((id, plan), result) in plans.into_iter().zip(results) {
            match result {
                Ok(path) => {
                    if !plan.is_noop() {
                        changed += 1;
                    }
                    self.forget_metadata(id);
                    if let Some(doc) = self.docs.get_mut(&id) {
                        doc.source = ImageSource::Path(path);
                        doc.file_name = doc.source.name();
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        match errors.as_slice() {
            [] => self.toast(format!("Zmieniono nazwy plików: {changed}"), false),
            [first, ..] => self.toast(format!("Zmieniono: {changed} · błędy: {} – {first}", errors.len()), true),
        }
    }

    pub(super) fn start_export(&mut self, ctx: &egui::Context, open: &[DocId]) {
        if self.export_blocker(open).is_some() {
            return;
        }
        let Some(gpu) = self.gpu.clone() else { return };
        let start = self.settings.start_number.max(1) as usize;
        let items: Vec<ExportItem> = open
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let doc = self.docs.get(id)?;
                let g = doc.gpu.as_ref()?;
                #[cfg(target_arch = "wasm32")]
                let _ = g;
                Some(ExportItem {
                    file_name: build_filename_ver(&self.base_name, start + i, doc.version, "jpg"),
                    edits: doc.edits.clone(),
                    #[cfg(not(target_arch = "wasm32"))]
                    gpu: crate::renderer::OffscreenSource {
                        source_view: g.source_view.clone(),
                        mask_view: g.mask_view.clone(),
                        size: g.size,
                        mip_count: g.mip_count,
                    },
                    #[cfg(target_arch = "wasm32")]
                    source: doc.source.clone(),
                    #[cfg(target_arch = "wasm32")]
                    mask: doc.mask_cpu.clone(),
                    #[cfg(target_arch = "wasm32")]
                    max_texture_dim: gpu.max_texture_dim,
                })
            })
            .collect();
        let out_dir = self.default_out_dir(open);
        let settings = ExportSettings {
            quality: self.settings.quality,
            max_long_side: self.settings.limit_long_side.then_some(self.settings.max_long_side),
            out_dir: out_dir.clone().unwrap_or_default(),
            limits: self.settings.limits(),
        };
        let total = items.len();
        #[cfg(not(target_arch = "wasm32"))]
        let rx = {
            let sources = self.docs.values().filter_map(|d| d.source.path().map(|p| p.to_path_buf())).collect();
            crate::export::start_export(gpu, items, settings, sources, ctx.clone())
        };
        #[cfg(target_arch = "wasm32")]
        let rx = {
            let _ = gpu;
            crate::export::start_export(items, settings, ctx.clone())
        };
        self.export = Some(ExportJob { rx, total, entries: Vec::new(), out_dir });
    }

    /// „Spakuj do ZIP” z okna raportu: pliki właśnie zapisane.
    pub(super) fn zip_last_export(&mut self) {
        let Some(report) = &self.report else { return };
        let Some(dir) = report.out_dir.clone() else { return };
        self.pending_save = report
            .entries
            .iter()
            .filter_map(|e| match e {
                super::ReportEntry::Saved { name, .. } => Some(dir.join(name)),
                super::ReportEntry::Failed { .. } => None,
            })
            .collect();
        let name = self.archive_name("zip");
        self.pickers.pick_save_target(SaveKind::ZipExport, name, Some(dir));
    }
}
