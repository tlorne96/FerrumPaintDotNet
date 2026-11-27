//! Clean-room effects modelled on the effect names and one-line descriptions published for a
//! second third-party Paint.NET plugin pack (see docs/REFERENCE_NOTES.md). That pack is freeware
//! with no published source or open licence, so nothing from it was decompiled, read or copied:
//! every algorithm here is our own.
//!
//! Black and White+, Color Balance, Transparency, Dream, Pastel, Complementary Colors,
//! Floyd-Steinberg Dithering, Switch Alpha to Gray, Switch Gray to Alpha, Feather Object,
//! Bevel Object, Inner Shadow, Grid / Checkerboard.

use super::*;

pub const BW_FILTERS: &[&str] = &["Custom", "Red", "Orange", "Yellow", "Green", "Blue", "Infrared"];
pub const BW_TONES: &[&str] = &["None", "Sepia", "Selenium", "Cyanotype", "Gold"];
pub const DITHER_PALETTES: &[&str] =
    &["Black and white", "Color 1 and Color 2", "8 colors", "Web safe (216)", "4 levels per channel", "16 grays", "4 grays"];
pub const G2A_COLORS: &[&str] = &["Keep image colors", "Black", "White", "Color"];
pub const COMPLEMENT: &[&str] = &["Opposite hue, same lightness", "Hue rotation (HSV)"];
pub const GRID_TYPES: &[&str] = &["Grid", "Checkerboard"];
pub const OPACITY_MODES: &[&str] = &["Multiply opacity", "Set opacity"];

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx) -> Buf {
    match kind {
        Fx::BlackWhitePlus => black_white(a, b),
        Fx::ColorBalance => color_balance(a, b),
        Fx::Transparency => transparency(a, b),
        Fx::Dream => dream(a, b),
        Fx::Pastel => pastel(a, b),
        Fx::ComplementaryColors => complementary(a, b),
        Fx::Dithering => dither(a, b, bx),
        Fx::AlphaToGray => alpha_to_gray(a, b),
        Fx::GrayToAlpha => gray_to_alpha(a, b),
        Fx::FeatherObject => feather(a, b),
        Fx::BevelObject => bevel(a, b),
        Fx::InnerShadow => inner_shadow(a, b),
        Fx::GridCheckerboard => grid(a, b, bx),
        _ => b.clone(),
    }
}

#[inline]
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Contrast factor for -100..100: negative flattens towards grey, positive steepens.
fn contrast_k(c: f32) -> f32 {
    if c >= 0.0 {
        1.0 / (1.0 - c / 100.0 * 0.95)
    } else {
        1.0 + c / 100.0
    }
}

// ----------------------------------------------------------------------------------------------
// Adjustments

/// Monochrome conversion with a channel mixer, colour-filter presets, brightness / contrast and
/// an optional split-free toning colour applied with an overlay.
fn black_white(a: &A, b: &Buf) -> Buf {
    let w = match a.c(0) {
        1 => [80.0, 20.0, 0.0],
        2 => [60.0, 35.0, 5.0],
        3 => [45.0, 50.0, 5.0],
        4 => [20.0, 70.0, 10.0],
        5 => [10.0, 20.0, 70.0],
        6 => [-40.0, 200.0, -60.0],
        _ => [a.f(1), a.f(2), a.f(3)],
    };
    let bright = a.f(4) / 100.0;
    let k = contrast_k(a.f(5));
    let tone: Option<[f32; 3]> = match a.c(6) {
        1 => Some([0.70, 0.52, 0.33]),
        2 => Some([0.56, 0.46, 0.58]),
        3 => Some([0.25, 0.45, 0.70]),
        4 => Some([0.76, 0.61, 0.26]),
        _ => None,
    };
    let ts = a.f(7) / 100.0;
    map_st(b, |p| {
        let mut g = (p[0] * w[0] + p[1] * w[1] + p[2] * w[2]) / 100.0;
        g = ((g - 0.5) * k + 0.5 + bright).clamp(0.0, 1.0);
        let mut o = [g, g, g, p[3]];
        if let Some(t) = tone {
            for c in 0..3 {
                let ov = if g < 0.5 { 2.0 * g * t[c] } else { 1.0 - 2.0 * (1.0 - g) * (1.0 - t[c]) };
                o[c] = g + (ov - g) * ts;
            }
        }
        o
    })
}

