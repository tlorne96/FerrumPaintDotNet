//! Extra effects inspired by the one-line public descriptions of a popular third-party Paint.NET
//! plugin pack (see docs/REFERENCE_NOTES.md, "Plugin-inspired effects"). The algorithms are
//! original; no plugin binaries were decompiled, and these plugins have no published source.
//!
//! Jitter, Stitch, Rectangular Pixelate, Gradient Blocks, Outline Object, Film, Random Shapes,
//! Splatter. (Ports of the MIT-licensed plugins live in pyrochild.rs.)

use super::*;
use crate::tools::ShapeKind;
use std::f32::consts::{PI, TAU};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Stroke, Transform};

pub const SHAPES: &[&str] = &[
    "Random", "Rectangle", "Rounded Rectangle", "Ellipse", "Triangle", "Right Triangle", "Diamond", "Pentagon", "Hexagon", "Octagon", "Star",
    "Arrow", "Heart", "Cross", "Speech Bubble",
];
pub const COLOR_MODES: &[&str] =
    &["None", "Primary", "Secondary", "Primary / secondary mix", "Random hue", "Random color", "Random gray", "Sampled from image"];
pub const SPLAT_COLORS: &[&str] = &["Color 1", "Color 2", "Mix of Color 1 and 2", "Random hues"];
pub const LETTERBOX: &[&str] = &["None", "1.85 : 1", "2.39 : 1"];

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx) -> Buf {
    match kind {
        Fx::Jitter => jitter(a, b, bx),
        Fx::Stitch => stitch(a, b, bx),
        Fx::RectPixelate => rect_pixelate(a, b, bx),
        Fx::GradientBlocks => gradient_blocks(a, b, bx),
        Fx::OutlineObject => outline_object(a, b),
        Fx::Film => film(a, b, bx),
        Fx::RandomShapes => random_shapes(a, b, bx),
        Fx::Splatter => splatter(a, b, bx),
        _ => b.clone(),
    }
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn sk(p: Px) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba(p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), p[3].clamp(0.0, 1.0)).unwrap_or(tiny_skia::Color::BLACK)
}

fn solid(c: tiny_skia::Color, aa: bool) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = aa;
    p
}

/// Unit direction for an angle in degrees, counter-clockwise with y pointing down on screen.
fn dir(deg: f32) -> (f32, f32) {
    let r = deg.to_radians();
    (r.cos(), -r.sin())
}

// ----------------------------------------------------------------------------------------------
// Distort

/// Rows (at any angle) are slid along their own direction by a random amount each.
fn jitter(a: &A, b: &Buf, bx: Bx) -> Buf {
    let (dx, dy) = dir(a.f(0));
    let rw = a.f(1).max(1.0);
    let amt = a.f(2);
    let edge = Edge::from(a.c(3));
    let seed = a.seed(4);
    let (cx, cy) = (bx.cx(), bx.cy());
    distort(b, 1, edge, |x, y| {
        let (px, py) = (x - cx, y - cy);
        let v = -px * dy + py * dx;
        let row = (v / rw).floor() as i32;
        let shift = ((rnd(row, 7, seed) * 2.0 - 1.0) * amt).round();
        (x + shift * dx, y + shift * dy)
    })
}

