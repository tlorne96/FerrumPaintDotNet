//! Document model: layers, selection, history (undo/redo) and image-level operations.

use crate::raster::{self, Col, Orient, Resampling};
use std::path::PathBuf;
use tiny_skia::{BlendMode, FilterQuality, Mask, Pixmap, PixmapPaint, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerBlend {
    Normal,
    Multiply,
    Additive,
    ColorBurn,
    ColorDodge,
    Overlay,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Lighten,
    Darken,
    Screen,
    Xor,
    Hue,
    Saturation,
    Color,
    Luminosity,
    Reflect,
    Glow,
    Negation,
}

impl LayerBlend {
    pub const ALL: [LayerBlend; 21] = [
        LayerBlend::Normal,
        LayerBlend::Multiply,
        LayerBlend::Additive,
        LayerBlend::ColorBurn,
        LayerBlend::ColorDodge,
        LayerBlend::Overlay,
        LayerBlend::HardLight,
        LayerBlend::SoftLight,
        LayerBlend::Difference,
        LayerBlend::Exclusion,
        LayerBlend::Lighten,
        LayerBlend::Darken,
        LayerBlend::Screen,
        LayerBlend::Xor,
        LayerBlend::Hue,
        LayerBlend::Saturation,
        LayerBlend::Color,
        LayerBlend::Luminosity,
        LayerBlend::Reflect,
        LayerBlend::Glow,
        LayerBlend::Negation,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LayerBlend::Normal => "Normal",
            LayerBlend::Multiply => "Multiply",
            LayerBlend::Additive => "Additive",
            LayerBlend::ColorBurn => "Color Burn",
            LayerBlend::ColorDodge => "Color Dodge",
            LayerBlend::Overlay => "Overlay",
            LayerBlend::HardLight => "Hard Light",
            LayerBlend::SoftLight => "Soft Light",
            LayerBlend::Difference => "Difference",
            LayerBlend::Exclusion => "Exclusion",
            LayerBlend::Lighten => "Lighten",
            LayerBlend::Darken => "Darken",
            LayerBlend::Screen => "Screen",
            LayerBlend::Xor => "Xor",
            LayerBlend::Hue => "Hue",
            LayerBlend::Saturation => "Saturation",
            LayerBlend::Color => "Color",
            LayerBlend::Luminosity => "Luminosity",
            LayerBlend::Reflect => "Reflect",
            LayerBlend::Glow => "Glow",
            LayerBlend::Negation => "Negation",
        }
    }
    pub fn skia(self) -> BlendMode {
        match self {
            LayerBlend::Normal => BlendMode::SourceOver,
            LayerBlend::Multiply => BlendMode::Multiply,
            LayerBlend::Additive => BlendMode::Plus,
            LayerBlend::ColorBurn => BlendMode::ColorBurn,
            LayerBlend::ColorDodge => BlendMode::ColorDodge,
            LayerBlend::Overlay => BlendMode::Overlay,
            LayerBlend::HardLight => BlendMode::HardLight,
            LayerBlend::SoftLight => BlendMode::SoftLight,
            LayerBlend::Difference => BlendMode::Difference,
            LayerBlend::Exclusion => BlendMode::Exclusion,
            LayerBlend::Lighten => BlendMode::Lighten,
            LayerBlend::Darken => BlendMode::Darken,
            LayerBlend::Screen => BlendMode::Screen,
            LayerBlend::Xor => BlendMode::Xor,
            LayerBlend::Hue => BlendMode::Hue,
            LayerBlend::Saturation => BlendMode::Saturation,
            LayerBlend::Color => BlendMode::Color,
            LayerBlend::Luminosity => BlendMode::Luminosity,
            // custom modes are composited by `draw_layer`; this is only a fallback
            LayerBlend::Reflect | LayerBlend::Glow | LayerBlend::Negation => BlendMode::SourceOver,
        }
    }
    /// Per-channel blend function for the modes tiny-skia doesn't provide (a = below, b = layer).
    fn custom(self) -> Option<fn(f32, f32) -> f32> {
        match self {
            LayerBlend::Reflect => Some(|a, b| if b >= 1.0 { 1.0 } else { (a * a / (1.0 - b)).min(1.0) }),
            LayerBlend::Glow => Some(|a, b| if a >= 1.0 { 1.0 } else { (b * b / (1.0 - a)).min(1.0) }),
            LayerBlend::Negation => Some(|a, b| 1.0 - (1.0 - a - b).abs()),
            _ => None,
        }
    }
    pub fn id(self) -> u8 {
        Self::ALL.iter().position(|&b| b == self).unwrap() as u8
    }
    pub fn from_id(id: u8) -> Self {
        *Self::ALL.get(id as usize).unwrap_or(&LayerBlend::Normal)
    }
}

