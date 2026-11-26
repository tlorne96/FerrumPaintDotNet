//! Colors window: primary/secondary swatches, HSV wheel, sliders, hex entry and palette.

use crate::app::App;
use crate::raster::{self, Col};
use egui::{epaint::Mesh, pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui};

fn checker(ui: &Ui, r: Rect) {
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::WHITE);
    let s = 5.0;
    let mut y = r.min.y;
    let mut row = 0;
    while y < r.max.y {
        let mut x = r.min.x + if row % 2 == 0 { 0.0 } else { s };
        while x < r.max.x {
            let c = Rect::from_min_max(pos2(x, y), pos2((x + s).min(r.max.x), (y + s).min(r.max.y)));
            p.rect_filled(c, 0.0, Color32::from_gray(200));
            x += 2.0 * s;
        }
        y += s;
        row += 1;
    }
}

fn outline(ui: &Ui, r: Rect, s: Stroke) {
    ui.painter().add(Shape::closed_line(vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()], s));
}

fn swatch(ui: &Ui, r: Rect, c: Col) {
    checker(ui, r);
    ui.painter().rect_filled(r, 0.0, c.egui());
    outline(ui, r, Stroke::new(1.0f32, Color32::from_gray(60)));
}

/// A horizontal gradient slider (0..=1) with a triangle marker.
fn grad_slider(ui: &mut Ui, label: &str, v: &mut f32, stops: &dyn Fn(f32) -> Color32, alpha_bg: bool, max: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([14.0, 16.0], egui::Label::new(label));
        let (r, resp) = ui.allocate_exact_size(vec2(150.0, 14.0), Sense::click_and_drag());
        if alpha_bg {
            checker(ui, r);
        }
        let mut mesh = Mesh::default();
        let n = 24;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = r.min.x + t * r.width();
            let c = stops(t);
            mesh.colored_vertex(pos2(x, r.min.y), c);
            mesh.colored_vertex(pos2(x, r.max.y), c);
            if i > 0 {
                let k = (i * 2) as u32;
                mesh.add_triangle(k - 2, k - 1, k);
                mesh.add_triangle(k - 1, k, k + 1);
            }
        }
        ui.painter().add(Shape::mesh(mesh));
        outline(ui, r, Stroke::new(1.0f32, Color32::from_gray(90)));
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on() {
                let t = ((p.x - r.min.x) / r.width()).clamp(0.0, 1.0);
                if (t - *v / max).abs() > f32::EPSILON {
                    *v = t * max;
                    changed = true;
                }
            }
        }
        let mx = r.min.x + (*v / max).clamp(0.0, 1.0) * r.width();
        ui.painter().add(Shape::convex_polygon(
            vec![pos2(mx, r.max.y - 4.0), pos2(mx + 5.0, r.max.y + 4.0), pos2(mx - 5.0, r.max.y + 4.0)],
            Color32::from_gray(30),
            Stroke::new(1.0f32, Color32::WHITE),
        ));
        let mut iv = v.round() as i32;
        if ui.add(egui::DragValue::new(&mut iv).range(0..=max as i32).speed(1.0)).changed() {
            *v = iv as f32;
            changed = true;
        }
    });
    changed
}

impl App {
    fn active_col(&self) -> Col {
        if self.active_slot_secondary {
            self.secondary
        } else {
            self.primary
        }
    }

    fn set_slot(&mut self, secondary: bool, c: Col) {
        if secondary {
            self.secondary = c;
        } else {
            self.primary = c;
        }
    }

    /// Set active colour from the HSV cache (keeps hue stable at zero saturation).
    fn set_from_hsv(&mut self, a: u8) {
        let (r, g, b) = raster::hsv_to_rgb(self.hsv.0, self.hsv.1, self.hsv.2);
        let c = Col::rgba(r, g, b, a);
        self.set_slot(self.active_slot_secondary, c);
        self.hsv_for = c;
        self.hex_edit = c.to_hex();
    }