/// Embroidery: the image is cut into rows of short "stitches"; each stitch shows a slanted,
/// stretched sliver of the image and is shaded like a raised thread.
fn stitch(a: &A, b: &Buf, bx: Bx) -> Buf {
    let ra = a.f(0);
    let (dx, dy) = dir(ra);
    let (nx, ny) = (-dy, dx);
    let d = (a.f(1) - ra).to_radians();
    let (tc, ts) = (d.cos(), d.sin());
    let size = a.f(2).max(1.0);
    let len = a.f(3).max(2.0);
    let relief = a.f(4) / 100.0;
    let seed = a.seed(5);
    let (cx, cy) = (bx.cx(), bx.cy());
    par_map(b.w, b.h, |x, y| {
        let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
        let (u, v) = (px * dx + py * dy, px * nx + py * ny);
        let row = (v / size).floor();
        let s = v / size - row;
        let off = rnd(row as i32, 3, seed) * len;
        let seg = ((u + off) / len).floor();
        let t = (u + off) / len - seg;
        let (uc, vc) = ((seg + 0.5) * len - off, (row + 0.5) * size);
        let along = (t - 0.5) * len;
        let su = uc + along * tc;
        let sv = vc - along * ts + (s - 0.5) * size * 0.3;
        let (sx, sy) = (cx + su * dx + sv * nx, cy + su * dy + sv * ny);
        let p = b.sample(sx - 0.5, sy - 0.5, Edge::Clamp);
        let shade = 1.0 - relief * (1.0 - ((PI * t).sin() * (PI * s).sin()).max(0.0).sqrt());
        [p[0] * shade, p[1] * shade, p[2] * shade, p[3]]
    })
}

// ----------------------------------------------------------------------------------------------
// Blocks (Rectangular Pixelate, Gradient Blocks)

struct Grid {
    ox: i64,
    oy: i64,
    cw: i64,
    ch: i64,
    gx0: i64,
    gy0: i64,
    nx: usize,
    ny: usize,
}
impl Grid {
    fn new(b: &Buf, bx: Bx, cw: i64, ch: i64) -> Grid {
        let (ox, oy) = (bx.x0.floor() as i64, bx.y0.floor() as i64);
        let gx0 = (-ox).div_euclid(cw);
        let gy0 = (-oy).div_euclid(ch);
        let gx1 = (b.w as i64 - 1 - ox).div_euclid(cw);
        let gy1 = (b.h as i64 - 1 - oy).div_euclid(ch);
        Grid { ox, oy, cw, ch, gx0, gy0, nx: (gx1 - gx0 + 1).max(1) as usize, ny: (gy1 - gy0 + 1).max(1) as usize }
    }
    /// Cell index plus the position inside the cell.
    fn cell(&self, x: usize, y: usize) -> (usize, i64, i64) {
        let (rx, ry) = (x as i64 - self.ox, y as i64 - self.oy);
        let (gx, gy) = ((rx.div_euclid(self.cw) - self.gx0) as usize, (ry.div_euclid(self.ch) - self.gy0) as usize);
        (gy.min(self.ny - 1) * self.nx + gx.min(self.nx - 1), rx.rem_euclid(self.cw), ry.rem_euclid(self.ch))
    }
}

fn rect_pixelate(a: &A, b: &Buf, bx: Bx) -> Buf {
    let g = Grid::new(b, bx, a.i(0).max(1) as i64, a.i(1).max(1) as i64);
    let mut cells = vec![[0.0f32; 5]; g.nx * g.ny];
    let center = a.c(2) == 1;
    for y in 0..b.h {
        for x in 0..b.w {
            let (i, lx, ly) = g.cell(x, y);
            let p = b.get(x, y);
            let c = &mut cells[i];
            if center {
                if lx == g.cw / 2 && ly == g.ch / 2 || c[4] == 0.0 {
                    *c = [p[0], p[1], p[2], p[3], 1.0];
                }
            } else {
                for k in 0..4 {
                    c[k] += p[k];
                }
                c[4] += 1.0;
            }
        }
    }
    let px: Vec<Px> = cells.iter().map(|c| if center { [c[0], c[1], c[2], c[3]] } else { scale4([c[0], c[1], c[2], c[3]], 1.0 / c[4].max(1.0)) }).collect();
    par_map(b.w, b.h, |x, y| px[g.cell(x, y).0])
}

