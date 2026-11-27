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
    // (Trail's default direction points away from the test image's only transparent strip)
    let neutral = [Fx::Exposure, Fx::TemperatureTint, Fx::Straighten, Fx::RedEyeRemoval, Fx::ObjectTrail, Fx::ColorBalance];
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
    assert_eq!(Fx::ALL.iter().filter(|k| k.is_effect()).count(), 69);
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
/// Opens every .pdn under the folders in FERRUM_PDN_DIRS (separated by ';') and writes a
/// flattened PNG of each to the temp folder. Run with:
/// FERRUM_PDN_DIRS="C:\pics" cargo test --release -- --ignored pdn_import --nocapture
#[test]
#[ignore]
fn pdn_import_samples() {
    let dirs = std::env::var("FERRUM_PDN_DIRS").unwrap_or_default();
    let out = std::env::temp_dir().join("ferrum_pdn_out");
    let _ = std::fs::create_dir_all(&out);
    let mut n = 0;
    for dir in dirs.split(';').filter(|s| !s.is_empty()) {
        let mut files = vec![];
        let mut stack = vec![std::path::PathBuf::from(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() { stack.push(p) } else if crate::app::file_ext(&p) == "pdn" { files.push(p) }
            }
        }
        for f in files {
            let t = std::time::Instant::now();
            match crate::fileio::load(&f) {
                Ok(doc) => {
                    let layers: Vec<String> = doc.layers.iter().map(|l| format!("{} [{}{} {}]", l.name, if l.visible { "" } else { "hidden " }, l.opacity, l.blend.name())).collect();
                    println!("OK  {} {}x{} {:.2}s: {}", f.display(), doc.width, doc.height, t.elapsed().as_secs_f32(), layers.join(", "));
                    let name = f.file_stem().unwrap().to_string_lossy().to_string();
                    crate::fileio::export_flat(&doc, &out.join(format!("{name}.png")), 90).unwrap();
                    n += 1;
                }
                Err(e) => println!("ERR {}: {e}", f.display()),
            }
        }
    }
    println!("{n} files opened; PNGs in {}", out.display());
}