    pub fn colors_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                // Swatches
                let (area, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
                let pr = Rect::from_min_size(area.min + vec2(2.0, 2.0), vec2(34.0, 34.0));
                let sr = Rect::from_min_size(area.min + vec2(24.0, 24.0), vec2(34.0, 34.0));
                let sresp = ui.interact(sr, ui.id().with("sec_sw"), Sense::click());
                let presp = ui.interact(pr, ui.id().with("pri_sw"), Sense::click());
                swatch(ui, sr, self.secondary);
                swatch(ui, pr, self.primary);
                let act = if self.active_slot_secondary { sr } else { pr };
                outline(ui, act.expand(2.0), Stroke::new(2.0f32, Color32::from_rgb(40, 110, 220)));
                if presp.clicked() {
                    self.active_slot_secondary = false;
                }
                if sresp.clicked() {
                    self.active_slot_secondary = true;
                }
                presp.on_hover_text("Primary colour (click to edit)");
                sresp.on_hover_text("Secondary colour (click to edit)");
                // swap and reset
                let swap_r = Rect::from_min_size(area.min + vec2(42.0, 2.0), vec2(18.0, 18.0));
                let sw = ui.interact(swap_r, ui.id().with("swap"), Sense::click()).on_hover_text("Swap colours (X)");
                ui.painter().text(swap_r.center(), egui::Align2::CENTER_CENTER, "⇄", egui::FontId::proportional(15.0), if sw.hovered() { Color32::from_rgb(40, 110, 220) } else { Color32::from_gray(50) });
                if sw.clicked() {
                    std::mem::swap(&mut self.primary, &mut self.secondary);
                }
                let rr = Rect::from_min_size(area.min + vec2(2.0, 44.0), vec2(16.0, 16.0));
                let rs = ui.interact(rr, ui.id().with("reset"), Sense::click()).on_hover_text("Reset to black and white");
                swatch(ui, Rect::from_min_size(rr.min + vec2(5.0, 5.0), vec2(9.0, 9.0)), Col::WHITE);
                swatch(ui, Rect::from_min_size(rr.min, vec2(9.0, 9.0)), Col::BLACK);
                if rs.clicked() {
                    self.primary = Col::BLACK;
                    self.secondary = Col::WHITE;
                }
                ui.add_space(4.0);
                let more = if self.colors_more { "Less <<" } else { "More >>" };
                if ui.button(more).clicked() {
                    self.colors_more = !self.colors_more;
                }
            });
            self.sync_hsv();
            self.wheel(ui);
            ui.vertical(|ui| {
                // value strip
                let (r, resp) = ui.allocate_exact_size(vec2(16.0, 150.0), Sense::click_and_drag());
                let (rr, gg, bb) = raster::hsv_to_rgb(self.hsv.0, self.hsv.1, 1.0);
                let mut mesh = Mesh::default();
                mesh.colored_vertex(r.left_top(), Color32::from_rgb(rr, gg, bb));
                mesh.colored_vertex(r.right_top(), Color32::from_rgb(rr, gg, bb));
                mesh.colored_vertex(r.left_bottom(), Color32::BLACK);
                mesh.colored_vertex(r.right_bottom(), Color32::BLACK);
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(1, 2, 3);
                ui.painter().add(Shape::mesh(mesh));
                outline(ui, r, Stroke::new(1.0f32, Color32::from_gray(90)));
                let y = r.max.y - self.hsv.2 * r.height();
                ui.painter().line_segment([pos2(r.min.x - 3.0, y), pos2(r.max.x + 3.0, y)], Stroke::new(2.0f32, Color32::from_gray(20)));
                if let Some(p) = resp.interact_pointer_pos() {
                    if resp.is_pointer_button_down_on() {
                        self.hsv.2 = ((r.max.y - p.y) / r.height()).clamp(0.0, 1.0);
                        let a = self.active_col().a;
                        self.set_from_hsv(a);
                    }
                }
                resp.on_hover_text("Value (brightness)");
            });
        });
        // Alpha slider always visible (like the compact colour window)
        let cur = self.active_col();
        let mut a = cur.a as f32;
        let opaque = Col { a: 255, ..cur };
        if grad_slider(ui, "A", &mut a, &|t| { let c = opaque.egui(); Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (t * 255.0) as u8) }, true, 255.0) {
            let c = Col { a: a.round() as u8, ..cur };
            self.set_slot(self.active_slot_secondary, c);
            self.hsv_for = c;
            self.hex_edit = c.to_hex();
        }
        if self.colors_more {
            ui.separator();
            let cur = self.active_col();
            let (mut r, mut g, mut b) = (cur.r as f32, cur.g as f32, cur.b as f32);
            let mut ch = false;
            ch |= grad_slider(ui, "R", &mut r, &|t| Color32::from_rgb((t * 255.0) as u8, cur.g, cur.b), false, 255.0);
            ch |= grad_slider(ui, "G", &mut g, &|t| Color32::from_rgb(cur.r, (t * 255.0) as u8, cur.b), false, 255.0);
            ch |= grad_slider(ui, "B", &mut b, &|t| Color32::from_rgb(cur.r, cur.g, (t * 255.0) as u8), false, 255.0);
            if ch {
                let c = Col::rgba(r.round() as u8, g.round() as u8, b.round() as u8, cur.a);
                self.set_slot(self.active_slot_secondary, c);
            }
            let (hh, ss, vv) = self.hsv;
            let (mut h, mut s, mut v) = (hh, ss * 100.0, vv * 100.0);
            let mut hch = false;
            hch |= grad_slider(ui, "H", &mut h, &|t| { let (r, g, b) = raster::hsv_to_rgb(t * 360.0, 1.0, 1.0); Color32::from_rgb(r, g, b) }, false, 360.0);
            hch |= grad_slider(ui, "S", &mut s, &|t| { let (r, g, b) = raster::hsv_to_rgb(hh, t, vv); Color32::from_rgb(r, g, b) }, false, 100.0);
            hch |= grad_slider(ui, "V", &mut v, &|t| { let (r, g, b) = raster::hsv_to_rgb(hh, ss, t); Color32::from_rgb(r, g, b) }, false, 100.0);
            if hch {
                self.hsv = (h.min(359.99), s / 100.0, v / 100.0);
                self.set_from_hsv(cur.a);
            }
            ui.horizontal(|ui| {
                ui.label("Hex:");
                let r = ui.add(egui::TextEdit::singleline(&mut self.hex_edit).desired_width(80.0).char_limit(9));
                if r.lost_focus() || r.changed() {
                    if let Some(c) = Col::from_hex(&self.hex_edit) {
                        let c = if self.hex_edit.trim_start_matches('#').len() <= 6 { Col { a: cur.a, ..c } } else { c };
                        if c != cur {
                            self.set_slot(self.active_slot_secondary, c);
                            let (h, s, v) = raster::rgb_to_hsv(c.r, c.g, c.b);
                            self.hsv = (h, s, v);
                            self.hsv_for = c;
                        }
                    } else if r.lost_focus() {
                        self.hex_edit = cur.to_hex();
                    }
                }
            });
        }
        ui.separator();
        // palette
        let cols = 16;
        let cell = 15.0;
        let mut set: Option<(bool, Col)> = None;
        egui::Grid::new("palette").min_col_width(cell).spacing(vec2(1.0, 1.0)).show(ui, |ui| {
            for (i, &c) in self.palette.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(cell, cell), Sense::click());
                ui.painter().rect_filled(r, 0.0, c.egui());
                if resp.hovered() {
                    outline(ui, r, Stroke::new(1.0f32, Color32::BLACK));
                }
                if resp.clicked() {
                    set = Some((false, c));
                }
                if resp.secondary_clicked() {
                    set = Some((true, c));
                }
                resp.on_hover_text(format!("#{}  (left: primary, right: secondary)", c.to_hex()));
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
        if let Some((sec, c)) = set {
            self.set_slot(sec, c);
        }
    }

    fn wheel(&mut self, ui: &mut Ui) {
        let size = 150.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
        let c = rect.center();
        let rad = size / 2.0 - 3.0;
        let mut mesh = Mesh::default();
        let segs = 72;
        let rings = 8;
        let col_at = |h: f32, s: f32| {
            let (r, g, b) = raster::hsv_to_rgb(h, s, 1.0);
            Color32::from_rgb(r, g, b)
        };
        // vertices: ring k (0 = centre) x segs
        mesh.colored_vertex(c, col_at(0.0, 0.0));
        for k in 1..=rings {
            let s = k as f32 / rings as f32;
            for i in 0..segs {
                let h = i as f32 / segs as f32 * 360.0;
                let a = (-h).to_radians();
                mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * rad * s, col_at(h, s));
            }
        }
        let idx = |k: u32, i: u32| -> u32 { 1 + (k - 1) * segs + (i % segs) };
        for i in 0..segs {
            mesh.add_triangle(0, idx(1, i), idx(1, i + 1));
        }
        for k in 1..rings {
            for i in 0..segs {
                mesh.add_triangle(idx(k, i), idx(k + 1, i), idx(k + 1, i + 1));
                mesh.add_triangle(idx(k, i), idx(k, i + 1), idx(k + 1, i + 1));
            }
        }
        ui.painter().add(Shape::mesh(mesh));
        ui.painter().circle_stroke(c, rad, Stroke::new(1.0f32, Color32::from_gray(110)));
        // marker
        let a = (-self.hsv.0).to_radians();
        let mp: Pos2 = c + vec2(a.cos(), a.sin()) * rad * self.hsv.1;
        ui.painter().circle_stroke(mp, 4.5, Stroke::new(2.0f32, Color32::BLACK));
        ui.painter().circle_stroke(mp, 3.0, Stroke::new(1.0f32, Color32::WHITE));
        if let Some(p) = resp.interact_pointer_pos() {
            if resp.is_pointer_button_down_on() {
                let d = p - c;
                let (ctrl, alt, right) = ui.input(|i| (i.modifiers.ctrl, i.modifiers.alt, i.pointer.secondary_down()));
                let mut h = (-d.y.atan2(d.x)).to_degrees().rem_euclid(360.0);
                let mut s = (d.length() / rad).clamp(0.0, 1.0);
                if ctrl {
                    s = self.hsv.1; // hue only
                }
                if alt {
                    h = self.hsv.0; // saturation only
                }
                if right {
                    // right button edits the inactive slot
                    let other = !self.active_slot_secondary;
                    let oc = if other { self.secondary } else { self.primary };
                    let (_, _, ov) = raster::rgb_to_hsv(oc.r, oc.g, oc.b);
                    let (r, g, b) = raster::hsv_to_rgb(h, s, if ov == 0.0 { 1.0 } else { ov });
                    self.set_slot(other, Col::rgba(r, g, b, oc.a));
                } else {
                    self.hsv.0 = h;
                    self.hsv.1 = s;
                    if self.hsv.2 == 0.0 {
                        self.hsv.2 = 1.0;
                    }
                    let a = self.active_col().a;
                    self.set_from_hsv(a);
                }
            }
        }
        resp.on_hover_text("Left: active colour, right: other colour. Ctrl = hue only, Alt = saturation only.");
    }
}
