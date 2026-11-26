//! Original, procedurally drawn icons. No bitmap assets are used anywhere in Ferrum Paint.

use crate::tools::{ShapeKind, Tool};
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Tool(Tool),
    New,
    Open,
    Save,
    Cut,
    Copy,
    Paste,
    Crop,
    Deselect,
    Undo,
    Redo,
    Grid,
    Snap,
    LayerAdd,
    LayerDelete,
    LayerDuplicate,
    LayerMerge,
    LayerUp,
    LayerDown,
    LayerProps,
    Shape(ShapeKind),
    Tools,
    History,
    Layers,
    Colors,
}

const INK: Color32 = Color32::from_rgb(40, 44, 52);
const RUST: Color32 = Color32::from_rgb(196, 88, 40);
const BLUE: Color32 = Color32::from_rgb(52, 120, 210);
const GREEN: Color32 = Color32::from_rgb(60, 160, 90);
const GOLD: Color32 = Color32::from_rgb(222, 170, 40);
const RED: Color32 = Color32::from_rgb(210, 60, 60);
const PAPER: Color32 = Color32::from_rgb(250, 250, 246);

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    dark: bool,
}

impl Pen<'_> {
    /// Theme adaptation: near-black neutral ink becomes light grey on dark backgrounds.
    fn k(&self, c: Color32) -> Color32 {
        if !self.dark || c.a() == 0 {
            return c;
        }
        let (r, g, b) = (c.r() as i32, c.g() as i32, c.b() as i32);
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        if mx < 90 && mx - mn < 30 {
            let l = (228 - mx) as u8;
            Color32::from_rgba_unmultiplied(l, l, l.saturating_add(6), c.a())
        } else {
            c
        }
    }
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.r.min.x + x * self.r.width(), self.r.min.y + y * self.r.height())
    }
    fn s(&self, w: f32) -> f32 {
        w * self.r.width() / 24.0
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), w: f32, c: Color32) {
        self.p.line_segment([self.at(a.0, a.1), self.at(b.0, b.1)], Stroke::new(self.s(w), self.k(c)));
    }
    fn poly(&self, pts: &[(f32, f32)], fill: Color32, w: f32, c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::convex_polygon(v, self.k(fill), Stroke::new(self.s(w), self.k(c))));
    }
    fn path(&self, pts: &[(f32, f32)], w: f32, c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::line(v, Stroke::new(self.s(w), self.k(c))));
    }
    fn closed(&self, pts: &[(f32, f32)], w: f32, c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::closed_line(v, Stroke::new(self.s(w), self.k(c))));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32, w: f32, c: Color32) {
        self.poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], fill, w, c);
    }
    fn dashed_rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        let pts = [self.at(x0, y0), self.at(x1, y0), self.at(x1, y1), self.at(x0, y1), self.at(x0, y0)];
        let d = self.s(2.0);
        self.p.extend(Shape::dashed_line(&pts, Stroke::new(self.s(1.3), self.k(c)), d, d));
    }
    fn circle(&self, x: f32, y: f32, r: f32, fill: Color32, w: f32, c: Color32) {
        let rr = r * self.r.width();
        self.p.circle(self.at(x, y), rr, self.k(fill), Stroke::new(self.s(w), self.k(c)));
    }
    fn ellipse_pts(&self, cx: f32, cy: f32, rx: f32, ry: f32, n: usize) -> Vec<(f32, f32)> {
        (0..n)
            .map(|i| {
                let t = i as f32 / n as f32 * std::f32::consts::TAU;
                (cx + rx * t.cos(), cy + ry * t.sin())
            })
            .collect()
    }
    fn arrow_head(&self, tip: (f32, f32), dir: (f32, f32), size: f32, c: Color32) {
        let l = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1e-3);
        let (dx, dy) = (dir.0 / l, dir.1 / l);
        let (px, py) = (-dy, dx);
        let b = (tip.0 - dx * size, tip.1 - dy * size);
        self.poly(
            &[tip, (b.0 + px * size * 0.6, b.1 + py * size * 0.6), (b.0 - px * size * 0.6, b.1 - py * size * 0.6)],
            c,
            0.0,
            c,
        );
    }
}

