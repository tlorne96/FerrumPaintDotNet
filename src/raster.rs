//! Low level pixel helpers shared by the document, tools, adjustments and effects.
//! All `Pixmap` data is premultiplied RGBA8 (tiny-skia's native format).

use tiny_skia::{IntSize, Mask, Pixmap};

/// A straight (non-premultiplied) RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Col {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Col {
    pub const BLACK: Col = Col { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Col = Col { r: 255, g: 255, b: 255, a: 255 };
    pub const TRANSPARENT: Col = Col { r: 0, g: 0, b: 0, a: 0 };

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Col { r, g, b, a }
    }
    pub fn skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
    pub fn egui(self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }
    pub fn premul(self) -> [u8; 4] {
        let a = self.a as u32;
        [mul8(self.r as u32, a), mul8(self.g as u32, a), mul8(self.b as u32, a), self.a]
    }
    pub fn from_premul(p: [u8; 4]) -> Self {
        let [r, g, b, a] = unpremul(p);
        Col { r, g, b, a }
    }
    pub fn to_hex(self) -> String {
        format!("{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
    pub fn from_hex(s: &str) -> Option<Col> {
        let s = s.trim().trim_start_matches('#');
        let v = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Col::rgba(v(0)?, v(2)?, v(4)?, 255)),
            8 => Some(Col::rgba(v(0)?, v(2)?, v(4)?, v(6)?)),
            _ => None,
        }
    }
    pub fn lerp(a: Col, b: Col, t: f32) -> Col {
        let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
        Col::rgba(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
    }
}

#[inline]
pub fn mul8(c: u32, a: u32) -> u8 {
    let t = c * a + 128;
    (((t >> 8) + t) >> 8) as u8
}

#[inline]
pub fn unpremul(p: [u8; 4]) -> [u8; 4] {
    let a = p[3] as u32;
    if a == 0 {
        return [0, 0, 0, 0];
    }
    if a == 255 {
        return p;
    }
    let f = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
    [f(p[0]), f(p[1]), f(p[2]), p[3]]
}

#[inline]
pub fn premul(p: [u8; 4]) -> [u8; 4] {
    let a = p[3] as u32;
    [mul8(p[0] as u32, a), mul8(p[1] as u32, a), mul8(p[2] as u32, a), p[3]]
}

pub fn new_pixmap(w: u32, h: u32) -> Pixmap {
    Pixmap::new(w.max(1), h.max(1)).expect("pixmap allocation failed")
}

pub fn pixmap_from_vec(data: Vec<u8>, w: u32, h: u32) -> Pixmap {
    Pixmap::from_vec(data, IntSize::from_wh(w.max(1), h.max(1)).unwrap()).expect("bad pixmap size")
}

pub fn mask_from_vec(data: Vec<u8>, w: u32, h: u32) -> Mask {
    Mask::from_vec(data, IntSize::from_wh(w.max(1), h.max(1)).unwrap()).expect("bad mask size")
}

pub fn full_mask(w: u32, h: u32) -> Mask {
    mask_from_vec(vec![255; (w * h) as usize], w, h)
}

/// Straight RGBA bytes -> premultiplied pixmap.
pub fn pixmap_from_straight(rgba: &[u8], w: u32, h: u32) -> Pixmap {
    let mut data = rgba.to_vec();
    for px in data.chunks_exact_mut(4) {
        let p = premul([px[0], px[1], px[2], px[3]]);
        px.copy_from_slice(&p);
    }
    pixmap_from_vec(data, w, h)
}

/// Premultiplied pixmap -> straight RGBA bytes.
pub fn pixmap_to_straight(p: &Pixmap) -> Vec<u8> {
    let mut data = p.data().to_vec();
    for px in data.chunks_exact_mut(4) {
        let u = unpremul([px[0], px[1], px[2], px[3]]);
        px.copy_from_slice(&u);
    }
    data
}