/// Shift cyan-red, magenta-green and yellow-blue separately in the shadows, midtones and
/// highlights, optionally keeping each pixel's original luminosity.
fn color_balance(a: &A, b: &Buf) -> Buf {
    let s = [a.f(0), a.f(1), a.f(2)];
    let m = [a.f(3), a.f(4), a.f(5)];
    let h = [a.f(6), a.f(7), a.f(8)];
    let keep = a.b(9);
    map_st(b, |p| {
        let l = luma(p);
        let ws = smooth(1.0 - l / 0.6);
        let wh = smooth((l - 0.4) / 0.6);
        let wm = smooth(1.0 - (l - 0.5).abs() / 0.5);
        let mut o = p;
        for c in 0..3 {
            o[c] = (p[c] + (s[c] * ws + m[c] * wm + h[c] * wh) / 100.0 * 0.4).clamp(0.0, 1.0);
        }
        if keep {
            let d = l - luma(o);
            for c in o.iter_mut().take(3) {
                *c = (*c + d).clamp(0.0, 1.0);
            }
        }
        o
    })
}

/// Scale or set the alpha channel.
fn transparency(a: &A, b: &Buf) -> Buf {
    let set = a.c(0) == 1;
    let v = a.f(1) / 100.0;
    map_st(b, |p| [p[0], p[1], p[2], if set { if p[3] > 0.0 { v } else { 0.0 } } else { p[3] * v }])
}

// ----------------------------------------------------------------------------------------------
// Artistic

/// Soft-focus "dream" look: the image is screened with a blurred copy of itself, then
/// saturation and brightness are adjusted.
fn dream(a: &A, b: &Buf) -> Buf {
    let blur = gauss(b, a.f(0).max(1.0), 3);
    let amt = a.f(1) / 100.0;
    let sat = a.f(2) / 100.0;
    let bright = a.f(3) / 100.0;
    par_map(b.w, b.h, |x, y| {
        let p = st(b.get(x, y));
        let q = st(blur.get(x, y));
        let mut o = p;
        for c in 0..3 {
            let scr = 1.0 - (1.0 - p[c]) * (1.0 - q[c]);
            o[c] = p[c] + (scr - p[c]) * amt + bright;
        }
        let l = luma(o);
        for c in o.iter_mut().take(3) {
            *c = l + (*c - l) * sat;
        }
        pm(o)
    })
}

/// Pastel: lighten, desaturate gently, soften and quantise into chalky bands.
fn pastel(a: &A, b: &Buf) -> Buf {
    let soft = gauss(b, a.f(0).max(0.0), 3);
    let light = a.f(1) / 100.0;
    let levels = a.i(2).max(2) as f32 - 1.0;
    let sat = a.f(3) / 100.0;
    par_map(b.w, b.h, |x, y| {
        let p = st(soft.get(x, y));
        let mut o = p;
        for c in 0..3 {
            let v = p[c] + (1.0 - p[c]) * light;
            o[c] = (v * levels).round() / levels;
        }
        let l = luma(o);
        for c in o.iter_mut().take(3) {
            *c = l + (*c - l) * sat;
        }
        pm(o)
    })
}

// ----------------------------------------------------------------------------------------------
// Color

fn complementary(a: &A, b: &Buf) -> Buf {
    let hsv = a.c(0) == 1;
    let amt = a.f(1) / 100.0;
    map_st(b, |p| {
        let c = if hsv {
            let (h, s, v) = rgb_to_hsv([p[0], p[1], p[2]]);
            hsv_to_rgb(h + 180.0, s, v)
        } else {
            let mx = p[0].max(p[1]).max(p[2]);
            let mn = p[0].min(p[1]).min(p[2]);
            [mx + mn - p[0], mx + mn - p[1], mx + mn - p[2]]
        };
        [p[0] + (c[0] - p[0]) * amt, p[1] + (c[1] - p[1]) * amt, p[2] + (c[2] - p[2]) * amt, p[3]]
    })
}

