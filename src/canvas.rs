//! The canvas: view transform, mouse interaction, tool state machine and overlays.

use crate::app::App;
use crate::document::{DocState, Document, Layer, Snapshot};
use crate::paint::{self, BrushKind, LineParams, ShapeParams};
use crate::raster::{self, Col, SelMode};
use crate::text;
use crate::tools::Tool;
use egui::{pos2, vec2, Color32, CursorIcon, Modifiers, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use tiny_skia::{FillRule, FilterQuality, Mask, MaskType, Paint, PathBuilder, Pixmap, PixmapPaint, Transform};


pub const ZOOMS: [f32; 28] = [
    0.01, 0.02, 0.03, 0.05, 0.08, 0.12, 0.16, 0.25, 0.33, 0.5, 0.66, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0,
    16.0, 24.0, 32.0, 48.0, 64.0, 96.0,
];

pub fn zoom_step(z: f32, dir: i32) -> f32 {
    if dir > 0 {
        ZOOMS.iter().copied().find(|&v| v > z * 1.001).unwrap_or(*ZOOMS.last().unwrap())
    } else {
        ZOOMS.iter().rev().copied().find(|&v| v < z / 1.001).unwrap_or(ZOOMS[0])
    }
}

pub struct StrokeEdit {
    pub tool: Tool,
    pub base: Layer,
    pub layer: usize,
    pub points: Vec<Pos2>,
    pub color: Col,
    pub other: Col,
    pub offset: Vec2,
}

pub struct SelectEdit {
    pub tool: Tool,
    pub start: Pos2,
    pub cur: Pos2,
    pub points: Vec<Pos2>,
    pub mode: SelMode,
    pub before: Option<Mask>,
    pub square: bool,
}

#[derive(Clone, Copy)]
pub enum ShapeDrag {
    Create { origin: Pos2 },
    Handle { idx: usize, sa: Pos2, sb: Pos2, start: Pos2 },
    Move { start: Pos2, sa: Pos2, sb: Pos2 },
}

pub struct ShapeEdit {
    pub base: Layer,
    pub layer: usize,
    pub a: Pos2,
    pub b: Pos2,
    pub swap: bool,
    pub drag: Option<ShapeDrag>,
}

#[derive(Clone, Copy)]
pub enum LineDrag {
    Create { origin: Pos2 },
    Handle(usize),
    Move { start: Pos2, s0: Pos2, s1: Pos2, sc: Pos2 },
}

pub struct LineEdit {
    pub base: Layer,
    pub layer: usize,
    pub p0: Pos2,
    pub p1: Pos2,
    pub c: Pos2,
    pub bent: bool,
    pub swap: bool,
    pub drag: Option<LineDrag>,
}

#[derive(Clone, Copy)]
pub enum GradDrag {
    Create,
    Handle(usize),
    Move { start: Pos2, s0: Pos2, s1: Pos2 },
}

pub struct GradEdit {
    pub base: Layer,
    pub layer: usize,
    pub s: Pos2,
    pub e: Pos2,
    pub swap: bool,
    pub drag: Option<GradDrag>,
}

pub struct TextEdit {
    pub base: Layer,
    pub layer: usize,
    pub pos: Pos2,
    pub text: String,
    pub cursor: usize,
    pub drag: Option<(Pos2, Pos2)>,
}

#[derive(Clone, Copy)]
pub struct MoveDrag {
    pub button: PointerButton,
    pub start: Pos2,
    pub tx: f32,
    pub ty: f32,
    pub angle: f32,
}

pub struct MoveEdit {
    pub name: &'static str,
    pub before: DocState,
    pub layer: usize,
    pub hole: Pixmap,
    pub floating: Pixmap,
    pub mask_px: Pixmap,
    pub tx: f32,
    pub ty: f32,
    pub angle: f32,
    pub pivot: Pos2,
    pub drag: Option<MoveDrag>,
    pub changed: bool,
}

pub struct SelMoveEdit {
    pub before: Option<Mask>,
    pub mask_px: Pixmap,
    pub tx: f32,
    pub ty: f32,
    pub angle: f32,
    pub pivot: Pos2,
    pub drag: Option<MoveDrag>,
}

#[derive(Default)]
pub enum Pending {
    #[default]
    None,
    Stroke(StrokeEdit),
    Select(SelectEdit),
    Shape(ShapeEdit),
    Line(LineEdit),
    Gradient(GradEdit),
    Text(TextEdit),
    MovePixels(MoveEdit),
    MoveSel(SelMoveEdit),
    ZoomRect { start: Pos2, cur: Pos2, button: PointerButton },
    Picking,
}

impl Pending {
    pub fn is_none(&self) -> bool {
        matches!(self, Pending::None)
    }
    /// Editable states that survive between drags and show a "Finish" button.
    pub fn is_editable(&self) -> bool {
        matches!(
            self,
            Pending::Shape(_) | Pending::Line(_) | Pending::Gradient(_) | Pending::Text(_) | Pending::MovePixels(_) | Pending::MoveSel(_)
        )
    }
}

#[derive(Clone, Copy)]
pub struct DragInfo {
    pub button: PointerButton,
    pub pan: bool,
    pub last_screen: Pos2,
}

pub fn shape_handles(a: Pos2, b: Pos2) -> [Pos2; 8] {
    let (x0, y0, x1, y1) = paint::norm_rect(a, b);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        pos2(x0, y0),
        pos2(cx, y0),
        pos2(x1, y0),
        pos2(x1, cy),
        pos2(x1, y1),
        pos2(cx, y1),
        pos2(x0, y1),
        pos2(x0, cy),
    ]
}

