//! Parametric adjustments and effects.
//!
//! Every effect declares its parameters (`P`) so that the dialog UI, defaults, repeat-last-effect
//! and tests are all generic. Algorithms are original implementations written from the public
//! descriptions of what each effect does; see docs/REFERENCE_NOTES.md.

mod adjust;
mod artistic;
mod blurs;
mod distort;
mod photo;
mod render;
mod stylize;

use crate::raster::{premul, unpremul, Col};
use tiny_skia::{Mask, Pixmap};

// ----------------------------------------------------------------------------------------------
// Parameters

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColDef {
    Primary,
    Secondary,
    Rgba(u8, u8, u8, u8),
}

/// Parameter specification: label, range and default.
#[derive(Clone, Copy, Debug)]
pub enum P {
    Int(&'static str, i32, i32, i32),
    Float(&'static str, f32, f32, f32),
    Angle(&'static str, f32),
    Bool(&'static str, bool),
    Choice(&'static str, &'static [&'static str], usize),
    Color(&'static str, ColDef),
    /// Centre point, stored as an offset in -1..1 of the selection bounds (0,0 = centre).
    Point(&'static str),
    /// "Randomize" button.
    Seed,
}

impl P {
    pub fn label(&self) -> &'static str {
        match *self {
            P::Int(n, ..) | P::Float(n, ..) | P::Angle(n, _) | P::Bool(n, _) | P::Choice(n, ..) | P::Color(n, _) | P::Point(n) => n,
            P::Seed => "Reseed",
        }
    }
    fn default_value(&self, primary: Col, secondary: Col) -> V {
        match *self {
            P::Int(_, _, _, d) => V::I(d),
            P::Float(_, _, _, d) => V::F(d),
            P::Angle(_, d) => V::F(d),
            P::Bool(_, d) => V::B(d),
            P::Choice(_, _, d) => V::C(d),
            P::Color(_, c) => V::Col(match c {
                ColDef::Primary => primary,
                ColDef::Secondary => secondary,
                ColDef::Rgba(r, g, b, a) => Col::rgba(r, g, b, a),
            }),
            P::Point(_) => V::Pt(0.0, 0.0),
            P::Seed => V::Seed(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum V {
    I(i32),
    F(f32),
    B(bool),
    C(usize),
    Col(Col),
    Pt(f32, f32),
    Seed(u32),
}

/// Typed accessors for parameter values.
pub struct A<'a>(pub &'a [V]);
impl A<'_> {
    pub fn i(&self, k: usize) -> i32 {
        match self.0[k] {
            V::I(v) => v,
            V::F(v) => v.round() as i32,
            V::C(v) => v as i32,
            _ => 0,
        }
    }
    pub fn f(&self, k: usize) -> f32 {
        match self.0[k] {
            V::F(v) => v,
            V::I(v) => v as f32,
            _ => 0.0,
        }
    }
    pub fn b(&self, k: usize) -> bool {
        matches!(self.0[k], V::B(true))
    }
    pub fn c(&self, k: usize) -> usize {
        match self.0[k] {
            V::C(v) => v,
            _ => 0,
        }
    }
    pub fn col(&self, k: usize) -> Col {
        match self.0[k] {
            V::Col(c) => c,
            _ => Col::BLACK,
        }
    }
    pub fn pt(&self, k: usize) -> (f32, f32) {
        match self.0[k] {
            V::Pt(x, y) => (x, y),
            _ => (0.0, 0.0),
        }
    }
    pub fn seed(&self, k: usize) -> u32 {
        match self.0[k] {
            V::Seed(s) => s,
            _ => 1,
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Registry

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Adjustments,
    Artistic,
    Blurs,
    Color,
    Distort,
    Noise,
    Object,
    Photo,
    Render,
    Stylize,
}

impl Menu {
    pub const EFFECT_MENUS: [Menu; 9] =
        [Menu::Artistic, Menu::Blurs, Menu::Color, Menu::Distort, Menu::Noise, Menu::Object, Menu::Photo, Menu::Render, Menu::Stylize];
    pub fn name(self) -> &'static str {
        match self {
            Menu::Adjustments => "Adjustments",
            Menu::Artistic => "Artistic",
            Menu::Blurs => "Blurs",
            Menu::Color => "Color",
            Menu::Distort => "Distort",
            Menu::Noise => "Noise",
            Menu::Object => "Object",
            Menu::Photo => "Photo",
            Menu::Render => "Render",
            Menu::Stylize => "Stylize",
        }
    }
}

macro_rules! effects {
    ($($v:ident => $name:literal, $menu:ident;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Fx { $($v),* }
        impl Fx {
            pub const ALL: &'static [Fx] = &[$(Fx::$v),*];
            pub fn name(self) -> &'static str { match self { $(Fx::$v => $name),* } }
            pub fn menu(self) -> Menu { match self { $(Fx::$v => Menu::$menu),* } }
        }
    };
}

effects! {
    Exposure => "Exposure", Adjustments;
    HighlightsShadows => "Highlights / Shadows", Adjustments;
    Sepia => "Sepia", Adjustments;
    TemperatureTint => "Temperature and Tint", Adjustments;

    InkSketch => "Ink Sketch", Artistic;
    Linocut => "Linocut", Artistic;
    Mosaic => "Mosaic", Artistic;
    OilPainting => "Oil Painting", Artistic;
    PencilSketch => "Pencil Sketch", Artistic;
    Pointillism => "Pointillism", Artistic;
    StainedGlass => "Stained Glass", Artistic;

    BokehBlur => "Bokeh Blur", Blurs;
    FragmentBlur => "Fragment Blur", Blurs;
    GaussianBlur => "Gaussian Blur", Blurs;
    MedianBlur => "Median Blur", Blurs;
    MotionBlur => "Motion Blur", Blurs;
    RadialBlur => "Radial Blur", Blurs;
    SketchBlur => "Sketch Blur", Blurs;
    SquareBlur => "Square Blur", Blurs;
    SurfaceBlur => "Surface Blur", Blurs;
    ZoomBlur => "Zoom Blur", Blurs;

    Quantize => "Quantize", Color;

    Bulge => "Bulge", Distort;
    Crystalize => "Crystalize", Distort;
    Dents => "Dents", Distort;
    FrostedGlass => "Frosted Glass", Distort;
    Morphology => "Morphology", Distort;
    Pixelate => "Pixelate", Distort;
    PolarInversion => "Polar Inversion", Distort;
    Spherize => "Spherize", Distort;
    TileReflection => "Tile Reflection", Distort;
    Twist => "Twist", Distort;
    Waves => "Waves", Distort;

    AddNoise => "Add Noise", Noise;
    ReduceNoise => "Reduce Noise", Noise;

    DropShadow => "Drop Shadow", Object;

    Glow => "Glow", Photo;
    RedEyeRemoval => "Red Eye Removal", Photo;
    Sharpen => "Sharpen", Photo;
    SoftenPortrait => "Soften Portrait", Photo;
    Straighten => "Straighten", Photo;
    Vignette => "Vignette", Photo;

    Clouds => "Clouds", Render;
    JuliaFractal => "Julia Fractal", Render;
    MandelbrotFractal => "Mandelbrot Fractal", Render;
    Turbulence => "Turbulence", Render;

    EdgeDetect => "Edge Detect", Stylize;
    Emboss => "Emboss", Stylize;
    Halftone => "Halftone", Stylize;
    Outline => "Outline", Stylize;
    Relief => "Relief", Stylize;
}

pub const EDGE: &[&str] = &["Clamp", "Reflect", "Wrap", "Transparent"];
pub const SAMPLING: &[&str] = &["Bicubic", "Bilinear", "Nearest Neighbor"];
pub const BLEND: &[&str] = &[
    "Normal", "Multiply", "Additive", "Color Burn", "Color Dodge", "Reflect", "Glow", "Overlay", "Difference", "Negation", "Lighten", "Darken",
    "Screen", "Xor",
];

impl Fx {
    pub fn is_effect(self) -> bool {
        self.menu() != Menu::Adjustments
    }
    pub fn params(self) -> Vec<P> {
        use P::*;
        match self {
            Fx::Exposure => vec![Float("Exposure", -5.0, 5.0, 0.0)],
            Fx::HighlightsShadows => vec![Int("Shadows", 0, 100, 25), Int("Highlights", 0, 100, 0), Int("Radius", 1, 100, 20)],
            Fx::Sepia => vec![Int("Intensity", 0, 100, 100)],
            Fx::TemperatureTint => vec![Int("Temperature", -100, 100, 0), Int("Tint", -100, 100, 0)],

            Fx::InkSketch => vec![Int("Ink outline", 0, 99, 50), Int("Coloring", 0, 100, 50)],
            Fx::Linocut => vec![
                Choice("Line mode", &["Parallel", "Contour", "Cross-hatch"], 0),
                Angle("Angle", 45.0),
                Int("Line spacing", 2, 64, 8),
                Int("Contrast", 0, 100, 50),
                Float("Gamma", 0.2, 5.0, 1.0),
                Int("Roughness", 0, 100, 20),
                Seed,
                Color("Ink", ColDef::Primary),
                Color("Paper", ColDef::Secondary),
            ],
            Fx::Mosaic => vec![
                Choice("Tile shape", artistic::TILE_SHAPES, 0),
                Int("Tile size", 4, 200, 16),
                Int("Bevel depth", 0, 100, 30),
                Int("Grout width", 0, 20, 1),
                Color("Grout color", ColDef::Rgba(70, 70, 70, 255)),
                Choice("Tile color", &["Average", "Center pixel"], 0),
                Angle("Light angle", 135.0),
                Int("Lightness jitter", 0, 100, 10),
                Int("Position jitter", 0, 100, 0),
                Seed,
            ],
            Fx::OilPainting => vec![Int("Brush size", 1, 8, 3), Int("Coarseness", 3, 255, 50)],
            Fx::PencilSketch => vec![Int("Pencil tip size", 1, 20, 2), Int("Range", -20, 20, 0)],
            Fx::Pointillism => vec![
                Int("Cell size", 2, 64, 8),
                Int("Cell fill", 0, 100, 80),
                Int("Tone response", 0, 100, 50),
                Int("Placement jitter", 0, 100, 50),
                Int("Color jitter", 0, 100, 20),
                Color("Background color", ColDef::Rgba(255, 255, 255, 255)),
                Seed,
            ],
            Fx::StainedGlass => vec![
                Int("Cell size", 4, 200, 24),
                Choice("Cell color", &["Average", "Center pixel"], 0),
                Int("Border width", 0, 20, 2),
                Int("Bevel depth", 0, 100, 30),
                Angle("Light angle", 135.0),
                Color("Lead color", ColDef::Rgba(20, 20, 20, 255)),
                Seed,
            ],

            Fx::BokehBlur => vec![Int("Radius", 1, 100, 6), Float("Gamma boost", 1.0, 8.0, 3.0), Int("Quality", 1, 5, 3)],
            Fx::FragmentBlur => vec![Int("Fragment count", 2, 50, 4), Int("Distance", 0, 100, 8), Angle("Rotation", 0.0)],
            Fx::GaussianBlur => vec![Int("Radius", 0, 200, 2), Float("Gamma boost", 0.2, 4.0, 1.0), Int("Quality", 1, 5, 3)],
            Fx::MedianBlur => vec![Int("Radius", 1, 100, 10), Int("Percentile", 0, 100, 50), Int("Quality", 1, 5, 4)],
            Fx::MotionBlur => vec![Angle("Angle", 25.0), Int("Distance", 1, 200, 10), Bool("Centered", true), Choice("Edge behavior", EDGE, 0)],
            Fx::RadialBlur => vec![Float("Angle", 0.0, 360.0, 2.0), Point("Center"), Int("Quality", 1, 5, 2)],
            Fx::SketchBlur => vec![Int("Radius", 1, 100, 10), Int("Percentile", 0, 100, 50), Int("Smoothness", 1, 10, 4), Seed],
            Fx::SquareBlur => vec![Int("Radius", 1, 200, 4), Float("Gamma boost", 0.2, 4.0, 1.0)],
            Fx::SurfaceBlur => vec![Int("Radius", 1, 100, 6), Int("Threshold", 1, 100, 15)],
            Fx::ZoomBlur => vec![Int("Distance", 0, 100, 10), Int("Focus", 0, 100, 0), Point("Center"), Int("Quality", 1, 5, 2)],

            Fx::Quantize => vec![Int("Colors", 2, 256, 256), Choice("Algorithm", &["Median Cut", "Octree"], 1), Int("Dithering", 0, 8, 8), Bool("Preserve transparency", true)],

            Fx::Bulge => vec![Int("Bulge", -200, 100, 45), Point("Center"), Choice("Edge behavior", EDGE, 0), Int("Quality", 1, 5, 2)],
            Fx::Crystalize => vec![Int("Cell size", 2, 250, 8), Int("Quality", 1, 5, 2), Seed],
            Fx::Dents => vec![
                Int("Scale", 1, 200, 25),
                Int("Refraction", 0, 200, 50),
                Int("Detail", 0, 100, 50),
                Int("Turbulence", 0, 100, 10),
                Angle("Angle", 0.0),
                Choice("Edge behavior", EDGE, 1),
                Int("Quality", 1, 5, 2),
                Seed,
            ],
            Fx::FrostedGlass => vec![Int("Maximum scatter radius", 0, 200, 3), Int("Minimum scatter radius", 0, 200, 0), Int("Diffusion", 0, 100, 50), Int("Smoothness", 1, 10, 2), Seed],
            Fx::Morphology => vec![Choice("Mode", &["Erode", "Dilate"], 1), Int("Width", 1, 100, 2), Int("Height", 1, 100, 2), Bool("Linked", true)],
            Fx::Pixelate => vec![Int("Cell size", 1, 100, 8), Choice("Scale down", &["Average", "Center pixel"], 0), Choice("Scale up", &["Nearest Neighbor", "Bilinear"], 0)],
            Fx::PolarInversion => vec![Float("Scale", -4.0, 4.0, 1.0), Point("Offset"), Choice("Edge behavior", EDGE, 1), Int("Quality", 1, 5, 2)],
            Fx::Spherize => vec![Int("Amount", -100, 100, 50), Float("Radius", 0.1, 2.0, 1.0), Point("Center"), Choice("Edge behavior", EDGE, 0), Int("Quality", 1, 5, 2)],
            Fx::TileReflection => vec![Angle("Angle", 30.0), Int("Tile size", 2, 200, 40), Int("Curvature", -100, 100, 8), Point("Offset"), Choice("Edge behavior", EDGE, 1), Int("Quality", 1, 5, 2)],
            Fx::Twist => vec![Int("Amount / direction", -200, 200, 30), Float("Size", 0.01, 2.0, 1.0), Point("Center"), Int("Quality", 1, 5, 2)],
            Fx::Waves => vec![
                Choice("Pattern", distort::WAVE_PATTERNS, 0),
                Choice("Waveform", &["Sine", "Triangle", "Square", "Sawtooth"], 0),
                Int("Wavelength", 2, 500, 40),
                Int("Amplitude", 0, 200, 10),
                Int("Lobes", 1, 32, 6),
                Int("Damping", 0, 100, 0),
                Angle("Angle", 0.0),
                Float("Phase", 0.0, 360.0, 0.0),
                Point("Center"),
                Choice("Edge behavior", EDGE, 1),
                Int("Quality", 1, 5, 2),
            ],

            Fx::AddNoise => vec![Int("Intensity", 0, 100, 64), Int("Color saturation", 0, 400, 100), Int("Coverage", 0, 100, 100), Seed],
            Fx::ReduceNoise => vec![Int("Radius", 1, 100, 6), Float("Strength", 0.0, 1.0, 0.4)],

            Fx::DropShadow => vec![
                Int("Shadow radius", 0, 100, 4),
                Int("Distance", 0, 100, 4),
                Angle("Angle", -45.0),
                Int("Opacity", 0, 255, 128),
                Color("Color", ColDef::Rgba(0, 0, 0, 255)),
                Bool("Only draw shadow", false),
            ],

            Fx::Glow => vec![Int("Radius", 1, 20, 6), Int("Brightness", -100, 100, 10), Int("Contrast", -100, 100, 10)],
            Fx::RedEyeRemoval => vec![Int("Tolerance", 0, 100, 70), Int("Strength", 0, 100, 90)],
            Fx::Sharpen => vec![Int("Amount", 1, 20, 10), Int("Threshold", 0, 255, 0)],
            Fx::SoftenPortrait => vec![Int("Softness", 0, 10, 5), Int("Lighting", -20, 20, 0), Int("Warmth", 0, 20, 10)],
            Fx::Straighten => vec![Float("Angle", -45.0, 45.0, 0.0), Choice("Sampling", SAMPLING, 0)],
            Fx::Vignette => vec![Point("Center"), Float("Radius", 0.1, 4.0, 1.0), Float("Strength", 0.0, 1.0, 1.0)],

            Fx::Clouds => vec![Int("Scale", 2, 1000, 250), Float("Roughness", 0.0, 1.0, 0.5), Choice("Blend mode", BLEND, 0), Seed, Color("Color 1", ColDef::Primary), Color("Color 2", ColDef::Secondary)],
            Fx::JuliaFractal => vec![Float("Factor", 1.0, 10.0, 4.0), Float("Zoom", 0.1, 50.0, 1.0), Angle("Angle", 0.0), Int("Quality", 1, 5, 2), Choice("Blend mode", BLEND, 0)],
            Fx::MandelbrotFractal => vec![Float("Factor", 1.0, 10.0, 1.0), Float("Zoom", 0.1, 100.0, 1.0), Angle("Angle", 0.0), Int("Quality", 1, 5, 2), Bool("Invert colors", false), Choice("Blend mode", BLEND, 0)],
            Fx::Turbulence => vec![Int("Octaves", 1, 10, 5), Int("Period", 2, 1000, 120), Float("Size", 0.1, 1.0, 0.5), Choice("Noise", &["Turbulence", "Fractal sum"], 0), Choice("Blend mode", BLEND, 0), Seed],

            Fx::EdgeDetect => vec![Float("Strength", 0.0, 4.0, 1.0), Int("Blurring", 0, 20, 0), Choice("Algorithm", &["Sobel", "Prewitt"], 0), Bool("Overlay edges", false)],
            Fx::Emboss => vec![Angle("Angle", 0.0)],
            Fx::Halftone => vec![
                Choice("Pattern", stylize::HALFTONE_PATTERNS, 0),
                Bool("Monochrome", false),
                Int("Cell size", 2, 100, 8),
                Angle("Angle", 45.0),
                Int("Black generation", 0, 100, 50),
                Int("Softness", 0, 100, 30),
                Int("Quality", 1, 5, 2),
            ],
            Fx::Outline => vec![Int("Thickness", 1, 200, 3), Int("Intensity", 0, 100, 50)],
            Fx::Relief => vec![Angle("Angle", 45.0)],
        }
    }
}

/// An effect together with its parameter values.
#[derive(Clone, Debug, PartialEq)]
pub struct FxState {
    pub kind: Fx,
    pub vals: Vec<V>,
}

impl FxState {
    pub fn new(kind: Fx, primary: Col, secondary: Col) -> Self {
        let vals = kind.params().iter().map(|p| p.default_value(primary, secondary)).collect();
        FxState { kind, vals }
    }
}

/// Selection bounds (or the whole image), used for centre points and render effects.
#[derive(Clone, Copy, Debug)]
pub struct Bx {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}
impl Bx {
    pub fn w(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn h(&self) -> f32 {
        self.y1 - self.y0
    }
    pub fn cx(&self) -> f32 {
        (self.x0 + self.x1) * 0.5
    }
    pub fn cy(&self) -> f32 {
        (self.y0 + self.y1) * 0.5
    }
    /// Convert a -1..1 offset into pixel coordinates.
    pub fn point(&self, p: (f32, f32)) -> (f32, f32) {
        (self.cx() + p.0 * self.w() * 0.5, self.cy() + p.1 * self.h() * 0.5)
    }
}

/// Apply an effect to the whole layer; the caller masks the result with the selection.
pub fn apply(st: &FxState, src: &Pixmap, mask: Option<&Mask>) -> Pixmap {
    let bx = match mask.and_then(crate::raster::mask_bounds) {
        Some((x0, y0, x1, y1)) => Bx { x0: x0 as f32, y0: y0 as f32, x1: x1 as f32, y1: y1 as f32 },
        None => Bx { x0: 0.0, y0: 0.0, x1: src.width() as f32, y1: src.height() as f32 },
    };
    let a = A(&st.vals);
    let b = Buf::from_pixmap(src);
    let out = match st.kind {
        Fx::Exposure | Fx::HighlightsShadows | Fx::Sepia | Fx::TemperatureTint => adjust::apply(st.kind, &a, &b),
        Fx::InkSketch | Fx::Linocut | Fx::Mosaic | Fx::OilPainting | Fx::PencilSketch | Fx::Pointillism | Fx::StainedGlass => {
            artistic::apply(st.kind, &a, &b, bx)
        }
        Fx::BokehBlur
        | Fx::FragmentBlur
        | Fx::GaussianBlur
        | Fx::MedianBlur
        | Fx::MotionBlur
        | Fx::RadialBlur
        | Fx::SketchBlur
        | Fx::SquareBlur
        | Fx::SurfaceBlur
        | Fx::ZoomBlur => blurs::apply(st.kind, &a, &b, bx),
        Fx::Bulge
        | Fx::Crystalize
        | Fx::Dents
        | Fx::FrostedGlass
        | Fx::Morphology
        | Fx::Pixelate
        | Fx::PolarInversion
        | Fx::Spherize
        | Fx::TileReflection
        | Fx::Twist
        | Fx::Waves => distort::apply(st.kind, &a, &b, bx),
        Fx::Quantize | Fx::AddNoise | Fx::ReduceNoise | Fx::DropShadow | Fx::Glow | Fx::RedEyeRemoval | Fx::Sharpen | Fx::SoftenPortrait | Fx::Straighten | Fx::Vignette => {
            photo::apply(st.kind, &a, &b, bx, src)
        }
        Fx::Clouds | Fx::JuliaFractal | Fx::MandelbrotFractal | Fx::Turbulence => render::apply(st.kind, &a, &b, bx),
        Fx::EdgeDetect | Fx::Emboss | Fx::Halftone | Fx::Outline | Fx::Relief => stylize::apply(st.kind, &a, &b, bx),
    };
    out.to_pixmap()
}

// ----------------------------------------------------------------------------------------------
// Float image buffer (premultiplied RGBA, 0..1)

pub type Px = [f32; 4];

#[derive(Clone)]
pub struct Buf {
    pub w: usize,
    pub h: usize,
    pub d: Vec<Px>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    Clamp,
    Reflect,
    Wrap,
    Transparent,
}
impl Edge {
    pub fn from(i: usize) -> Edge {
        [Edge::Clamp, Edge::Reflect, Edge::Wrap, Edge::Transparent][i.min(3)]
    }
}

fn fold(v: i64, n: i64, e: Edge) -> Option<usize> {
    if v >= 0 && v < n {
        return Some(v as usize);
    }
    match e {
        Edge::Clamp => Some(v.clamp(0, n - 1) as usize),
        Edge::Wrap => Some(v.rem_euclid(n) as usize),
        Edge::Reflect => {
            let p = 2 * n;
            let m = v.rem_euclid(p);
            Some(if m < n { m } else { p - 1 - m } as usize)
        }
        Edge::Transparent => None,
    }
}

impl Buf {
    pub fn new(w: usize, h: usize) -> Buf {
        Buf { w, h, d: vec![[0.0; 4]; w * h] }
    }
    pub fn from_pixmap(p: &Pixmap) -> Buf {
        let d = p.data().chunks_exact(4).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0]).collect();
        Buf { w: p.width() as usize, h: p.height() as usize, d }
    }
    pub fn to_pixmap(&self) -> Pixmap {
        let mut data = Vec::with_capacity(self.d.len() * 4);
        for p in &self.d {
            let a = p[3].clamp(0.0, 1.0);
            let q = |v: f32| (v.clamp(0.0, a) * 255.0).round() as u8;
            data.extend_from_slice(&[q(p[0]), q(p[1]), q(p[2]), (a * 255.0).round() as u8]);
        }
        crate::raster::pixmap_from_vec(data, self.w as u32, self.h as u32)
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Px {
        self.d[y * self.w + x]
    }
    #[inline]
    pub fn at(&self, x: i64, y: i64, e: Edge) -> Px {
        match (fold(x, self.w as i64, e), fold(y, self.h as i64, e)) {
            (Some(x), Some(y)) => self.d[y * self.w + x],
            _ => [0.0; 4],
        }
    }
    /// Bilinear sample where integer coordinates are pixel centres.
    pub fn sample(&self, x: f32, y: f32, e: Edge) -> Px {
        if !x.is_finite() || !y.is_finite() {
            return [0.0; 4];
        }
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i64, y0 as i64);
        let a = self.at(ix, iy, e);
        let b = self.at(ix + 1, iy, e);
        let c = self.at(ix, iy + 1, e);
        let d = self.at(ix + 1, iy + 1, e);
        let mut o = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bot = c[k] + (d[k] - c[k]) * fx;
            o[k] = top + (bot - top) * fy;
        }
        o
    }
}

/// Straight (unpremultiplied) colour from premultiplied.
#[inline]
pub fn st(p: Px) -> Px {
    if p[3] <= 1e-6 {
        [0.0, 0.0, 0.0, 0.0]
    } else {
        [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]]
    }
}
#[inline]
pub fn pm(p: Px) -> Px {
    let a = p[3].clamp(0.0, 1.0);
    [p[0].clamp(0.0, 1.0) * a, p[1].clamp(0.0, 1.0) * a, p[2].clamp(0.0, 1.0) * a, a]
}
#[inline]
pub fn luma(p: Px) -> f32 {
    0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]
}
#[inline]
pub fn lerp4(a: Px, b: Px, t: f32) -> Px {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}
#[inline]
pub fn add4(a: &mut Px, b: Px, w: f32) {
    for k in 0..4 {
        a[k] += b[k] * w;
    }
}
#[inline]
pub fn scale4(a: Px, s: f32) -> Px {
    [a[0] * s, a[1] * s, a[2] * s, a[3] * s]
}
pub fn col_px(c: Col) -> Px {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
}
#[allow(dead_code)]
pub fn px_u8(p: Px) -> [u8; 4] {
    unpremul([(p[0] * 255.0) as u8, (p[1] * 255.0) as u8, (p[2] * 255.0) as u8, (p[3] * 255.0) as u8])
}
#[allow(dead_code)]
pub fn u8_px(p: [u8; 4]) -> Px {
    let q = premul(p);
    [q[0] as f32 / 255.0, q[1] as f32 / 255.0, q[2] as f32 / 255.0, q[3] as f32 / 255.0]
}

/// Map every pixel in parallel. `f(x, y)` returns a premultiplied pixel.
pub fn par_map(w: usize, h: usize, f: impl Fn(usize, usize) -> Px + Sync) -> Buf {
    let mut out = Buf::new(w, h);
    if w == 0 || h == 0 {
        return out;
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 32);
    let rows = h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in out.d.chunks_mut(rows * w).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (i, px) in chunk.iter_mut().enumerate() {
                    let y = ci * rows + i / w;
                    *px = f(i % w, y);
                }
            });
        }
    });
    out
}

/// Map pixels of a buffer in parallel using the straight colour.
pub fn map_st(b: &Buf, f: impl Fn(Px) -> Px + Sync) -> Buf {
    par_map(b.w, b.h, |x, y| pm(f(st(b.get(x, y)))))
}

/// Supersampling offsets (q×q grid inside the pixel).
pub fn ss(q: i32) -> Vec<(f32, f32)> {
    let q = q.clamp(1, 5);
    let mut v = vec![];
    for j in 0..q {
        for i in 0..q {
            v.push(((i as f32 + 0.5) / q as f32, (j as f32 + 0.5) / q as f32));
        }
    }
    v
}

/// Generic inverse-mapping distortion. `map(px, py)` takes the output position (pixel units,
/// continuous) and returns the source position.
pub fn distort(src: &Buf, quality: i32, edge: Edge, map: impl Fn(f32, f32) -> (f32, f32) + Sync) -> Buf {
    let offs = ss(quality);
    let n = offs.len() as f32;
    par_map(src.w, src.h, |x, y| {
        let mut acc = [0.0; 4];
        for &(ox, oy) in &offs {
            let (sx, sy) = map(x as f32 + ox, y as f32 + oy);
            add4(&mut acc, src.sample(sx - 0.5, sy - 0.5, edge), 1.0);
        }
        scale4(acc, 1.0 / n)
    })
}

// ----------------------------------------------------------------------------------------------
// Random numbers and noise

#[inline]
pub fn hash(x: i32, y: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ s.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    h
}
#[inline]
pub fn rnd(x: i32, y: i32, s: u32) -> f32 {
    (hash(x, y, s) >> 8) as f32 / (1u32 << 24) as f32
}

fn grad(ix: i32, iy: i32, s: u32, x: f32, y: f32) -> f32 {
    let a = rnd(ix, iy, s) * std::f32::consts::TAU;
    a.cos() * x + a.sin() * y
}

/// 2D gradient noise in roughly -1..1.
pub fn perlin(x: f32, y: f32, s: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (ix, iy) = (x0 as i32, y0 as i32);
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v) = (fade(fx), fade(fy));
    let n00 = grad(ix, iy, s, fx, fy);
    let n10 = grad(ix + 1, iy, s, fx - 1.0, fy);
    let n01 = grad(ix, iy + 1, s, fx, fy - 1.0);
    let n11 = grad(ix + 1, iy + 1, s, fx - 1.0, fy - 1.0);
    let a = n00 + (n10 - n00) * u;
    let b = n01 + (n11 - n01) * u;
    (a + (b - a) * v) * 1.414
}

/// Fractal sum of noise. `abs` gives turbulence. Returns roughly -1..1 (0..1 for abs).
pub fn fbm(x: f32, y: f32, octaves: i32, persistence: f32, s: u32, abs: bool) -> f32 {
    let (mut amp, mut freq, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for o in 0..octaves.max(1) {
        let n = perlin(x * freq, y * freq, s.wrapping_add(o as u32 * 7919));
        sum += if abs { n.abs() } else { n } * amp;
        norm += amp;
        amp *= persistence;
        freq *= 2.0;
    }
    sum / norm.max(1e-6)
}

/// Jittered-grid Voronoi cell lookup. Returns (cell id, F1, F2, feature point).
pub fn voronoi(x: f32, y: f32, size: f32, jitter: f32, s: u32) -> ((i32, i32), f32, f32, (f32, f32)) {
    let (gx, gy) = ((x / size).floor() as i32, (y / size).floor() as i32);
    let mut best = (f32::MAX, (0, 0), (0.0, 0.0));
    let mut second = f32::MAX;
    for j in -2..=2 {
        for i in -2..=2 {
            let (cx, cy) = (gx + i, gy + j);
            let px = (cx as f32 + 0.5 + (rnd(cx, cy, s) - 0.5) * jitter) * size;
            let py = (cy as f32 + 0.5 + (rnd(cx, cy, s ^ 0x55aa) - 0.5) * jitter) * size;
            let d = ((px - x).powi(2) + (py - y).powi(2)).sqrt();
            if d < best.0 {
                second = best.0;
                best = (d, (cx, cy), (px, py));
            } else if d < second {
                second = d;
            }
        }
    }
    (best.1, best.0, second, best.2)
}

// ----------------------------------------------------------------------------------------------
// Blurs shared by several effects

/// Separable box blur (premultiplied), clamped edges.
pub fn box_blur(b: &Buf, rx: usize, ry: usize) -> Buf {
    let mut t = b.clone();
    if rx > 0 {
        t = par_rows(&t, |row, out| box_line(row, out, rx));
    }
    if ry > 0 {
        let tr = transpose(&t);
        let tr = par_rows(&tr, |row, out| box_line(row, out, ry));
        t = transpose(&tr);
    }
    t
}

fn box_line(row: &[Px], out: &mut [Px], r: usize) {
    let n = row.len();
    let inv = 1.0 / (2 * r + 1) as f32;
    let at = |i: isize| row[i.clamp(0, n as isize - 1) as usize];
    let mut acc = [0.0f32; 4];
    for i in -(r as isize)..=(r as isize) {
        add4(&mut acc, at(i), 1.0);
    }
    for x in 0..n {
        out[x] = scale4(acc, inv);
        add4(&mut acc, at(x as isize + r as isize + 1), 1.0);
        add4(&mut acc, at(x as isize - r as isize), -1.0);
    }
}

/// Run `f(row, out_row)` on every row in parallel.
pub fn par_rows(b: &Buf, f: impl Fn(&[Px], &mut [Px]) + Sync) -> Buf {
    let mut out = Buf::new(b.w, b.h);
    if b.w == 0 || b.h == 0 {
        return out;
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 32);
    let rows = b.h.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (ci, chunk) in out.d.chunks_mut(rows * b.w).enumerate() {
            let f = &f;
            let src = &b.d;
            let w = b.w;
            s.spawn(move || {
                for (ri, orow) in chunk.chunks_mut(w).enumerate() {
                    let y = ci * rows + ri;
                    f(&src[y * w..(y + 1) * w], orow);
                }
            });
        }
    });
    out
}