/// Each block becomes a smooth four-corner gradient between the averages of its quadrants.
fn gradient_blocks(a: &A, b: &Buf, bx: Bx) -> Buf {
    let g = Grid::new(b, bx, a.i(0).max(2) as i64, a.i(1).max(2) as i64);
    let mut q = vec![[[0.0f32; 5]; 4]; g.nx * g.ny];
    for y in 0..b.h {
        for x in 0..b.w {
            let (i, lx, ly) = g.cell(x, y);
            let k = (lx * 2 >= g.cw) as usize + 2 * ((ly * 2 >= g.ch) as usize);
            let p = b.get(x, y);
            let c = &mut q[i][k];
            for j in 0..4 {
                c[j] += p[j];
            }
            c[4] += 1.0;
        }
    }
    let corners: Vec<[Px; 4]> = q
        .iter()
        .map(|qs| {
            let tot = qs.iter().fold([0.0f32; 5], |mut s, c| {
                for j in 0..5 {
                    s[j] += c[j];
                }
                s
            });
            let avg = |c: &[f32; 5]| if c[4] > 0.0 { scale4([c[0], c[1], c[2], c[3]], 1.0 / c[4]) } else { scale4([tot[0], tot[1], tot[2], tot[3]], 1.0 / tot[4].max(1.0)) };
            [avg(&qs[0]), avg(&qs[1]), avg(&qs[2]), avg(&qs[3])]
        })
        .collect();
    par_map(b.w, b.h, |x, y| {
        let (i, lx, ly) = g.cell(x, y);
        let tx = smooth(lx as f32 / (g.cw - 1).max(1) as f32);
        let ty = smooth(ly as f32 / (g.ch - 1).max(1) as f32);
        let c = &corners[i];
        lerp4(lerp4(c[0], c[1], tx), lerp4(c[2], c[3], tx), ty)
    })
}

// ----------------------------------------------------------------------------------------------
// Object

/// 1-D squared distance transform of a sampled function (lower envelope of parabolas).
fn dt1(f: &[f64], out: &mut [f64], v: &mut [usize], z: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let qf = q as f64;
        let mut s;
        loop {
            let p = v[k] as f64;
            s = ((f[q] + qf * qf) - (f[v[k]] + p * p)) / (2.0 * qf - 2.0 * p);
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                break;
            }
        }
        if s <= z[k] {
            // k == 0 and the new parabola dominates the first one entirely
            v[0] = q;
            z[0] = f64::NEG_INFINITY;
            z[1] = f64::INFINITY;
            continue;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let d = q as f64 - v[k] as f64;
        *o = d * d + f[v[k]];
    }
}

fn dt_rows(g: &mut [f64], w: usize) {
    if w == 0 {
        return;
    }
    let rows = g.len() / w;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 32);
    let per = rows.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for chunk in g.chunks_mut(per * w) {
            s.spawn(move || {
                let (mut v, mut z, mut out) = (vec![0usize; w], vec![0.0f64; w + 1], vec![0.0f64; w]);
                for row in chunk.chunks_mut(w) {
                    dt1(row, &mut out, &mut v, &mut z);
                    row.copy_from_slice(&out);
                }
            });
        }
    });
}

fn transpose_f(g: &[f64], w: usize, h: usize) -> Vec<f64> {
    let mut t = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            t[x * h + y] = g[y * w + x];
        }
    }
    t
}

/// Euclidean distance (pixels) from each pixel to the nearest pixel where `inside` is true.
pub fn distance_field(inside: &[bool], w: usize, h: usize) -> Vec<f32> {
    const FAR: f64 = 1e12;
    let g: Vec<f64> = inside.iter().map(|&i| if i { 0.0 } else { FAR }).collect();
    let mut t = transpose_f(&g, w, h);
    dt_rows(&mut t, h);
    let mut g = transpose_f(&t, h, w);
    dt_rows(&mut g, w);
    g.iter().map(|&d| (d.min(FAR) as f32).sqrt()).collect()
}

