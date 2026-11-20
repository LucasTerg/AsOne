//! Główny stan aplikacji. Układ okna (styl Photoshop / PhotoCraft): pasek tytułu z menu,
//! górny pasek eksportu, lewy pasek narzędzi, prawy inspektor i centralny dok zakładek.

mod files;
mod inspector;
mod metadata;
mod panels;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use asone_core::naming::{build_filename_ver, slugify};
use crossbeam_channel::{Receiver, Sender};
use egui::{Key, Ui};
use egui_dock::{DockArea, DockState};

use crate::ai::{AiModel, AiWorker, DownloadMsg, OnnxBackgroundRemover};
use crate::canvas::{BrushSettings, Tool, zoom_at};
use crate::document::{AiState, DocId, Document, LoadState, PxRect, ThumbState, fill_rect, union_opt};
use crate::export::ExportMsg;
use crate::loader::{LoadResult, Loader};
use crate::platform::{self, ImageSource, Instant, PickerEvent, Pickers, Tasks};
use crate::prefs::{DownloadView, PrefsDialog};
use crate::renderer::GpuShared;
use crate::settings::{STORAGE_KEY, Settings};
use crate::tabs::{AppTab, Viewer, photo_tab_order, reorder_list_by_tabs, reorder_photo_tabs};
use crate::theme;
use crate::titlebar::{self, MenuAction};

pub(crate) enum ReportEntry {
    Saved { name: String, bytes: u64, size: [u32; 2], quality: u8, notes: Vec<String> },
    Failed { name: String, error: String },
}

pub(crate) struct ExportJob {
    rx: Receiver<ExportMsg>,
    total: usize,
    entries: Vec<ReportEntry>,
    out_dir: Option<PathBuf>,
}

pub(crate) struct ExportReport {
    entries: Vec<ReportEntry>,
    elapsed: Duration,
    out_dir: Option<PathBuf>,
}

/// Wyniki operacji w tle (auto-kadr, ZIP, PDF).
pub(crate) enum OpsMsg {
    Toast(String, bool),
    AutoCrop { id: DocId, rect: Option<PxRect> },
}

/// Pobieranie modelu AI w toku.
pub(crate) struct AiDownload {
    rx: Receiver<DownloadMsg>,
    model: AiModel,
    done: u64,
    total: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AboutTab {
    #[default]
    About,
    Contributors,
    Models,
}

#[derive(Default)]
pub(crate) struct Dialogs {
    delete: Option<Vec<DocId>>,
    clear_list: bool,
    /// „Zmień tylko nazwę”: podgląd zmian do potwierdzenia.
    rename: Option<Vec<(DocId, asone_core::rename::RenamePlan)>>,
    about: bool,
    about_tab: AboutTab,
    shortcuts: bool,
}

pub struct AsOneApp {
    dock: DockState<AppTab>,
    dock_style: egui_dock::Style,
    docs: HashMap<DocId, Document>,
    /// Kolejność plików – wspólna dla listy i zakładek ze zdjęciami.
    order: Vec<DocId>,
    last_tabs: Vec<DocId>,
    selected: HashSet<DocId>,
    next_id: u64,

    settings: Settings,
    applied_scale: u32,
    applied_decorations: Option<bool>,
    prefs: PrefsDialog,

    base_name: String,
    out_dir: Option<PathBuf>,

    tool: Tool,
    brush: BrushSettings,
    inspection: bool,
    active_doc: Option<DocId>,
    copy_crop_too: bool,
    folder_input: String,