pub fn shape_outline(kind: ShapeKind, x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
    let (w, h) = (x1 - x0, y1 - y0);
    let at = |u: f32, v: f32| (x0 + u * w, y0 + v * h);
    let reg = |n: usize, rot: f32| -> Vec<(f32, f32)> {
        (0..n)
            .map(|i| {
                let t = rot + i as f32 / n as f32 * std::f32::consts::TAU;
                at(0.5 + 0.5 * t.cos(), 0.5 + 0.5 * t.sin())
            })
            .collect()
    };
    use std::f32::consts::FRAC_PI_2;
    match kind {
        ShapeKind::Rectangle | ShapeKind::RoundedRectangle => vec![at(0., 0.), at(1., 0.), at(1., 1.), at(0., 1.)],
        ShapeKind::Ellipse => reg(32, 0.0),
        ShapeKind::Triangle => vec![at(0.5, 0.), at(1., 1.), at(0., 1.)],
        ShapeKind::RightTriangle => vec![at(0., 0.), at(1., 1.), at(0., 1.)],
        ShapeKind::Diamond => vec![at(0.5, 0.), at(1., 0.5), at(0.5, 1.), at(0., 0.5)],
        ShapeKind::Pentagon => reg(5, -FRAC_PI_2),
        ShapeKind::Hexagon => reg(6, 0.0),
        ShapeKind::Octagon => reg(8, std::f32::consts::PI / 8.0),
        ShapeKind::Star => (0..10)
            .map(|i| {
                let t = -FRAC_PI_2 + i as f32 / 10.0 * std::f32::consts::TAU;
                let r = if i % 2 == 0 { 0.5 } else { 0.2 };
                at(0.5 + r * t.cos(), 0.55 + r * t.sin())
            })
            .collect(),
        ShapeKind::Arrow => vec![
            at(0., 0.3),
            at(0.6, 0.3),
            at(0.6, 0.),
            at(1., 0.5),
            at(0.6, 1.),
            at(0.6, 0.7),
            at(0., 0.7),
        ],
        ShapeKind::Heart => {
            let mut v = vec![];
            for i in 0..40 {
                let t = i as f32 / 40.0 * std::f32::consts::TAU;
                let hx = 16.0 * t.sin().powi(3);
                let hy = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                v.push(at(0.5 + hx / 34.0, 0.45 - hy / 32.0));
            }
            v
        }
        ShapeKind::Cross => vec![
            at(0.33, 0.),
            at(0.67, 0.),
            at(0.67, 0.33),
            at(1., 0.33),
            at(1., 0.67),
            at(0.67, 0.67),
            at(0.67, 1.),
            at(0.33, 1.),
            at(0.33, 0.67),
            at(0., 0.67),
            at(0., 0.33),
            at(0.33, 0.33),
        ],
        ShapeKind::SpeechBubble => vec![
            at(0., 0.),
            at(1., 0.),
            at(1., 0.7),
            at(0.45, 0.7),
            at(0.2, 1.),
            at(0.25, 0.7),
            at(0., 0.7),
        ],
    }
}

