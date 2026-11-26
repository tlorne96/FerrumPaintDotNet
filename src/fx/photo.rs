//! Color (Quantize), Noise (Add / Reduce), Object (Drop Shadow) and Photo effects.

use super::*;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx, src: &Pixmap) -> Buf {
    match kind {
        Fx::Quantize => quantize(b, a.i(0).clamp(2, 256) as usize, a.c(1) == 1, a.f(2) / 8.0, a.b(3)),
        Fx::AddNoise => {
            let amp = a.f(0) / 100.0 * 0.5;
            let sat = a.f(1) / 100.0;
            let cov = a.f(2) / 100.0;
            let seed = a.seed(3);
            par_map(b.w, b.h, |x, y| {
                let (xi, yi) = (x as i32, y as i32);
                let p = st(b.get(x, y));
                if rnd(xi, yi, seed) >= cov || p[3] <= 0.0 {
                    return b.get(x, y);
                }
                let n = |s: u32| (rnd(xi, yi, seed ^ s) * 2.0 - 1.0) * amp;
                let base = n(0x111);
                let mut o = p;
                for (k, salt) in [0x222u32, 0x333, 0x444].iter().enumerate() {
                    let cn = n(*salt) * sat + base * (1.0 - sat.min(1.0));
                    o[k] = (p[k] + cn).clamp(0.0, 1.0);
                }
                pm(o)
            })
        }
        Fx::ReduceNoise => {
            let m = super::blurs::median(b, a.i(0).max(1) as usize, 0.5, 256);
            let s = a.f(1);
            par_map(b.w, b.h, |x, y| lerp4(b.get(x, y), m.get(x, y), s))
        }
        Fx::DropShadow => {
            let r = a.f(0);
            let dist = a.f(1);
            let ang = a.f(2).to_radians();
            let op = a.f(3) / 255.0;
            let c = col_px(a.col(4));
            let only = a.b(5);
            let (dx, dy) = (ang.cos() * dist, -ang.sin() * dist);
            let alpha = par_map(b.w, b.h, |x, y| {
                let v = b.sample(x as f32 - dx, y as f32 - dy, Edge::Transparent)[3];
                [v, v, v, v]
            });
            let sh = gauss(&alpha, r, 3);
            par_map(b.w, b.h, |x, y| {
                let s = [c[0], c[1], c[2], sh.get(x, y)[3] * op * c[3]];
                if only {
                    pm(s)
                } else {
                    pm(blend(0, s, st(b.get(x, y))))
                }
            })
        }
        Fx::Glow => {
            let g = gauss(b, a.f(0) * 2.0, 3);
            let br = a.f(1) / 100.0 * 0.5;
            let c = a.f(2) / 100.0;
            let k = if c >= 0.0 { 1.0 / (1.0 - c * 0.95) } else { 1.0 + c };
            par_map(b.w, b.h, |x, y| {
                let o = st(b.get(x, y));
                let gl = st(g.get(x, y));
                let mut r = o;
                for i in 0..3 {
                    let gv = ((gl[i] - 0.5) * k + 0.5 + br).clamp(0.0, 1.0);
                    r[i] = 1.0 - (1.0 - o[i]) * (1.0 - gv);
                }
                pm(r)
            })
        }
        Fx::RedEyeRemoval => {
            let tol = a.f(0) / 100.0;
            let s = a.f(1) / 100.0;
            map_st(b, |p| {
                let redness = p[0] - p[1].max(p[2]);
                let thr = 0.35 * (1.0 - tol) + 0.02;
                let amt = ((redness - thr) / 0.15).clamp(0.0, 1.0) * s;
                let target = (p[1] + p[2]) * 0.5;
                [p[0] + (target - p[0]) * amt, p[1], p[2], p[3]]
            })
        }
        Fx::Sharpen => {
            let amount = a.f(0);
            let thr = a.f(1) / 255.0;
            let bl = gauss(b, 1.0 + amount * 0.1, 3);
            let k = amount / 8.0;
            par_map(b.w, b.h, |x, y| {
                let (o, g) = (st(b.get(x, y)), st(bl.get(x, y)));
                let mut r = o;
                for i in 0..3 {
                    let d = o[i] - g[i];
                    if d.abs() > thr {
                        r[i] = o[i] + d * k;
                    }
                }
                pm(r)
            })
        }
        Fx::SoftenPortrait => {
            let soft = a.f(0) / 10.0;
            let light = a.f(1) / 100.0;
            let warm = a.f(2) / 100.0;
            let g = gauss(b, 1.0 + a.f(0) * 1.5, 3);
            par_map(b.w, b.h, |x, y| {
                let (o, gl) = (st(b.get(x, y)), st(g.get(x, y)));
                let mut r = o;
                for i in 0..3 {
                    let screen = 1.0 - (1.0 - o[i]) * (1.0 - gl[i]);
                    let m = o[i] + (gl[i] * 0.6 + screen * 0.4 - o[i]) * soft * 0.6;
                    r[i] = m + light;
                }
                r[0] *= 1.0 + warm * 0.6;
                r[1] *= 1.0 + warm * 0.15;
                r[2] *= 1.0 - warm * 0.6;
                pm(r)
            })
        }
        Fx::Straighten => {
            let ang = a.f(0).to_radians();
            let q = [FilterQuality::Bicubic, FilterQuality::Bilinear, FilterQuality::Nearest][a.c(1).min(2)];
            let (w, h) = (bx.w().max(1.0), bx.h().max(1.0));
            let (s, c) = (ang.sin().abs(), ang.cos().abs());
            let scale = ((w * c + h * s) / w).max((w * s + h * c) / h);
            let t = Transform::from_translate(bx.cx(), bx.cy())
                .pre_concat(Transform::from_rotate(a.f(0)))
                .pre_concat(Transform::from_scale(scale, scale))
                .pre_concat(Transform::from_translate(-bx.cx(), -bx.cy()));
            let mut out = Pixmap::new(src.width(), src.height()).unwrap();
            out.draw_pixmap(0, 0, src.as_ref(), &PixmapPaint { quality: q, ..Default::default() }, t, None);
            Buf::from_pixmap(&out)
        }
        Fx::Vignette => {
            let (cx, cy) = bx.point(a.pt(0));
            let rad = a.f(1).max(0.05);
            let strength = a.f(2);
            let (hw, hh) = (bx.w() * 0.5, bx.h() * 0.5);
            par_map(b.w, b.h, |x, y| {
                let (u, v) = ((x as f32 + 0.5 - cx) / hw.max(1.0), (y as f32 + 0.5 - cy) / hh.max(1.0));
                let t = (u * u + v * v).sqrt() / (rad * std::f32::consts::SQRT_2);
                let f = 1.0 - strength * (t.clamp(0.0, 1.0)).powi(2) * (3.0 - 2.0 * t.clamp(0.0, 1.0));
                let p = st(b.get(x, y));
                pm([p[0] * f, p[1] * f, p[2] * f, p[3]])
            })
        }
        _ => b.clone(),
    }
}

