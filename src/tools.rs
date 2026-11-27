//! Tool definitions, keyboard groups and shared tool settings.

use crate::raster::SelMode;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tool {
    RectSelect,
    MovePixels,
    LassoSelect,
    MoveSelection,
    EllipseSelect,
    Zoom,
    MagicWand,
    Pan,
    PaintBucket,
    Gradient,
    Paintbrush,
    Eraser,
    Pencil,
    ColorPicker,
    CloneStamp,
    SmartClone,
    Recolor,
    Text,
    LineCurve,
    Shapes,
}

/// Tools whose points snap to the grid when Snap to Grid is on (freehand tools don't).
pub fn snaps(t: Tool) -> bool {
    matches!(t, Tool::RectSelect | Tool::EllipseSelect | Tool::MovePixels | Tool::MoveSelection | Tool::Gradient | Tool::Text | Tool::LineCurve | Tool::Shapes)
}

impl Tool {
    /// Order shown in the Tools window (two columns).
    pub const ALL: [Tool; 20] = [
        Tool::RectSelect,
        Tool::MovePixels,
        Tool::LassoSelect,
        Tool::MoveSelection,
        Tool::EllipseSelect,
        Tool::Zoom,
        Tool::MagicWand,
        Tool::Pan,
        Tool::PaintBucket,
        Tool::Gradient,
        Tool::Paintbrush,
        Tool::Eraser,
        Tool::Pencil,
        Tool::ColorPicker,
        Tool::CloneStamp,
        Tool::SmartClone,
        Tool::Recolor,
        Tool::Text,
        Tool::LineCurve,
        Tool::Shapes,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::RectSelect => "Rectangle Select",
            Tool::MovePixels => "Move Selected Pixels",
            Tool::LassoSelect => "Lasso Select",
            Tool::MoveSelection => "Move Selection",
            Tool::EllipseSelect => "Ellipse Select",
            Tool::Zoom => "Zoom",
            Tool::MagicWand => "Magic Wand",
            Tool::Pan => "Pan",
            Tool::PaintBucket => "Paint Bucket",
            Tool::Gradient => "Gradient",
            Tool::Paintbrush => "Paintbrush",
            Tool::Eraser => "Eraser",
            Tool::Pencil => "Pencil",
            Tool::ColorPicker => "Color Picker",
            Tool::CloneStamp => "Clone Stamp",
            Tool::SmartClone => "Smart Clone",
            Tool::Recolor => "Recolor",
            Tool::Text => "Text",
            Tool::LineCurve => "Line / Curve",
            Tool::Shapes => "Shapes",
        }
    }

    pub fn key(self) -> char {
        match self {
            Tool::RectSelect | Tool::LassoSelect | Tool::EllipseSelect | Tool::MagicWand => 'S',
            Tool::MovePixels | Tool::MoveSelection => 'M',
            Tool::Zoom => 'Z',
            Tool::Pan => 'H',
            Tool::PaintBucket => 'F',
            Tool::Gradient => 'G',
            Tool::Paintbrush => 'B',
            Tool::Eraser => 'E',
            Tool::Pencil => 'P',
            Tool::ColorPicker => 'K',
            Tool::CloneStamp | Tool::SmartClone => 'L',
            Tool::Recolor => 'R',
            Tool::Text => 'T',
            Tool::LineCurve | Tool::Shapes => 'O',
        }
    }

    /// Tools that share a key, in cycle order.
    pub fn group(key: char) -> Vec<Tool> {
        match key {
            'S' => vec![Tool::RectSelect, Tool::LassoSelect, Tool::EllipseSelect, Tool::MagicWand],
            'M' => vec![Tool::MovePixels, Tool::MoveSelection],
            'O' => vec![Tool::LineCurve, Tool::Shapes],
            _ => Tool::ALL.iter().copied().filter(|t| t.key() == key).collect(),
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::RectSelect => "Click and drag to draw a rectangular selection. Hold Shift to constrain to a square. Ctrl = add, Alt = subtract, Ctrl+right = invert, Alt+right = intersect.",
            Tool::MovePixels => "Drag the selection to move its pixels. Right-drag to rotate (Shift snaps to 15°). Hold Ctrl to move a copy. Arrow keys nudge.",
            Tool::LassoSelect => "Click and drag to draw the outline of a freeform selection.",
            Tool::MoveSelection => "Drag to move the selection outline. Right-drag to rotate. Arrow keys nudge.",
            Tool::EllipseSelect => "Click and drag to draw an elliptical selection. Hold Shift to constrain to a circle.",
            Tool::Zoom => "Left click to zoom in, right click to zoom out, drag a rectangle to zoom to it.",
            Tool::MagicWand => "Click to select an area of similar colour. Hold Shift for global mode, Ctrl to add, Alt to subtract.",
            Tool::Pan => "Click and drag to pan the image. You can also hold Space with any tool.",
            Tool::PaintBucket => "Left click to fill with the primary colour, right click for secondary. Shift = global fill, Ctrl = sample image.",
            Tool::Gradient => "Drag to draw a gradient from primary to secondary (right-drag reverses). Drag the nubs to adjust. Press Enter to finish.",
            Tool::Paintbrush => "Left click to draw with the primary colour, right click for secondary. [ and ] change brush width.",
            Tool::Eraser => "Click and drag to erase to transparency.",
            Tool::Pencil => "Left click to draw 1-pixel freehand lines with the primary colour, right click for secondary.",
            Tool::ColorPicker => "Left click to set the primary colour, right click for secondary. Hold Ctrl to sample the whole image.",
            Tool::CloneStamp => "Ctrl+click to set the source point, then drag to copy pixels from the source.",
            Tool::SmartClone => "Blend clone: Ctrl+click a source, drag to paint (colours matched). Heal: paint over a blemish and release. Mode is in the toolbar.",
            Tool::Recolor => "Left-drag to replace the secondary colour with the primary colour. Right-drag does the reverse.",
            Tool::Text => "Click to place the cursor and type. Drag the text to move it. Press Esc to finish.",
            Tool::LineCurve => "Drag to draw a line. Drag the middle nub to bend it into a curve. Alt draws from the centre. Enter finishes.",
            Tool::Shapes => "Drag to draw a shape (primary colour; right-drag for secondary). Shift keeps proportions, Alt draws from the centre. Drag inside to move, nubs to resize. Enter finishes.",
        }
    }

    #[allow(dead_code)]
    pub fn uses_brush_width(self) -> bool {
        matches!(self, Tool::Paintbrush | Tool::Eraser | Tool::CloneStamp | Tool::SmartClone | Tool::Recolor | Tool::LineCurve | Tool::Shapes)
    }

    #[allow(dead_code)]
    pub fn is_selection_tool(self) -> bool {
        matches!(self, Tool::RectSelect | Tool::LassoSelect | Tool::EllipseSelect | Tool::MagicWand)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    Octagon,
    Star,
    Arrow,
    Heart,
    Cross,
    SpeechBubble,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 14] = [
        ShapeKind::Rectangle,
        ShapeKind::RoundedRectangle,
        ShapeKind::Ellipse,
        ShapeKind::Triangle,
        ShapeKind::RightTriangle,
        ShapeKind::Diamond,
        ShapeKind::Pentagon,
        ShapeKind::Hexagon,
        ShapeKind::Octagon,
        ShapeKind::Star,
        ShapeKind::Arrow,
        ShapeKind::Heart,
        ShapeKind::Cross,
        ShapeKind::SpeechBubble,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::RoundedRectangle => "Rounded Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::RightTriangle => "Right Triangle",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Pentagon => "Pentagon",
            ShapeKind::Hexagon => "Hexagon",
            ShapeKind::Octagon => "Octagon",
            ShapeKind::Star => "Star",
            ShapeKind::Arrow => "Arrow",
            ShapeKind::Heart => "Heart",
            ShapeKind::Cross => "Cross",
            ShapeKind::SpeechBubble => "Speech Bubble",
        }
    }
    pub fn cycle(self, dir: i32) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap() as i32;
        let n = Self::ALL.len() as i32;
        Self::ALL[((i + dir).rem_euclid(n)) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShapeStyle {
    Outline,
    Filled,
    FilledOutline,
}

impl ShapeStyle {
    pub const ALL: [ShapeStyle; 3] = [ShapeStyle::Outline, ShapeStyle::Filled, ShapeStyle::FilledOutline];
    pub fn name(self) -> &'static str {
        match self {
            ShapeStyle::Outline => "Draw Shape Outline",
            ShapeStyle::Filled => "Draw Filled Shape",
            ShapeStyle::FilledOutline => "Draw Filled Shape With Outline",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DashStyle {
    Solid,
    Dash,
    Dot,
    DashDot,
}

impl DashStyle {
    pub const ALL: [DashStyle; 4] = [DashStyle::Solid, DashStyle::Dash, DashStyle::Dot, DashStyle::DashDot];
    pub fn name(self) -> &'static str {
        match self {
            DashStyle::Solid => "Solid",
            DashStyle::Dash => "Dashed",
            DashStyle::Dot => "Dotted",
            DashStyle::DashDot => "Dash-Dot",
        }
    }
    pub fn pattern(self, width: f32) -> Option<Vec<f32>> {
        let w = width.max(1.0);
        match self {
            DashStyle::Solid => None,
            DashStyle::Dash => Some(vec![w * 3.0, w * 2.0]),
            DashStyle::Dot => Some(vec![w, w]),
            DashStyle::DashDot => Some(vec![w * 3.0, w, w, w]),
        }
    }
    pub fn cycle(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap();
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineCap {
    Flat,
    Rounded,
    Arrow,
    ArrowFilled,
}

impl LineCap {
    pub const ALL: [LineCap; 4] = [LineCap::Flat, LineCap::Rounded, LineCap::Arrow, LineCap::ArrowFilled];
    pub fn name(self) -> &'static str {
        match self {
            LineCap::Flat => "Flat",
            LineCap::Rounded => "Rounded",
            LineCap::Arrow => "Arrow",
            LineCap::ArrowFilled => "Filled Arrow",
        }
    }
    pub fn cycle(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap();
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GradKind {
    Linear,
    LinearReflected,
    Diamond,
    Radial,
    Conical,
}

impl GradKind {
    pub const ALL: [GradKind; 5] =
        [GradKind::Linear, GradKind::LinearReflected, GradKind::Diamond, GradKind::Radial, GradKind::Conical];
    pub fn name(self) -> &'static str {
        match self {
            GradKind::Linear => "Linear",
            GradKind::LinearReflected => "Linear (Reflected)",
            GradKind::Diamond => "Diamond",
            GradKind::Radial => "Radial",
            GradKind::Conical => "Conical",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontChoice {
    Sans,
    Mono,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug)]
pub struct ToolSettings {
    pub brush_width: f32,
    pub antialias: bool,
    pub overwrite: bool,
    pub sel_mode: SelMode,
    pub tolerance: f32,
    pub flood_global: bool,
    pub sample_image: bool,
    pub shape: ShapeKind,
    pub shape_style: ShapeStyle,
    pub from_center: bool,
    pub corner_radius: f32,
    pub dash: DashStyle,
    pub smart_mode: crate::smart_clone::SmartMode,
    pub start_cap: LineCap,
    pub end_cap: LineCap,
    pub gradient: GradKind,
    pub grad_transparency: bool,
    pub picker_sample: u32,
    pub picker_image: bool,
    pub picker_switch_back: bool,
    pub font: FontChoice,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub align: TextAlign,
}

impl Default for ToolSettings {
    fn default() -> Self {
        ToolSettings {
            brush_width: 2.0,
            antialias: true,
            overwrite: false,
            sel_mode: SelMode::Replace,
            tolerance: 0.5,
            flood_global: false,
            sample_image: false,
            shape: ShapeKind::Rectangle,
            shape_style: ShapeStyle::Outline,
            from_center: false,
            corner_radius: 20.0,
            dash: DashStyle::Solid,
            smart_mode: crate::smart_clone::SmartMode::Blend,
            start_cap: LineCap::Flat,
            end_cap: LineCap::Flat,
            gradient: GradKind::Linear,
            grad_transparency: false,
            picker_sample: 1,
            picker_image: false,
            picker_switch_back: false,
            font: FontChoice::Sans,
            font_size: 24.0,
            bold: false,
            italic: false,
            underline: false,
            align: TextAlign::Left,
        }
    }
}

pub const BRUSH_PRESETS: [f32; 16] =
    [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 15.0, 20.0, 25.0, 50.0, 100.0, 200.0];