/// Convert a mask into a pixmap whose alpha channel equals the mask (for transforming masks).
pub fn mask_to_pixmap(m: &Mask) -> Pixmap {
    let mut data = vec![0u8; m.data().len() * 4];
    for (i, &v) in m.data().iter().enumerate() {
        data[i * 4] = v;
        data[i * 4 + 1] = v;
        data[i * 4 + 2] = v;
        data[i * 4 + 3] = v;
    }
    pixmap_from_vec(data, m.width(), m.height())
}

/// Generic remap of a packed buffer (bpp bytes per pixel).
pub fn remap(
    data: &[u8],
    w: u32,
    h: u32,
    bpp: usize,
    nw: u32,
    nh: u32,
    f: impl Fn(u32, u32) -> (u32, u32),
) -> Vec<u8> {
    let mut out = vec![0u8; (nw * nh) as usize * bpp];
    for ny in 0..nh {
        for nx in 0..nw {
            let (sx, sy) = f(nx, ny);
            if sx < w && sy < h {
                let si = (sy * w + sx) as usize * bpp;
                let di = (ny * nw + nx) as usize * bpp;
                out[di..di + bpp].copy_from_slice(&data[si..si + bpp]);
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orient {
    FlipH,
    FlipV,
    Rot90Cw,
    Rot90Ccw,
    Rot180,
}

impl Orient {
    pub fn new_size(self, w: u32, h: u32) -> (u32, u32) {
        match self {
            Orient::Rot90Cw | Orient::Rot90Ccw => (h, w),
            _ => (w, h),
        }
    }
    fn src(self, w: u32, h: u32, nx: u32, ny: u32) -> (u32, u32) {
        match self {
            Orient::FlipH => (w - 1 - nx, ny),
            Orient::FlipV => (nx, h - 1 - ny),
            Orient::Rot180 => (w - 1 - nx, h - 1 - ny),
            // new image is h x w. new (nx, ny) came from old (ny, h-1-nx)
            Orient::Rot90Cw => (ny, h - 1 - nx),
            Orient::Rot90Ccw => (w - 1 - ny, nx),
        }
    }
}

pub fn orient_pixmap(p: &Pixmap, o: Orient) -> Pixmap {
    let (w, h) = (p.width(), p.height());
    let (nw, nh) = o.new_size(w, h);
    pixmap_from_vec(remap(p.data(), w, h, 4, nw, nh, |x, y| o.src(w, h, x, y)), nw, nh)
}

pub fn orient_mask(m: &Mask, o: Orient) -> Mask {
    let (w, h) = (m.width(), m.height());
    let (nw, nh) = o.new_size(w, h);
    mask_from_vec(remap(m.data(), w, h, 1, nw, nh, |x, y| o.src(w, h, x, y)), nw, nh)
}

/// Place `p` into a new canvas of size nw x nh at offset (ox, oy), filling empty area with `fill`.
pub fn recanvas_pixmap(p: &Pixmap, nw: u32, nh: u32, ox: i32, oy: i32, fill: Option<Col>) -> Pixmap {
    let mut out = new_pixmap(nw, nh);
    if let Some(c) = fill {
        out.fill(c.skia());
    }
    let (w, h) = (p.width() as i32, p.height() as i32);
    let src = p.data();
    let dst = out.data_mut();
    for y in 0..h {
        let ny = y + oy;
        if ny < 0 || ny >= nh as i32 {
            continue;
        }
        for x in 0..w {
            let nx = x + ox;
            if nx < 0 || nx >= nw as i32 {
                continue;
            }
            let si = ((y * w + x) * 4) as usize;
            let di = ((ny * nw as i32 + nx) * 4) as usize;
            dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

pub fn recanvas_mask(m: &Mask, nw: u32, nh: u32, ox: i32, oy: i32) -> Mask {
    let mut out = vec![0u8; (nw * nh) as usize];
    let (w, h) = (m.width() as i32, m.height() as i32);
    for y in 0..h {
        let ny = y + oy;
        if ny < 0 || ny >= nh as i32 {
            continue;
        }
        for x in 0..w {
            let nx = x + ox;
            if nx < 0 || nx >= nw as i32 {
                continue;
            }
            out[(ny * nw as i32 + nx) as usize] = m.data()[(y * w + x) as usize];
        }
    }
    mask_from_vec(out, nw, nh)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resampling {
    NearestNeighbor,
    Bilinear,
    Bicubic,
    Lanczos,
}

impl Resampling {
    pub const ALL: [Resampling; 4] =
        [Resampling::Bicubic, Resampling::Bilinear, Resampling::Lanczos, Resampling::NearestNeighbor];
    pub fn name(self) -> &'static str {
        match self {
            Resampling::NearestNeighbor => "Nearest Neighbor",
            Resampling::Bilinear => "Bilinear",
            Resampling::Bicubic => "Bicubic",
            Resampling::Lanczos => "Lanczos (best quality)",
        }
    }
}

pub fn resize_pixmap(p: &Pixmap, nw: u32, nh: u32, r: Resampling) -> Pixmap {
    use image::imageops::FilterType;
    // Resizing premultiplied data is the correct way to avoid dark fringes.
    let img = image::RgbaImage::from_raw(p.width(), p.height(), p.data().to_vec()).unwrap();
    let f = match r {
        Resampling::NearestNeighbor => FilterType::Nearest,
        Resampling::Bilinear => FilterType::Triangle,
        Resampling::Bicubic => FilterType::CatmullRom,
        Resampling::Lanczos => FilterType::Lanczos3,
    };
    let out = image::imageops::resize(&img, nw.max(1), nh.max(1), f);
    let mut data = out.into_raw();
    // Ringing filters can produce colour > alpha; clamp to stay valid premultiplied.
    for px in data.chunks_exact_mut(4) {
        let a = px[3];
        px[0] = px[0].min(a);
        px[1] = px[1].min(a);
        px[2] = px[2].min(a);
    }
    pixmap_from_vec(data, nw, nh)
}

pub fn resize_mask(m: &Mask, nw: u32, nh: u32) -> Mask {
    let img = image::GrayImage::from_raw(m.width(), m.height(), m.data().to_vec()).unwrap();
    let out = image::imageops::resize(&img, nw.max(1), nh.max(1), image::imageops::FilterType::Triangle);
    mask_from_vec(out.into_raw(), nw, nh)
}

/// Blend `adjusted` over `base` according to selection mask; writes into `out`.
pub fn masked_mix(base: &Pixmap, adjusted: &Pixmap, mask: Option<&Mask>, out: &mut Pixmap) {
    let b = base.data();
    let a = adjusted.data();
    let o = out.data_mut();
    match mask {
        None => o.copy_from_slice(a),
        Some(m) => {
            let md = m.data();
            for i in 0..md.len() {
                let mv = md[i] as u32;
                let j = i * 4;
                if mv == 0 {
                    o[j..j + 4].copy_from_slice(&b[j..j + 4]);
                } else if mv == 255 {
                    o[j..j + 4].copy_from_slice(&a[j..j + 4]);
                } else {
                    for k in 0..4 {
                        let bv = b[j + k] as u32;
                        let av = a[j + k] as u32;
                        o[j + k] = ((bv * (255 - mv) + av * mv + 127) / 255) as u8;
                    }
                }
            }
        }
    }
}

/// Colour distance in 0..=1 (RGBA euclidean, normalised).
#[inline]
pub fn color_dist(a: [u8; 4], b: [u8; 4]) -> f32 {
    let mut s = 0.0f32;
    for k in 0..4 {
        let d = a[k] as f32 - b[k] as f32;
        s += d * d;
    }
    (s / (4.0 * 255.0 * 255.0)).sqrt()
}

/// Flood fill returning a 0/255 mask. `tolerance` in 0..=1. Works on straight colours.
pub fn flood_mask(p: &Pixmap, x: u32, y: u32, tolerance: f32, global: bool) -> Mask {
    let (w, h) = (p.width(), p.height());
    let px = p.data();
    let get = |i: usize| unpremul([px[i * 4], px[i * 4 + 1], px[i * 4 + 2], px[i * 4 + 3]]);
    let seed = get((y * w + x) as usize);
    let tol = tolerance * tolerance; // perceptual-ish curve
    let matches = |i: usize| color_dist(get(i), seed) <= tol + 1e-6;
    let mut out = vec![0u8; (w * h) as usize];
    if global {
        for i in 0..(w * h) as usize {
            if matches(i) {
                out[i] = 255;
            }
        }
        return mask_from_vec(out, w, h);
    }
    let mut visited = vec![false; (w * h) as usize];
    let mut stack = vec![(x, y)];
    while let Some((sx, sy)) = stack.pop() {
        let row = (sy * w) as usize;
        if visited[row + sx as usize] {
            continue;
        }
        // scan left & right
        let mut l = sx;
        while l > 0 && !visited[row + l as usize - 1] && matches(row + l as usize - 1) {
            l -= 1;
        }
        let mut r = sx;
        while r + 1 < w && !visited[row + r as usize + 1] && matches(row + r as usize + 1) {
            r += 1;
        }
        if !matches(row + sx as usize) {
            visited[row + sx as usize] = true;
            continue;
        }
        for xx in l..=r {
            visited[row + xx as usize] = true;
            out[row + xx as usize] = 255;
        }
        for ny in [sy.wrapping_sub(1), sy + 1] {
            if ny >= h {
                continue;
            }
            let nrow = (ny * w) as usize;
            let mut xx = l;
            while xx <= r {
                let i = nrow + xx as usize;
                if !visited[i] && matches(i) {
                    stack.push((xx, ny));
                    // skip the rest of this run
                    while xx <= r && matches(nrow + xx as usize) {
                        xx += 1;
                    }
                } else {
                    xx += 1;
                }
            }
        }
    }
    mask_from_vec(out, w, h)
}

/// Bounding box of mask values >= 1: (x0, y0, x1, y1) exclusive.
pub fn mask_bounds(m: &Mask) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (m.width(), m.height());
    let d = m.data();
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..h {
        let row = &d[(y * w) as usize..((y + 1) * w) as usize];
        if let Some(first) = row.iter().position(|&v| v > 0) {
            let last = row.iter().rposition(|&v| v > 0).unwrap();
            x0 = x0.min(first as u32);
            x1 = x1.max(last as u32 + 1);
            y0 = y0.min(y);
            y1 = y + 1;
        }
    }
    if x0 == u32::MAX {
        None
    } else {
        Some((x0, y0, x1, y1))
    }
}

// ----------------------------------------------------------------------------------------------
// Colour space helpers

pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |t: f32| ((t + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r), f(g), f(b))
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r + m, g + m, b + m)
}

#[inline]
fn luma(r: f32, g: f32, b: f32) -> f32 {
    0.299 * r + 0.587 * g + 0.114 * b
}

// ----------------------------------------------------------------------------------------------
// Adjustments & effects

#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    // Adjustments
    AutoLevel,
    BlackAndWhite,
    BrightnessContrast { brightness: i32, contrast: i32 },
    Curves { points: Vec<(u8, u8)>, luminosity: bool },
    /// Curves+ (port of pyrochild's plugin): multi-mode, per-channel curves.
    CurvesPlus(Box<crate::curves_plus::CurvesPlus>),
    /// Color Match (clean-room): match colours to a reference image.
    ColorMatch(Box<crate::color_match::ColorMatch>),
    HueSaturation { hue: i32, saturation: i32, lightness: i32 },
    InvertAlpha,
    InvertColors,
    Levels { in_lo: u8, in_hi: u8, gamma: f32, out_lo: u8, out_hi: u8 },
    Posterize { levels: u8 },
    /// Parametric adjustments and all effects.
    Fx(crate::fx::FxState),
}

impl Filter {
    pub fn name(&self) -> &'static str {
        match self {
            Filter::AutoLevel => "Auto-Level",
            Filter::BlackAndWhite => "Black and White",
            Filter::BrightnessContrast { .. } => "Brightness / Contrast",
            Filter::Curves { .. } => "Curves",
            Filter::CurvesPlus(_) => "Curves+",
            Filter::ColorMatch(_) => "Color Match",
            Filter::HueSaturation { .. } => "Hue / Saturation",
            Filter::InvertAlpha => "Invert Alpha",
            Filter::InvertColors => "Invert Colors",
            Filter::Levels { .. } => "Levels",
            Filter::Posterize { .. } => "Posterize",
            Filter::Fx(s) => s.kind.name(),
        }
    }
    pub fn has_dialog(&self) -> bool {
        !matches!(
            self,
            Filter::AutoLevel | Filter::BlackAndWhite | Filter::InvertAlpha | Filter::InvertColors
        )
    }
    pub fn is_effect(&self) -> bool {
        matches!(self, Filter::Fx(s) if s.kind.is_effect())
    }
}

fn map_straight(src: &Pixmap, f: impl Fn([u8; 4]) -> [u8; 4]) -> Pixmap {
    let mut out = src.clone();
    for px in out.data_mut().chunks_exact_mut(4) {
        let s = unpremul([px[0], px[1], px[2], px[3]]);
        let r = premul(f(s));
        px.copy_from_slice(&r);
    }
    out
}

fn lut_map(src: &Pixmap, lut: &[u8; 256]) -> Pixmap {
    map_straight(src, |p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize], p[3]])
}

