//! Smart Clone tool maths (clean-room).
//!
//! * Blend clone: a Clone Stamp whose copied pixels are colour-corrected so the stroke melts into its
//!   surroundings. The correction is the classic membrane (Laplace) interpolation of the difference
//!   between destination and source along the stroke's border, i.e. seamless cloning without seams.
//! * Content-aware heal: the stroke marks a hole, which is filled by a coarse-to-fine patch-based
//!   synthesis (PatchMatch-style nearest-neighbour search + patch voting) from the surrounding image.
use crate::paint::{self, BrushKind};
use crate::raster::Col;
use egui::{Pos2, Vec2};
use tiny_skia::{Mask, Pixmap};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SmartMode {
    Blend,
    Heal,
}
impl SmartMode {
    pub const ALL: [SmartMode; 2] = [SmartMode::Blend, SmartMode::Heal];
    pub fn name(self) -> &'static str {
        match self {
            SmartMode::Blend => "Blend clone",
            SmartMode::Heal => "Content-aware heal",
        }
    }
}

/// Stroke coverage (0..255) over its bounding box (with a small margin), clipped to the image.
pub struct Cover {
    pub x0: i32,
    pub y0: i32,
    pub w: i32,
    pub h: i32,
    pub a: Vec<u8>,
}

pub fn stroke_cover(points: &[Pos2], width: f32, aa: bool, iw: u32, ih: u32) -> Option<Cover> {
    if points.is_empty() {
        return None;
    }
    let r = width.max(1.0) / 2.0 + 3.0;
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in points {
        minx = minx.min(p.x);
        miny = miny.min(p.y);
        maxx = maxx.max(p.x);
        maxy = maxy.max(p.y);
    }
    let x0 = ((minx - r).floor() as i32).max(0);
    let y0 = ((miny - r).floor() as i32).max(0);
    let x1 = ((maxx + r).ceil() as i32).min(iw as i32);
    let y1 = ((maxy + r).ceil() as i32).min(ih as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let mut pm = Pixmap::new(w as u32, h as u32)?;
    let sh: Vec<Pos2> = points.iter().map(|p| Pos2::new(p.x - x0 as f32, p.y - y0 as f32)).collect();
    paint::draw_brush_stroke(&mut pm, None, &sh, width, aa, false, BrushKind::Paint(Col::rgba(255, 255, 255, 255)));
    let a = pm.data().chunks_exact(4).map(|c| c[3]).collect();
    Some(Cover { x0, y0, w, h, a })
}

fn clip_ok(clip: Option<&Mask>, iw: i32, x: i32, y: i32) -> bool {
    clip.map(|m| m.data()[(y * iw + x) as usize] >= 128).unwrap_or(true)
}

// ---------------------------------------------------------------------------------------------
// Blend clone

/// Draws a colour-matched clone stroke into `layer`, reading the original pixels from `base`.
pub fn blend_clone(layer: &mut Pixmap, base: &Pixmap, clip: Option<&Mask>, points: &[Pos2], width: f32, aa: bool, offset: Vec2) {
    let (iw, ih) = (base.width() as i32, base.height() as i32);
    let Some(cv) = stroke_cover(points, width, aa, base.width(), base.height()) else { return };
    let (ox, oy) = (offset.x.round() as i32, offset.y.round() as i32);
    let bd = base.data();
    let src_px = |x: i32, y: i32| -> [f32; 4] {
        let sx = (x - ox).clamp(0, iw - 1);
        let sy = (y - oy).clamp(0, ih - 1);
        let i = ((sy * iw + sx) * 4) as usize;
        [bd[i] as f32, bd[i + 1] as f32, bd[i + 2] as f32, bd[i + 3] as f32]
    };
    let (bw, bh) = (cv.w as usize, cv.h as usize);
    // difference field D = destination - source (rgb)
    let mut d = vec![[0f32; 3]; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let (gx, gy) = (cv.x0 + x as i32, cv.y0 + y as i32);
            let i = ((gy * iw + gx) * 4) as usize;
            let s = src_px(gx, gy);
            d[y * bw + x] = [bd[i] as f32 - s[0], bd[i + 1] as f32 - s[1], bd[i + 2] as f32 - s[2]];
        }
    }
    let corr = membrane(&d, &cv.a, bw, bh);
    let ld = layer.data_mut();
    for y in 0..bh {
        for x in 0..bw {
            let c = cv.a[y * bw + x];
            if c == 0 {
                continue;
            }
            let (gx, gy) = (cv.x0 + x as i32, cv.y0 + y as i32);
            if !clip_ok(clip, iw, gx, gy) {
                continue;
            }
            let s = src_px(gx, gy);
            let k = corr[y * bw + x];
            let al = s[3];
            let mut o = [0f32; 4];
            for ch in 0..3 {
                o[ch] = (s[ch] + k[ch]).clamp(0.0, al);
            }
            o[3] = al;
            let a = c as f32 / 255.0;
            let i = ((gy * iw + gx) * 4) as usize;
            for ch in 0..4 {
                let b = bd[i + ch] as f32;
                ld[i + ch] = (b + (o[ch] - b) * a).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Harmonic interpolation: inside the mask (cov > 0) the field is solved from the border values.
fn membrane(d: &[[f32; 3]], cov: &[u8], w: usize, h: usize) -> Vec<[f32; 3]> {
    let f = ((w.max(h) as f32 / 160.0).ceil() as usize).max(1);
    let (cw, ch) = (w.div_ceil(f), h.div_ceil(f));
    let mut v = vec![[0f32; 3]; cw * ch];
    let mut omega = vec![false; cw * ch];
    let (mut sum, mut cnt) = ([0f32; 3], 0f32);
    for cy in 0..ch {
        for cx in 0..cw {
            let (mut acc, mut n, mut inside) = ([0f32; 3], 0f32, false);
            for y in (cy * f)..((cy + 1) * f).min(h) {
                for x in (cx * f)..((cx + 1) * f).min(w) {
                    if cov[y * w + x] > 0 {
                        inside = true;
                    } else {
                        let p = d[y * w + x];
                        for k in 0..3 {
                            acc[k] += p[k];
                        }
                        n += 1.0;
                    }
                }
            }
            let i = cy * cw + cx;
            if inside || n == 0.0 {
                omega[i] = true;
            } else {
                v[i] = [acc[0] / n, acc[1] / n, acc[2] / n];
                for k in 0..3 {
                    sum[k] += v[i][k];
                }
                cnt += 1.0;
            }
        }
    }
    let mean = if cnt > 0.0 { [sum[0] / cnt, sum[1] / cnt, sum[2] / cnt] } else { [0.0; 3] };
    for i in 0..cw * ch {
        if omega[i] {
            v[i] = mean;
        }
    }
    let iters = (3 * cw.max(ch)).clamp(30, 500);
    let wsor = 1.7f32;
    for _ in 0..iters {
        for cy in 0..ch {
            for cx in 0..cw {
                let i = cy * cw + cx;
                if !omega[i] {
                    continue;
                }
                let l = v[cy * cw + cx.saturating_sub(1)];
                let r = v[cy * cw + (cx + 1).min(cw - 1)];
                let u = v[cy.saturating_sub(1) * cw + cx];
                let dn = v[(cy + 1).min(ch - 1) * cw + cx];
                for k in 0..3 {
                    let t = (l[k] + r[k] + u[k] + dn[k]) * 0.25;
                    v[i][k] += wsor * (t - v[i][k]);
                }
            }
        }
    }
    if f == 1 {
        return v;
    }
    // bilinear upsample
    let mut out = vec![[0f32; 3]; w * h];
    for y in 0..h {
        let fy = ((y as f32 + 0.5) / f as f32 - 0.5).clamp(0.0, (ch - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(ch - 1);
        for x in 0..w {
            let fx = ((x as f32 + 0.5) / f as f32 - 0.5).clamp(0.0, (cw - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(cw - 1);
            for k in 0..3 {
                let a = v[y0 * cw + x0][k] * (1.0 - tx) + v[y0 * cw + x1][k] * tx;
                let b = v[y1 * cw + x0][k] * (1.0 - tx) + v[y1 * cw + x1][k] * tx;
                out[y * w + x][k] = a * (1.0 - ty) + b * ty;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Content-aware heal

type Px = [f32; 4];

struct Level {
    w: i32,
    h: i32,
    img: Vec<Px>,
    hole: Vec<bool>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    fn range(&mut self, n: i32) -> i32 {
        (self.next() % n.max(1) as u32) as i32
    }
}

/// Fills the area painted by the stroke from the rest of the layer. Returns false if nothing to do.
pub fn heal(layer: &mut Pixmap, clip: Option<&Mask>, points: &[Pos2], width: f32, aa: bool) -> bool {
    let (iw, ih) = (layer.width() as i32, layer.height() as i32);
    let Some(cv) = stroke_cover(points, width, aa, layer.width(), layer.height()) else { return false };
    // hole in image coordinates (stroke dilated by one pixel)
    let mut hmin = (i32::MAX, i32::MAX);
    let mut hmax = (i32::MIN, i32::MIN);
    let mut raw = vec![false; (cv.w * cv.h) as usize];
    for y in 0..cv.h {
        for x in 0..cv.w {
            if cv.a[(y * cv.w + x) as usize] > 0 && clip_ok(clip, iw, cv.x0 + x, cv.y0 + y) {
                raw[(y * cv.w + x) as usize] = true;
                hmin = (hmin.0.min(x), hmin.1.min(y));
                hmax = (hmax.0.max(x), hmax.1.max(y));
            }
        }
    }
    if hmax.0 < hmin.0 {
        return false;
    }
    // region of interest: hole plus generous context
    let margin = ((width * 2.0) as i32).max(40);
    let rx0 = (cv.x0 + hmin.0 - margin).max(0);
    let ry0 = (cv.y0 + hmin.1 - margin).max(0);
    let rx1 = (cv.x0 + hmax.0 + 1 + margin).min(iw);
    let ry1 = (cv.y0 + hmax.1 + 1 + margin).min(ih);
    let (rw, rh) = (rx1 - rx0, ry1 - ry0);
    let mut img = vec![[0f32; 4]; (rw * rh) as usize];
    let mut hole = vec![false; (rw * rh) as usize];
    {
        let d = layer.data();
        for y in 0..rh {
            for x in 0..rw {
                let i = (((ry0 + y) * iw + rx0 + x) * 4) as usize;
                img[(y * rw + x) as usize] = [d[i] as f32 / 255.0, d[i + 1] as f32 / 255.0, d[i + 2] as f32 / 255.0, d[i + 3] as f32 / 255.0];
            }
        }
    }
    for y in 0..cv.h {
        for x in 0..cv.w {
            if raw[(y * cv.w + x) as usize] {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (gx, gy) = (cv.x0 + x + dx - rx0, cv.y0 + y + dy - ry0);
                        if gx >= 0 && gy >= 0 && gx < rw && gy < rh {
                            if clip_ok(clip, iw, cv.x0 + x + dx, cv.y0 + y + dy) {
                                hole[(gy * rw + gx) as usize] = true;
                            }
                        }
                    }
                }
            }
        }
    }
    let n_hole = hole.iter().filter(|&&b| b).count();
    if n_hole == 0 || n_hole as i32 > rw * rh - 64 {
        return false;
    }
    // pyramid
    let mut levels = vec![Level { w: rw, h: rh, img, hole }];
    while levels.len() < 4 {
        let l = levels.last().unwrap();
        let cnt = l.hole.iter().filter(|&&b| b).count();
        if cnt < 1200 || l.w < 48 || l.h < 48 {
            break;
        }
        let nl = downsample(l);
        levels.push(nl);
    }
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let top = levels.len() - 1;
    diffuse_fill(&mut levels[top]);
    for li in (0..=top).rev() {
        if li < top {
            let (a, b) = levels.split_at_mut(li + 1);
            upsample_into(&mut a[li], &b[0]);
        }
        let iters = if li == 0 { 4 } else { 5 };
        refine(&mut levels[li], iters, &mut rng);
    }
    // write back
    let fin = &levels[0];
    let d = layer.data_mut();
    for y in 0..rh {
        for x in 0..rw {
            if fin.hole[(y * rw + x) as usize] {
                let p = fin.img[(y * rw + x) as usize];
                let i = (((ry0 + y) * iw + rx0 + x) * 4) as usize;
                let a = p[3].clamp(0.0, 1.0);
                for k in 0..3 {
                    d[i + k] = (p[k].clamp(0.0, a) * 255.0).round() as u8;
                }
                d[i + 3] = (a * 255.0).round() as u8;
            }
        }
    }
    true
}

fn downsample(l: &Level) -> Level {
    let (w, h) = ((l.w + 1) / 2, (l.h + 1) / 2);
    let mut img = vec![[0f32; 4]; (w * h) as usize];
    let mut hole = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut acc, mut n, mut hl) = ([0f32; 4], 0f32, false);
            let mut acc_all = [0f32; 4];
            let mut n_all = 0f32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let (sx, sy) = ((2 * x + dx).min(l.w - 1), (2 * y + dy).min(l.h - 1));
                    let i = (sy * l.w + sx) as usize;
                    for k in 0..4 {
                        acc_all[k] += l.img[i][k];
                    }
                    n_all += 1.0;
                    if l.hole[i] {
                        hl = true;
                    } else {
                        for k in 0..4 {
                            acc[k] += l.img[i][k];
                        }
                        n += 1.0;
                    }
                }
            }
            let i = (y * w + x) as usize;
            hole[i] = hl;
            img[i] = if n > 0.0 { [acc[0] / n, acc[1] / n, acc[2] / n, acc[3] / n] } else { [acc_all[0] / n_all, acc_all[1] / n_all, acc_all[2] / n_all, acc_all[3] / n_all] };
        }
    }
    Level { w, h, img, hole }
}

/// Onion-peel then relax: a smooth starting guess for the coarsest level.
fn diffuse_fill(l: &mut Level) {
    let (w, h) = (l.w, l.h);
    let mut known: Vec<bool> = l.hole.iter().map(|&b| !b).collect();
    loop {
        let mut newly = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if known[i] {
                    continue;
                }
                let (mut acc, mut n) = ([0f32; 4], 0f32);
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < w && ny < h && known[(ny * w + nx) as usize] {
                        let p = l.img[(ny * w + nx) as usize];
                        for k in 0..4 {
                            acc[k] += p[k];
                        }
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    newly.push((i, [acc[0] / n, acc[1] / n, acc[2] / n, acc[3] / n]));
                }
            }
        }
        if newly.is_empty() {
            break;
        }
        for (i, p) in newly {
            l.img[i] = p;
            known[i] = true;
        }
    }
    for _ in 0..40 {
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if !l.hole[i] {
                    continue;
                }
                let mut acc = [0f32; 4];
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let p = l.img[((y + dy).clamp(0, h - 1) * w + (x + dx).clamp(0, w - 1)) as usize];
                    for k in 0..4 {
                        acc[k] += p[k] * 0.25;
                    }
                }
                l.img[i] = acc;
            }
        }
    }
}

