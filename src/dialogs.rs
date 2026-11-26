//! Modal dialogs. Filter, layer property and rotate/zoom dialogs preview live on the canvas.

use crate::app::App;
use crate::document::{Document, Layer, LayerBlend, Snapshot};
use crate::raster::{self, Col, Filter, Resampling};
use egui::{vec2, Color32, Sense, Stroke, Ui};
use tiny_skia::{FilterQuality, PixmapPaint, Transform};

pub enum Dialog {
    NewImage { w: u32, h: u32, keep: bool, bg: u8 },
    Resize { pct: f32, by_pct: bool, w: u32, h: u32, ow: u32, oh: u32, keep: bool, res: Resampling },
    Canvas { w: u32, h: u32, ow: u32, oh: u32, keep: bool, anchor: (u8, u8) },
    LayerProps { uid: u64, idx: usize, before: Layer, name: String, visible: bool, opacity: u8, blend: LayerBlend },
    Filter { uid: u64, idx: usize, before: Layer, filter: Filter, applied: Option<Filter>, sel_curve: Option<usize> },
    RotateZoom { uid: u64, idx: usize, before: Layer, angle: f32, pan: (f32, f32), zoom: f32, applied: Option<(f32, (f32, f32), f32)> },
    PasteExpand { img: tiny_skia::Pixmap, new_layer: bool },
    ConfirmClose { uid: u64 },
    About,
    Shortcuts,
    Message(String),
}

impl Dialog {
    pub fn new_image(w: u32, h: u32) -> Self {
        Dialog::NewImage { w, h, keep: false, bg: 0 }
    }
    pub fn resize(w: u32, h: u32) -> Self {
        Dialog::Resize { pct: 100.0, by_pct: false, w, h, ow: w, oh: h, keep: true, res: Resampling::ALL[0] }
    }
    pub fn canvas(w: u32, h: u32) -> Self {
        Dialog::Canvas { w, h, ow: w, oh: h, keep: false, anchor: (1, 1) }
    }
    pub fn layer_props(doc: &Document, idx: usize) -> Self {
        let l = &doc.layers[idx];
        Dialog::LayerProps { uid: doc.uid, idx, before: l.clone(), name: l.name.clone(), visible: l.visible, opacity: l.opacity, blend: l.blend }
    }
    pub fn filter(doc: &Document, f: Filter) -> Self {
        Dialog::Filter { uid: doc.uid, idx: doc.active, before: doc.layers[doc.active].clone(), filter: f, applied: None, sel_curve: None }
    }
    pub fn rotate_zoom(doc: &Document) -> Self {
        Dialog::RotateZoom { uid: doc.uid, idx: doc.active, before: doc.layers[doc.active].clone(), angle: 0.0, pan: (0.0, 0.0), zoom: 1.0, applied: None }
    }
}

#[derive(PartialEq)]
enum Outcome {
    Open,
    Ok,
    Cancel,
}

fn ok_cancel(ui: &mut Ui) -> Outcome {
    let mut o = Outcome::Open;
    ui.add_space(6.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add_sized([80.0, 24.0], egui::Button::new("Cancel")).clicked() {
            o = Outcome::Cancel;
        }
        if ui.add_sized([80.0, 24.0], egui::Button::new("OK")).clicked() {
            o = Outcome::Ok;
        }
    });
    o
}

fn keys(ui: &Ui) -> Outcome {
    ui.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            Outcome::Cancel
        } else if i.key_pressed(egui::Key::Enter) {
            Outcome::Ok
        } else {
            Outcome::Open
        }
    })
}

fn merge(a: Outcome, b: Outcome) -> Outcome {
    if a != Outcome::Open {
        a
    } else {
        b
    }
}

fn int_slider(ui: &mut Ui, label: &str, v: &mut i32, lo: i32, hi: i32) {
    ui.label(label);
    ui.add(egui::Slider::new(v, lo..=hi));
    ui.end_row();
}

fn uint_slider(ui: &mut Ui, label: &str, v: &mut u32, lo: u32, hi: u32) {
    ui.label(label);
    ui.add(egui::Slider::new(v, lo..=hi));
    ui.end_row();
}

fn f_slider(ui: &mut Ui, label: &str, v: &mut f32, lo: f32, hi: f32) {
    ui.label(label);
    ui.add(egui::Slider::new(v, lo..=hi));
    ui.end_row();
}