    gpu: Option<Arc<GpuShared>>,
    loader: Loader,
    ai: AiWorker,
    pickers: Pickers,
    ops: Tasks,
    ops_tx: Sender<OpsMsg>,
    ops_rx: Receiver<OpsMsg>,
    /// Pliki czekające na wybór miejsca zapisu ZIP/PDF.
    pending_save: Vec<PathBuf>,
    pending_pdf: Vec<DocId>,
    export: Option<ExportJob>,
    report: Option<ExportReport>,
    ai_download: Option<AiDownload>,
    dialogs: Dialogs,
    toast: Option<(String, bool, Instant)>,
    /// Prawy panel: „Narzędzia” albo „Metadane”.
    inspector_tab: InspectorTab,
    /// Android: zdjęcia z okna wyboru, zanim przyjdzie koniec wyboru.
    #[cfg(target_os = "android")]
    android_picked: Vec<ImageSource>,
    /// Android: ostatnio ustawiony kolor ikon paska stanu (jasny motyw = ciemne ikony).
    #[cfg(target_os = "android")]
    android_light_bars: Option<bool>,
    meta: HashMap<DocId, metadata::MetaState>,
    meta_channel: metadata::MetaChannel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum InspectorTab {
    #[default]
    Tools,
    Metadata,
}

fn polish_translations() -> egui_dock::Translations {
    let mut t = egui_dock::Translations::english();
    t.tab_context_menu.close_button = "Zamknij".into();
    t.tab_context_menu.eject_button = "Odłącz".into();
    t.tab_context_menu.hide_tab_bar_button = "Ukryj pasek zakładek".into();
    t.tab_context_menu.show_tab_bar_button = "Pokaż pasek zakładek".into();
    t.leaf.close_button_disabled_tooltip = "Tej zakładki nie można zamknąć".into();
    t.leaf.close_all_button = "Zamknij wszystkie".into();
    t.leaf.close_all_button_disabled_tooltip = "Niektórych zakładek nie można zamknąć".into();
    t.leaf.minimize_button = "Zwiń".into();
    t
}

impl AsOneApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        #[allow(unused_mut)]
        let mut settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, STORAGE_KEY)).unwrap_or_default();
        if settings.hf_token.trim().is_empty() {
            if let Ok(tok) = std::env::var("HF_TOKEN") {
                settings.hf_token = tok;
            } else if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
                let flatpak_ron = home.join(".local/share/io.github.lucasterg.asone/app.ron");
                if let Ok(content) = std::fs::read_to_string(&flatpak_ron)
                    && let Some(pos) = content.find("hf_token:\\\"")
                {
                    let rem = &content[pos + 11..];
                    if let Some(end) = rem.find("\\\"") {
                        let tok = &rem[..end];
                        if !tok.is_empty() {
                            settings.hf_token = tok.to_string();
                        }
                    }
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            crate::web::register_context(ctx);
            if let Some(theme) = crate::web::url_params().theme {
                settings.theme = theme;
            }
        }
        theme::apply(ctx, &settings.appearance());
        egui_extras::install_image_loaders(ctx);
        // Ctrl+0 / Ctrl+1 / Ctrl± obsługujemy sami (zdjęcie vs. skala interfejsu).
        ctx.options_mut(|o| o.zoom_with_keyboard = false);

        let gpu = cc.wgpu_render_state.as_ref().map(GpuShared::new);
        #[cfg(target_os = "android")]
        crate::android::set_context(ctx);
        let remover = Arc::new(OnnxBackgroundRemover::default());
        let (ops_tx, ops_rx) = crossbeam_channel::unbounded();
        let mut app = Self {
            dock: DockState::new(vec![AppTab::FileList]).with_translations(polish_translations()),
            dock_style: theme::dock_style(&theme::current_style(ctx)),
            docs: HashMap::new(),
            order: Vec::new(),
            last_tabs: Vec::new(),
            selected: HashSet::new(),
            next_id: 1,
            applied_scale: 0,
            applied_decorations: None,
            prefs: PrefsDialog::default(),
            base_name: String::new(),
            out_dir: cfg!(target_os = "android").then(|| PathBuf::from(platform::ANDROID_DEFAULT_OUTPUT)),
            tool: Tool::Move,
            brush: BrushSettings::default(),
            inspection: false,
            active_doc: None,
            copy_crop_too: false,
            folder_input: platform::ANDROID_DEFAULT_INPUT.to_string(),
            loader: Loader::new(ctx.clone(), gpu.clone()),
            ai: AiWorker::new(remover, ctx.clone()),
            pickers: Pickers::new(ctx.clone()),
            ops: Tasks::new("ops", 2),
            ops_tx,
            ops_rx,
            pending_save: Vec::new(),
            pending_pdf: Vec::new(),
            gpu,
            export: None,
            report: None,
            ai_download: None,
            dialogs: Dialogs::default(),
            toast: None,
            inspector_tab: InspectorTab::default(),
            #[cfg(target_os = "android")]
            android_picked: Vec::new(),
            #[cfg(target_os = "android")]
            android_light_bars: None,
            meta: HashMap::new(),
            meta_channel: metadata::MetaChannel::default(),
            settings,
        };
        app.apply_view_settings(ctx);
        app
    }

    /// Pliki z wiersza poleceń / „Otwórz za pomocą”: trafiają na listę i od razu do zakładek.
    pub fn with_files(mut self, paths: Vec<PathBuf>) -> Self {
        let files = asone_core::imageio::expand_paths(paths);
        if !files.is_empty() {
            let first_new = self.order.len();
            self.add_sources(files.into_iter().map(ImageSource::Path).collect(), 0);
            let new_ids = self.order[first_new..].to_vec();
            for id in new_ids {
                self.open_doc(id);
            }
        }
        self
    }

    /// Android: zdarzenia z powłoki Kotlin (zdjęcia, schowek, tekst) i kolor ikon paska stanu.
    #[cfg(target_os = "android")]
    fn poll_android(&mut self) {
        use crate::android::Event;
        for event in crate::android::take_events() {
            match event {
                Event::Picked(name, bytes) => self.android_picked.push(ImageSource::Bytes { name, bytes: bytes.into() }),
                Event::PickFinished { failed, .. } => {
                    let picked = std::mem::take(&mut self.android_picked);
                    self.pickers.inject(PickerEvent::Files(picked, failed));
                }
                Event::Clipboard(text) => self.pickers.inject(PickerEvent::Pasted(text)),
                Event::Text(text) => self.base_name = text.trim().to_string(),
            }
        }
        let light = !theme::p().is_dark();
        if self.android_light_bars != Some(light) {
            crate::android::set_light_system_bars(light);
            self.android_light_bars = Some(light);
        }
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some((text.into(), error, Instant::now()));
    }

    /// Motyw, skala interfejsu i pasek tytułu wg ustawień.
    fn apply_view_settings(&mut self, ctx: &egui::Context) {
        theme::apply(ctx, &self.settings.appearance());
        self.dock_style = theme::dock_style(&theme::current_style(ctx));
        if self.applied_scale != self.settings.ui_scale {
            ctx.set_zoom_factor(self.settings.ui_scale as f32 / 100.0);
            self.applied_scale = self.settings.ui_scale;
        }
        if cfg!(not(any(target_os = "macos", target_os = "android", target_arch = "wasm32"))) {
            let decorations = self.settings.system_title_bar;
            if self.applied_decorations != Some(decorations) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(decorations));
                self.applied_decorations = Some(decorations);
            }
        }
    }

    fn numbering(&self, open: &[DocId]) -> HashMap<DocId, (usize, Option<String>)> {
        let has_name = !slugify(&self.base_name).is_empty();
        let start = self.settings.start_number.max(1) as usize;
        open.iter()
            .enumerate()
            .map(|(i, id)| {
                let version = self.docs.get(id).and_then(|d| d.version);
                (*id, (start + i, has_name.then(|| build_filename_ver(&self.base_name, start + i, version, "jpg"))))
            })
            .collect()
    }

    /// Lista i zakładki mają jedną kolejność.
    fn sync_order(&mut self) {
        let tabs = photo_tab_order(&self.dock);
        if tabs == self.last_tabs {
            return;
        }
        let mut a = tabs.clone();
        let mut b = self.last_tabs.clone();
        a.sort();
        b.sort();
        if a == b {
            // Przeciągnięto zakładkę: lista idzie za zakładkami.
            reorder_list_by_tabs(&mut self.order, &tabs);
        } else {
            // Otwarto/zamknięto zakładkę: zakładki idą za listą.
            let open: HashSet<DocId> = tabs.iter().copied().collect();
            let desired: Vec<DocId> = self.order.iter().copied().filter(|d| open.contains(d)).collect();
            reorder_photo_tabs(&mut self.dock, &desired);
        }
        self.last_tabs = photo_tab_order(&self.dock);
    }

    fn update_active_doc(&mut self, open: &[DocId]) {
        if let Some((_, AppTab::Photo(id))) = self.dock.find_active_focused() {
            self.active_doc = Some(*id);
        }
        if self.active_doc.is_some_and(|id| !open.contains(&id)) {
            self.active_doc = open.last().copied();
        }
    }

    // --- Wyniki pracy w tle ----------------------------------------------------------------

    fn poll_background(&mut self, ctx: &egui::Context) {
        for msg in self.loader.rx.try_iter().collect::<Vec<_>>() {
            match msg {
                LoadResult::Thumbnail { id, dims, image } => {
                    if let Some(doc) = self.docs.get_mut(&id) {
                        let tex = ctx.load_texture(format!("thumb-{}", id.0), image, egui::TextureOptions::LINEAR);
                        doc.thumb = ThumbState::Ready(tex);
                        doc.dims = Some(dims);
                    }
                }
                LoadResult::Full { id, gpu, mask, has_alpha, downscaled_from, ai_proxy } => {
                    if let Some(doc) = self.docs.get_mut(&id) {
                        doc.dims.get_or_insert(downscaled_from.unwrap_or(gpu.size));
                        doc.gpu = Some(gpu);
                        doc.mask_cpu = mask;
                        doc.mask_dirty = None;
                        doc.downscaled_from = downscaled_from;
                        doc.ai_proxy = Some(ai_proxy);
                        doc.load = LoadState::Ready;
                        doc.camera.needs_fit = true;
                        if has_alpha {
                            doc.edits.use_bg_removal = true;
                            doc.mask_touched = true;
                        }
                        doc.reset_history();
                    }
                }
                LoadResult::Failed { id, thumbnail, error } => {
                    if let Some(doc) = self.docs.get_mut(&id) {
                        if thumbnail {
                            doc.thumb = ThumbState::Failed(error);
                        } else {
                            doc.load = LoadState::Failed(error);
                        }
                    }
                }
            }
        }

        for msg in self.ai.rx.try_iter().collect::<Vec<_>>() {
            self.ai.job_finished();
            match msg {
                crate::ai::AiResult::Done { id, mask } => {
                    if let Some(doc) = self.docs.get_mut(&id) {
                        if let Some(size) = doc.src_size().filter(|s| mask.len() == s[0] as usize * s[1] as usize) {
                            doc.begin_mask_change();
                            doc.mask_cpu = mask;
                            doc.mask_dirty = Some(PxRect { x0: 0, y0: 0, x1: size[0], y1: size[1] });
                            doc.edits.use_bg_removal = true;
                            doc.mask_touched = true;
                        }
                        doc.ai = AiState::Idle;
                    }
                }
                crate::ai::AiResult::Failed { id, error } => {
                    if let Some(doc) = self.docs.get_mut(&id) {
                        doc.ai = AiState::Failed(error);
                    }
                }
            }
        }

        self.ai.release_when_idle();
        self.poll_metadata();
        #[cfg(target_os = "android")]
        self.poll_android();

        for event in self.pickers.rx.try_iter().collect::<Vec<_>>() {
            match event {
                PickerEvent::Files(sources, skipped) => self.add_sources(sources, skipped),
                PickerEvent::OutputFolder(dir) => self.out_dir = Some(dir),
                PickerEvent::Pasted(text) => {
                    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if line.is_empty() {
                        self.toast("Schowek jest pusty", true);
                    } else {
                        self.base_name = line;
                    }
                }
                PickerEvent::SaveTarget(kind, path) => self.save_archive(kind, path),
                PickerEvent::ModelFile(path) => self.prefs.set_model_path(path),
            }
        }

        for msg in self.ops_rx.try_iter().collect::<Vec<_>>() {
            match msg {
                OpsMsg::Toast(text, error) => self.toast(text, error),
                OpsMsg::AutoCrop { id, rect } => match (rect, self.docs.get_mut(&id)) {
                    (Some(r), Some(doc)) => {
                        if let Some(src) = doc.src_size() {
                            doc.edits.set_crop_px(r, src);
                            let msg = format!("Kadr do obiektu: {} × {} px", r.width(), r.height());
                            self.toast(msg, false);
                        }
                    }
                    (None, Some(_)) => self.toast("Nie znaleziono obiektu na jasnym tle", true),
                    _ => {}
                },
            }
        }

        if let Some(dl) = &mut self.ai_download {
            let mut finished = None;
            for msg in dl.rx.try_iter() {
                match msg {
                    DownloadMsg::Progress { done, total } => {
                        dl.done = done;
                        dl.total = total;
                    }
                    DownloadMsg::Done(path) => finished = Some(Ok(path)),
                    DownloadMsg::Failed(e) => finished = Some(Err(e)),
                }
            }
            match finished {
                Some(Ok(path)) => {
                    self.ai_download = None;
                    self.toast(format!("Model zapisany: {}", path.display()), false);
                }
                Some(Err(e)) => {
                    self.ai_download = None;
                    self.toast(e, true);
                }
                None => {}
            }
        }

        if let Some(job) = &mut self.export {
            let mut finished = None;
            for msg in job.rx.try_iter() {
                match msg {
                    ExportMsg::Saved { file_name, bytes, size, quality, notes } => {
                        job.entries.push(ReportEntry::Saved { name: file_name, bytes, size, quality, notes })
                    }
                    ExportMsg::Failed { file_name, error } => job.entries.push(ReportEntry::Failed { name: file_name, error }),
                    ExportMsg::Finished { elapsed } => finished = Some(elapsed),
                }
            }
            if let Some(elapsed) = finished
                && let Some(job) = self.export.take()
            {
                self.report = Some(ExportReport { entries: job.entries, elapsed, out_dir: job.out_dir });
            }
        }
    }

    // --- Menu i skróty ---------------------------------------------------------------------

    fn menu_action(&mut self, ctx: &egui::Context, action: MenuAction, open: &[DocId]) {
        match action {
            MenuAction::AddFiles => self.pickers.pick_images(),
            MenuAction::AddFolder => self.pickers.pick_image_folder(),
            MenuAction::Export => self.start_export(ctx, open),
            MenuAction::RenameOnly => self.prepare_rename(open),
            MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            MenuAction::Preferences(section) => self.prefs.open(section, &self.settings),
            MenuAction::SetTheme(kind) => {
                self.settings.theme = kind;
                self.apply_view_settings(ctx);
            }
            MenuAction::UiZoomIn | MenuAction::UiZoomOut => {
                self.settings.step_ui_scale(action == MenuAction::UiZoomIn);
                self.apply_view_settings(ctx);
            }
            MenuAction::UiZoomReset => {
                self.settings.ui_scale = 100;
                self.apply_view_settings(ctx);
            }
            MenuAction::FitImage => {
                if let Some(doc) = self.active_doc.and_then(|id| self.docs.get_mut(&id)) {
                    doc.camera.needs_fit = true;
                }
            }
            MenuAction::ActualSize => {
                if let Some(doc) = self.active_doc.and_then(|id| self.docs.get_mut(&id)) {
                    let center = doc.canvas.view_center();
                    let factor = 1.0 / doc.camera.zoom;
                    zoom_at(&mut doc.camera, center, factor);
                }
            }
            MenuAction::ToggleInspection => {
                if !self.settings.hide_inspection {
                    self.inspection = !self.inspection;
                }
            }
            MenuAction::Undo | MenuAction::Redo => {
                if let Some(doc) = self.active_doc.and_then(|id| self.docs.get_mut(&id)) {
                    doc.step_history(action == MenuAction::Redo);
                }
            }
            MenuAction::ToggleTabNumberOnly => {
                use crate::settings::TabTitle;
                self.settings.tab_title =
                    if self.settings.tab_title == TabTitle::NumberOnly { TabTitle::Full } else { TabTitle::NumberOnly };
            }
            MenuAction::TogglePhotoTabsBottom => self.settings.photo_tabs_bottom = !self.settings.photo_tabs_bottom,
            MenuAction::ToggleHidden(item) => {
                let hidden = self.settings.hidden_mut(item);
                *hidden = !*hidden;
            }
            MenuAction::Shortcuts => self.dialogs.shortcuts = true,
            MenuAction::About => self.dialogs.about = true,
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context, open: &[DocId]) {
        let (ui_in, ui_out, open_files, quit, f1) = ctx.input(|i| {
            let c = i.modifiers.command;
            (
                c && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                c && i.key_pressed(Key::Minus),
                c && i.key_pressed(Key::O),
                c && i.key_pressed(Key::Q),
                i.key_pressed(Key::F1),
            )
        });
        if ui_in {
            self.menu_action(ctx, MenuAction::UiZoomIn, open);
        }
        if ui_out {
            self.menu_action(ctx, MenuAction::UiZoomOut, open);
        }
        if open_files && platform::CAN_PICK_FILES {
            self.pickers.pick_images();
        }
        if quit && cfg!(not(target_arch = "wasm32")) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if f1 {
            self.dialogs.shortcuts = true;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // Ctrl+Z – Cofnij; Ctrl+Shift+Z lub Ctrl+Y – Ponów (pole tekstowe ma własne Cofnij, stąd test wyżej).
        let (undo, redo) = ctx.input_mut(|i| {
            let redo = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z)
                || i.consume_key(egui::Modifiers::COMMAND, Key::Y);
            let undo = !redo && i.consume_key(egui::Modifiers::COMMAND, Key::Z);
            (undo, redo)
        });
        if undo || redo {
            self.menu_action(ctx, if redo { MenuAction::Redo } else { MenuAction::Undo }, open);
        }
        let (tool_key, enter, brush_down, brush_up, delete, shift, escape, fit, actual, save, inspect) = ctx.input(|i| {
            let plain = !i.modifiers.any();
            let tool_key = Tool::ALL.into_iter().find(|t| plain && i.key_pressed(t.key()));
            (
                tool_key,
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::OpenBracket),
                i.key_pressed(Key::CloseBracket),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.modifiers.shift,
                i.key_pressed(Key::Escape),
                i.modifiers.command && i.key_pressed(Key::Num0),
                i.modifiers.command && i.key_pressed(Key::Num1),
                i.modifiers.command && i.key_pressed(Key::S),
                plain && i.key_pressed(Key::I),
            )
        });
        if let Some(t) = tool_key {
            self.tool = t;
        }
        if inspect && !self.settings.hide_inspection {
            self.inspection = !self.inspection;
        }
        if enter && self.tool == Tool::Crop {
            self.tool = Tool::Move;
        }
        if brush_down {
            self.brush.radius = (self.brush.radius / 1.25).max(1.0);
        }
        if brush_up {
            self.brush.radius = (self.brush.radius * 1.25).min(1000.0);
        }
        if save {
            self.start_export(ctx, open);
        }
        if fit {
            self.menu_action(ctx, MenuAction::FitImage, open);
        }
        if actual {
            self.menu_action(ctx, MenuAction::ActualSize, open);
        }
        let Some(doc) = self.active_doc.and_then(|id| self.docs.get_mut(&id)) else { return };
        if delete && let (Some(sel), Some(size)) = (doc.selection, doc.src_size()) {
            doc.begin_mask_change();
            let dirty = fill_rect(&mut doc.mask_cpu, size, sel, if shift { 255 } else { 0 });
            doc.mask_dirty = union_opt(doc.mask_dirty, dirty);
            doc.edits.use_bg_removal = true;
            doc.mask_touched = true;
        }
        if escape {
            doc.selection = None;
        }
    }

    /// Przeglądarka: pliki pobrane przez stronę (`?file=`, `?url=`, `?sample`) – na listę i do zakładek.
    #[cfg(target_arch = "wasm32")]
    fn poll_web_files(&mut self) {
        for (text, error) in crate::web::take_notices() {
            self.toast(text, error);
        }
        let files = crate::web::take_incoming();
        if files.is_empty() {
            return;
        }
        let first_new = self.order.len();
        let sources = files
            .into_iter()
            .map(|(name, bytes)| ImageSource::Bytes { name, bytes: bytes.into() })
            .collect();
        self.add_sources(sources, 0);
        for id in self.order[first_new..].to_vec() {
            self.open_doc(id);
        }
    }

    fn start_model_download(&mut self, ctx: &egui::Context, model: AiModel, path: PathBuf, token: String) {
        if self.ai_download.is_some() {
            return;
        }
        #[cfg(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let rx = crate::ai::download_model(model, path, token, ctx.clone());
            self.ai_download = Some(AiDownload { rx, model, done: 0, total: None });
        }
        #[cfg(not(all(feature = "ai", any(target_os = "linux", target_os = "windows", target_os = "macos"))))]
        {
            let _ = (ctx, model, path, token);
            self.toast("Pobieranie modeli AI działa w wersji desktopowej", true);
        }
    }

    fn window_title(&self) -> String {
        match self.active_doc.and_then(|id| self.docs.get(&id)) {
            Some(doc) => format!("AsOne — {}", doc.file_name),
            None => "AsOne — zdjęcia produktowe do internetu".into(),
        }
    }
}

