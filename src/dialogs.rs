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
    Filter { uid: u64, idx: usize, before: Layer, filter: Filter, initial: Filter, applied: Option<Filter>, sel_curve: Option<usize>, job: Option<FilterJob>, pad: Option<egui::TextureHandle> },
    RotateZoom { uid: u64, idx: usize, before: Layer, angle: f32, pan: (f32, f32), zoom: f32, applied: Option<(f32, (f32, f32), f32)> },
    PasteExpand { img: tiny_skia::Pixmap, new_layer: bool },
    ConfirmClose { uid: u64 },
    FlattenSave { uid: u64, path: std::path::PathBuf, close_after: bool },
    ExportJpeg { uid: u64, path: std::path::PathBuf, quality: u8 },
    Interactive(Box<crate::interactive::State>),
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
        Dialog::Filter { uid: doc.uid, idx: doc.active, before: doc.layers[doc.active].clone(), initial: f.clone(), filter: f, applied: None, sel_curve: None, job: None, pad: None }
    }
    pub fn rotate_zoom(doc: &Document) -> Self {
        Dialog::RotateZoom { uid: doc.uid, idx: doc.active, before: doc.layers[doc.active].clone(), angle: 0.0, pan: (0.0, 0.0), zoom: 1.0, applied: None }
    }
}

/// A filter preview rendering on a worker thread.
pub struct FilterJob {
    filter: Filter,
    rx: std::sync::mpsc::Receiver<tiny_skia::Pixmap>,
}

impl FilterJob {
    fn spawn(f: &Filter, src: &tiny_skia::Pixmap, sel: Option<&tiny_skia::Mask>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let (f2, src, sel) = (f.clone(), src.clone(), sel.cloned());
        std::thread::spawn(move || {
            let _ = tx.send(render_filter(&f2, &src, sel.as_ref()));
        });
        FilterJob { filter: f.clone(), rx }
    }
}

/// Filter applied to `src`, limited to the selection.
fn render_filter(f: &Filter, src: &tiny_skia::Pixmap, sel: Option<&tiny_skia::Mask>) -> tiny_skia::Pixmap {
    let adj = raster::apply_filter(f, src, sel);
    let mut out = src.clone();
    raster::masked_mix(src, &adj, sel, &mut out);
    out
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

/// Filter-specific controls. Returns nothing; mutates the filter.
fn filter_controls(ui: &mut Ui, f: &mut Filter, sel_curve: &mut Option<usize>, pad: Option<&egui::TextureHandle>) {
    if let Filter::Fx(s) = f {
        fx_controls(ui, s, pad);
        return;
    }
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
        Filter::CurvesPlus(cp) => curves_plus_controls(ui, cp, sel_curve),
        Filter::ColorMatch(cm) => color_match_controls(ui, cm),
        _ => {
            ui.label("No options");
            ui.end_row();
        }
    });
}

fn col32(c: Col) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Pseudo-random seed for the "Reseed" button.
fn new_seed() -> u32 {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(7) as u64;
    let mut x = n ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    (x ^ (x >> 29)) as u32 | 1
}

/// Circular angle chooser; returns true when changed.
fn angle_dial(ui: &mut Ui, deg: &mut f32) -> bool {
    let size = 34.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let c = rect.center();
    let r = size * 0.5 - 2.0;
    let v = ui.visuals();
    let p = ui.painter();
    p.circle(c, r, v.extreme_bg_color, v.widgets.inactive.bg_stroke);
    let a = deg.to_radians();
    let tip = c + vec2(a.cos(), -a.sin()) * (r - 2.0);
    let acc = crate::theme::pal(v.dark_mode).accent;
    p.line_segment([c, tip], Stroke::new(2.0f32, acc));
    p.circle_filled(tip, 3.0, acc);
    p.circle_filled(c, 2.0, v.text_color());
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let d = pos - c;
            if d.length() > 1.0 {
                let mut n = (-d.y).atan2(d.x).to_degrees();
                if ui.input(|i| i.modifiers.shift) {
                    n = (n / 15.0).round() * 15.0;
                }
                *deg = (n * 100.0).round() / 100.0;
                return true;
            }
        }
    }
    false
}

