//! Interactive "Tools" effects: Rust ports of pyrochild's (Zach Walker's) MIT-licensed Paint.NET
//! plugins Smudge, Liquify and Grid Warp, from the published source at https://github.com/bsneeze
//! (pdn-smudge, pdn-liquify, pdn-gridwarp).
//!
//! Copyright (c) 2007-2026 Zach Walker. Ported to Rust for Ferrum Paint. MIT License; the full
//! licence text is in THIRD_PARTY_NOTICES.md.
//!
//! What was ported is the plugins' own algorithms: the smudge brush (SmudgeRenderer, RoundBrush),
//! the Liquify displacement mesh and brush modes (DisplacementMesh, LiquifyRenderer) and Grid Warp's
//! handle grid with cosine interpolation (DisplacementGrid, CosineInterpolator). The plugins'
//! WinForms canvas, sliders and the helper files that carry Paint.NET copyright headers were not
//! used; the dialog here is Ferrum's own egui UI.

use egui::{vec2, Color32, Pos2, Rect, Sense, Stroke, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Smudge,
    Liquify,
    GridWarp,
}
impl Kind {
    pub const ALL: [Kind; 3] = [Kind::GridWarp, Kind::Liquify, Kind::Smudge];
    pub fn name(self) -> &'static str {
        match self {
            Kind::Smudge => "Smudge",
            Kind::Liquify => "Liquify",
            Kind::GridWarp => "Grid Warp",
        }
    }
}

type Rgba = [u8; 4];

/// Integer rectangle, right/bottom exclusive.
#[derive(Clone, Copy, Debug, PartialEq)]
struct R {
    l: i32,
    t: i32,
    r: i32,
    b: i32,
}
impl R {
    fn xywh(x: i32, y: i32, w: i32, h: i32) -> R {
        R { l: x, t: y, r: x + w, b: y + h }
    }
    fn union(self, o: R) -> R {
        R { l: self.l.min(o.l), t: self.t.min(o.t), r: self.r.max(o.r), b: self.b.max(o.b) }
    }
    fn intersect(self, o: R) -> R {
        let r = R { l: self.l.max(o.l), t: self.t.max(o.t), r: self.r.min(o.r), b: self.b.min(o.b) };
        if r.r <= r.l || r.b <= r.t {
            R { l: 0, t: 0, r: 0, b: 0 }
        } else {
            r
        }
    }
    fn inflate(self, d: i32) -> R {
        R { l: self.l - d, t: self.t - d, r: self.r + d, b: self.b + d }
    }
    fn empty(self) -> bool {
        self.r <= self.l || self.b <= self.t
    }
}

// ----------------------------------------------------------------------------------------------
// Shared: displacement mesh (Liquify and Grid Warp) and bilinear image sampling

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Dv {
    x: f32,
    y: f32,
    mask: u8,
}
impl Dv {
    fn lerp(a: Dv, b: Dv, f: f32) -> Dv {
        let i = 1.0 - f;
        Dv { x: a.x * i + b.x * f, y: a.y * i + b.y * f, mask: 0 }
    }
}

#[derive(Clone)]
struct Mesh {
    w: i32,
    h: i32,
    v: Vec<Dv>,
}
impl Mesh {
    fn new(w: i32, h: i32) -> Mesh {
        Mesh { w, h, v: vec![Dv::default(); (w * h) as usize] }
    }
    #[inline]
    fn at(&self, x: i32, y: i32) -> Dv {
        self.v[(y * self.w + x) as usize]
    }
    #[inline]
    fn at_mut(&mut self, x: i32, y: i32) -> &mut Dv {
        &mut self.v[(y * self.w + x) as usize]
    }
    fn bounds(&self) -> R {
        R { l: 0, t: 0, r: self.w, b: self.h }
    }
    /// DisplacementMesh.GetBilinearSample (clamped to the mesh; the mask is not interpolated).
    fn bilinear(&self, mut x: f32, mut y: f32) -> Dv {
        let (x0, y0);
        if x >= (self.w - 1) as f32 {
            x = (self.w - 1) as f32;
            x0 = (self.w - 2).max(0);
        } else {
            if x < 0.0 {
                x = 0.0;
            }
            x0 = x as i32;
        }
        if y >= (self.h - 1) as f32 {
            y = (self.h - 1) as f32;
            y0 = (self.h - 2).max(0);
        } else {
            if y < 0.0 {
                y = 0.0;
            }
            y0 = y as i32;
        }
        let xs = if self.w > 1 { 1 } else { 0 };
        let ys = if self.h > 1 { 1 } else { 0 };
        let fx = x - x0 as f32;
        let t = Dv::lerp(self.at(x0, y0), self.at(x0 + xs, y0), fx);
        let b = Dv::lerp(self.at(x0, y0 + ys), self.at(x0 + xs, y0 + ys), fx);
        Dv::lerp(t, b, y - y0 as f32)
    }
    fn clear_offsets(&mut self) {
        for d in &mut self.v {
            d.x = 0.0;
            d.y = 0.0;
        }
    }
}

/// Bilinear sample of a straight-alpha image, clamped at the edges, alpha-weighted.
fn sample(src: &[Rgba], w: i32, h: i32, x: f32, y: f32) -> Rgba {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let px = |xx: i32, yy: i32| src[(yy * w + xx) as usize];
    let ws = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
    let ps = [px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1)];
    let mut a = 0.0;
    let mut c = [0.0f32; 3];
    for k in 0..4 {
        let wa = ws[k] * ps[k][3] as f32;
        a += wa;
        for i in 0..3 {
            c[i] += wa * ps[k][i] as f32;
        }
    }
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    [(c[0] / a + 0.5) as u8, (c[1] / a + 0.5) as u8, (c[2] / a + 0.5) as u8, (a + 0.5).min(255.0) as u8]
}

/// DisplacementMesh.Render: dst[x,y] = src[x + dv.x, y + dv.y] (bilinear), over `rect`.
fn render_mesh(mesh: &Mesh, src: &[Rgba], dst: &mut [Rgba], rect: R, mask_tint: bool) {
    let (w, h) = (mesh.w, mesh.h);
    let rect = rect.intersect(mesh.bounds());
    for y in rect.t..rect.b {
        for x in rect.l..rect.r {
            let d = mesh.at(x, y);
            let mut p = sample(src, w, h, x as f32 + d.x, y as f32 + d.y);
            if mask_tint && d.mask > 0 {
                // frozen areas shown with a translucent red tint (the plugin's mask colour overlay)
                let k = d.mask as f32 / 510.0;
                let a = p[3] as f32 / 255.0;
                let oa = k + a * (1.0 - k);
                if oa > 0.0 {
                    for i in 0..3 {
                        let top = if i == 0 { 255.0 } else { 0.0 };
                        p[i] = ((top * k + p[i] as f32 * a * (1.0 - k)) / oa + 0.5) as u8;
                    }
                    p[3] = (oa * 255.0 + 0.5) as u8;
                }
            }
            dst[(y * w + x) as usize] = p;
        }
    }
}