/// Resize a rectangle by dragging handle `idx` by `d`. `centre` mirrors the opposite edge,
/// `keep` preserves the aspect ratio when dragging a corner.
pub fn resize_rect(sa: Pos2, sb: Pos2, idx: usize, d: Vec2, centre: bool, keep: bool) -> (Pos2, Pos2) {
    let (mut x0, mut y0, mut x1, mut y1) = paint::norm_rect(sa, sb);
    let (ow, oh) = (x1 - x0, y1 - y0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let left = matches!(idx, 0 | 6 | 7);
    let right = matches!(idx, 2 | 3 | 4);
    let top = matches!(idx, 0 | 1 | 2);
    let bottom = matches!(idx, 4 | 5 | 6);
    if left {
        x0 += d.x;
        if centre {
            x1 -= d.x;
        }
    }
    if right {
        x1 += d.x;
        if centre {
            x0 -= d.x;
        }
    }
    if top {
        y0 += d.y;
        if centre {
            y1 -= d.y;
        }
    }
    if bottom {
        y1 += d.y;
        if centre {
            y0 -= d.y;
        }
    }
    if keep && ow > 0.5 && oh > 0.5 && (left || right) && (top || bottom) {
        let (nw, nh) = (x1 - x0, y1 - y0);
        let s = (nw / ow).abs().max((nh / oh).abs());
        let tw = ow * s * if nw < 0.0 { -1.0 } else { 1.0 };
        let th = oh * s * if nh < 0.0 { -1.0 } else { 1.0 };
        if centre {
            x0 = cx - tw / 2.0;
            x1 = cx + tw / 2.0;
            y0 = cy - th / 2.0;
            y1 = cy + th / 2.0;
        } else {
            if left {
                x0 = x1 - tw;
            } else {
                x1 = x0 + tw;
            }
            if top {
                y0 = y1 - th;
            } else {
                y1 = y0 + th;
            }
        }
    }
    (pos2(x0, y0), pos2(x1, y1))
}

fn sgn(v: f32) -> f32 {
    if v < 0.0 {
        -1.0
    } else {
        1.0
    }
}

fn pts_rect(points: &[Pos2], pad: f32, w: u32, h: u32) -> Option<[i32; 4]> {
    if points.is_empty() {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let r = [(x0 - pad).floor().max(0.0) as i32, (y0 - pad).floor().max(0.0) as i32, ((x1 + pad).ceil() as i32).min(w as i32), ((y1 + pad).ceil() as i32).min(h as i32)];
    (r[2] > r[0] && r[3] > r[1]).then_some(r)
}
fn union_rect(a: Option<[i32; 4]>, b: [i32; 4]) -> Option<[i32; 4]> {
    Some(match a {
        None => b,
        Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
    })
}
fn restore_rect(dst: &mut Pixmap, src: &Pixmap, r: [i32; 4]) {
    if dst.width() != src.width() || dst.height() != src.height() {
        *dst = src.clone();
        return;
    }
    let w = dst.width() as usize;
    let (x0, x1) = (r[0] as usize * 4, r[2] as usize * 4);
    let sd = src.data();
    let dd = dst.data_mut();
    for y in r[1] as usize..r[3] as usize {
        let o = y * w * 4;
        dd[o + x0..o + x1].copy_from_slice(&sd[o + x0..o + x1]);
    }
}
fn restore(dst: &mut Pixmap, src: &Pixmap) {
    if dst.width() == src.width() && dst.height() == src.height() {
        dst.data_mut().copy_from_slice(src.data());
    } else {
        *dst = src.clone();
    }
}

fn move_transform(tx: f32, ty: f32, angle: f32, pivot: Pos2) -> Transform {
    Transform::from_translate(tx, ty).pre_concat(Transform::from_rotate_at(angle, pivot.x, pivot.y))
}

fn transformed_mask(mask_px: &Pixmap, t: Transform, smooth: bool) -> Mask {
    let mut mp = raster::new_pixmap(mask_px.width(), mask_px.height());
    let q = if smooth { FilterQuality::Bilinear } else { FilterQuality::Nearest };
    mp.draw_pixmap(0, 0, mask_px.as_ref(), &PixmapPaint { quality: q, ..Default::default() }, t, None);
    Mask::from_pixmap(mp.as_ref(), MaskType::Alpha)
}

impl App {
    pub fn doc_mut(&mut self) -> Option<&mut Document> {
        self.docs.get_mut(self.cur)
    }

    pub fn text_params(&self) -> text::TextParams {
        text::TextParams {
            font: self.ts.font,
            size: self.ts.font_size,
            italic: self.ts.italic,
            underline: self.ts.underline,
            align: self.ts.align,
        }
    }

    fn shape_params(&self) -> ShapeParams {
        ShapeParams {
            kind: self.ts.shape,
            style: self.ts.shape_style,
            width: self.ts.brush_width,
            aa: self.ts.antialias,
            overwrite: self.ts.overwrite,
            dash: self.ts.dash,
            corner_radius: self.ts.corner_radius,
        }
    }

    fn line_params(&self) -> LineParams {
        LineParams {
            width: self.ts.brush_width,
            aa: self.ts.antialias,
            overwrite: self.ts.overwrite,
            dash: self.ts.dash,
            start_cap: self.ts.start_cap,
            end_cap: self.ts.end_cap,
        }
    }

    pub fn settings_sig(&self) -> String {
        format!("{:?}{:?}{:?}", self.ts, self.primary, self.secondary)
    }

    // ------------------------------------------------------------------------------------------
    // Commit / render pending edits

    /// Finalise any in-progress edit, recording history.
    pub fn commit_pending(&mut self) {
        // Make sure the layer reflects the latest state (points/settings) before recording it.
        if self.pending_dirty && !self.pending.is_none() {
            self.render_pending();
        }
        let pending = std::mem::take(&mut self.pending);
        let (smart, sbw, saa) = (self.ts.smart_mode, self.ts.brush_width, self.ts.antialias);
        self.stroke_dirty = None;
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        match pending {
            Pending::None | Pending::Picking | Pending::ZoomRect { .. } => {}
            Pending::Stroke(s) => {
                if s.tool == Tool::SmartClone && smart == crate::smart_clone::SmartMode::Heal && s.layer < doc.layers.len() {
                    let clip = doc.selection.clone();
                    let layer = &mut doc.layers[s.layer];
                    restore(&mut layer.pixmap, &s.base.pixmap);
                    crate::smart_clone::heal(&mut layer.pixmap, clip.as_ref(), &s.points, sbw, saa);
                    doc.composite_dirty = true;
                }
                doc.push_history(s.tool.name(), Snapshot::Layer(s.layer, s.base));
            }
            Pending::Select(s) => {
                doc.push_history(s.tool.name(), Snapshot::Selection(s.before));
            }
            Pending::Shape(s) => doc.push_history("Shapes", Snapshot::Layer(s.layer, s.base)),
            Pending::Line(s) => doc.push_history("Line / Curve", Snapshot::Layer(s.layer, s.base)),
            Pending::Gradient(s) => doc.push_history("Gradient", Snapshot::Layer(s.layer, s.base)),
            Pending::Text(s) => {
                if s.text.is_empty() {
                    if s.layer < doc.layers.len() {
                        doc.layers[s.layer] = s.base;
                        doc.changed();
                    }
                } else {
                    doc.push_history("Text", Snapshot::Layer(s.layer, s.base));
                }
            }
            Pending::MovePixels(m) => {
                if m.changed || m.tx != 0.0 || m.ty != 0.0 || m.angle != 0.0 {
                    doc.push_history(m.name, Snapshot::Full(m.before));
                } else {
                    // Nothing moved: restore exactly.
                    doc.layers[m.layer] = m.before.layers[m.layer].clone();
                    doc.set_selection(m.before.selection);
                    doc.changed();
                }
            }
            Pending::MoveSel(m) => {
                if m.tx != 0.0 || m.ty != 0.0 || m.angle != 0.0 {
                    doc.push_history("Move Selection", Snapshot::Selection(m.before));
                }
            }
        }
        self.pending_dirty = true;
    }

    /// Re-render live previews into the target layer.
    pub fn render_pending(&mut self) {
        let sp = self.shape_params();
        let lp = self.line_params();
        let tp = self.text_params();
        let (primary, secondary) = (self.primary, self.secondary);
        let (bw, aa, ow) = (self.ts.brush_width, self.ts.antialias, self.ts.overwrite);
        let (gk, gt) = (self.ts.gradient, self.ts.grad_transparency);
        let bold = self.ts.bold;
        let smart = self.ts.smart_mode;
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        match &mut self.pending {
            Pending::Stroke(s) => {
                let incremental = s.tool == Tool::CloneStamp || (s.tool == Tool::SmartClone && smart == crate::smart_clone::SmartMode::Blend);
                if !incremental && matches!(s.tool, Tool::Paintbrush | Tool::Eraser | Tool::SmartClone) && s.layer < doc.layers.len() {
                    let clip = doc.selection.as_ref();
                    let layer = &mut doc.layers[s.layer];
                    let (iw, ih) = (layer.pixmap.width(), layer.pixmap.height());
                    if let Some(r) = pts_rect(&s.points, bw / 2.0 + 4.0, iw, ih) {
                        restore_rect(&mut layer.pixmap, &s.base.pixmap, r);
                        self.stroke_dirty = union_rect(self.stroke_dirty, r);
                    }
                    if s.tool == Tool::SmartClone {
                        // preview of the area that will be healed
                        paint::draw_brush_stroke(&mut layer.pixmap, clip, &s.points, bw, aa, false, BrushKind::Paint(Col::rgba(255, 60, 60, 110)));
                    } else {
                        let kind = if s.tool == Tool::Eraser { BrushKind::Erase } else { BrushKind::Paint(s.color) };
                        paint::draw_brush_stroke(&mut layer.pixmap, clip, &s.points, bw, aa, ow && s.tool == Tool::Paintbrush, kind);
                    }
                }
                doc.composite_dirty = true;
            }            Pending::Shape(s) => {
                let clip = doc.selection.clone();
                let layer = &mut doc.layers[s.layer];
                restore(&mut layer.pixmap, &s.base.pixmap);
                let (o, f) = if s.swap { (secondary, primary) } else { (primary, secondary) };
                paint::draw_shape(&mut layer.pixmap, clip.as_ref(), s.a, s.b, &sp, o, f);
                doc.composite_dirty = true;
            }
            Pending::Line(l) => {
                let clip = doc.selection.clone();
                let layer = &mut doc.layers[l.layer];
                restore(&mut layer.pixmap, &l.base.pixmap);
                let c = if l.swap { secondary } else { primary };
                paint::draw_line(&mut layer.pixmap, clip.as_ref(), l.p0, l.p1, l.c, &lp, c);
                doc.composite_dirty = true;
            }
            Pending::Gradient(g) => {
                let clip = doc.selection.clone();
                let layer = &mut doc.layers[g.layer];
                let (mut c0, mut c1) = (primary, secondary);
                if gt {
                    c0 = Col::rgba(0, 0, 0, 255);
                    c1 = Col::rgba(0, 0, 0, 0);
                }
                if g.swap {
                    std::mem::swap(&mut c0, &mut c1);
                }
                paint::render_gradient(&g.base.pixmap, &mut layer.pixmap, clip.as_ref(), gk, g.s, g.e, c0, c1, gt);
                doc.composite_dirty = true;
            }
            Pending::Text(t) => {
                let clip = doc.selection.clone();
                let layer = &mut doc.layers[t.layer];
                restore(&mut layer.pixmap, &t.base.pixmap);
                let lay = text::layout(&t.text, (t.pos.x, t.pos.y), &tp);
                paint::draw_text(&mut layer.pixmap, clip.as_ref(), &lay, primary, bold, tp.size, aa);
                doc.composite_dirty = true;
            }
            Pending::MovePixels(m) => {
                let t = move_transform(m.tx, m.ty, m.angle, m.pivot);
                let smooth = m.angle != 0.0;
                let layer = &mut doc.layers[m.layer];
                restore(&mut layer.pixmap, &m.hole);
                let q = if smooth { FilterQuality::Bilinear } else { FilterQuality::Nearest };
                layer.pixmap.draw_pixmap(0, 0, m.floating.as_ref(), &PixmapPaint { quality: q, ..Default::default() }, t, None);
                doc.set_selection(Some(transformed_mask(&m.mask_px, t, smooth)));
                doc.composite_dirty = true;
            }
            Pending::MoveSel(m) => {
                let t = move_transform(m.tx, m.ty, m.angle, m.pivot);
                doc.set_selection(Some(transformed_mask(&m.mask_px, t, m.angle != 0.0)));
            }
            _ => {}
        }
        self.pending_dirty = false;
    }

    // ------------------------------------------------------------------------------------------
    // View helpers

    pub fn to_screen(&self, rect: Rect, p: Pos2) -> Pos2 {
        let v = &self.docs[self.cur].view;
        rect.center() + (p.to_vec2() - v.center) * v.zoom
    }

    pub fn to_image(&self, rect: Rect, s: Pos2) -> Pos2 {
        let v = &self.docs[self.cur].view;
        (v.center + (s - rect.center()) / v.zoom).to_pos2()
    }

    pub fn set_zoom(&mut self, z: f32, anchor: Option<Pos2>) {
        let rect = self.canvas_rect;
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        let z = z.clamp(ZOOMS[0], *ZOOMS.last().unwrap());
        let anchor = anchor.unwrap_or(rect.center());
        let img = doc.view.center + (anchor - rect.center()) / doc.view.zoom;
        doc.view.zoom = z;
        doc.view.center = img - (anchor - rect.center()) / z;
        Self::clamp_center(doc);
    }

    pub fn clamp_center(doc: &mut Document) {
        doc.view.center.x = doc.view.center.x.clamp(0.0, doc.width as f32);
        doc.view.center.y = doc.view.center.y.clamp(0.0, doc.height as f32);
    }

    pub fn zoom_to_window(&mut self) {
        let rect = self.canvas_rect;
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        let z = ((rect.width() - 24.0) / doc.width as f32).min((rect.height() - 24.0) / doc.height as f32);
        doc.view.zoom = z.clamp(ZOOMS[0], 1.0f32.max(z.min(ZOOMS[ZOOMS.len() - 1])));
        if z >= 1.0 {
            doc.view.zoom = z.min(*ZOOMS.last().unwrap());
        }
        doc.view.center = vec2(doc.width as f32 / 2.0, doc.height as f32 / 2.0);
    }

    pub fn zoom_to_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let rect = self.canvas_rect;
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        let z = ((rect.width() - 24.0) / w).min((rect.height() - 24.0) / h);
        doc.view.zoom = z.clamp(ZOOMS[0], *ZOOMS.last().unwrap());
        doc.view.center = vec2((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Self::clamp_center(doc);
    }

    fn sel_mode(&self, right: bool, m: Modifiers) -> SelMode {
        match (right, m.ctrl, m.alt) {
            (false, true, _) => SelMode::Union,
            (false, false, true) => SelMode::Exclude,
            (true, true, _) => SelMode::Xor,
            (true, false, true) => SelMode::Intersect,
            _ => self.ts.sel_mode,
        }
    }

    fn sample_source(&self, image: bool) -> Option<Pixmap> {
        let doc = self.docs.get(self.cur)?;
        Some(if image { doc.composite() } else { doc.layers[doc.active].pixmap.clone() })
    }

    fn pick_color(&mut self, p: Pos2, right: bool, ctrl: bool) {
        let image = self.ts.picker_image || ctrl;
        let n = self.ts.picker_sample.max(1) as i32;
        let Some(src) = (if image {
            self.docs.get(self.cur).map(|d| d.composite())
        } else {
            self.docs.get(self.cur).map(|d| d.layers[d.active].pixmap.clone())
        }) else {
            return;
        };
        let (w, h) = (src.width() as i32, src.height() as i32);
        let (cx, cy) = (p.x.floor() as i32, p.y.floor() as i32);
        if cx < 0 || cy < 0 || cx >= w || cy >= h {
            return;
        }
        let r = (n - 1) / 2;
        let mut acc = [0u32; 4];
        let mut cnt = 0;
        for y in (cy - r).max(0)..=(cy + r).min(h - 1) {
            for x in (cx - r).max(0)..=(cx + r).min(w - 1) {
                let i = ((y * w + x) * 4) as usize;
                for k in 0..4 {
                    acc[k] += src.data()[i + k] as u32;
                }
                cnt += 1;
            }
        }
        let avg = [(acc[0] / cnt) as u8, (acc[1] / cnt) as u8, (acc[2] / cnt) as u8, (acc[3] / cnt) as u8];
        let c = Col::from_premul(avg);
        if right {
            self.secondary = c;
        } else {
            self.primary = c;
        }
    }

    pub fn lift_selection(&mut self, copy: bool) -> Option<MoveEdit> {
        let doc = self.docs.get_mut(self.cur)?;
        let (w, h) = (doc.width, doc.height);
        let before = doc.snapshot_full();
        let mask = doc.selection.clone().unwrap_or_else(|| raster::full_mask(w, h));
        let li = doc.active;
        let src = doc.layers[li].pixmap.clone();
        let mut floating = raster::new_pixmap(w, h);
        floating.draw_pixmap(0, 0, src.as_ref(), &PixmapPaint::default(), Transform::identity(), Some(&mask));
        let mut hole = src;
        if !copy {
            let mut p = Paint::default();
            p.set_color(tiny_skia::Color::BLACK);
            p.blend_mode = tiny_skia::BlendMode::DestinationOut;
            let r = tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32).unwrap();
            hole.fill_rect(r, &p, Transform::identity(), Some(&mask));
        }
        let (x0, y0, x1, y1) = raster::mask_bounds(&mask).unwrap_or((0, 0, w, h));
        let pivot = pos2((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
        let mask_px = raster::mask_to_pixmap(&mask);
        if doc.selection.is_none() {
            doc.set_selection(Some(mask));
        }
        Some(MoveEdit {
            name: if copy { "Move Selected Pixels (copy)" } else { "Move Selected Pixels" },
            before,
            layer: li,
            hole,
            floating,
            mask_px,
            tx: 0.0,
            ty: 0.0,
            angle: 0.0,
            pivot,
            drag: None,
            changed: copy,
        })
    }

    /// Start a floating paste of `img` at image position (ox, oy).
    pub fn begin_paste(&mut self, img: Pixmap, ox: i32, oy: i32, name: &'static str) {
        self.commit_pending();
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        let (w, h) = (doc.width, doc.height);
        let before = doc.snapshot_full();
        let mut floating = raster::new_pixmap(w, h);
        floating.draw_pixmap(ox, oy, img.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        let mut mask_px = raster::new_pixmap(w, h);
        if let Some(r) = tiny_skia::Rect::from_xywh(ox as f32, oy as f32, img.width() as f32, img.height() as f32) {
            let mut p = Paint::default();
            p.set_color(tiny_skia::Color::BLACK);
            mask_px.fill_rect(r, &p, Transform::identity(), None);
        }
        let li = doc.active;
        let hole = doc.layers[li].pixmap.clone();
        let pivot = pos2(ox as f32 + img.width() as f32 / 2.0, oy as f32 + img.height() as f32 / 2.0);
        self.pending = Pending::MovePixels(MoveEdit {
            name,
            before,
            layer: li,
            hole,
            floating,
            mask_px,
            tx: 0.0,
            ty: 0.0,
            angle: 0.0,
            pivot,
            drag: None,
            changed: true,
        });
        self.set_tool(Tool::MovePixels);
        self.pending_dirty = true;
    }

    // ------------------------------------------------------------------------------------------
    // Mouse handlers

    pub(crate) fn tool_press(&mut self, btn: PointerButton, p: Pos2, m: Modifiers) {
        let p = self.snap_pt(p);
        let right = btn == PointerButton::Secondary;
        let zoom = self.docs[self.cur].view.zoom;
        let tol = 7.0 / zoom;
        self.pending_dirty = true;
        match self.tool {
            Tool::RectSelect | Tool::EllipseSelect | Tool::LassoSelect => {
                self.commit_pending();
                let mode = self.sel_mode(right, m);
                let doc = &mut self.docs[self.cur];
                self.pending = Pending::Select(SelectEdit {
                    tool: self.tool,
                    start: p,
                    cur: p,
                    points: vec![p],
                    mode,
                    before: doc.selection.clone(),
                    square: m.shift,
                });
            }
            Tool::MagicWand => {
                self.commit_pending();
                let mode = self.sel_mode(right, m);
                let global = self.ts.flood_global || m.shift;
                let Some(src) = self.sample_source(self.ts.sample_image) else { return };
                let doc = &mut self.docs[self.cur];
                if p.x < 0.0 || p.y < 0.0 || p.x >= doc.width as f32 || p.y >= doc.height as f32 {
                    return;
                }
                let mask = raster::flood_mask(&src, p.x as u32, p.y as u32, self.ts.tolerance, global);
                let before = doc.selection.clone();
                let new = raster::combine_masks(before.as_ref(), &mask, mode);
                doc.set_selection(Some(new));
                doc.push_history("Magic Wand", Snapshot::Selection(before));
            }
            Tool::MovePixels => {
                let copy = m.ctrl;
                let lifted = matches!(self.pending, Pending::MovePixels(_));
                if !lifted || copy {
                    self.commit_pending();
                    match self.lift_selection(copy) {
                        Some(me) => self.pending = Pending::MovePixels(me),
                        None => return,
                    }
                }
                if let Pending::MovePixels(me) = &mut self.pending {
                    me.drag = Some(MoveDrag { button: btn, start: p, tx: me.tx, ty: me.ty, angle: me.angle });
                }
            }
            Tool::MoveSelection => {
                if !matches!(self.pending, Pending::MoveSel(_)) {
                    self.commit_pending();
                    let doc = &mut self.docs[self.cur];
                    let Some(mask) = doc.selection.clone() else { return };
                    let (w, h) = (doc.width, doc.height);
                    let (x0, y0, x1, y1) = raster::mask_bounds(&mask).unwrap_or((0, 0, w, h));
                    self.pending = Pending::MoveSel(SelMoveEdit {
                        before: Some(mask.clone()),
                        mask_px: raster::mask_to_pixmap(&mask),
                        tx: 0.0,
                        ty: 0.0,
                        angle: 0.0,
                        pivot: pos2((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0),
                        drag: None,
                    });
                }
                if let Pending::MoveSel(me) = &mut self.pending {
                    me.drag = Some(MoveDrag { button: btn, start: p, tx: me.tx, ty: me.ty, angle: me.angle });
                }
            }
            Tool::Zoom => {
                self.pending = Pending::ZoomRect { start: p, cur: p, button: btn };
            }
            Tool::Pan => {}
            Tool::PaintBucket => {
                self.commit_pending();
                let global = self.ts.flood_global || m.shift;
                let Some(src) = self.sample_source(self.ts.sample_image || m.ctrl) else { return };
                let color = if right { self.secondary } else { self.primary };
                let tol_v = self.ts.tolerance;
                let (aa, ow) = (self.ts.antialias, self.ts.overwrite);
                let doc = &mut self.docs[self.cur];
                if p.x < 0.0 || p.y < 0.0 || p.x >= doc.width as f32 || p.y >= doc.height as f32 {
                    return;
                }
                let mut mask = raster::flood_mask(&src, p.x as u32, p.y as u32, tol_v, global);
                if aa {
                    // soften the fill edge by one pixel for anti-aliased fills
                    let soft = raster::gaussian_blur(&raster::mask_to_pixmap(&mask), 1);
                    let sm = Mask::from_pixmap(soft.as_ref(), MaskType::Alpha);
                    for (d, s) in mask.data_mut().iter_mut().zip(sm.data()) {
                        *d = (*d).max(*s);
                    }
                }
                if let Some(sel) = &doc.selection {
                    for (d, s) in mask.data_mut().iter_mut().zip(sel.data()) {
                        *d = raster::mul8(*d as u32, *s as u32);
                    }
                }
                let before = doc.snapshot_layer(doc.active);
                let mut paint = Paint::default();
                paint.set_color(color.skia());
                if ow {
                    paint.blend_mode = tiny_skia::BlendMode::Source;
                }
                let r = tiny_skia::Rect::from_xywh(0.0, 0.0, doc.width as f32, doc.height as f32).unwrap();
                let li = doc.active;
                doc.layers[li].pixmap.fill_rect(r, &paint, Transform::identity(), Some(&mask));
                doc.push_history("Paint Bucket", before);
            }
            Tool::Gradient => {
                if let Pending::Gradient(g) = &mut self.pending {
                    for (i, h) in [g.s, g.e].iter().enumerate() {
                        if (*h - p).length() < tol {
                            if right {
                                g.swap = !g.swap;
                            } else {
                                g.drag = Some(GradDrag::Handle(i));
                            }
                            return;
                        }
                    }
                    let mid = pos2((g.s.x + g.e.x) / 2.0, (g.s.y + g.e.y) / 2.0);
                    if (mid - p).length() < tol {
                        g.drag = Some(GradDrag::Move { start: p, s0: g.s, s1: g.e });
                        return;
                    }
                }
                self.commit_pending();
                let doc = &mut self.docs[self.cur];
                let li = doc.active;
                self.pending = Pending::Gradient(GradEdit {
                    base: doc.layers[li].clone(),
                    layer: li,
                    s: p,
                    e: p,
                    swap: right,
                    drag: Some(GradDrag::Create),
                });
            }
            Tool::Paintbrush | Tool::Eraser | Tool::Pencil | Tool::CloneStamp | Tool::SmartClone | Tool::Recolor => {
                self.commit_pending();
                let mut offset = Vec2::ZERO;
                if self.tool == Tool::CloneStamp || (self.tool == Tool::SmartClone && self.ts.smart_mode == crate::smart_clone::SmartMode::Blend) {
                    if m.ctrl {
                        self.clone_source = Some(p);
                        self.status_msg = format!("Clone source set at {}, {}", p.x as i32, p.y as i32);
                        return;
                    }
                    let Some(src) = self.clone_source else {
                        self.status_msg = "Hold Ctrl and click to set the clone source first.".into();
                        return;
                    };
                    offset = vec2((p.x - src.x).round(), (p.y - src.y).round());
                }
                let (color, other) = if right { (self.secondary, self.primary) } else { (self.primary, self.secondary) };
                let (ow, tolv, bw) = (self.ts.overwrite, self.ts.tolerance, self.ts.brush_width);
                let tool = self.tool;
                let doc = &mut self.docs[self.cur];
                let li = doc.active;
                let base = doc.layers[li].clone();
                let clip = doc.selection.clone();
                if tool == Tool::CloneStamp {
                    let aa = self.ts.antialias;
                    paint::draw_brush_stroke(&mut doc.layers[li].pixmap, clip.as_ref(), &[p], bw, aa, false, BrushKind::Clone { source: &base.pixmap, offset });
                    doc.composite_dirty = true;
                } else if tool == Tool::SmartClone && self.ts.smart_mode == crate::smart_clone::SmartMode::Blend {
                    crate::smart_clone::blend_segment(&mut doc.layers[li].pixmap, &base.pixmap, clip.as_ref(), p, p, bw, offset);
                    doc.composite_dirty = true;
                } else if tool == Tool::Pencil {
                    paint::pencil_line(&mut doc.layers[li].pixmap, clip.as_ref(), p, p, color, ow);
                    doc.composite_dirty = true;
                } else if tool == Tool::Recolor {
                    paint::recolor_stamp(&mut doc.layers[li].pixmap, clip.as_ref(), p, bw / 2.0, other, color, tolv);
                    doc.composite_dirty = true;
                }
                self.pending = Pending::Stroke(StrokeEdit { tool, base, layer: li, points: vec![p], color, other, offset });
            }
            Tool::ColorPicker => {
                self.pick_color(p, right, m.ctrl);
                self.pending = Pending::Picking;
            }
            Tool::Text => {
                let tp = self.text_params();
                if let Pending::Text(t) = &mut self.pending {
                    let lay = text::layout(&t.text, (t.pos.x, t.pos.y), &tp);
                    let (x0, y0, x1, y1) = lay.bounds;
                    if p.x >= x0 - tol && p.x <= x1 + tol && p.y >= y0 - tol && p.y <= y1 + tol {
                        t.drag = Some((p, t.pos));
                        return;
                    }
                }
                self.commit_pending();
                let doc = &mut self.docs[self.cur];
                let li = doc.active;
                self.pending = Pending::Text(TextEdit {
                    base: doc.layers[li].clone(),
                    layer: li,
                    pos: p,
                    text: String::new(),
                    cursor: 0,
                    drag: None,
                });
            }
            Tool::LineCurve => {
                if let Pending::Line(l) = &mut self.pending {
                    for (i, h) in [l.p0, l.p1, l.c].iter().enumerate() {
                        if (*h - p).length() < tol {
                            l.drag = Some(LineDrag::Handle(i));
                            return;
                        }
                    }
                    // near the curve: move it
                    let q = paint::curve_control(l.p0, l.p1, l.c);
                    let near = (0..=32).any(|i| {
                        let t = i as f32 / 32.0;
                        let a = l.p0.to_vec2() * (1.0 - t) * (1.0 - t) + q.to_vec2() * 2.0 * t * (1.0 - t) + l.p1.to_vec2() * t * t;
                        (a - p.to_vec2()).length() < tol.max(self.ts.brush_width / 2.0)
                    });
                    if near && !right {
                        l.drag = Some(LineDrag::Move { start: p, s0: l.p0, s1: l.p1, sc: l.c });
                        return;
                    }
                }
                self.commit_pending();
                let doc = &mut self.docs[self.cur];
                let li = doc.active;
                self.pending = Pending::Line(LineEdit {
                    base: doc.layers[li].clone(),
                    layer: li,
                    p0: p,
                    p1: p,
                    c: p,
                    bent: false,
                    swap: right,
                    drag: Some(LineDrag::Create { origin: p }),
                });
            }
            Tool::Shapes => {
                if let Pending::Shape(s) = &mut self.pending {
                    for (i, h) in shape_handles(s.a, s.b).iter().enumerate() {
                        if (*h - p).length() < tol {
                            s.drag = Some(ShapeDrag::Handle { idx: i, sa: s.a, sb: s.b, start: p });
                            return;
                        }
                    }
                    let (x0, y0, x1, y1) = paint::norm_rect(s.a, s.b);
                    if !right && p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1 {
                        s.drag = Some(ShapeDrag::Move { start: p, sa: s.a, sb: s.b });
                        return;
                    }
                }
                self.commit_pending();
                let doc = &mut self.docs[self.cur];
                let li = doc.active;
                self.pending = Pending::Shape(ShapeEdit {
                    base: doc.layers[li].clone(),
                    layer: li,
                    a: p,
                    b: p,
                    swap: right,
                    drag: Some(ShapeDrag::Create { origin: p }),
                });
            }
        }
    }

    pub(crate) fn tool_drag(&mut self, p: Pos2, m: Modifiers) {
        let p = self.snap_pt(p);
        self.pending_dirty = true;
        let from_center = self.ts.from_center;
        let bw = self.ts.brush_width;
        let ow = self.ts.overwrite;
        let tolv = self.ts.tolerance;
        let aa = self.ts.antialias;
        let smart_blend = self.ts.smart_mode == crate::smart_clone::SmartMode::Blend;
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        match &mut self.pending {
            Pending::Select(s) => {
                s.cur = p;
                s.square = m.shift;
                if s.tool == Tool::LassoSelect {
                    if s.points.last().map(|l| (*l - p).length() >= 0.5).unwrap_or(true) {
                        s.points.push(p);
                    }
                }
                let (w, h) = (doc.width, doc.height);
                if let Some(mask) = selection_shape_mask(s, w, h) {
                    let combined = raster::combine_masks(s.before.as_ref(), &mask, s.mode);
                    doc.set_selection(Some(combined));
                }
            }
            Pending::Stroke(s) => {
                let last = *s.points.last().unwrap();
                if (last - p).length() < 0.3 {
                    return;
                }
                let clip = doc.selection.clone();
                match s.tool {
                    Tool::CloneStamp | Tool::SmartClone if s.tool == Tool::CloneStamp || smart_blend => {
                        let layer = &mut doc.layers[s.layer];
                        let (iw, ih) = (layer.pixmap.width(), layer.pixmap.height());
                        if s.tool == Tool::CloneStamp {
                            paint::draw_brush_stroke(&mut layer.pixmap, clip.as_ref(), &[last, p], bw, aa, false, BrushKind::Clone { source: &s.base.pixmap, offset: s.offset });
                        } else {
                            crate::smart_clone::blend_segment(&mut layer.pixmap, &s.base.pixmap, clip.as_ref(), last, p, bw, s.offset);
                        }
                        if let Some(r) = pts_rect(&[last, p], bw / 2.0 + 4.0, iw, ih) {
                            self.stroke_dirty = union_rect(self.stroke_dirty, r);
                        }
                        doc.composite_dirty = true;
                    }
                    Tool::Pencil => {
                        paint::pencil_line(&mut doc.layers[s.layer].pixmap, clip.as_ref(), last, p, s.color, ow);
                        doc.composite_dirty = true;
                    }
                    Tool::Recolor => {
                        let r = bw / 2.0;
                        let dist = (p - last).length();
                        let steps = ((dist / (r / 3.0).max(0.5)).ceil() as usize).max(1);
                        for i in 1..=steps {
                            let q = last + (p - last) * (i as f32 / steps as f32);
                            paint::recolor_stamp(&mut doc.layers[s.layer].pixmap, clip.as_ref(), q, r, s.other, s.color, tolv);
                        }
                        doc.composite_dirty = true;
                    }
                    _ => {}
                }
                s.points.push(p);
            }
            Pending::MovePixels(me) => {
                if let Some(d) = me.drag {
                    if d.button == PointerButton::Secondary {
                        let pv = me.pivot + vec2(me.tx, me.ty);
                        let a0 = (d.start - pv).angle();
                        let a1 = (p - pv).angle();
                        let mut ang = d.angle + (a1 - a0).to_degrees();
                        if m.shift {
                            ang = (ang / 15.0).round() * 15.0;
                        }
                        me.angle = ang;
                    } else {
                        me.tx = d.tx + (p.x - d.start.x).round();
                        me.ty = d.ty + (p.y - d.start.y).round();
                    }
                }
            }
            Pending::MoveSel(me) => {
                if let Some(d) = me.drag {
                    if d.button == PointerButton::Secondary {
                        let pv = me.pivot + vec2(me.tx, me.ty);
                        let a0 = (d.start - pv).angle();
                        let a1 = (p - pv).angle();
                        let mut ang = d.angle + (a1 - a0).to_degrees();
                        if m.shift {
                            ang = (ang / 15.0).round() * 15.0;
                        }
                        me.angle = ang;
                    } else {
                        me.tx = d.tx + (p.x - d.start.x).round();
                        me.ty = d.ty + (p.y - d.start.y).round();
                    }
                }
            }
            Pending::ZoomRect { cur, .. } => *cur = p,
            Pending::Picking => {}
            Pending::Gradient(g) => match g.drag {
                Some(GradDrag::Create) | Some(GradDrag::Handle(1)) => {
                    g.e = if m.shift { paint::snap_angle(g.s, p, 15.0) } else { p };
                }
                Some(GradDrag::Handle(_)) => {
                    g.s = if m.shift { paint::snap_angle(g.e, p, 15.0) } else { p };
                }
                Some(GradDrag::Move { start, s0, s1 }) => {
                    let d = p - start;
                    g.s = s0 + d;
                    g.e = s1 + d;
                }
                None => {}
            },
            Pending::Shape(s) => match s.drag {
                Some(ShapeDrag::Create { origin }) => {
                    let centre = from_center ^ m.alt;
                    let mut d = p - origin;
                    if m.shift {
                        let mm = d.x.abs().max(d.y.abs());
                        d = vec2(mm * sgn(d.x), mm * sgn(d.y));
                    }
                    if centre {
                        s.a = origin - d;
                        s.b = origin + d;
                    } else {
                        s.a = origin;
                        s.b = origin + d;
                    }
                }
                Some(ShapeDrag::Handle { idx, sa, sb, start }) => {
                    let centre = from_center ^ m.alt;
                    let (a, b) = resize_rect(sa, sb, idx, p - start, centre, m.shift);
                    s.a = a;
                    s.b = b;
                }
                Some(ShapeDrag::Move { start, sa, sb }) => {
                    let d = p - start;
                    s.a = sa + d;
                    s.b = sb + d;
                }
                None => {}
            },
            Pending::Line(l) => match l.drag {
                Some(LineDrag::Create { origin }) => {
                    let centre = from_center ^ m.alt;
                    let tip = if m.shift { paint::snap_angle(origin, p, 15.0) } else { p };
                    l.p1 = tip;
                    l.p0 = if centre { origin - (tip - origin) } else { origin };
                    l.c = pos2((l.p0.x + l.p1.x) / 2.0, (l.p0.y + l.p1.y) / 2.0);
                }
                Some(LineDrag::Handle(i)) => {
                    match i {
                        0 => l.p0 = if m.shift { paint::snap_angle(l.p1, p, 15.0) } else { p },
                        1 => l.p1 = if m.shift { paint::snap_angle(l.p0, p, 15.0) } else { p },
                        _ => {
                            l.c = p;
                            l.bent = true;
                        }
                    }
                    if !l.bent {
                        l.c = pos2((l.p0.x + l.p1.x) / 2.0, (l.p0.y + l.p1.y) / 2.0);
                    }
                }
                Some(LineDrag::Move { start, s0, s1, sc }) => {
                    let d = p - start;
                    l.p0 = s0 + d;
                    l.p1 = s1 + d;
                    l.c = sc + d;
                }
                None => {}
            },
            Pending::Text(t) => {
                if let Some((start, spos)) = t.drag {
                    t.pos = spos + (p - start);
                }
            }
            Pending::None => {}
        }
        if matches!(self.pending, Pending::Picking) {
            self.pick_color(p, self.drag.map(|d| d.button == PointerButton::Secondary).unwrap_or(false), m.ctrl);
        }
    }

    pub(crate) fn tool_release(&mut self, p: Pos2, _m: Modifiers) {
        let p = self.snap_pt(p);
        self.pending_dirty = true;
        let zoom = self.docs[self.cur].view.zoom;
        match &mut self.pending {
            Pending::Stroke(_) => self.commit_pending(),
            Pending::Select(s) => {
                let click = (s.cur - s.start).length() * zoom < 2.0 && s.tool != Tool::LassoSelect
                    || (s.tool == Tool::LassoSelect && s.points.len() < 3);
                if click {
                    let before = s.before.clone();
                    let mode = s.mode;
                    self.pending = Pending::None;
                    let doc = &mut self.docs[self.cur];
                    if mode == SelMode::Replace {
                        doc.set_selection(None);
                        if before.is_some() {
                            doc.push_history("Deselect All", Snapshot::Selection(before));
                        }
                    } else {
                        doc.set_selection(before);
                    }
                } else {
                    self.commit_pending();
                }
            }
            Pending::MovePixels(me) => me.drag = None,
            Pending::MoveSel(me) => me.drag = None,
            Pending::ZoomRect { start, cur, button } => {
                let (start, cur, button) = (*start, *cur, *button);
                self.pending = Pending::None;
                let screen = self.drag.map(|d| d.last_screen);
                if (cur - start).length() * zoom < 4.0 {
                    let z = zoom_step(zoom, if button == PointerButton::Secondary { -1 } else { 1 });
                    self.set_zoom(z, screen);
                } else {
                    let (x0, y0, x1, y1) = paint::norm_rect(start, cur);
                    self.zoom_to_rect(x0, y0, x1, y1);
                }
            }
            Pending::Picking => {
                self.pending = Pending::None;
                if self.ts.picker_switch_back {
                    let t = self.prev_tool;
                    self.set_tool(t);
                }
            }
            Pending::Gradient(g) => {
                let created = matches!(g.drag, Some(GradDrag::Create));
                g.drag = None;
                if created && (g.e - g.s).length() < 0.5 {
                    self.cancel_pending();
                }
            }
            Pending::Shape(s) => {
                let created = matches!(s.drag, Some(ShapeDrag::Create { .. }));
                s.drag = None;
                if created && ((s.a.x - s.b.x).abs() < 0.5 || (s.a.y - s.b.y).abs() < 0.5) {
                    self.cancel_pending();
                }
            }
            Pending::Line(l) => {
                let created = matches!(l.drag, Some(LineDrag::Create { .. }));
                l.drag = None;
                if created && (l.p1 - l.p0).length() < 0.5 {
                    self.cancel_pending();
                }
            }
            Pending::Text(t) => t.drag = None,
            Pending::None => {}
        }
        let _ = p;
    }

    /// Abandon an editable pending operation, restoring its base layer.
    pub fn cancel_pending(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        match pending {
            Pending::Shape(ShapeEdit { base, layer, .. })
            | Pending::Line(LineEdit { base, layer, .. })
            | Pending::Gradient(GradEdit { base, layer, .. })
            | Pending::Text(TextEdit { base, layer, .. })
            | Pending::Stroke(StrokeEdit { base, layer, .. }) => {
                if layer < doc.layers.len() {
                    doc.layers[layer] = base;
                }
            }
            Pending::MovePixels(m) => {
                let mut st = m.before;
                doc.layers = std::mem::take(&mut st.layers);
                doc.set_selection(st.selection);
            }
            Pending::MoveSel(m) => doc.set_selection(m.before),
            Pending::Select(s) => doc.set_selection(s.before),
            _ => {}
        }
        doc.changed();
        self.pending_dirty = true;
    }

    /// Arrow-key nudging of editable objects. Returns true if consumed.
    pub fn nudge(&mut self, d: Vec2) -> bool {
        self.pending_dirty = true;
        match &mut self.pending {
            Pending::Shape(s) => {
                s.a += d;
                s.b += d;
            }
            Pending::Line(l) => {
                l.p0 += d;
                l.p1 += d;
                l.c += d;
            }
            Pending::Gradient(g) => {
                g.s += d;
                g.e += d;
            }
            Pending::Text(t) => t.pos += d,
            Pending::MovePixels(m) => {
                m.tx += d.x;
                m.ty += d.y;
            }
            Pending::MoveSel(m) => {
                m.tx += d.x;
                m.ty += d.y;
            }
            _ => {
                if matches!(self.tool, Tool::MovePixels | Tool::MoveSelection) && self.docs.get(self.cur).map(|d| d.has_selection()).unwrap_or(false) {
                    if self.tool == Tool::MovePixels {
                        if let Some(me) = self.lift_selection(false) {
                            self.pending = Pending::MovePixels(me);
                        }
                    } else {
                        let doc = &self.docs[self.cur];
                        let mask = doc.selection.clone().unwrap();
                        let (x0, y0, x1, y1) = raster::mask_bounds(&mask).unwrap_or((0, 0, doc.width, doc.height));
                        self.pending = Pending::MoveSel(SelMoveEdit {
                            before: Some(mask.clone()),
                            mask_px: raster::mask_to_pixmap(&mask),
                            tx: 0.0,
                            ty: 0.0,
                            angle: 0.0,
                            pivot: pos2((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0),
                            drag: None,
                        });
                    }
                    return self.nudge(d);
                }
                return false;
            }
        }
        true
    }

    // ------------------------------------------------------------------------------------------
    // Texture management

    pub fn ensure_texture(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        if !doc.composite_dirty && doc.texture.is_some() {
            return;
        }
        let opts = egui::TextureOptions { magnification: egui::TextureFilter::Nearest, ..egui::TextureOptions::LINEAR };
        // brush strokes only touch a small region: update just that part of the texture
        if let (Some(r), true, true, true) = (self.stroke_dirty.take(), self.drag.is_some(), matches!(self.pending, Pending::Stroke(_)), doc.texture.is_some()) {
            let r = [r[0].clamp(0, doc.width as i32), r[1].clamp(0, doc.height as i32), r[2].clamp(0, doc.width as i32), r[3].clamp(0, doc.height as i32)];
            if r[2] > r[0] && r[3] > r[1] {
                let comp = doc.composite_rect(r);
                let img = egui::ColorImage::from_rgba_premultiplied([(r[2] - r[0]) as usize, (r[3] - r[1]) as usize], comp.data());
                if let Some(t) = doc.texture.as_mut() {
                    t.set_partial([r[0] as usize, r[1] as usize], img, opts);
                }
            }
            doc.composite_dirty = false;
            return;
        }
        self.stroke_dirty = None;
        let comp = doc.composite();
        let img = egui::ColorImage::from_rgba_premultiplied([doc.width as usize, doc.height as usize], comp.data());
        match &mut doc.texture {
            Some(t) => t.set(img, opts),
            None => doc.texture = Some(ctx.load_texture(format!("doc{}", doc.uid), img, opts)),
        }
        doc.composite_dirty = false;
    }

    pub fn update_thumbnails(&mut self, ctx: &egui::Context) {
        if self.drag.is_some() {
            return;
        }
        for doc in &mut self.docs {
            if !doc.thumbs_dirty {
                continue;
            }
            doc.thumbs_dirty = false;
            let make = |p: &Pixmap, size: f32| -> egui::ColorImage {
                let s = (size / p.width() as f32).min(size / p.height() as f32).min(1.0);
                let (tw, th) = (((p.width() as f32 * s).round() as u32).max(1), ((p.height() as f32 * s).round() as u32).max(1));
                let mut t = raster::new_pixmap(tw, th);
                t.draw_pixmap(
                    0,
                    0,
                    p.as_ref(),
                    &PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() },
                    Transform::from_scale(tw as f32 / p.width() as f32, th as f32 / p.height() as f32),
                    None,
                );
                egui::ColorImage::from_rgba_premultiplied([tw as usize, th as usize], t.data())
            };
            doc.layer_thumbs = doc
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| ctx.load_texture(format!("lt{}_{}", doc.uid, i), make(&l.pixmap, 40.0), egui::TextureOptions::LINEAR))
                .collect();
            let comp = doc.composite();
            doc.doc_thumb = Some(ctx.load_texture(format!("dt{}", doc.uid), make(&comp, 44.0), egui::TextureOptions::LINEAR));
        }
    }

    pub fn checker_texture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        if self.checker.is_none() {
            let a = Color32::from_gray(255);
            let b = Color32::from_gray(204);
            let img = egui::ColorImage { size: [2, 2], pixels: vec![a, b, b, a] };
            self.checker = Some(ctx.load_texture("checker", img, egui::TextureOptions::NEAREST_REPEAT));
        }
        self.checker.as_ref().unwrap().id()
    }

    // ------------------------------------------------------------------------------------------
    // The canvas widget

    pub fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        self.canvas_rect = rect;
        let painter = ui.painter_at(rect);
        let pal = crate::theme::ui_pal(ui);
        painter.rect_filled(rect, 0.0, pal.canvas);
        if self.docs.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No image open. Use File → New (Ctrl+N) or File → Open (Ctrl+O),\nor drop an image file here.",
                egui::FontId::proportional(16.0),
                pal.weak,
            );
            return;
        }
        if self.docs[self.cur].view.needs_fit {
            self.docs[self.cur].view.needs_fit = false;
            let doc = &self.docs[self.cur];
            let fits = doc.width as f32 <= rect.width() - 24.0 && doc.height as f32 <= rect.height() - 24.0;
            if fits {
                let d = &mut self.docs[self.cur];
                d.view.zoom = 1.0;
                d.view.center = vec2(d.width as f32 / 2.0, d.height as f32 / 2.0);
            } else {
                self.zoom_to_window();
            }
        }
        if self.dialog.is_none() {
            self.handle_canvas_input(ui, &resp, rect);
        }
        let sig = self.settings_sig();
        if sig != self.last_sig {
            self.last_sig = sig;
            if self.pending.is_editable() {
                self.pending_dirty = true;
            }
        }
        if self.pending_dirty {
            self.render_pending();
        }
        self.ensure_texture(ui.ctx());
        self.draw_canvas(ui, &painter, rect, resp.hovered());
    }

    fn handle_canvas_input(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect) {
        let (hover, mods, moves, pressed, down, scroll, zoomd, latest, space) = ui.input(|i| {
            let moves: Vec<Pos2> = i
                .events
                .iter()
                .filter_map(|e| if let egui::Event::PointerMoved(p) = e { Some(*p) } else { None })
                .collect();
            (
                i.pointer.hover_pos(),
                i.modifiers,
                moves,
                [
                    i.pointer.button_pressed(PointerButton::Primary),
                    i.pointer.button_pressed(PointerButton::Secondary),
                    i.pointer.button_pressed(PointerButton::Middle),
                ],
                [
                    i.pointer.button_down(PointerButton::Primary),
                    i.pointer.button_down(PointerButton::Secondary),
                    i.pointer.button_down(PointerButton::Middle),
                ],
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.pointer.latest_pos(),
                i.key_down(egui::Key::Space),
            )
        });
        let text_editing = matches!(self.pending, Pending::Text(_));
        self.space_down = space && !text_editing;
        let hovered = resp.hovered() || self.drag.is_some();
        if let Some(h) = hover.filter(|_| hovered) {
            let ip = self.to_image(rect, h);
            self.status_pos = Some((ip.x.floor() as i32, ip.y.floor() as i32));
        } else if self.drag.is_none() {
            self.status_pos = None;
        }

        // wheel: zoom with Ctrl, otherwise scroll
        if resp.hovered() {
            if zoomd != 1.0 {
                // accumulate wheel/pinch input so one notch is one zoom step, however many events it arrives as
                let id = egui::Id::new("zoom_accum");
                let mut acc: f32 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(1.0);
                if (acc > 1.0) != (zoomd > 1.0) {
                    acc = 1.0;
                }
                acc *= zoomd;
                const STEP: f32 = 1.2;
                if acc >= STEP || acc <= 1.0 / STEP {
                    let z = self.docs[self.cur].view.zoom;
                    let nz = zoom_step(z, if acc > 1.0 { 1 } else { -1 });
                    self.set_zoom(nz, hover);
                    acc = 1.0;
                }
                ui.ctx().data_mut(|d| d.insert_temp(id, acc));
            } else if scroll != Vec2::ZERO {
                let doc = &mut self.docs[self.cur];
                doc.view.center -= scroll / doc.view.zoom;
                Self::clamp_center(doc);
            }
        }

        // press
        if self.drag.is_none() && resp.hovered() {
            let btns = [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle];
            if let Some(idx) = (0..3).find(|&i| pressed[i]) {
                let Some(sp) = latest.or(hover) else { return };
                let btn = btns[idx];
                let pan = btn == PointerButton::Middle || self.space_down || self.tool == Tool::Pan;
                self.drag = Some(DragInfo { button: btn, pan, last_screen: sp });
                if !pan {
                    let ip = self.to_image(rect, sp);
                    self.tool_press(btn, ip, mods);
                }
            }
        }

        // drag
        if let Some(d) = self.drag {
            let mut pts = moves;
            if pts.is_empty() {
                if let Some(l) = latest {
                    if l != d.last_screen {
                        pts.push(l);
                    }
                }
            }
            for sp in pts {
                let last = self.drag.unwrap().last_screen;
                if d.pan {
                    let doc = &mut self.docs[self.cur];
                    doc.view.center -= (sp - last) / doc.view.zoom;
                    Self::clamp_center(doc);
                } else {
                    let ip = self.to_image(rect, sp);
                    self.tool_drag(ip, mods);
                }
                if let Some(dd) = &mut self.drag {
                    dd.last_screen = sp;
                }
            }
            let idx = match d.button {
                PointerButton::Primary => 0,
                PointerButton::Secondary => 1,
                _ => 2,
            };
            if !down[idx] {
                if !d.pan {
                    let sp = self.drag.unwrap().last_screen;
                    let ip = self.to_image(rect, sp);
                    self.tool_release(ip, mods);
                }
                self.drag = None;
            }
        }

        // cursor
        if resp.hovered() {
            let icon = if self.drag.map(|d| d.pan).unwrap_or(false) {
                CursorIcon::Grabbing
            } else if self.space_down || self.tool == Tool::Pan {
                CursorIcon::Grab
            } else {
                match self.tool {
                    Tool::MovePixels | Tool::MoveSelection => CursorIcon::Move,
                    Tool::Zoom => {
                        if mods.alt {
                            CursorIcon::ZoomOut
                        } else {
                            CursorIcon::ZoomIn
                        }
                    }
                    Tool::Text => CursorIcon::Text,
                    Tool::Paintbrush | Tool::Eraser | Tool::CloneStamp | Tool::SmartClone | Tool::Recolor => CursorIcon::None,
                    _ => CursorIcon::Crosshair,
                }
            };
            ui.ctx().set_cursor_icon(icon);
        }
    }

    fn draw_canvas(&mut self, ui: &egui::Ui, painter: &Painter, rect: Rect, hovered: bool) {
        let ctx = ui.ctx().clone();
        let checker = self.checker_texture(&ctx);
        let doc = &self.docs[self.cur];
        let zoom = doc.view.zoom;
        let img_rect = Rect::from_min_max(self.to_screen(rect, Pos2::ZERO), self.to_screen(rect, pos2(doc.width as f32, doc.height as f32)));
        // drop shadow
        painter.add(egui::Shadow { offset: [0, 4], blur: 16, spread: 1, color: Color32::from_black_alpha(if ui.visuals().dark_mode { 150 } else { 70 }) }.as_shape(img_rect, 0.0));
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(img_rect.width() / 16.0, img_rect.height() / 16.0));
        painter.image(checker, img_rect, uv, Color32::WHITE);
        if let Some(t) = &doc.texture {
            painter.image(t.id(), img_rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
        }
        painter.add(Shape::closed_line(
            vec![img_rect.left_top(), img_rect.right_top(), img_rect.right_bottom(), img_rect.left_bottom()],
            Stroke::new(1.0f32, Color32::from_gray(90)),
        ));

        // pixel grid
        if self.show_grid && zoom >= 4.0 {
            let vis = img_rect.intersect(rect);
            let tl = self.to_image(rect, vis.min);
            let br = self.to_image(rect, vis.max);
            let stroke = Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(80, 80, 80, 90));
            for x in (tl.x.floor().max(0.0) as i32)..=(br.x.ceil().min(doc.width as f32) as i32) {
                let sx = self.to_screen(rect, pos2(x as f32, 0.0)).x;
                painter.line_segment([pos2(sx, vis.top()), pos2(sx, vis.bottom())], stroke);
            }
            for y in (tl.y.floor().max(0.0) as i32)..=(br.y.ceil().min(doc.height as f32) as i32) {
                let sy = self.to_screen(rect, pos2(0.0, y as f32)).y;
                painter.line_segment([pos2(vis.left(), sy), pos2(vis.right(), sy)], stroke);
            }
        }

        // snap grid (every `grid_size` px, hidden when the lines would be too dense)
        if self.snap {
            let g = self.grid_size.max(1) as f32;
            if g * zoom >= 6.0 {
                let vis = img_rect.intersect(rect);
                let tl = self.to_image(rect, vis.min);
                let br = self.to_image(rect, vis.max);
                let acc = crate::theme::pal(ui.visuals().dark_mode).accent;
                let stroke = Stroke::new(1.0f32, acc.gamma_multiply(0.45));
                let (x0, x1) = ((tl.x.max(0.0) / g).floor() as i32, (br.x.min(doc.width as f32) / g).ceil() as i32);
                let (y0, y1) = ((tl.y.max(0.0) / g).floor() as i32, (br.y.min(doc.height as f32) / g).ceil() as i32);
                for i in x0..=x1 {
                    let x = (i as f32 * g).min(doc.width as f32);
                    let sx = self.to_screen(rect, pos2(x, 0.0)).x.round() + 0.5;
                    painter.line_segment([pos2(sx, vis.top()), pos2(sx, vis.bottom())], stroke);
                }
                for i in y0..=y1 {
                    let y = (i as f32 * g).min(doc.height as f32);
                    let sy = self.to_screen(rect, pos2(0.0, y)).y.round() + 0.5;
                    painter.line_segment([pos2(vis.left(), sy), pos2(vis.right(), sy)], stroke);
                }
            }
        }

        // marching ants
        if !doc.sel_outline.is_empty() {
            let t = ctx.input(|i| i.time) as f32;
            let off = (t * 12.0) % 8.0;
            let many = doc.sel_outline.len() > 40_000;
            let black = Stroke::new(1.0f32, Color32::BLACK);
            let white = Stroke::new(1.0f32, Color32::WHITE);
            let mut shapes = Vec::with_capacity(doc.sel_outline.len() * 2);
            for seg in &doc.sel_outline {
                let a = self.to_screen(rect, seg[0]);
                let b = self.to_screen(rect, seg[1]);
                if !rect.expand(2.0).intersects(Rect::from_two_pos(a, b)) {
                    continue;
                }
                shapes.push(Shape::line_segment([a, b], white));
                if !many {
                    shapes.extend(Shape::dashed_line_with_offset(&[a, b], black, &[4.0], &[4.0], off + a.x + a.y));
                }
            }
            painter.extend(shapes);
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }

        // pending overlays
        let handle = |p: Pos2| {
            painter.circle(p, 4.5, Color32::WHITE, Stroke::new(1.2f32, Color32::from_rgb(30, 60, 120)));
        };
        let dashed_box = |a: Pos2, b: Pos2| {
            let r = Rect::from_two_pos(a, b);
            let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0f32, Color32::from_rgb(40, 90, 200)), 4.0, 3.0));
        };
        match &self.pending {
            Pending::Select(s) if s.tool == Tool::LassoSelect => {
                let pts: Vec<Pos2> = s.points.iter().map(|p| self.to_screen(rect, *p)).collect();
                painter.add(Shape::line(pts, Stroke::new(1.0f32, Color32::from_rgb(40, 90, 200))));
            }
            Pending::Shape(s) => {
                dashed_box(self.to_screen(rect, s.a), self.to_screen(rect, s.b));
                for h in shape_handles(s.a, s.b) {
                    handle(self.to_screen(rect, h));
                }
            }
            Pending::Line(l) => {
                handle(self.to_screen(rect, l.p0));
                handle(self.to_screen(rect, l.p1));
                let c = self.to_screen(rect, l.c);
                painter.rect_filled(Rect::from_center_size(c, vec2(8.0, 8.0)), 1.0, Color32::from_rgb(250, 200, 60));
            }
            Pending::Gradient(g) => {
                let (a, b) = (self.to_screen(rect, g.s), self.to_screen(rect, g.e));
                painter.line_segment([a, b], Stroke::new(1.0f32, Color32::from_rgb(40, 90, 200)));
                handle(a);
                handle(b);
                let mid = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                painter.rect_filled(Rect::from_center_size(mid, vec2(7.0, 7.0)), 1.0, Color32::from_rgb(40, 90, 200));
            }
            Pending::Text(t) => {
                let lay = text::layout(&t.text, (t.pos.x, t.pos.y), &self.text_params());
                let (x0, y0, x1, y1) = lay.bounds;
                dashed_box(self.to_screen(rect, pos2(x0 - 2.0, y0 - 2.0)), self.to_screen(rect, pos2(x1 + 2.0, y1 + 2.0)));
                let blink = (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0;
                if blink {
                    if let Some(&(cx, cy, ch)) = lay.carets.get(t.cursor.min(lay.carets.len().saturating_sub(1))) {
                        let a = self.to_screen(rect, pos2(cx, cy));
                        let b = self.to_screen(rect, pos2(cx, cy + ch));
                        painter.line_segment([a, b], Stroke::new(1.5f32, Color32::BLACK));
                    }
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            Pending::MovePixels(MoveEdit { tx, ty, pivot, .. }) | Pending::MoveSel(SelMoveEdit { tx, ty, pivot, .. }) => {
                let pv = self.to_screen(rect, *pivot + vec2(*tx, *ty));
                painter.circle_stroke(pv, 5.0, Stroke::new(1.2f32, Color32::from_rgb(40, 90, 200)));
                painter.line_segment([pv - vec2(8.0, 0.0), pv + vec2(8.0, 0.0)], Stroke::new(1.0f32, Color32::from_rgb(40, 90, 200)));
                painter.line_segment([pv - vec2(0.0, 8.0), pv + vec2(0.0, 8.0)], Stroke::new(1.0f32, Color32::from_rgb(40, 90, 200)));
            }
            Pending::ZoomRect { start, cur, .. } => {
                dashed_box(self.to_screen(rect, *start), self.to_screen(rect, *cur));
            }
            _ => {}
        }

        // clone source marker
        if self.tool == Tool::CloneStamp || (self.tool == Tool::SmartClone && self.ts.smart_mode == crate::smart_clone::SmartMode::Blend) {
            if let Some(src) = self.clone_source {
                let mut s = src;
                if let (Pending::Stroke(st), Some(_)) = (&self.pending, self.drag) {
                    if let Some(last) = st.points.last() {
                        s = *last - st.offset;
                    }
                }
                let c = self.to_screen(rect, s);
                let k = Stroke::new(1.5f32, Color32::from_rgb(220, 40, 40));
                painter.line_segment([c - vec2(7.0, 0.0), c + vec2(7.0, 0.0)], k);
                painter.line_segment([c - vec2(0.0, 7.0), c + vec2(0.0, 7.0)], k);
                painter.circle_stroke(c, self.ts.brush_width / 2.0 * zoom, k);
            }
        }

        // brush outline cursor
        if hovered && !self.space_down && matches!(self.tool, Tool::Paintbrush | Tool::Eraser | Tool::CloneStamp | Tool::SmartClone | Tool::Recolor) {
            if let Some(h) = ctx.input(|i| i.pointer.hover_pos()) {
                let r = (self.ts.brush_width / 2.0 * zoom).max(1.5);
                painter.circle_stroke(h, r, Stroke::new(1.0f32, Color32::BLACK));
                painter.circle_stroke(h, r + 1.0, Stroke::new(1.0f32, Color32::WHITE));
            }
        }
    }
}

fn selection_shape_mask(s: &SelectEdit, w: u32, h: u32) -> Option<Mask> {
    let mut mask = Mask::new(w, h)?;
    let path = match s.tool {
        Tool::LassoSelect => {
            if s.points.len() < 3 {
                return None;
            }
            let mut pb = PathBuilder::new();
            pb.move_to(s.points[0].x, s.points[0].y);
            for p in &s.points[1..] {
                pb.line_to(p.x, p.y);
            }
            pb.close();
            pb.finish()?
        }
        _ => {
            let mut d = s.cur - s.start;
            if s.square {
                let m = d.x.abs().max(d.y.abs());
                d = vec2(m * sgn(d.x), m * sgn(d.y));
            }
            let a = s.start;
            let b = s.start + d;
            let (x0, y0, x1, y1) = paint::norm_rect(a, b);
            if s.tool == Tool::RectSelect {
                let (x0, y0, x1, y1) = (x0.round(), y0.round(), x1.round(), y1.round());
                if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
                    return None;
                }
                PathBuilder::from_rect(tiny_skia::Rect::from_ltrb(x0, y0, x1, y1)?)
            } else {
                if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
                    return None;
                }
                PathBuilder::from_oval(tiny_skia::Rect::from_ltrb(x0, y0, x1, y1)?)?
            }
        }
    };
    let aa = s.tool != Tool::RectSelect;
    mask.fill_path(&path, FillRule::Winding, aa, Transform::identity());
    Some(mask)
}