/// Outline everything that is not transparent, drawn behind the object.
fn outline_object(a: &A, b: &Buf) -> Buf {
    let width = a.f(0).max(1.0);
    let band = 1.0 + a.f(1) / 100.0 * width;
    let col = pm(col_px(a.col(2)));
    let only = a.b(3);
    let inside: Vec<bool> = b.d.iter().map(|p| p[3] >= 0.5).collect();
    let dist = distance_field(&inside, b.w, b.h);
    par_map(b.w, b.h, |x, y| {
        let i = y * b.w + x;
        let cov = ((width - dist[i]) / band + 0.5).clamp(0.0, 1.0);
        let p = b.d[i];
        let o = scale4(col, cov * (1.0 - p[3]));
        if only {
            o
        } else {
            [p[0] + o[0], p[1] + o[1], p[2] + o[2], p[3] + o[3]]
        }
    })
}


// ----------------------------------------------------------------------------------------------
// Photo

/// Film look: optional centre zoom, colour grading, luminance-dependent grain and letterboxing.
fn film(a: &A, b: &Buf, bx: Bx) -> Buf {
    let grain = a.f(0) / 100.0 * 0.35;
    let gs = a.f(1).max(1.0);
    let z = a.f(2) / 100.0;
    let sat = a.f(3) / 100.0;
    let con = 1.0 + a.f(4) / 100.0;
    let warm = a.f(5) / 100.0 * 0.08;
    let ratio = [0.0, 1.85, 2.39][a.c(6).min(2)];
    let seed = a.seed(7);
    let (cx, cy) = (bx.cx(), bx.cy());
    let src = if z > 1.001 { distort(b, 2, Edge::Clamp, |x, y| (cx + (x - cx) / z, cy + (y - cy) / z)) } else { b.clone() };
    let bar = if ratio > 0.0 { ((bx.h() - bx.w() / ratio) * 0.5).max(0.0) } else { 0.0 };
    par_map(b.w, b.h, |x, y| {
        let yc = y as f32 + 0.5;
        if bar > 0.0 && (yc < bx.y0 + bar || yc > bx.y1 - bar) {
            return [0.0, 0.0, 0.0, 1.0];
        }
        let p = st(src.get(x, y));
        if p[3] <= 0.0 {
            return [0.0; 4];
        }
        let l = luma(p);
        let mut c = [p[0], p[1], p[2]];
        for v in c.iter_mut() {
            *v = l + (*v - l) * sat;
            *v = (*v - 0.5) * con + 0.5;
        }
        c[0] += warm;
        c[2] -= warm;
        let n = perlin(x as f32 / gs, y as f32 / gs, seed) * 0.8 + (rnd(x as i32, y as i32, seed ^ 0x9e37) - 0.5) * 0.5;
        let amp = grain * (0.3 + 2.8 * l * (1.0 - l));
        for v in c.iter_mut() {
            *v += n * amp;
        }
        pm([c[0], c[1], c[2], p[3]])
    })
}

// ----------------------------------------------------------------------------------------------
// Render





/// Colour for a random item according to one of COLOR_MODES; None means "don't draw".
fn pick_color(mode: usize, i: i32, salt: i32, seed: u32, c1: Px, c2: Px, b: &Buf, at: (f32, f32)) -> Option<Px> {
    let r = |k: i32| rnd(i, salt * 16 + k, seed);
    Some(match mode {
        0 => return None,
        1 => c1,
        2 => c2,
        3 => lerp4(c1, c2, r(0)),
        4 => {
            let c = hsv_to_rgb(r(0) * 360.0, 0.75 + 0.25 * r(1), 0.8 + 0.2 * r(2));
            [c[0], c[1], c[2], 1.0]
        }
        5 => [r(0), r(1), r(2), 1.0],
        6 => {
            let g = r(0);
            [g, g, g, 1.0]
        }
        _ => {
            let p = st(b.at(at.0 as i64, at.1 as i64, Edge::Clamp));
            if p[3] <= 0.0 {
                return None;
            }
            p
        }
    })
}