/// Draw an icon into `r` (designed on a 24x24 grid).
pub fn paint_icon(p: &Painter, r: Rect, icon: Icon) {
    let k = Pen { p, r, dark: p.ctx().style().visuals.dark_mode };
    match icon {
        Icon::Tool(t) => paint_tool(&k, t),
        Icon::Shape(s) => {
            let pts = shape_outline(s, 0.12, 0.18, 0.88, 0.82);
            k.closed(&pts, 1.6, INK);
        }
        Icon::New => {
            k.poly(&[(0.22, 0.1), (0.62, 0.1), (0.8, 0.28), (0.8, 0.9), (0.22, 0.9)], PAPER, 1.3, INK);
            k.line((0.62, 0.1), (0.62, 0.28), 1.2, INK);
            k.line((0.62, 0.28), (0.8, 0.28), 1.2, INK);
            k.circle(0.75, 0.78, 0.13, GOLD, 0.0, GOLD);
        }
        Icon::Open => {
            k.poly(&[(0.08, 0.25), (0.38, 0.25), (0.45, 0.33), (0.85, 0.33), (0.85, 0.82), (0.08, 0.82)], GOLD, 1.2, INK);
            k.poly(&[(0.18, 0.45), (0.95, 0.45), (0.85, 0.82), (0.08, 0.82)], Color32::from_rgb(245, 205, 90), 1.2, INK);
        }
        Icon::Save => {
            k.rect(0.14, 0.14, 0.86, 0.86, BLUE, 1.2, INK);
            k.rect(0.28, 0.14, 0.72, 0.4, PAPER, 1.0, INK);
            k.rect(0.3, 0.58, 0.7, 0.86, Color32::from_rgb(220, 230, 245), 1.0, INK);
        }
        Icon::Cut => {
            k.circle(0.3, 0.75, 0.12, Color32::TRANSPARENT, 1.6, RED);
            k.circle(0.7, 0.75, 0.12, Color32::TRANSPARENT, 1.6, RED);
            k.line((0.36, 0.65), (0.68, 0.12), 1.6, INK);
            k.line((0.64, 0.65), (0.32, 0.12), 1.6, INK);
        }
        Icon::Copy => {
            k.rect(0.12, 0.1, 0.6, 0.66, PAPER, 1.2, INK);
            k.rect(0.38, 0.34, 0.86, 0.9, PAPER, 1.2, INK);
        }
        Icon::Paste => {
            k.rect(0.16, 0.16, 0.76, 0.9, Color32::from_rgb(190, 140, 90), 1.2, INK);
            k.rect(0.34, 0.08, 0.58, 0.24, Color32::from_rgb(160, 160, 170), 1.0, INK);
            k.rect(0.42, 0.38, 0.9, 0.94, PAPER, 1.2, INK);
        }
        Icon::Crop => {
            k.path(&[(0.28, 0.06), (0.28, 0.72), (0.94, 0.72)], 2.0, INK);
            k.path(&[(0.06, 0.28), (0.72, 0.28), (0.72, 0.94)], 2.0, RUST);
        }
        Icon::Deselect => {
            k.dashed_rect(0.12, 0.12, 0.88, 0.88, INK);
            k.line((0.3, 0.3), (0.7, 0.7), 1.8, RED);
            k.line((0.7, 0.3), (0.3, 0.7), 1.8, RED);
        }
        Icon::Undo | Icon::Redo => {
            let flip = |x: f32| if icon == Icon::Redo { 1.0 - x } else { x };
            let pts: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let t = std::f32::consts::PI * (1.1 - i as f32 / 12.0 * 1.1);
                    (flip(0.52 + 0.3 * t.cos()), 0.56 - 0.3 * t.sin())
                })
                .collect();
            k.path(&pts, 2.0, BLUE);
            let tip = pts[0];
            k.arrow_head((tip.0, tip.1 + 0.08), (0.0, 1.0), 0.18, BLUE);
        }
        Icon::Grid => {
            for i in 0..4 {
                let v = 0.15 + i as f32 * 0.233;
                k.line((v, 0.15), (v, 0.85), 1.0, INK);
                k.line((0.15, v), (0.85, v), 1.0, INK);
            }
        }
        Icon::Snap => {
            // dot grid with a rectangle whose corners sit on grid points
            for i in 0..4 {
                for j in 0..4 {
                    k.circle(0.14 + i as f32 * 0.24, 0.14 + j as f32 * 0.24, 0.035, INK, 0.0, INK);
                }
            }
            k.rect(0.38, 0.38, 0.86, 0.86, Color32::from_rgba_unmultiplied(70, 130, 220, 70), 1.6, BLUE);
            k.circle(0.38, 0.38, 0.07, BLUE, 0.0, BLUE);
        }
        Icon::LayerAdd | Icon::LayerDelete | Icon::LayerDuplicate | Icon::LayerMerge | Icon::LayerUp | Icon::LayerDown | Icon::LayerProps => {
            k.poly(&[(0.1, 0.55), (0.5, 0.35), (0.9, 0.55), (0.5, 0.75)], Color32::from_rgb(170, 190, 220), 1.0, INK);
            k.poly(&[(0.1, 0.4), (0.5, 0.2), (0.9, 0.4), (0.5, 0.6)], Color32::from_rgb(120, 160, 220), 1.0, INK);
            match icon {
                Icon::LayerAdd => {
                    k.line((0.78, 0.66), (0.78, 0.98), 2.2, GREEN);
                    k.line((0.62, 0.82), (0.94, 0.82), 2.2, GREEN);
                }
                Icon::LayerDelete => {
                    k.line((0.64, 0.68), (0.94, 0.98), 2.2, RED);
                    k.line((0.94, 0.68), (0.64, 0.98), 2.2, RED);
                }
                Icon::LayerDuplicate => {
                    k.rect(0.6, 0.66, 0.96, 0.98, PAPER, 1.0, INK);
                }
                Icon::LayerMerge => {
                    k.line((0.8, 0.62), (0.8, 0.92), 2.0, GREEN);
                    k.arrow_head((0.8, 1.0), (0.0, 1.0), 0.14, GREEN);
                }
                Icon::LayerUp => {
                    k.line((0.82, 0.98), (0.82, 0.7), 2.0, BLUE);
                    k.arrow_head((0.82, 0.6), (0.0, -1.0), 0.14, BLUE);
                }
                Icon::LayerDown => {
                    k.line((0.82, 0.6), (0.82, 0.88), 2.0, BLUE);
                    k.arrow_head((0.82, 0.98), (0.0, 1.0), 0.14, BLUE);
                }
                _ => {
                    k.circle(0.8, 0.82, 0.13, GOLD, 1.0, INK);
                    k.circle(0.8, 0.82, 0.04, INK, 0.0, INK);
                }
            }
        }
        Icon::Tools => {
            k.line((0.2, 0.8), (0.6, 0.4), 2.4, RUST);
            k.circle(0.68, 0.32, 0.15, Color32::TRANSPARENT, 2.0, INK);
        }
        Icon::History => {
            k.circle(0.5, 0.5, 0.36, Color32::TRANSPARENT, 1.6, INK);
            k.line((0.5, 0.5), (0.5, 0.26), 1.6, INK);
            k.line((0.5, 0.5), (0.68, 0.6), 1.6, INK);
        }
        Icon::Layers => {
            paint_icon(p, r, Icon::LayerProps);
        }
        Icon::Colors => {
            k.circle(0.38, 0.4, 0.22, RED, 0.0, RED);
            k.circle(0.62, 0.4, 0.22, Color32::from_rgba_unmultiplied(40, 160, 60, 200), 0.0, GREEN);
            k.circle(0.5, 0.62, 0.22, Color32::from_rgba_unmultiplied(50, 110, 220, 200), 0.0, BLUE);
        }
    }
}

