//! Distort: Bulge, Crystalize, Dents, Frosted Glass, Morphology, Pixelate, Polar Inversion,
//! Spherize, Tile Reflection, Twist, Waves.

use super::*;
use std::f32::consts::{PI, TAU};

pub const WAVE_PATTERNS: &[&str] = &["Linear", "Radial", "Tangential", "Angular", "Spiral", "Square rings", "Diamond rings", "From edge"];

fn rot(x: f32, y: f32, a: f32) -> (f32, f32) {
    let (s, c) = a.sin_cos();
    (x * c - y * s, x * s + y * c)
}

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx) -> Buf {
    match kind {
        Fx::Bulge => {
            let k = a.f(0) / 100.0;
            let (cx, cy) = bx.point(a.pt(1));
            let r = bx.w().min(bx.h()) * 0.5;
            distort(b, a.i(3), Edge::from(a.c(2)), |x, y| {
                let (vx, vy) = (x - cx, y - cy);
                let t = (vx * vx + vy * vy).sqrt() / r.max(1.0);
                if t >= 1.0 {
                    return (x, y);
                }
                let f = (1.0 - k * 0.5 * (1.0 - t * t).powi(2)).max(0.05);
                (cx + vx * f, cy + vy * f)
            })
        }
        Fx::Crystalize => {
            let size = a.f(0).max(2.0);
            let seed = a.seed(2);
            let offs = ss(a.i(1));
            let n = offs.len() as f32;
            par_map(b.w, b.h, |x, y| {
                let mut acc = [0.0; 4];
                for &(ox, oy) in &offs {
                    let (_, _, _, c) = voronoi(x as f32 + ox, y as f32 + oy, size, 1.0, seed);
                    add4(&mut acc, b.at(c.0 as i64, c.1 as i64, Edge::Clamp), 1.0);
                }
                scale4(acc, 1.0 / n)
            })
        }
        Fx::Dents => {
            let s = a.f(0).max(1.0);
            let refr = a.f(1) / 100.0 * s * 0.6;
            let oct = 1 + (a.f(2) / 25.0) as i32;
            let pers = 0.3 + a.f(3) / 100.0 * 0.6;
            let ang = a.f(4).to_radians();
            let seed = a.seed(7);
            distort(b, a.i(6), Edge::from(a.c(5)), |x, y| {
                let (u, v) = rot(x / s, y / s, ang);
                let dx = fbm(u, v, oct, pers, seed, false);
                let dy = fbm(u + 31.7, v - 17.3, oct, pers, seed ^ 0x5151, false);
                (x + dx * refr, y + dy * refr)
            })
        }
        Fx::FrostedGlass => {
            let (mx, mn) = (a.f(0), a.f(1));
            let (lo, hi) = (mn.min(mx), mx.max(mn));
            let diff = a.f(2) / 100.0;
            let k = a.i(3).max(1);
            let seed = a.seed(4);
            par_map(b.w, b.h, |x, y| {
                let (xi, yi) = (x as i32, y as i32);
                if rnd(xi, yi, seed ^ 0xf00d) >= diff || hi <= 0.0 {
                    return b.get(x, y);
                }
                let mut acc = [0.0; 4];
                for i in 0..k {
                    let r = lo + (hi - lo) * rnd(xi * 7 + i, yi, seed);
                    let t = rnd(xi, yi * 13 + i, seed ^ 0x77) * TAU;
                    let sx = x as f32 + r * t.cos();
                    let sy = y as f32 + r * t.sin();
                    let p = if k == 1 { b.at(sx.round() as i64, sy.round() as i64, Edge::Clamp) } else { b.sample(sx, sy, Edge::Clamp) };
                    add4(&mut acc, p, 1.0);
                }
                scale4(acc, 1.0 / k as f32)
            })
        }
        Fx::Morphology => {
            let dilate = a.c(0) == 1;
            let w = a.i(1).max(1) as usize;
            let h = if a.b(3) { w } else { a.i(2).max(1) as usize };
            morph_pub(b, w, h, dilate)
        }
        Fx::Pixelate => {
            let c = a.i(0).max(1) as usize;
            let centre = a.c(1) == 1;
            let bilinear = a.c(2) == 1;
            let (gw, gh) = (b.w.div_ceil(c), b.h.div_ceil(c));
            let mut g = Buf::new(gw, gh);
            for gy in 0..gh {
                for gx in 0..gw {
                    let (x0, y0) = (gx * c, gy * c);
                    let (x1, y1) = ((x0 + c).min(b.w), (y0 + c).min(b.h));
                    g.d[gy * gw + gx] = if centre {
                        b.get((x0 + x1) / 2, (y0 + y1) / 2)
                    } else {
                        let mut acc = [0.0; 4];
                        for yy in y0..y1 {
                            for xx in x0..x1 {
                                add4(&mut acc, b.get(xx, yy), 1.0);
                            }
                        }
                        scale4(acc, 1.0 / ((x1 - x0) * (y1 - y0)) as f32)
                    };
                }
            }
            par_map(b.w, b.h, |x, y| {
                if bilinear {
                    g.sample((x as f32 + 0.5) / c as f32 - 0.5, (y as f32 + 0.5) / c as f32 - 0.5, Edge::Clamp)
                } else {
                    g.get(x / c, y / c)
                }
            })
        }
        Fx::PolarInversion => {
            let k = a.f(0);
            let (cx, cy) = bx.point(a.pt(1));
            let (hw, hh) = (bx.w() * 0.5, bx.h() * 0.5);
            distort(b, a.i(3), Edge::from(a.c(2)), |x, y| {
                let (u, v) = ((x - cx) / hw.max(1.0), (y - cy) / hh.max(1.0));
                let r2 = (u * u + v * v).max(1e-4);
                (cx + k * u / r2 * hw, cy + k * v / r2 * hh)
            })
        }
        Fx::Spherize => {
            let amt = a.f(0) / 100.0;
            let (cx, cy) = bx.point(a.pt(2));
            let r = bx.w().min(bx.h()) * 0.5 * a.f(1);
            distort(b, a.i(4), Edge::from(a.c(3)), |x, y| {
                let (vx, vy) = (x - cx, y - cy);
                let d = (vx * vx + vy * vy).sqrt();
                let t = d / r.max(1.0);
                if t >= 1.0 || d < 1e-4 {
                    return (x, y);
                }
                let t2 = if amt >= 0.0 { t + (1.0 - (1.0 - t * t).sqrt() - t) * amt } else { t + ((t * PI * 0.5).sin() - t) * -amt };
                let f = t2 / t;
                (cx + vx * f, cy + vy * f)
            })
        }
        Fx::TileReflection => {
            let ang = a.f(0).to_radians();
            let size = a.f(1).max(2.0);
            let curv = a.f(2) / 20.0;
            let (cx, cy) = bx.point(a.pt(3));
            distort(b, a.i(5), Edge::from(a.c(4)), |x, y| {
                let (u, v) = rot(x - cx, y - cy, -ang);
                let fu = (u / size).rem_euclid(1.0) - 0.5;
                let fv = (v / size).rem_euclid(1.0) - 0.5;
                let (u2, v2) = (u + fu * size * curv, v + fv * size * curv);
                let (rx, ry) = rot(u2, v2, ang);
                (cx + rx, cy + ry)
            })
        }
        Fx::Twist => {
            let turns = a.f(0) / 100.0;
            let (cx, cy) = bx.point(a.pt(2));
            let r = a.f(1) * (bx.w().max(bx.h()) * 0.5);
            distort(b, a.i(3), Edge::Clamp, |x, y| {
                let (vx, vy) = (x - cx, y - cy);
                let d = (vx * vx + vy * vy).sqrt();
                if d >= r {
                    return (x, y);
                }
                let th = turns * TAU * (1.0 - d / r).powi(2);
                let (rx, ry) = rot(vx, vy, th);
                (cx + rx, cy + ry)
            })
        }
        Fx::Waves => waves(a, b, bx),
        _ => b.clone(),
    }
}

