//! Headless behaviour tests: drive the tool state machine and commands without a window.

use crate::app::{App, Cmd};
use crate::canvas::Pending;
use crate::raster::{self, Col, Filter};
use crate::tools::{ShapeKind, ShapeStyle, Tool};
use egui::{pos2, Key, Modifiers, PointerButton, Pos2};

fn app(w: u32, h: u32) -> App {
    let mut a = App::with_files(vec![]);
    a.docs.clear();
    a.new_document(w, h);
    a
}

fn px(a: &App, x: u32, y: u32) -> [u8; 4] {
    let d = &a.docs[a.cur];
    let c = d.layers[d.active].pixmap.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn drag(a: &mut App, from: Pos2, to: Pos2, m: Modifiers) {
    a.tool_press(PointerButton::Primary, from, m);
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        a.tool_drag(from + (to - from) * t, m);
    }
    a.tool_release(to, m);
    a.render_pending();
}

fn shape_rect(a: &App) -> (Pos2, Pos2) {
    match &a.pending {
        Pending::Shape(s) => {
            let (x0, y0, x1, y1) = crate::paint::norm_rect(s.a, s.b);
            (pos2(x0, y0), pos2(x1, y1))
        }
        _ => panic!("no shape pending"),
    }
}

#[test]
fn shapes_corner_mode() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    a.ts.from_center = false;
    drag(&mut a, pos2(20.0, 20.0), pos2(60.0, 40.0), Modifiers::NONE);
    assert_eq!(shape_rect(&a), (pos2(20.0, 20.0), pos2(60.0, 40.0)));
}

#[test]
fn shapes_centre_mode() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    a.ts.from_center = true;
    drag(&mut a, pos2(50.0, 50.0), pos2(70.0, 60.0), Modifiers::NONE);
    assert_eq!(shape_rect(&a), (pos2(30.0, 40.0), pos2(70.0, 60.0)));
}

#[test]
fn alt_toggles_centre_mode_both_ways() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    a.ts.from_center = false;
    drag(&mut a, pos2(50.0, 50.0), pos2(70.0, 60.0), Modifiers::ALT);
    assert_eq!(shape_rect(&a), (pos2(30.0, 40.0), pos2(70.0, 60.0)));
    a.commit_pending();
    a.ts.from_center = true;
    drag(&mut a, pos2(10.0, 10.0), pos2(30.0, 20.0), Modifiers::ALT);
    assert_eq!(shape_rect(&a), (pos2(10.0, 10.0), pos2(30.0, 20.0)));
}

#[test]
fn shift_constrains_proportions() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    drag(&mut a, pos2(10.0, 10.0), pos2(50.0, 20.0), Modifiers::SHIFT);
    assert_eq!(shape_rect(&a), (pos2(10.0, 10.0), pos2(50.0, 50.0)));
}

#[test]
fn shape_draws_outline_and_commits_to_history() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    a.ts.shape = ShapeKind::Rectangle;
    a.ts.shape_style = ShapeStyle::Outline;
    a.ts.brush_width = 2.0;
    a.ts.antialias = false;
    drag(&mut a, pos2(20.0, 20.0), pos2(60.0, 60.0), Modifiers::NONE);
    // edge is primary (black), interior remains white
    assert_eq!(px(&a, 20, 40), [0, 0, 0, 255]);
    assert_eq!(px(&a, 40, 40), [255, 255, 255, 255]);
    let before = a.docs[0].undo.len();
    a.commit_pending();
    assert_eq!(a.docs[0].undo.len(), before + 1);
    assert_eq!(a.docs[0].undo.last().unwrap().name, "Shapes");
    a.docs[0].undo();
    assert_eq!(px(&a, 20, 40), [255, 255, 255, 255]);
}

#[test]
fn filled_shape_with_outline_uses_secondary_fill() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    a.ts.shape_style = ShapeStyle::FilledOutline;
    a.ts.antialias = false;
    a.primary = Col::rgba(255, 0, 0, 255);
    a.secondary = Col::rgba(0, 0, 255, 255);
    drag(&mut a, pos2(20.0, 20.0), pos2(60.0, 60.0), Modifiers::NONE);
    assert_eq!(px(&a, 40, 40), [0, 0, 255, 255]);
    assert_eq!(px(&a, 20, 40)[0], 255);
}