#[derive(Clone)]
pub struct Layer {
    pub name: String,
    pub pixmap: Pixmap,
    pub visible: bool,
    pub opacity: u8,
    pub blend: LayerBlend,
}

impl Layer {
    pub fn new(name: impl Into<String>, w: u32, h: u32) -> Self {
        Layer { name: name.into(), pixmap: raster::new_pixmap(w, h), visible: true, opacity: 255, blend: LayerBlend::Normal }
    }
}

#[derive(Clone)]
pub struct DocState {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active: usize,
    pub selection: Option<Mask>,
}

pub enum Snapshot {
    Layer(usize, Layer),
    Selection(Option<Mask>),
    Full(DocState),
}

pub struct HistoryEntry {
    pub id: u64,
    pub name: String,
    pub snap: Snapshot,
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub zoom: f32,
    /// image coordinate shown at the centre of the viewport
    pub center: egui::Vec2,
    pub needs_fit: bool,
    pub prev_zoom: Option<(f32, egui::Vec2)>,
}

pub struct Document {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active: usize,
    pub selection: Option<Mask>,
    pub sel_outline: Vec<[egui::Pos2; 2]>,
    pub sel_bounds: Option<(u32, u32, u32, u32)>,
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub base_name: String,
    pub path: Option<PathBuf>,
    pub name: String,
    pub saved_state: u64,
    pub view: View,
    pub composite_dirty: bool,
    pub thumbs_dirty: bool,
    pub texture: Option<egui::TextureHandle>,
    pub layer_thumbs: Vec<egui::TextureHandle>,
    pub doc_thumb: Option<egui::TextureHandle>,
    pub uid: u64,
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub const MAX_HISTORY: usize = 100;

impl Document {
    pub fn new(name: impl Into<String>, w: u32, h: u32, background: Option<Col>) -> Self {
        let mut bg = Layer::new("Background", w, h);
        if let Some(c) = background {
            bg.pixmap.fill(c.skia());
        }
        Self::from_layers(name, w, h, vec![bg], "New Image")
    }

    pub fn from_layers(name: impl Into<String>, w: u32, h: u32, layers: Vec<Layer>, base: &str) -> Self {
        let active = layers.len().saturating_sub(1);
        Document {
            width: w,
            height: h,
            layers,
            active,
            selection: None,
            sel_outline: vec![],
            sel_bounds: None,
            undo: vec![],
            redo: vec![],
            base_name: base.to_string(),
            path: None,
            name: name.into(),
            saved_state: 0,
            view: View { zoom: 1.0, center: egui::vec2(w as f32 / 2.0, h as f32 / 2.0), needs_fit: true, prev_zoom: None },
            composite_dirty: true,
            thumbs_dirty: true,
            texture: None,
            layer_thumbs: vec![],
            doc_thumb: None,
            uid: next_id(),
        }
    }

    pub fn state_id(&self) -> u64 {
        self.undo.last().map(|e| e.id).unwrap_or(0)
    }
    pub fn is_dirty(&self) -> bool {
        self.state_id() != self.saved_state
    }
    pub fn mark_saved(&mut self) {
        self.saved_state = self.state_id();
    }