fn upsample_into(fine: &mut Level, coarse: &Level) {
    for y in 0..fine.h {
        let fy = ((y as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (coarse.h - 1) as f32);
        let (y0, ty) = (fy.floor() as i32, fy.fract());
        let y1 = (y0 + 1).min(coarse.h - 1);
        for x in 0..fine.w {
            let i = (y * fine.w + x) as usize;
            if !fine.hole[i] {
                continue;
            }
            let fx = ((x as f32 + 0.5) / 2.0 - 0.5).clamp(0.0, (coarse.w - 1) as f32);
            let (x0, tx) = (fx.floor() as i32, fx.fract());
            let x1 = (x0 + 1).min(coarse.w - 1);
            let g = |xx: i32, yy: i32| coarse.img[(yy * coarse.w + xx) as usize];
            let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
            let mut o = [0f32; 4];
            for k in 0..4 {
                o[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
            }
            fine.img[i] = o;
        }
    }
}

const HALF: i32 = 2;

/// PatchMatch search + patch voting, `iters` rounds.
fn refine(l: &mut Level, iters: usize, rng: &mut Rng) {
    let (w, h) = (l.w, l.h);
    let n = (w * h) as usize;
    // valid source centres: whole patch inside the level and outside the hole
    let mut valid = vec![false; n];
    let mut vlist = Vec::new();
    for y in HALF..h - HALF {
        for x in HALF..w - HALF {
            let mut ok = true;
            'o: for dy in -HALF..=HALF {
                for dx in -HALF..=HALF {
                    if l.hole[((y + dy) * w + x + dx) as usize] {
                        ok = false;
                        break 'o;
                    }
                }
            }
            if ok {
                valid[(y * w + x) as usize] = true;
                vlist.push((x, y));
            }
        }
    }
    if vlist.is_empty() {
        return;
    }
    // active pixels: any pixel whose patch overlaps the hole
    let mut active = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let mut hit = false;
            'a: for dy in -HALF..=HALF {
                for dx in -HALF..=HALF {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < w && ny < h && l.hole[(ny * w + nx) as usize] {
                        hit = true;
                        break 'a;
                    }
                }
            }
            if hit {
                active.push((x, y));
            }
        }
    }
    let mut nnf = vec![(0i32, 0i32); n];
    let mut cost = vec![f32::MAX; n];
    for &(x, y) in &active {
        let s = vlist[rng.range(vlist.len() as i32) as usize];
        nnf[(y * w + x) as usize] = s;
    }
    let dist = |img: &[Px], px: i32, py: i32, sx: i32, sy: i32, best: f32| -> f32 {
        let mut s = 0f32;
        for dy in -HALF..=HALF {
            let ty = (py + dy).clamp(0, h - 1);
            for dx in -HALF..=HALF {
                let tx = (px + dx).clamp(0, w - 1);
                let a = img[(ty * w + tx) as usize];
                let b = img[((sy + dy) * w + sx + dx) as usize];
                for k in 0..4 {
                    let e = a[k] - b[k];
                    s += e * e;
                }
            }
            if s >= best {
                return s;
            }
        }
        s
    };
    for &(x, y) in &active {
        let i = (y * w + x) as usize;
        let (sx, sy) = nnf[i];
        cost[i] = dist(&l.img, x, y, sx, sy, f32::MAX);
    }
    let maxr = w.max(h);
    for it in 0..iters {
        let fwd = it % 2 == 0;
        let order: Vec<(i32, i32)> = if fwd { active.clone() } else { active.iter().rev().copied().collect() };
        let step = if fwd { 1 } else { -1 };
        for &(x, y) in &order {
            let i = (y * w + x) as usize;
            // propagation
            for (dx, dy) in [(-step, 0), (0, -step)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let (cx, cy) = nnf[(ny * w + nx) as usize];
                let (cx, cy) = (cx - dx, cy - dy);
                if cx >= 0 && cy >= 0 && cx < w && cy < h && valid[(cy * w + cx) as usize] {
                    let c = dist(&l.img, x, y, cx, cy, cost[i]);
                    if c < cost[i] {
                        cost[i] = c;
                        nnf[i] = (cx, cy);
                    }
                }
            }
            // random search
            let mut r = maxr;
            while r >= 1 {
                let (bx, by) = nnf[i];
                let (cx, cy) = (bx + rng.range(2 * r + 1) - r, by + rng.range(2 * r + 1) - r);
                if cx >= 0 && cy >= 0 && cx < w && cy < h && valid[(cy * w + cx) as usize] {
                    let c = dist(&l.img, x, y, cx, cy, cost[i]);
                    if c < cost[i] {
                        cost[i] = c;
                        nnf[i] = (cx, cy);
                    }
                }
                r /= 2;
            }
        }
        // vote
        let mut acc = vec![[0f32; 4]; n];
        let mut wt = vec![0f32; n];
        for &(x, y) in &active {
            let i = (y * w + x) as usize;
            let (sx, sy) = nnf[i];
            let wgt = (-cost[i] * 0.5).exp().max(1e-3);
            for dy in -HALF..=HALF {
                for dx in -HALF..=HALF {
                    let (qx, qy) = (x + dx, y + dy);
                    if qx < 0 || qy < 0 || qx >= w || qy >= h {
                        continue;
                    }
                    let q = (qy * w + qx) as usize;
                    if !l.hole[q] {
                        continue;
                    }
                    let s = l.img[((sy + dy) * w + sx + dx) as usize];
                    for k in 0..4 {
                        acc[q][k] += s[k] * wgt;
                    }
                    wt[q] += wgt;
                }
            }
        }
        for q in 0..n {
            if l.hole[q] && wt[q] > 0.0 {
                l.img[q] = [acc[q][0] / wt[q], acc[q][1] / wt[q], acc[q][2] / wt[q], acc[q][3] / wt[q]];
            }
        }
        // costs depend on the updated image
        for &(x, y) in &active {
            let i = (y * w + x) as usize;
            let (sx, sy) = nnf[i];
            cost[i] = dist(&l.img, x, y, sx, sy, f32::MAX);
        }
    }
}