/// Filter-specific controls. Returns nothing; mutates the filter.
fn filter_controls(ui: &mut Ui, f: &mut Filter, sel_curve: &mut Option<usize>) {
    egui::Grid::new("fgrid").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| match f {
        Filter::BrightnessContrast { brightness, contrast } => {
            int_slider(ui, "Brightness", brightness, -100, 100);
            int_slider(ui, "Contrast", contrast, -100, 100);
        }
        Filter::HueSaturation { hue, saturation, lightness } => {
            int_slider(ui, "Hue", hue, -180, 180);
            int_slider(ui, "Saturation", saturation, 0, 200);
            int_slider(ui, "Lightness", lightness, -100, 100);
        }
        Filter::Levels { in_lo, in_hi, gamma, out_lo, out_hi } => {
            ui.label("Input low");
            ui.add(egui::Slider::new(in_lo, 0..=254));
            ui.end_row();
            ui.label("Input high");
            ui.add(egui::Slider::new(in_hi, 1..=255));
            ui.end_row();
            ui.label("Gamma");
            ui.add(egui::Slider::new(gamma, 0.1..=10.0).logarithmic(true));
            ui.end_row();
            ui.label("Output low");
            ui.add(egui::Slider::new(out_lo, 0..=255));
            ui.end_row();
            ui.label("Output high");
            ui.add(egui::Slider::new(out_hi, 0..=255));
            ui.end_row();
            if *in_hi <= *in_lo {
                *in_hi = *in_lo + 1;
            }
        }
        Filter::Posterize { levels } => {
            ui.label("Levels");
            ui.add(egui::Slider::new(levels, 2..=64));
            ui.end_row();
        }
        Filter::Curves { points, luminosity } => {
            ui.label("Mode");
            ui.horizontal(|ui| {
                ui.selectable_value(luminosity, true, "Luminosity");
                ui.selectable_value(luminosity, false, "RGB");
            });
            ui.end_row();
            ui.label("");
            curve_editor(ui, points, sel_curve);
            ui.end_row();
        }
        Filter::GaussianBlur { radius } => uint_slider(ui, "Radius", radius, 0, 100),
        Filter::Sharpen { amount } => uint_slider(ui, "Amount", amount, 1, 20),
        Filter::AddNoise { intensity, saturation, coverage } => {
            uint_slider(ui, "Intensity", intensity, 0, 100);
            uint_slider(ui, "Color saturation", saturation, 0, 400);
            uint_slider(ui, "Coverage", coverage, 0, 100);
        }
        Filter::Pixelate { cell } => uint_slider(ui, "Cell size", cell, 1, 100),
        Filter::Emboss { angle } => f_slider(ui, "Angle", angle, -180.0, 180.0),
        Filter::EdgeDetect { angle } => f_slider(ui, "Angle", angle, -180.0, 180.0),
        Filter::Vignette { radius, strength } => {
            f_slider(ui, "Radius", radius, 0.1, 1.5);
            f_slider(ui, "Strength", strength, 0.0, 1.0);
        }
        Filter::OilPaint { radius } => uint_slider(ui, "Brush size", radius, 1, 8),
        _ => {
            ui.label("No options");
            ui.end_row();
        }
    });
}

