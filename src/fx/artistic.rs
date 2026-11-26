//! Artistic: Ink Sketch, Linocut, Mosaic, Oil Painting, Pencil Sketch, Pointillism, Stained Glass.

use super::*;
use std::collections::HashMap;

pub const TILE_SHAPES: &[&str] = &["Square", "Diamond", "Triangle", "Hexagon", "Brick", "Basketweave", "Cobblestone"];

pub fn apply(kind: Fx, a: &A, b: &Buf, _bx: Bx) -> Buf {
    match kind {
        Fx::InkSketch => ink_sketch(b, a.f(0) / 100.0, a.f(1) / 100.0),
        Fx::Linocut => linocut(b, a),
        Fx::Mosaic => {
            let shape = a.c(0);
            let size = a.f(1);
            let seed = a.seed(9);
            let cfg = TileCfg {
                bevel: a.f(2) / 100.0,
                grout: a.f(3),
                grout_col: col_px(a.col(4)),
                center_mode: a.c(5) == 1,
                light: a.f(6).to_radians(),
                ljitter: a.f(7) / 100.0,
                pjitter: a.f(8) / 100.0,
                seed,
                size,
            };
            tiles(b, &cfg, |x, y| tile(shape, x, y, size, seed))
        }
        Fx::OilPainting => oil(b, a.i(0).max(1) as i64, (a.f(1) / 255.0 * 60.0 + 4.0) as usize),
        Fx::PencilSketch => pencil(b, a.f(0), a.f(1)),
        Fx::Pointillism => pointillism(b, a),
        Fx::StainedGlass => {
            let size = a.f(0);
            let seed = a.seed(6);
            let cfg = TileCfg {
                bevel: a.f(3) / 100.0,
                grout: a.f(2),
                grout_col: col_px(a.col(5)),
                center_mode: a.c(1) == 1,
                light: a.f(4).to_radians(),
                ljitter: 0.0,
                pjitter: 0.0,
                seed,
                size,
            };
            tiles(b, &cfg, |x, y| {
                let (id, f1, f2, c) = voronoi(x, y, size, 1.0, seed);
                ((id.0, id.1, 0), c, (f2 - f1) * 0.5)
            })
        }
        _ => b.clone(),
    }
}