/// DisplacementMesh.RenderSupersampled: averages up to 4x4 samples where the mesh squeezes the
/// source together, otherwise the same as render_mesh.
fn render_mesh_supersampled(mesh: &Mesh, src: &[Rgba], dst: &mut [Rgba]) {
    let (w, h) = (mesh.w, mesh.h);
    for y in 0..h {
        let down = if y < h - 1 { 1 } else { 0 };
        for x in 0..w {
            let o = mesh.at(x, y);
            let right = mesh.at(x + if x < w - 1 { 1 } else { 0 }, y);
            let below = mesh.at(x, y + down);
            let ax = 1.0 + right.x - o.x;
            let ay = right.y - o.y;
            let bx = below.x - o.x;
            let by = 1.0 + below.y - o.y;
            let stretch = (ax * ax + ay * ay).max(bx * bx + by * by).sqrt();
            let out = if stretch <= 1.05 {
                sample(src, w, h, x as f32 + o.x, y as f32 + o.y)
            } else {
                let n = 4.min(2.max(stretch.ceil() as i32));
                let (mut a, mut r, mut g, mut b) = (0i64, 0i64, 0i64, 0i64);
                for j in 0..n {
                    let sy = y as f32 + (j as f32 + 0.5) / n as f32 - 0.5;
                    for i in 0..n {
                        let sx = x as f32 + (i as f32 + 0.5) / n as f32 - 0.5;
                        let v = mesh.bilinear(sx, sy);
                        let s = sample(src, w, h, sx + v.x, sy + v.y);
                        let sa = s[3] as i64;
                        a += sa;
                        r += s[0] as i64 * sa;
                        g += s[1] as i64 * sa;
                        b += s[2] as i64 * sa;
                    }
                }
                if a == 0 {
                    [0, 0, 0, 0]
                } else {
                    let nn = (n * n) as i64;
                    [((r + a / 2) / a) as u8, ((g + a / 2) / a) as u8, ((b + a / 2) / a) as u8, ((a + nn / 2) / nn) as u8]
                }
            };
            dst[(y * w + x) as usize] = out;
        }
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}
fn lerp_pt(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
}

// ----------------------------------------------------------------------------------------------
// Smudge (SmudgeRenderer + RoundBrush)

pub const SMUDGE_BRUSHES: &[&str] = &["Soft", "Hard"];

/// RoundBrush.CreateBrush: per-pixel strength for a round brush of `d` pixels.
fn round_brush(d: i32, center: f32, edge: f32, hole_ratio: f32) -> Vec<f32> {
    let mut out = vec![0.0; (d * d) as usize];
    let radius = d as f32 / 2.0 - 1.0;
    let hole = radius * hole_ratio - 1.0;
    let recip = 1.0 / (radius - hole);
    for y in 0..d {
        let dy = y as f32 - radius;
        for x in 0..d {
            let dx = x as f32 - radius;
            let dd = (dx * dx + dy * dy).sqrt();
            let f = (dd - hole) * recip;
            let v = if dd < radius && dd >= hole {
                center + (edge - center) * f
            } else if dd <= radius + 1.0 && dd >= radius {
                edge * (radius + 1.0 - dd)
            } else if dd < hole && dd >= hole - 1.0 {
                center * (1.0 - hole + dd)
            } else {
                0.0
            };
            out[(y * d + x) as usize] = v;
        }
    }
    out
}

struct SmStroke {
    bw: i32,
    bh: i32,
    brush: Vec<[f32; 4]>,
    strmask: Vec<f32>,
    spacing: f32,
}

struct Smudge {
    img: Vec<Rgba>,
    brush: usize,
    width: i32,
    strength: f32,
    jitter: f32,
    quality: f32,
    stroke: Option<SmStroke>,
    last: (i32, i32),
    remaining: f32,
    rng: u64,
}
impl Smudge {
    fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        // xorshift; Random.Next(lo, hi) is [lo, hi)
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        if hi <= lo {
            return lo;
        }
        lo + (self.rng % (hi - lo) as u64) as i32
    }
    fn down(&mut self, w: i32, h: i32, p: (i32, i32)) {
        self.last = p;
        let d = self.width.max(2);
        let (c, e) = if self.brush == 1 { (1.0, 1.0) } else { (1.0, 0.0) };
        let alpha: Vec<u8> = round_brush(d, c, e, 0.0).iter().map(|v| (255.0 * v + 0.5).clamp(0.0, 255.0) as u8).collect();
        let (bw, bh) = (d, d);
        let spacing = {
            let size = bw.max(bh) as f32;
            size + self.quality - size * self.quality
        };
        let strength = self.strength.max(0.0).powf(0.25) / 255.0;
        let strmask: Vec<f32> = alpha.iter().map(|&a| a as f32 * strength).collect();
        let (sx, sy) = (p.0 - bw / 2, p.1 - bh / 2);
        let mut brush = Vec::with_capacity((bw * bh) as usize);
        for y in 0..bh {
            let yy = (sy + y).clamp(0, h - 1);
            for x in 0..bw {
                let xx = (sx + x).clamp(0, w - 1);
                let s = self.img[(yy * w + xx) as usize];
                brush.push([s[0] as f32, s[1] as f32, s[2] as f32, s[3] as f32]);
            }
        }
        self.stroke = Some(SmStroke { bw, bh, brush, strmask, spacing: spacing.max(0.01) });
    }
    fn moved(&mut self, w: i32, h: i32, e: (i32, i32)) -> Option<R> {
        let st = self.stroke.as_ref()?;
        let (bw, bh, spacing) = (st.bw, st.bh, st.spacing);
        let jm = (self.jitter * self.width as f32) as i32;
        let inv = R::xywh(self.last.0 - bw / 2, self.last.1 - bh / 2, bw, bh).union(R::xywh(e.0 - bw / 2, e.1 - bh / 2, bw, bh)).inflate(jm);
        let lastf = (self.last.0 as f32, self.last.1 as f32);
        let ef = (e.0 as f32, e.1 as f32);
        let d = dist(lastf, ef);
        let mut f = self.remaining;
        while f < d {
            let mut cp = lerp_pt(lastf, ef, f / d);
            if jm > 0 {
                cp.0 += self.rand_range(-jm, jm) as f32;
                cp.1 += self.rand_range(-jm, jm) as f32;
            }
            let (dx, dy) = (cp.0 as i32 - bw / 2, cp.1 as i32 - bh / 2);
            let st = self.stroke.as_mut().unwrap();
            let mut k = 0usize;
            for y in dy..dy + bh {
                let yy = y.clamp(0, h - 1);
                for x in dx..dx + bw {
                    let xx = x.clamp(0, w - 1);
                    let s = &mut self.img[(yy * w + xx) as usize];
                    let b = &mut st.brush[k];
                    let m = st.strmask[k];
                    let inv = 1.0 - m;
                    // no fringing from fully transparent pixels
                    if b[3] == 0.0 {
                        b[0] = s[0] as f32;
                        b[1] = s[1] as f32;
                        b[2] = s[2] as f32;
                    } else if s[3] == 0 {
                        s[0] = b[0] as u8;
                        s[1] = b[1] as u8;
                        s[2] = b[2] as u8;
                    }
                    for c in 0..4 {
                        b[c] = b[c] * m + s[c] as f32 * inv;
                        s[c] = (b[c] + 0.5).clamp(0.0, 255.0) as u8;
                    }
                    k += 1;
                }
            }
            f += spacing;
        }
        self.remaining = f - d;
        self.last = e;
        Some(inv)
    }
}

