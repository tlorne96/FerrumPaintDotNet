//! Text layout: converts strings into tiny-skia paths using fonts bundled with egui
//! (Ubuntu Light and Hack), so text renders identically on every platform.

use crate::tools::{FontChoice, TextAlign};
use ab_glyph::{Font, FontRef, OutlineCurve, ScaleFont};
use std::sync::OnceLock;
use tiny_skia::{Path, PathBuilder};

fn font(choice: FontChoice) -> &'static FontRef<'static> {
    static SANS: OnceLock<FontRef<'static>> = OnceLock::new();
    static MONO: OnceLock<FontRef<'static>> = OnceLock::new();
    match choice {
        FontChoice::Sans => SANS.get_or_init(|| FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).unwrap()),
        FontChoice::Mono => MONO.get_or_init(|| FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).unwrap()),
    }
}

pub struct TextLayout {
    pub path: Option<Path>,
    pub underlines: Vec<(f32, f32, f32, f32)>,
    /// (x0, y0, x1, y1)
    pub bounds: (f32, f32, f32, f32),
    /// caret (x, top, height) for every char boundary 0..=n (in chars)
    pub carets: Vec<(f32, f32, f32)>,
    #[allow(dead_code)]
    pub line_height: f32,
}

pub struct TextParams {
    pub font: FontChoice,
    pub size: f32,
    pub italic: bool,
    pub underline: bool,
    pub align: TextAlign,
}

pub fn layout(text: &str, origin: (f32, f32), p: &TextParams) -> TextLayout {
    let f = font(p.font);
    let sf = f.as_scaled(p.size.max(1.0));
    let scale = sf.scale_factor();
    let ascent = sf.ascent();
    let descent = sf.descent();
    let line_h = sf.height() + sf.line_gap();
    let skew = if p.italic { 0.2 } else { 0.0 };

    let lines: Vec<&str> = text.split('\n').collect();
    // measure widths
    let widths: Vec<f32> = lines
        .iter()
        .map(|line| {
            let mut w = 0.0;
            let mut prev = None;
            for ch in line.chars() {
                let id = f.glyph_id(ch);
                if let Some(pr) = prev {
                    w += sf.kern(pr, id);
                }
                w += sf.h_advance(id);
                prev = Some(id);
            }
            w
        })
        .collect();
    let max_w = widths.iter().cloned().fold(0.0, f32::max);

    let mut pb = PathBuilder::new();
    let mut any = false;
    let mut carets = Vec::new();
    let mut underlines = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        let top = origin.1 + li as f32 * line_h;
        let baseline = top + ascent;
        let x_start = origin.0
            + match p.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (max_w - widths[li]) / 2.0,
                TextAlign::Right => max_w - widths[li],
            };
        let mut x = x_start;
        let mut prev = None;
        for ch in line.chars() {
            carets.push((x, top, line_h));
            let id = f.glyph_id(ch);
            if let Some(pr) = prev {
                x += sf.kern(pr, id);
            }
            if let Some(outline) = f.outline(id) {
                let map = |pt: ab_glyph::Point| -> (f32, f32) {
                    let gx = pt.x * scale.horizontal;
                    let gy = -pt.y * scale.vertical; // font units are y-up
                    (x + gx - gy * skew, baseline + gy)
                };
                let mut last: Option<(f32, f32)> = None;
                for c in &outline.curves {
                    let (p0, rest): ((f32, f32), Vec<(f32, f32)>) = match c {
                        OutlineCurve::Line(a, b) => (map(*a), vec![map(*b)]),
                        OutlineCurve::Quad(a, b, c2) => (map(*a), vec![map(*b), map(*c2)]),
                        OutlineCurve::Cubic(a, b, c2, d) => (map(*a), vec![map(*b), map(*c2), map(*d)]),
                    };
                    let need_move = match last {
                        Some(l) => (l.0 - p0.0).abs() > 0.001 || (l.1 - p0.1).abs() > 0.001,
                        None => true,
                    };
                    if need_move {
                        if last.is_some() {
                            pb.close();
                        }
                        pb.move_to(p0.0, p0.1);
                    }
                    match rest.len() {
                        1 => pb.line_to(rest[0].0, rest[0].1),
                        2 => pb.quad_to(rest[0].0, rest[0].1, rest[1].0, rest[1].1),
                        _ => pb.cubic_to(rest[0].0, rest[0].1, rest[1].0, rest[1].1, rest[2].0, rest[2].1),
                    }
                    last = rest.last().copied();
                    any = true;
                }
                if last.is_some() {
                    pb.close();
                }
            }
            x += sf.h_advance(id);
            prev = Some(id);
        }
        carets.push((x, top, line_h));
        if p.underline && widths[li] > 0.0 {
            let th = (p.size / 16.0).max(1.0);
            let uy = baseline + (-descent) * 0.4;
            underlines.push((x_start, uy, x_start + widths[li], uy + th));
        }
    }
    // Each line's end caret is at the newline position; carets now has one entry per char plus
    // one per line end. Chars include '\n', which maps to the line-end caret, so the vector
    // has exactly n_chars + 1 entries.
    let total_h = lines.len() as f32 * line_h;
    TextLayout {
        path: if any { pb.finish() } else { None },
        underlines,
        bounds: (origin.0, origin.1, origin.0 + max_w.max(p.size * 0.3), origin.1 + total_h),
        carets,
        line_height: line_h,
    }
}
