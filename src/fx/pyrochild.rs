//! Rust ports of pyrochild's (Zach Walker's) MIT-licensed Paint.NET plugins, from the published
//! source at https://github.com/bsneeze: Borders N' Shapes, Gradient Bars, Gradient Grid, Trail, Gradient Mapping.
//!
//! Copyright (c) 2007-2018 Zach Walker. Ported to Rust for Ferrum Paint. MIT License; the full
//! licence text is in THIRD_PARTY_NOTICES.md. Only the plugins' own code was ported; the shared
//! helper files that carry a Paint.NET copyright header were not used (gamma-correct blending
//! uses Ferrum's own sRGB functions).

use super::*;
use std::f64::consts::PI;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Stroke, StrokeDash, Transform};

pub const BNS_SHAPES: &[&str] = &["Rectangle", "Ellipse", "Arc", "Pie"];
pub const BNS_PATTERNS: &[&str] = &["Solid", "Dotted", "Dashed", "Dash-Dot", "Dash-Dot-Dot", "Dash-Dash-Dot", "Dash-Dash-Dot-Dot"];
pub const BNS_DASH_CAPS: &[&str] = &["Flat", "Round", "Square"];
pub const BNS_LINE_CAPS: &[&str] = &["None", "Flat", "Round", "Triangle", "Square anchor", "Round anchor", "Diamond anchor", "Arrow anchor"];
pub const GB_REPEAT: &[&str] = &["None", "Repeat", "Mirror"];
pub const GG_TYPES: &[&str] = &["Radial", "Diagonal 1", "Diagonal 2", "Conical", "Square", "Horizontal", "Vertical"];

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx) -> Buf {
    match kind {
        Fx::BordersShapes => borders_n_shapes(a, b, bx),
        Fx::GradientBars => gradient_bars(a, b, bx),
        Fx::GradientGrid => gradient_grid(a, b),
        Fx::ObjectTrail => trail(a, b),
        Fx::GradientMap => gradient_mapping(a, b),
        _ => b.clone(),
    }
}

/// Straight-alpha lerp of two colours, optionally in linear light (the plugins' "gamma-adjusted
/// color blend"). Returns a premultiplied pixel.
fn mix(c1: Px, c2: Px, t: f32, gamma: bool) -> Px {
    let t = t.clamp(0.0, 1.0);
    let mut o = [0.0; 4];
    for k in 0..3 {
        o[k] = if gamma { to_srgb(to_lin(c1[k]) * (1.0 - t) + to_lin(c2[k]) * t) } else { c1[k] + (c2[k] - c1[k]) * t };
    }
    o[3] = c1[3] + (c2[3] - c1[3]) * t;
    pm(o)
}

fn solid(c: tiny_skia::Color, aa: bool) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = aa;
    p
}

// ----------------------------------------------------------------------------------------------
// Borders N' Shapes

/// Point on the ellipse at a true (visual) angle, degrees counter-clockwise (y up).
fn ellipse_pt(cx: f32, cy: f32, rx: f32, ry: f32, deg: f32) -> (f32, f32) {
    let t = deg.to_radians();
    let (c, s) = (t.cos(), t.sin());
    let r = rx * ry / ((ry * c).powi(2) + (rx * s).powi(2)).sqrt().max(1e-6);
    (cx + r * c, cy - r * s)
}