// ----------------------------------------------------------------------------------------------
// Quantize

fn quantize(b: &Buf, n: usize, octree: bool, dither: f32, keep_alpha: bool) -> Buf {
    let total = b.w * b.h;
    let step = (total / 400_000).max(1);
    let mut cols: Vec<[u8; 3]> = Vec::with_capacity(total / step + 1);
    for p in b.d.iter().step_by(step) {
        let s = st(*p);
        if s[3] > 0.0 {
            cols.push([(s[0] * 255.0).round() as u8, (s[1] * 255.0).round() as u8, (s[2] * 255.0).round() as u8]);
        }
    }
    if cols.is_empty() {
        return b.clone();
    }
    let pal = if octree { octree_palette(&cols, n) } else { median_cut(cols, n) };
    // Nearest-colour cache on a 5-bit grid.
    let mut cache = vec![0u16; 32 * 32 * 32];
    for (i, c) in cache.iter_mut().enumerate() {
        let q = [((i >> 10) & 31) as f32 * 8.0 + 4.0, ((i >> 5) & 31) as f32 * 8.0 + 4.0, (i & 31) as f32 * 8.0 + 4.0];
        *c = nearest(&pal, q) as u16;
    }
    let look = |c: [f32; 3]| -> usize {
        let k = |v: f32| (v.clamp(0.0, 255.0) as usize) >> 3;
        cache[(k(c[0]) << 10) | (k(c[1]) << 5) | k(c[2])] as usize
    };
    let (w, h) = (b.w, b.h);
    let mut err = vec![[0.0f32; 3]; (w + 2) * 2];
    let mut out = Buf::new(w, h);
    for y in 0..h {
        let (cur, nxt) = ((y % 2) * (w + 2), ((y + 1) % 2) * (w + 2));
        for e in err[nxt..nxt + w + 2].iter_mut() {
            *e = [0.0; 3];
        }
        for x in 0..w {
            let s = st(b.d[y * w + x]);
            let a = if keep_alpha { s[3] } else if s[3] >= 0.5 { 1.0 } else { 0.0 };
            if s[3] <= 0.0 {
                out.d[y * w + x] = [0.0; 4];
                continue;
            }
            let e = err[cur + x + 1];
            let want = [s[0] * 255.0 + e[0], s[1] * 255.0 + e[1], s[2] * 255.0 + e[2]];
            let pc = pal[look(want)];
            let got = [pc[0] as f32, pc[1] as f32, pc[2] as f32];
            out.d[y * w + x] = pm([got[0] / 255.0, got[1] / 255.0, got[2] / 255.0, a]);
            if dither > 0.0 {
                for k in 0..3 {
                    let d = (want[k] - got[k]) * dither;
                    err[cur + x + 2][k] += d * 7.0 / 16.0;
                    err[nxt + x][k] += d * 3.0 / 16.0;
                    err[nxt + x + 1][k] += d * 5.0 / 16.0;
                    err[nxt + x + 2][k] += d / 16.0;
                }
            }
        }
    }
    out
}