// ----------------------------------------------------------------------------------------------
// Liquify (LiquifyRenderer)

/// Brush modes in the plugin's order (Turbulence exists in its enum but does nothing, so it's left out).
pub const LIQUIFY_MODES: &[&str] = &["Push", "Bloat", "Pucker", "Twist Left", "Twist Right", "Reconstruct", "Freeze", "Thaw"];

struct LqStroke {
    size: i32,
    radius: i32,
    center: f32,
    inv_r2: f32,
    spacing: i32,
    pressure: f32,
    density: Vec<f32>,
    mode: usize,
    buffer: Vec<Dv>,
}

struct Liquify {
    mesh: Mesh,
    mode: usize,
    size: i32,
    pressure: f32,
    density: f32,
    stroke: Option<LqStroke>,
    last: (i32, i32),
    remaining: f32,
}
impl Liquify {
    fn falloff_exponent(density: f32) -> f32 {
        2.0 - 2.0 * density
    }
    /// Right-button mode: Freeze <-> Thaw, otherwise Reconstruct.
    fn alt_mode(mode: usize) -> usize {
        match mode {
            6 => 7,
            7 => 6,
            _ => 5,
        }
    }
    fn down(&mut self, p: (i32, i32), mode: usize) -> Option<R> {
        let size = self.size.max(2);
        let mut density = vec![0.0; 64.max(size as usize)];
        let n = density.len();
        let ex = Self::falloff_exponent(self.density);
        for (i, d) in density.iter_mut().enumerate() {
            *d = (1.0 - i as f32 / n as f32).powf(ex);
        }
        self.stroke = Some(LqStroke {
            size,
            radius: size / 2,
            center: (size - 1) as f32 / 2.0,
            inv_r2: 4.0 / (size as f32 * size as f32),
            spacing: (size / 2 / 4).max(1),
            pressure: self.pressure,
            density,
            mode,
            buffer: vec![Dv::default(); (size * size) as usize],
        });
        self.remaining = 0.0;
        self.last = p;
        self.moved(p)
    }
    fn hold(&mut self) -> Option<R> {
        let st = self.stroke.as_ref()?;
        if st.mode != 0 {
            self.remaining = 0.0;
        }
        let p = self.last;
        self.moved(p)
    }
    fn moved(&mut self, e: (i32, i32)) -> Option<R> {
        let Some(st) = self.stroke.as_mut() else {
            self.last = e;
            return None;
        };
        let (size, radius, spacing) = (st.size, st.radius, st.spacing);
        let inv = R::xywh(self.last.0 - radius, self.last.1 - radius, size, size).union(R::xywh(e.0 - radius, e.1 - radius, size, size));
        let lastf = (self.last.0 as f32, self.last.1 as f32);
        let ef = (e.0 as f32, e.1 as f32);
        let mut d = dist(lastf, ef);
        if d == 0.0 {
            d = 1.0;
        }
        let u = ((ef.0 - lastf.0) / d, (ef.1 - lastf.1) / d);
        let mut f = self.remaining;
        let bounds = self.mesh.bounds();
        while f < d {
            let cp = lerp_pt(lastf, ef, f / d);
            let dst = R::xywh((cp.0 - radius as f32) as i32, (cp.1 - radius as f32) as i32, size, size);
            let clip = dst.intersect(bounds);
            if !clip.empty() {
                // build the brush area in an off-mesh buffer, then copy it onto the mesh
                for y in clip.t..clip.b {
                    let yc = (y - dst.t) as f32 - st.center;
                    for x in clip.l..clip.r {
                        let xc = (x - dst.l) as f32 - st.center;
                        let t = (xc * xc + yc * yc) * st.inv_r2;
                        if t >= 1.0 {
                            continue;
                        }
                        let n = st.density.len();
                        let strength = st.density[((t * n as f32) as usize).min(n - 1)];
                        let m0 = self.mesh.at(x, y);
                        let mut mask = m0.mask as f32 / 255.0;
                        let amount = strength * st.pressure * (1.0 - mask);
                        let mut b = m0;
                        let mut disp = (0.0f32, 0.0f32);
                        match st.mode {
                            0 => disp = (-amount * spacing as f32 * u.0, -amount * spacing as f32 * u.1),
                            1 => disp = (-amount * xc, -amount * yc),
                            2 => disp = (amount * xc, amount * yc),
                            3 => disp = (-amount * yc, amount * xc),
                            4 => disp = (amount * yc, -amount * xc),
                            5 => {
                                b.x *= 1.0 - amount;
                                b.y *= 1.0 - amount;
                            }
                            6 => mask = (mask + strength * st.pressure).min(1.0),
                            _ => mask = (mask - strength * st.pressure).max(0.0),
                        }
                        if st.mode <= 4 {
                            b = self.mesh.bilinear(x as f32 + disp.0, y as f32 + disp.1);
                        }
                        b.x += disp.0;
                        b.y += disp.1;
                        b.mask = (mask * 255.0) as u8;
                        st.buffer[((y - dst.t) * size + (x - dst.l)) as usize] = b;
                    }
                }
                for y in clip.t..clip.b {
                    let yc = (y - dst.t) as f32 - st.center;
                    for x in clip.l..clip.r {
                        let xc = (x - dst.l) as f32 - st.center;
                        if (xc * xc + yc * yc) * st.inv_r2 < 1.0 {
                            *self.mesh.at_mut(x, y) = st.buffer[((y - dst.t) * size + (x - dst.l)) as usize];
                        }
                    }
                }
            }
            f += spacing as f32;
        }
        self.remaining = f - d;
        self.last = e;
        Some(inv)
    }
}

