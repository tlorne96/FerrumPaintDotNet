//! Color Match: recolour the active layer so its colours resemble a reference image (taken from
//! the clipboard or a file). Clean-room: the plugin of the same name has no published source, so
//! this is built from two textbook methods.
//!
//! * Histogram matching, per RGB channel (cumulative distributions are aligned).
//! * Statistics transfer in CIE Lab (Reinhard, Ashikhmin, Gooch & Shirley, 2001): the target's
//!   per-channel mean and standard deviation are moved onto the reference's.

use tiny_skia::{Mask, Pixmap};

pub const MODES: &[&str] = &["Histogram match (RGB)", "Statistics transfer (Lab)"];

#[derive(Clone, Debug, PartialEq)]
pub struct ColorMatch {
    pub mode: usize,
    /// 0..=100 blend between the original and the matched colours.
    pub strength: i32,
    /// Keep each pixel's original lightness (statistics mode only changes colour then).
    pub keep_lightness: bool,
    pub label: String,
    reference: Option<Reference>,
}

#[derive(Clone, Debug, PartialEq)]
struct Reference {
    /// Normalised cumulative distribution of each RGB channel.
    cdf: [[f32; 256]; 3],
    /// Mean and standard deviation of Lab L, a, b.
    mean: [f32; 3],
    std: [f32; 3],
}

impl Default for ColorMatch {
    fn default() -> Self {
        ColorMatch { mode: 0, strength: 100, keep_lightness: false, label: String::new(), reference: None }
    }
}