pub fn transpose(b: &Buf) -> Buf {
    let mut o = Buf::new(b.h, b.w);
    for y in 0..b.h {
        for x in 0..b.w {
            o.d[x * b.h + y] = b.d[y * b.w + x];
        }
    }
    o
}

/// Gaussian approximation with `passes` box blurs. Radius ~ 2 sigma.
pub fn gauss(b: &Buf, radius: f32, passes: i32) -> Buf {
    if radius < 0.5 {
        return b.clone();
    }
    let n = passes.clamp(1, 5) as f32;
    let sigma = radius / 2.0;
    let r = (((12.0 * sigma * sigma / n) + 1.0).sqrt() / 2.0).round().max(1.0) as usize;
    let mut t = b.clone();
    for _ in 0..passes.clamp(1, 5) {
        t = box_blur(&t, r, r);
    }
    t
}

/// Single-channel helper: luma plane of a buffer (straight colour, weighted by nothing).
pub fn luma_plane(b: &Buf) -> Vec<f32> {
    b.d.iter().map(|&p| luma(st(p))).collect()
}

/// Gaussian blur of a single-channel plane.
pub fn gauss_plane(v: &[f32], w: usize, h: usize, radius: f32) -> Vec<f32> {
    let b = Buf { w, h, d: v.iter().map(|&x| [x, 0.0, 0.0, 1.0]).collect() };
    gauss(&b, radius, 3).d.iter().map(|p| p[0]).collect()
}