/// Point chooser: thumbnail of the layer with a draggable crosshair. Offsets are in -1..1.
fn point_pad(ui: &mut Ui, pt: &mut (f32, f32), pad: Option<&egui::TextureHandle>) {
    let aspect = pad.map(|t| t.size_vec2()).map(|s| s.y / s.x.max(1.0)).unwrap_or(0.75).clamp(0.25, 2.0);
    let w = if aspect > 1.0 { 180.0 / aspect } else { 180.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, w * aspect), Sense::click_and_drag());
    let v = ui.visuals().clone();
    let p = ui.painter();
    p.rect_filled(rect, 3.0, v.extreme_bg_color);
    if let Some(t) = pad {
        p.image(t.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
    p.rect_stroke(rect, 3.0, v.widgets.inactive.bg_stroke, egui::StrokeKind::Inside);
    if (resp.dragged() || resp.clicked()) && resp.interact_pointer_pos().is_some() {
        let pos = resp.interact_pointer_pos().unwrap();
        pt.0 = (((pos.x - rect.min.x) / rect.width()) * 2.0 - 1.0).clamp(-1.0, 1.0);
        pt.1 = (((pos.y - rect.min.y) / rect.height()) * 2.0 - 1.0).clamp(-1.0, 1.0);
    }
    let c = egui::pos2(rect.min.x + (pt.0 + 1.0) * 0.5 * rect.width(), rect.min.y + (pt.1 + 1.0) * 0.5 * rect.height());
    for (s, col) in [(3.0f32, Color32::from_black_alpha(160)), (1.0, Color32::WHITE)] {
        let st = Stroke::new(s, col);
        p.line_segment([egui::pos2(c.x - 9.0, c.y), egui::pos2(c.x + 9.0, c.y)], st);
        p.line_segment([egui::pos2(c.x, c.y - 9.0), egui::pos2(c.x, c.y + 9.0)], st);
        p.circle_stroke(c, 5.0, st);
    }
}

/// Generic controls generated from an effect's parameter specification.
fn fx_controls(ui: &mut Ui, s: &mut crate::fx::FxState, pad: Option<&egui::TextureHandle>) {
    use crate::fx::{P, V};
    let specs = s.kind.params();
    egui::Grid::new("fxgrid").num_columns(2).spacing(vec2(12.0, 8.0)).min_col_width(110.0).show(ui, |ui| {
        for (k, spec) in specs.iter().enumerate() {
            let val = &mut s.vals[k];
            if !matches!(spec, P::Seed | P::Bool(..)) {
                ui.label(spec.label());
            } else {
                ui.label("");
            }
            match (*spec, val) {
                (P::Int(_, lo, hi, _), V::I(v)) => {
                    ui.add(egui::Slider::new(v, lo..=hi));
                }
                (P::Float(_, lo, hi, _), V::F(v)) => {
                    let log = lo > 0.0 && hi / lo >= 100.0;
                    ui.add(egui::Slider::new(v, lo..=hi).logarithmic(log).max_decimals(2));
                }
                (P::Angle(..), V::F(v)) => {
                    ui.horizontal(|ui| {
                        angle_dial(ui, v);
                        ui.add(egui::Slider::new(v, -180.0..=180.0).suffix("°").max_decimals(2));
                    });
                }
                (P::Bool(l, _), V::B(v)) => {
                    ui.checkbox(v, l);
                }
                (P::Choice(_, opts, _), V::C(v)) => {
                    egui::ComboBox::from_id_salt(("fxc", k)).width(170.0).selected_text(opts.get(*v).copied().unwrap_or("")).show_ui(ui, |ui| {
                        for (i, o) in opts.iter().enumerate() {
                            ui.selectable_value(v, i, *o);
                        }
                    });
                }
                (P::Color(..), V::Col(c)) => {
                    let mut c32 = col32(*c);
                    if ui.color_edit_button_srgba(&mut c32).changed() {
                        let [r, g, b, a] = c32.to_srgba_unmultiplied();
                        *c = Col::rgba(r, g, b, a);
                    }
                }
                (P::Point(_), V::Pt(x, y)) => {
                    ui.vertical(|ui| {
                        let mut pt = (*x, *y);
                        point_pad(ui, &mut pt, pad);
                        ui.horizontal(|ui| {
                            ui.label("X");
                            ui.add(egui::DragValue::new(&mut pt.0).range(-1.0..=1.0).speed(0.005).max_decimals(2));
                            ui.label("Y");
                            ui.add(egui::DragValue::new(&mut pt.1).range(-1.0..=1.0).speed(0.005).max_decimals(2));
                            if ui.small_button("Center").clicked() {
                                pt = (0.0, 0.0);
                            }
                        });
                        (*x, *y) = pt;
                    });
                }
                (P::Seed, V::Seed(v)) => {
                    if ui.button("Reseed").clicked() {
                        *v = new_seed();
                    }
                }
                _ => {
                    ui.label("?");
                }
            }
            ui.end_row();
        }
    });
}

/// Downscaled copy of a layer for the point chooser.
fn pad_texture(ctx: &egui::Context, src: &tiny_skia::Pixmap) -> egui::TextureHandle {
    let s = (180.0 / src.width().max(src.height()) as f32).min(1.0);
    let (w, h) = (((src.width() as f32 * s).round() as u32).max(1), ((src.height() as f32 * s).round() as u32).max(1));
    let mut pm = tiny_skia::Pixmap::new(w, h).unwrap();
    // checkerboard so transparent layers are visible
    for y in 0..h {
        for x in 0..w {
            let g = if ((x / 6) + (y / 6)) % 2 == 0 { 255 } else { 220 };
            pm.pixels_mut()[(y * w + x) as usize] = tiny_skia::PremultipliedColorU8::from_rgba(g, g, g, 255).unwrap();
        }
    }
    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
    pm.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_scale(w as f32 / src.width() as f32, h as f32 / src.height() as f32), None);
    let img = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pm.data());
    ctx.load_texture("fxpad", img, egui::TextureOptions::LINEAR)
}