#[test]
fn shape_stays_editable_move_and_resize() {
    let mut a = app(200, 200);
    a.set_tool(Tool::Shapes);
    drag(&mut a, pos2(20.0, 20.0), pos2(60.0, 60.0), Modifiers::NONE);
    // drag inside moves it
    drag(&mut a, pos2(40.0, 40.0), pos2(50.0, 45.0), Modifiers::NONE);
    assert_eq!(shape_rect(&a), (pos2(30.0, 25.0), pos2(70.0, 65.0)));
    // drag bottom-right handle resizes
    drag(&mut a, pos2(70.0, 65.0), pos2(90.0, 85.0), Modifiers::NONE);
    assert_eq!(shape_rect(&a), (pos2(30.0, 25.0), pos2(90.0, 85.0)));
    // still a single history entry once committed
    let n = a.docs[0].undo.len();
    a.commit_pending();
    assert_eq!(a.docs[0].undo.len(), n + 1);
}

#[test]
fn tiny_click_does_not_create_shape() {
    let mut a = app(100, 100);
    a.set_tool(Tool::Shapes);
    drag(&mut a, pos2(20.0, 20.0), pos2(20.0, 20.0), Modifiers::NONE);
    assert!(a.pending.is_none());
    assert!(a.docs[0].undo.is_empty());
}

fn key(a: &mut App, k: Key, m: Modifiers) -> Option<Cmd> {
    a.map_shortcut(k, m, false)
}

#[test]
fn tool_letter_shortcuts_cycle_groups() {
    let mut a = app(10, 10);
    key(&mut a, Key::O, Modifiers::NONE);
    assert_eq!(a.tool, Tool::LineCurve);
    key(&mut a, Key::O, Modifiers::NONE);
    assert_eq!(a.tool, Tool::Shapes);
    key(&mut a, Key::S, Modifiers::NONE);
    assert_eq!(a.tool, Tool::RectSelect);
    key(&mut a, Key::S, Modifiers::NONE);
    assert_eq!(a.tool, Tool::LassoSelect);
    key(&mut a, Key::S, Modifiers::SHIFT);
    assert_eq!(a.tool, Tool::RectSelect);
    key(&mut a, Key::M, Modifiers::NONE);
    assert_eq!(a.tool, Tool::MovePixels);
    key(&mut a, Key::M, Modifiers::NONE);
    assert_eq!(a.tool, Tool::MoveSelection);
    key(&mut a, Key::K, Modifiers::NONE);
    assert_eq!(a.tool, Tool::ColorPicker);
}

#[test]
fn shapes_a_key_cycles_shape_type() {
    let mut a = app(10, 10);
    a.set_tool(Tool::Shapes);
    let s0 = a.ts.shape;
    key(&mut a, Key::A, Modifiers::NONE);
    assert_eq!(a.ts.shape, s0.cycle(1));
    key(&mut a, Key::A, Modifiers::SHIFT);
    assert_eq!(a.ts.shape, s0);
}

#[test]
fn brush_width_brackets() {
    let mut a = app(10, 10);
    a.ts.brush_width = 10.0;
    key(&mut a, Key::CloseBracket, Modifiers::NONE);
    assert_eq!(a.ts.brush_width, 11.0);
    key(&mut a, Key::OpenBracket, Modifiers::CTRL);
    assert_eq!(a.ts.brush_width, 6.0);
}

#[test]
fn exact_modifier_command_shortcuts() {
    let mut a = app(10, 10);
    let cs = Modifiers::CTRL | Modifiers::SHIFT;
    let ca = Modifiers::CTRL | Modifiers::ALT;
    assert_eq!(key(&mut a, Key::N, Modifiers::CTRL), Some(Cmd::New));
    assert_eq!(key(&mut a, Key::N, cs), Some(Cmd::AddLayer));
    assert_eq!(key(&mut a, Key::V, Modifiers::CTRL), Some(Cmd::Paste));
    assert_eq!(key(&mut a, Key::V, cs), Some(Cmd::PasteNewLayer));
    assert_eq!(key(&mut a, Key::V, ca), Some(Cmd::PasteNewImage));
    assert_eq!(key(&mut a, Key::S, ca), Some(Cmd::SaveAll));
    assert_eq!(key(&mut a, Key::X, cs), Some(Cmd::Crop));
    assert_eq!(key(&mut a, Key::I, ca), Some(Cmd::Filter(Filter::InvertAlpha)));
    assert_eq!(key(&mut a, Key::I, cs), Some(Cmd::Filter(Filter::InvertColors)));
    assert_eq!(key(&mut a, Key::F7, Modifiers::NONE), Some(Cmd::ToggleWindow(2)));
    assert_eq!(key(&mut a, Key::F7, cs), Some(Cmd::ResetWindow(2)));
    assert_eq!(key(&mut a, Key::Num3, Modifiers::CTRL), Some(Cmd::SwitchImage(2)));
    assert_eq!(key(&mut a, Key::PageUp, Modifiers::ALT | Modifiers::SHIFT), Some(Cmd::MoveLayer(1)));
    // X alone swaps colours
    assert_eq!(key(&mut a, Key::X, Modifiers::NONE), Some(Cmd::SwapColors));
}

