//! Curves+ : the per-channel pixel maths is a Rust port of pyrochild's (Zach Walker's) MIT-licensed
//! `UnaryPixelHistogramOps.cs` from https://github.com/bsneeze/pdn-curvesplus (Luminosity, RGB,
//! HSV, CMYK, Alpha and Advanced input->output curves).
//!
//! Copyright (c) 2007-2016 Zach Walker. MIT License; see THIRD_PARTY_NOTICES.md.
//!
//! The plugin's dialog, curve control and spline interpolator carry Paint.NET copyright headers,
//! so none of those were used: the curve shape comes from Ferrum's own `raster::curve_lut` and
//! the editor is Ferrum's own.

use crate::raster::curve_lut;

pub const MODES: &[&str] = &["Luminosity", "RGB", "HSV", "CMYK", "Alpha", "Advanced"];
/// Channels in the plugin's enum order.
pub const CHANNELS: &[&str] = &["Alpha", "Red", "Green", "Blue", "Cyan", "Magenta", "Yellow", "Black (K)", "Hue", "Saturation", "Value", "Luminosity"];

#[derive(Clone, Debug, PartialEq)]
pub struct CurvesPlus {
    pub mode: usize,
    /// One control-point list per channel of the current mode (up to 4).
    pub curves: [Vec<(u8, u8)>; 4],
    /// Which channels the editor changes together.
    pub edit: [bool; 4],
    pub adv_in: usize,
    pub adv_out: usize,
}

impl Default for CurvesPlus {
    fn default() -> Self {
        let id = vec![(0, 0), (255, 255)];
        CurvesPlus { mode: 0, curves: [id.clone(), id.clone(), id.clone(), id], edit: [true; 4], adv_in: 11, adv_out: 11 }
    }
}

impl CurvesPlus {
    /// Channel names for the current mode's curves.
    pub fn channel_names(&self) -> &'static [&'static str] {
        match self.mode {
            1 => &["Red", "Green", "Blue"],
            2 => &["Hue", "Saturation", "Value"],
            3 => &["Cyan", "Magenta", "Yellow", "Black"],
            4 => &["Alpha"],
            5 => &["Curve"],
            _ => &["Luminosity"],
        }
    }
    pub fn reset_curves(&mut self) {
        let id = vec![(0, 0), (255, 255)];
        self.curves = [id.clone(), id.clone(), id.clone(), id];
        self.edit = [true; 4];
    }
    pub fn op(&self) -> Op {
        let l = [curve_lut(&self.curves[0]), curve_lut(&self.curves[1]), curve_lut(&self.curves[2]), curve_lut(&self.curves[3])];
        Op { mode: self.mode, l, adv_in: self.adv_in, adv_out: self.adv_out }
    }
}

pub struct Op {
    mode: usize,
    l: [[u8; 256]; 4],
    adv_in: usize,
    adv_out: usize,
}

/// The plugin's IntDiv (integer division by a byte, flooring); division by zero gives 0.
#[inline]
fn idiv(x: i32, z: i32) -> i32 {
    if z == 0 {
        0
    } else {
        x.div_euclid(z)
    }
}
#[inline]
fn intensity(r: i32, g: i32, b: i32) -> i32 {
    (7471 * b + 38470 * g + 19595 * r) >> 16
}
#[inline]
fn cb(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}
/// Hue on the plugin's 0..1535 scale (before dividing by 6), given max/delta.
fn hue1536(r: i32, g: i32, b: i32, max: i32, delta: i32) -> i32 {
    let mut h = if r == max {
        idiv(255 * (g - b), delta)
    } else if g == max {
        512 + idiv(255 * (b - r), delta)
    } else {
        1024 + idiv(255 * (r - g), delta)
    };
    if h < 0 {
        h += 1536;
    }
    h
}
/// Sector-based HSV -> RGB used throughout the plugin. `h6` is hue * 6 (0..1535).
fn sector(h6: i32, s: i32, v: i32) -> [i32; 3] {
    let f = h6 & 0xff;
    let lo = ((v * (255 - s)) + 128) >> 8;
    match h6 >> 8 {
        0 => [v, ((v * (255 - (((s * (255 - f)) + 128) >> 8))) + 128) >> 8, lo],
        1 => [((v * (255 - (((s * f) + 128) >> 8))) + 128) >> 8, v, lo],
        2 => [lo, v, ((v * (255 - (((s * (255 - f)) + 128) >> 8))) + 128) >> 8],
        3 => [lo, ((v * (255 - (((s * f) + 128) >> 8))) + 128) >> 8, v],
        4 => [((v * (255 - (((s * (255 - f)) + 128) >> 8))) + 128) >> 8, lo, v],
        5 => [v, lo, ((v * (255 - (((s * f) + 128) >> 8))) + 128) >> 8],
        _ => [0, 0, 0],
    }
}