fn paint_tool(k: &Pen, t: Tool) {
    match t {
        Tool::RectSelect => k.dashed_rect(0.14, 0.2, 0.86, 0.8, INK),
        Tool::EllipseSelect => {
            let mut pts: Vec<Pos2> = k.ellipse_pts(0.5, 0.5, 0.38, 0.3, 28).iter().map(|&(x, y)| k.at(x, y)).collect();
            pts.push(pts[0]);
            let d = k.s(2.0);
            k.p.extend(Shape::dashed_line(&pts, Stroke::new(k.s(1.3), k.k(INK)), d, d));
        }
        Tool::LassoSelect => {
            let pts: Vec<Pos2> = [(0.2, 0.5), (0.3, 0.2), (0.65, 0.15), (0.85, 0.35), (0.7, 0.62), (0.4, 0.62), (0.3, 0.75), (0.38, 0.92)]
                .iter()
                .map(|&(x, y)| k.at(x, y))
                .collect();
            let d = k.s(2.0);
            k.p.extend(Shape::dashed_line(&pts, Stroke::new(k.s(1.3), k.k(INK)), d, d));
        }
        Tool::MagicWand => {
            k.line((0.15, 0.85), (0.62, 0.38), 2.6, INK);
            k.line((0.15, 0.85), (0.3, 0.7), 2.6, PAPER);
            for (a, b) in [((0.72, 0.1), (0.72, 0.28)), ((0.63, 0.19), (0.81, 0.19)), ((0.88, 0.34), (0.88, 0.46)), ((0.82, 0.4), (0.94, 0.4))] {
                k.line(a, b, 1.4, GOLD);
            }
        }
        Tool::MovePixels | Tool::MoveSelection => {
            if t == Tool::MoveSelection {
                k.dashed_rect(0.08, 0.08, 0.92, 0.92, INK);
            } else {
                k.rect(0.2, 0.2, 0.8, 0.8, Color32::from_rgb(170, 200, 240), 0.0, INK);
            }
            k.line((0.5, 0.22), (0.5, 0.78), 1.6, INK);
            k.line((0.22, 0.5), (0.78, 0.5), 1.6, INK);
            k.arrow_head((0.5, 0.14), (0.0, -1.0), 0.12, INK);
            k.arrow_head((0.5, 0.86), (0.0, 1.0), 0.12, INK);
            k.arrow_head((0.14, 0.5), (-1.0, 0.0), 0.12, INK);
            k.arrow_head((0.86, 0.5), (1.0, 0.0), 0.12, INK);
        }
        Tool::Zoom => {
            k.circle(0.42, 0.42, 0.25, Color32::from_rgb(220, 235, 250), 1.8, INK);
            k.line((0.6, 0.6), (0.88, 0.88), 2.8, INK);
            k.line((0.3, 0.42), (0.54, 0.42), 1.4, INK);
            k.line((0.42, 0.3), (0.42, 0.54), 1.4, INK);
        }
        Tool::Pan => {
            // an open hand made of rounded strokes
            for (i, h) in [0.3f32, 0.18, 0.2, 0.32].iter().enumerate() {
                let x = 0.3 + i as f32 * 0.13;
                k.line((x, *h + 0.12), (x, 0.6), 2.6, Color32::from_rgb(230, 190, 150));
            }
            k.circle(0.5, 0.66, 0.2, Color32::from_rgb(230, 190, 150), 0.0, INK);
            k.line((0.2, 0.52), (0.32, 0.68), 2.6, Color32::from_rgb(230, 190, 150));
        }
        Tool::PaintBucket => {
            k.poly(&[(0.2, 0.4), (0.5, 0.15), (0.8, 0.45), (0.5, 0.75)], Color32::from_rgb(200, 205, 215), 1.3, INK);
            k.poly(&[(0.2, 0.4), (0.8, 0.45), (0.5, 0.75)], BLUE, 0.0, INK);
            k.circle(0.86, 0.72, 0.08, BLUE, 0.0, BLUE);
        }
        Tool::Gradient => {
            let n = 8;
            for i in 0..n {
                let t = i as f32 / n as f32;
                let c = Color32::from_rgb((40.0 + 200.0 * t) as u8, (40.0 + 200.0 * t) as u8, (60.0 + 190.0 * t) as u8);
                k.rect(0.12 + t * 0.76, 0.2, 0.12 + (t + 1.0 / n as f32) * 0.76 + 0.01, 0.8, c, 0.0, c);
            }
            k.closed(&[(0.12, 0.2), (0.88, 0.2), (0.88, 0.8), (0.12, 0.8)], 1.0, INK);
        }
        Tool::Paintbrush => {
            k.line((0.82, 0.1), (0.45, 0.55), 2.4, Color32::from_rgb(150, 100, 60));
            k.poly(&[(0.4, 0.5), (0.52, 0.62), (0.3, 0.86), (0.12, 0.9), (0.18, 0.7)], RUST, 1.0, INK);
        }
        Tool::Eraser => {
            k.poly(&[(0.12, 0.62), (0.5, 0.22), (0.88, 0.5), (0.5, 0.9)], Color32::from_rgb(240, 150, 170), 1.2, INK);
            k.poly(&[(0.12, 0.62), (0.31, 0.42), (0.69, 0.7), (0.5, 0.9)], PAPER, 1.2, INK);
        }
        Tool::Pencil => {
            k.poly(&[(0.75, 0.1), (0.9, 0.25), (0.35, 0.8), (0.2, 0.65)], GOLD, 1.2, INK);
            k.poly(&[(0.2, 0.65), (0.35, 0.8), (0.1, 0.9)], Color32::from_rgb(240, 220, 180), 1.0, INK);
        }
        Tool::ColorPicker => {
            k.line((0.2, 0.8), (0.58, 0.42), 2.2, INK);
            k.poly(&[(0.55, 0.3), (0.7, 0.45), (0.9, 0.25), (0.75, 0.1)], INK, 1.0, INK);
            k.circle(0.16, 0.84, 0.06, BLUE, 0.0, BLUE);
        }
        Tool::CloneStamp => {
            k.rect(0.32, 0.12, 0.68, 0.46, Color32::from_rgb(150, 100, 60), 1.0, INK);
            k.rect(0.14, 0.46, 0.86, 0.64, RUST, 1.0, INK);
            k.rect(0.2, 0.74, 0.8, 0.9, GREEN, 0.0, GREEN);
        }
        Tool::Recolor => {
            k.line((0.82, 0.1), (0.45, 0.55), 2.4, Color32::from_rgb(150, 100, 60));
            k.poly(&[(0.4, 0.5), (0.52, 0.62), (0.3, 0.86), (0.12, 0.9), (0.18, 0.7)], GREEN, 1.0, INK);
            k.circle(0.78, 0.78, 0.12, RED, 1.0, INK);
        }
        Tool::Text => {
            k.line((0.2, 0.18), (0.8, 0.18), 2.6, INK);
            k.line((0.5, 0.18), (0.5, 0.86), 2.6, INK);
            k.line((0.36, 0.86), (0.64, 0.86), 1.6, INK);
        }
        Tool::LineCurve => {
            let pts: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let x = 0.12 + 0.76 * t;
                    let y = 0.8 - 0.6 * t + 0.25 * (t * std::f32::consts::PI * 2.0).sin();
                    (x, y)
                })
                .collect();
            k.path(&pts, 2.0, BLUE);
            k.rect(0.06, 0.74, 0.18, 0.86, PAPER, 1.0, INK);
            k.rect(0.82, 0.14, 0.94, 0.26, PAPER, 1.0, INK);
        }
        Tool::Shapes => {
            k.rect(0.1, 0.12, 0.55, 0.57, Color32::from_rgb(120, 170, 230), 1.2, INK);
            k.circle(0.64, 0.64, 0.24, Color32::from_rgba_unmultiplied(230, 120, 60, 220), 1.2, INK);
        }
    }
}