#[test]
fn text_tool_typing_and_commit() {
    let mut a = app(200, 80);
    a.set_tool(Tool::Text);
    a.tool_press(PointerButton::Primary, pos2(10.0, 10.0), Modifiers::NONE);
    a.tool_release(pos2(10.0, 10.0), Modifiers::NONE);
    assert!(matches!(a.pending, Pending::Text(_)));
    a.text_key(&egui::Event::Text("Hi".into()));
    a.text_key(&egui::Event::Key { key: Key::Backspace, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    a.text_key(&egui::Event::Text("ey".into()));
    match &a.pending {
        Pending::Text(t) => assert_eq!(t.text, "Hey"),
        _ => panic!(),
    }
    a.render_pending();
    a.commit_pending();
    assert_eq!(a.docs[0].undo.last().unwrap().name, "Text");
    // some dark pixels were drawn
    let d = &a.docs[0];
    let dark = d.layers[0].pixmap.pixels().iter().filter(|p| p.red() < 128).count();
    assert!(dark > 10, "text should paint pixels, got {dark}");
}

#[test]
fn invert_colors_respects_selection_and_undo() {
    let mut a = app(20, 20);
    // select left half
    let mut m = raster::new_pixmap(20, 20);
    m.fill_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, 10.0, 20.0).unwrap(), &tiny_skia::Paint::default(), tiny_skia::Transform::identity(), None);
    let mask = tiny_skia::Mask::from_pixmap(m.as_ref(), tiny_skia::MaskType::Alpha);
    a.docs[0].set_selection(Some(mask));
    a.apply_filter_now(&Filter::InvertColors);
    assert_eq!(px(&a, 2, 5), [0, 0, 0, 255]);
    assert_eq!(px(&a, 15, 5), [255, 255, 255, 255]);
    a.docs[0].undo();
    assert_eq!(px(&a, 2, 5), [255, 255, 255, 255]);
}

#[test]
fn native_format_roundtrip_keeps_layers() {
    let mut a = app(16, 8);
    a.docs[0].add_layer();
    {
        let d = &mut a.docs[0];
        d.layers[1].name = "Ink".into();
        d.layers[1].opacity = 128;
        d.layers[1].blend = crate::document::LayerBlend::ALL[3];
        d.layers[1].pixmap.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
    }
    let dir = std::env::temp_dir().join(format!("ferrum_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("t.ferrum");
    crate::fileio::save(&a.docs[0], &p).unwrap();
    let d = crate::fileio::load(&p).unwrap();
    assert_eq!((d.width, d.height, d.layers.len()), (16, 8, 2));
    assert_eq!(d.layers[1].name, "Ink");
    assert_eq!(d.layers[1].opacity, 128);
    assert_eq!(d.layers[1].blend, crate::document::LayerBlend::ALL[3]);
    let c = d.layers[1].pixmap.pixel(3, 3).unwrap();
    assert_eq!((c.red(), c.green(), c.blue()), (10, 20, 30));
    assert!(!d.is_dirty());
    // flat export works too
    let png = dir.join("t.png");
    crate::fileio::save(&a.docs[0], &png).unwrap();
    let jpg = dir.join("t.jpg");
    crate::fileio::save(&a.docs[0], &jpg).unwrap();
    let back = crate::fileio::load(&png).unwrap();
    assert_eq!((back.width, back.height, back.layers.len()), (16, 8, 1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paintbrush_stroke_commits_last_segment() {
    let mut a = app(100, 20);
    a.set_tool(Tool::Paintbrush);
    a.ts.brush_width = 4.0;
    a.tool_press(PointerButton::Primary, pos2(10.0, 10.0), Modifiers::NONE);
    a.tool_drag(pos2(50.0, 10.0), Modifiers::NONE);
    a.tool_drag(pos2(90.0, 10.0), Modifiers::NONE);
    // release without an intermediate render: the commit must include the whole stroke
    a.tool_release(pos2(90.0, 10.0), Modifiers::NONE);
    assert!(px(&a, 85, 10)[0] < 100);
    assert_eq!(a.docs[0].undo.last().unwrap().name, "Paintbrush");
}

#[test]
fn rect_select_then_crop() {
    let mut a = app(100, 80);
    a.set_tool(Tool::RectSelect);
    drag(&mut a, pos2(10.0, 10.0), pos2(40.0, 30.0), Modifiers::NONE);
    assert_eq!(a.docs[0].sel_bounds, Some((10, 10, 40, 30)));
    a.docs[0].crop_to_selection();
    assert_eq!((a.docs[0].width, a.docs[0].height), (30, 20));
    a.docs[0].undo();
    assert_eq!((a.docs[0].width, a.docs[0].height), (100, 80));
}
