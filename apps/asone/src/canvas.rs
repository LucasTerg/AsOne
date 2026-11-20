//! Płótno zakładki ze zdjęciem: kamera 2D (zoom/pan), narzędzia i nakładki.

use std::sync::Arc;

use eframe::egui_wgpu;
use egui::{Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Vec2, pos2, vec2};

use crate::document::{Camera, Document, LoadState, OVERLAY_PREVIEW, PxRect, stamp_segment, union_opt};
use crate::renderer::{CanvasCallback, GpuShared, Layout, RenderMode, build_uniforms, upload_mask_region};
use crate::theme;

pub const MIN_ZOOM: f32 = 0.005;
pub const MAX_ZOOM: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Move,
    Crop,
    Eraser,
    RectCut,
}

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::Move, Tool::Crop, Tool::Eraser, Tool::RectCut];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Move => "Przesuwanie",
            Tool::Crop => "Kadrowanie",
            Tool::Eraser => "Gumka / Przywróć maskę",
            Tool::RectCut => "Wycinanie obszaru prostokątem",
        }
    }

    pub fn key(self) -> Key {
        match self {
            Tool::Move => Key::H,
            Tool::Crop => Key::C,
            Tool::Eraser => Key::E,
            Tool::RectCut => Key::M,
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Tool::Move => "LPM / Spacja / środkowy – przesuwanie · kółko – zoom · PPM – menu · Ctrl+0 dopasuj · Ctrl+1 100%",
            Tool::Crop => "Przeciągaj uchwyty lub ramkę · poza kadrem – nowy kadr · Enter / dwuklik – zatwierdź",
            Tool::Eraser => "LPM – usuń · PPM lub Alt+LPM – przywróć · [ ] albo Ctrl+kółko – rozmiar pędzla",
            Tool::RectCut => "Przeciągnij, aby zaznaczyć · Delete – wytnij · Shift+Delete – przywróć · Esc – odznacz",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BrushSettings {
    /// Promień pędzla w pikselach źródła.
    pub radius: f32,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self { radius: 40.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CropHandle {
    Move,
    New,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

#[derive(Clone, Copy, Debug)]
struct CropDrag {
    handle: CropHandle,
    start_rect: [f32; 4],
    start_ptr: [f32; 2],
}

/// Przeciąganie warstwy (smartfona): przesunięcie albo skalowanie z rogu (z zachowaniem proporcji).
#[derive(Clone, Copy, Debug)]
struct OverlayDrag {
    /// `None` – przesunięcie; `Some((sx, sy))` – róg: −1 lewy/górny, +1 prawy/dolny.
    corner: Option<(f32, f32)>,
    start_rect: [f32; 4],
    start_ptr: [f32; 2],
}

fn overlay_corner(h: CropHandle) -> Option<(f32, f32)> {
    match h {
        CropHandle::NW => Some((-1.0, -1.0)),
        CropHandle::NE => Some((1.0, -1.0)),
        CropHandle::SW => Some((-1.0, 1.0)),
        CropHandle::SE => Some((1.0, 1.0)),
        _ => None,
    }
}

/// Nowe położenie warstwy po przesunięciu wskaźnika o `d` (px obrazu); róg skaluje względem
/// przeciwległego rogu, z zachowaniem proporcji (min. 16 px).
fn drag_overlay(drag: &OverlayDrag, d: [f32; 2]) -> [f32; 4] {
    let [x, y, w, h] = drag.start_rect;
    let Some((sx, sy)) = drag.corner else { return [x + d[0], y + d[1], w, h] };
    let (ax, ay) = (if sx > 0.0 { x } else { x + w }, if sy > 0.0 { y } else { y + h });
    let (cx, cy) = ((if sx > 0.0 { x + w } else { x }) + d[0], (if sy > 0.0 { y + h } else { y }) + d[1]);
    let s = ((cx - ax) * sx / w.max(1.0)).max((cy - ay) * sy / h.max(1.0)).max(16.0 / w.min(h).max(1.0));
    let (nw, nh) = (w * s, h * s);
    [if sx > 0.0 { ax } else { ax - nw }, if sy > 0.0 { ay } else { ay - nh }, nw, nh]
}

/// Tekstura podglądu warstwy (pomniejszona do [`OVERLAY_PREVIEW`] px).
fn overlay_texture(ctx: &egui::Context, img: &image::RgbaImage) -> egui::TextureHandle {
    use asone_core::imageio::{fit_within, lanczos3, resize_rgba};
    let size = fit_within([img.width(), img.height()], OVERLAY_PREVIEW);
    let small = resize_rgba(img, size, lanczos3()).unwrap_or_else(|_| img.clone());
    let color = egui::ColorImage::from_rgba_unmultiplied([small.width() as usize, small.height() as usize], small.as_raw());
    ctx.load_texture("asone-overlay", color, egui::TextureOptions::LINEAR)
}

/// Polecenia z menu kontekstowego płótna (wykonuje je aplikacja).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasAction {
    PastePhone,
    PickPhone,
    FitPhone,
    RemovePhone,
    /// Scala kadr, marginesy, tło i warstwę w nowe zdjęcie zakładki.
    Flatten,
    /// Po „Spłaszcz”: powrót do pliku źródłowego.
    RestoreOriginal,
    /// Edycja AI (Replicate) poleceniem z panelu.
    AiEdit,
    /// Edycja AI gotowym poleceniem (`settings::AI_PRESETS`).
    AiPreset(u8),
    RemoveBackground,
    AutoCrop,
    FitSquare,
    Frame5,
    ResetMargins,
    FitView,
    Undo,
    Redo,
}

/// Menu pod prawym przyciskiem na zdjęciu.
fn canvas_menu(
    ui: &mut egui::Ui,
    has_overlay: bool,
    flattened: bool,
    replicate: bool,
    can_undo: bool,
    can_redo: bool,
    actions: &mut Vec<CanvasAction>,
) {
    use crate::platform::{CAN_PASTE_IMAGE, CAN_PICK_OVERLAY};
    ui.set_min_width(240.0);
    let mut item = |ui: &mut egui::Ui, enabled: bool, label: &str, action: CanvasAction| {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            actions.push(action);
            ui.close();
        }
    };
    let paste = if has_overlay { "Podmień smartfon ze schowka" } else { "Wklej smartfon ze schowka" };
    item(ui, CAN_PASTE_IMAGE, paste, CanvasAction::PastePhone);
    item(ui, CAN_PICK_OVERLAY, "Wklej smartfon z pliku…", CanvasAction::PickPhone);
    if has_overlay {
        item(ui, true, "Dopasuj smartfon do pasa", CanvasAction::FitPhone);
        item(ui, true, "Usuń smartfon", CanvasAction::RemovePhone);
    }
    item(ui, true, "Spłaszcz zdjęcie", CanvasAction::Flatten);
    if flattened {
        item(ui, true, "Przywróć oryginał (z pliku)", CanvasAction::RestoreOriginal);
    }
    ui.separator();
    if replicate {
        ui.menu_button("Edycja AI (Replicate)", |ui| {
            ui.set_min_width(220.0);
            for (i, (label, _)) in crate::settings::AI_PRESETS.iter().enumerate() {
                item(ui, true, label, CanvasAction::AiPreset(i as u8));
            }
            ui.separator();
            item(ui, true, "Polecenie z panelu", CanvasAction::AiEdit);
        });
    }
    item(ui, crate::ai::AVAILABLE || crate::ai::FAL_AVAILABLE, "Usuń tło", CanvasAction::RemoveBackground);
    item(ui, true, "Kadruj (auto)", CanvasAction::AutoCrop);
    item(ui, true, "Dopasuj do kwadratu 1:1", CanvasAction::FitSquare);
    item(ui, true, "Ramka 5 px", CanvasAction::Frame5);
    item(ui, true, "Zeruj marginesy", CanvasAction::ResetMargins);
    ui.separator();
    item(ui, true, "Dopasuj widok (Ctrl+0)", CanvasAction::FitView);
    item(ui, can_undo, "Cofnij (Ctrl+Z)", CanvasAction::Undo);
    item(ui, can_redo, "Ponów (Ctrl+Shift+Z)", CanvasAction::Redo);
}

/// Stan interakcji płótna przechowywany w dokumencie.
#[derive(Default)]
pub struct CanvasState {
    crop_drag: Option<CropDrag>,
    overlay_drag: Option<OverlayDrag>,
    last_brush: Option<[f32; 2]>,
    rect_anchor: Option<[f32; 2]>,
    last_canvas_size: Vec2,
}

impl CanvasState {
    /// Środek płótna (względem jego lewego górnego rogu) z ostatniej klatki.
    pub fn view_center(&self) -> Vec2 {
        self.last_canvas_size * 0.5
    }
}

#[derive(Default)]
pub struct CanvasOutput {
    pub switch_tool: Option<Tool>,
    /// Polecenia z menu kontekstowego.
    pub actions: Vec<CanvasAction>,
    /// Gumka + Ctrl + kółko: mnożnik promienia pędzla.
    pub brush_factor: Option<f32>,
}

/// Dopasowuje widok tak, by `size` (od `origin`, w px źródła) zmieścił się w oknie.
pub fn fit_camera(cam: &mut Camera, view: Vec2, origin: [f32; 2], size: [f32; 2]) {
    let pad = 24.0;
    let zx = (view.x - 2.0 * pad).max(16.0) / size[0].max(1.0);
    let zy = (view.y - 2.0 * pad).max(16.0) / size[1].max(1.0);
    cam.zoom = zx.min(zy).clamp(MIN_ZOOM, 1.0);
    let center = vec2(origin[0] + size[0] * 0.5, origin[1] + size[1] * 0.5);
    cam.pan = view * 0.5 - center * cam.zoom;
    cam.needs_fit = false;
}

/// Zoom w punkt: piksel obrazu pod `anchor` (względem lewego górnego rogu płótna) zostaje w miejscu.
pub fn zoom_at(cam: &mut Camera, anchor: Vec2, factor: f32) {
    let new_zoom = (cam.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    let src = (anchor - cam.pan) / cam.zoom;
    cam.pan = anchor - src * new_zoom;
    cam.zoom = new_zoom;
}

fn hit_test(r: Rect, p: Pos2) -> Option<CropHandle> {
    let tol = 8.0;
    let near_l = (p.x - r.left()).abs() <= tol;
    let near_r = (p.x - r.right()).abs() <= tol;
    let near_t = (p.y - r.top()).abs() <= tol;
    let near_b = (p.y - r.bottom()).abs() <= tol;
    let in_x = p.x >= r.left() - tol && p.x <= r.right() + tol;
    let in_y = p.y >= r.top() - tol && p.y <= r.bottom() + tol;
    Some(match () {
        _ if near_t && near_l => CropHandle::NW,
        _ if near_t && near_r => CropHandle::NE,
        _ if near_b && near_l => CropHandle::SW,
        _ if near_b && near_r => CropHandle::SE,
        _ if near_t && in_x => CropHandle::N,
        _ if near_b && in_x => CropHandle::S,
        _ if near_l && in_y => CropHandle::W,
        _ if near_r && in_y => CropHandle::E,
        _ if r.contains(p) => CropHandle::Move,
        _ => return None,
    })
}

fn handle_cursor(h: CropHandle) -> CursorIcon {
    match h {
        CropHandle::Move => CursorIcon::Move,
        CropHandle::New => CursorIcon::Crosshair,
        CropHandle::N | CropHandle::S => CursorIcon::ResizeVertical,
        CropHandle::E | CropHandle::W => CursorIcon::ResizeHorizontal,
        CropHandle::NE | CropHandle::SW => CursorIcon::ResizeNeSw,
        CropHandle::NW | CropHandle::SE => CursorIcon::ResizeNwSe,
    }
}

/// Nowy prostokąt kadru po przeciągnięciu (px źródła, zaokrąglony i przycięty do obrazu).
fn drag_crop(drag: &CropDrag, cur: [f32; 2], src: [u32; 2]) -> PxRect {
    let (sw, sh) = (src[0] as f32, src[1] as f32);
    let [mut x0, mut y0, mut x1, mut y1] = drag.start_rect;
    let (dx, dy) = (cur[0] - drag.start_ptr[0], cur[1] - drag.start_ptr[1]);
    use CropHandle::*;
    match drag.handle {
        Move => {
            let dx = dx.clamp(-x0, sw - x1);
            let dy = dy.clamp(-y0, sh - y1);
            x0 += dx;
            x1 += dx;
            y0 += dy;
            y1 += dy;
        }
        New => {
            [x0, y0, x1, y1] = [drag.start_ptr[0], drag.start_ptr[1], cur[0], cur[1]];
        }
        _ => {
            if matches!(drag.handle, N | NE | NW) {
                y0 += dy;
            }
            if matches!(drag.handle, S | SE | SW) {
                y1 += dy;
            }
            if matches!(drag.handle, W | NW | SW) {
                x0 += dx;
            }
            if matches!(drag.handle, E | NE | SE) {
                x1 += dx;
            }
        }
    }
    let clamp_x = |v: f32| v.round().clamp(0.0, sw) as u32;
    let clamp_y = |v: f32| v.round().clamp(0.0, sh) as u32;
    let (mut ax, mut bx) = (clamp_x(x0.min(x1)), clamp_x(x0.max(x1)));
    let (mut ay, mut by) = (clamp_y(y0.min(y1)), clamp_y(y0.max(y1)));
    if bx <= ax {
        if ax >= src[0] { ax = src[0] - 1 } else { bx = ax + 1 }
        bx = bx.max(ax + 1);
    }
    if by <= ay {
        if ay >= src[1] { ay = src[1] - 1 } else { by = ay + 1 }
        by = by.max(ay + 1);
    }
    PxRect { x0: ax, y0: ay, x1: bx, y1: by }
}

/// Zawartość zakładki `AppTab::Photo`.
pub fn photo_ui(
    ui: &mut egui::Ui,
    doc: &mut Document,
    gpu: Option<&Arc<GpuShared>>,
    tool: Tool,
    brush: &BrushSettings,
    inspection: bool,
    replicate: bool,
) -> CanvasOutput {
    let mut out = CanvasOutput::default();
    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, 0.0, theme::p().canvas);

    let message = match (&doc.load, gpu) {
        (_, None) => Some(("Brak akceleracji GPU (wgpu) – podgląd niedostępny".to_string(), true)),
        (LoadState::Failed(e), _) => Some((format!("Nie udało się wczytać {}:\n{e}", doc.file_name), true)),
        (LoadState::NotLoaded | LoadState::Loading, _) => Some((format!("Wczytywanie {}…", doc.file_name), false)),
        (LoadState::Ready, _) => None,
    };
    if let Some((text, is_error)) = message {
        ui.scope_builder(egui::UiBuilder::new().max_rect(full), |ui| {
            ui.centered_and_justified(|ui| {
                if is_error {
                    ui.colored_label(theme::p().danger, text);
                } else {
                    ui.horizontal_centered(|ui| {
                        ui.spinner();
                        ui.label(text);
                    });
                }
            });
        });
        ui.allocate_rect(full, Sense::hover());
        return out;
    }
    let (Some(gpu), Some(src), Some(mip_count)) = (gpu, doc.src_size(), doc.gpu.as_ref().map(|g| g.mip_count)) else {
        return out;
    };

    let status_h = 22.0;
    let canvas_rect = Rect::from_min_max(full.min, pos2(full.max.x, (full.max.y - status_h).max(full.min.y)));
    let status_rect = Rect::from_min_max(pos2(full.min.x, canvas_rect.max.y), full.max);
    let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());

    let full_source = tool == Tool::Crop;
    let layout = Layout::new(&doc.edits, src, full_source);
    let extent_origin = layout.origin_in_src();

    // --- Kamera ---------------------------------------------------------------------------
    let view_size = canvas_rect.size();
    let st = &mut doc.canvas;
    if st.last_canvas_size != Vec2::ZERO && st.last_canvas_size != view_size {
        doc.camera.pan += (view_size - st.last_canvas_size) * 0.5;
    }
    st.last_canvas_size = view_size;
    if doc.camera.needs_fit {
        fit_camera(&mut doc.camera, view_size, extent_origin, layout.out_size);
    }

    let (scroll, zoom_delta, space_down, alt_down, primary_down, secondary_down, latest_pos, press_origin) =
        ui.input(|i| {
            (
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.key_down(Key::Space),
                i.modifiers.alt,
                i.pointer.primary_down(),
                i.pointer.secondary_down(),
                i.pointer.latest_pos(),
                i.pointer.press_origin(),
            )
        });

    if response.contains_pointer() {
        let mut factor = zoom_delta;
        if scroll.y != 0.0 {
            factor *= (scroll.y * 0.0025).exp();
        }
        // Gumka: Ctrl (Cmd) + kółko zmienia rozmiar pędzla zamiast przybliżać.
        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if factor != 1.0 && tool == Tool::Eraser && ctrl {
            out.brush_factor = Some(factor);
        } else if factor != 1.0
            && let Some(p) = response.hover_pos()
        {
            zoom_at(&mut doc.camera, p - canvas_rect.min, factor);
        }
    }

    // --- Warstwa (smartfon): przesuwanie i skalowanie narzędziem „Przesuwanie” ---------------
    let overlay = doc.edits.overlay.filter(|_| doc.overlay_image.is_some() && !full_source);
    let overlay_screen = |cam: Camera, r: [f32; 4]| {
        Rect::from_min_size(
            canvas_rect.min + cam.pan + vec2(extent_origin[0] + r[0], extent_origin[1] + r[1]) * cam.zoom,
            vec2(r[2], r[3]) * cam.zoom,
        )
    };
    if tool == Tool::Move
        && !space_down
        && let Some(r) = overlay
        && response.drag_started_by(PointerButton::Primary)
        && let Some(p) = press_origin
        && let Some(h) = hit_test(overlay_screen(doc.camera, r), p)
    {
        let v = (p - canvas_rect.min - doc.camera.pan) / doc.camera.zoom;
        doc.canvas.overlay_drag = Some(OverlayDrag { corner: overlay_corner(h), start_rect: r, start_ptr: [v.x, v.y] });
    }

    let panning = response.dragged_by(PointerButton::Middle)
        || (response.dragged_by(PointerButton::Primary)
            && (space_down || (tool == Tool::Move && doc.canvas.overlay_drag.is_none())));
    if panning {
        doc.camera.pan += response.drag_delta();
    }

    let cam = doc.camera;
    let to_screen = |s: [f32; 2]| canvas_rect.min + cam.pan + vec2(s[0], s[1]) * cam.zoom;
    let to_src = |p: Pos2| {
        let v = (p - canvas_rect.min - cam.pan) / cam.zoom;
        [v.x, v.y]
    };
    let tools_active = !space_down && !panning;

    // --- Narzędzia ------------------------------------------------------------------------
    let mut cursor = None;
    match tool {
        Tool::Move => {
            if let Some(drag) = doc.canvas.overlay_drag {
                if response.dragged_by(PointerButton::Primary)
                    && let Some(p) = latest_pos
                {
                    let cur = to_src(p);
                    doc.edits.overlay = Some(drag_overlay(&drag, [cur[0] - drag.start_ptr[0], cur[1] - drag.start_ptr[1]]));
                }
                cursor = Some(match drag.corner {
                    None => CursorIcon::Grabbing,
                    Some((sx, sy)) if sx * sy > 0.0 => CursorIcon::ResizeNwSe,
                    Some(_) => CursorIcon::ResizeNeSw,
                });
                if response.drag_stopped() || !primary_down {
                    doc.canvas.overlay_drag = None;
                }
            } else {
                let hover = overlay.zip(response.hover_pos()).and_then(|(r, p)| hit_test(overlay_screen(cam, r), p));
                cursor = Some(match hover {
                    Some(h) if overlay_corner(h).is_some() => handle_cursor(h),
                    Some(_) => CursorIcon::Move,
                    None if panning => CursorIcon::Grabbing,
                    None => CursorIcon::Grab,
                });
            }
        }
        Tool::Crop if tools_active => {
            let c = doc.edits.crop_px(src);
            let crop_screen = Rect::from_min_max(
                to_screen([c.x0 as f32, c.y0 as f32]),
                to_screen([c.x1 as f32, c.y1 as f32]),
            );
            if response.drag_started_by(PointerButton::Primary)
                && let Some(p) = press_origin {
                    doc.canvas.crop_drag = Some(CropDrag {
                        handle: hit_test(crop_screen, p).unwrap_or(CropHandle::New),
                        start_rect: [c.x0 as f32, c.y0 as f32, c.x1 as f32, c.y1 as f32],
                        start_ptr: to_src(p),
                    });
                }
            if let (Some(drag), Some(p)) = (doc.canvas.crop_drag, latest_pos) {
                if response.dragged_by(PointerButton::Primary) {
                    let r = drag_crop(&drag, to_src(p), src);
                    doc.edits.set_crop_px(r, src);
                }
                cursor = Some(handle_cursor(drag.handle));
            } else if let Some(p) = response.hover_pos() {
                cursor = Some(handle_cursor(hit_test(crop_screen, p).unwrap_or(CropHandle::New)));
            }
            if response.drag_stopped() {
                doc.canvas.crop_drag = None;
            }
            if response.double_clicked() {
                out.switch_tool = Some(Tool::Move);
            }
        }
        Tool::Eraser if tools_active => {
            let painting = response.is_pointer_button_down_on() && (primary_down || secondary_down);
            match (painting, latest_pos) {
                (true, Some(p)) => {
                    let restore = secondary_down || alt_down;
                    let cur = to_src(p);
                    let from = doc.canvas.last_brush.unwrap_or(cur);
                    doc.begin_mask_change();
                    let dirty = stamp_segment(&mut doc.mask_cpu, src, from, cur, brush.radius, restore);
                    if dirty.is_some() {
                        doc.edits.use_bg_removal = true;
                        doc.mask_touched = true;
                    }
                    doc.mask_dirty = union_opt(doc.mask_dirty, dirty);
                    doc.canvas.last_brush = Some(cur);
                }
                _ => doc.canvas.last_brush = None,
            }
            if response.contains_pointer() {
                cursor = Some(CursorIcon::None);
            }
        }
        Tool::RectCut if tools_active => {
            if response.drag_started_by(PointerButton::Primary) {
                doc.canvas.rect_anchor = press_origin.map(to_src);
                doc.selection = None;
            }
            if let (Some(anchor), Some(p)) = (doc.canvas.rect_anchor, latest_pos)
                && response.dragged_by(PointerButton::Primary) {
                    doc.selection = PxRect::from_points(anchor, to_src(p), src);
                }
            if response.drag_stopped() {
                doc.canvas.rect_anchor = None;
            }
            if response.clicked() {
                doc.selection = None;
            }
            cursor = Some(CursorIcon::Crosshair);
        }
        _ => {}
    }
    if space_down && response.contains_pointer() {
        cursor = Some(if panning { CursorIcon::Grabbing } else { CursorIcon::Grab });
    }
    if let (Some(c), true) = (cursor, response.contains_pointer() || response.dragged()) {
        ui.ctx().set_cursor_icon(c);
    }

    // --- GPU: maska (dirty rect) + płótno -------------------------------------------------
    let Some(doc_gpu) = doc.gpu.as_ref() else { return out };
    if let Some(r) = doc.mask_dirty.take() {
        upload_mask_region(&gpu.queue, &doc_gpu.mask_texture, &doc.mask_cpu, src, r);
    }
    // Kadr mógł się zmienić podczas przeciągania – przeliczamy geometrię.
    let layout = Layout::new(&doc.edits, src, full_source);
    let origin_src = layout.origin_in_src();
    let ppp = ui.ctx().pixels_per_point();
    let origin_screen = to_screen(origin_src);
    let uniforms = build_uniforms(
        &doc.edits,
        &layout,
        src,
        mip_count,
        [origin_screen.x * ppp, origin_screen.y * ppp],
        cam.zoom * ppp,
        RenderMode::Preview { checker_px: (8.0 * ppp).round(), target_srgb: gpu.target_is_srgb, inspection },
    );
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        canvas_rect,
        CanvasCallback {
            shared: gpu.clone(),
            uniform_buffer: doc_gpu.uniform_buffer.clone(),
            bind_group: doc_gpu.bind_group.clone(),
            uniforms,
        },
    ));

    // --- Nakładki -------------------------------------------------------------------------
    let painter = ui.painter_at(canvas_rect);
    let image_rect = Rect::from_min_size(origin_screen, vec2(layout.out_size[0], layout.out_size[1]) * cam.zoom);
    painter.rect_stroke(image_rect, 0.0, Stroke::new(1.0, Color32::from_black_alpha(140)), egui::StrokeKind::Outside);

    // Warstwa (smartfon) na zdjęciu – przycięta do obrazu wynikowego, jak w zapisanym pliku.
    if let (Some(r), Some(img)) = (doc.edits.overlay.filter(|_| !full_source), doc.overlay_image.clone()) {
        let tex = doc.overlay_texture.get_or_insert_with(|| overlay_texture(ui.ctx(), &img));
        let rs = Rect::from_min_size(to_screen([origin_src[0] + r[0], origin_src[1] + r[1]]), vec2(r[2], r[3]) * cam.zoom);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.with_clip_rect(image_rect.intersect(canvas_rect)).image(tex.id(), rs, uv, Color32::WHITE);
        if tool == Tool::Move {
            let pts = [rs.left_top(), rs.right_top(), rs.right_bottom(), rs.left_bottom(), rs.left_top()];
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, Color32::from_white_alpha(220)), 5.0, 4.0));
            for p in [rs.left_top(), rs.right_top(), rs.left_bottom(), rs.right_bottom()] {
                let h = Rect::from_center_size(p, vec2(8.0, 8.0));
                painter.rect_filled(h, 1.0, Color32::WHITE);
                painter.rect_stroke(h, 1.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Outside);
            }
        }
    }

    if tool == Tool::Crop {
        let c = doc.edits.crop_px(src);
        let cr = Rect::from_min_max(to_screen([c.x0 as f32, c.y0 as f32]), to_screen([c.x1 as f32, c.y1 as f32]));
        let dim = Color32::from_black_alpha(150);
        let ir = image_rect;
        for r in [
            Rect::from_min_max(ir.min, pos2(ir.max.x, cr.min.y)),
            Rect::from_min_max(pos2(ir.min.x, cr.max.y), ir.max),
            Rect::from_min_max(pos2(ir.min.x, cr.min.y), pos2(cr.min.x, cr.max.y)),
            Rect::from_min_max(pos2(cr.max.x, cr.min.y), pos2(ir.max.x, cr.max.y)),
        ] {
            if r.is_positive() {
                painter.rect_filled(r, 0.0, dim);
            }
        }
        let thirds = Stroke::new(1.0, Color32::from_white_alpha(70));
        for t in [1.0 / 3.0, 2.0 / 3.0] {
            let x = cr.min.x + cr.width() * t;
            let y = cr.min.y + cr.height() * t;
            painter.line_segment([pos2(x, cr.min.y), pos2(x, cr.max.y)], thirds);
            painter.line_segment([pos2(cr.min.x, y), pos2(cr.max.x, y)], thirds);
        }
        painter.rect_stroke(cr, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Middle);
        let hs = 7.0;
        for p in [
            cr.left_top(),
            cr.center_top(),
            cr.right_top(),
            cr.left_center(),
            cr.right_center(),
            cr.left_bottom(),
            cr.center_bottom(),
            cr.right_bottom(),
        ] {
            let h = Rect::from_center_size(p, vec2(hs, hs));
            painter.rect_filled(h, 1.0, Color32::WHITE);
            painter.rect_stroke(h, 1.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Outside);
        }
        painter.text(
            cr.left_top() + vec2(0.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{} × {} px", c.width(), c.height()),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    }

    if let Some(sel) = doc.selection {
        let r = Rect::from_min_max(to_screen([sel.x0 as f32, sel.y0 as f32]), to_screen([sel.x1 as f32, sel.y1 as f32]));
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Middle);
        let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, Color32::BLACK), 4.0, 4.0));
    }

    if tool == Tool::Eraser && !space_down
        && let Some(p) = response.hover_pos() {
            let r = (brush.radius * cam.zoom).max(1.5);
            painter.circle_stroke(p, r, Stroke::new(2.5, Color32::from_black_alpha(160)));
            painter.circle_stroke(p, r, Stroke::new(1.0, Color32::WHITE));
            if r < 6.0 {
                painter.line_segment([p - vec2(4.0, 0.0), p + vec2(4.0, 0.0)], Stroke::new(1.0, Color32::WHITE));
                painter.line_segment([p - vec2(0.0, 4.0), p + vec2(0.0, 4.0)], Stroke::new(1.0, Color32::WHITE));
            }
        }

    // --- Menu kontekstowe (PPM; gumka używa PPM do przywracania) ---------------------------
    if tool != Tool::Eraser {
        let has_overlay = doc.overlay_image.is_some() && doc.edits.overlay.is_some();
        let flattened = doc.content.is_some();
        let (can_undo, can_redo) = (doc.history.can_undo(), doc.history.can_redo());
        response.context_menu(|ui| canvas_menu(ui, has_overlay, flattened, replicate, can_undo, can_redo, &mut out.actions));
    }

    // --- Pasek stanu ----------------------------------------------------------------------
    let sp = ui.painter_at(status_rect);
    sp.rect_filled(status_rect, 0.0, theme::p().chrome);
    let out_size = doc.edits.output_size(src);
    let mut left = format!(
        "{:.0}%   źródło {}×{}   wynik {}×{}",
        cam.zoom * 100.0,
        src[0],
        src[1],
        out_size[0],
        out_size[1]
    );
    if let Some(p) = response.hover_pos() {
        let s = to_src(p);
        if s[0] >= 0.0 && s[1] >= 0.0 && s[0] < src[0] as f32 && s[1] < src[1] as f32 {
            left.push_str(&format!("   x {} y {}", s[0] as u32, s[1] as u32));
        }
    }
    let font = egui::FontId::proportional(12.0);
    if inspection {
        left.push_str("   · INSPEKCJA TŁA (I)");
    }
    let left_rect = sp.text(status_rect.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, left, font.clone(), theme::p().text);
    // Podpowiedź narzędzia po prawej – tylko tyle, ile się zmieści (bez nachodzenia na lewy tekst).
    let room = status_rect.right() - 8.0 - (left_rect.right() + 24.0);
    if room > 60.0 {
        let mut job = egui::text::LayoutJob::simple_singleline(tool.hint().to_string(), font, theme::p().text_dim);
        job.wrap = egui::text::TextWrapping::truncate_at_width(room);
        let galley = sp.layout_job(job);
        let pos = egui::pos2(status_rect.right() - 8.0 - galley.size().x, status_rect.center().y - galley.size().y / 2.0);
        sp.galley(pos, galley, theme::p().text_dim);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_w_punkt_kursora() {
        let mut cam = Camera { zoom: 0.5, pan: vec2(100.0, 50.0), needs_fit: false };
        let anchor = vec2(300.0, 250.0);
        let before = (anchor - cam.pan) / cam.zoom;
        zoom_at(&mut cam, anchor, 2.0);
        let after = (anchor - cam.pan) / cam.zoom;
        assert!((before - after).length() < 1e-3);
        assert_eq!(cam.zoom, 1.0);
    }

    #[test]
    fn dopasowanie_widoku() {
        let mut cam = Camera::default();
        fit_camera(&mut cam, vec2(1048.0, 848.0), [0.0, 0.0], [4000.0, 2000.0]);
        assert!((cam.zoom - 0.25).abs() < 1e-6);
        // środek obrazu na środku płótna
        let center = cam.pan + vec2(2000.0, 1000.0) * cam.zoom;
        assert!((center - vec2(524.0, 424.0)).length() < 1e-3);
    }

    #[test]
    fn przeciaganie_kadru() {
        let src = [1000, 800];
        let drag = CropDrag { handle: CropHandle::SE, start_rect: [100.0, 100.0, 500.0, 400.0], start_ptr: [500.0, 400.0] };
        assert_eq!(drag_crop(&drag, [700.4, 9999.0], src), PxRect { x0: 100, y0: 100, x1: 700, y1: 800 });
        let drag = CropDrag { handle: CropHandle::Move, ..drag };
        assert_eq!(drag_crop(&drag, [-1000.0, 400.0], src), PxRect { x0: 0, y0: 100, x1: 400, y1: 400 });
        let drag = CropDrag { handle: CropHandle::New, start_rect: [0.0; 4], start_ptr: [300.0, 300.0] };
        assert_eq!(drag_crop(&drag, [200.0, 350.0], src), PxRect { x0: 200, y0: 300, x1: 300, y1: 350 });
        // zerowa szerokość → min. 1 px
        assert_eq!(drag_crop(&drag, [300.0, 300.0], src), PxRect { x0: 300, y0: 300, x1: 301, y1: 301 });
        // przeciągnięcie lewej krawędzi za prawą odwraca prostokąt
        let drag = CropDrag { handle: CropHandle::W, start_rect: [100.0, 100.0, 500.0, 400.0], start_ptr: [100.0, 200.0] };
        assert_eq!(drag_crop(&drag, [600.0, 200.0], src), PxRect { x0: 500, y0: 100, x1: 600, y1: 400 });
    }

    #[test]
    fn przeciaganie_warstwy() {
        let start = [100.0, 50.0, 40.0, 80.0];
        let mv = OverlayDrag { corner: None, start_rect: start, start_ptr: [0.0, 0.0] };
        assert_eq!(drag_overlay(&mv, [10.0, -5.0]), [110.0, 45.0, 40.0, 80.0]);
        // Prawy dolny róg o +40 px w poziomie → ×2, lewy górny róg w miejscu.
        let se = OverlayDrag { corner: Some((1.0, 1.0)), ..mv };
        assert_eq!(drag_overlay(&se, [40.0, 0.0]), [100.0, 50.0, 80.0, 160.0]);
        // Lewy górny róg do środka → mniejsza, prawy dolny róg w miejscu, proporcje zachowane.
        let nw = OverlayDrag { corner: Some((-1.0, -1.0)), ..mv };
        let r = drag_overlay(&nw, [20.0, 0.0]);
        assert_eq!([r[0] + r[2], r[1] + r[3]], [140.0, 130.0]);
        assert!((r[2] / r[3] - 0.5).abs() < 1e-4);
        // Przeciągnięcie za przeciwległy róg – minimalny rozmiar zamiast odwrócenia.
        assert!(drag_overlay(&se, [-500.0, -500.0])[2] >= 16.0 - 1e-3);
    }

    #[test]
    fn trafienia_uchwytow() {
        let r = Rect::from_min_max(pos2(100.0, 100.0), pos2(300.0, 200.0));
        assert_eq!(hit_test(r, pos2(102.0, 98.0)), Some(CropHandle::NW));
        assert_eq!(hit_test(r, pos2(200.0, 204.0)), Some(CropHandle::S));
        assert_eq!(hit_test(r, pos2(200.0, 150.0)), Some(CropHandle::Move));
        assert_eq!(hit_test(r, pos2(10.0, 10.0)), None);
    }
}
