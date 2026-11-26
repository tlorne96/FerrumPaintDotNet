//! Adjustments with parameters: Exposure, Highlights / Shadows, Sepia, Temperature and Tint.

use super::*;

pub fn apply(kind: Fx, a: &A, b: &Buf) -> Buf {
    match kind {
        Fx::Exposure => {
            // Multiply linear light by 2^stops.
            let k = 2f32.powf(a.f(0));
            map_st(b, |p| [to_srgb(to_lin(p[0]) * k), to_srgb(to_lin(p[1]) * k), to_srgb(to_lin(p[2]) * k), p[3]])
        }
        Fx::HighlightsShadows => {
            let (sh, hi, r) = (a.f(0) / 100.0, a.f(1) / 100.0, a.f(2));
            // Local brightness from a blurred luma plane decides how much each pixel is lifted/recovered.
            let lp = gauss_plane(&luma_plane(b), b.w, b.h, r);
            par_map(b.w, b.h, |x, y| {
                let p = st(b.get(x, y));
                let l = lp[y * b.w + x].clamp(0.0, 1.0);
                let ws = (1.0 - l).powi(2);
                let wh = l.powi(2);
                let mut o = p;
                for k in 0..3 {
                    let lifted = p[k] + (1.0 - p[k]) * p[k].sqrt().min(1.0) * sh * ws * 1.2;
                    o[k] = lifted - lifted * hi * wh * 0.6;
                }
                pm(o)
            })
        }
        Fx::Sepia => {
            let t = a.f(0) / 100.0;
            map_st(b, |p| {
                let l = luma(p);
                let s = [l * 1.07 + 0.03, l * 0.92 + 0.01, l * 0.70];
                [p[0] + (s[0] - p[0]) * t, p[1] + (s[1] - p[1]) * t, p[2] + (s[2] - p[2]) * t, p[3]]
            })
        }
        Fx::TemperatureTint => {
            let (t, n) = (a.f(0) / 100.0, a.f(1) / 100.0);
            // Warm = more red / less blue; tint = green <-> magenta. Luma is preserved.
            let gains = [1.0 + 0.25 * t + 0.08 * n, 1.0 - 0.18 * n, 1.0 - 0.25 * t + 0.08 * n];
            map_st(b, |p| {
                let l0 = luma(p);
                let mut o = [p[0] * gains[0], p[1] * gains[1], p[2] * gains[2], p[3]];
                let l1 = luma(o);
                if l1 > 1e-4 {
                    let k = l0 / l1;
                    for c in o.iter_mut().take(3) {
                        *c *= k;
                    }
                }
                o
            })
        }
        _ => b.clone(),
    }
}
