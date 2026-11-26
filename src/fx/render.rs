//! Render: Clouds, Julia Fractal, Mandelbrot Fractal, Turbulence.

use super::*;

pub fn apply(kind: Fx, a: &A, b: &Buf, bx: Bx) -> Buf {
    match kind {
        Fx::Clouds => {
            let s = a.f(0).max(2.0);
            let rough = a.f(1);
            let mode = a.c(2);
            let seed = a.seed(3);
            let (c1, c2) = (col_px(a.col(4)), col_px(a.col(5)));
            let oct = (s.log2().ceil() as i32).clamp(1, 12);
            par_map(b.w, b.h, |x, y| {
                let v = fbm(x as f32 / s, y as f32 / s, oct, rough, seed, false);
                let t = (v * 1.3 + 0.5).clamp(0.0, 1.0);
                pm(blend(mode, st(b.get(x, y)), lerp4(c1, c2, t)))
            })
        }
        Fx::JuliaFractal | Fx::MandelbrotFractal => {
            let julia = kind == Fx::JuliaFractal;
            let factor = a.f(0);
            let zoom = a.f(1).max(0.01);
            let ang = a.f(2).to_radians();
            let q = a.i(3);
            let invert = !julia && a.b(4);
            let mode = if julia { a.c(4) } else { a.c(5) };
            let offs = ss(q);
            let (cx, cy) = (bx.cx(), bx.cy());
            let scale = if julia { 3.0 } else { 3.2 } / (zoom * bx.w().min(bx.h()).max(1.0));
            let (sa, ca) = ang.sin_cos();
            par_map(b.w, b.h, |x, y| {
                let mut acc = [0.0f32; 3];
                for &(ox, oy) in &offs {
                    let (u, v) = ((x as f32 + ox - cx) * scale, (y as f32 + oy - cy) * scale);
                    let (u, v) = (u * ca - v * sa, u * sa + v * ca);
                    let c = fractal(julia, u as f64, v as f64, factor);
                    for k in 0..3 {
                        acc[k] += c[k];
                    }
                }
                let n = offs.len() as f32;
                let mut c = [acc[0] / n, acc[1] / n, acc[2] / n, 1.0];
                if invert {
                    for v in c.iter_mut().take(3) {
                        *v = 1.0 - *v;
                    }
                }
                pm(blend(mode, st(b.get(x, y)), c))
            })
        }
        Fx::Turbulence => {
            let oct = a.i(0).clamp(1, 10);
            let period = a.f(1).max(2.0);
            let size = a.f(2);
            let sum = a.c(3) == 1;
            let mode = a.c(4);
            let seed = a.seed(5);
            par_map(b.w, b.h, |x, y| {
                let (u, v) = (x as f32 / period, y as f32 / period);
                let ch = |s: u32| {
                    let n = fbm(u, v, oct, size, seed.wrapping_add(s), !sum);
                    if sum {
                        (n * 1.4 + 0.55).clamp(0.0, 1.0)
                    } else {
                        (n * 2.2).clamp(0.0, 1.0)
                    }
                };
                pm(blend(mode, st(b.get(x, y)), [ch(0), ch(101), ch(202), 1.0]))
            })
        }
        _ => b.clone(),
    }
}

/// Escape-time fractal colour (smooth iteration count mapped to an original palette).
fn fractal(julia: bool, u: f64, v: f64, factor: f32) -> [f32; 3] {
    let max_it = 256;
    let (mut zr, mut zi, cr, ci) = if julia { (u, v, -0.745, 0.113) } else { (0.0, 0.0, u - 0.6, v) };
    let mut i = 0;
    while i < max_it && zr * zr + zi * zi < 256.0 {
        let t = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = t;
        i += 1;
    }
    if i >= max_it {
        return [0.0, 0.0, 0.0];
    }
    let mu = i as f32 + 1.0 - ((zr * zr + zi * zi).sqrt().ln().ln() / std::f64::consts::LN_2) as f32;
    let t = (mu / 24.0).max(0.0);
    let sat = (factor / 10.0).clamp(0.0, 1.0);
    let gray = (t * 0.9).fract();
    let col = [
        0.5 + 0.5 * (t * 3.1 + 0.0).cos(),
        0.5 + 0.5 * (t * 3.1 + 2.1).cos(),
        0.5 + 0.5 * (t * 3.1 + 4.2).cos(),
    ];
    let bright = (1.0 - (-mu / 6.0).exp()).clamp(0.0, 1.0);
    [
        (gray + (col[0] - gray) * sat) * bright,
        (gray + (col[1] - gray) * sat) * bright,
        (gray + (col[2] - gray) * sat) * bright,
    ]
}