fn random_shapes(a: &A, b: &Buf, bx: Bx) -> Buf {
    let shape = a.c(0);
    let count = a.i(1).max(1);
    let (lo, hi) = (a.f(2).min(a.f(3)).max(1.0), a.f(3).max(a.f(2)).max(1.0));
    let (fmode, omode) = (a.c(4), a.c(5));
    let ow = a.f(6).max(1.0);
    let transp = a.f(7) / 100.0;
    let rotate = a.b(8);
    let inside = a.b(9);
    let aa = a.b(10);
    let seed = a.seed(11);
    let (c1, c2) = (col_px(a.col(12)), col_px(a.col(13)));
    let mut out = b.to_pixmap();
    for i in 0..count {
        let r = |k: i32| rnd(i, k, seed);
        let kind = if shape == 0 { ShapeKind::ALL[(r(0) * ShapeKind::ALL.len() as f32) as usize % ShapeKind::ALL.len()] } else { ShapeKind::ALL[(shape - 1).min(ShapeKind::ALL.len() - 1)] };
        let s = lo + (hi - lo) * r(1);
        let (w, h) = (s, s * (0.75 + 0.5 * r(2)));
        let ang = if rotate { r(3) * 360.0 } else { 0.0 };
        let (ex, ey) = if !inside {
            (0.0, 0.0)
        } else if rotate {
            let rr = (w * w + h * h).sqrt() * 0.5 + ow;
            (rr, rr)
        } else {
            (w * 0.5 + ow, h * 0.5 + ow)
        };
        let span = |a0: f32, a1: f32, e: f32, t: f32| if a1 - a0 > 2.0 * e { a0 + e + (a1 - a0 - 2.0 * e) * t } else { (a0 + a1) * 0.5 };
        let (cx, cy) = (span(bx.x0, bx.x1, ex, r(4)), span(bx.y0, bx.y1, ey, r(5)));
        let alpha = 1.0 - transp * r(6);
        let Some(path) = crate::paint::shape_path(kind, egui::pos2(cx - w * 0.5, cy - h * 0.5), egui::pos2(cx + w * 0.5, cy + h * 0.5), w.min(h) * 0.2) else { continue };
        let ts = Transform::from_rotate_at(ang, cx, cy);
        if let Some(c) = pick_color(fmode, i, 1, seed, c1, c2, b, (cx, cy)) {
            out.fill_path(&path, &solid(sk([c[0], c[1], c[2], c[3] * alpha]), aa), FillRule::Winding, ts, None);
        }
        if let Some(c) = pick_color(omode, i, 2, seed, c1, c2, b, (cx, cy)) {
            let st_ = Stroke { width: ow, line_join: LineJoin::Round, ..Default::default() };
            out.stroke_path(&path, &solid(sk([c[0], c[1], c[2], c[3] * alpha]), aa), &st_, ts, None);
        }
    }
    Buf::from_pixmap(&out)
}

// ----------------------------------------------------------------------------------------------
// Artistic

