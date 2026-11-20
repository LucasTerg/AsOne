//! Prawy panel › „Metadane”: skąd jest plik, prawa autorskie, AI, GPS, dane osobowe,
//! a pod spodem pełne EXIF / XMP / IPTC / teksty PNG. Czytane w tle, raz na plik.

use std::sync::Arc;

use asone_core::metadata::{MetaReport, NoticeKind, read_metadata};
use crossbeam_channel::{Receiver, Sender};
use egui::{RichText, Ui};

use super::AsOneApp;
use crate::document::DocId;
use crate::platform::{self, ImageSource};
use crate::theme;

/// Metadane jednego pliku do pokazania.
pub struct MetaView {
    pub report: MetaReport,
    /// Skąd pobrano plik (atrybuty pliku zapisane przez przeglądarkę).
    pub origin: Vec<(String, String)>,
    pub path: Option<String>,
    pub bytes: u64,
}

pub enum MetaState {
    Loading,
    Ready(Arc<MetaView>),
    Failed(String),
}

/// Kanał wyników czytania metadanych.
pub struct MetaChannel {
    pub tx: Sender<(DocId, Result<MetaView, String>)>,
    pub rx: Receiver<(DocId, Result<MetaView, String>)>,
}

impl Default for MetaChannel {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

fn read_view(source: &ImageSource) -> Result<MetaView, String> {
    let (bytes, path, origin) = match source {
        ImageSource::Path(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("Nie można odczytać pliku: {e}"))?;
            (bytes, Some(p.display().to_string()), platform::download_origin(p))
        }
        ImageSource::Bytes { bytes, .. } => (bytes.to_vec(), None, Vec::new()),
        ImageSource::Pixels { .. } => (Vec::new(), None, Vec::new()),
    };
    Ok(MetaView { report: read_metadata(&bytes), origin, path, bytes: bytes.len() as u64 })
}

impl AsOneApp {
    /// Odbiera wyniki z wątku czytającego metadane.
    pub(super) fn poll_metadata(&mut self) {
        for (id, result) in self.meta_channel.rx.try_iter().collect::<Vec<_>>() {
            let state = match result {
                Ok(view) => MetaState::Ready(Arc::new(view)),
                Err(e) => MetaState::Failed(e),
            };
            self.meta.insert(id, state);
        }
    }

    /// Unieważnia metadane (po zmianie nazwy, przeładowaniu, usunięciu).
    pub(super) fn forget_metadata(&mut self, id: DocId) {
        self.meta.remove(&id);
    }

    fn request_metadata(&mut self, id: DocId, ctx: &egui::Context) {
        let Some(doc) = self.docs.get(&id) else { return };
        self.meta.insert(id, MetaState::Loading);
        let (source, tx, ctx) = (doc.source.clone(), self.meta_channel.tx.clone(), ctx.clone());
        platform::spawn_detached("metadata", move || {
            let _ = tx.send((id, read_view(&source)));
            ctx.request_repaint();
        });
    }

    pub(super) fn metadata_panel(&mut self, ui: &mut Ui) {
        let pal = theme::p();
        let Some(id) = self.active_doc.filter(|id| self.docs.contains_key(id)) else {
            ui.colored_label(pal.text_dim, "Kliknij zdjęcie na liście plików, aby zobaczyć jego metadane.");
            return;
        };
        if !self.meta.contains_key(&id) {
            let ctx = ui.ctx().clone();
            self.request_metadata(id, &ctx);
        }
        let file_name = self.docs.get(&id).map(|d| d.file_name.clone()).unwrap_or_default();
        ui.label(RichText::new(&file_name).strong().size(14.0));
        let view = match self.meta.get(&id) {
            Some(MetaState::Ready(v)) => v.clone(),
            Some(MetaState::Failed(e)) => {
                ui.colored_label(pal.danger, e);
                return;
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Czytanie metadanych…");
                });
                return;
            }
        };

        let dims = self.docs.get(&id).and_then(|d| d.dims);
        let mut line = format!("{} · {}", view.report.format, crate::tabs::human_size(view.bytes));
        if let Some([w, h]) = dims {
            line.push_str(&format!(" · {w}×{h} px"));
        }
        ui.colored_label(pal.text_dim, line);
        if let Some(path) = &view.path {
            ui.add(egui::Label::new(RichText::new(path).small().color(pal.text_dim)).wrap());
        }
        for (label, value) in &view.origin {
            ui.add_space(2.0);
            ui.label(RichText::new(label).small().color(pal.text_dim));
            if value.starts_with("http") {
                let link = egui::Label::new(RichText::new(value).color(pal.accent).underline()).wrap().sense(egui::Sense::click());
                if ui.add(link).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Otwórz w przeglądarce").clicked() {
                    platform::open_url(value);
                }
            } else {
                ui.add(egui::Label::new(value).wrap());
            }
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Podsumowanie").strong());
        ui.add_space(2.0);
        for n in &view.report.notices {
            let (tag, color) = match n.kind {
                NoticeKind::Rights => ("Prawa", pal.warning),
                NoticeKind::Ai => ("AI", pal.warning),
                NoticeKind::Location => ("GPS", pal.warning),
                NoticeKind::Personal => ("Osoba", pal.text),
                NoticeKind::Info => ("Info", pal.text_dim),
            };
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(tag).small().strong().color(color));
                ui.add(egui::Label::new(RichText::new(&n.text).color(color)).wrap());
            });
        }

        for group in &view.report.groups {
            ui.add_space(4.0);
            egui::CollapsingHeader::new(format!("{} ({})", group.title, group.entries.len()))
                .id_salt(("meta", &group.title))
                .default_open(group.entries.len() <= 12)
                .show(ui, |ui| {
                    for (key, value) in &group.entries {
                        ui.label(RichText::new(key).small().color(pal.text_dim));
                        ui.add(egui::Label::new(value).wrap());
                        ui.add_space(2.0);
                    }
                });
        }

        ui.add_space(10.0);
        if ui.button("Kopiuj metadane").on_hover_text("Do schowka, jako tekst").clicked() {
            ui.ctx().copy_text(report_text(&file_name, &view));
        }
        ui.add_space(6.0);
        ui.add(
            egui::Label::new(
                RichText::new(
                    "„Zapisz z zakładek” usuwa wszystkie metadane (EXIF, XMP, IPTC, ICC, C2PA) – w zapisanych JPG ich nie ma. \
                     „Zmień tylko nazwę” nie zmienia zawartości pliku, więc metadane zostają.",
                )
                .small()
                .color(pal.text_dim),
            )
            .wrap(),
        );
    }
}

fn report_text(file_name: &str, v: &MetaView) -> String {
    let mut s = format!("{file_name} ({}, {} B)\n", v.report.format, v.bytes);
    if let Some(p) = &v.path {
        s.push_str(&format!("{p}\n"));
    }
    for (k, val) in &v.origin {
        s.push_str(&format!("{k}: {val}\n"));
    }
    for n in &v.report.notices {
        s.push_str(&format!("[{:?}] {}\n", n.kind, n.text));
    }
    for g in &v.report.groups {
        s.push_str(&format!("\n== {}\n", g.title));
        for (k, val) in &g.entries {
            s.push_str(&format!("{k}: {val}\n"));
        }
    }
    s
}