impl Op {
    /// Apply to one straight-alpha RGBA8 pixel.
    pub fn apply(&self, p: [u8; 4]) -> [u8; 4] {
        let (r, g, b, a) = (p[0] as i32, p[1] as i32, p[2] as i32, p[3] as i32);
        let l = &self.l;
        match self.mode {
            0 => {
                let lumi = intensity(r, g, b);
                let d = l[0][lumi as usize] as i32 - lumi;
                [cb(r + d), cb(g + d), cb(b + d), p[3]]
            }
            1 => [l[0][p[0] as usize], l[1][p[1] as usize], l[2][p[2] as usize], p[3]],
            2 => self.hsv(r, g, b, p[3]),
            3 => {
                let (c, m, y) = (255 - r, 255 - g, 255 - b);
                let k = c.min(m).min(y);
                let c2 = l[0][(c - k) as usize] as i32;
                let m2 = l[1][(m - k) as usize] as i32;
                let y2 = l[2][(y - k) as usize] as i32;
                let k2 = l[3][k as usize] as i32;
                [(255 - cb(c2 + k2) as i32) as u8, (255 - cb(m2 + k2) as i32) as u8, (255 - cb(y2 + k2) as i32) as u8, p[3]]
            }
            4 => [p[0], p[1], p[2], l[0][p[3] as usize]],
            _ => self.advanced(r, g, b, a),
        }
    }

    fn hsv(&self, r: i32, g: i32, b: i32, a: u8) -> [u8; 4] {
        let (ch, cs, cv) = (&self.l[0], &self.l[1], &self.l[2]);
        let max = r.max(g).max(b);
        let v = max;
        let nv = cv[v as usize] as i32;
        if nv == 0 {
            return [0, 0, 0, a];
        }
        let min = r.min(g).min(b);
        let delta = max - min;
        let s = if delta > 0 { idiv(255 * delta, max) } else { 0 };
        let ns = cs[s as usize] as i32;
        if ns == 0 {
            return [nv as u8, nv as u8, nv as u8, a];
        }
        let h = idiv(hue1536(r, g, b, max, delta), 6).clamp(0, 255);
        let nh = ch[h as usize] as i32;
        if nh == h && ns == s {
            return [cb(idiv(r * nv, v)), cb(idiv(g * nv, v)), cb(idiv(b * nv, v)), a];
        }
        let o = sector(nh * 6, ns, nv);
        [cb(o[0]), cb(o[1]), cb(o[2]), a]
    }

    fn advanced(&self, r: i32, g: i32, b: i32, a: i32) -> [u8; 4] {
        let c = &self.l[0];
        let kk = (255 - r).min(255 - g).min(255 - b);
        let v = r.max(g).max(b);
        let delta = v - r.min(g).min(b);
        let s = if v == 0 || delta <= 0 { 0 } else { idiv(255 * delta, v) };
        let raw = match self.adv_in {
            0 => a,
            1 => r,
            2 => g,
            3 => b,
            4 => 255 - r - kk,
            5 => 255 - g - kk,
            6 => 255 - b - kk,
            7 => kk,
            8 => {
                if s == 0 {
                    0
                } else {
                    idiv(hue1536(r, g, b, v, delta), 6).clamp(0, 255)
                }
            }
            9 => s,
            10 => v,
            _ => intensity(r, g, b),
        };
        let i = c[raw.clamp(0, 255) as usize] as i32;
        let pa = a as u8;
        match self.adv_out {
            0 => [r as u8, g as u8, b as u8, i as u8],
            1 => [i as u8, g as u8, b as u8, pa],
            2 => [r as u8, i as u8, b as u8, pa],
            3 => [r as u8, g as u8, i as u8, pa],
            4 => [(255 - (i + kk).clamp(0, 255)) as u8, g as u8, b as u8, pa],
            5 => [r as u8, (255 - (i + kk).clamp(0, 255)) as u8, b as u8, pa],
            6 => [r as u8, g as u8, (255 - (i + kk).clamp(0, 255)) as u8, pa],
            7 => {
                let (cc, m, y) = (255 - r, 255 - g, 255 - b);
                [cb(255 - (cc - kk + i)), cb(255 - (m - kk + i)), cb(255 - (y - kk + i)), pa]
            }
            8 => {
                let o = sector(i * 6, s, v);
                [cb(o[0]), cb(o[1]), cb(o[2]), pa]
            }
            9 => {
                let h = if s == 0 { 0 } else { hue1536(r, g, b, v, delta) };
                let o = sector(h, i, v);
                [cb(o[0]), cb(o[1]), cb(o[2]), pa]
            }
            10 => [cb(idiv(r * i, v)), cb(idiv(g * i, v)), cb(idiv(b * i, v)), pa],
            _ => {
                let li = intensity(r, g, b);
                [cb(r + i - li), cb(g + i - li), cb(b + i - li), pa]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_curves_preserve_pixels() {
        let mut cp = CurvesPlus::default();
        let px = [[200u8, 40, 90, 255], [10, 10, 10, 128], [0, 255, 128, 255], [77, 77, 200, 255]];
        for mode in 0..6 {
            cp.mode = mode;
            for (ai, ao) in [(11, 11), (1, 1), (10, 10), (3, 3)] {
                cp.adv_in = ai;
                cp.adv_out = ao;
                let op = cp.op();
                for p in px {
                    let q = op.apply(p);
                    for k in 0..4 {
                        assert!((q[k] as i32 - p[k] as i32).abs() <= 2, "mode {mode} adv {ai}->{ao}: {p:?} -> {q:?}");
                    }
                }
            }
        }
    }
    #[test]
    fn advanced_red_to_alpha() {
        let cp = CurvesPlus { mode: 5, adv_in: 1, adv_out: 0, ..Default::default() };
        assert_eq!(cp.op().apply([180, 0, 0, 255]), [180, 0, 0, 180]);
        let mut inv = CurvesPlus { mode: 1, ..Default::default() };
        inv.curves[0] = vec![(0, 255), (255, 0)];
        assert_eq!(inv.op().apply([200, 30, 40, 255]), [55, 30, 40, 255]);
    }
}
