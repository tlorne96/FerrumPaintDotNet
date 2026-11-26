//! Main window chrome: menu bar with image tabs, toolbar, tool options, status bar,
//! and the floating Tools / History / Layers windows.

use crate::app::{App, Cmd};
use crate::canvas::Pending;
use crate::icons::{icon_button, paint_icon, Icon};
use crate::fx::{Fx, FxState, Menu};
use crate::raster::{Filter, Orient, SelMode};
use crate::tools::*;
use egui::{vec2, Color32, RichText, Sense, Ui};

const SC_MENU_W: f32 = 230.0;

fn item(ui: &mut Ui, label: &str, sc: &str, enabled: bool) -> bool {
    let b = egui::Button::new(label).shortcut_text(sc);
    let r = ui.add_enabled(enabled, b);
    if r.clicked() {
        ui.close_menu();
        true
    } else {
        false
    }
}

impl App {
    pub fn top_panels(&mut self, ctx: &egui::Context) {
        let mut cmds: Vec<Cmd> = vec![];
        let p = crate::theme::pal(self.dark);
        let bar = |fill: Color32, m: (i8, i8)| egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(m.0, m.1));
        egui::TopBottomPanel::top("menubar").frame(bar(p.bar, (6, 2))).show_separator_line(false).show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                egui::menu::bar(ui, |ui| self.menus(ui, &mut cmds));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.image_tabs(ui, &mut cmds);
                });
            });
        });
        egui::TopBottomPanel::top("toolbar").frame(bar(p.bar, (8, 3))).show(ctx, |ui| {
            ui.horizontal(|ui| self.main_toolbar(ui, &mut cmds));
        });
        egui::TopBottomPanel::top("tooloptions").frame(bar(p.panel, (8, 4))).show(ctx, |ui| {
            ui.horizontal(|ui| self.tool_options(ui, &mut cmds));
        });
        for c in cmds {
            self.run(ctx, c);
        }
    }

    fn menus(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let has = !self.docs.is_empty();
        let (sel, layers, active, can_undo, can_redo) = match self.docs.get(self.cur) {
            Some(d) => (d.has_selection(), d.layers.len(), d.active, !d.undo.is_empty() || self.pending.is_editable(), !d.redo.is_empty()),
            None => (false, 0, 0, false, false),
        };
        let mut c = |cmd: Cmd, ok: bool| if ok { cmds.push(cmd) };
        ui.menu_button("File", |ui| {
            ui.set_min_width(SC_MENU_W);
            if item(ui, "New...", "Ctrl+N", true) { c(Cmd::New, true) }
            if item(ui, "Open...", "Ctrl+O", true) { c(Cmd::Open, true) }
            ui.separator();
            if item(ui, "Save", "Ctrl+S", has) { c(Cmd::Save, true) }
            if item(ui, "Save As...", "Ctrl+Shift+S", has) { c(Cmd::SaveAs, true) }
            if item(ui, "Save All", "Ctrl+Alt+S", has) { c(Cmd::SaveAll, true) }
            if item(ui, "Export...", "Ctrl+Alt+E", has) { c(Cmd::Export, true) }
            ui.separator();
            if item(ui, "Close", "Ctrl+W", has) { c(Cmd::Close, true) }
            ui.separator();
            if item(ui, "Exit", "Alt+F4", true) { c(Cmd::Exit, true) }
        });
        ui.menu_button("Edit", |ui| {
            ui.set_min_width(SC_MENU_W);
            if item(ui, "Undo", "Ctrl+Z", can_undo) { c(Cmd::Undo, true) }
            if item(ui, "Redo", "Ctrl+Y", can_redo) { c(Cmd::Redo, true) }
            ui.separator();
            if item(ui, "Cut", "Ctrl+X", has) { c(Cmd::Cut, true) }
            if item(ui, "Copy", "Ctrl+C", has) { c(Cmd::Copy, true) }
            if item(ui, "Copy Merged", "Ctrl+Shift+C", has) { c(Cmd::CopyMerged, true) }
            if item(ui, "Paste", "Ctrl+V", has) { c(Cmd::Paste, true) }
            if item(ui, "Paste Into New Layer", "Ctrl+Shift+V", has) { c(Cmd::PasteNewLayer, true) }
            if item(ui, "Paste Into New Image", "Ctrl+Alt+V", true) { c(Cmd::PasteNewImage, true) }
            ui.separator();
            if item(ui, "Select All", "Ctrl+A", has) { c(Cmd::SelectAll, true) }
            if item(ui, "Deselect All", "Ctrl+D", sel) { c(Cmd::Deselect, true) }
            ui.separator();
            if item(ui, "Erase Selection", "Delete", has) { c(Cmd::EraseSelection, true) }
            if item(ui, "Fill Selection", "Backspace", has) { c(Cmd::FillSelection(false), true) }
            if item(ui, "Invert Selection", "Ctrl+I", sel) { c(Cmd::InvertSelection, true) }
        });
        ui.menu_button("View", |ui| {
            ui.set_min_width(SC_MENU_W);
            if item(ui, "Zoom In", "Ctrl++", has) { c(Cmd::ZoomIn, true) }
            if item(ui, "Zoom Out", "Ctrl+-", has) { c(Cmd::ZoomOut, true) }
            if item(ui, "Zoom to Window", "Ctrl+B", has) { c(Cmd::ZoomWindow, true) }
            if item(ui, "Zoom to Selection", "Ctrl+Shift+B", sel) { c(Cmd::ZoomSelection, true) }
            if item(ui, "Actual Size", "Ctrl+0", has) { c(Cmd::ActualSize, true) }
            ui.separator();
            let g = if self.show_grid { "✔ Pixel Grid" } else { "Pixel Grid" };
            if item(ui, g, "Ctrl+'", has) { c(Cmd::ToggleGrid, true) }
            let s = if self.snap { "✔ Snap to Grid" } else { "Snap to Grid" };
            if item(ui, s, "Ctrl+;", true) { c(Cmd::ToggleSnap, true) }
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.label("Grid size:");
                let r = ui.add(egui::DragValue::new(&mut self.grid_size).range(2..=1024).suffix(" px").speed(0.25));
                if r.changed() { crate::theme::save_pref("grid", &self.grid_size.to_string()) }
            });
            ui.separator();
            let d = if self.dark { "✔ Dark Theme" } else { "Dark Theme" };
            if item(ui, d, "", true) { c(Cmd::ToggleTheme, true) }
        });
        ui.menu_button("Image", |ui| {
            ui.set_min_width(SC_MENU_W);
            if item(ui, "Crop to Selection", "Ctrl+Shift+X", sel) { c(Cmd::Crop, true) }
            if item(ui, "Resize...", "Ctrl+R", has) { c(Cmd::Resize, true) }
            if item(ui, "Canvas Size...", "Ctrl+Shift+R", has) { c(Cmd::CanvasSize, true) }
            ui.separator();
            if item(ui, "Flip Horizontal", "", has) { c(Cmd::Orient(Orient::FlipH), true) }
            if item(ui, "Flip Vertical", "", has) { c(Cmd::Orient(Orient::FlipV), true) }
            ui.separator();
            if item(ui, "Rotate 90° Clockwise", "Ctrl+H", has) { c(Cmd::Orient(Orient::Rot90Cw), true) }
            if item(ui, "Rotate 90° Counter-clockwise", "Ctrl+G", has) { c(Cmd::Orient(Orient::Rot90Ccw), true) }
            if item(ui, "Rotate 180°", "Ctrl+J", has) { c(Cmd::Orient(Orient::Rot180), true) }
            ui.separator();
            if item(ui, "Flatten", "Ctrl+Shift+F", layers > 1) { c(Cmd::Flatten, true) }
        });
        ui.menu_button("Layers", |ui| {
            ui.set_min_width(SC_MENU_W);
            if item(ui, "Add New Layer", "Ctrl+Shift+N", has) { c(Cmd::AddLayer, true) }
            if item(ui, "Delete Layer", "Ctrl+Shift+Del", layers > 1) { c(Cmd::DeleteLayer, true) }
            if item(ui, "Duplicate Layer", "Ctrl+Shift+D", has) { c(Cmd::DuplicateLayer, true) }
            if item(ui, "Merge Layer Down", "Ctrl+M", has && active > 0) { c(Cmd::MergeDown, true) }
            if item(ui, "Import From File...", "", has) { c(Cmd::ImportLayer, true) }
            ui.separator();
            if item(ui, "Flip Layer Horizontal", "", has) { c(Cmd::LayerOrient(Orient::FlipH), true) }
            if item(ui, "Flip Layer Vertical", "", has) { c(Cmd::LayerOrient(Orient::FlipV), true) }
            if item(ui, "Rotate / Zoom...", "Ctrl+Shift+Z", has) { c(Cmd::RotateZoom, true) }
            ui.separator();
            if item(ui, "Move Layer Up", "Alt+Shift+PgUp", has && active + 1 < layers) { c(Cmd::MoveLayer(1), true) }
            if item(ui, "Move Layer Down", "Alt+Shift+PgDn", has && active > 0) { c(Cmd::MoveLayer(-1), true) }
            ui.separator();
            if item(ui, "Layer Properties...", "F4", has) { c(Cmd::LayerProperties, true) }
        });
        let (pc, sc2) = (self.primary, self.secondary);
        let fx = |k: Fx| Filter::Fx(FxState::new(k, pc, sc2));
        ui.menu_button("Adjustments", |ui| {
            ui.set_min_width(SC_MENU_W);
            let adj: [(Filter, &str); 13] = [
                (Filter::AutoLevel, "Ctrl+Shift+L"),
                (Filter::BlackAndWhite, "Ctrl+Shift+G"),
                (Filter::BrightnessContrast { brightness: 0, contrast: 0 }, "Ctrl+Shift+T"),
                (Filter::Curves { points: vec![(0, 0), (255, 255)], luminosity: true }, "Ctrl+Shift+M"),
                (fx(Fx::Exposure), ""),
                (fx(Fx::HighlightsShadows), ""),
                (Filter::HueSaturation { hue: 0, saturation: 100, lightness: 0 }, "Ctrl+Shift+U"),
                (Filter::InvertAlpha, "Ctrl+Alt+I"),
                (Filter::InvertColors, "Ctrl+Shift+I"),
                (Filter::Levels { in_lo: 0, in_hi: 255, gamma: 1.0, out_lo: 0, out_hi: 255 }, "Ctrl+L"),
                (Filter::Posterize { levels: 16 }, "Ctrl+Shift+P"),
                (fx(Fx::Sepia), "Ctrl+Shift+E"),
                (fx(Fx::TemperatureTint), ""),
            ];
            for (f, sc) in adj {
                let label = if f.has_dialog() { format!("{}...", f.name()) } else { f.name().to_string() };
                if item(ui, &label, sc, has) { c(Cmd::Filter(f), true) }
            }
        });
        ui.menu_button("Effects", |ui| {
            ui.set_min_width(SC_MENU_W);
            let last = self.last_effect.as_ref().map(|f| format!("Repeat {}", f.name())).unwrap_or("Repeat Effect".into());
            if item(ui, &last, "Ctrl+F", has && self.last_effect.is_some()) { c(Cmd::RepeatEffect, true) }
            ui.separator();
            for m in Menu::EFFECT_MENUS {
                ui.menu_button(m.name(), |ui| {
                    ui.set_min_width(180.0);
                    for &k in Fx::ALL.iter().filter(|k| k.menu() == m) {
                        if item(ui, &format!("{}...", k.name()), "", has) { c(Cmd::Filter(fx(k)), true) }
                    }
                });
            }
        });        ui.menu_button("Window", |ui| {
            ui.set_min_width(SC_MENU_W);
            let names = ["Tools", "History", "Layers", "Colors"];
            let keys = ["F5", "F6", "F7", "F8"];
            for i in 0..4 {
                let l = if self.show[i] { format!("✔ {}", names[i]) } else { names[i].to_string() };
                if item(ui, &l, keys[i], true) { c(Cmd::ToggleWindow(i as u8), true) }
            }
            ui.separator();
            for i in 0..4 {
                if item(ui, &format!("Reset {} Window", names[i]), &format!("Ctrl+Shift+{}", keys[i]), true) {
                    c(Cmd::ResetWindow(i as u8), true)
                }
            }
            ui.separator();
            if item(ui, "Next Image", "Ctrl+Tab", self.docs.len() > 1) { c(Cmd::NextImage(1), true) }
            if item(ui, "Previous Image", "Ctrl+Shift+Tab", self.docs.len() > 1) { c(Cmd::NextImage(-1), true) }
        });
        ui.menu_button("Help", |ui| {
            ui.set_min_width(SC_MENU_W);
            if item(ui, "Keyboard Shortcuts", "F1", true) { c(Cmd::Shortcuts, true) }
            if item(ui, "About Ferrum Paint", "", true) { c(Cmd::About, true) }
        });
    }

    fn image_tabs(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        // right-to-left layout: iterate in reverse so the first image is leftmost
        let n = self.docs.len();
        let mut close: Option<usize> = None;
        for i in (0..n).rev() {
            let doc = &self.docs[i];
            let selected = i == self.cur;
            let size = vec2(46.0, 38.0);
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            let vis = ui.style().interact_selectable(&resp, selected);
            if selected || resp.hovered() {
                ui.painter().rect_filled(rect, 3.0, vis.weak_bg_fill);
            }
            if let Some(t) = &doc.doc_thumb {
                let ts0 = t.size_vec2();
                let k = (40.0 / ts0.x).min(32.0 / ts0.y).min(1.0);
                let ts = ts0 * k;
                let r = egui::Rect::from_center_size(rect.center(), ts);
                if let Some(ck) = &self.checker {
                    ui.painter().image(ck.id(), r, egui::Rect::from_min_size(egui::Pos2::ZERO, ts / 8.0), Color32::WHITE);
                }
                ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                ui.painter().add(egui::Shape::closed_line(
                    vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()],
                    egui::Stroke::new(1.0f32, if selected { crate::theme::ui_pal(ui).accent } else { Color32::from_gray(120) }),
                ));
            }
            if doc.is_dirty() {
                ui.painter().circle_filled(rect.right_top() + vec2(-6.0, 6.0), 3.5, Color32::from_rgb(220, 110, 40));
            }
            // close button on hover
            let close_rect = egui::Rect::from_min_size(rect.left_top(), vec2(13.0, 13.0));
            if resp.hovered() || selected {
                ui.painter().rect_filled(close_rect, 2.0, Color32::from_gray(90));
                ui.painter().text(close_rect.center(), egui::Align2::CENTER_CENTER, "×", egui::FontId::proportional(12.0), Color32::WHITE);
            }
            let tip = format!("{}  ({} × {})", doc.name, doc.width, doc.height);
            if resp.clicked() {
                let on_close = resp.interact_pointer_pos().map(|p| close_rect.contains(p)).unwrap_or(false);
                if on_close {
                    close = Some(i);
                } else {
                    cmds.push(Cmd::SwitchImage(i));
                }
            }
            if resp.middle_clicked() {
                close = Some(i);
            }
            resp.on_hover_text(tip);
        }
        if let Some(i) = close {
            self.commit_pending();
            self.request_close(i);
        }
    }

    fn main_toolbar(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let sz = 24.0;
        let buttons: [(Icon, Cmd, &str); 12] = [
            (Icon::New, Cmd::New, "New (Ctrl+N)"),
            (Icon::Open, Cmd::Open, "Open (Ctrl+O)"),
            (Icon::Save, Cmd::Save, "Save (Ctrl+S)"),
            (Icon::Cut, Cmd::Cut, "Cut (Ctrl+X)"),
            (Icon::Copy, Cmd::Copy, "Copy (Ctrl+C)"),
            (Icon::Paste, Cmd::Paste, "Paste (Ctrl+V)"),
            (Icon::Crop, Cmd::Crop, "Crop to Selection (Ctrl+Shift+X)"),
            (Icon::Deselect, Cmd::Deselect, "Deselect (Ctrl+D)"),
            (Icon::Undo, Cmd::Undo, "Undo (Ctrl+Z)"),
            (Icon::Redo, Cmd::Redo, "Redo (Ctrl+Y)"),
            (Icon::Grid, Cmd::ToggleGrid, "Toggle Pixel Grid (Ctrl+')"),
            (Icon::Snap, Cmd::ToggleSnap, "Snap to Grid (Ctrl+;) - shapes, lines, selections, gradients, text and moves snap to the grid"),
        ];
        for (i, (icon, cmd, tip)) in buttons.into_iter().enumerate() {
            if matches!(i, 3 | 6 | 8 | 10) {
                ui.separator();
            }
            let sel = (matches!(cmd, Cmd::ToggleGrid) && self.show_grid) || (matches!(cmd, Cmd::ToggleSnap) && self.snap);
            if icon_button(ui, icon, sel, sz, tip).clicked() {
                cmds.push(cmd);
            }
        }
        if self.snap {
            let r = ui.add(egui::DragValue::new(&mut self.grid_size).range(2..=1024).suffix(" px").speed(0.25));
            if r.changed() { crate::theme::save_pref("grid", &self.grid_size.to_string()) }
            r.on_hover_text("Grid size");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let wins = [(Icon::Colors, 3u8, "Colors (F8)"), (Icon::Layers, 2, "Layers (F7)"), (Icon::History, 1, "History (F6)"), (Icon::Tools, 0, "Tools (F5)")];
            for (icon, i, tip) in wins {
                if icon_button(ui, icon, self.show[i as usize], sz, tip).clicked() {
                    cmds.push(Cmd::ToggleWindow(i));
                }
            }
            ui.separator();
            if let Some(doc) = self.docs.get(self.cur) {
                let mut z = doc.view.zoom * 100.0;
                let r = ui.add(egui::DragValue::new(&mut z).range(1.0..=9600.0).suffix("%").speed(1.0).max_decimals(0));
                if r.changed() {
                    self.set_zoom(z / 100.0, None);
                }
                ui.label("Zoom:");
            }
        });
    }

    fn width_widget(&mut self, ui: &mut Ui, label: &str) {
        ui.label(label);
        let r = ui.add(egui::DragValue::new(&mut self.ts.brush_width).range(1.0..=2000.0).speed(0.25).max_decimals(0));
        r.on_hover_text("[ and ] change the width (Ctrl = ×5)");
        egui::ComboBox::from_id_salt("bw_presets").width(18.0).selected_text("").show_ui(ui, |ui| {
            for &p in crate::app::brush_presets() {
                ui.selectable_value(&mut self.ts.brush_width, p, format!("{p}"));
            }
        });
    }

    fn aa_widget(&mut self, ui: &mut Ui) {
        ui.separator();
        let t = if self.ts.antialias { "Antialiasing: On" } else { "Antialiasing: Off" };
        ui.toggle_value(&mut self.ts.antialias, t)
            .on_hover_text("Smooth edges");
    }

    fn blend_widget(&mut self, ui: &mut Ui) {
        let t = if self.ts.overwrite { "Blending: Overwrite" } else { "Blending: Normal" };
        ui.toggle_value(&mut self.ts.overwrite, t)
            .on_hover_text("Normal blends with the layer; Overwrite replaces pixels including alpha");
    }

    fn selmode_widget(&mut self, ui: &mut Ui) {
        ui.label("Selection mode:");
        egui::ComboBox::from_id_salt("selmode").selected_text(self.ts.sel_mode.name()).show_ui(ui, |ui| {
            for m in SelMode::ALL {
                ui.selectable_value(&mut self.ts.sel_mode, m, m.name());
            }
        });
    }

    fn flood_widgets(&mut self, ui: &mut Ui, sample: bool) {
        ui.separator();
        ui.label("Flood mode:");
        egui::ComboBox::from_id_salt("flood").selected_text(if self.ts.flood_global { "Global" } else { "Contiguous" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut self.ts.flood_global, false, "Contiguous");
            ui.selectable_value(&mut self.ts.flood_global, true, "Global");
        });
        ui.label("Tolerance:");
        let mut t = self.ts.tolerance * 100.0;
        if ui.add(egui::Slider::new(&mut t, 0.0..=100.0).suffix("%").max_decimals(0)).changed() {
            self.ts.tolerance = t / 100.0;
        }
        if sample {
            ui.label("Sampling:");
            egui::ComboBox::from_id_salt("fsample").selected_text(if self.ts.sample_image { "Image" } else { "Layer" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.ts.sample_image, false, "Layer");
                ui.selectable_value(&mut self.ts.sample_image, true, "Image");
            });
        }
    }

    fn dash_caps(&mut self, ui: &mut Ui, caps: bool) {
        ui.separator();
        if caps {
            ui.label("Start:");
            egui::ComboBox::from_id_salt("scap").selected_text(self.ts.start_cap.name()).show_ui(ui, |ui| {
                for c in LineCap::ALL {
                    ui.selectable_value(&mut self.ts.start_cap, c, c.name());
                }
            });
        }
        ui.label("Style:");
        egui::ComboBox::from_id_salt("dash").selected_text(self.ts.dash.name()).show_ui(ui, |ui| {
            for d in DashStyle::ALL {
                ui.selectable_value(&mut self.ts.dash, d, d.name());
            }
        });
        if caps {
            ui.label("End:");
            egui::ComboBox::from_id_salt("ecap").selected_text(self.ts.end_cap.name()).show_ui(ui, |ui| {
                for c in LineCap::ALL {
                    ui.selectable_value(&mut self.ts.end_cap, c, c.name());
                }
            });
        }
    }

    fn tool_options(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        ui.spacing_mut().combo_width = 90.0;
        // Tool selector
        let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        paint_icon(ui.painter(), r, Icon::Tool(self.tool));
        ui.label(RichText::new(self.tool.name()).strong());
        ui.separator();
        match self.tool {
            Tool::RectSelect | Tool::LassoSelect | Tool::EllipseSelect => {
                self.selmode_widget(ui);
            }
            Tool::MagicWand => {
                self.selmode_widget(ui);
                self.flood_widgets(ui, true);
            }
            Tool::MovePixels | Tool::MoveSelection => {
                ui.label("Drag to move, right-drag to rotate. Hold Ctrl when starting to move a copy (Move Pixels).");
                self.aa_widget(ui);
            }
            Tool::Zoom | Tool::Pan => {
                ui.label(self.tool.hint());
            }
            Tool::PaintBucket => {
                self.flood_widgets(ui, true);
                self.aa_widget(ui);
                self.blend_widget(ui);
            }
            Tool::Gradient => {
                for g in GradKind::ALL {
                    ui.selectable_value(&mut self.ts.gradient, g, g.name());
                }
                ui.separator();
                ui.label("Mode:");
                ui.selectable_value(&mut self.ts.grad_transparency, false, "Color");
                ui.selectable_value(&mut self.ts.grad_transparency, true, "Transparency");
            }
            Tool::Paintbrush | Tool::Eraser => {
                self.width_widget(ui, "Brush width:");
                self.aa_widget(ui);
                if self.tool == Tool::Paintbrush {
                    self.blend_widget(ui);
                }
            }
            Tool::Pencil => {
                self.blend_widget(ui);
            }
            Tool::ColorPicker => {
                ui.label("Sampling:");
                let names = [(1, "Single Pixel"), (3, "3 × 3 Region"), (5, "5 × 5 Region"), (7, "7 × 7 Region"), (9, "9 × 9 Region")];
                let cur = names.iter().find(|n| n.0 == self.ts.picker_sample).map(|n| n.1).unwrap_or("Single Pixel");
                egui::ComboBox::from_id_salt("pick").width(110.0).selected_text(cur).show_ui(ui, |ui| {
                    for (v, n) in names {
                        ui.selectable_value(&mut self.ts.picker_sample, v, n);
                    }
                });
                egui::ComboBox::from_id_salt("picksrc").selected_text(if self.ts.picker_image { "Image" } else { "Layer" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ts.picker_image, false, "Layer");
                    ui.selectable_value(&mut self.ts.picker_image, true, "Image");
                });
                ui.label("After click:");
                egui::ComboBox::from_id_salt("pickafter")
                    .width(150.0)
                    .selected_text(if self.ts.picker_switch_back { "Switch to previous tool" } else { "Do not switch tool" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.ts.picker_switch_back, false, "Do not switch tool");
                        ui.selectable_value(&mut self.ts.picker_switch_back, true, "Switch to previous tool");
                    });
            }
            Tool::CloneStamp => {
                self.width_widget(ui, "Brush width:");
                self.aa_widget(ui);
                self.blend_widget(ui);
            }
            Tool::Recolor => {
                self.width_widget(ui, "Brush width:");
                ui.label("Tolerance:");
                let mut t = self.ts.tolerance * 100.0;
                if ui.add(egui::Slider::new(&mut t, 0.0..=100.0).suffix("%").max_decimals(0)).changed() {
                    self.ts.tolerance = t / 100.0;
                }
                self.aa_widget(ui);
            }
            Tool::Text => {
                ui.label("Font:");
                egui::ComboBox::from_id_salt("font").selected_text(if self.ts.font == FontChoice::Sans { "Sans" } else { "Monospace" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ts.font, FontChoice::Sans, "Sans");
                    ui.selectable_value(&mut self.ts.font, FontChoice::Mono, "Monospace");
                });
                ui.add(egui::DragValue::new(&mut self.ts.font_size).range(4.0..=1000.0).speed(0.5).max_decimals(0).suffix(" px"));
                ui.toggle_value(&mut self.ts.bold, RichText::new("B").strong()).on_hover_text("Bold");
                ui.toggle_value(&mut self.ts.italic, RichText::new("I").italics()).on_hover_text("Italic");
                ui.toggle_value(&mut self.ts.underline, RichText::new("U").underline()).on_hover_text("Underline");
                ui.separator();
                ui.selectable_value(&mut self.ts.align, TextAlign::Left, "Left");
                ui.selectable_value(&mut self.ts.align, TextAlign::Center, "Center");
                ui.selectable_value(&mut self.ts.align, TextAlign::Right, "Right");
                self.aa_widget(ui);
                self.blend_widget(ui);
            }
            Tool::LineCurve => {
                self.width_widget(ui, "Brush width:");
                self.dash_caps(ui, true);
                self.aa_widget(ui);
                self.blend_widget(ui);
            }
            Tool::Shapes => {
                ui.label("Shape:");
                egui::ComboBox::from_id_salt("shape").width(130.0).selected_text(self.ts.shape.name()).show_ui(ui, |ui| {
                    for s in ShapeKind::ALL {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            paint_icon(ui.painter(), r, Icon::Shape(s));
                            ui.selectable_value(&mut self.ts.shape, s, s.name());
                        });
                    }
                })
                .response
                .on_hover_text("A / Shift+A cycle shapes");
                ui.separator();
                ui.label("Draw from:");
                ui.selectable_value(&mut self.ts.from_center, false, "Corner").on_hover_text("Drag from one corner to the opposite corner");
                ui.selectable_value(&mut self.ts.from_center, true, "Centre").on_hover_text("Drag outward from the centre point (hold Alt to toggle temporarily)");
                ui.separator();
                egui::ComboBox::from_id_salt("sstyle").width(170.0).selected_text(self.ts.shape_style.name()).show_ui(ui, |ui| {
                    for s in ShapeStyle::ALL {
                        ui.selectable_value(&mut self.ts.shape_style, s, s.name());
                    }
                });
                self.width_widget(ui, "Width:");
                if self.ts.shape == ShapeKind::RoundedRectangle {
                    ui.label("Radius:");
                    ui.add(egui::DragValue::new(&mut self.ts.corner_radius).range(0.0..=1000.0).max_decimals(0));
                }
                self.dash_caps(ui, false);
                self.aa_widget(ui);
                self.blend_widget(ui);
            }
        }
        if self.pending.is_editable() {
            ui.separator();
            if ui.button("✔ Finish").on_hover_text("Finish (Enter)").clicked() {
                cmds.push(Cmd::Finish);
            }
        }
    }

    pub fn status_bar(&mut self, ctx: &egui::Context) {
        let p = crate::theme::pal(self.dark);
        let fr = egui::Frame::new().fill(p.bar).inner_margin(egui::Margin::symmetric(8, 3));
        egui::TopBottomPanel::bottom("status").frame(fr).show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(p.weak);
            ui.horizontal(|ui| {
                let hint = if self.status_msg.is_empty() { self.tool.hint().to_string() } else { self.status_msg.clone() };
                ui.add(egui::Label::new(hint).truncate());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(doc) = self.docs.get(self.cur) {
                        ui.label(format!("{:.0}%", doc.view.zoom * 100.0));
                        ui.separator();
                        ui.label(format!("Image: {} × {}", doc.width, doc.height)).on_hover_text("Image size");
                        ui.separator();
                        let sel = match doc.sel_bounds {
                            Some((x0, y0, x1, y1)) => format!("Selection: {} × {}", x1 - x0, y1 - y0),
                            None if doc.selection.is_some() => "Selection: 0 × 0".into(),
                            None => format!("Selection: {} × {}", doc.width, doc.height),
                        };
                        ui.label(sel).on_hover_text("Selection size");
                        ui.separator();
                        let pos = self.status_pos.map(|(x, y)| format!("{x}, {y} px")).unwrap_or_default();
                        ui.add_sized([80.0, 16.0], egui::Label::new(pos)).on_hover_text("Cursor position");
                    }
                });
            });
        });
    }

    // ------------------------------------------------------------------------------------------
    // Floating windows

    pub fn floating_windows(&mut self, ctx: &egui::Context) {
        use crate::theme::tool_window;
        let area = ctx.available_rect();
        let mut cmds = vec![];
        if self.show[0] {
            let id = egui::Id::new(("win_tools", self.window_gen[0]));
            self.show[0] = tool_window(ctx, id, "Tools", |w| w.default_pos(area.left_top() + vec2(8.0, 8.0)).resizable(false), |ui| {
                egui::Grid::new("toolgrid").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                    for (i, t) in Tool::ALL.iter().enumerate() {
                        let tip = format!("{} ({})", t.name(), t.key());
                        if icon_button(ui, Icon::Tool(*t), self.tool == *t, 28.0, &tip).clicked() {
                            self.set_tool(*t);
                        }
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            });
        }
        if self.show[1] {
            let id = egui::Id::new(("win_history", self.window_gen[1]));
            self.show[1] = tool_window(ctx, id, "History", |w| w.default_pos(area.right_top() + vec2(-232.0, 8.0)).default_size(vec2(214.0, 220.0)), |ui| {
                self.history_ui(ui, &mut cmds)
            });
        }
        if self.show[2] {
            let id = egui::Id::new(("win_layers", self.window_gen[2]));
            self.show[2] = tool_window(ctx, id, "Layers", |w| w.default_pos(area.right_top() + vec2(-232.0, 272.0)).default_size(vec2(214.0, 230.0)), |ui| {
                self.layers_ui(ui, &mut cmds)
            });
        }
        if self.show[3] {
            let id = egui::Id::new(("win_colors", self.window_gen[3]));
            self.show[3] = tool_window(
                ctx,
                id,
                "Colors",
                |w| w.pivot(egui::Align2::LEFT_BOTTOM).default_pos(area.left_bottom() + vec2(8.0, -8.0)).resizable(false),
                |ui| self.colors_ui(ui),
            );
        }
        for c in cmds {
            self.run(ctx, c);
        }
    }
    fn history_ui(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let Some(doc) = self.docs.get(self.cur) else {
            ui.label("No image");
            return;
        };
        let n_undo = doc.undo.len();
        let mut target: Option<usize> = None; // number of undo entries wanted
        egui::ScrollArea::vertical().id_salt("hist").auto_shrink([false, true]).min_scrolled_height(150.0).max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
            ui.set_min_width(190.0);
            let base = ui.selectable_label(n_undo == 0, doc.base_name.clone());
            if base.clicked() {
                target = Some(0);
            }
            for (i, e) in doc.undo.iter().enumerate() {
                if ui.selectable_label(i + 1 == n_undo, &e.name).clicked() {
                    target = Some(i + 1);
                }
            }
            for (j, e) in doc.redo.iter().rev().enumerate() {
                let r = ui.add(egui::SelectableLabel::new(false, RichText::new(&e.name).color(Color32::from_gray(150))));
                if r.clicked() {
                    target = Some(n_undo + j + 1);
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Undo, false, 22.0, "Undo (Ctrl+Z)").clicked() {
                cmds.push(Cmd::Undo);
            }
            if icon_button(ui, Icon::Redo, false, 22.0, "Redo (Ctrl+Y)").clicked() {
                cmds.push(Cmd::Redo);
            }
        });
        if let Some(t) = target {
            self.commit_pending();
            let doc = &mut self.docs[self.cur];
            while doc.undo.len() > t && doc.undo() {}
            while doc.undo.len() < t && doc.redo() {}
        }
    }

    fn layers_ui(&mut self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        if self.docs.is_empty() {
            ui.label("No image");
            return;
        }
        let checker = self.checker.as_ref().map(|c| c.id());
        let mut select: Option<usize> = None;
        let mut toggle: Option<usize> = None;
        let mut props = false;
        {
            let doc = &self.docs[self.cur];
            egui::ScrollArea::vertical().id_salt("layers").auto_shrink([false, true]).min_scrolled_height(150.0).max_height(220.0).show(ui, |ui| {
                ui.set_min_width(190.0);
                for i in (0..doc.layers.len()).rev() {
                    let l = &doc.layers[i];
                    let selected = i == doc.active;
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                    let vis = ui.style().interact_selectable(&resp, selected);
                    if selected {
                        ui.painter().rect_filled(rect, 2.0, ui.visuals().selection.bg_fill);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, 2.0, vis.weak_bg_fill);
                    }
                    let tr = egui::Rect::from_min_size(rect.left_top() + vec2(3.0, 3.0), vec2(40.0, 40.0));
                    if let Some(t) = doc.layer_thumbs.get(i) {
                        let ts = t.size_vec2();
                        let r = egui::Rect::from_center_size(tr.center(), ts);
                        if let Some(ck) = checker {
                            ui.painter().image(ck, r, egui::Rect::from_min_size(egui::Pos2::ZERO, ts / 8.0), Color32::WHITE);
                        }
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                        ui.painter().add(egui::Shape::closed_line(
                            vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()],
                            egui::Stroke::new(1.0f32, Color32::from_gray(140)),
                        ));
                    }
                    let tc = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
                    ui.painter().text(
                        tr.right_center() + vec2(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &l.name,
                        egui::FontId::proportional(13.0),
                        tc,
                    );
                    // visibility checkbox
                    let cb = egui::Rect::from_center_size(rect.right_center() - vec2(14.0, 0.0), vec2(14.0, 14.0));
                    let pl = crate::theme::ui_pal(ui);
                    ui.painter().rect_filled(cb, 3.0, if l.visible { pl.accent } else { pl.field });
                    ui.painter().add(egui::Shape::closed_line(
                        vec![cb.left_top(), cb.right_top(), cb.right_bottom(), cb.left_bottom()],
                        egui::Stroke::new(1.0f32, if l.visible { pl.accent } else { pl.weak }),
                    ));
                    if l.visible {
                        let c = cb.center();
                        ui.painter().add(egui::Shape::line(vec![c + vec2(-3.5, 0.0), c + vec2(-1.0, 2.8), c + vec2(3.8, -2.8)], egui::Stroke::new(1.8f32, Color32::WHITE)));
                    }
                    if resp.clicked() {
                        let on_cb = resp.interact_pointer_pos().map(|p| cb.expand(3.0).contains(p)).unwrap_or(false);
                        if on_cb {
                            toggle = Some(i);
                        } else {
                            select = Some(i);
                        }
                    }
                    if resp.double_clicked() {
                        select = Some(i);
                        props = true;
                    }
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let b: [(Icon, Cmd, &str); 7] = [
                (Icon::LayerAdd, Cmd::AddLayer, "Add New Layer (Ctrl+Shift+N)"),
                (Icon::LayerDelete, Cmd::DeleteLayer, "Delete Layer (Ctrl+Shift+Del)"),
                (Icon::LayerDuplicate, Cmd::DuplicateLayer, "Duplicate Layer (Ctrl+Shift+D)"),
                (Icon::LayerMerge, Cmd::MergeDown, "Merge Layer Down (Ctrl+M)"),
                (Icon::LayerUp, Cmd::MoveLayer(1), "Move Layer Up (Alt+Shift+PgUp)"),
                (Icon::LayerDown, Cmd::MoveLayer(-1), "Move Layer Down (Alt+Shift+PgDn)"),
                (Icon::LayerProps, Cmd::LayerProperties, "Layer Properties (F4)"),
            ];
            for (icon, cmd, tip) in b {
                if icon_button(ui, icon, false, 22.0, tip).clicked() {
                    cmds.push(cmd);
                }
            }
        });
        if let Some(i) = select {
            if i != self.docs[self.cur].active {
                self.commit_pending();
                self.docs[self.cur].active = i;
            }
        }
        if let Some(i) = toggle {
            self.commit_pending();
            let doc = &mut self.docs[self.cur];
            let before = doc.snapshot_layer(i);
            doc.layers[i].visible = !doc.layers[i].visible;
            let name = if doc.layers[i].visible { "Layer Shown" } else { "Layer Hidden" };
            doc.push_history(name, before);
        }
        if props {
            cmds.push(Cmd::LayerProperties);
        }
    }
}

#[allow(dead_code)]
fn _unused(_p: &Pending) {}