/// Separable min (erode) / max (dilate) filter.
pub fn morph_pub(b: &Buf, rw: usize, rh: usize, dilate: bool) -> Buf {
    let pass = |r: usize| {
        move |row: &[Px], out: &mut [Px]| {
            let n = row.len();
            for x in 0..n {
                let (x0, x1) = (x.saturating_sub(r), (x + r).min(n - 1));
                let mut m = row[x];
                for p in &row[x0..=x1] {
                    for k in 0..4 {
                        m[k] = if dilate { m[k].max(p[k]) } else { m[k].min(p[k]) };
                    }
                }
                out[x] = m;
            }
        }
    };
    let h = par_rows(b, pass(rw));
    let t = par_rows(&transpose(&h), pass(rh));
    let mut o = transpose(&t);
    for p in o.d.iter_mut() {
        for k in 0..3 {
            p[k] = p[k].min(p[3]);
        }
    }
    o
}

fn wave(form: usize, ph: f32) -> f32 {
    let t = (ph / TAU).rem_euclid(1.0);
    match form {
        1 => 1.0 - 4.0 * (t - 0.5).abs(),
        2 => ((ph.sin()) * 6.0).clamp(-1.0, 1.0),
        3 => 2.0 * t - 1.0,
        _ => ph.sin(),
    }
}