// ----------------------------------------------------------------------------------------------
// Grid Warp (DisplacementGrid + CosineInterpolator)

/// CosineInterpolator: a sorted x -> y list (a repeated x replaces the earlier value).
struct Cosine {
    pts: Vec<(f32, f32)>,
}
impl Cosine {
    fn new() -> Self {
        Cosine { pts: Vec::new() }
    }
    fn add(&mut self, x: f32, y: f32) {
        match self.pts.binary_search_by(|p| p.0.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => self.pts[i].1 = y,
            Err(i) => self.pts.insert(i, (x, y)),
        }
    }
    fn interpolate(&self, x: f32) -> f32 {
        let p = &self.pts;
        if x < p[0].0 {
            return p[0].1;
        }
        for i in 0..p.len() - 1 {
            if p[i + 1].0 > x {
                let xf = (x - p[i].0) / (p[i + 1].0 - p[i].0);
                let yf = (1.0 - (xf * std::f32::consts::PI).cos()) / 2.0;
                return yf * p[i + 1].1 + (1.0 - yf) * p[i].1;
            }
        }
        p[p.len() - 1].1
    }
}

#[derive(Clone)]
struct Grid {
    width: i32,
    height: i32,
    hx: i32,
    hy: i32,
    xs: f32,
    ys: f32,
    delx: f32,
    dely: f32,
    /// handle offsets, in grid units, indexed [i + j * hx]
    pts: Vec<(f32, f32)>,
}
impl Grid {
    fn new(width: i32, height: i32, mw: i32, mh: i32) -> Grid {
        let (hx, hy) = (width + 1, height + 1);
        let xs = (mw - 1) as f32 / width as f32;
        let ys = (mh - 1) as f32 / height as f32;
        Grid { width, height, hx, hy, xs, ys, delx: 1.0 / xs, dely: 1.0 / ys, pts: vec![(0.0, 0.0); (hx * hy) as usize] }
    }
    #[inline]
    fn p(&self, i: i32, j: i32) -> (f32, f32) {
        self.pts[(i + j * self.hx) as usize]
    }
    /// Handle position in image pixels.
    fn handle_pos(&self, i: i32, j: i32) -> (f32, f32) {
        let p = self.p(i, j);
        ((i as f32 + p.0) * self.xs, (j as f32 + p.1) * self.ys)
    }
    /// DisplacementGrid.SetPoint: move handle (pi, pj) to offset (nx, ny), keeping it from crossing
    /// its neighbours or leaving the grid, then update the mesh around it.
    fn set_point(&mut self, mesh: &mut Mesh, pi: i32, pj: i32, mut nx: f32, mut ny: f32) -> R {
        let istart = (pi - 1).max(0);
        let iend = (pi + 2).min(self.hx);
        let jstart = (pj - 1).max(0);
        let jend = (pj + 2).min(self.hy);
        let (mut l, mut t, mut r, mut b) = (mesh.w, mesh.h, 0, 0);
        for j in jstart..jend {
            for i in istart..iend {
                let q = self.p(i, j);
                let bx = i as f32 + q.0;
                let by = j as f32 + q.1;
                let x = (bx * self.xs) as i32;
                let y = (by * self.ys) as i32;
                l = l.min(x);
                r = r.max(x);
                t = t.min(y);
                b = b.max(y);
                if i > pi && nx + pi as f32 > bx {
                    nx = bx - pi as f32 - self.delx;
                } else if i < pi && nx + (pi as f32) < bx {
                    nx = bx - pi as f32 + self.delx;
                }
                if j > pj && ny + pj as f32 > by {
                    ny = by - pj as f32 - self.dely;
                } else if j < pj && ny + (pj as f32) < by {
                    ny = by - pj as f32 + self.dely;
                }
            }
        }
        if nx + (pi as f32) < 0.0 {
            nx = -pi as f32;
        } else if nx + pi as f32 > self.width as f32 {
            nx = (self.width - pi) as f32;
        }
        if ny + (pj as f32) < 0.0 {
            ny = -pj as f32;
        } else if ny + pj as f32 > self.height as f32 {
            ny = (self.height - pj) as f32;
        }
        let k = (pi + pj * self.hx) as usize;
        self.pts[k] = (nx, ny);
        let before = R { l: l.max(0), t: t.max(0), r: r.min(mesh.w), b: b.min(mesh.h) };
        let after = self.update_point(mesh, pi, pj);
        before.union(after)
    }
    /// DisplacementGrid.UpdatePoint.
    fn update_point(&self, mesh: &mut Mesh, px: i32, py: i32) -> R {
        let istart = (px - 1).max(0);
        let iend = (px + 2).min(self.hx);
        let jstart = (py - 1).max(0);
        let jend = (py + 2).min(self.hy);
        let (mut l, mut t, mut r, mut b) = (mesh.w, mesh.h, 0, 0);
        if px == 0 {
            l = 0;
        } else if px == self.width {
            r = mesh.w;
        }
        if py == 0 {
            t = 0;
        } else if py == self.height {
            b = mesh.h;
        }
        for j in jstart..jend {
            for i in istart..iend {
                let q = self.p(i, j);
                let x = ((i as f32 + q.0) * self.xs) as i32;
                let y = ((j as f32 + q.1) * self.ys) as i32;
                l = l.min(x);
                r = r.max(x);
                t = t.min(y);
                b = b.max(y);
            }
        }
        let (l, r, t, b) = (l.max(0), r.min(mesh.w), t.max(0), b.min(mesh.h));
        self.update_vertical(mesh, py, jstart, l, r);
        self.update_horizontal(mesh, px, istart, t, b);
        R { l, t, r, b }
    }
    fn column(&self, i: i32) -> Cosine {
        let mut c = Cosine::new();
        for j in 0..self.hy {
            let q = self.p(i, j);
            c.add((j as f32 + q.1) * self.ys, q.0 * self.xs);
        }
        c
    }
    fn row(&self, j: i32) -> Cosine {
        let mut c = Cosine::new();
        for i in 0..self.hx {
            let q = self.p(i, j);
            c.add((i as f32 + q.0) * self.xs, q.1 * self.ys);
        }
        c
    }
    fn update_horizontal(&self, mesh: &mut Mesh, px: i32, istart: i32, ystart: i32, yend: i32) {
        let mut left = self.column(istart);
        let mut right = self.column(istart);
        for i in (px - 1)..(px + 1) {
            if i >= -1 && i < self.width {
                right = self.column(i + 1);
            }
            let ixl = i as f32 * self.xs;
            let ixr = (i + 1) as f32 * self.xs;
            for y in ystart..yend {
                let lv = -left.interpolate(y as f32);
                let rv = -right.interpolate(y as f32);
                let mut x1 = (ixl - lv) as i32;
                let mut x2 = (ixr - rv) as i32 + 1;
                let xdif = (x2 - x1) as f32;
                if x1 < 0 {
                    x1 = 0;
                }
                if x2 > mesh.w {
                    x2 = mesh.w;
                }
                for x in x1..x2 {
                    let k = (x - x1) as f32 / xdif;
                    mesh.at_mut(x, y).x = k * rv + (1.0 - k) * lv;
                }
            }
            left = right;
            right = Cosine { pts: left.pts.clone() };
        }
    }
    fn update_vertical(&self, mesh: &mut Mesh, py: i32, jstart: i32, xstart: i32, xend: i32) {
        let mut top = self.row(jstart);
        let mut bot = self.row(jstart);
        for j in (py - 1)..(py + 1) {
            if j >= -1 && j < self.height {
                bot = self.row(j + 1);
            }
            let jyt = j as f32 * self.ys;
            let jyb = (j + 1) as f32 * self.ys;
            for x in xstart..xend {
                let tv = -top.interpolate(x as f32);
                let bv = -bot.interpolate(x as f32);
                let mut y1 = (jyt - tv) as i32;
                let mut y2 = (jyb - bv) as i32 + 1;
                let ydif = (y2 - y1) as f32;
                if y1 < 0 {
                    y1 = 0;
                }
                if y2 > mesh.h {
                    y2 = mesh.h;
                }
                for y in y1..y2 {
                    let k = (y - y1) as f32 / ydif;
                    mesh.at_mut(x, y).y = k * bv + (1.0 - k) * tv;
                }
            }
            top = bot;
            bot = Cosine { pts: top.pts.clone() };
        }
    }
}