impl AsOneApp {
    /// Cały interfejs (pasek tytułu, panele, dok, okna).
    fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);
        self.sync_order();

        let open = photo_tab_order(&self.dock);
        let numbering = self.numbering(&open);
        self.handle_shortcuts(&ctx, &open);

        let title = self.window_title();
        let history = self.active_doc.and_then(|id| self.docs.get(&id)).map(|d| &d.history);
        let state = titlebar::MenuState {
            inspection: self.inspection,
            can_undo: history.is_some_and(|h| h.can_undo()),
            can_redo: history.is_some_and(|h| h.can_redo()),
        };
        for action in titlebar::title_bar(ui, &title, &self.settings, state) {
            self.menu_action(&ctx, action, &open);
        }
        self.top_bar(ui, &open);
        self.tool_bar(ui);
        self.inspector(ui, &open, &numbering);

        let mut actions = Vec::new();
        let mut list = self.settings.list;
        {
            let style = self.dock_style.clone();
            let dock = &mut self.dock;
            let mut viewer = Viewer {
                docs: &mut self.docs,
                order: &self.order,
                numbering: &numbering,
                selected: &mut self.selected,
                settings: &self.settings,
                list: &mut list,
                gpu: self.gpu.as_ref(),
                tool: self.tool,
                brush: &self.brush,
                inspection: self.inspection,
                actions: &mut actions,
                folder_input: &mut self.folder_input,
            };
            egui::CentralPanel::no_frame().show(ui, |ui| {
                DockArea::new(dock)
                    .style(style)
                    .show_close_buttons(true)
                    .show_add_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .draggable_tabs(true)
                    .show_inside(ui, &mut viewer);
            });
        }
        self.settings.list = list;
        self.apply_actions(actions);
        self.sync_order();

        let open = photo_tab_order(&self.dock);
        self.update_active_doc(&open);
        // Historia: zmiana staje się krokiem Cofnij, gdy nic nie jest przeciągane ani wpisywane.
        if !ctx.input(|i| i.pointer.any_down()) && !ctx.egui_wants_keyboard_input() {
            for doc in self.docs.values_mut() {
                doc.commit_history();
            }
        }
        self.report_window(&ctx);
        self.dialogs_ui(&ctx);
        let view = self.ai_download.as_ref().map(|d| DownloadView { model: d.model, done: d.done, total: d.total });
        let prefs = self.prefs.show(&ctx, view.as_ref());
        if let Some(new) = prefs.apply {
            self.settings = new;
            self.apply_view_settings(&ctx);
        }
        if prefs.pick_model {
            let start = self.settings.ai_model_path().parent().map(|p| p.to_path_buf());
            self.pickers.pick_model_file(start);
        }
        if let Some((model, path, token)) = prefs.download {
            self.start_model_download(&ctx, model, path, token);
        }
        if self.settings.hide_inspection {
            self.inspection = false;
        }
        self.overlays(&ctx);
        titlebar::resize_edges(&ctx, &self.settings);

        ctx.output_mut(|o| {
            for cmd in &o.commands {
                if let egui::OutputCommand::OpenUrl(open_url) = cmd {
                    crate::platform::open_url(&open_url.url);
                }
            }
        });
    }
}