// ----------------------------------------------------------------------------------------------
// Blend modes for render effects (straight colours)

pub fn blend(mode: usize, base: Px, top: Px) -> Px {
    let f = |b: f32, t: f32| -> f32 {
        match mode {
            1 => b * t,
            2 => (b + t).min(1.0),
            3 => {
                if t <= 0.0 {
                    0.0
                } else {
                    (1.0 - (1.0 - b) / t).max(0.0)
                }
            }
            4 => {
                if t >= 1.0 {
                    1.0
                } else {
                    (b / (1.0 - t)).min(1.0)
                }
            }
            5 => {
                if t >= 1.0 {
                    1.0
                } else {
                    (b * b / (1.0 - t)).min(1.0)
                }
            }
            6 => {
                if b >= 1.0 {
                    1.0
                } else {
                    (t * t / (1.0 - b)).min(1.0)
                }
            }
            7 => {
                if b < 0.5 {
                    2.0 * b * t
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - t)
                }
            }
            8 => (b - t).abs(),
            9 => 1.0 - (1.0 - b - t).abs(),
            10 => b.max(t),
            11 => b.min(t),
            12 => 1.0 - (1.0 - b) * (1.0 - t),
            13 => (((b * 255.0) as u8) ^ ((t * 255.0) as u8)) as f32 / 255.0,
            _ => t,
        }
    };
    if mode == 0 || base[3] <= 0.0 {
        // normal "over"
        let a = top[3] + base[3] * (1.0 - top[3]);
        if a <= 0.0 {
            return [0.0; 4];
        }
        let c = |k: usize| (top[k] * top[3] + base[k] * base[3] * (1.0 - top[3])) / a;
        return [c(0), c(1), c(2), a];
    }
    let mixed = [f(base[0], top[0]), f(base[1], top[1]), f(base[2], top[2])];
    let ta = top[3];
    let a = ta + base[3] * (1.0 - ta);
    let c = |k: usize| ((mixed[k] * ta * base[3]) + top[k] * ta * (1.0 - base[3]) + base[k] * base[3] * (1.0 - ta)) / a.max(1e-6);
    [c(0), c(1), c(2), a]
}

/// sRGB <-> linear helpers.
#[inline]
pub fn to_lin(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
#[inline]
pub fn to_srgb(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

pub fn rgb_to_hsv(c: [f32; 3]) -> (f32, f32, f32) {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = mx - mn;
    let h = if d <= 1e-6 {
        0.0
    } else if mx == c[0] {
        60.0 * ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        60.0 * ((c[2] - c[0]) / d + 2.0)
    } else {
        60.0 * ((c[0] - c[1]) / d + 4.0)
    };
    (h, if mx <= 0.0 { 0.0 } else { d / mx }, mx)
}