struct GridWarp {
    mesh: Mesh,
    grid: Grid,
    gw: i32,
    gh: i32,
    drag: Option<(i32, i32)>,
}

// ----------------------------------------------------------------------------------------------
// The dialog

enum Work {
    Smudge(Smudge),
    Liquify(Liquify),
    Grid(GridWarp),
}

#[derive(Clone)]
enum Undo {
    Img(Vec<Rgba>),
    Mesh(Mesh),
    Grid(Mesh, Grid),
}

pub enum Outcome {
    Open,
    Ok,
    Cancel,
}

pub struct State {
    pub uid: u64,
    pub idx: usize,
    pub kind: Kind,
    w: i32,
    h: i32,
    src: Vec<Rgba>,
    out: Vec<Rgba>,
    work: Work,
    undo: Vec<Undo>,
    redo: Vec<Undo>,
    tex: Option<egui::TextureHandle>,
    /// display texture scale (texture px per image px)
    ds: f32,
    dirty: Option<R>,
    down: bool,
    last_hold: f64,
    zoom: f32,
}

fn to_straight(p: &tiny_skia::Pixmap) -> Vec<Rgba> {
    p.pixels().iter().map(|c| {
        let d = c.demultiply();
        [d.red(), d.green(), d.blue(), d.alpha()]
    }).collect()
}

impl State {
    pub fn new(uid: u64, idx: usize, kind: Kind, pix: &tiny_skia::Pixmap) -> State {
        let (w, h) = (pix.width() as i32, pix.height() as i32);
        let src = to_straight(pix);
        let work = match kind {
            Kind::Smudge => Work::Smudge(Smudge {
                img: src.clone(),
                brush: 0,
                width: 30,
                strength: 0.25,
                jitter: 0.0,
                quality: 1.0,
                stroke: None,
                last: (0, 0),
                remaining: 0.0,
                rng: 0x9E37_79B9_7F4A_7C15,
            }),
            Kind::Liquify => Work::Liquify(Liquify { mesh: Mesh::new(w, h), mode: 0, size: 50, pressure: 0.25, density: 0.25, stroke: None, last: (0, 0), remaining: 0.0 }),
            Kind::GridWarp => Work::Grid(GridWarp { mesh: Mesh::new(w, h), grid: Grid::new(4, 3, w, h), gw: 4, gh: 3, drag: None }),
        };
        let ds = (2048.0 / w.max(h) as f32).min(1.0);
        State { uid, idx, kind, w, h, out: src.clone(), src, work, undo: Vec::new(), redo: Vec::new(), tex: None, ds, dirty: None, down: false, last_hold: 0.0, zoom: 1.0 }
    }

    /// The finished image (full quality), as a premultiplied pixmap.
    pub fn result(&self) -> tiny_skia::Pixmap {
        let img: Vec<Rgba> = match &self.work {
            Work::Smudge(s) => s.img.clone(),
            Work::Liquify(l) => {
                let mut o = self.src.clone();
                render_mesh_supersampled(&l.mesh, &self.src, &mut o);
                o
            }
            Work::Grid(g) => {
                let mut o = self.src.clone();
                render_mesh(&g.mesh, &self.src, &mut o, g.mesh.bounds(), false);
                o
            }
        };
        let flat: Vec<u8> = img.iter().flatten().copied().collect();
        crate::raster::pixmap_from_straight(&flat, self.w as u32, self.h as u32)
    }

    fn snapshot(&self) -> Undo {
        match &self.work {
            Work::Smudge(s) => Undo::Img(s.img.clone()),
            Work::Liquify(l) => Undo::Mesh(l.mesh.clone()),
            Work::Grid(g) => Undo::Grid(g.mesh.clone(), g.grid.clone()),
        }
    }
    fn restore(&mut self, u: Undo) {
        match (&mut self.work, u) {
            (Work::Smudge(s), Undo::Img(i)) => s.img = i,
            (Work::Liquify(l), Undo::Mesh(m)) => l.mesh = m,
            (Work::Grid(g), Undo::Grid(m, gr)) => {
                g.gw = gr.width;
                g.gh = gr.height;
                g.mesh = m;
                g.grid = gr;
            }
            _ => {}
        }
        self.full_dirty();
    }
    fn push_undo(&mut self) {
        let s = self.snapshot();
        self.undo.push(s);
        self.redo.clear();
        // keep memory bounded: about 1 GB of undo for this dialog
        let per = (self.w as usize * self.h as usize) * if matches!(self.work, Work::Smudge(_)) { 4 } else { 12 };
        let max = (1usize << 30) / per.max(1);
        let max = max.clamp(1, 50);
        while self.undo.len() > max {
            self.undo.remove(0);
        }
    }
    fn undo(&mut self) {
        if let Some(u) = self.undo.pop() {
            let cur = self.snapshot();
            self.redo.push(cur);
            self.restore(u);
        }
    }
    fn redo(&mut self) {
        if let Some(u) = self.redo.pop() {
            let cur = self.snapshot();
            self.undo.push(cur);
            self.restore(u);
        }
    }
    fn full_dirty(&mut self) {
        self.dirty = Some(R { l: 0, t: 0, r: self.w, b: self.h });
    }
    fn mark(&mut self, r: R) {
        let r = r.intersect(R { l: 0, t: 0, r: self.w, b: self.h });
        if r.empty() {
            return;
        }
        self.dirty = Some(match self.dirty {
            Some(d) => d.union(r),
            None => r,
        });
    }