/// Generate the application icon as straight RGBA (size x size).
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Point, SpreadMode, Stroke as SStroke, Transform};
    let s = size as f32;
    let mut pm = tiny_skia::Pixmap::new(size, size).unwrap();
    // rounded square background with a warm iron-oxide gradient
    let r = s * 0.2;
    let mut pb = PathBuilder::new();
    let (x0, y0, x1, y1) = (s * 0.04, s * 0.04, s * 0.96, s * 0.96);
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.quad_to(x1, y0, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.quad_to(x1, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.quad_to(x0, y1, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.quad_to(x0, y0, x0 + r, y0);
    pb.close();
    let bg = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(s, s),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(232, 120, 52, 255)),
            GradientStop::new(1.0, Color::from_rgba8(140, 46, 24, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();
    pm.fill_path(&bg, &paint, FillRule::Winding, Transform::identity(), None);
    // white canvas sheet
    let mut sheet = PathBuilder::new();
    sheet.push_rect(tiny_skia::Rect::from_xywh(s * 0.22, s * 0.2, s * 0.56, s * 0.6).unwrap());
    let sheet = sheet.finish().unwrap();
    let mut p2 = Paint::default();
    p2.set_color_rgba8(255, 250, 240, 240);
    pm.fill_path(&sheet, &p2, FillRule::Winding, Transform::identity(), None);
    // a bold brush stroke "F"-like swoosh in deep blue
    let mut sw = PathBuilder::new();
    sw.move_to(s * 0.34, s * 0.7);
    sw.cubic_to(s * 0.36, s * 0.45, s * 0.42, s * 0.32, s * 0.66, s * 0.3);
    sw.move_to(s * 0.38, s * 0.5);
    sw.line_to(s * 0.58, s * 0.48);
    let sw = sw.finish().unwrap();
    let mut p3 = Paint::default();
    p3.anti_alias = true;
    p3.set_color_rgba8(36, 82, 168, 255);
    let st = SStroke { width: s * 0.085, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
    pm.stroke_path(&sw, &p3, &st, Transform::identity(), None);
    // gold paint drop
    if let Some(c) = PathBuilder::from_circle(s * 0.7, s * 0.7, s * 0.07) {
        let mut p4 = Paint::default();
        p4.anti_alias = true;
        p4.set_color_rgba8(250, 196, 50, 255);
        pm.fill_path(&c, &p4, FillRule::Winding, Transform::identity(), None);
    }
    crate::raster::pixmap_to_straight(&pm)
}

/// Small helper: an icon button with hover/selected background and tooltip.
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, size: f32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let p = crate::theme::ui_pal(ui);
        if selected {
            ui.painter().rect(rect, 5.0, p.accent_soft, egui::Stroke::new(1.0f32, p.accent), egui::StrokeKind::Inside);
        } else if resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 5.0, p.accent_soft);
        } else if resp.hovered() {
            ui.painter().rect(rect, 5.0, p.hover, egui::Stroke::new(1.0f32, p.accent.gamma_multiply(0.45)), egui::StrokeKind::Inside);
        }        paint_icon(ui.painter(), rect.shrink(size * 0.12), icon);
    }
    resp.on_hover_text(tip)
}