fn curve_editor(ui: &mut Ui, points: &mut Vec<(u8, u8)>, sel: &mut Option<usize>) {
    let size = 256.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Color32::WHITE);
    for i in 1..4 {
        let t = i as f32 / 4.0 * size;
        p.line_segment([rect.min + vec2(t, 0.0), rect.min + vec2(t, size)], Stroke::new(1.0f32, Color32::from_gray(225)));
        p.line_segment([rect.min + vec2(0.0, t), rect.min + vec2(size, t)], Stroke::new(1.0f32, Color32::from_gray(225)));
    }
    let to_s = |x: f32, y: f32| egui::pos2(rect.min.x + x / 255.0 * size, rect.max.y - y / 255.0 * size);
    let lut = raster::curve_lut(points);
    let line: Vec<egui::Pos2> = (0..256).map(|i| to_s(i as f32, lut[i] as f32)).collect();
    p.add(egui::Shape::line(line, Stroke::new(1.5f32, Color32::from_gray(30))));
    for (i, &(x, y)) in points.iter().enumerate() {
        let c = if Some(i) == *sel { Color32::from_rgb(40, 110, 220) } else { Color32::from_gray(30) };
        p.circle_filled(to_s(x as f32, y as f32), 4.0, c);
    }
    let from_s = |s: egui::Pos2| -> (u8, u8) {
        (
            (((s.x - rect.min.x) / size) * 255.0).round().clamp(0.0, 255.0) as u8,
            (((rect.max.y - s.y) / size) * 255.0).round().clamp(0.0, 255.0) as u8,
        )
    };
    let hit = |s: egui::Pos2, pts: &Vec<(u8, u8)>| pts.iter().position(|&(x, y)| to_s(x as f32, y as f32).distance(s) < 7.0);
    if resp.drag_started() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            *sel = hit(pos, points).or_else(|| {
                points.push(from_s(pos));
                Some(points.len() - 1)
            });
        }
    }
    if resp.dragged() {
        if let (Some(i), Some(pos)) = (*sel, resp.interact_pointer_pos()) {
            if i < points.len() {
                points[i] = from_s(pos);
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(i) = hit(pos, points) {
                if points.len() > 2 {
                    points.remove(i);
                    *sel = None;
                }
            }
        }
    }
    resp.on_hover_text("Click to add a point, drag to move, right-click to remove");
}

fn rotate_zoom_render(before: &Layer, angle: f32, pan: (f32, f32), zoom: f32, sel: Option<&tiny_skia::Mask>, out: &mut tiny_skia::Pixmap) {
    let (w, h) = (before.pixmap.width() as f32, before.pixmap.height() as f32);
    let t = Transform::from_translate(w / 2.0 + pan.0, h / 2.0 + pan.1)
        .pre_concat(Transform::from_rotate(angle))
        .pre_concat(Transform::from_scale(zoom, zoom))
        .pre_concat(Transform::from_translate(-w / 2.0, -h / 2.0));
    let mut adj = raster::new_pixmap(before.pixmap.width(), before.pixmap.height());
    if sel.is_some() {
        // Keep unselected pixels; only the selected area is transformed.
        let mut src = before.pixmap.clone();
        let m = sel.unwrap();
        for (i, px) in src.data_mut().chunks_exact_mut(4).enumerate() {
            let k = m.data()[i] as u32;
            for c in px.iter_mut() {
                *c = raster::mul8(*c as u32, k);
            }
        }
        adj.draw_pixmap(0, 0, src.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t, None);
        // the hole left behind becomes transparent; composite transformed pixels over it
        let mut base = before.pixmap.clone();
        for (i, px) in base.data_mut().chunks_exact_mut(4).enumerate() {
            let k = 255 - m.data()[i] as u32;
            for c in px.iter_mut() {
                *c = raster::mul8(*c as u32, k);
            }
        }
        base.draw_pixmap(0, 0, adj.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        *out = base;
    } else {
        adj.draw_pixmap(0, 0, before.pixmap.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t, None);
        *out = adj;
    }
}

pub const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    ("Tools", &[
        ("S", "Selection tools (Rectangle, Lasso, Ellipse, Magic Wand); Shift+S cycles backwards"),
        ("M", "Move Selected Pixels / Move Selection"),
        ("Z / H", "Zoom / Pan"),
        ("F / G", "Paint Bucket / Gradient"),
        ("B / E / P", "Paintbrush / Eraser / Pencil"),
        ("K / L / R", "Color Picker / Clone Stamp / Recolor"),
        ("T", "Text"),
        ("O", "Line / Curve, then Shapes"),
        ("A / Shift+A", "Shapes tool: next / previous shape"),
        (", . /", "Line and Shapes: start cap / dash style / end cap"),
        ("[ ]", "Brush width −1 / +1 (Ctrl: ±5)"),
        ("X / C", "Swap colours / switch active colour slot"),
        ("Alt (Shapes)", "Toggle draw-from-centre while dragging"),
        ("Shift (Shapes)", "Constrain proportions"),
        ("Arrows", "Nudge the current selection/shape by 1 px (Ctrl: 10 px)"),
        ("Space + drag", "Pan with any tool"),
        ("Enter / Esc", "Finish the current drawing"),
    ]),
    ("File", &[
        ("Ctrl+N / Ctrl+O", "New / Open"),
        ("Ctrl+S / Ctrl+Shift+S", "Save / Save As"),
        ("Ctrl+Alt+S", "Save All"),
        ("Ctrl+W", "Close image"),
    ]),
    ("Edit", &[
        ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
        ("Ctrl+X / Ctrl+C / Ctrl+V", "Cut / Copy / Paste"),
        ("Ctrl+Shift+C", "Copy Merged"),
        ("Ctrl+Shift+V", "Paste into new layer"),
        ("Ctrl+Alt+V", "Paste into new image"),
        ("Delete / Backspace", "Erase selection / Fill selection with primary (Shift: secondary)"),
        ("Ctrl+A / Ctrl+D / Ctrl+I", "Select all / Deselect / Invert selection"),
    ]),
    ("View", &[
        ("Ctrl++ / Ctrl+-", "Zoom in / out (also Ctrl+wheel)"),
        ("Ctrl+B", "Zoom to window (press again to restore)"),
        ("Ctrl+Shift+B", "Zoom to selection"),
        ("Ctrl+0", "Actual size"),
        ("Ctrl+'", "Pixel grid"),
        ("F5 – F8", "Toggle Tools / History / Layers / Colors (Ctrl+Shift resets)"),
        ("Ctrl+Tab, Ctrl+1–9", "Switch image"),
    ]),
    ("Image", &[
        ("Ctrl+Shift+X", "Crop to selection"),
        ("Ctrl+R / Ctrl+Shift+R", "Resize / Canvas size"),
        ("Ctrl+H / Ctrl+G / Ctrl+J", "Rotate 90° CW / 90° CCW / 180°"),
        ("Ctrl+Shift+F", "Flatten"),
    ]),
    ("Layers", &[
        ("Ctrl+Shift+N / Ctrl+Shift+Del", "Add / Delete layer"),
        ("Ctrl+Shift+D / Ctrl+M", "Duplicate / Merge down"),
        ("Ctrl+,", "Toggle layer visibility"),
        ("Ctrl+Shift+Z", "Rotate / Zoom layer"),
        ("Alt+PgUp / Alt+PgDn", "Select layer above / below"),
        ("Alt+Shift+PgUp / PgDn", "Move layer up / down"),
        ("F4", "Layer properties"),
    ]),
    ("Adjustments & Effects", &[
        ("Ctrl+Shift+L / G / T / M / U", "Auto-Level / B&W / Brightness-Contrast / Curves / Hue-Saturation"),
        ("Ctrl+Shift+I / Ctrl+Alt+I", "Invert colours / Invert alpha"),
        ("Ctrl+L / Ctrl+Shift+P / Ctrl+Shift+E", "Levels / Posterize / Sepia"),
        ("Ctrl+F", "Repeat last effect"),
    ]),
];