#[test]
fn pdn_rejects_garbage_and_custom_blends_work() {
    assert!(crate::pdn::load(b"not a pdn file", "x".into()).is_err());
    assert!(crate::pdn::load(b"PDN3\x05\x00\x00<a/>\x00\x01\x00", "x".into()).is_err());
    use crate::document::{draw_layer, LayerBlend};
    let mut dst = raster::new_pixmap(1, 1);
    dst.fill(tiny_skia::Color::from_rgba8(64, 128, 192, 255));
    let mut src = raster::new_pixmap(1, 1);
    src.fill(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
    draw_layer(&mut dst, &src, 255, LayerBlend::Negation);
    let p = dst.pixel(0, 0).unwrap();
    // negation: 1 - |1 - a - b|
    assert!((p.red() as i32 - 192).abs() <= 1 && (p.green() as i32 - 255).abs() <= 1 && (p.blue() as i32 - 191).abs() <= 2, "{p:?}");
}
#[test]
#[ignore]
fn pdn_dump() {
    let (Ok(f), Ok(out)) = (std::env::var("FERRUM_PDN_FILE"), std::env::var("FERRUM_PDN_DUMP")) else { return };
    std::fs::write(out, crate::pdn::dump(&std::fs::read(f).unwrap())).unwrap();
}
fn pdn_test_doc() -> crate::document::Document {
    use crate::document::{Document, Layer, LayerBlend};
    let mut layers = vec![];
    let modes: Vec<LayerBlend> = [0usize, 1, 2, 3, 4, 18, 19, 5, 8, 20, 10, 11, 12, 13].iter().map(|&i| LayerBlend::ALL[i]).collect();
    for (i, b) in modes.iter().enumerate() {
        let mut pm = raster::new_pixmap(300, 200);
        if i == 0 {
            pm.fill(tiny_skia::Color::from_rgba8(40, 110, 220, 255));
        } else {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8((i * 37 % 256) as u8, (255 - i * 17) as u8, (i * 90 % 256) as u8, if i % 3 == 0 { 128 } else { 255 });
            let r = tiny_skia::Rect::from_xywh((i * 18) as f32, (i * 11) as f32, 60.0, 40.0).unwrap();
            pm.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
        }
        layers.push(Layer { name: format!("{} layer \u{e9}", b.name()), pixmap: pm, visible: i != 7, opacity: if i == 4 { 161 } else { 255 }, blend: *b });
    }
    Document::from_layers("blend test", 300, 200, layers, "New Image")
}

#[test]
fn pdn_export_round_trips() {
    let doc = pdn_test_doc();
    let bytes = crate::pdn::save(&doc).unwrap();
    let back = crate::pdn::load(&bytes, "x".into()).unwrap();
    assert_eq!((back.width, back.height, back.layers.len()), (300, 200, 14));
    for (a, b) in doc.layers.iter().zip(&back.layers) {
        assert_eq!(a.name, b.name);
        assert_eq!((a.visible, a.opacity, a.blend), (b.visible, b.opacity, b.blend));
        for (p, q) in a.pixmap.data().chunks(4).zip(b.pixmap.data().chunks(4)) {
            assert_eq!(p[3], q[3]);
            for c in 0..3 {
                assert!((p[c] as i32 - q[c] as i32).abs() <= 1, "{p:?} vs {q:?}");
            }
        }
    }
    // a blend mode Paint.NET lacks is reported and saved as Normal
    let mut doc2 = pdn_test_doc();
    doc2.layers[2].blend = crate::document::LayerBlend::SoftLight;
    assert_eq!(crate::pdn::unsupported_blends(&doc2).len(), 1);
    let back2 = crate::pdn::load(&crate::pdn::save(&doc2).unwrap(), "x".into()).unwrap();
    assert_eq!(back2.layers[2].blend, crate::document::LayerBlend::Normal);
}

/// Re-saves every .pdn under FERRUM_PDN_DIRS into FERRUM_PDN_OUT (plus a 14-blend-mode test
/// file), for opening in Paint.NET.
#[test]
#[ignore]
fn pdn_resave_samples() {
    let Ok(out) = std::env::var("FERRUM_PDN_OUT") else { return };
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    crate::fileio::save(&pdn_test_doc(), &out.join("ferrum_blend_modes.pdn")).unwrap();
    for dir in std::env::var("FERRUM_PDN_DIRS").unwrap_or_default().split(';').filter(|s| !s.is_empty()) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if crate::app::file_ext(&p) != "pdn" {
                continue;
            }
            let doc = crate::fileio::load(&p).unwrap();
            let t = std::time::Instant::now();
            let dst = out.join(format!("resaved_{}", p.file_name().unwrap().to_string_lossy()));
            crate::fileio::save(&doc, &dst).unwrap();
            println!("{} -> {:.2}s, {} KB (original {} KB)", p.display(), t.elapsed().as_secs_f32(), std::fs::metadata(&dst).unwrap().len() / 1024, std::fs::metadata(&p).unwrap().len() / 1024);
        }
    }
}
#[test]
fn xor_blend_is_bitwise_like_paint_net() {
    use crate::document::{draw_layer, LayerBlend};
    let mut dst = raster::new_pixmap(1, 1);
    dst.fill(tiny_skia::Color::from_rgba8(40, 110, 220, 255));
    let mut src = raster::new_pixmap(1, 1);
    src.fill(tiny_skia::Color::from_rgba8(225, 34, 146, 255));
    draw_layer(&mut dst, &src, 255, LayerBlend::Xor);
    let p = dst.pixel(0, 0).unwrap();
    assert_eq!((p.red(), p.green(), p.blue(), p.alpha()), (201, 76, 78, 255));
}
/// Plugin-inspired effects: outline/trail only touch the transparent surroundings, the distance
/// field is exact, seeded renders are deterministic and Random Effect always yields valid values.
#[test]
fn plugin_inspired_effects() {
    use crate::fx::{random_state, Fx, FxState, V};
    let (w, h) = (40u32, 30u32);
    let mut src = tiny_skia::Pixmap::new(w, h).unwrap();
    for y in 10..20 {
        for x in 10..20 {
            src.pixels_mut()[(y * w + x) as usize] = tiny_skia::ColorU8::from_rgba(200, 40, 40, 255).premultiply();
        }
    }
    let run = |st: FxState| raster::apply_filter(&Filter::Fx(st), &src, None);
    // Outline Object: 3 px outline in the primary colour, object unchanged, far pixels untouched
    let out = run(FxState::new(Fx::OutlineObject, Col::BLACK, Col::WHITE));
    assert_eq!(out.pixel(15, 15), src.pixel(15, 15));
    let p = out.pixel(8, 15).unwrap();
    assert_eq!((p.red(), p.alpha()), (0, 255));
    assert_eq!(out.pixel(2, 2).unwrap().alpha(), 0);
    // Trail at 0 degrees, no fade, extends to the right only
    let mut st = FxState::new(Fx::ObjectTrail, Col::BLACK, Col::WHITE);
    st.vals[0] = V::I(10);
    st.vals[2] = V::F(0.0);
    st.vals[3] = V::B(false);
    let out = run(st);
    assert!(out.pixel(25, 15).unwrap().alpha() > 200);
    assert_eq!(out.pixel(5, 15).unwrap().alpha(), 0);
    assert_eq!(out.pixel(25, 5).unwrap().alpha(), 0);
    // distance field matches brute force
    let inside: Vec<bool> = (0..w * h).map(|i| (i * 7919) % 37 == 0).collect();
    let d = crate::fx::extras_distance_field(&inside, w as usize, h as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let mut best = f32::MAX;
            for (j, &on) in inside.iter().enumerate() {
                if on {
                    let (u, v) = ((j % w as usize) as f32 - x as f32, (j / w as usize) as f32 - y as f32);
                    best = best.min((u * u + v * v).sqrt());
                }
            }
            assert!((d[y * w as usize + x] - best).abs() < 1e-3, "({x},{y}) {} vs {best}", d[y * w as usize + x]);
        }
    }
    // seeded renders are deterministic
    let a1 = run(FxState::new(Fx::RandomShapes, Col::BLACK, Col::WHITE));
    let a2 = run(FxState::new(Fx::RandomShapes, Col::BLACK, Col::WHITE));
    assert_eq!(a1.data(), a2.data());
    // random effect: every value has the right type and lies in range
    for seed in 1..200u32 {
        let st = random_state(Col::BLACK, Col::WHITE, seed);
        assert!(st.kind.is_effect());
        assert_eq!(st.vals.len(), st.kind.params().len());
        for (p, v) in st.kind.params().iter().zip(&st.vals) {
            match (p, v) {
                (crate::fx::P::Int(_, lo, hi, _), V::I(x)) => assert!(x >= lo && x <= hi),
                (crate::fx::P::Float(_, lo, hi, _), V::F(x)) => assert!(x >= lo && x <= hi),
                (crate::fx::P::Choice(_, o, _), V::C(x)) => assert!(*x < o.len()),
                _ => {}
            }
        }
    }
}
/// Manual gallery: `cargo test --release -- --ignored plugin_gallery`; writes a contact sheet
/// of the plugin-inspired effects to %TEMP%/ferrum-plugin-gallery.png.
#[test]
#[ignore]
fn plugin_gallery() {
    use crate::fx::{Fx, FxState};
    let (w, h) = (240u32, 180u32);
    let photo = {
        let mut p = tiny_skia::Pixmap::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
                let r = (255.0 * (0.5 + 0.5 * (u * 9.0).sin())) as u8;
                let g = (255.0 * v) as u8;
                let b = (255.0 * (0.5 + 0.5 * ((u + v) * 6.0).cos())) as u8;
                p.pixels_mut()[(y * w + x) as usize] = tiny_skia::ColorU8::from_rgba(r, g, b, 255).premultiply();
            }
        }
        p
    };
    let object = {
        let mut p = tiny_skia::Pixmap::new(w, h).unwrap();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(230, 120, 30, 255);
        paint.anti_alias = true;
        let path = tiny_skia::PathBuilder::from_circle(150.0, 100.0, 40.0).unwrap();
        p.fill_path(&path, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
        p
    };
    let white = {
        let mut p = tiny_skia::Pixmap::new(w, h).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        p
    };
    let list = [
        (Fx::Jitter, &photo), (Fx::Stitch, &photo), (Fx::RectPixelate, &photo), (Fx::GradientBlocks, &photo),
        (Fx::OutlineObject, &object), (Fx::ObjectTrail, &object), (Fx::Film, &photo), (Fx::GradientMap, &photo),
        (Fx::GradientBars, &white), (Fx::GradientGrid, &white), (Fx::BordersShapes, &white), (Fx::RandomShapes, &white),
        (Fx::Splatter, &white),
        (Fx::BlackWhitePlus, &photo), (Fx::ColorBalance, &photo), (Fx::Dream, &photo), (Fx::Pastel, &photo),
        (Fx::ComplementaryColors, &photo), (Fx::Dithering, &photo), (Fx::BevelObject, &object), (Fx::InnerShadow, &object),
        (Fx::FeatherObject, &object), (Fx::GridCheckerboard, &white), (Fx::Transparency, &object), (Fx::AlphaToGray, &object),
    ];
    let mut outs: Vec<(String, tiny_skia::Pixmap)> = Vec::new();
    let cols = 4u32;
    for (i, (k, src)) in list.iter().enumerate() {
        let mut st = FxState::new(*k, Col::rgba(20, 60, 160, 255), Col::rgba(250, 210, 60, 255));
        if *k == Fx::ColorBalance {
            st.vals[3] = crate::fx::V::I(70);
            st.vals[8] = crate::fx::V::I(-60);
        }
        if *k == Fx::BlackWhitePlus {
            st.vals[6] = crate::fx::V::C(1);
        }
        if *k == Fx::GradientMap {
            st.vals[0] = crate::fx::V::C(5); // Synthwave preset
        }
        let mut out = raster::apply_filter(&Filter::Fx(st), src, None);
        if src.pixel(0, 0).unwrap().alpha() == 0 {
            // show transparent results over a light background
            let mut bg = white.clone();
            bg.draw_pixmap(0, 0, out.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            out = bg;
        }
        let _ = i;
        outs.push((k.name().to_string(), out));
    }
    let mut cp = crate::curves_plus::CurvesPlus { mode: 2, ..Default::default() };
    cp.curves[0] = vec![(0, 128), (127, 255), (128, 0), (255, 127)];
    cp.curves[1] = vec![(0, 0), (128, 220), (255, 255)];
    cp.curves[2] = vec![(0, 0), (255, 255)];
    outs.push(("Curves+ (HSV)".into(), raster::apply_filter(&Filter::CurvesPlus(Box::new(cp)), &photo, None)));
    for k in [crate::interactive::Kind::Liquify, crate::interactive::Kind::GridWarp, crate::interactive::Kind::Smudge] {
        outs.push((k.name().into(), crate::interactive::demo(k, &photo)));
    }
    let rows = (outs.len() as u32).div_ceil(cols);
    let mut sheet = tiny_skia::Pixmap::new(cols * (w + 8) + 8, rows * (h + 8) + 8).unwrap();
    sheet.fill(tiny_skia::Color::from_rgba8(60, 60, 60, 255));
    for (i, (name, out)) in outs.iter().enumerate() {
        let (cx, cy) = (8 + (i as u32 % cols) * (w + 8), 8 + (i as u32 / cols) * (h + 8));
        sheet.draw_pixmap(cx as i32, cy as i32, out.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        println!("{i}: {name}");
    }
    sheet.save_png(std::env::temp_dir().join("ferrum-plugin-gallery.png")).unwrap();
}
#[cfg(test)]
mod smart_clone_tests {
    use crate::smart_clone::*;
    use egui::{pos2, vec2};
    use tiny_skia::Pixmap;

    fn stripes(w: u32, h: u32) -> Pixmap {
        let mut pm = Pixmap::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                let v = if (x / 4) % 2 == 0 { 200u8 } else { 60 };
                let i = ((y * w + x) * 4) as usize;
                pm.data_mut()[i..i + 4].copy_from_slice(&[v, v / 2, 255 - v, 255]);
            }
        }
        pm
    }

    #[test]
    fn heal_removes_blemish() {
        let mut pm = stripes(120, 120);
        let clean = pm.clone();
        for y in 55..65 {
            for x in 55..65 {
                let i = ((y * 120 + x) * 4) as usize;
                pm.data_mut()[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        assert!(heal(&mut pm, None, &[pos2(60.0, 60.0)], 18.0, true));
        let mut err = 0u32;
        for y in 55..65 {
            for x in 55..65 {
                let i = ((y * 120 + x) * 4) as usize;
                err += (pm.data()[i] as i32 - clean.data()[i] as i32).unsigned_abs();
                assert_eq!(pm.data()[i + 3], 255);
            }
        }
        // the red blemish (|255-v| ~ 55..195) must be gone; stripes are hard so allow some slack
        assert!(err / 100 < 90, "mean error {}", err / 100);
        // far pixels untouched
        assert_eq!(&pm.data()[0..4], &clean.data()[0..4]);
    }

    #[test]
    fn blend_clone_matches_brightness() {
        // left half dark flat, right half bright flat; clone bright source onto the dark side
        let mut base = Pixmap::new(100, 60).unwrap();
        for y in 0..60 {
            for x in 0..100 {
                let v = if x < 50 { 60u8 } else { 180 };
                let i = ((y * 100 + x) * 4) as usize;
                base.data_mut()[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let mut layer = base.clone();
        let pts = [pos2(20.0, 30.0), pos2(35.0, 30.0)];
        blend_clone(&mut layer, &base, None, &pts, 14.0, true, vec2(60.0, 0.0));
        let i = ((30 * 100 + 28) * 4) as usize;
        // colour-corrected: stays near the dark surroundings rather than the bright source
        assert!((layer.data()[i] as i32 - 60).abs() < 25, "got {}", layer.data()[i]);
    }
}

#[cfg(test)]
mod smart_clone_visual {
    use crate::smart_clone::*;
    use egui::{pos2, vec2};
    use tiny_skia::Pixmap;
    #[test]
    #[ignore]
    fn smart_clone_gallery() {
        let (w, h) = (240u32, 180u32);
        let mut pm = Pixmap::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32, y as f32);
                let wood = ((fx * 0.15 + (fy * 0.05).sin() * 3.0 + (fx * 0.02).cos() * 2.0).sin() * 0.5 + 0.5) * 70.0;
                let n = ((x * 7919 + y * 104729) % 23) as f32;
                let r = (130.0 + wood + n) as u8;
                let g = (90.0 + wood * 0.7 + n) as u8;
                let b = (50.0 + wood * 0.3) as u8;
                let i = ((y * w + x) * 4) as usize;
                pm.data_mut()[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
        let orig = pm.clone();
        // blemishes: a blue blob and a scribble
        let mut pts = vec![];
        for k in 0..30 { pts.push(pos2(60.0 + k as f32 * 3.0, 70.0 + (k as f32 * 0.5).sin() * 12.0)); }
        for y in 0..h { for x in 0..w {
            let d = ((x as f32 - 150.0).powi(2) + (y as f32 - 110.0).powi(2)).sqrt();
            if d < 14.0 { let i = ((y*w+x)*4) as usize; pm.data_mut()[i..i+4].copy_from_slice(&[30,60,200,255]); }
        }}
        let blem = pm.clone();
        let mut healed = pm.clone();
        heal(&mut healed, None, &[pos2(150.0, 110.0)], 36.0, true);
        // blend clone: bright patch from the left to a darker area
        let mut dark = orig.clone();
        for y in 0..h { for x in 120..w { let i=((y*w+x)*4) as usize; for c in 0..3 { dark.data_mut()[i+c] = (dark.data()[i+c] as f32*0.55) as u8; } } }
        let mut plain = dark.clone();
        let mut blend = dark.clone();
        let cp: Vec<_> = (0..20).map(|k| pos2(150.0 + k as f32 * 2.0, 60.0)).collect();
        blend_clone(&mut blend, &dark, None, &cp, 30.0, true, vec2(-80.0, 0.0));
        crate::paint::draw_brush_stroke(&mut plain, None, &cp, 30.0, true, false, crate::paint::BrushKind::Clone { source: &dark.clone(), offset: vec2(-80.0, 0.0) });
        let tiles = [&blem, &healed, &orig, &plain, &blend, &dark];
        let mut out = Pixmap::new(w * 3, h * 2).unwrap();
        for (k, t) in tiles.iter().enumerate() {
            out.draw_pixmap(((k as u32 % 3) * w) as i32, ((k as u32 / 3) * h) as i32, t.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        }
        let p = std::env::temp_dir().join("ferrum-smart-gallery.png");
        out.save_png(p).unwrap();
    }
}