    /// Re-render the dirty region into `out` and upload it to the display texture.
    fn refresh(&mut self, ctx: &egui::Context) {
        let (w, h) = (self.w, self.h);
        let (tw, th) = (((w as f32 * self.ds).round() as usize).max(1), ((h as f32 * self.ds).round() as usize).max(1));
        if self.tex.is_none() {
            self.full_dirty();
        }
        let Some(r) = self.dirty.take() else { return };
        match &self.work {
            Work::Smudge(s) => {
                for y in r.t..r.b {
                    let a = (y * w + r.l) as usize;
                    let b = (y * w + r.r) as usize;
                    self.out[a..b].copy_from_slice(&s.img[a..b]);
                }
            }
            Work::Liquify(l) => render_mesh(&l.mesh, &self.src, &mut self.out, r, true),
            Work::Grid(g) => render_mesh(&g.mesh, &self.src, &mut self.out, r, false),
        }
        // display rect in texture pixels
        let tx0 = ((r.l as f32 * self.ds).floor() as usize).min(tw - 1);
        let ty0 = ((r.t as f32 * self.ds).floor() as usize).min(th - 1);
        let tx1 = ((r.r as f32 * self.ds).ceil() as usize).clamp(tx0 + 1, tw);
        let ty1 = ((r.b as f32 * self.ds).ceil() as usize).clamp(ty0 + 1, th);
        let (pw, ph) = (tx1 - tx0, ty1 - ty0);
        let mut px = Vec::with_capacity(pw * ph);
        for ty in ty0..ty1 {
            let sy = (((ty as f32 + 0.5) / self.ds) as i32).clamp(0, h - 1);
            for tx in tx0..tx1 {
                let sx = (((tx as f32 + 0.5) / self.ds) as i32).clamp(0, w - 1);
                let p = self.out[(sy * w + sx) as usize];
                let g = if ((tx / 8) + (ty / 8)) % 2 == 0 { 255.0 } else { 204.0 };
                let a = p[3] as f32 / 255.0;
                let c = |v: u8| (v as f32 * a + g * (1.0 - a) + 0.5) as u8;
                px.push(Color32::from_rgb(c(p[0]), c(p[1]), c(p[2])));
            }
        }
        let img = egui::ColorImage { size: [pw, ph], pixels: px };
        match &mut self.tex {
            Some(t) if pw == tw && ph == th => t.set(img, egui::TextureOptions::LINEAR),
            Some(t) => t.set_partial([tx0, ty0], img, egui::TextureOptions::LINEAR),
            None => {
                if pw == tw && ph == th {
                    self.tex = Some(ctx.load_texture("interactive-fx", img, egui::TextureOptions::LINEAR));
                }
            }
        }
    }