// ----------------------------------------------------------------------------------------------
// Stylize

/// Floyd-Steinberg error diffusion to a fixed palette, within the selection bounds.
fn dither(a: &A, b: &Buf, bx: Bx) -> Buf {
    let pal = a.c(0);
    let serp = a.b(1);
    let amt = a.f(2) / 100.0;
    let c1 = col_px(a.col(3));
    let c2 = col_px(a.col(4));
    let gray = matches!(pal, 0 | 5 | 6);
    let levels = match pal {
        0 | 2 => 2.0,
        3 => 6.0,
        4 | 6 => 4.0,
        5 => 16.0,
        _ => 0.0,
    };
    let nearest = |c: [f32; 3]| -> [f32; 3] {
        if pal == 1 {
            let d = |q: Px| (0..3).map(|k| (c[k] - q[k]) * (c[k] - q[k])).sum::<f32>();
            let q = if d(c1) <= d(c2) { c1 } else { c2 };
            [q[0], q[1], q[2]]
        } else {
            let n = levels - 1.0;
            [(c[0].clamp(0.0, 1.0) * n).round() / n, (c[1].clamp(0.0, 1.0) * n).round() / n, (c[2].clamp(0.0, 1.0) * n).round() / n]
        }
    };
    let (x0, y0) = (bx.x0.max(0.0) as usize, bx.y0.max(0.0) as usize);
    let (x1, y1) = ((bx.x1 as usize).min(b.w), (bx.y1 as usize).min(b.h));
    let mut out = b.clone();
    if x1 <= x0 || y1 <= y0 {
        return out;
    }
    let rw = x1 - x0;
    let mut work: Vec<[f32; 3]> = Vec::with_capacity(rw * (y1 - y0));
    for y in y0..y1 {
        for x in x0..x1 {
            let p = st(b.get(x, y));
            work.push(if gray { [luma(p); 3] } else { [p[0], p[1], p[2]] });
        }
    }
    let rh = y1 - y0;
    for j in 0..rh {
        let rev = serp && j % 2 == 1;
        for ii in 0..rw {
            let i = if rev { rw - 1 - ii } else { ii };
            let old = work[j * rw + i];
            let new = nearest(old);
            work[j * rw + i] = new;
            let err = [(old[0] - new[0]) * amt, (old[1] - new[1]) * amt, (old[2] - new[2]) * amt];
            let fwd: i64 = if rev { -1 } else { 1 };
            let mut push = |dx: i64, dy: usize, wgt: f32| {
                let xi = i as i64 + dx;
                let yj = j + dy;
                if xi >= 0 && (xi as usize) < rw && yj < rh {
                    let t = &mut work[yj * rw + xi as usize];
                    for k in 0..3 {
                        t[k] += err[k] * wgt;
                    }
                }
            };
            push(fwd, 0, 7.0 / 16.0);
            push(-fwd, 1, 3.0 / 16.0);
            push(0, 1, 5.0 / 16.0);
            push(fwd, 1, 1.0 / 16.0);
        }
    }
    for j in 0..rh {
        for i in 0..rw {
            let (x, y) = (x0 + i, y0 + j);
            let a0 = st(b.get(x, y))[3];
            let c = work[j * rw + i];
            out.d[y * b.w + x] = pm([c[0], c[1], c[2], a0]);
        }
    }
    out
}

// ----------------------------------------------------------------------------------------------
// Object

fn alpha_to_gray(a: &A, b: &Buf) -> Buf {
    let inv = a.b(0);
    map_st(b, |p| {
        let g = if inv { 1.0 - p[3] } else { p[3] };
        [g, g, g, 1.0]
    })
}

