//! Stylize: Edge Detect, Emboss, Halftone, Outline, Relief.

use super::*;

pub const HALFTONE_PATTERNS: &[&str] = &["Dots", "Lines", "Squares", "Diamonds", "Crosses"];

pub fn apply(kind: Fx, a: &A, b: &Buf, _bx: Bx) -> Buf {
    match kind {
        Fx::EdgeDetect => {
            let strength = a.f(0);
            let l = gauss_plane(&luma_plane(b), b.w, b.h, a.f(1));
            let (gx, gy) = super::artistic::sobel(&l, b.w, b.h, a.c(2) == 1);
            let overlay = a.b(3);
            let norm = if a.c(2) == 1 { 3.0 } else { 4.0 };
            par_map(b.w, b.h, |x, y| {
                let i = y * b.w + x;
                let m = ((gx[i] * gx[i] + gy[i] * gy[i]).sqrt() / norm * strength * 2.0).clamp(0.0, 1.0);
                let p = st(b.d[i]);
                let e = [p[0] * m + m * 0.25, p[1] * m + m * 0.25, p[2] * m + m * 0.25];
                if overlay {
                    pm([1.0 - (1.0 - p[0]) * (1.0 - e[0]), 1.0 - (1.0 - p[1]) * (1.0 - e[1]), 1.0 - (1.0 - p[2]) * (1.0 - e[2]), p[3]])
                } else {
                    pm([e[0], e[1], e[2], p[3]])
                }
            })
        }
        Fx::Emboss | Fx::Relief => {
            let ang = a.f(0).to_radians();
            let (dx, dy) = (ang.cos(), -ang.sin());
            let l = luma_plane(b);
            let lb = Buf { w: b.w, h: b.h, d: l.iter().map(|&v| [v, v, v, 1.0]).collect() };
            let relief = kind == Fx::Relief;
            par_map(b.w, b.h, |x, y| {
                let (fx, fy) = (x as f32, y as f32);
                let d = lb.sample(fx - dx, fy - dy, Edge::Clamp)[0] - lb.sample(fx + dx, fy + dy, Edge::Clamp)[0];
                let p = st(b.get(x, y));
                if relief {
                    pm([p[0] + d, p[1] + d, p[2] + d, p[3]])
                } else {
                    let g = 0.5 + d * 2.0;
                    pm([g, g, g, p[3]])
                }
            })
        }
        Fx::Halftone => halftone(a, b),
        Fx::Outline => {
            let r = a.i(0).max(1) as usize;
            let int = a.f(1) / 100.0;
            let l = luma_plane(b);
            let lb = Buf { w: b.w, h: b.h, d: l.iter().map(|&v| [v, v, v, 1.0]).collect() };
            let lo = super::distort::morph_pub(&lb, r, r, false);
            let hi = super::distort::morph_pub(&lb, r, r, true);
            let dark = super::distort::morph_pub(b, r, r, false);
            par_map(b.w, b.h, |x, y| {
                let i = y * b.w + x;
                let range = hi.d[i][0] - lo.d[i][0];
                let e = ((range - (1.0 - int) * 0.6) * 4.0).clamp(0.0, 1.0);
                let p = st(b.d[i]);
                let d = st(dark.d[i]);
                pm([1.0 + (d[0] - 1.0) * e, 1.0 + (d[1] - 1.0) * e, 1.0 + (d[2] - 1.0) * e, p[3]])
            })
        }
        _ => b.clone(),
    }
}

/// Ink coverage of one halftone screen at a point.
fn screen(pattern: usize, x: f32, y: f32, cell: f32, ang: f32, soft: f32, tone: &dyn Fn(f32, f32) -> f32) -> f32 {
    let (s, c) = ang.sin_cos();
    let (u, v) = ((x * c + y * s) / cell, (-x * s + y * c) / cell);
    let (iu, iv) = (u.floor() + 0.5, v.floor() + 0.5);
    let (fu, fv) = (u - iu, v - iv);
    // tone sampled at the cell centre (rotated back into image space)
    let (cx, cy) = ((iu * c - iv * s) * cell, (iu * s + iv * c) * cell);
    let k = tone(cx, cy).clamp(0.0, 1.0);
    let (d, r) = match pattern {
        1 => (fv.abs(), k * 0.5),
        2 => (fu.abs().max(fv.abs()), k.sqrt() * 0.5),
        3 => (fu.abs() + fv.abs(), (k * 0.5).sqrt()),
        4 => (fu.abs().min(fv.abs()), (1.0 - (1.0 - k).sqrt()) * 0.5),
        _ => ((fu * fu + fv * fv).sqrt(), (k / std::f32::consts::PI).sqrt()),
    };
    let aa = 1.0 / cell + soft;
    ((r - d) / aa + 0.5).clamp(0.0, 1.0)
}

fn halftone(a: &A, b: &Buf) -> Buf {
    let pattern = a.c(0);
    let mono = a.b(1);
    let cell = a.f(2).max(2.0);
    let ang = a.f(3).to_radians();
    let bg = a.f(4) / 100.0;
    let soft = a.f(5) / 100.0 * 0.3;
    let offs = ss(a.i(6));
    let tone_at = |x: f32, y: f32| st(b.at(x.floor() as i64, y.floor() as i64, Edge::Clamp));
    par_map(b.w, b.h, |x, y| {
        let mut acc = [0.0f32; 3];
        for &(ox, oy) in &offs {
            let (px, py) = (x as f32 + ox, y as f32 + oy);
            if mono {
                let ink = screen(pattern, px, py, cell, ang, soft, &|cx, cy| 1.0 - luma(tone_at(cx, cy)));
                for v in acc.iter_mut() {
                    *v += 1.0 - ink;
                }
            } else {
                let cmyk = |cx: f32, cy: f32| {
                    let p = tone_at(cx, cy);
                    let k = bg * (1.0 - p[0].max(p[1]).max(p[2]));
                    let d = (1.0 - k).max(1e-4);
                    [(1.0 - p[0] - k) / d, (1.0 - p[1] - k) / d, (1.0 - p[2] - k) / d, k]
                };
                let base = ang - 45f32.to_radians();
                let angles = [15f32, 75.0, 0.0, 45.0];
                let mut ink = [0.0; 4];
                for ch in 0..4 {
                    ink[ch] = screen(pattern, px, py, cell, base + angles[ch].to_radians(), soft, &|cx, cy| cmyk(cx, cy)[ch]);
                }
                for k in 0..3 {
                    acc[k] += (1.0 - ink[k]) * (1.0 - ink[3]);
                }
            }
        }
        let n = offs.len() as f32;
        pm([acc[0] / n, acc[1] / n, acc[2] / n, st(b.get(x, y))[3]])
    })
}
