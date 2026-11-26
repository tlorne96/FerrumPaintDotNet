//! Blurs: Bokeh, Fragment, Gaussian, Median, Motion, Radial, Sketch, Square, Surface, Zoom.

use super::*;

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx) -> Buf {
    match kind {
        Fx::BokehBlur => {
            let g = a.f(1);
            let step = match a.i(2) {
                1 => 3,
                2 => 2,
                _ => 1,
            };
            unboost(&disc_blur(&boost(b, g), a.i(0).max(1) as i64, step), g)
        }
        Fx::FragmentBlur => {
            let n = a.i(0).max(2);
            let d = a.f(1);
            let rot = a.f(2).to_radians();
            let offs: Vec<(f32, f32)> = (0..n)
                .map(|i| {
                    let t = rot + i as f32 / n as f32 * std::f32::consts::TAU;
                    (d * t.cos(), -d * t.sin())
                })
                .collect();
            par_map(b.w, b.h, |x, y| {
                let mut acc = [0.0; 4];
                for &(ox, oy) in &offs {
                    add4(&mut acc, b.sample(x as f32 - ox, y as f32 - oy, Edge::Clamp), 1.0);
                }
                scale4(acc, 1.0 / n as f32)
            })
        }
        Fx::GaussianBlur => {
            let g = a.f(1);
            unboost(&gauss(&boost(b, g), a.f(0), a.i(2).clamp(1, 5)), g)
        }
        Fx::MedianBlur => {
            let bins = [16usize, 32, 64, 128, 256][(a.i(2).clamp(1, 5) - 1) as usize];
            median(b, a.i(0).max(1) as usize, a.f(1) / 100.0, bins)
        }
        Fx::MotionBlur => {
            let ang = a.f(0).to_radians();
            let dist = a.f(1).max(1.0);
            let centered = a.b(2);
            let edge = Edge::from(a.c(3));
            let (dx, dy) = (ang.cos(), -ang.sin());
            let n = dist.ceil() as i32 + 1;
            par_map(b.w, b.h, |x, y| {
                let mut acc = [0.0; 4];
                for i in 0..n {
                    let t = i as f32 / (n - 1).max(1) as f32 * dist - if centered { dist * 0.5 } else { 0.0 };
                    add4(&mut acc, b.sample(x as f32 - dx * t, y as f32 - dy * t, edge), 1.0);
                }
                scale4(acc, 1.0 / n as f32)
            })
        }
        Fx::RadialBlur => {
            let ang = a.f(0).to_radians();
            let (cx, cy) = bx.point(a.pt(1));
            let q = a.i(2).clamp(1, 5) as f32;
            par_map(b.w, b.h, |x, y| {
                let (vx, vy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let r = (vx * vx + vy * vy).sqrt();
                let n = ((r * ang * q * 0.5).ceil() as i32).clamp(1, 48 * q as i32);
                let mut acc = [0.0; 4];
                for i in 0..n {
                    let t = if n == 1 { 0.0 } else { (i as f32 / (n - 1) as f32 - 0.5) * ang };
                    let (s, c) = t.sin_cos();
                    let (sx, sy) = (cx + vx * c - vy * s, cy + vx * s + vy * c);
                    add4(&mut acc, b.sample(sx - 0.5, sy - 0.5, Edge::Clamp), 1.0);
                }
                scale4(acc, 1.0 / n as f32)
            })
        }
        Fx::SketchBlur => {
            let r = a.f(0).max(1.0);
            let pct = a.f(1) / 100.0;
            let k = (6 + a.i(2) * 6) as usize;
            let seed = a.seed(3);
            par_map(b.w, b.h, |x, y| {
                let mut s: Vec<(f32, Px)> = Vec::with_capacity(k);
                for i in 0..k {
                    let u = rnd(x as i32 * 31 + i as i32, y as i32, seed);
                    let v = rnd(y as i32 * 17 + i as i32, x as i32, seed ^ 0xabc);
                    let (rr, t) = (r * u.sqrt(), v * std::f32::consts::TAU);
                    let p = b.at((x as f32 + rr * t.cos()).round() as i64, (y as f32 + rr * t.sin()).round() as i64, Edge::Clamp);
                    s.push((luma(st(p)), p));
                }
                s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let c = ((k - 1) as f32 * pct).round() as usize;
                let (lo, hi) = (c.saturating_sub(1), (c + 1).min(k - 1));
                let mut acc = [0.0; 4];
                for item in &s[lo..=hi] {
                    add4(&mut acc, item.1, 1.0);
                }
                scale4(acc, 1.0 / (hi - lo + 1) as f32)
            })
        }
        Fx::SquareBlur => {
            let g = a.f(1);
            let r = a.i(0).max(1) as usize;
            unboost(&box_blur(&boost(b, g), r, r), g)
        }
        Fx::SurfaceBlur => surface(b, a.f(0), a.f(1) / 100.0),
        Fx::ZoomBlur => {
            let amount = a.f(0) / 100.0;
            let focus = a.f(1) / 100.0;
            let (cx, cy) = bx.point(a.pt(2));
            let q = a.i(3).clamp(1, 5) as f32;
            let maxd = (bx.w().powi(2) + bx.h().powi(2)).sqrt() * 0.5;
            par_map(b.w, b.h, |x, y| {
                let (vx, vy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (vx * vx + vy * vy).sqrt();
                let f = if focus >= 1.0 { 0.0 } else { ((d / maxd.max(1.0) - focus) / (1.0 - focus)).clamp(0.0, 1.0) };
                let span = amount * f;
                let n = ((d * span * q * 0.5).ceil() as i32).clamp(1, 48 * q as i32);
                let mut acc = [0.0; 4];
                for i in 0..n {
                    let t = 1.0 - span * i as f32 / n as f32;
                    add4(&mut acc, b.sample(cx + vx * t - 0.5, cy + vy * t - 0.5, Edge::Clamp), 1.0);
                }
                scale4(acc, 1.0 / n as f32)
            })
        }
        _ => b.clone(),
    }
}

/// Gamma boost: raise straight colour to `g` (keeping premultiplication) before blurring.
fn boost(b: &Buf, g: f32) -> Buf {
    if (g - 1.0).abs() < 1e-3 {
        return b.clone();
    }
    map_st(b, |p| [p[0].powf(g), p[1].powf(g), p[2].powf(g), p[3]])
}
fn unboost(b: &Buf, g: f32) -> Buf {
    boost(b, 1.0 / g)
}

/// Uniform disc kernel using per-row prefix sums: O(r) per pixel.
fn disc_blur(b: &Buf, r: i64, step: i64) -> Buf {
    let (w, h) = (b.w, b.h);
    let mut pre = vec![[0.0f32; 4]; (w + 1) * h];
    for y in 0..h {
        let mut acc = [0.0f32; 4];
        for x in 0..w {
            add4(&mut acc, b.d[y * w + x], 1.0);
            pre[y * (w + 1) + x + 1] = acc;
        }
    }
    let spans: Vec<(i64, i64)> = (-r..=r).step_by(step as usize).map(|dy| (dy, (((r * r - dy * dy) as f32).sqrt()).floor() as i64)).collect();
    par_map(w, h, |x, y| {
        let mut acc = [0.0f32; 4];
        let mut n = 0.0;
        for &(dy, hw) in &spans {
            let yy = y as i64 + dy;
            if yy < 0 || yy >= h as i64 {
                continue;
            }
            let x0 = (x as i64 - hw).max(0) as usize;
            let x1 = (x as i64 + hw).min(w as i64 - 1) as usize;
            let row = yy as usize * (w + 1);
            let (a, c) = (pre[row + x1 + 1], pre[row + x0]);
            for k in 0..4 {
                acc[k] += a[k] - c[k];
            }
            n += (x1 - x0 + 1) as f32;
        }
        scale4(acc, 1.0 / n.max(1.0))
    })
}

/// Sliding-histogram percentile filter on each premultiplied channel.
pub fn median(b: &Buf, r: usize, pct: f32, bins: usize) -> Buf {
    let (w, h) = (b.w, b.h);
    let q: Vec<[u16; 4]> = b
        .d
        .iter()
        .map(|p| {
            let f = |v: f32| ((v.clamp(0.0, 1.0) * (bins - 1) as f32).round()) as u16;
            [f(p[0]), f(p[1]), f(p[2]), f(p[3])]
        })
        .collect();
    let mut out = Buf::new(w, h);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let rows = h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in out.d.chunks_mut(rows * w.max(1)).enumerate() {
            let q = &q;
            s.spawn(move || {
                let mut hist = vec![0u32; bins * 4];
                for (ri, orow) in chunk.chunks_mut(w).enumerate() {
                    let y = ci * rows + ri;
                    let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
                    hist.iter_mut().for_each(|v| *v = 0);
                    let mut count = 0u32;
                    let col = |hist: &mut Vec<u32>, x: usize, sign: i32| {
                        for yy in y0..=y1 {
                            let v = q[yy * w + x];
                            for k in 0..4 {
                                let i = k * bins + v[k] as usize;
                                hist[i] = (hist[i] as i32 + sign) as u32;
                            }
                        }
                    };
                    for x in 0..=r.min(w - 1) {
                        col(&mut hist, x, 1);
                        count += (y1 - y0 + 1) as u32;
                    }
                    for (x, o) in orow.iter_mut().enumerate() {
                        let target = ((count.max(1) - 1) as f32 * pct).round() as u32;
                        for k in 0..4 {
                            let mut c = 0;
                            let mut val = bins - 1;
                            for i in 0..bins {
                                c += hist[k * bins + i];
                                if c > target {
                                    val = i;
                                    break;
                                }
                            }
                            o[k] = val as f32 / (bins - 1) as f32;
                        }
                        o[0] = o[0].min(o[3]);
                        o[1] = o[1].min(o[3]);
                        o[2] = o[2].min(o[3]);
                        // slide the window
                        if x + r + 1 < w {
                            col(&mut hist, x + r + 1, 1);
                            count += (y1 - y0 + 1) as u32;
                        }
                        if x >= r {
                            col(&mut hist, x - r, -1);
                            count -= (y1 - y0 + 1) as u32;
                        }
                    }
                }
            });
        }
    });
    out
}