fn nearest(pal: &[[u8; 3]], c: [f32; 3]) -> usize {
    let mut best = (f32::MAX, 0);
    for (i, p) in pal.iter().enumerate() {
        let d = (p[0] as f32 - c[0]).powi(2) * 0.30 + (p[1] as f32 - c[1]).powi(2) * 0.59 + (p[2] as f32 - c[2]).powi(2) * 0.11;
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

fn median_cut(cols: Vec<[u8; 3]>, n: usize) -> Vec<[u8; 3]> {
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![cols];
    while boxes.len() < n {
        // split the box with the largest channel range
        let mut best: Option<(usize, usize, u8)> = None;
        for (i, bx) in boxes.iter().enumerate() {
            if bx.len() < 2 {
                continue;
            }
            for k in 0..3 {
                let (mn, mx) = bx.iter().fold((255u8, 0u8), |(a, b), c| (a.min(c[k]), b.max(c[k])));
                let r = mx - mn;
                if r > 0 && best.map(|b| r > b.2).unwrap_or(true) {
                    best = Some((i, k, r));
                }
            }
        }
        let Some((i, k, _)) = best else { break };
        let mut bx = boxes.swap_remove(i);
        bx.sort_unstable_by_key(|c| c[k]);
        let hi = bx.split_off(bx.len() / 2);
        boxes.push(bx);
        boxes.push(hi);
    }
    boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let s = b.iter().fold([0u64; 3], |a, c| [a[0] + c[0] as u64, a[1] + c[1] as u64, a[2] + c[2] as u64]);
            let l = b.len() as u64;
            [(s[0] / l) as u8, (s[1] / l) as u8, (s[2] / l) as u8]
        })
        .collect()
}

struct Node {
    kids: [u32; 8],
    sum: [u64; 3],
    count: u64,
    leaf: bool,
}

fn octree_palette(cols: &[[u8; 3]], n: usize) -> Vec<[u8; 3]> {
    const DEPTH: usize = 6;
    let mut nodes = vec![Node { kids: [0; 8], sum: [0; 3], count: 0, leaf: false }];
    let mut levels: Vec<Vec<u32>> = vec![vec![]; DEPTH];
    let mut leaves = 0usize;
    for c in cols {
        let mut ni = 0usize;
        for lvl in 0..DEPTH {
            nodes[ni].count += 1;
            let shift = 7 - lvl;
            let idx = (((c[0] >> shift) & 1) << 2 | ((c[1] >> shift) & 1) << 1 | ((c[2] >> shift) & 1)) as usize;
            if nodes[ni].kids[idx] == 0 {
                nodes.push(Node { kids: [0; 8], sum: [0; 3], count: 0, leaf: lvl + 1 == DEPTH });
                let id = (nodes.len() - 1) as u32;
                nodes[ni].kids[idx] = id;
                if lvl + 1 == DEPTH {
                    leaves += 1;
                } else {
                    levels[lvl + 1].push(id);
                }
            }
            ni = nodes[ni].kids[idx] as usize;
        }
        let nd = &mut nodes[ni];
        nd.count += 1;
        for k in 0..3 {
            nd.sum[k] += c[k] as u64;
        }
    }
    levels[0].push(0);
    // Merge the least-used deepest internal nodes until the leaf count fits.
    let mut lvl = DEPTH - 1;
    while leaves > n {
        while levels[lvl].is_empty() {
            if lvl == 0 {
                break;
            }
            lvl -= 1;
        }
        if levels[lvl].is_empty() {
            break;
        }
        let (pos, _) = levels[lvl].iter().enumerate().min_by_key(|(_, &id)| nodes[id as usize].count).unwrap();
        let id = levels[lvl].swap_remove(pos) as usize;
        let mut sum = [0u64; 3];
        let mut cnt = 0;
        let mut kids = 0;
        for k in 0..8 {
            let c = nodes[id].kids[k] as usize;
            if c != 0 {
                for j in 0..3 {
                    sum[j] += nodes[c].sum[j];
                }
                cnt += nodes[c].count;
                kids += 1;
                nodes[c].leaf = false;
                nodes[c].count = 0;
            }
        }
        let nd = &mut nodes[id];
        nd.kids = [0; 8];
        nd.sum = sum;
        nd.count = cnt;
        nd.leaf = true;
        leaves = leaves + 1 - kids;
    }
    nodes
        .iter()
        .filter(|n| n.leaf && n.count > 0)
        .map(|n| [(n.sum[0] / n.count) as u8, (n.sum[1] / n.count) as u8, (n.sum[2] / n.count) as u8])
        .collect()
}
