//! Rendering helpers that draw tool output onto layer pixmaps with tiny-skia.

use crate::raster::{self, Col};
use crate::tools::{DashStyle, GradKind, LineCap, ShapeKind, ShapeStyle};
use egui::Pos2;
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, LineJoin, Mask, Paint, Path, PathBuilder, Pattern, Pixmap, Rect, SpreadMode,
    Stroke, StrokeDash, Transform,
};

pub fn norm_rect(a: Pos2, b: Pos2) -> (f32, f32, f32, f32) {
    (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}

fn rounded_rect(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Option<Path> {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let k = 0.5523 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.cubic_to(x1 - r + k, y0, x1, y0 + r - k, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.cubic_to(x0 + r - k, y1, x0, y1 - r + k, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.cubic_to(x0, y0 + r - k, x0 + r - k, y0, x0 + r, y0);
    pb.close();
    pb.finish()
}

pub fn shape_path(kind: ShapeKind, a: Pos2, b: Pos2, corner_radius: f32) -> Option<Path> {
    let (x0, y0, x1, y1) = norm_rect(a, b);
    if x1 - x0 < 0.5 && y1 - y0 < 0.5 {
        return None;
    }
    match kind {
        ShapeKind::Ellipse => PathBuilder::from_oval(Rect::from_ltrb(x0, y0, x1.max(x0 + 0.5), y1.max(y0 + 0.5))?),
        ShapeKind::Rectangle => rounded_rect(x0, y0, x1, y1, 0.0),
        ShapeKind::RoundedRectangle => rounded_rect(x0, y0, x1, y1, corner_radius),
        _ => {
            let pts = crate::icons::shape_outline(kind, x0, y0, x1, y1);
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            for p in &pts[1..] {
                pb.line_to(p.0, p.1);
            }
            pb.close();
            pb.finish()
        }
    }
}

fn paint_for(c: Col, aa: bool, overwrite: bool) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c.skia());
    p.anti_alias = aa;
    p.blend_mode = if overwrite { BlendMode::Source } else { BlendMode::SourceOver };
    p
}

fn stroke_for(width: f32, dash: DashStyle, join: LineJoin, cap: tiny_skia::LineCap) -> Stroke {
    let mut s = Stroke { width: width.max(0.5), line_join: join, line_cap: cap, miter_limit: 4.0, ..Default::default() };
    if let Some(pat) = dash.pattern(width) {
        s.dash = StrokeDash::new(pat, 0.0);
    }
    s
}

pub struct ShapeParams {
    pub kind: ShapeKind,
    pub style: ShapeStyle,
    pub width: f32,
    pub aa: bool,
    pub overwrite: bool,
    pub dash: DashStyle,
    pub corner_radius: f32,
}

pub fn draw_shape(pm: &mut Pixmap, clip: Option<&Mask>, a: Pos2, b: Pos2, p: &ShapeParams, outline: Col, fill: Col) {
    let Some(path) = shape_path(p.kind, a, b, p.corner_radius) else { return };
    match p.style {
        ShapeStyle::Filled => {
            pm.fill_path(&path, &paint_for(outline, p.aa, p.overwrite), FillRule::Winding, Transform::identity(), clip);
        }
        ShapeStyle::Outline | ShapeStyle::FilledOutline => {
            if p.style == ShapeStyle::FilledOutline {
                pm.fill_path(&path, &paint_for(fill, p.aa, p.overwrite), FillRule::Winding, Transform::identity(), clip);
            }
            let join = if matches!(p.kind, ShapeKind::Rectangle | ShapeKind::Diamond) { LineJoin::Miter } else { LineJoin::Round };
            let st = stroke_for(p.width, p.dash, join, tiny_skia::LineCap::Butt);
            pm.stroke_path(&path, &paint_for(outline, p.aa, p.overwrite), &st, Transform::identity(), clip);
        }
    }
}

pub struct LineParams {
    pub width: f32,
    pub aa: bool,
    pub overwrite: bool,
    pub dash: DashStyle,
    pub start_cap: LineCap,
    pub end_cap: LineCap,
}

/// Quadratic control so that the curve passes through `c` at t = 0.5.
pub fn curve_control(p0: Pos2, p1: Pos2, c: Pos2) -> Pos2 {
    let mid = Pos2::new((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
    Pos2::new(2.0 * c.x - mid.x, 2.0 * c.y - mid.y)
}

pub fn draw_line(pm: &mut Pixmap, clip: Option<&Mask>, p0: Pos2, p1: Pos2, c: Pos2, lp: &LineParams, col: Col) {
    if (p0 - p1).length() < 0.01 && (c - p0).length() < 0.01 {
        // single click: a dot
        if let Some(d) = PathBuilder::from_circle(p0.x, p0.y, lp.width / 2.0) {
            pm.fill_path(&d, &paint_for(col, lp.aa, lp.overwrite), FillRule::Winding, Transform::identity(), clip);
        }
        return;
    }
    let q = curve_control(p0, p1, c);
    let mut pb = PathBuilder::new();
    pb.move_to(p0.x, p0.y);
    let straight = (c - Pos2::new((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0)).length() < 0.01;
    if straight {
        pb.line_to(p1.x, p1.y);
    } else {
        pb.quad_to(q.x, q.y, p1.x, p1.y);
    }
    let Some(path) = pb.finish() else { return };
    let cap = if lp.start_cap == LineCap::Rounded || lp.end_cap == LineCap::Rounded {
        tiny_skia::LineCap::Round
    } else {
        tiny_skia::LineCap::Butt
    };
    let st = stroke_for(lp.width, lp.dash, LineJoin::Round, cap);
    let paint = paint_for(col, lp.aa, lp.overwrite);
    pm.stroke_path(&path, &paint, &st, Transform::identity(), clip);
    // arrow heads
    let dir_end = if straight { p1 - p0 } else { p1 - q };
    let dir_start = if straight { p0 - p1 } else { p0 - q };
    for (cap, tip, dir) in [(lp.end_cap, p1, dir_end), (lp.start_cap, p0, dir_start)] {
        if !matches!(cap, LineCap::Arrow | LineCap::ArrowFilled) {
            continue;
        }
        let l = dir.length();
        if l < 0.01 {
            continue;
        }
        let d = dir / l;
        let n = egui::vec2(-d.y, d.x);
        let size = (lp.width * 3.0).max(8.0);
        let base = tip - d * size;
        let a = base + n * size * 0.55;
        let b = base - n * size * 0.55;
        let mut pb = PathBuilder::new();
        pb.move_to(a.x, a.y);
        pb.line_to(tip.x, tip.y);
        pb.line_to(b.x, b.y);
        if cap == LineCap::ArrowFilled {
            pb.close();
        }
        if let Some(ap) = pb.finish() {
            if cap == LineCap::ArrowFilled {
                pm.fill_path(&ap, &paint, FillRule::Winding, Transform::identity(), clip);
            } else {
                let st = Stroke { width: lp.width.max(1.0), line_join: LineJoin::Miter, ..Default::default() };
                pm.stroke_path(&ap, &paint, &st, Transform::identity(), clip);
            }
        }
    }
}

pub fn polyline_path(points: &[Pos2]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].x, points[0].y);
    if points.len() == 1 {
        pb.line_to(points[0].x + 0.01, points[0].y);
    }
    for p in &points[1..] {
        pb.line_to(p.x, p.y);
    }
    pb.finish()
}

pub enum BrushKind<'a> {
    Paint(Col),
    Erase,
    Clone { source: &'a Pixmap, offset: egui::Vec2 },
}

pub fn draw_brush_stroke(pm: &mut Pixmap, clip: Option<&Mask>, points: &[Pos2], width: f32, aa: bool, overwrite: bool, kind: BrushKind) {
    if points.is_empty() {
        return;
    }
    let mut paint = Paint::default();
    paint.anti_alias = aa;
    match kind {
        BrushKind::Paint(c) => {
            paint.set_color(c.skia());
            paint.blend_mode = if overwrite { BlendMode::Source } else { BlendMode::SourceOver };
        }
        BrushKind::Erase => {
            paint.set_color(tiny_skia::Color::BLACK);
            paint.blend_mode = BlendMode::DestinationOut;
        }
        BrushKind::Clone { source, offset } => {
            paint.shader = Pattern::new(
                source.as_ref(),
                SpreadMode::Pad,
                FilterQuality::Nearest,
                1.0,
                Transform::from_translate(offset.x, offset.y),
            );
        }
    }
    let st = Stroke {
        width: width.max(0.5),
        line_cap: tiny_skia::LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    if points.len() == 1 || points.iter().all(|p| (*p - points[0]).length() < 0.01) {
        if let Some(c) = PathBuilder::from_circle(points[0].x, points[0].y, (width / 2.0).max(0.5)) {
            pm.fill_path(&c, &paint, FillRule::Winding, Transform::identity(), clip);
        }
        return;
    }
    if let Some(path) = polyline_path(points) {
        pm.stroke_path(&path, &paint, &st, Transform::identity(), clip);
    }
}

/// Aliased 1px line (pencil). Blends with source-over (or overwrite).
pub fn pencil_line(pm: &mut Pixmap, clip: Option<&Mask>, a: Pos2, b: Pos2, c: Col, overwrite: bool) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let (mut x0, mut y0) = (a.x.floor() as i32, a.y.floor() as i32);
    let (x1, y1) = (b.x.floor() as i32, b.y.floor() as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let src = c.premul();
    let data = pm.data_mut();
    loop {
        if x0 >= 0 && y0 >= 0 && x0 < w && y0 < h {
            let i = (y0 * w + x0) as usize;
            let allowed = clip.map(|m| m.data()[i] >= 128).unwrap_or(true);
            if allowed {
                let j = i * 4;
                if overwrite {
                    data[j..j + 4].copy_from_slice(&src);
                } else {
                    let ia = 255 - src[3] as u32;
                    for k in 0..4 {
                        data[j + k] = (src[k] as u32 + raster::mul8(data[j + k] as u32, ia) as u32).min(255) as u8;
                    }
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Recolor: replace colours similar to `from` with `to` within a circular brush.
pub fn recolor_stamp(pm: &mut Pixmap, clip: Option<&Mask>, center: Pos2, radius: f32, from: Col, to: Col, tol: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let r = radius.max(0.5);
    let (x0, x1) = (((center.x - r).floor() as i32).max(0), ((center.x + r).ceil() as i32).min(w));
    let (y0, y1) = (((center.y - r).floor() as i32).max(0), ((center.y + r).ceil() as i32).min(h));
    let target = [from.r, from.g, from.b, from.a];
    let thr = tol * tol;
    let data = pm.data_mut();
    for y in y0..y1 {
        for x in x0..x1 {
            let d = ((x as f32 + 0.5 - center.x).powi(2) + (y as f32 + 0.5 - center.y).powi(2)).sqrt();
            if d > r {
                continue;
            }
            let i = (y * w + x) as usize;
            if let Some(m) = clip {
                if m.data()[i] < 128 {
                    continue;
                }
            }
            let j = i * 4;
            let px = raster::unpremul([data[j], data[j + 1], data[j + 2], data[j + 3]]);
            if px[3] == 0 {
                continue;
            }
            let dist = raster::color_dist([px[0], px[1], px[2], 255], [target[0], target[1], target[2], 255]);
            if dist <= thr + 1e-6 {
                let na = (px[3] as u32 * to.a as u32 / 255) as u8;
                let np = raster::premul([to.r, to.g, to.b, na]);
                data[j..j + 4].copy_from_slice(&np);
            }
        }
    }
}

pub fn snap_angle(origin: Pos2, p: Pos2, step_deg: f32) -> Pos2 {
    let d = p - origin;
    let len = d.length();
    let ang = d.y.atan2(d.x);
    let step = step_deg.to_radians();
    let a = (ang / step).round() * step;
    origin + egui::vec2(a.cos(), a.sin()) * len
}

/// Render a gradient over `base` into `out` (both same size).
#[allow(clippy::too_many_arguments)]
pub fn render_gradient(base: &Pixmap, out: &mut Pixmap, clip: Option<&Mask>, kind: GradKind, s: Pos2, e: Pos2, c0: Col, c1: Col, transparency: bool) {
    let w = base.width() as usize;
    let d = e - s;
    let len = d.length().max(0.001);
    let dn = d / len;
    let b = base.data();
    let o = out.data_mut();
    let md = clip.map(|m| m.data());
    for (i, px) in o.chunks_exact_mut(4).enumerate() {
        let m = md.map(|m| m[i]).unwrap_or(255) as u32;
        let j = i * 4;
        if m == 0 {
            px.copy_from_slice(&b[j..j + 4]);
            continue;
        }
        let x = (i % w) as f32 + 0.5;
        let y = (i / w) as f32 + 0.5;
        let v = egui::vec2(x - s.x, y - s.y);
        let along = v.x * dn.x + v.y * dn.y;
        let across = -v.x * dn.y + v.y * dn.x;
        let t = match kind {
            GradKind::Linear => along / len,
            GradKind::LinearReflected => along.abs() / len,
            GradKind::Diamond => (along.abs() + across.abs()) / len,
            GradKind::Radial => v.length() / len,
            GradKind::Conical => {
                let a = across.atan2(along); // -pi..pi relative to direction
                a.abs() / std::f32::consts::PI
            }
        }
        .clamp(0.0, 1.0);
        let res: [u8; 4] = if transparency {
            // multiply existing pixel alpha by the gradient's alpha
            let fa = (c0.a as f32 + (c1.a as f32 - c0.a as f32) * t) / 255.0;
            let k = (fa * 255.0).round() as u32;
            [
                raster::mul8(b[j] as u32, k),
                raster::mul8(b[j + 1] as u32, k),
                raster::mul8(b[j + 2] as u32, k),
                raster::mul8(b[j + 3] as u32, k),
            ]
        } else {
            let c = Col::lerp(c0, c1, t).premul();
            let ia = 255 - c[3] as u32;
            [
                (c[0] as u32 + raster::mul8(b[j] as u32, ia) as u32).min(255) as u8,
                (c[1] as u32 + raster::mul8(b[j + 1] as u32, ia) as u32).min(255) as u8,
                (c[2] as u32 + raster::mul8(b[j + 2] as u32, ia) as u32).min(255) as u8,
                (c[3] as u32 + raster::mul8(b[j + 3] as u32, ia) as u32).min(255) as u8,
            ]
        };
        if m == 255 {
            px.copy_from_slice(&res);
        } else {
            for k in 0..4 {
                px[k] = ((b[j + k] as u32 * (255 - m) + res[k] as u32 * m + 127) / 255) as u8;
            }
        }
    }
}

/// Draw text path with optional faux-bold stroke and underline rectangles.
pub fn draw_text(pm: &mut Pixmap, clip: Option<&Mask>, layout: &crate::text::TextLayout, c: Col, bold: bool, size: f32, aa: bool) {
    let paint = paint_for(c, aa, false);
    if let Some(path) = &layout.path {
        pm.fill_path(path, &paint, FillRule::Winding, Transform::identity(), clip);
        if bold {
            let st = Stroke { width: (size / 24.0).max(0.8), line_join: LineJoin::Round, ..Default::default() };
            pm.stroke_path(path, &paint, &st, Transform::identity(), clip);
        }
    }
    for &(x0, y0, x1, y1) in &layout.underlines {
        if let Some(r) = Rect::from_ltrb(x0, y0, x1, y1) {
            pm.fill_rect(r, &paint, Transform::identity(), clip);
        }
    }
}