/// Monotone-ish smooth curve through control points (Catmull-Rom, clamped), as a 256 entry LUT.
pub fn curve_lut(points: &[(u8, u8)]) -> [u8; 256] {
    let mut pts: Vec<(f32, f32)> = points.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    pts.dedup_by(|a, b| a.0 == b.0);
    let mut lut = [0u8; 256];
    if pts.is_empty() {
        for (i, v) in lut.iter_mut().enumerate() {
            *v = i as u8;
        }
        return lut;
    }
    for (i, v) in lut.iter_mut().enumerate() {
        let x = i as f32;
        let y = if x <= pts[0].0 {
            pts[0].1
        } else if x >= pts[pts.len() - 1].0 {
            pts[pts.len() - 1].1
        } else {
            let k = pts.iter().position(|p| p.0 > x).unwrap() - 1;
            let p1 = pts[k];
            let p2 = pts[k + 1];
            let p0 = if k > 0 { pts[k - 1] } else { (2.0 * p1.0 - p2.0, 2.0 * p1.1 - p2.1) };
            let p3 = if k + 2 < pts.len() { pts[k + 2] } else { (2.0 * p2.0 - p1.0, 2.0 * p2.1 - p1.1) };
            let t = (x - p1.0) / (p2.0 - p1.0);
            // tangents scaled by segment width
            let m1 = (p2.1 - p0.1) / (p2.0 - p0.0) * (p2.0 - p1.0);
            let m2 = (p3.1 - p1.1) / (p3.0 - p1.0) * (p2.0 - p1.0);
            let t2 = t * t;
            let t3 = t2 * t;
            (2.0 * t3 - 3.0 * t2 + 1.0) * p1.1 + (t3 - 2.0 * t2 + t) * m1 + (-2.0 * t3 + 3.0 * t2) * p2.1 + (t3 - t2) * m2
        };
        *v = y.round().clamp(0.0, 255.0) as u8;
    }
    lut
}