    fn controls(&mut self, ui: &mut Ui) {
        let mut reset = false;
        let mut regrid = None;
        match &mut self.work {
            Work::Smudge(s) => {
                ui.label("Brush");
                egui::ComboBox::from_id_salt("sm_brush").selected_text(SMUDGE_BRUSHES[s.brush]).show_ui(ui, |ui| {
                    for (i, n) in SMUDGE_BRUSHES.iter().enumerate() {
                        ui.selectable_value(&mut s.brush, i, *n);
                    }
                });
                ui.label("Size");
                ui.add(egui::DragValue::new(&mut s.width).range(2..=1500).speed(1.0));
                ui.label("Strength");
                ui.add(egui::Slider::new(&mut s.strength, 0.0..=1.0).fixed_decimals(2));
                ui.label("Jitter");
                ui.add(egui::Slider::new(&mut s.jitter, 0.0..=1.0).fixed_decimals(2));
                ui.label("Quality");
                ui.add(egui::Slider::new(&mut s.quality, 0.0..=1.0).fixed_decimals(2)).on_hover_text("Dab spacing: 1 = a dab every pixel, 0 = one brush width apart");
            }
            Work::Liquify(l) => {
                ui.label("Mode");
                egui::ComboBox::from_id_salt("lq_mode").selected_text(LIQUIFY_MODES[l.mode]).show_ui(ui, |ui| {
                    for (i, n) in LIQUIFY_MODES.iter().enumerate() {
                        ui.selectable_value(&mut l.mode, i, *n);
                    }
                });
                ui.label("Size");
                ui.add(egui::DragValue::new(&mut l.size).range(2..=1500).speed(1.0));
                ui.label("Pressure");
                ui.add(egui::Slider::new(&mut l.pressure, 0.0..=1.0).fixed_decimals(2));
                ui.label("Density");
                ui.add(egui::Slider::new(&mut l.density, 0.0..=1.0).fixed_decimals(2));
                if ui.button("Reconstruct All").clicked() {
                    reset = true;
                }
            }
            Work::Grid(g) => {
                ui.label("Grid");
                let (mut gw, mut gh) = (g.gw, g.gh);
                ui.add(egui::DragValue::new(&mut gw).range(1..=100).speed(0.1));
                ui.label("x");
                ui.add(egui::DragValue::new(&mut gh).range(1..=100).speed(0.1));
                if (gw, gh) != (g.gw, g.gh) {
                    regrid = Some((gw, gh));
                }
                if ui.button("Reset").clicked() {
                    reset = true;
                }
            }
        }
        ui.separator();
        if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).on_hover_text("Ctrl+Z").clicked() {
            self.undo();
        }
        if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).on_hover_text("Ctrl+Y").clicked() {
            self.redo();
        }
        if reset || regrid.is_some() {
            self.push_undo();
            let (w, h) = (self.w, self.h);
            match &mut self.work {
                Work::Liquify(l) => l.mesh.clear_offsets(),
                Work::Grid(g) => {
                    if let Some((gw, gh)) = regrid {
                        g.gw = gw;
                        g.gh = gh;
                    }
                    g.mesh = Mesh::new(w, h);
                    g.grid = Grid::new(g.gw, g.gh, w, h);
                }
                _ => {}
            }
            self.full_dirty();
        }
    }

    fn canvas(&mut self, ui: &mut Ui, max: egui::Vec2) {
        let fit = (max.x / self.w as f32).min(max.y / self.h as f32).min(1.0);
        let vs = fit * self.zoom;
        let size = vec2(self.w as f32 * vs, self.h as f32 * vs);
        // The modal sizes itself to its widest row; make sure that is the canvas, not the toolbar.
        ui.set_min_width(size.x.min(max.x));
        egui::ScrollArea::both().max_width(max.x).max_height(max.y).auto_shrink([true, true]).show(ui, |ui| {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let p = ui.painter_at(rect);
            if let Some(t) = &self.tex {
                p.image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            let to_img = |s: Pos2| -> (i32, i32) { (((s.x - rect.min.x) / vs).floor() as i32, ((s.y - rect.min.y) / vs).floor() as i32) };
            let to_scr = |x: f32, y: f32| Pos2::new(rect.min.x + x * vs, rect.min.y + y * vs);
            let hover = resp.hover_pos();
            let ptr = resp.interact_pointer_pos().or(hover);
            let primary = ui.input(|i| i.pointer.primary_down());
            let secondary = ui.input(|i| i.pointer.secondary_down());
            let now = ui.input(|i| i.time);
            let (w, h) = (self.w, self.h);
            let pressed = resp.drag_started() || resp.is_pointer_button_down_on() && !self.down;
            // ---- input
            if pressed && (primary || secondary) {
                if let Some(pos) = ptr {
                    let ip = to_img(pos);
                    let begin = match &self.work {
                        Work::Grid(g) => {
                            let mut best = None;
                            let mut bd = 9.0f32;
                            for j in 0..g.grid.hy {
                                for i in 0..g.grid.hx {
                                    let hp = g.grid.handle_pos(i, j);
                                    let d = to_scr(hp.0, hp.1).distance(pos);
                                    if d < bd {
                                        bd = d;
                                        best = Some((i, j));
                                    }
                                }
                            }
                            best
                        }
                        _ => Some((0, 0)),
                    };
                    if let Some(hit) = begin {
                        self.push_undo();
                        self.down = true;
                        self.last_hold = now;
                        let r = match &mut self.work {
                            Work::Smudge(s) => {
                                if primary {
                                    s.down(w, h, ip);
                                }
                                None
                            }
                            Work::Liquify(l) => {
                                let m = if secondary { Liquify::alt_mode(l.mode) } else { l.mode };
                                l.down(ip, m)
                            }
                            Work::Grid(g) => {
                                g.drag = Some(hit);
                                None
                            }
                        };
                        if let Some(r) = r {
                            self.mark(r);
                        }
                    }
                }
            } else if self.down && (primary || secondary) {
                if let Some(pos) = ptr {
                    let ip = to_img(pos);
                    let moved = ui.input(|i| i.pointer.delta()) != egui::Vec2::ZERO;
                    let r = match &mut self.work {
                        Work::Smudge(s) => {
                            if moved {
                                s.moved(w, h, ip)
                            } else {
                                None
                            }
                        }
                        Work::Liquify(l) => {
                            if moved {
                                self.last_hold = now;
                                l.moved(ip)
                            } else if now - self.last_hold > 0.03 {
                                self.last_hold = now;
                                l.hold()
                            } else {
                                None
                            }
                        }
                        Work::Grid(g) => {
                            if let (Some((i, j)), true) = (g.drag, moved) {
                                let fx = (pos.x - rect.min.x) / vs;
                                let fy = (pos.y - rect.min.y) / vs;
                                let nx = fx / g.grid.xs - i as f32;
                                let ny = fy / g.grid.ys - j as f32;
                                Some(g.grid.set_point(&mut g.mesh, i, j, nx, ny))
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(r) = r {
                        self.mark(r);
                    }
                    ui.ctx().request_repaint();
                }
            } else if self.down {
                self.down = false;
                match &mut self.work {
                    Work::Smudge(s) => s.stroke = None,
                    Work::Liquify(l) => l.stroke = None,
                    Work::Grid(g) => g.drag = None,
                }
            }
            // ---- overlays
            match &self.work {
                Work::Grid(g) => {
                    let gr = &g.grid;
                    let line = Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(0, 0, 0, 160));
                    let line2 = Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(255, 255, 255, 160));
                    for j in 0..gr.hy {
                        for i in 0..gr.hx {
                            let a = gr.handle_pos(i, j);
                            let a = to_scr(a.0, a.1);
                            for (di, dj) in [(1, 0), (0, 1)] {
                                if i + di < gr.hx && j + dj < gr.hy {
                                    let b = gr.handle_pos(i + di, j + dj);
                                    let b = to_scr(b.0, b.1);
                                    p.line_segment([a + vec2(1.0, 1.0), b + vec2(1.0, 1.0)], line2);
                                    p.line_segment([a, b], line);
                                }
                            }
                        }
                    }
                    for j in 0..gr.hy {
                        for i in 0..gr.hx {
                            let a = gr.handle_pos(i, j);
                            let a = to_scr(a.0, a.1);
                            let on = g.drag == Some((i, j));
                            p.rect_filled(Rect::from_center_size(a, vec2(7.0, 7.0)), 0.0, if on { Color32::from_rgb(40, 110, 220) } else { Color32::WHITE });
                            p.rect_stroke(Rect::from_center_size(a, vec2(7.0, 7.0)), 0.0, Stroke::new(1.0f32, Color32::BLACK), egui::StrokeKind::Inside);
                        }
                    }
                }
                Work::Smudge(_) | Work::Liquify(_) => {
                    if let Some(pos) = hover {
                        let d = match &self.work {
                            Work::Smudge(s) => s.width,
                            Work::Liquify(l) => l.size,
                            _ => 0,
                        } as f32
                            * vs;
                        p.circle_stroke(pos, d / 2.0, Stroke::new(1.0f32, Color32::WHITE));
                        p.circle_stroke(pos, d / 2.0 + 1.0, Stroke::new(1.0f32, Color32::BLACK));
                    }
                }
            }
            if !matches!(self.work, Work::Grid(_)) && hover.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if resp.hovered() {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 && ui.input(|i| i.modifiers.ctrl) {
                    self.zoom = (self.zoom * if scroll > 0.0 { 1.25 } else { 0.8 }).clamp(0.1, 16.0);
                }
            }
        });
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.refresh(ctx);
        let mut out = Outcome::Open;
        let screen = ctx.screen_rect();
        let max = vec2((screen.width() - 120.0).max(200.0), (screen.height() - 200.0).max(150.0));
        egui::Modal::new("interactive-fx".into()).show(ctx, |ui| {
            ui.heading(self.kind.name());
            ui.horizontal_wrapped(|ui| self.controls(ui));
            ui.add_space(4.0);
            self.canvas(ui, max);
            ui.horizontal(|ui| {
                ui.weak(match self.kind {
                    Kind::Smudge => "Drag to smudge. Ctrl+wheel zooms.",
                    Kind::Liquify => "Drag to distort; right-drag reconstructs (or thaws/freezes). Hold still to keep bloating or twisting. Ctrl+wheel zooms.",
                    Kind::GridWarp => "Drag the handles to warp the image. Ctrl+wheel zooms.",
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_sized([80.0, 24.0], egui::Button::new("Cancel")).clicked() {
                        out = Outcome::Cancel;
                    }
                    if ui.add_sized([80.0, 24.0], egui::Button::new("OK")).clicked() {
                        out = Outcome::Ok;
                    }
                });
            });
        });
        let (z, y, esc, enter) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::Z),
                i.modifiers.command && i.key_pressed(egui::Key::Y),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter),
            )
        });
        if z {
            self.undo();
        }
        if y {
            self.redo();
        }
        if esc {
            out = Outcome::Cancel;
        }
        if enter && !ctx.wants_keyboard_input() {
            out = Outcome::Ok;
        }
        if self.dirty.is_some() {
            ctx.request_repaint();
        }
        out
    }
}