fn gray_to_alpha(a: &A, b: &Buf) -> Buf {
    let inv = a.b(0);
    let mode = a.c(1);
    let col = col_px(a.col(2));
    map_st(b, |p| {
        let l = luma(p);
        let al = (if inv { 1.0 - l } else { l }) * p[3];
        let c = match mode {
            1 => [0.0, 0.0, 0.0],
            2 => [1.0, 1.0, 1.0],
            3 => [col[0], col[1], col[2]],
            _ => [p[0], p[1], p[2]],
        };
        [c[0], c[1], c[2], al]
    })
}

/// Distance (in pixels) from every pixel to the nearest transparent pixel (alpha < 0.5).
/// The canvas border does not count as transparent.
fn inside_distance(b: &Buf, dx: i64, dy: i64) -> Vec<f32> {
    let outside: Vec<bool> = (0..b.w * b.h)
        .map(|i| {
            let (x, y) = ((i % b.w) as i64 - dx, (i / b.w) as i64 - dy);
            if x < 0 || y < 0 || x >= b.w as i64 || y >= b.h as i64 {
                false
            } else {
                b.d[y as usize * b.w + x as usize][3] < 0.5
            }
        })
        .collect();
    if !outside.iter().any(|&o| o) {
        return vec![f32::INFINITY; b.w * b.h];
    }
    extras::distance_field(&outside, b.w, b.h)
}

/// Fade the object's alpha towards its edges.
fn feather(a: &A, b: &Buf) -> Buf {
    let r = a.f(0).max(1.0);
    let d = inside_distance(b, 0, 0);
    par_map(b.w, b.h, |x, y| {
        let i = y * b.w + x;
        scale4(b.d[i], smooth(d[i] / r))
    })
}

/// Raised-edge shading: a height ramp built from the distance to the object's edge is lit from
/// the chosen angle.
fn bevel(a: &A, b: &Buf) -> Buf {
    let depth = a.f(0).max(1.0);
    let (lx, ly) = {
        let r = a.f(1).to_radians();
        (r.cos(), -r.sin())
    };
    let strength = a.f(2) / 100.0;
    let d = inside_distance(b, 0, 0);
    // Smooth the ramp so the integer-ish distance field doesn't show up as terraces.
    let raw: Vec<f32> = d.iter().map(|&v| smooth(v / depth)).collect();
    let hp = gauss_plane(&raw, b.w, b.h, (depth * 0.25).clamp(1.5, 6.0));
    let ht = |x: i64, y: i64| -> f32 {
        let x = x.clamp(0, b.w as i64 - 1) as usize;
        let y = y.clamp(0, b.h as i64 - 1) as usize;
        hp[y * b.w + x]
    };
    let (w, h) = (b.w, b.h);
    let (lz, ln) = (0.7f32, (lx * lx + ly * ly + 0.49f32).sqrt());
    par_map(w, h, |x, y| {
        let p = b.get(x, y);
        if p[3] <= 0.0 {
            return p;
        }
        let (xi, yi) = (x as i64, y as i64);
        let gx = (ht(xi + 1, yi) - ht(xi - 1, yi)) * 0.5 * depth;
        let gy = (ht(xi, yi + 1) - ht(xi, yi - 1)) * 0.5 * depth;
        let n = (gx * gx + gy * gy + 1.0).sqrt();
        let shade = ((-gx * lx - gy * ly + lz) / n - lz) / ln;
        let s = st(p);
        let t = (shade * strength * 1.5).clamp(-1.0, 1.0);
        let o = if t >= 0.0 {
            [s[0] + (1.0 - s[0]) * t, s[1] + (1.0 - s[1]) * t, s[2] + (1.0 - s[2]) * t, s[3]]
        } else {
            [s[0] * (1.0 + t), s[1] * (1.0 + t), s[2] * (1.0 + t), s[3]]
        };
        pm(o)
    })
}