/// Apply a filter to a whole pixmap (selection masking is done by the caller).
/// `bounds` is the selection bounding box, used by Auto-Level and Vignette.
pub fn apply_filter(f: &Filter, src: &Pixmap, mask: Option<&Mask>) -> Pixmap {
    match f {
        Filter::InvertColors => map_straight(src, |p| [255 - p[0], 255 - p[1], 255 - p[2], p[3]]),
        Filter::InvertAlpha => map_straight(src, |p| [p[0], p[1], p[2], 255 - p[3]]),
        Filter::BlackAndWhite => map_straight(src, |p| {
            let l = luma(p[0] as f32, p[1] as f32, p[2] as f32).round().clamp(0.0, 255.0) as u8;
            [l, l, l, p[3]]
        }),
        Filter::AutoLevel => {
            let mut lo = [255u8; 3];
            let mut hi = [0u8; 3];
            let d = src.data();
            for i in 0..(src.width() * src.height()) as usize {
                if let Some(m) = mask {
                    if m.data()[i] < 128 {
                        continue;
                    }
                }
                let p = unpremul([d[i * 4], d[i * 4 + 1], d[i * 4 + 2], d[i * 4 + 3]]);
                if p[3] == 0 {
                    continue;
                }
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let mut luts = [[0u8; 256]; 3];
            for k in 0..3 {
                let (l, h) = (lo[k] as f32, (hi[k].max(lo[k] + 1)) as f32);
                for i in 0..256 {
                    luts[k][i] = (((i as f32 - l) / (h - l)) * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
            map_straight(src, |p| [luts[0][p[0] as usize], luts[1][p[1] as usize], luts[2][p[2] as usize], p[3]])
        }
        Filter::BrightnessContrast { brightness, contrast } => {
            let b = *brightness as f32 / 100.0 * 255.0 * 0.5;
            let c = *contrast as f32 / 100.0;
            let k = if c >= 0.0 { 1.0 / (1.0 - c * 0.99) } else { 1.0 + c };
            let mut lut = [0u8; 256];
            for (i, v) in lut.iter_mut().enumerate() {
                let x = (i as f32 - 127.5) * k + 127.5 + b;
                *v = x.round().clamp(0.0, 255.0) as u8;
            }
            lut_map(src, &lut)
        }
        Filter::ColorMatch(cm) => cm.apply(src, mask),
        Filter::CurvesPlus(cp) => {
            let op = cp.op();
            map_straight(src, |p| op.apply(p))
        }
        Filter::Curves { points, luminosity } => {
            let lut = curve_lut(points);
            if *luminosity {
                map_straight(src, |p| {
                    let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
                    let l = luma(r, g, b);
                    let nl = lut[l.round().clamp(0.0, 255.0) as usize] as f32;
                    let d = nl - l;
                    let c = |v: f32| (v + d).round().clamp(0.0, 255.0) as u8;
                    [c(r), c(g), c(b), p[3]]
                })
            } else {
                lut_map(src, &lut)
            }
        }
        Filter::HueSaturation { hue, saturation, lightness } => {
            let (dh, sm, dl) = (*hue as f32, *saturation as f32 / 100.0, *lightness as f32 / 100.0);
            map_straight(src, |p| {
                let (h, s, l) = rgb_to_hsl(p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0);
                let s2 = (s * sm).clamp(0.0, 1.0);
                let l2 = if dl >= 0.0 { l + (1.0 - l) * dl } else { l * (1.0 + dl) };
                let (r, g, b) = hsl_to_rgb(h + dh, s2, l2.clamp(0.0, 1.0));
                let c = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
                [c(r), c(g), c(b), p[3]]
            })
        }
        Filter::Levels { in_lo, in_hi, gamma, out_lo, out_hi } => {
            let mut lut = [0u8; 256];
            let (il, ih) = (*in_lo as f32, (*in_hi).max(in_lo.saturating_add(1)) as f32);
            for (i, v) in lut.iter_mut().enumerate() {
                let t = ((i as f32 - il) / (ih - il)).clamp(0.0, 1.0).powf(1.0 / gamma.max(0.05));
                *v = (*out_lo as f32 + t * (*out_hi as f32 - *out_lo as f32)).round().clamp(0.0, 255.0) as u8;
            }
            lut_map(src, &lut)
        }
        Filter::Posterize { levels } => {
            let n = (*levels).max(2) as f32;
            let mut lut = [0u8; 256];
            for (i, v) in lut.iter_mut().enumerate() {
                let q = ((i as f32 / 255.0) * (n - 1.0)).round() / (n - 1.0);
                *v = (q * 255.0).round() as u8;
            }
            lut_map(src, &lut)
        }
        Filter::Fx(s) => crate::fx::apply(s, src, mask),
    }
}

/// 3-pass box blur approximation of a gaussian blur on premultiplied data.
pub fn gaussian_blur(src: &Pixmap, radius: u32) -> Pixmap {
    if radius == 0 {
        return src.clone();
    }
    let (w, h) = (src.width() as usize, src.height() as usize);
    let sigma = radius as f32 / 2.0;
    // box sizes for 3 passes
    let n = 3.0;
    let wideal = ((12.0 * sigma * sigma / n) + 1.0).sqrt();
    let mut wl = wideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let mideal = (12.0 * sigma * sigma - n * (wl * wl) as f32 - 4.0 * n * wl as f32 - 3.0 * n) / (-4.0 * wl as f32 - 4.0);
    let m = mideal.round() as i32;
    let sizes: Vec<i32> = (0..3).map(|i| if i < m { wl } else { wu }).collect();
    let mut a: Vec<f32> = src.data().iter().map(|&v| v as f32).collect();
    let mut b = vec![0f32; a.len()];
    for s in sizes {
        let r = ((s - 1) / 2).max(0) as usize;
        if r == 0 {
            continue;
        }
        box_h(&a, &mut b, w, h, r);
        box_v(&b, &mut a, w, h, r);
    }
    let data: Vec<u8> = a.iter().map(|v| v.round().clamp(0.0, 255.0) as u8).collect();
    let mut p = pixmap_from_vec(data, w as u32, h as u32);
    for px in p.data_mut().chunks_exact_mut(4) {
        let al = px[3];
        px[0] = px[0].min(al);
        px[1] = px[1].min(al);
        px[2] = px[2].min(al);
    }
    p
}

fn box_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let iarr = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        for c in 0..4 {
            let at = |x: isize| src[(y * w + x.clamp(0, w as isize - 1) as usize) * 4 + c];
            let mut acc = 0.0;
            for x in -(r as isize)..=(r as isize) {
                acc += at(x);
            }
            for x in 0..w {
                dst[(y * w + x) * 4 + c] = acc * iarr;
                acc += at(x as isize + r as isize + 1) - at(x as isize - r as isize);
            }
        }
    }
}

fn box_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let iarr = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        for c in 0..4 {
            let at = |y: isize| src[(y.clamp(0, h as isize - 1) as usize * w + x) * 4 + c];
            let mut acc = 0.0;
            for y in -(r as isize)..=(r as isize) {
                acc += at(y);
            }
            for y in 0..h {
                dst[(y * w + x) * 4 + c] = acc * iarr;
                acc += at(y as isize + r as isize + 1) - at(y as isize - r as isize);
            }
        }
    }
}

