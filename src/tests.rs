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

/// Every non-ASCII character used anywhere in the UI source must exist in the
/// proportional font chain, otherwise it renders as a box.
#[test]
fn all_ui_glyphs_render() {
    use ab_glyph::Font;
    let defs = crate::app::ui_fonts();
    let fam = &defs.families[&egui::FontFamily::Proportional];
    let fonts: Vec<ab_glyph::FontRef> = fam
        .iter()
        .map(|n| ab_glyph::FontRef::try_from_slice(&defs.font_data[n].font).unwrap())
        .collect();
    let sources = [
        include_str!("app.rs"), include_str!("ui.rs"), include_str!("canvas.rs"),
        include_str!("colors.rs"), include_str!("dialogs.rs"), include_str!("document.rs"),
        include_str!("fileio.rs"), include_str!("tools.rs"), include_str!("raster.rs"),
    ];
    let mut missing = std::collections::BTreeSet::new();
    for src in sources {
        for line in src.lines() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for ch in line.chars().filter(|c| !c.is_ascii()) {
                if !fonts.iter().any(|f| f.glyph_id(ch).0 != 0) {
                    missing.insert(format!("{ch} U+{:04X}", ch as u32));
                }
            }
        }
    }
    assert!(missing.is_empty(), "glyphs missing from UI fonts: {missing:?}");
}
/// Every adjustment and effect runs with its defaults, keeps the image size, honours the
/// selection, and (except for the documented no-op defaults) changes the image.
#[test]
fn all_effects_run() {
    use crate::fx::{Fx, FxState};
    let (w, h) = (64u32, 48u32);
    let mut src = tiny_skia::Pixmap::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            let a = if x < 6 { 0 } else { 255 };
            let r = (x * 4) as u8;
            let g = (y * 5) as u8;
            let b = ((x * y) % 256) as u8;
            let c = tiny_skia::ColorU8::from_rgba(r, g, if (x / 8 + y / 8) % 2 == 0 { b } else { 255 - b }, a).premultiply();
            src.pixels_mut()[(y * w + x) as usize] = c;
        }
    }
    let mut sel = tiny_skia::Mask::new(w, h).unwrap();
    for y in 10..30 {
        for x in 20..50 {
            sel.data_mut()[(y * w + x) as usize] = 255;
        }
    }
    // effects whose default settings are intentionally neutral on this test image
    let neutral = [Fx::Exposure, Fx::TemperatureTint, Fx::Straighten, Fx::RedEyeRemoval];
    let mut unchanged = vec![];
    for &k in Fx::ALL {
        let st = FxState::new(k, Col::BLACK, Col::WHITE);
        assert_eq!(st.vals.len(), k.params().len());
        let f = Filter::Fx(st);
        let out = raster::apply_filter(&f, &src, None);
        assert_eq!((out.width(), out.height()), (w, h), "{}", k.name());
        if out.data() == src.data() && !neutral.contains(&k) {
            unchanged.push(k.name());
        }
        // with a selection, pixels outside it are untouched
        let adj = raster::apply_filter(&f, &src, Some(&sel));
        let mut mixed = src.clone();
        raster::masked_mix(&src, &adj, Some(&sel), &mut mixed);
        assert_eq!(mixed.pixel(2, 2), src.pixel(2, 2), "{}", k.name());
        assert_eq!(mixed.pixel(60, 40), src.pixel(60, 40), "{}", k.name());
    }
    assert!(unchanged.is_empty(), "effects with no visible result: {unchanged:?}");
    assert_eq!(Fx::ALL.iter().filter(|k| k.is_effect()).count(), 47);
}
/// Manual benchmark: `cargo test --release -- --ignored effect_timing --nocapture`.
#[test]
#[ignore]
fn effect_timing() {
    use crate::fx::{Fx, FxState};
    let (w, h) = (2000u32, 1500u32);
    let mut src = tiny_skia::Pixmap::new(w, h).unwrap();
    for (i, p) in src.pixels_mut().iter_mut().enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        *p = tiny_skia::ColorU8::from_rgba((x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8, 255).premultiply();
    }
    for &k in Fx::ALL {
        let t = std::time::Instant::now();
        let _ = raster::apply_filter(&Filter::Fx(FxState::new(k, Col::BLACK, Col::WHITE)), &src, None);
        println!("{:>22}: {:>7.0} ms", k.name(), t.elapsed().as_secs_f64() * 1000.0);
    }
}
/// Saving a layered image to a flat format asks first and never flattens the open document;
/// saving to .ferrum writes directly.
#[test]
fn flat_save_of_layered_image_warns_and_keeps_layers() {
    let dir = std::env::temp_dir().join(format!("ferrum-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut a = app(40, 30);
    a.docs[0].add_layer();
    a.docs[0].path = Some(dir.join("pic.png"));
    assert!(!a.save(0, false));
    assert!(matches!(a.dialog, Some(crate::dialogs::Dialog::FlattenSave { .. })));
    assert_eq!(a.docs[0].layers.len(), 2);
    a.dialog = None;
    assert!(a.write_file(0, dir.join("pic.png")));
    assert_eq!(a.docs[0].layers.len(), 2, "exporting flat must not merge the open document's layers");
    a.docs[0].path = Some(dir.join("pic.ferrum"));
    assert!(a.save(0, false));
    assert!(a.dialog.is_none());
    let back = crate::fileio::load(&dir.join("pic.ferrum")).unwrap();
    assert_eq!(back.layers.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn snap_to_grid_aligns_shape_corners() {
    let mut a = app(200, 200);
    a.set_tool(Tool::Shapes);
    a.ts.from_center = false;
    a.snap = true;
    a.grid_size = 10;
    drag(&mut a, pos2(23.0, 18.0), pos2(64.0, 47.0), Modifiers::NONE);
    assert_eq!(shape_rect(&a), (pos2(20.0, 20.0), pos2(60.0, 50.0)));
    // freehand tools are not snapped
    a.set_tool(Tool::Paintbrush);
    assert_eq!(a.snap_pt(pos2(23.0, 18.0)), pos2(23.0, 18.0));
    a.snap = false;
    a.set_tool(Tool::Shapes);
    assert_eq!(a.snap_pt(pos2(23.0, 18.0)), pos2(23.0, 18.0));
}

#[test]
fn export_writes_flat_copy_and_keeps_document() {
    let mut a = app(40, 30);
    a.docs[0].add_layer();
    let dir = std::env::temp_dir().join("ferrum_export_test");
    let _ = std::fs::create_dir_all(&dir);
    let png = dir.join("out.png");
    let jpg = dir.join("out.jpg");
    assert!(a.write_export(0, png.clone(), 95));
    assert!(a.write_export(0, jpg.clone(), 60));
    assert_eq!(image::image_dimensions(&png).unwrap(), (40, 30));
    assert_eq!(image::image_dimensions(&jpg).unwrap(), (40, 30));
    let d = &a.docs[0];
    assert_eq!(d.layers.len(), 2, "export must not flatten the open document");
    assert!(d.path.is_none(), "export must not change the document's file");
    assert_eq!(a.export_paths.get(&d.uid), Some(&jpg));
}