fn waves(a: &A, b: &Buf, bx: Bx) -> Buf {
    let pattern = a.c(0);
    let form = a.c(1);
    let wl = a.f(2).max(2.0);
    let amp = a.f(3);
    let lobes = a.f(4).max(1.0);
    let damp = a.f(5) / 100.0;
    let ang = a.f(6).to_radians();
    let phase = a.f(7).to_radians();
    let (cx, cy) = bx.point(a.pt(8));
    let maxd = (bx.w().powi(2) + bx.h().powi(2)).sqrt() * 0.5;
    distort(b, a.i(10), Edge::from(a.c(9)), |x, y| {
        let (vx, vy) = (x - cx, y - cy);
        let d = (vx * vx + vy * vy).sqrt().max(1e-4);
        let (rx, ry) = (vx / d, vy / d);
        let (s, dir): (f32, (f32, f32)) = match pattern {
            1 => (d, (rx, ry)),
            2 => (d, (-ry, rx)),
            3 => {
                let th = vy.atan2(vx) - ang;
                (th / TAU * lobes * wl, (rx, ry))
            }
            4 => {
                let th = vy.atan2(vx) - ang;
                (d + th / TAU * lobes * wl, (rx, ry))
            }
            5 => {
                let (u, v) = rot(vx, vy, -ang);
                let dir = if u.abs() > v.abs() { rot(u.signum(), 0.0, ang) } else { rot(0.0, v.signum(), ang) };
                (u.abs().max(v.abs()), dir)
            }
            6 => {
                let (u, v) = rot(vx, vy, -ang);
                let r = std::f32::consts::FRAC_1_SQRT_2;
                (u.abs() + v.abs(), rot(u.signum() * r, v.signum() * r, ang))
            }
            7 => {
                let (l, r, t, bm) = (x - bx.x0, bx.x1 - x, y - bx.y0, bx.y1 - y);
                let m = l.min(r).min(t).min(bm);
                let dir = if m == l {
                    (1.0, 0.0)
                } else if m == r {
                    (-1.0, 0.0)
                } else if m == t {
                    (0.0, 1.0)
                } else {
                    (0.0, -1.0)
                };
                (m, dir)
            }
            _ => {
                let n = (ang.cos(), -ang.sin());
                (vx * n.0 + vy * n.1, (-n.1, n.0))
            }
        };
        let dd = if pattern == 0 || pattern == 3 { 1.0 } else { (-damp * 5.0 * d / maxd.max(1.0)).exp() };
        let off = amp * wave(form, s / wl * TAU + phase) * dd;
        (x + dir.0 * off, y + dir.1 * off)
    })
}