/// Sobel gradient magnitude of a plane.
pub fn sobel(l: &[f32], w: usize, h: usize, prewitt: bool) -> (Vec<f32>, Vec<f32>) {
    let at = |x: isize, y: isize| l[(y.clamp(0, h as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize];
    let c = if prewitt { 1.0 } else { 2.0 };
    let mut gx = vec![0.0; w * h];
    let mut gy = vec![0.0; w * h];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let i = y as usize * w + x as usize;
            gx[i] = (at(x + 1, y - 1) + c * at(x + 1, y) + at(x + 1, y + 1)) - (at(x - 1, y - 1) + c * at(x - 1, y) + at(x - 1, y + 1));
            gy[i] = (at(x - 1, y + 1) + c * at(x, y + 1) + at(x + 1, y + 1)) - (at(x - 1, y - 1) + c * at(x, y - 1) + at(x + 1, y - 1));
        }
    }
    (gx, gy)
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ink_sketch(b: &Buf, outline: f32, coloring: f32) -> Buf {
    let l = gauss_plane(&luma_plane(b), b.w, b.h, 1.5);
    let (gx, gy) = sobel(&l, b.w, b.h, false);
    let thr = 0.5 - outline * 0.45;
    par_map(b.w, b.h, |x, y| {
        let i = y * b.w + x;
        let p = st(b.get(x, y));
        let g = (gx[i] * gx[i] + gy[i] * gy[i]).sqrt();
        let ink = smooth(thr, thr + 0.25, g);
        let lum = luma(p);
        let paper = 1.0 - (1.0 - lum) * 0.35;
        let mut o = [0.0, 0.0, 0.0, p[3]];
        for k in 0..3 {
            let c = paper + (p[k] - lum) * coloring * 1.2;
            o[k] = c * (1.0 - ink);
        }
        pm(o)
    })
}

fn linocut(b: &Buf, a: &A) -> Buf {
    let mode = a.c(0);
    let ang = a.f(1).to_radians();
    let sp = a.f(2).max(2.0);
    let k = 0.5 + a.f(3) / 100.0 * 2.0;
    let gamma = a.f(4);
    let rough = a.f(5) / 100.0;
    let seed = a.seed(6);
    let (ink, paper) = (col_px(a.col(7)), col_px(a.col(8)));
    let tone = gauss_plane(&luma_plane(b), b.w, b.h, 1.5);
    let contour = if mode == 1 { gauss_plane(&luma_plane(b), b.w, b.h, sp) } else { vec![] };
    let (s, c) = ang.sin_cos();
    let stripes = move |u: f32, d: f32, x: f32, y: f32| -> f32 {
        let n = fbm(x / sp * 0.7, y / sp * 0.7, 3, 0.5, seed, false) * rough;
        let ph = (u / sp + n * 0.6).rem_euclid(1.0);
        let dist = (ph - 0.5).abs() * 2.0;
        let d = (d + n * 0.3).clamp(0.0, 1.0);
        ((d - dist) * sp * 0.5 + 0.5).clamp(0.0, 1.0)
    };
    par_map(b.w, b.h, |x, y| {
        let i = y * b.w + x;
        let t = ((tone[i] - 0.5) * k + 0.5).clamp(0.0, 1.0).powf(gamma);
        let d = 1.0 - t;
        let (fx, fy) = (x as f32, y as f32);
        let u = -fx * s + fy * c;
        let cov = match mode {
            1 => {
                let u2 = contour[i] * 255.0 / sp * 6.0 + (fx * c + fy * s) / sp * 0.15;
                stripes(u2 * sp, d, fx, fy)
            }
            2 => {
                let v = fx * c + fy * s;
                let c1 = stripes(u, (d * 1.4).min(1.0), fx, fy);
                let c2 = stripes(v, ((d - 0.45) * 1.8).clamp(0.0, 1.0), fx, fy);
                c1.max(c2)
            }
            _ => stripes(u, d, fx, fy),
        };
        let alpha = st(b.get(x, y))[3];
        let o = lerp4(paper, ink, cov);
        pm([o[0], o[1], o[2], o[3] * alpha])
    })
}

struct TileCfg {
    bevel: f32,
    grout: f32,
    grout_col: Px,
    center_mode: bool,
    light: f32,
    ljitter: f32,
    pjitter: f32,
    seed: u32,
    size: f32,
}

type TileId = (i32, i32, i32);

/// Shared tile renderer used by Mosaic and Stained Glass.
/// `f(x, y)` returns (tile id, tile centre, distance to the nearest tile edge in pixels).
fn tiles(b: &Buf, cfg: &TileCfg, f: impl Fn(f32, f32) -> (TileId, (f32, f32), f32) + Sync) -> Buf {
    let (w, h) = (b.w, b.h);
    // Pass 1: tile lookup for every pixel.
    let mut info: Vec<(TileId, (f32, f32), f32)> = vec![((0, 0, 0), (0.0, 0.0), 0.0); w * h];
    {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let rows = h.div_ceil(threads).max(1);
        std::thread::scope(|s| {
            for (ci, chunk) in info.chunks_mut(rows * w.max(1)).enumerate() {
                let f = &f;
                s.spawn(move || {
                    for (i, v) in chunk.iter_mut().enumerate() {
                        let (x, y) = (i % w, ci * rows + i / w);
                        *v = f(x as f32 + 0.5, y as f32 + 0.5);
                    }
                });
            }
        });
    }
    // Pass 2: tile colours.
    let mut avg: HashMap<TileId, (Px, f32)> = HashMap::new();
    if !cfg.center_mode {
        for (i, t) in info.iter().enumerate() {
            let e = avg.entry(t.0).or_insert(([0.0; 4], 0.0));
            add4(&mut e.0, b.d[i], 1.0);
            e.1 += 1.0;
        }
    }
    let colour = |t: &(TileId, (f32, f32), f32)| -> Px {
        if cfg.center_mode {
            let j = cfg.pjitter * cfg.size * 0.5;
            let ox = (rnd(t.0 .0, t.0 .1, cfg.seed ^ 0x1234 ^ t.0 .2 as u32) - 0.5) * 2.0 * j;
            let oy = (rnd(t.0 .0, t.0 .1, cfg.seed ^ 0x4321 ^ t.0 .2 as u32) - 0.5) * 2.0 * j;
            b.at((t.1 .0 + ox) as i64, (t.1 .1 + oy) as i64, Edge::Clamp)
        } else {
            let (s, n) = avg.get(&t.0).copied().unwrap_or(([0.0; 4], 1.0));
            scale4(s, 1.0 / n.max(1.0))
        }
    };
    let (lx, ly) = (cfg.light.cos(), -cfg.light.sin());
    let bevel_px = (cfg.size * 0.3).max(1.0);
    par_map(w, h, |x, y| {
        let t = &info[y * w + x];
        let mut c = st(colour(t));
        // Lightness jitter per tile.
        let j = 1.0 + (rnd(t.0 .0, t.0 .1, cfg.seed ^ t.0 .2 as u32) - 0.5) * cfg.ljitter;
        // Bevel lighting: pixels near the edge facing the light get brighter.
        let (dx, dy) = (x as f32 + 0.5 - t.1 .0, y as f32 + 0.5 - t.1 .1);
        let dl = (dx * dx + dy * dy).sqrt().max(1e-3);
        let edge_in = (t.2 - cfg.grout * 0.5).max(0.0);
        let fall = 1.0 - (edge_in / bevel_px).min(1.0);
        let shade = 1.0 + (dx / dl * lx + dy / dl * ly) * fall * cfg.bevel * 0.8;
        for k in 0..3 {
            c[k] = (c[k] * j * shade).clamp(0.0, 1.0);
        }
        // Grout / lead lines with 1px antialiasing.
        let g = if cfg.grout > 0.0 { (cfg.grout * 0.5 - t.2 + 0.5).clamp(0.0, 1.0) } else { 0.0 };
        let gc = cfg.grout_col;
        let o = lerp4(c, gc, g);
        pm(o)
    })
}

/// Mosaic tile geometry.
fn tile(shape: usize, x: f32, y: f32, s: f32, seed: u32) -> (TileId, (f32, f32), f32) {
    let sq = |x: f32, y: f32, sw: f32, sh: f32, ox: f32| -> (TileId, (f32, f32), f32) {
        let gx = ((x - ox) / sw).floor();
        let gy = (y / sh).floor();
        let (u, v) = (x - ox - gx * sw, y - gy * sh);
        let e = u.min(sw - u).min(v).min(sh - v);
        ((gx as i32, gy as i32, 0), (ox + (gx + 0.5) * sw, (gy + 0.5) * sh), e)
    };
    match shape {
        1 => {
            // Diamond: square grid rotated 45°.
            let r = std::f32::consts::FRAC_1_SQRT_2;
            let (xr, yr) = ((x + y) * r, (y - x) * r);
            let (id, c, e) = sq(xr, yr, s, s, 0.0);
            (id, ((c.0 - c.1) * r, (c.0 + c.1) * r), e)
        }
        2 => {
            // Equilateral triangles.
            let th = s * 0.866_025_4;
            let row = (y / th).floor();
            let shift = if (row as i32).rem_euclid(2) == 1 { 0.5 * s } else { 0.0 };
            let col = ((x + shift) / s).floor();
            let (lx, ly) = (x + shift - col * s, y - row * th);
            let v = ly / th;
            let fu = lx / s;
            let (verts, sub): ([(f32, f32); 3], i32) = if v >= 2.0 * (fu - 0.5).abs() {
                ([(0.5 * s, 0.0), (s, th), (0.0, th)], 0)
            } else if fu < 0.5 {
                ([(-0.5 * s, 0.0), (0.5 * s, 0.0), (0.0, th)], -1)
            } else {
                ([(0.5 * s, 0.0), (1.5 * s, 0.0), (s, th)], 1)
            };
            let e = tri_edge((lx, ly), verts);
            let cx = (verts[0].0 + verts[1].0 + verts[2].0) / 3.0 + col * s - shift;
            let cy = (verts[0].1 + verts[1].1 + verts[2].1) / 3.0 + row * th;
            ((col as i32 * 2 + sub, row as i32, 2), (cx, cy), e)
        }
        3 => {
            // Pointy-top hexagons.
            let rr = s * 0.6;
            let q = (3f32.sqrt() / 3.0 * x - y / 3.0) / rr;
            let r = (2.0 / 3.0 * y) / rr;
            let (mut cx, mut cz) = (q.round(), r.round());
            let cy = (-q - r).round();
            let (dq, dr, ds) = ((cx - q).abs(), (cz - r).abs(), (cy - (-q - r)).abs());
            if dq > dr && dq > ds {
                cx = -cy - cz;
            } else if dr > ds {
                cz = -cx - cy;
            }
            let px = rr * 3f32.sqrt() * (cx + cz / 2.0);
            let py = rr * 1.5 * cz;
            let (dx, dy) = (x - px, y - py);
            let ap = rr * 0.866_025_4;
            let mut d: f32 = 0.0;
            for k in 0..3 {
                let a = (k as f32 * 60.0).to_radians();
                d = d.max((dx * a.cos() + dy * a.sin()).abs());
            }
            ((cx as i32, cz as i32, 3), (px, py), ap - d)
        }
        4 => {
            // Brick: running bond.
            let row = (y / s).floor();
            let off = if (row as i32).rem_euclid(2) == 1 { s } else { 0.0 };
            let (id, c, e) = sq(x, y, s * 2.0, s, off);
            ((id.0, id.1, 4), c, e)
        }
        5 => {
            // Basketweave: 2×2 blocks of paired rectangles alternating direction.
            let bs = s * 2.0;
            let (bxi, byi) = ((x / bs).floor(), (y / bs).floor());
            let (u, v) = (x - bxi * bs, y - byi * bs);
            let horiz = ((bxi as i32 + byi as i32) & 1) == 0;
            let (k, sx, sy, w, h) = if horiz {
                let k = (v / s).floor();
                (k, 0.0, k * s, bs, s)
            } else {
                let k = (u / s).floor();
                (k, k * s, 0.0, s, bs)
            };
            let (lu, lv) = (u - sx, v - sy);
            let e = lu.min(w - lu).min(lv).min(h - lv);
            ((bxi as i32 * 2 + k as i32, byi as i32, 5 + horiz as i32), (bxi * bs + sx + w / 2.0, byi * bs + sy + h / 2.0), e)
        }
        6 => {
            let (id, f1, f2, c) = voronoi(x, y, s, 0.9, seed);
            ((id.0, id.1, 6), c, (f2 - f1) * 0.5)
        }
        _ => sq(x, y, s, s, 0.0),
    }
}

fn tri_edge(p: (f32, f32), v: [(f32, f32); 3]) -> f32 {
    let mut d = f32::MAX;
    for i in 0..3 {
        let (a, b) = (v[i], v[(i + 1) % 3]);
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let len = (ex * ex + ey * ey).sqrt().max(1e-6);
        d = d.min(((p.0 - a.0) * ey - (p.1 - a.1) * ex).abs() / len);
    }
    d
}

fn oil(b: &Buf, r: i64, levels: usize) -> Buf {
    let lum: Vec<f32> = luma_plane(b);
    par_map(b.w, b.h, |x, y| {
        let mut count = vec![0u32; levels];
        let mut sum = vec![[0.0f32; 4]; levels];
        let (x, y) = (x as i64, y as i64);
        for yy in (y - r).max(0)..=(y + r).min(b.h as i64 - 1) {
            for xx in (x - r).max(0)..=(x + r).min(b.w as i64 - 1) {
                if (xx - x).pow(2) + (yy - y).pow(2) > r * r + r {
                    continue;
                }
                let i = yy as usize * b.w + xx as usize;
                let bin = ((lum[i].clamp(0.0, 0.9999)) * levels as f32) as usize;
                count[bin] += 1;
                add4(&mut sum[bin], b.d[i], 1.0);
            }
        }
        let (best, &n) = count.iter().enumerate().max_by_key(|(_, &c)| c).unwrap();
        if n == 0 {
            b.get(x as usize, y as usize)
        } else {
            scale4(sum[best], 1.0 / n as f32)
        }
    })
}

fn pencil(b: &Buf, tip: f32, range: f32) -> Buf {
    let l = luma_plane(b);
    let inv: Vec<f32> = l.iter().map(|v| 1.0 - v).collect();
    let bl = gauss_plane(&inv, b.w, b.h, tip * 2.0);
    let gamma = 2f32.powf(-range / 10.0);
    par_map(b.w, b.h, |x, y| {
        let i = y * b.w + x;
        let dodge = (l[i] / (1.0 - bl[i]).max(1e-3)).min(1.0);
        let v = dodge.powf(gamma * 1.6);
        pm([v, v, v, st(b.d[i])[3]])
    })
}

fn pointillism(b: &Buf, a: &A) -> Buf {
    let cell = a.f(0).max(2.0);
    let fill = a.f(1) / 100.0;
    let tone = a.f(2) / 100.0;
    let pj = a.f(3) / 100.0;
    let cj = a.f(4) / 100.0;
    let bg = col_px(a.col(5));
    let seed = a.seed(6);
    let dot = |i: i32, j: i32| -> ((f32, f32), f32, Px) {
        let cx = (i as f32 + 0.5 + (rnd(i, j, seed) - 0.5) * pj) * cell;
        let cy = (j as f32 + 0.5 + (rnd(i, j, seed ^ 0x9e37) - 0.5) * pj) * cell;
        let c = st(b.at(cx as i64, cy as i64, Edge::Clamp));
        let l = luma(c);
        let size = 1.0 - tone * (l - 0.5);
        let r = cell * 0.5 * (0.35 + fill * 0.85) * size;
        let (h, s, v) = rgb_to_hsv([c[0], c[1], c[2]]);
        let h2 = h + (rnd(i, j, seed ^ 0x77) - 0.5) * cj * 60.0;
        let v2 = (v + (rnd(i, j, seed ^ 0x99) - 0.5) * cj * 0.4).clamp(0.0, 1.0);
        let rgb = hsv_to_rgb(h2, s, v2);
        ((cx, cy), r, [rgb[0], rgb[1], rgb[2], c[3]])
    };
    par_map(b.w, b.h, |x, y| {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (gi, gj) = ((px / cell).floor() as i32, (py / cell).floor() as i32);
        let mut o = bg;
        for j in gj - 1..=gj + 1 {
            for i in gi - 1..=gi + 1 {
                let ((cx, cy), r, c) = dot(i, j);
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0) * c[3];
                if cov > 0.0 {
                    o = blend(0, o, [c[0], c[1], c[2], cov]);
                }
            }
        }
        pm(o)
    })
}