/// Curves+ dialog body: mode, channel toggles (or Advanced input/output), and the curve editor.
fn color_match_controls(ui: &mut Ui, cm: &mut crate::color_match::ColorMatch) {
    ui.label("Reference");
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            if ui.button("From clipboard").clicked() {
                let img = arboard::Clipboard::new().ok().and_then(|mut c| c.get_image().ok());
                match img {
                    Some(i) if i.width > 0 && i.height > 0 => {
                        let p = crate::raster::pixmap_from_straight(&i.bytes, i.width as u32, i.height as u32);
                        if !cm.set_reference(&p, "Clipboard image") {
                            cm.label = "Clipboard image is empty".into();
                        }
                    }
                    _ => cm.label = "No image on the clipboard".into(),
                }
            }
            if ui.button("From file...").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff", "ferrum", "pdn"]).pick_file() {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    match crate::fileio::load_flat(&path) {
                        Ok(p) if cm.set_reference(&p, &name) => {}
                        Ok(_) => cm.label = format!("{name} has no visible pixels"),
                        Err(e) => cm.label = e,
                    }
                }
            }
        });
        let shown = if cm.label.is_empty() { "Choose a reference image".to_string() } else { cm.label.clone() };
        ui.weak(shown);
    });
    ui.end_row();
    ui.label("Method");
    egui::ComboBox::from_id_salt("cm-mode").selected_text(crate::color_match::MODES[cm.mode.min(1)]).show_ui(ui, |ui| {
        for (i, m) in crate::color_match::MODES.iter().enumerate() {
            ui.selectable_value(&mut cm.mode, i, *m);
        }
    });
    ui.end_row();
    int_slider(ui, "Strength", &mut cm.strength, 0, 100);
    ui.label("");
    ui.add_enabled(cm.mode == 1, egui::Checkbox::new(&mut cm.keep_lightness, "Keep original lightness"));
    ui.end_row();
}