/// Paint splats: lumpy blobs with spikes, flung droplets and streaks, plus dribbled strings.
fn splatter(a: &A, b: &Buf, bx: Bx) -> Buf {
    let n = a.i(0).max(1);
    let size = a.f(1).max(2.0);
    let drops = a.i(2).max(0);
    let strings = a.i(3).max(0);
    let cmode = a.c(4);
    let op = a.f(5) / 100.0;
    let (c1, c2) = (col_px(a.col(6)), col_px(a.col(7)));
    let seed = a.seed(8);
    let colour = |i: i32, k: i32| -> tiny_skia::Color {
        let r = rnd(i, 900 + k, seed);
        let c = match cmode {
            0 => c1,
            1 => c2,
            2 => lerp4(c1, c2, r),
            _ => {
                let c = hsv_to_rgb(r * 360.0, 0.85, 0.95);
                [c[0], c[1], c[2], 1.0]
            }
        };
        sk([c[0], c[1], c[2], c[3] * op])
    };
    let mut out = b.to_pixmap();
    let pt = |i: i32, k: i32| (bx.x0 + bx.w() * rnd(i, k, seed), bx.y0 + bx.h() * rnd(i, k + 1, seed));
    // dribbled strings go underneath
    for s in 0..strings {
        let i = 10_000 + s;
        let mut pb = PathBuilder::new();
        let (mut x, mut y) = pt(i, 0);
        pb.move_to(x, y);
        let segs = 4 + (rnd(i, 2, seed) * 6.0) as i32;
        let mut heading = rnd(i, 3, seed) * TAU;
        for k in 0..segs {
            heading += (rnd(i, 10 + k, seed) - 0.5) * 2.4;
            let l = size * (1.5 + 3.0 * rnd(i, 40 + k, seed));
            let (mx, my) = (x + heading.cos() * l * 0.5 + (rnd(i, 70 + k, seed) - 0.5) * l, y + heading.sin() * l * 0.5 + (rnd(i, 100 + k, seed) - 0.5) * l);
            x += heading.cos() * l;
            y += heading.sin() * l;
            pb.quad_to(mx, my, x, y);
        }
        if let Some(path) = pb.finish() {
            let w = (size * (0.04 + 0.12 * rnd(i, 5, seed))).max(1.0);
            let s_ = Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            out.stroke_path(&path, &solid(colour(i, 0), true), &s_, Transform::identity(), None);
        }
    }
    for i in 0..n {
        let (cx, cy) = pt(i, 0);
        let r0 = size * (0.5 + rnd(i, 2, seed));
        let paint = solid(colour(i, 0), true);
        let (p1, p2, p3) = (rnd(i, 3, seed) * TAU, rnd(i, 4, seed) * TAU, rnd(i, 5, seed) * TAU);
        let mut pb = PathBuilder::new();
        let m = 72;
        for j in 0..m {
            let t = j as f32 / m as f32 * TAU;
            let mut r = r0 * (0.8 + 0.12 * (3.0 * t + p1).sin() + 0.08 * (5.0 * t + p2).sin() + 0.05 * (11.0 * t + p3).sin());
            if rnd(i, 200 + j, seed) < 0.08 {
                r *= 1.25 + 0.5 * rnd(i, 300 + j, seed);
            }
            let (x, y) = (cx + r * t.cos(), cy + r * t.sin());
            if j == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        if let Some(path) = pb.finish() {
            out.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
        let nd = (drops as f32 * (0.5 + rnd(i, 6, seed))) as i32;
        for d in 0..nd {
            let ang = rnd(i, 1000 + d, seed) * TAU;
            let q = rnd(i, 2000 + d, seed);
            let dist = r0 * (1.0 + 2.5 * q * q);
            let rad = (r0 * 0.14 * (1.0 - q * 0.8) * (0.3 + rnd(i, 3000 + d, seed))).max(0.6);
            let (x, y) = (cx + ang.cos() * dist, cy + ang.sin() * dist);
            if rnd(i, 4000 + d, seed) < 0.25 {
                // a streak pointing away from the splat centre
                let len = rad * (2.0 + 4.0 * rnd(i, 5000 + d, seed));
                if let Some(r) = tiny_skia::Rect::from_xywh(x - len * 0.5, y - rad * 0.5, len, rad) {
                    if let Some(path) = PathBuilder::from_oval(r) {
                        let ts = Transform::from_rotate_at(ang.to_degrees(), x, y);
                        out.fill_path(&path, &paint, FillRule::Winding, ts, None);
                    }
                }
            } else if let Some(path) = PathBuilder::from_circle(x, y, rad) {
                out.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }
    Buf::from_pixmap(&out)
}