    pub fn snapshot_full(&self) -> DocState {
        DocState {
            width: self.width,
            height: self.height,
            layers: self.layers.clone(),
            active: self.active,
            selection: self.selection.clone(),
        }
    }

    pub fn snapshot_layer(&self, i: usize) -> Snapshot {
        Snapshot::Layer(i, self.layers[i].clone())
    }

    /// Record an already-captured "before" snapshot under `name`.
    pub fn push_history(&mut self, name: impl Into<String>, snap: Snapshot) {
        self.redo.clear();
        self.undo.push(HistoryEntry { id: next_id(), name: name.into(), snap });
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
            // If the saved state fell off the stack the document can never be "clean" again.
            if self.saved_state == 0 {
                self.saved_state = u64::MAX;
            }
        }
        self.changed();
    }

    pub fn changed(&mut self) {
        self.composite_dirty = true;
        self.thumbs_dirty = true;
    }

    fn swap_snapshot(&mut self, snap: &mut Snapshot) {
        match snap {
            Snapshot::Layer(i, layer) => {
                if *i < self.layers.len() {
                    std::mem::swap(&mut self.layers[*i], layer);
                }
            }
            Snapshot::Selection(sel) => {
                std::mem::swap(&mut self.selection, sel);
                self.update_selection_outline();
            }
            Snapshot::Full(st) => {
                std::mem::swap(&mut self.width, &mut st.width);
                std::mem::swap(&mut self.height, &mut st.height);
                std::mem::swap(&mut self.layers, &mut st.layers);
                std::mem::swap(&mut self.active, &mut st.active);
                std::mem::swap(&mut self.selection, &mut st.selection);
                self.update_selection_outline();
            }
        }
        self.active = self.active.min(self.layers.len() - 1);
        self.changed();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(mut e) = self.undo.pop() {
            self.swap_snapshot(&mut e.snap);
            self.redo.push(e);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(mut e) = self.redo.pop() {
            self.swap_snapshot(&mut e.snap);
            self.undo.push(e);
            true
        } else {
            false
        }
    }

    // ------------------------------------------------------------------------------------------
    // Selection

    pub fn set_selection(&mut self, sel: Option<Mask>) {
        self.selection = sel;
        self.update_selection_outline();
    }

    /// Selection mask suitable for clipping painting (None = everything).
    #[allow(dead_code)]
    pub fn clip(&self) -> Option<&Mask> {
        self.selection.as_ref()
    }

    pub fn update_selection_outline(&mut self) {
        self.sel_outline.clear();
        self.sel_bounds = None;
        let Some(m) = &self.selection else { return };
        let (w, h) = (m.width() as i32, m.height() as i32);
        let d = m.data();
        let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && d[(y * w + x) as usize] >= 128;
        // horizontal edges
        for y in 0..=h {
            let mut run: Option<i32> = None;
            for x in 0..=w {
                let edge = x < w && inside(x, y - 1) != inside(x, y);
                match (edge, run) {
                    (true, None) => run = Some(x),
                    (false, Some(s)) => {
                        self.sel_outline.push([egui::pos2(s as f32, y as f32), egui::pos2(x as f32, y as f32)]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        // vertical edges
        for x in 0..=w {
            let mut run: Option<i32> = None;
            for y in 0..=h {
                let edge = y < h && inside(x - 1, y) != inside(x, y);
                match (edge, run) {
                    (true, None) => run = Some(y),
                    (false, Some(s)) => {
                        self.sel_outline.push([egui::pos2(x as f32, s as f32), egui::pos2(x as f32, y as f32)]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        self.sel_bounds = raster::mask_bounds(m);
    }

    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    // ------------------------------------------------------------------------------------------
    // Rendering

    pub fn composite(&self) -> Pixmap {
        let mut out = raster::new_pixmap(self.width, self.height);
        for l in &self.layers {
            if l.visible && l.opacity > 0 {
                draw_layer(&mut out, &l.pixmap, l.opacity, l.blend);
            }
        }
        out
    }

    #[allow(dead_code)]
    fn composite_skia_only(&self) -> Pixmap {
        let mut out = raster::new_pixmap(self.width, self.height);
        for l in &self.layers {
            if !l.visible || l.opacity == 0 {
                continue;
            }
            let paint = PixmapPaint {
                opacity: l.opacity as f32 / 255.0,
                blend_mode: l.blend.skia(),
                quality: FilterQuality::Nearest,
            };
            out.draw_pixmap(0, 0, l.pixmap.as_ref(), &paint, Transform::identity(), None);
        }
        out
    }

    // ------------------------------------------------------------------------------------------
    // Layer operations (each records history)

    pub fn add_layer(&mut self) {
        let before = Snapshot::Full(self.snapshot_full());
        let n = self.layers.len() + 1;
        let layer = Layer::new(format!("Layer {}", n), self.width, self.height);
        self.layers.insert(self.active + 1, layer);
        self.active += 1;
        self.push_history("Add New Layer", before);
    }

    pub fn delete_layer(&mut self) {
        if self.layers.len() <= 1 {
            return;
        }
        let before = Snapshot::Full(self.snapshot_full());
        self.layers.remove(self.active);
        self.active = self.active.min(self.layers.len() - 1).max(0);
        if self.active > 0 && self.active >= self.layers.len() {
            self.active = self.layers.len() - 1;
        }
        self.push_history("Delete Layer", before);
    }

    pub fn duplicate_layer(&mut self) {
        let before = Snapshot::Full(self.snapshot_full());
        let mut l = self.layers[self.active].clone();
        l.name = format!("{} copy", l.name);
        self.layers.insert(self.active + 1, l);
        self.active += 1;
        self.push_history("Duplicate Layer", before);
    }

    pub fn merge_down(&mut self) {
        if self.active == 0 {
            return;
        }
        let before = Snapshot::Full(self.snapshot_full());
        let top = self.layers.remove(self.active);
        let below = &mut self.layers[self.active - 1];
        if top.visible {
            draw_layer(&mut below.pixmap, &top.pixmap, top.opacity, top.blend);
        }
        self.active -= 1;
        self.push_history("Merge Layer Down", before);
    }

    pub fn move_layer(&mut self, to: usize) {
        let to = to.min(self.layers.len() - 1);
        if to == self.active {
            return;
        }
        let before = Snapshot::Full(self.snapshot_full());
        let l = self.layers.remove(self.active);
        self.layers.insert(to, l);
        let name = if to > self.active { "Move Layer Up" } else { "Move Layer Down" };
        self.active = to;
        self.push_history(name, before);
    }

    pub fn flatten(&mut self) {
        if self.layers.len() <= 1 {
            return;
        }
        let before = Snapshot::Full(self.snapshot_full());
        let mut bg = Layer::new("Background", self.width, self.height);
        bg.pixmap = self.composite();
        self.layers = vec![bg];
        self.active = 0;
        self.push_history("Flatten", before);
    }

    pub fn orient_layer(&mut self, o: Orient) {
        let before = self.snapshot_layer(self.active);
        let l = &mut self.layers[self.active];
        l.pixmap = raster::orient_pixmap(&l.pixmap, o);
        let name = match o {
            Orient::FlipH => "Flip Layer Horizontal",
            _ => "Flip Layer Vertical",
        };
        self.push_history(name, before);
    }

    // ------------------------------------------------------------------------------------------
    // Image operations

    pub fn orient_image(&mut self, o: Orient) {
        let before = Snapshot::Full(self.snapshot_full());
        for l in &mut self.layers {
            l.pixmap = raster::orient_pixmap(&l.pixmap, o);
        }
        let (nw, nh) = o.new_size(self.width, self.height);
        self.width = nw;
        self.height = nh;
        if let Some(m) = &self.selection {
            self.selection = Some(raster::orient_mask(m, o));
        }
        self.update_selection_outline();
        let name = match o {
            Orient::FlipH => "Flip Horizontal",
            Orient::FlipV => "Flip Vertical",
            Orient::Rot90Cw => "Rotate 90° Clockwise",
            Orient::Rot90Ccw => "Rotate 90° Counter-clockwise",
            Orient::Rot180 => "Rotate 180°",
        };
        self.push_history(name, before);
        self.view.needs_fit = true;
    }

    pub fn resize_image(&mut self, nw: u32, nh: u32, r: Resampling) {
        if nw == self.width && nh == self.height {
            return;
        }
        let before = Snapshot::Full(self.snapshot_full());
        for l in &mut self.layers {
            l.pixmap = raster::resize_pixmap(&l.pixmap, nw, nh, r);
        }
        if let Some(m) = &self.selection {
            self.selection = Some(raster::resize_mask(m, nw, nh));
        }
        self.width = nw;
        self.height = nh;
        self.update_selection_outline();
        self.push_history("Resize Image", before);
        self.view.needs_fit = true;
    }

    /// anchor: (ax, ay) each in {0, 1, 2} = left/centre/right, top/middle/bottom
    pub fn canvas_size(&mut self, nw: u32, nh: u32, anchor: (u8, u8), secondary: Col) {
        if nw == self.width && nh == self.height {
            return;
        }
        let before = Snapshot::Full(self.snapshot_full());
        let off = |old: u32, new: u32, a: u8| -> i32 {
            match a {
                0 => 0,
                1 => (new as i32 - old as i32) / 2,
                _ => new as i32 - old as i32,
            }
        };
        let ox = off(self.width, nw, anchor.0);
        let oy = off(self.height, nh, anchor.1);
        for (i, l) in self.layers.iter_mut().enumerate() {
            // The bottom layer gets the secondary colour in new areas, like a background.
            let fill = if i == 0 { Some(secondary) } else { None };
            l.pixmap = raster::recanvas_pixmap(&l.pixmap, nw, nh, ox, oy, fill);
        }
        if let Some(m) = &self.selection {
            self.selection = Some(raster::recanvas_mask(m, nw, nh, ox, oy));
        }
        self.width = nw;
        self.height = nh;
        self.update_selection_outline();
        self.push_history("Canvas Size", before);
        self.view.needs_fit = true;
    }

    pub fn crop_to_selection(&mut self) {
        let Some((x0, y0, x1, y1)) = self.sel_bounds else { return };
        let before = Snapshot::Full(self.snapshot_full());
        let (nw, nh) = (x1 - x0, y1 - y0);
        let sel = self.selection.clone().unwrap();
        let cropped_mask = raster::recanvas_mask(&sel, nw, nh, -(x0 as i32), -(y0 as i32));
        let rect_like = cropped_mask.data().iter().all(|&v| v == 255);
        for l in &mut self.layers {
            let mut p = raster::recanvas_pixmap(&l.pixmap, nw, nh, -(x0 as i32), -(y0 as i32), None);
            if !rect_like {
                // Pixels outside a non-rectangular selection become transparent.
                let md = cropped_mask.data();
                for (i, px) in p.data_mut().chunks_exact_mut(4).enumerate() {
                    let m = md[i] as u32;
                    for c in px.iter_mut() {
                        *c = raster::mul8(*c as u32, m);
                    }
                }
            }
            l.pixmap = p;
        }
        self.width = nw;
        self.height = nh;
        self.selection = if rect_like { None } else { Some(cropped_mask) };
        self.update_selection_outline();
        self.push_history("Crop to Selection", before);
        self.view.needs_fit = true;
    }

    /// Erase selection on active layer.
    pub fn erase_selection(&mut self) {
        let before = self.snapshot_layer(self.active);
        let sel = self.selection.clone();
        let l = &mut self.layers[self.active];
        match &sel {
            None => l.pixmap.fill(tiny_skia::Color::TRANSPARENT),
            Some(m) => {
                let md = m.data();
                for (i, px) in l.pixmap.data_mut().chunks_exact_mut(4).enumerate() {
                    let k = 255 - md[i] as u32;
                    for c in px.iter_mut() {
                        *c = raster::mul8(*c as u32, k);
                    }
                }
            }
        }
        self.push_history("Erase Selection", before);
    }

    pub fn fill_selection(&mut self, c: Col) {
        let before = self.snapshot_layer(self.active);
        let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, self.width as f32, self.height as f32).unwrap();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(c.skia());
        let sel = self.selection.clone();
        self.layers[self.active].pixmap.fill_rect(rect, &paint, Transform::identity(), sel.as_ref());
        self.push_history("Fill Selection", before);
    }

    pub fn select_all(&mut self) {
        let before = Snapshot::Selection(self.selection.clone());
        self.set_selection(Some(raster::full_mask(self.width, self.height)));
        self.push_history("Select All", before);
    }

    pub fn deselect(&mut self) {
        if self.selection.is_none() {
            return;
        }
        let before = Snapshot::Selection(self.selection.clone());
        self.set_selection(None);
        self.push_history("Deselect All", before);
    }

    pub fn invert_selection(&mut self) {
        let Some(m) = &self.selection else { return };
        let before = Snapshot::Selection(self.selection.clone());
        let mut m = m.clone();
        m.invert();
        self.set_selection(Some(m));
        self.push_history("Invert Selection", before);
    }

    /// Extract the selected area of a pixmap, cropped to selection bounds.
    pub fn extract_selected(&self, src: &Pixmap) -> Option<Pixmap> {
        let (x0, y0, x1, y1) = match self.sel_bounds {
            Some(b) => b,
            None if self.selection.is_none() => (0, 0, self.width, self.height),
            None => return None,
        };
        let (w, h) = (x1 - x0, y1 - y0);
        let mut p = raster::recanvas_pixmap(src, w, h, -(x0 as i32), -(y0 as i32), None);
        if let Some(m) = &self.selection {
            let cm = raster::recanvas_mask(m, w, h, -(x0 as i32), -(y0 as i32));
            for (i, px) in p.data_mut().chunks_exact_mut(4).enumerate() {
                let k = cm.data()[i] as u32;
                for c in px.iter_mut() {
                    *c = raster::mul8(*c as u32, k);
                }
            }
        }
        Some(p)
    }
}

/// Composite `src` over `dst` with a layer opacity and blend mode.
pub fn draw_layer(dst: &mut Pixmap, src: &Pixmap, opacity: u8, blend: LayerBlend) {
    let Some(f) = blend.custom() else {
        let paint = PixmapPaint { opacity: opacity as f32 / 255.0, blend_mode: blend.skia(), quality: FilterQuality::Nearest };
        dst.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::identity(), None);
        return;
    };
    // Separable blend on premultiplied data: (1-sa)·D + (1-da)·S + sa·da·F(Dc, Sc).
    let op = opacity as f32 / 255.0;
    let s = src.data();
    for (i, d) in dst.data_mut().chunks_exact_mut(4).enumerate() {
        let sp = &s[i * 4..i * 4 + 4];
        let sa = sp[3] as f32 / 255.0 * op;
        if sa <= 0.0 {
            continue;
        }
        let da = d[3] as f32 / 255.0;
        for c in 0..3 {
            let sc_pm = sp[c] as f32 / 255.0 * op;
            let dc_pm = d[c] as f32 / 255.0;
            let sc = if sa > 0.0 { (sc_pm / sa).min(1.0) } else { 0.0 };
            let dc = if da > 0.0 { (dc_pm / da).min(1.0) } else { 0.0 };
            let v = (1.0 - sa) * dc_pm + (1.0 - da) * sc_pm + sa * da * f(dc, sc);
            d[c] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
        let a = sa + da - sa * da;
        d[3] = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        for c in 0..3 {
            d[c] = d[c].min(d[3]);
        }
    }
}