impl App {
    pub fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.dialog.take() else { return };
        let keep = self.dialog_ui(ctx, &mut d);
        if keep && self.dialog.is_none() {
            self.dialog = Some(d);
        }
    }

    fn doc_by_uid(&mut self, uid: u64) -> Option<usize> {
        self.docs.iter().position(|d| d.uid == uid)
    }

    /// Returns true to keep the dialog open.
    fn dialog_ui(&mut self, ctx: &egui::Context, d: &mut Dialog) -> bool {
        let title_frame = |ui: &mut Ui, t: &str| {
            ui.heading(t);
            ui.separator();
        };
        match d {
            Dialog::Message(m) => {
                let mut close = false;
                egui::Modal::new("msg".into()).show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    title_frame(ui, "Ferrum Paint");
                    ui.label(m.as_str());
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([80.0, 24.0], egui::Button::new("OK")).clicked() {
                            close = true;
                        }
                    });
                    if keys(ui) != Outcome::Open {
                        close = true;
                    }
                });
                !close
            }
            Dialog::About => {
                let mut close = false;
                egui::Modal::new("about".into()).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
                        if let Some(t) = &self.checker {
                            let _ = t;
                        }
                        let tex = ui.ctx().data_mut(|dm| dm.get_temp::<egui::TextureHandle>("about_icon".into()));
                        let tex = tex.unwrap_or_else(|| {
                            let px = crate::icons::app_icon_rgba(128);
                            let img = egui::ColorImage::from_rgba_unmultiplied([128, 128], &px);
                            let t = ui.ctx().load_texture("about_icon", img, egui::TextureOptions::LINEAR);
                            ui.ctx().data_mut(|dm| dm.insert_temp("about_icon".into(), t.clone()));
                            t
                        });
                        ui.painter().image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                        ui.vertical(|ui| {
                            ui.heading("Ferrum Paint");
                            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                            ui.label("A layered raster image editor written in Rust,\nbuilt with egui and tiny-skia.");
                        });
                    });
                    ui.add_space(8.0);
                    ui.label("Ferrum Paint is an independent clean-room project. Its workflow is inspired by popular \
                              Windows image editors, but it contains no code, icons or other assets from any of them.");
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([80.0, 24.0], egui::Button::new("Close")).clicked() {
                            close = true;
                        }
                    });
                    if keys(ui) != Outcome::Open {
                        close = true;
                    }
                });
                !close
            }
            Dialog::Shortcuts => {
                let mut close = false;
                egui::Modal::new("shortcuts".into()).show(ctx, |ui| {
                    ui.set_width(560.0);
                    title_frame(ui, "Keyboard Shortcuts");
                    egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() * 0.7).show(ui, |ui| {
                        for (sec, items) in SHORTCUTS {
                            ui.label(egui::RichText::new(*sec).strong().size(15.0));
                            egui::Grid::new(*sec).num_columns(2).striped(true).spacing(vec2(16.0, 3.0)).show(ui, |ui| {
                                for (k, v) in items.iter() {
                                    ui.monospace(*k);
                                    ui.label(*v);
                                    ui.end_row();
                                }
                            });
                            ui.add_space(6.0);
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([80.0, 24.0], egui::Button::new("Close")).clicked() {
                            close = true;
                        }
                    });
                    if keys(ui) != Outcome::Open {
                        close = true;
                    }
                });
                !close
            }
            Dialog::NewImage { w, h, keep, bg } => {
                let mut out = Outcome::Open;
                egui::Modal::new("newimg".into()).show(ctx, |ui| {
                    ui.set_width(300.0);
                    title_frame(ui, "New Image");
                    let ratio = *w as f32 / (*h).max(1) as f32;
                    egui::Grid::new("ni").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        ui.label("Width:");
                        let rw = ui.add(egui::DragValue::new(w).range(1..=32768).suffix(" px"));
                        ui.end_row();
                        ui.label("Height:");
                        let rh = ui.add(egui::DragValue::new(h).range(1..=32768).suffix(" px"));
                        ui.end_row();
                        if *keep {
                            if rw.changed() {
                                *h = ((*w as f32 / ratio).round() as u32).max(1);
                            } else if rh.changed() {
                                *w = ((*h as f32 * ratio).round() as u32).max(1);
                            }
                        }
                        ui.label("");
                        ui.checkbox(keep, "Maintain aspect ratio");
                        ui.end_row();
                        ui.label("Background:");
                        ui.vertical(|ui| {
                            ui.radio_value(bg, 0, "White");
                            ui.radio_value(bg, 1, "Secondary colour");
                            ui.radio_value(bg, 2, "Transparent");
                        });
                        ui.end_row();
                    });
                    let mb = ((*w as u64) * (*h as u64) * 4) as f64 / (1024.0 * 1024.0);
                    ui.label(egui::RichText::new(format!("Memory: {mb:.1} MB")).weak());
                    out = merge(ok_cancel(ui), keys(ui));
                });
                if out == Outcome::Ok {
                    let (w, h, bg) = (*w, *h, *bg);
                    self.new_document(w, h);
                    let doc = &mut self.docs[self.cur];
                    match bg {
                        1 => doc.layers[0].pixmap.fill(self.secondary.skia()),
                        2 => doc.layers[0].pixmap.fill(tiny_skia::Color::TRANSPARENT),
                        _ => {}
                    }
                    doc.changed();
                }
                out == Outcome::Open
            }
            Dialog::Resize { pct, by_pct, w, h, ow, oh, keep, res } => {
                let mut out = Outcome::Open;
                egui::Modal::new("resize".into()).show(ctx, |ui| {
                    ui.set_width(320.0);
                    title_frame(ui, "Resize Image");
                    ui.horizontal(|ui| {
                        ui.label("Resampling:");
                        egui::ComboBox::from_id_salt("resamp").selected_text(res.name()).show_ui(ui, |ui| {
                            for r in Resampling::ALL {
                                ui.selectable_value(res, r, r.name());
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.radio_value(by_pct, true, "By percentage:");
                    ui.add_enabled_ui(*by_pct, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            if ui.add(egui::DragValue::new(pct).range(1.0..=1000.0).suffix(" %")).changed() {
                                *w = ((*ow as f32 * *pct / 100.0).round() as u32).max(1);
                                *h = ((*oh as f32 * *pct / 100.0).round() as u32).max(1);
                            }
                        });
                    });
                    ui.radio_value(by_pct, false, "By absolute size:");
                    ui.add_enabled_ui(!*by_pct, |ui| {
                        ui.checkbox(keep, "Maintain aspect ratio");
                        let ratio = *ow as f32 / *oh as f32;
                        egui::Grid::new("rs").num_columns(2).show(ui, |ui| {
                            ui.label("Width:");
                            if ui.add(egui::DragValue::new(w).range(1..=65535).suffix(" px")).changed() && *keep {
                                *h = ((*w as f32 / ratio).round() as u32).max(1);
                            }
                            ui.end_row();
                            ui.label("Height:");
                            if ui.add(egui::DragValue::new(h).range(1..=65535).suffix(" px")).changed() && *keep {
                                *w = ((*h as f32 * ratio).round() as u32).max(1);
                            }
                            ui.end_row();
                        });
                    });
                    ui.label(egui::RichText::new(format!("Original: {} × {}", ow, oh)).weak());
                    out = merge(ok_cancel(ui), keys(ui));
                });
                if out == Outcome::Ok {
                    let (w, h, r) = (*w, *h, *res);
                    self.docs[self.cur].resize_image(w, h, r);
                }
                out == Outcome::Open
            }
            Dialog::Canvas { w, h, ow, oh, keep, anchor } => {
                let mut out = Outcome::Open;
                egui::Modal::new("canvas".into()).show(ctx, |ui| {
                    ui.set_width(320.0);
                    title_frame(ui, "Canvas Size");
                    ui.checkbox(keep, "Maintain aspect ratio");
                    let ratio = *ow as f32 / *oh as f32;
                    egui::Grid::new("cs").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        ui.label("Width:");
                        if ui.add(egui::DragValue::new(w).range(1..=65535).suffix(" px")).changed() && *keep {
                            *h = ((*w as f32 / ratio).round() as u32).max(1);
                        }
                        ui.end_row();
                        ui.label("Height:");
                        if ui.add(egui::DragValue::new(h).range(1..=65535).suffix(" px")).changed() && *keep {
                            *w = ((*h as f32 * ratio).round() as u32).max(1);
                        }
                        ui.end_row();
                        ui.label("Anchor:");
                        egui::Grid::new("anchor").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                            for ay in 0..3u8 {
                                for ax in 0..3u8 {
                                    let sel = *anchor == (ax, ay);
                                    let arrow = if sel {
                                        "●"
                                    } else {
                                        let dx = ax as i32 - anchor.0 as i32;
                                        let dy = ay as i32 - anchor.1 as i32;
                                        match (dx.signum(), dy.signum(), dx.abs() <= 1 && dy.abs() <= 1) {
                                            (_, _, false) => " ",
                                            (-1, -1, _) => "↖",
                                            (0, -1, _) => "↑",
                                            (1, -1, _) => "↗",
                                            (-1, 0, _) => "←",
                                            (1, 0, _) => "→",
                                            (-1, 1, _) => "↙",
                                            (0, 1, _) => "↓",
                                            _ => "↘",
                                        }
                                    };
                                    if ui.add_sized([26.0, 26.0], egui::Button::new(arrow).selected(sel)).clicked() {
                                        *anchor = (ax, ay);
                                    }
                                }
                                ui.end_row();
                            }
                        });
                        ui.end_row();
                    });
                    ui.label(egui::RichText::new(format!("Original: {} × {}. New areas on the bottom layer use the secondary colour.", ow, oh)).weak());
                    out = merge(ok_cancel(ui), keys(ui));
                });
                if out == Outcome::Ok {
                    let (w, h, a) = (*w, *h, *anchor);
                    let sec = self.secondary;
                    self.docs[self.cur].canvas_size(w, h, a, sec);
                }
                out == Outcome::Open
            }
            Dialog::LayerProps { uid, idx, before, name, visible, opacity, blend } => {
                let Some(di) = self.doc_by_uid(*uid) else { return false };
                if *idx >= self.docs[di].layers.len() {
                    return false;
                }
                let mut out = Outcome::Open;
                egui::Modal::new("layerprops".into()).show(ctx, |ui| {
                    ui.set_width(320.0);
                    title_frame(ui, "Layer Properties");
                    egui::Grid::new("lp").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("Visible:");
                        ui.checkbox(visible, "");
                        ui.end_row();
                        ui.label("Blend mode:");
                        egui::ComboBox::from_id_salt("blend").selected_text(blend.name()).show_ui(ui, |ui| {
                            for b in LayerBlend::ALL {
                                ui.selectable_value(blend, b, b.name());
                            }
                        });
                        ui.end_row();
                        ui.label("Opacity:");
                        ui.add(egui::Slider::new(opacity, 0..=255));
                        ui.end_row();
                    });
                    out = ok_cancel(ui);
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        out = Outcome::Cancel;
                    }
                });
                let doc = &mut self.docs[di];
                let l = &mut doc.layers[*idx];
                match out {
                    Outcome::Cancel => {
                        l.name = before.name.clone();
                        l.visible = before.visible;
                        l.opacity = before.opacity;
                        l.blend = before.blend;
                        doc.changed();
                        false
                    }
                    Outcome::Ok => {
                        let changed = l.name != before.name || l.visible != before.visible || l.opacity != before.opacity || l.blend != before.blend;
                        if changed {
                            let mut snap = before.clone();
                            // the snapshot keeps the current pixels (unchanged) to save a copy
                            std::mem::swap(&mut snap.pixmap, &mut l.pixmap.clone());
                            doc.push_history("Layer Properties", Snapshot::Layer(*idx, snap));
                        }
                        false
                    }
                    _ => {
                        // live preview
                        if l.name != *name || l.visible != *visible || l.opacity != *opacity || l.blend != *blend {
                            l.name = name.clone();
                            l.visible = *visible;
                            l.opacity = *opacity;
                            l.blend = *blend;
                            doc.changed();
                        }
                        true
                    }
                }
            }
            Dialog::Filter { uid, idx, before, filter, applied, sel_curve } => {
                let Some(di) = self.doc_by_uid(*uid) else { return false };
                if *idx >= self.docs[di].layers.len() {
                    return false;
                }
                let mut out = Outcome::Open;
                let title = filter.name();
                egui::Modal::new("filter".into()).show(ctx, |ui| {
                    ui.set_min_width(320.0);
                    title_frame(ui, title);
                    filter_controls(ui, filter, sel_curve);
                    out = ok_cancel(ui);
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        out = Outcome::Cancel;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ui.ctx().wants_keyboard_input() {
                        out = Outcome::Ok;
                    }
                });
                let doc = &mut self.docs[di];
                if out != Outcome::Cancel && applied.as_ref() != Some(filter) {
                    let adj = raster::apply_filter(filter, &before.pixmap, doc.selection.as_ref());
                    let sel = doc.selection.clone();
                    raster::masked_mix(&before.pixmap, &adj, sel.as_ref(), &mut doc.layers[*idx].pixmap);
                    doc.changed();
                    *applied = Some(filter.clone());
                }
                match out {
                    Outcome::Cancel => {
                        doc.layers[*idx].pixmap = before.pixmap.clone();
                        doc.changed();
                        false
                    }
                    Outcome::Ok => {
                        let snap = Snapshot::Layer(*idx, before.clone());
                        doc.push_history(filter.name(), snap);
                        if filter.is_effect() {
                            self.last_effect = Some(filter.clone());
                        }
                        false
                    }
                    _ => true,
                }
            }
            Dialog::RotateZoom { uid, idx, before, angle, pan, zoom, applied } => {
                let Some(di) = self.doc_by_uid(*uid) else { return false };
                if *idx >= self.docs[di].layers.len() {
                    return false;
                }
                let mut out = Outcome::Open;
                egui::Modal::new("rotzoom".into()).show(ctx, |ui| {
                    ui.set_width(340.0);
                    title_frame(ui, "Layer Rotate / Zoom");
                    egui::Grid::new("rz").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        ui.label("Angle:");
                        ui.add(egui::Slider::new(angle, -180.0..=180.0).suffix("°"));
                        ui.end_row();
                        ui.label("Pan X:");
                        ui.add(egui::DragValue::new(&mut pan.0).speed(1.0).suffix(" px"));
                        ui.end_row();
                        ui.label("Pan Y:");
                        ui.add(egui::DragValue::new(&mut pan.1).speed(1.0).suffix(" px"));
                        ui.end_row();
                        ui.label("Zoom:");
                        ui.add(egui::Slider::new(zoom, 0.05..=16.0).logarithmic(true).suffix("×"));
                        ui.end_row();
                    });
                    if ui.button("Reset").clicked() {
                        *angle = 0.0;
                        *pan = (0.0, 0.0);
                        *zoom = 1.0;
                    }
                    out = merge(ok_cancel(ui), keys(ui));
                });
                let doc = &mut self.docs[di];
                let cur = (*angle, *pan, *zoom);
                if out != Outcome::Cancel && *applied != Some(cur) {
                    let sel = doc.selection.clone();
                    rotate_zoom_render(before, *angle, *pan, *zoom, sel.as_ref(), &mut doc.layers[*idx].pixmap);
                    doc.changed();
                    *applied = Some(cur);
                }
                match out {
                    Outcome::Cancel => {
                        doc.layers[*idx].pixmap = before.pixmap.clone();
                        doc.changed();
                        false
                    }
                    Outcome::Ok => {
                        doc.push_history("Rotate / Zoom Layer", Snapshot::Layer(*idx, before.clone()));
                        false
                    }
                    _ => true,
                }
            }
            Dialog::PasteExpand { img, new_layer } => {
                let mut choice = 0u8;
                egui::Modal::new("pasteexp".into()).show(ctx, |ui| {
                    ui.set_width(380.0);
                    title_frame(ui, "Paste");
                    ui.label(format!("The image being pasted ({} × {}) is larger than the canvas. What would you like to do?", img.width(), img.height()));
                    ui.add_space(8.0);
                    if ui.add_sized([360.0, 26.0], egui::Button::new("Expand canvas")).clicked() {
                        choice = 1;
                    }
                    if ui.add_sized([360.0, 26.0], egui::Button::new("Keep canvas size")).clicked() {
                        choice = 2;
                    }
                    if ui.add_sized([360.0, 26.0], egui::Button::new("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        choice = 3;
                    }
                });
                if choice == 0 {
                    return true;
                }
                if choice == 3 {
                    return false;
                }
                let img = std::mem::replace(img, raster::new_pixmap(1, 1));
                let nl = *new_layer;
                if choice == 1 {
                    let doc = &mut self.docs[self.cur];
                    let (nw, nh) = (doc.width.max(img.width()), doc.height.max(img.height()));
                    let sec = self.secondary;
                    doc.canvas_size(nw, nh, (0, 0), sec);
                }
                if nl {
                    self.docs[self.cur].add_layer();
                }
                self.begin_paste(img, 0, 0, if nl { "Paste Into New Layer" } else { "Paste" });
                false
            }
            Dialog::ConfirmClose { uid } => {
                let Some(di) = self.doc_by_uid(*uid) else {
                    return false;
                };
                let mut choice = 0u8;
                let name = self.docs[di].name.clone();
                let thumb = self.docs[di].doc_thumb.as_ref().map(|t| (t.id(), t.size_vec2()));
                egui::Modal::new("confirmclose".into()).show(ctx, |ui| {
                    ui.set_width(400.0);
                    title_frame(ui, "Unsaved Changes");
                    ui.horizontal(|ui| {
                        if let Some((id, sz)) = thumb {
                            ui.image((id, sz));
                        }
                        ui.label(format!("Save changes to \"{name}\" before closing?"));
                    });
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([90.0, 24.0], egui::Button::new("Cancel")).clicked() {
                            choice = 3;
                        }
                        if ui.add_sized([90.0, 24.0], egui::Button::new("Don't Save")).clicked() {
                            choice = 2;
                        }
                        if ui.add_sized([90.0, 24.0], egui::Button::new("Save")).clicked() {
                            choice = 1;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        choice = 3;
                    }
                });
                match choice {
                    1 => {
                        self.cur = di;
                        if self.save(di, false) {
                            self.close_doc(di);
                        } else {
                            self.quitting = false;
                        }
                        false
                    }
                    2 => {
                        self.close_doc(di);
                        false
                    }
                    3 => {
                        self.quitting = false;
                        false
                    }
                    _ => true,
                }
            }
        }
    }
}

#[allow(dead_code)]
fn _col(_c: Col) {}