fn borders_n_shapes(a: &A, b: &Buf, bx: Bx) -> Buf {
    let shape = a.c(0);
    let width = a.i(1).max(1) as f32;
    let mut col = a.col(2);
    col.a = 255;
    let aa = a.b(3);
    let pattern = a.c(4);
    let dash_cap = a.c(5);
    let spacing = a.f(6).max(f32::EPSILON);
    let (mut start, mut end) = (a.f(7), a.f(8));
    let line_cap = if shape == 2 { a.c(9) } else { 0 };
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    // inset by half the line width (and again for the wide "anchor" caps)
    let mut inset = width / 2.0;
    if line_cap >= 4 {
        inset += width / 2.0;
    }
    let (x0, y0, x1, y1) = (bx.x0 + inset, bx.y0 + inset, bx.x1 - inset, bx.y1 - inset);
    if x1 - x0 < 0.5 || y1 - y0 < 0.5 {
        return b.clone();
    }
    let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let arc = |pb: &mut PathBuilder, first: bool| {
        let n = (((end - start) / 360.0) * 256.0).ceil().max(2.0) as i32;
        for i in 0..=n {
            let (px, py) = ellipse_pt(cx, cy, rx, ry, start + (end - start) * i as f32 / n as f32);
            if i == 0 && first {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
    };
    let mut pb = PathBuilder::new();
    match shape {
        0 => pb.push_rect(tiny_skia::Rect::from_ltrb(x0, y0, x1, y1).unwrap()),
        1 => pb.push_oval(tiny_skia::Rect::from_ltrb(x0, y0, x1, y1).unwrap()),
        2 => arc(&mut pb, true),
        _ => {
            pb.move_to(cx, cy);
            arc(&mut pb, false);
            pb.close();
        }
    }
    let Some(path) = pb.finish() else { return b.clone() };
    // dash patterns are in multiples of the line width, gaps are `spacing` widths
    let units: &[f32] = match pattern {
        1 => &[1.0, -1.0],
        2 => &[3.0, -1.0],
        3 => &[3.0, -1.0, 1.0, -1.0],
        4 => &[3.0, -1.0, 1.0, -1.0, 1.0, -1.0],
        5 => &[3.0, -1.0, 3.0, -1.0, 1.0, -1.0],
        6 => &[3.0, -1.0, 3.0, -1.0, 1.0, -1.0, 1.0, -1.0],
        _ => &[],
    };
    let mut stroke = Stroke { width, line_join: LineJoin::Miter, miter_limit: 10.0, ..Default::default() };
    if !units.is_empty() {
        let pat: Vec<f32> = units.iter().map(|&u| if u < 0.0 { spacing * width } else { u * width }).collect();
        stroke.dash = StrokeDash::new(pat, 0.0);
        stroke.line_cap = [LineCap::Butt, LineCap::Round, LineCap::Square][dash_cap.min(2)];
    } else if line_cap == 2 {
        stroke.line_cap = LineCap::Round;
    }
    let paint = solid(col.skia(), aa);
    let mut out = b.to_pixmap();
    out.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    // arc end caps
    if shape == 2 && line_cap >= 2 {
        for (ang, sign) in [(start, -1.0f32), (end, 1.0f32)] {
            let (px, py) = ellipse_pt(cx, cy, rx, ry, ang);
            let (qx, qy) = ellipse_pt(cx, cy, rx, ry, ang + sign * 0.5);
            let (mut tx, mut ty) = (qx - px, qy - py);
            let l = (tx * tx + ty * ty).sqrt().max(1e-6);
            tx /= l;
            ty /= l;
            let (nx, ny) = (-ty, tx);
            let w = width;
            let mut cap = PathBuilder::new();
            match line_cap {
                2 => {} // round: handled by the stroke's round caps (solid lines)
                3 => {
                    cap.move_to(px + nx * w / 2.0, py + ny * w / 2.0);
                    cap.line_to(px + tx * w / 2.0, py + ty * w / 2.0);
                    cap.line_to(px - nx * w / 2.0, py - ny * w / 2.0);
                    cap.close();
                }
                4 => {
                    let s = w;
                    cap.move_to(px + (tx + nx) * s, py + (ty + ny) * s);
                    cap.line_to(px + (nx - tx) * s, py + (ny - ty) * s);
                    cap.line_to(px - (tx + nx) * s, py - (ty + ny) * s);
                    cap.line_to(px + (tx - nx) * s, py + (ty - ny) * s);
                    cap.close();
                }
                5 => cap.push_circle(px, py, w),
                6 => {
                    let s = w * 1.2;
                    cap.move_to(px + tx * s, py + ty * s);
                    cap.line_to(px + nx * s, py + ny * s);
                    cap.line_to(px - tx * s, py - ty * s);
                    cap.line_to(px - nx * s, py - ny * s);
                    cap.close();
                }
                _ => {
                    let s = w * 1.5;
                    cap.move_to(px + tx * s, py + ty * s);
                    cap.line_to(px - tx * s * 0.5 + nx * s, py - ty * s * 0.5 + ny * s);
                    cap.line_to(px - tx * s * 0.5 - nx * s, py - ty * s * 0.5 - ny * s);
                    cap.close();
                }
            }
            if let Some(p) = cap.finish() {
                out.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }
    Buf::from_pixmap(&out)
}

// ----------------------------------------------------------------------------------------------
// Gradient Bars

fn gradient_bars(a: &A, b: &Buf, bx: Bx) -> Buf {
    let (c1, c2) = (col_px(a.col(0)), col_px(a.col(1)));
    let gamma = a.b(2);
    let width = a.i(3).max(1) as usize;
    let spacing = a.i(4).max(0) as usize;
    let angle = (a.f(5) as f64).rem_euclid(360.0) * PI / 180.0;
    let skew = a.f(6) as f64 * PI / 180.0;
    let scale = a.f(7) as f64;
    let align = a.f(8) as f64;
    let rep = a.c(9);
    let aa = a.b(10);
    let seed = a.seed(11);
    let (left, top) = (bx.x0.floor() as f64, bx.y0.floor() as f64);
    let (sw, sh) = ((bx.x1 - bx.x0).round() as f64, (bx.y1 - bx.y0).round() as f64);
    let period = width + spacing;
    let mut table = vec![0.0f64; (sw.max(sh) as usize * 2).max(period)];
    let mut i = 0;
    let mut n = 0;
    while i < table.len() {
        let val = rnd(n, 0, seed) as f64 * scale;
        n += 1;
        for j in 0..width.min(table.len() - i) {
            table[i + j] = val;
        }
        i += period;
    }
    let half = table.len() / 2;
    let wrap = (half - half % period).max(period) as i64;
    let max_dist = (sw / angle.cos()).abs().min((sh / angle.sin()).abs());
    let quadrant = (angle / (PI / 2.0)).floor() as i32;
    let (sa, ca) = angle.sin_cos();
    let (ssk, csk) = (angle + skew).sin_cos();
    let factor = |distance: f64, idx: i64| -> f64 {
        let idx = if idx < 0 { idx.rem_euclid(wrap) } else { idx } as usize;
        let bar_len = max_dist * table[idx.min(table.len() - 1)];
        let adj = distance - align * (max_dist - bar_len);
        if bar_len > 0.0 {
            match rep {
                0 => (adj / bar_len).clamp(0.0, 1.0),
                1 => {
                    let d = adj.rem_euclid(bar_len);
                    let mut f = d / bar_len;
                    if aa {
                        let k = bar_len - d;
                        if k > 0.0 && k < 1.0 {
                            f = k;
                        }
                    }
                    f
                }
                _ => {
                    let f = (adj / bar_len).abs() % 2.0;
                    if f > 1.0 {
                        2.0 - f
                    } else {
                        f
                    }
                }
            }
        } else if aa {
            (adj / (bar_len + 1.0)).clamp(0.0, 1.0)
        } else if adj > 0.0 {
            1.0
        } else {
            0.0
        }
    };
    par_map(b.w, b.h, |x, y| {
        let (x, y) = (x as f64, y as f64);
        let distance = match quadrant {
            0 => (sh - y + top) * sa + (x - left) * ca,
            1 => (sh - y + top) * sa - (sw - x + left) * ca,
            2 => (top - y) * sa - (sw - x + left) * ca,
            _ => (top - y) * sa + (x - left) * ca,
        };
        let bar = (x - left) * ssk + (y - top) * csk;
        let f = if aa {
            let f1 = factor(distance, bar.floor() as i64);
            let f2 = factor(distance, bar.ceil() as i64);
            let k = bar - bar.floor();
            f1 * (1.0 - k) + f2 * k
        } else {
            factor(distance, (bar + 0.5) as i64)
        };
        mix(c1, c2, f as f32, gamma)
    })
}

// ----------------------------------------------------------------------------------------------
// Gradient Grid

fn gradient_grid(a: &A, b: &Buf) -> Buf {
    let size = a.i(0).max(2) as i64;
    let kind = a.c(1);
    let (start, end) = (a.f(2) as f64, a.f(3) as f64);
    let reflected = a.b(4);
    let (c1, c2) = (col_px(a.col(5)), col_px(a.col(6)));
    let gamma = a.b(7);
    let (ox, oy) = ((a.f(8) as f64 * size as f64) as i64, (a.f(9) as f64 * size as f64) as i64);
    let lines = a.b(10);
    let mut lc = a.col(11);
    lc.a = 255;
    let line = pm(col_px(lc));
    let h2 = size / 2;
    let cellrad = (((h2 * size / 2) + (h2 * size / 2)) as f64).sqrt();
    par_map(b.w, b.h, |x, y| {
        let (x, y) = (x as i64, y as i64);
        let (mut cx, mut cy) = (x % size, y % size);
        if lines && (cx == 0 || cy == 0) {
            return line;
        }
        cx -= ox;
        cy -= oy;
        let (cl, ct) = (x / size * size, y / size * size);
        let ccx = x - cl - h2 - ox;
        let ccy = y - ct - h2 - oy;
        let s = size as f64;
        let (fx, fy) = (cx as f64, cy as f64);
        let mut frac = match kind {
            0 => ((ccx * ccx + ccy * ccy) as f64).sqrt() / cellrad,
            1 => (fx + fy) / s / 2.0,
            2 => (fx - fy + s) / s / 2.0,
            3 => (ccy.abs() as f64).atan2(ccx as f64) / PI,
            4 => 1.0 - 2.0 * (1.0 - fx.max(fy) / s).min(fx.min(fy) / s),
            5 => fx / s,
            _ => fy / s,
        };
        frac = if start == end {
            if frac < start {
                0.0
            } else {
                1.0
            }
        } else {
            (frac - start) / (end - start)
        };
        frac = frac.clamp(0.0, 1.0);
        if reflected {
            frac = (-2.0 * frac + 1.0).abs();
        }
        mix(c1, c2, frac as f32, gamma)
    })
}

// ----------------------------------------------------------------------------------------------
// Trail

/// Copies of the layer at increasing offsets, each placed underneath what is already there.
fn trail(a: &A, b: &Buf) -> Buf {
    let distance = a.i(0).max(1) as f64;
    let spacing = a.i(1).max(1) as f64;
    let dir = (a.f(2) as f64).to_radians();
    let fade = a.b(3);
    let bilinear = a.b(4);
    let (dx, dy) = (dir.cos(), -dir.sin());
    let steps: Vec<f64> = (0..).map(|k| k as f64 * spacing).take_while(|&o| o <= distance + 1e-9).collect();
    let (w, h) = (b.w as f32, b.h as f32);
    par_map(b.w, b.h, |x, y| {
        let mut o = b.get(x, y);
        for &off in &steps {
            if o[3] >= 1.0 {
                break;
            }
            let sx = (x as f64 - off * dx) as f32;
            let sy = (y as f64 - off * dy) as f32;
            if !(0.0..w).contains(&sx) || !(0.0..h).contains(&sy) {
                continue;
            }
            let mut s = if bilinear { b.sample(sx, sy, Edge::Clamp) } else { b.get(sx as usize, sy as usize) };
            if fade {
                s = scale4(s, (1.0 - off / distance) as f32);
            }
            let k = 1.0 - o[3];
            add4(&mut o, s, k);
        }
        o
    })
}

// ----------------------------------------------------------------------------------------------
// Gradient Mapping

pub const GM_PRESETS: &[&str] = &[
    "Default (Black to White)", "Primary to Secondary", "Rainbow", "High Contrast", "Hot", "Synthwave", "Sepia",
    "Duotone Blue", "Thermal", "Cyanotype", "Posterize Grays", "Cotton Candy", "Toxic", "Fire", "Vaporwave",
];
/// Input channels, in the plugin's enum order (A R G B C M Y K H S V L); Luminosity is the default.
pub const GM_CHANNELS: &[&str] = &[
    "Alpha", "Red", "Green", "Blue", "Cyan", "Magenta", "Yellow", "Black (K)", "Hue", "Saturation", "Value", "Luminosity",
];

fn rgb8(r: u8, g: u8, b: u8) -> Px {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// The plugin's gradient: control points sorted by position (stable for equal positions).
fn gm_stops(preset: usize, c1: Px, c2: Px) -> Vec<(f64, Px)> {
    let k = rgb8(0, 0, 0);
    let w = rgb8(255, 255, 255);
    let red = rgb8(255, 0, 0);
    let yellow = rgb8(255, 255, 0);
    let indigo = rgb8(75, 0, 130);
    let orange_red = rgb8(255, 69, 0);
    let cyan = rgb8(0, 255, 255);
    match preset {
        1 => vec![(0.0, c1), (1.0, c2)],
        2 => vec![
            (0.0, red),
            (1.0 / 6.0, rgb8(255, 165, 0)),
            (1.0 / 3.0, yellow),
            (0.5, rgb8(0, 255, 0)),
            (2.0 / 3.0, rgb8(0, 0, 255)),
            (5.0 / 6.0, indigo),
            (1.0, rgb8(238, 130, 238)),
        ],
        3 => vec![(0.6, k), (0.75, w)],
        4 => vec![(0.2, k), (0.75, red), (0.95, yellow), (1.0, w)],
        5 => vec![(0.0, indigo), (1.0 / 3.0, rgb8(255, 20, 147)), (2.0 / 3.0, orange_red), (1.0, cyan)],
        6 => vec![(0.0, rgb8(29, 17, 9)), (0.5, rgb8(112, 66, 20)), (1.0, rgb8(222, 196, 151))],
        7 => vec![(0.0, rgb8(0, 0, 128)), (1.0, w)],
        8 => vec![(0.0, k), (0.2, rgb8(0, 0, 255)), (0.4, rgb8(0, 255, 0)), (0.6, yellow), (0.8, red), (1.0, w)],
        9 => vec![(0.0, rgb8(10, 20, 60)), (1.0, rgb8(220, 235, 250))],
        10 => vec![(0.0, k), (1.0 / 3.0, rgb8(85, 85, 85)), (2.0 / 3.0, rgb8(170, 170, 170)), (1.0, w)],
        11 => vec![(0.0, rgb8(255, 182, 213)), (0.5, rgb8(200, 170, 230)), (1.0, rgb8(150, 220, 255))],
        12 => vec![(0.0, k), (0.5, rgb8(100, 255, 20)), (1.0, rgb8(173, 255, 47))],
        13 => vec![(0.0, k), (0.4, red), (0.7, orange_red), (1.0, yellow)],
        14 => vec![(0.0, cyan), (0.5, rgb8(255, 0, 255)), (1.0, indigo)],
        _ => vec![(0.0, k), (1.0, w)],
    }
}

/// Gradient.GetColor: last stop at/before `pos` and first stop at/after it, blended linearly.
fn gm_color(stops: &[(f64, Px)], pos: f64) -> Px {
    let (mut i1, mut i2) = (0usize, usize::MAX);
    for (i, s) in stops.iter().enumerate() {
        if s.0 <= pos {
            i1 = i;
        }
        if i2 == usize::MAX && s.0 >= pos {
            i2 = i;
        }
    }
    if i2 == usize::MAX {
        i2 = stops.len() - 1;
    }
    let (a, b) = (stops[i1], stops[i2]);
    if a.0 == b.0 {
        return a.1;
    }
    let t = ((pos - a.0) / (b.0 - a.0)) as f32;
    let mut o = [0.0; 4];
    for k in 0..4 {
        o[k] = a.1[k] + (b.1[k] - a.1[k]) * t;
    }
    o
}

#[inline]
fn b8(v: f32) -> i32 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as i32
}

/// The plugin's per-pixel channel extraction (integer maths on 8-bit values).
fn gm_channel(ch: usize, p: Px) -> i32 {
    let (r, g, b, a) = (b8(p[0]), b8(p[1]), b8(p[2]), b8(p[3]));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    match ch {
        0 => a,
        1 => r,
        2 => g,
        3 => b,
        4 => max - r,
        5 => max - g,
        6 => max - b,
        7 => 255 - max,
        8 => {
            if delta == 0 {
                return 0;
            }
            let mut h = if r == max {
                (255 * (g - b)).div_euclid(delta)
            } else if g == max {
                512 + (255 * (b - r)).div_euclid(delta)
            } else {
                1024 + (255 * (r - g)).div_euclid(delta)
            };
            if h < 0 {
                h += 1536;
            }
            h.div_euclid(6) & 255
        }
        9 => {
            if delta == 0 {
                0
            } else {
                255 * delta / max
            }
        }
        10 => max,
        _ => (7471 * b + 38470 * g + 19595 * r) >> 16,
    }
}

fn gradient_mapping(a: &A, b: &Buf) -> Buf {
    let stops = gm_stops(a.c(0), col_px(a.col(5)), col_px(a.col(6)));
    let ch = a.c(1);
    let offset = a.i(2);
    let wrap = a.b(3);
    let lock_alpha = a.b(4);
    // 256-entry lookup, as in the plugin
    let lut: Vec<Px> = (0..256).map(|i| gm_color(&stops, (i as f32 / 255.0) as f64)).collect();
    map_st(b, |p| {
        let v = gm_channel(ch, p) + offset;
        let idx = if wrap { v.rem_euclid(256) } else { v.clamp(0, 255) } as usize;
        let mut c = lut[idx];
        if lock_alpha {
            c[3] = p[3];
        }
        c
    })
}