/// Edge-preserving blur: separable bilateral filter.
fn surface(b: &Buf, r: f32, thr: f32) -> Buf {
    let ri = r.ceil() as isize;
    let sig_s = (r / 2.0).max(0.5);
    let sig_r = (thr * 0.5).max(0.01);
    let ws: Vec<f32> = (-ri..=ri).map(|d| (-(d * d) as f32 / (2.0 * sig_s * sig_s)).exp()).collect();
    let pass = |row: &[Px], out: &mut [Px]| {
        let n = row.len() as isize;
        for x in 0..n {
            let c = st(row[x as usize]);
            let mut acc = [0.0; 4];
            let mut wsum = 0.0;
            for d in -ri..=ri {
                let p = row[(x + d).clamp(0, n - 1) as usize];
                let s = st(p);
                let diff = (s[0] - c[0]).abs().max((s[1] - c[1]).abs()).max((s[2] - c[2]).abs());
                let wgt = ws[(d + ri) as usize] * (-(diff * diff) / (2.0 * sig_r * sig_r)).exp();
                add4(&mut acc, p, wgt);
                wsum += wgt;
            }
            out[x as usize] = scale4(acc, 1.0 / wsum.max(1e-6));
        }
    };
    let h = par_rows(b, pass);
    let t = par_rows(&transpose(&h), pass);
    transpose(&t)
}