impl eframe::App for AsOneApp {
    /// Android: odsuwa cały interfejs od zegara, paska gestów i wycięcia aparatu
    /// (`safe_area_insets` – egui wtedy rysuje tylko w bezpiecznym obszarze).
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        #[cfg(target_os = "android")]
        {
            // Marginesy liczy powłoka Kotlin (WindowInsets) i przysyła przez JNI.
            let ppp = ctx.pixels_per_point().max(0.1);
            let [left, top, right, bottom] = crate::android::insets_px().map(|v| v.max(0) as f32 / ppp);
            raw_input.safe_area_insets = Some(egui::SafeAreaInsets(egui::epaint::MarginF32 { left, right, top, bottom }));
        }
        let _ = (ctx, raw_input);
    }

    /// Android: tło pod paskiem stanu i paskiem gestów w kolorze belki tytułu.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if cfg!(target_os = "android") {
            theme::p().chrome.to_normalized_gamma_f32()
        } else {
            // Domyślny kolor eframe.
            egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32()
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // W przeglądarce zadania „w tle” wykonujemy między klatkami.
        if platform::run_pending_tasks(12.0) {
            ctx.request_repaint();
        }
        self.poll_background(ctx);
        #[cfg(target_arch = "wasm32")]
        self.poll_web_files();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.settings);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // egui 0.36 daje główny Ui na cały ekran (viewport_rect), mimo `safe_area_insets` –
        // na Androidzie rysujemy więc w obszarze bez pasków systemowych i wycięcia aparatu.
        if cfg!(target_os = "android") {
            let safe = ui.ctx().content_rect();
            ui.scope_builder(egui::UiBuilder::new().max_rect(safe), |ui| self.draw(ui));
            // Tekst wpisuje się w oknie systemowym (Kotlin) – egui nie może włączać klawiatury
            // GameActivity: ta przejmuje fokus SurfaceView i gryzie się z oknem edycji.
            ui.ctx().output_mut(|o| o.ime = None);
        } else {
            self.draw(ui);
        }
    }
}