/// Shadow cast inwards from the object's edges, offset away from the light.
fn inner_shadow(a: &A, b: &Buf) -> Buf {
    let width = a.f(0).max(1.0);
    let r = a.f(1).to_radians();
    let dist = a.f(2);
    let op = a.f(3) / 100.0;
    let col = col_px(a.col(4));
    let (dx, dy) = ((-r.cos() * dist).round() as i64, (r.sin() * dist).round() as i64);
    let d = inside_distance(b, dx, dy);
    par_map(b.w, b.h, |x, y| {
        let i = y * b.w + x;
        let p = b.d[i];
        if p[3] <= 0.0 {
            return p;
        }
        let s = st(p);
        let k = (1.0 - smooth(d[i] / width)) * op * col[3];
        pm([s[0] + (col[0] - s[0]) * k, s[1] + (col[1] - s[1]) * k, s[2] + (col[2] - s[2]) * k, s[3]])
    })
}

// ----------------------------------------------------------------------------------------------
// Render

fn grid(a: &A, b: &Buf, bx: Bx) -> Buf {
    let checker = a.c(0) == 1;
    let cw = a.i(1).max(1) as i64;
    let ch = a.i(2).max(1) as i64;
    let lw = a.i(3).max(1) as i64;
    let c1 = pm(col_px(a.col(4)));
    let c2 = pm(col_px(a.col(5)));
    let keep = a.b(6);
    let (ox, oy) = (bx.x0 as i64, bx.y0 as i64);
    par_map(b.w, b.h, |x, y| {
        let (u, v) = (x as i64 - ox, y as i64 - oy);
        let first = if checker {
            (u.div_euclid(cw) + v.div_euclid(ch)) % 2 == 0
        } else {
            u.rem_euclid(cw) < lw || v.rem_euclid(ch) < lw
        };
        if first {
            c1
        } else if keep {
            b.get(x, y)
        } else {
            c2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_square(n: usize, s: usize) -> Buf {
        let mut b = Buf::new(n, n);
        for y in s..n - s {
            for x in s..n - s {
                b.d[y * n + x] = [0.5, 0.5, 0.5, 1.0];
            }
        }
        b
    }

    #[test]
    fn dither_bw_only_uses_black_and_white() {
        let mut b = Buf::new(16, 16);
        for (i, p) in b.d.iter_mut().enumerate() {
            let v = (i % 16) as f32 / 15.0;
            *p = [v, v, v, 1.0];
        }
        let st = FxState::new(Fx::Dithering, Col::BLACK, Col::WHITE);
        let out = dither(&A(&st.vals), &b, Bx { x0: 0.0, y0: 0.0, x1: 16.0, y1: 16.0 });
        assert!(out.d.iter().all(|p| p[0] == 0.0 || p[0] == 1.0));
        let mean: f32 = out.d.iter().map(|p| p[0]).sum::<f32>() / 256.0;
        assert!((mean - 0.5).abs() < 0.1, "mean {mean}");
    }

    #[test]
    fn feather_softens_edges_only() {
        let b = opaque_square(40, 5);
        let st = FxState::new(Fx::FeatherObject, Col::BLACK, Col::WHITE);
        let out = feather(&A(&st.vals), &b);
        assert!(out.get(5, 20)[3] < 0.5);
        assert!((out.get(20, 20)[3] - 1.0).abs() < 1e-6);
        assert_eq!(out.get(1, 1)[3], 0.0);
    }

    #[test]
    fn gray_alpha_round_trip() {
        let mut b = Buf::new(4, 1);
        for (i, p) in b.d.iter_mut().enumerate() {
            *p = scale4([1.0, 1.0, 1.0, 1.0], i as f32 / 3.0);
        }
        let st = FxState::new(Fx::AlphaToGray, Col::BLACK, Col::WHITE);
        let g = alpha_to_gray(&A(&st.vals), &b);
        assert!((g.get(2, 0)[0] - 2.0 / 3.0).abs() < 1e-5 && g.get(2, 0)[3] == 1.0);
        let st2 = FxState::new(Fx::GrayToAlpha, Col::BLACK, Col::WHITE);
        let back = gray_to_alpha(&A(&st2.vals), &g);
        assert!((back.get(2, 0)[3] - 2.0 / 3.0).abs() < 1e-3);
    }
}