/// Combine masks for selection modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelMode {
    Replace,
    Union,
    Exclude,
    Intersect,
    Xor,
}

impl SelMode {
    pub const ALL: [SelMode; 5] = [SelMode::Replace, SelMode::Union, SelMode::Exclude, SelMode::Intersect, SelMode::Xor];
    pub fn name(self) -> &'static str {
        match self {
            SelMode::Replace => "Replace",
            SelMode::Union => "Add (union)",
            SelMode::Exclude => "Subtract",
            SelMode::Intersect => "Intersect",
            SelMode::Xor => "Invert (xor)",
        }
    }
}

pub fn combine_masks(old: Option<&Mask>, new: &Mask, mode: SelMode) -> Mask {
    let mut out = new.clone();
    let Some(old) = old else {
        return match mode {
            SelMode::Replace | SelMode::Union | SelMode::Xor => out,
            SelMode::Exclude | SelMode::Intersect => {
                out.clear();
                out
            }
        };
    };
    let a = old.data();
    let o = out.data_mut();
    for i in 0..o.len() {
        let (x, y) = (a[i] as u32, o[i] as u32);
        o[i] = match mode {
            SelMode::Replace => y,
            SelMode::Union => x.max(y),
            SelMode::Exclude => x * (255 - y) / 255,
            SelMode::Intersect => x * y / 255,
            SelMode::Xor => (x as i32 - y as i32).unsigned_abs(),
        } as u8;
    }
    out
}