fn lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn gam(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn to_lab(p: [u8; 4]) -> [f32; 3] {
    let (r, g, b) = (lin(p[0] as f32 / 255.0), lin(p[1] as f32 / 255.0), lin(p[2] as f32 / 255.0));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn from_lab(l: [f32; 3]) -> [u8; 3] {
    let fy = (l[0] + 16.0) / 116.0;
    let fx = fy + l[1] / 500.0;
    let fz = fy - l[2] / 200.0;
    let inv = |t: f32| if t * t * t > 0.008856 { t * t * t } else { (t - 16.0 / 116.0) / 7.787 };
    let (x, y, z) = (inv(fx) * 0.95047, inv(fy), inv(fz) * 1.08883);
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    [(gam(r) * 255.0).round() as u8, (gam(g) * 255.0).round() as u8, (gam(b) * 255.0).round() as u8]
}

struct Stats {
    hist: [[u32; 256]; 3],
    mean: [f32; 3],
    std: [f32; 3],
    n: u32,
}

fn stats(src: &Pixmap, mask: Option<&Mask>) -> Stats {
    let mut s = Stats { hist: [[0; 256]; 3], mean: [0.0; 3], std: [0.0; 3], n: 0 };
    let d = src.data();
    let (mut sum, mut sq) = ([0f64; 3], [0f64; 3]);
    for i in 0..(src.width() * src.height()) as usize {
        if mask.is_some_and(|m| m.data()[i] < 128) {
            continue;
        }
        let p = crate::raster::unpremul([d[i * 4], d[i * 4 + 1], d[i * 4 + 2], d[i * 4 + 3]]);
        if p[3] == 0 {
            continue;
        }
        for k in 0..3 {
            s.hist[k][p[k] as usize] += 1;
        }
        let lab = to_lab(p);
        for k in 0..3 {
            sum[k] += lab[k] as f64;
            sq[k] += (lab[k] * lab[k]) as f64;
        }
        s.n += 1;
    }
    if s.n > 0 {
        let n = s.n as f64;
        for k in 0..3 {
            let m = sum[k] / n;
            s.mean[k] = m as f32;
            s.std[k] = ((sq[k] / n - m * m).max(0.0)).sqrt() as f32;
        }
    }
    s
}

fn cdf(h: &[u32; 256]) -> [f32; 256] {
    let total: u32 = h.iter().sum();
    let mut out = [0f32; 256];
    let mut acc = 0u32;
    for i in 0..256 {
        acc += h[i];
        out[i] = if total == 0 { i as f32 / 255.0 } else { acc as f32 / total as f32 };
    }
    out
}

impl ColorMatch {
    /// Use `img` as the reference. Returns false if it has no visible pixels.
    pub fn set_reference(&mut self, img: &Pixmap, label: &str) -> bool {
        let s = stats(img, None);
        if s.n == 0 {
            return false;
        }
        self.reference = Some(Reference { cdf: [cdf(&s.hist[0]), cdf(&s.hist[1]), cdf(&s.hist[2])], mean: s.mean, std: s.std });
        self.label = label.to_string();
        true
    }

    pub fn apply(&self, src: &Pixmap, mask: Option<&Mask>) -> Pixmap {
        let Some(r) = &self.reference else { return src.clone() };
        let t = stats(src, mask);
        if t.n == 0 {
            return src.clone();
        }
        let k = self.strength.clamp(0, 100) as f32 / 100.0;
        let mut out = src.clone();
        let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round().clamp(0.0, 255.0) as u8;
        if self.mode == 0 {
            let mut luts = [[0u8; 256]; 3];
            for c in 0..3 {
                let tc = cdf(&t.hist[c]);
                let mut j = 0usize;
                for i in 0..256 {
                    while j < 255 && r.cdf[c][j] < tc[i] - 1e-6 {
                        j += 1;
                    }
                    luts[c][i] = j as u8;
                }
            }
            for px in out.data_mut().chunks_exact_mut(4) {
                let s = crate::raster::unpremul([px[0], px[1], px[2], px[3]]);
                let m = [blend(s[0], luts[0][s[0] as usize]), blend(s[1], luts[1][s[1] as usize]), blend(s[2], luts[2][s[2] as usize]), s[3]];
                px.copy_from_slice(&crate::raster::premul(m));
            }
        } else {
            let gain: [f32; 3] = std::array::from_fn(|c| if t.std[c] < 1e-3 { 1.0 } else { (r.std[c] / t.std[c]).clamp(0.1, 10.0) });
            for px in out.data_mut().chunks_exact_mut(4) {
                let s = crate::raster::unpremul([px[0], px[1], px[2], px[3]]);
                if s[3] == 0 {
                    continue;
                }
                let lab = to_lab(s);
                let mut o: [f32; 3] = std::array::from_fn(|c| (lab[c] - t.mean[c]) * gain[c] + r.mean[c]);
                if self.keep_lightness {
                    o[0] = lab[0];
                }
                let rgb = from_lab(o);
                let m = [blend(s[0], rgb[0]), blend(s[1], rgb[1]), blend(s[2], rgb[2]), s[3]];
                px.copy_from_slice(&crate::raster::premul(m));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(r: u8, g: u8, b: u8) -> Pixmap {
        let mut p = Pixmap::new(8, 8).unwrap();
        p.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
        p
    }
    fn ramp() -> Pixmap {
        let mut p = Pixmap::new(64, 4).unwrap();
        for (i, px) in p.pixels_mut().iter_mut().enumerate() {
            let v = ((i % 64) * 4) as u8;
            *px = tiny_skia::ColorU8::from_rgba(v, v / 2, 255 - v, 255).premultiply();
        }
        p
    }

    #[test]
    fn lab_round_trip() {
        for c in [[10u8, 200, 90, 255], [255, 255, 255, 255], [0, 0, 0, 255], [128, 64, 200, 255]] {
            let o = from_lab(to_lab(c));
            for k in 0..3 {
                assert!((o[k] as i32 - c[k] as i32).abs() <= 1, "{c:?} -> {o:?}");
            }
        }
    }

    #[test]
    fn matching_a_flat_reference_moves_the_mean() {
        for mode in 0..2 {
            let mut cm = ColorMatch { mode, ..Default::default() };
            assert!(cm.set_reference(&flat(200, 60, 40), "t"));
            let out = cm.apply(&ramp(), None);
            let m = stats(&out, None).mean;
            let want = to_lab([200, 60, 40, 255]);
            assert!((m[1] - want[1]).abs() < 8.0 && (m[2] - want[2]).abs() < 8.0, "mode {mode}: {m:?} vs {want:?}");
        }
    }

    #[test]
    fn zero_strength_or_no_reference_is_identity() {
        let src = ramp();
        let cm = ColorMatch::default();
        assert_eq!(cm.apply(&src, None).data(), src.data());
        let mut cm = ColorMatch { strength: 0, ..Default::default() };
        cm.set_reference(&flat(10, 20, 30), "t");
        assert_eq!(cm.apply(&src, None).data(), src.data());
    }
}