fn curves_plus_controls(ui: &mut Ui, cp: &mut crate::curves_plus::CurvesPlus, sel: &mut Option<usize>) {
    use crate::curves_plus::{CHANNELS, MODES};
    ui.label("Mode");
    let old = cp.mode;
    egui::ComboBox::from_id_salt("cp_mode").selected_text(MODES[cp.mode]).show_ui(ui, |ui| {
        for (i, m) in MODES.iter().enumerate() {
            ui.selectable_value(&mut cp.mode, i, *m);
        }
    });
    if cp.mode != old {
        cp.reset_curves();
        *sel = None;
    }
    ui.end_row();
    let names = cp.channel_names();
    if cp.mode == 5 {
        for (label, v, id) in [("Input", &mut cp.adv_in, "cp_in"), ("Output", &mut cp.adv_out, "cp_out")] {
            ui.label(label);
            egui::ComboBox::from_id_salt(id).selected_text(CHANNELS[*v]).show_ui(ui, |ui| {
                for (i, c) in CHANNELS.iter().enumerate() {
                    ui.selectable_value(v, i, *c);
                }
            });
            ui.end_row();
        }
    } else if names.len() > 1 {
        ui.label("Edit");
        ui.horizontal(|ui| {
            for (i, n) in names.iter().enumerate() {
                if ui.checkbox(&mut cp.edit[i], *n).changed() {
                    *sel = None;
                }
            }
        });
        ui.end_row();
    }
    let n = names.len();
    if !cp.edit[..n].iter().any(|&e| e) {
        cp.edit[0] = true;
    }
    let first = (0..n).find(|&i| cp.edit[i]).unwrap_or(0);
    ui.label("");
    let before = cp.curves[first].clone();
    curve_editor(ui, &mut cp.curves[first], sel);
    if cp.curves[first] != before {
        let c = cp.curves[first].clone();
        for i in 0..n {
            if cp.edit[i] {
                cp.curves[i] = c.clone();
            }
        }
    }
    ui.end_row();
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
        ("Ctrl+Alt+E", "Export a flattened PNG/JPEG copy (keeps your layered file)"),
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
        ("Ctrl+;", "Snap to grid (size in View menu / toolbar)"),
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
            Dialog::Filter { uid, idx, before, filter, initial, applied, sel_curve, job, pad } => {
                let Some(di) = self.doc_by_uid(*uid) else { return false };
                if *idx >= self.docs[di].layers.len() {
                    return false;
                }
                let needs_pad = matches!(filter, Filter::Fx(s) if s.kind.params().iter().any(|p| matches!(p, crate::fx::P::Point(_))));
                if needs_pad && pad.is_none() {
                    *pad = Some(pad_texture(ctx, &before.pixmap));
                }
                let mut out = Outcome::Open;
                let title = filter.name();
                let busy = job.is_some();
                egui::Modal::new("filter".into()).show(ctx, |ui| {
                    ui.set_min_width(360.0);
                    title_frame(ui, title);
                    egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() - 180.0).show(ui, |ui| {
                        filter_controls(ui, filter, sel_curve, pad.as_ref());
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("Reset").on_hover_text("Restore the default settings").clicked() {
                            *filter = initial.clone();
                            *sel_curve = None;
                        }
                        if busy {
                            ui.add(egui::Spinner::new().size(14.0));
                            ui.weak("Rendering…");
                        }
                    });
                    out = ok_cancel(ui);
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        out = Outcome::Cancel;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ui.ctx().wants_keyboard_input() {
                        out = Outcome::Ok;
                    }
                });
                let doc = &mut self.docs[di];
                if out != Outcome::Cancel {
                    // collect a finished preview
                    if let Some(j) = job.as_ref() {
                        match j.rx.try_recv() {
                            Ok(pix) => {
                                doc.layers[*idx].pixmap = pix;
                                doc.changed();
                                *applied = Some(j.filter.clone());
                                *job = None;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                *applied = Some(j.filter.clone());
                                *job = None;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {}
                        }
                    }
                    if out == Outcome::Ok && applied.as_ref() != Some(filter) {
                        // final result: wait for a matching render, or render now
                        let pix = match job.take() {
                            Some(j) if j.filter == *filter => j.rx.recv().ok(),
                            _ => None,
                        };
                        let sel = doc.selection.clone();
                        let pix = pix.unwrap_or_else(|| render_filter(filter, &before.pixmap, sel.as_ref()));
                        doc.layers[*idx].pixmap = pix;
                        doc.changed();
                        *applied = Some(filter.clone());
                    } else if job.is_none() && applied.as_ref() != Some(filter) {
                        *job = Some(FilterJob::spawn(filter, &before.pixmap, doc.selection.as_ref()));
                    }
                    if job.is_some() {
                        ctx.request_repaint_after(std::time::Duration::from_millis(15));
                    }
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
            }            Dialog::Interactive(st) => {
                let Some(di) = self.doc_by_uid(st.uid) else { return false };
                if st.idx >= self.docs[di].layers.len() {
                    return false;
                }
                match st.show(ctx) {
                    crate::interactive::Outcome::Open => true,
                    crate::interactive::Outcome::Cancel => false,
                    crate::interactive::Outcome::Ok => {
                        let doc = &mut self.docs[di];
                        let before = doc.layers[st.idx].clone();
                        let res = st.result();
                        let mut out = before.pixmap.clone();
                        raster::masked_mix(&before.pixmap, &res, doc.selection.as_ref(), &mut out);
                        doc.layers[st.idx].pixmap = out;
                        doc.changed();
                        doc.push_history(st.kind.name(), Snapshot::Layer(st.idx, before));
                        false
                    }
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
            Dialog::FlattenSave { uid, path, close_after } => {
                let Some(di) = self.doc_by_uid(*uid) else { return false };
                let mut choice = 0u8;
                let n = self.docs[di].layers.len();
                let fname = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                egui::Modal::new("flattensave".into()).show(ctx, |ui| {
                    ui.set_width(440.0);
                    title_frame(ui, "Flatten Image");
                    ui.label(format!(
                        "\"{fname}\" is a flat format, so the {n} layers will be merged in the saved file.\n\n\
                         Your open image keeps its layers. To be able to reopen it later with layers intact, \
                         save it as a Ferrum Paint image (.ferrum) instead."
                    ));
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([80.0, 26.0], egui::Button::new("Cancel")).clicked() {
                            choice = 3;
                        }
                        if ui.add_sized([110.0, 26.0], egui::Button::new("Flatten & Save")).clicked() {
                            choice = 2;
                        }
                        if ui.add_sized([170.0, 26.0], egui::Button::new("Save as .ferrum instead")).clicked() {
                            choice = 1;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        choice = 3;
                    }
                });
                let target = match choice {
                    1 => Some(path.with_extension("ferrum")),
                    2 => Some(path.clone()),
                    3 => {
                        self.quitting = false;
                        return false;
                    }
                    _ => return true,
                };
                if let Some(t) = target {
                    if self.write_file(di, t) && *close_after {
                        self.close_doc(di);
                    } else if *close_after {
                        self.quitting = false;
                    }
                }
                false
            }
            Dialog::ExportJpeg { uid, path, quality } => {
                let Some(di) = self.doc_by_uid(*uid) else { return false };
                let mut choice = 0u8;
                let fname = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                egui::Modal::new("exportjpeg".into()).show(ctx, |ui| {
                    ui.set_width(360.0);
                    title_frame(ui, "JPEG Quality");
                    ui.label(format!("Exporting \"{fname}\". Layers are flattened and transparency becomes white in the exported file only."));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Quality:");
                        ui.add(egui::Slider::new(quality, 1..=100));
                    });
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([80.0, 26.0], egui::Button::new("Cancel")).clicked() {
                            choice = 2;
                        }
                        if ui.add_sized([80.0, 26.0], egui::Button::new("OK")).clicked() {
                            choice = 1;
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        choice = 2;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        choice = 1;
                    }
                });
                match choice {
                    1 => {
                        let (p, q) = (path.clone(), *quality);
                        self.jpeg_quality = q;
                        crate::theme::save_pref("jpeg_quality", &q.to_string());
                        self.write_export(di, p, q);
                        false
                    }
                    2 => false,
                    _ => true,
                }
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
                        if self.save_ext(di, false, true) {
                            self.close_doc(di);
                        } else if !matches!(self.dialog, Some(Dialog::FlattenSave { .. })) {
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