/// Scripted strokes for the gallery test.
#[cfg(test)]
pub fn demo(kind: Kind, pix: &tiny_skia::Pixmap) -> tiny_skia::Pixmap {
    let mut st = State::new(0, 0, kind, pix);
    let (w, h) = (st.w, st.h);
    match &mut st.work {
        Work::Liquify(l) => {
            l.size = 110;
            l.pressure = 0.5;
            l.down((w / 2, h / 2), 3);
            for _ in 0..60 {
                l.hold();
            }
            l.stroke = None;
            l.size = 60;
            l.down((20, h - 40), 0);
            for x in 21..90 {
                l.moved((x, h - 40 - (x - 20) / 3));
            }
            l.stroke = None;
        }
        Work::Grid(g) => {
            g.grid.set_point(&mut g.mesh, 1, 1, 0.45, 0.35);
            g.grid.set_point(&mut g.mesh, 3, 2, -0.4, -0.3);
            g.grid.set_point(&mut g.mesh, 2, 0, 0.0, 0.3);
        }
        Work::Smudge(s) => {
            s.width = 36;
            s.strength = 0.6;
            for k in 0..4 {
                let y = 30 + k * 40;
                s.down(w, h, (20, y));
                for x in 21..(w - 20) {
                    s.moved(w, h, (x, y + ((x as f32 / 15.0).sin() * 12.0) as i32));
                }
                s.stroke = None;
            }
        }
    }
    st.result()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: i32, h: i32) -> Vec<Rgba> {
        (0..w * h).map(|i| if ((i % w) / 4 + (i / w) / 4) % 2 == 0 { [255, 255, 255, 255] } else { [0, 0, 0, 255] }).collect()
    }

    #[test]
    fn liquify_push_moves_pixels_and_reconstruct_undoes() {
        let (w, h) = (64, 64);
        let src = checker(w, h);
        let mut l = Liquify { mesh: Mesh::new(w, h), mode: 0, size: 30, pressure: 0.8, density: 0.5, stroke: None, last: (0, 0), remaining: 0.0 };
        l.down((20, 32), 0);
        for x in 21..44 {
            l.moved((x, 32));
        }
        l.stroke = None;
        let moved = l.mesh.v.iter().filter(|d| d.x.abs() > 0.5).count();
        assert!(moved > 100, "push should displace the mesh ({moved})");
        // pixels in the stroke now come from behind it (negative x offsets)
        assert!(l.mesh.at(35, 32).x < 0.0);
        let mut out = src.clone();
        render_mesh(&l.mesh, &src, &mut out, l.mesh.bounds(), false);
        assert_ne!(out, src);
        for _ in 0..200 {
            l.down((32, 32), 5);
            l.stroke = None;
        }
        // full-pressure reconstruct strokes over the area pull everything back
        let mut l2 = l;
        l2.pressure = 1.0;
        l2.size = 80;
        for _ in 0..30 {
            l2.down((32, 32), 5);
            l2.stroke = None;
        }
        assert!(l2.mesh.at(35, 32).x.abs() < 0.05);
    }

    #[test]
    fn liquify_freeze_protects() {
        let (w, h) = (40, 40);
        let mut l = Liquify { mesh: Mesh::new(w, h), mode: 0, size: 40, pressure: 1.0, density: 1.0, stroke: None, last: (0, 0), remaining: 0.0 };
        l.down((20, 20), 6);
        l.stroke = None;
        assert_eq!(l.mesh.at(20, 20).mask, 255);
        l.down((10, 20), 1);
        l.moved((30, 20));
        l.stroke = None;
        assert_eq!(l.mesh.at(20, 20).x, 0.0);
    }

    #[test]
    fn smudge_drags_colour() {
        let (w, h) = (60, 20);
        let mut img: Vec<Rgba> = vec![[255, 255, 255, 255]; (w * h) as usize];
        for y in 0..h {
            for x in 0..15 {
                img[(y * w + x) as usize] = [255, 0, 0, 255];
            }
        }
        let mut s = Smudge { img, brush: 1, width: 10, strength: 0.8, jitter: 0.0, quality: 1.0, stroke: None, last: (0, 0), remaining: 0.0, rng: 1 };
        s.down(w, h, (8, 10));
        for x in 9..40 {
            s.moved(w, h, (x, 10));
        }
        let p = s.img[(10 * w + 30) as usize];
        assert!(p[1] < 250, "red should have been smudged to x=30: {p:?}");
        assert_eq!(s.img[(10 * w + 55) as usize], [255, 255, 255, 255]);
    }

    #[test]
    fn grid_warp_moves_content_with_handle() {
        let (w, h) = (101, 101);
        let mut mesh = Mesh::new(w, h);
        let mut g = Grid::new(2, 2, w, h);
        // move the centre handle right by a fifth of a cell
        let r = g.set_point(&mut mesh, 1, 1, 0.2, 0.0);
        assert!(!r.empty());
        // content follows the handle: the pixel at the handle's new spot samples from the old centre
        let d = mesh.at(60, 50);
        assert!((d.x + 10.0).abs() < 1.0, "{d:?}");
        // corners stay put
        assert_eq!(mesh.at(0, 0).x, 0.0);
        assert_eq!(mesh.at(100, 100).x, 0.0);
        // handles can't cross their neighbours
        g.set_point(&mut mesh, 1, 1, 5.0, 0.0);
        assert!(g.p(1, 1).0 < 1.0);
    }
}
