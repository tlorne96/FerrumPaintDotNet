//! Application state, command dispatch, keyboard shortcuts and menus.

use crate::canvas::{zoom_step, DragInfo, Pending};
use crate::dialogs::Dialog;
use crate::document::Document;
use crate::fileio;
use crate::raster::{self, Col, Filter, Orient};
use crate::tools::{Tool, ToolSettings, BRUSH_PRESETS};
use egui::{Key, Modifiers, Rect};
use std::path::PathBuf;

pub const APP_NAME: &str = "Ferrum Paint";

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    New,
    Open,
    Save,
    SaveAs,
    SaveAll,
    Close,
    Exit,
    Undo,
    Redo,
    Cut,
    Copy,
    CopyMerged,
    Paste,
    PasteNewLayer,
    PasteNewImage,
    EraseSelection,
    FillSelection(bool),
    InvertSelection,
    SelectAll,
    Deselect,
    ZoomIn,
    ZoomOut,
    ZoomWindow,
    ZoomSelection,
    ActualSize,
    ToggleGrid,
    Crop,
    Resize,
    CanvasSize,
    Orient(Orient),
    Flatten,
    AddLayer,
    DeleteLayer,
    DuplicateLayer,
    MergeDown,
    ImportLayer,
    LayerOrient(Orient),
    RotateZoom,
    ToggleLayerVisibility,
    SelectLayer(i32),
    MoveLayer(i32),
    LayerProperties,
    Filter(Filter),
    RepeatEffect,
    ToggleWindow(u8),
    ResetWindow(u8),
    About,
    Shortcuts,
    NextImage(i32),
    SwitchImage(usize),
    Finish,
    SwapColors,
}

pub struct App {
    pub docs: Vec<Document>,
    pub cur: usize,
    pub tool: Tool,
    pub prev_tool: Tool,
    pub ts: ToolSettings,
    pub primary: Col,
    pub secondary: Col,
    pub active_slot_secondary: bool,
    pub palette: Vec<Col>,
    pub pending: Pending,
    pub pending_dirty: bool,
    pub drag: Option<DragInfo>,
    pub clone_source: Option<egui::Pos2>,
    pub show: [bool; 4],
    pub window_gen: [u32; 4],
    pub show_grid: bool,
    pub dialog: Option<Dialog>,
    pub last_effect: Option<Filter>,
    pub status_pos: Option<(i32, i32)>,
    pub status_msg: String,
    pub checker: Option<egui::TextureHandle>,
    pub clipboard: Option<arboard::Clipboard>,
    pub space_down: bool,
    pub canvas_rect: Rect,
    pub last_sig: String,
    pub quitting: bool,
    pub allow_close: bool,
    pub colors_more: bool,
    pub hex_edit: String,
    pub hsv: (f32, f32, f32),
    pub hsv_for: Col,
    pub last_title: String,
    pub untitled_counter: u32,
    pub last_new_size: (u32, u32),
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_visuals(egui::Visuals::light());
        ctx.set_fonts(ui_fonts());
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(6.0, 4.0);
            s.spacing.button_padding = egui::vec2(6.0, 2.0);
        });
        Self::with_files(files)
    }

    /// Construct the application state without a UI context (also used by tests).
    pub fn with_files(files: Vec<PathBuf>) -> Self {
        let mut app = App {
            docs: vec![],
            cur: 0,
            tool: Tool::Paintbrush,
            prev_tool: Tool::Paintbrush,
            ts: ToolSettings::default(),
            primary: Col::BLACK,
            secondary: Col::WHITE,
            active_slot_secondary: false,
            palette: default_palette(),
            pending: Pending::None,
            pending_dirty: false,
            drag: None,
            clone_source: None,
            show: [true; 4],
            window_gen: [0; 4],
            show_grid: false,
            dialog: None,
            last_effect: None,
            status_pos: None,
            status_msg: String::new(),
            checker: None,
            clipboard: None,
            space_down: false,
            canvas_rect: Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            last_sig: String::new(),
            quitting: false,
            allow_close: false,
            colors_more: false,
            hex_edit: String::new(),
            hsv: (0.0, 0.0, 0.0),
            hsv_for: Col::TRANSPARENT,
            last_title: String::new(),
            untitled_counter: 1,
            last_new_size: (800, 600),
        };
        for f in files {
            app.open_path(&f);
        }
        if app.docs.is_empty() {
            app.new_document(800, 600);
        }
        app
    }

    pub fn new_document(&mut self, w: u32, h: u32) {
        self.commit_pending();
        let name = format!("Untitled{}", if self.untitled_counter > 1 { format!(" {}", self.untitled_counter) } else { String::new() });
        self.untitled_counter += 1;
        self.docs.push(Document::new(name, w, h, Some(Col::WHITE)));
        self.cur = self.docs.len() - 1;
        self.last_new_size = (w, h);
    }

    pub fn open_path(&mut self, p: &std::path::Path) {
        match fileio::load(p) {
            Ok(doc) => {
                self.commit_pending();
                self.docs.push(doc);
                self.cur = self.docs.len() - 1;
            }
            Err(e) => self.dialog = Some(Dialog::Message(format!("Could not open {}:\n{}", p.display(), e))),
        }
    }

    pub fn set_tool(&mut self, t: Tool) {
        if t == self.tool {
            return;
        }
        self.commit_pending();
        if t != Tool::ColorPicker {
            // remember the previous tool for the picker's "switch back" option
        }
        self.prev_tool = self.tool;
        self.tool = t;
        self.status_msg.clear();
    }

    pub fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    fn msg(&mut self, s: impl Into<String>) {
        self.dialog = Some(Dialog::Message(s.into()));
    }

    // ------------------------------------------------------------------------------------------
    // Commands

    pub fn run(&mut self, ctx: &egui::Context, cmd: Cmd) {
        // Commands that don't need a document
        match &cmd {
            Cmd::New => {
                self.commit_pending();
                let (w, h) = self
                    .clipboard()
                    .and_then(|c| c.get_image().ok())
                    .map(|i| (i.width as u32, i.height as u32))
                    .unwrap_or(self.last_new_size);
                self.dialog = Some(Dialog::new_image(w, h));
                return;
            }
            Cmd::Open => {
                let files = rfd::FileDialog::new()
                    .add_filter("All images", &["ferrum", "png", "jpg", "jpeg", "bmp", "gif", "tga", "tif", "tiff", "webp", "ico"])
                    .add_filter("Ferrum Paint image", &["ferrum"])
                    .add_filter("All files", &["*"])
                    .pick_files();
                for f in files.unwrap_or_default() {
                    self.open_path(&f);
                }
                return;
            }
            Cmd::Exit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            Cmd::ToggleWindow(i) => {
                self.show[*i as usize] = !self.show[*i as usize];
                return;
            }
            Cmd::ResetWindow(i) => {
                self.window_gen[*i as usize] += 1;
                self.show[*i as usize] = true;
                return;
            }
            Cmd::About => {
                self.dialog = Some(Dialog::About);
                return;
            }
            Cmd::Shortcuts => {
                self.dialog = Some(Dialog::Shortcuts);
                return;
            }
            Cmd::SwapColors => {
                std::mem::swap(&mut self.primary, &mut self.secondary);
                return;
            }
            Cmd::PasteNewImage => {
                self.commit_pending();
                match self.clipboard_image() {
                    Some(img) => {
                        let (w, h) = (img.width(), img.height());
                        self.new_document(w, h);
                        let doc = &mut self.docs[self.cur];
                        doc.layers[0].pixmap.fill(tiny_skia::Color::TRANSPARENT);
                        self.begin_paste(img, 0, 0, "Paste");
                    }
                    None => self.msg("The clipboard does not contain an image."),
                }
                return;
            }
            _ => {}
        }
        if self.docs.is_empty() {
            return;
        }
        // Commands that keep pending edits alive
        match &cmd {
            Cmd::ZoomIn => {
                let z = zoom_step(self.docs[self.cur].view.zoom, 1);
                self.set_zoom(z, None);
                return;
            }
            Cmd::ZoomOut => {
                let z = zoom_step(self.docs[self.cur].view.zoom, -1);
                self.set_zoom(z, None);
                return;
            }
            Cmd::ActualSize => {
                self.set_zoom(1.0, None);
                return;
            }
            Cmd::ZoomWindow => {
                let doc = &mut self.docs[self.cur];
                if let Some((z, c)) = doc.view.prev_zoom.take() {
                    doc.view.zoom = z;
                    doc.view.center = c;
                } else {
                    let prev = (doc.view.zoom, doc.view.center);
                    self.zoom_to_window();
                    self.docs[self.cur].view.prev_zoom = Some(prev);
                }
                return;
            }
            Cmd::ZoomSelection => {
                if let Some((x0, y0, x1, y1)) = self.docs[self.cur].sel_bounds {
                    self.zoom_to_rect(x0 as f32, y0 as f32, x1 as f32, y1 as f32);
                }
                return;
            }
            Cmd::ToggleGrid => {
                self.show_grid = !self.show_grid;
                return;
            }
            _ => {}
        }
        // Everything else commits the current edit first.
        let had_pending = self.pending.is_editable();
        self.commit_pending();
        let secondary = self.secondary;
        let primary = self.primary;
        match cmd {
            Cmd::Save => { self.save(self.cur, false); }
            Cmd::SaveAs => { self.save(self.cur, true); }
            Cmd::SaveAll => {
                for i in 0..self.docs.len() {
                    if self.docs[i].is_dirty() {
                        self.save(i, false);
                    }
                }
            }
            Cmd::Close => self.request_close(self.cur),
            Cmd::Undo => {
                if !had_pending {
                    self.docs[self.cur].undo();
                }
            }
            Cmd::Redo => {
                self.docs[self.cur].redo();
            }
            Cmd::Copy | Cmd::CopyMerged | Cmd::Cut => {
                let doc = &self.docs[self.cur];
                let src = if cmd == Cmd::CopyMerged { doc.composite() } else { doc.layers[doc.active].pixmap.clone() };
                if let Some(p) = doc.extract_selected(&src) {
                    let (w, h) = (p.width() as usize, p.height() as usize);
                    let bytes = raster::pixmap_to_straight(&p);
                    if let Some(cb) = self.clipboard() {
                        if let Err(e) = cb.set_image(arboard::ImageData { width: w, height: h, bytes: bytes.into() }) {
                            self.msg(format!("Could not copy to the clipboard: {e}"));
                            return;
                        }
                    }
                    if cmd == Cmd::Cut {
                        let doc = &mut self.docs[self.cur];
                        doc.erase_selection();
                        if let Some(e) = doc.undo.last_mut() {
                            e.name = "Cut".into();
                        }
                    }
                }
            }
            Cmd::Paste | Cmd::PasteNewLayer => {
                let Some(img) = self.clipboard_image() else {
                    self.msg("The clipboard does not contain an image.");
                    return;
                };
                let doc = &self.docs[self.cur];
                if img.width() > doc.width || img.height() > doc.height {
                    self.dialog = Some(Dialog::PasteExpand { img, new_layer: cmd == Cmd::PasteNewLayer });
                    return;
                }
                if cmd == Cmd::PasteNewLayer {
                    self.docs[self.cur].add_layer();
                }
                let (ox, oy) = self.paste_origin(img.width(), img.height());
                let name = if cmd == Cmd::PasteNewLayer { "Paste Into New Layer" } else { "Paste" };
                self.begin_paste(img, ox, oy, name);
            }
            Cmd::EraseSelection => self.docs[self.cur].erase_selection(),
            Cmd::FillSelection(sec) => self.docs[self.cur].fill_selection(if sec { secondary } else { primary }),
            Cmd::InvertSelection => self.docs[self.cur].invert_selection(),
            Cmd::SelectAll => self.docs[self.cur].select_all(),
            Cmd::Deselect => {
                if !had_pending {
                    self.docs[self.cur].deselect();
                }
            }
            Cmd::Finish => {
                if !had_pending {
                    self.docs[self.cur].deselect();
                }
            }
            Cmd::Crop => self.docs[self.cur].crop_to_selection(),
            Cmd::Resize => {
                let d = &self.docs[self.cur];
                self.dialog = Some(Dialog::resize(d.width, d.height));
            }
            Cmd::CanvasSize => {
                let d = &self.docs[self.cur];
                self.dialog = Some(Dialog::canvas(d.width, d.height));
            }
            Cmd::Orient(o) => self.docs[self.cur].orient_image(o),
            Cmd::Flatten => self.docs[self.cur].flatten(),
            Cmd::AddLayer => self.docs[self.cur].add_layer(),
            Cmd::DeleteLayer => self.docs[self.cur].delete_layer(),
            Cmd::DuplicateLayer => self.docs[self.cur].duplicate_layer(),
            Cmd::MergeDown => self.docs[self.cur].merge_down(),
            Cmd::ImportLayer => {
                if let Some(f) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "tga", "tif", "tiff", "webp", "ico"])
                    .pick_file()
                {
                    match fileio::load_flat(&f) {
                        Ok(px) => {
                            let doc = &mut self.docs[self.cur];
                            doc.add_layer();
                            let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            doc.layers[doc.active].name = name;
                            self.begin_paste(px, 0, 0, "Import From File");
                        }
                        Err(e) => self.msg(e),
                    }
                }
            }
            Cmd::LayerOrient(o) => self.docs[self.cur].orient_layer(o),
            Cmd::RotateZoom => {
                let doc = &self.docs[self.cur];
                self.dialog = Some(Dialog::rotate_zoom(doc));
            }
            Cmd::ToggleLayerVisibility => {
                let doc = &mut self.docs[self.cur];
                let before = doc.snapshot_layer(doc.active);
                let a = doc.active;
                doc.layers[a].visible = !doc.layers[a].visible;
                doc.push_history("Layer Properties", before);
            }
            Cmd::SelectLayer(d) => {
                let doc = &mut self.docs[self.cur];
                let n = doc.layers.len() as i32;
                doc.active = match d {
                    i32::MAX => n - 1,
                    i32::MIN => 0,
                    _ => (doc.active as i32 + d).clamp(0, n - 1),
                } as usize;
            }
            Cmd::MoveLayer(d) => {
                let doc = &mut self.docs[self.cur];
                let n = doc.layers.len() as i32;
                let to = match d {
                    i32::MAX => n - 1,
                    i32::MIN => 0,
                    _ => (doc.active as i32 + d).clamp(0, n - 1),
                } as usize;
                doc.move_layer(to);
            }
            Cmd::LayerProperties => {
                let doc = &self.docs[self.cur];
                self.dialog = Some(Dialog::layer_props(doc, doc.active));
            }
            Cmd::Filter(f) => self.start_filter(f),
            Cmd::RepeatEffect => {
                if let Some(f) = self.last_effect.clone() {
                    self.apply_filter_now(&f);
                }
            }
            Cmd::NextImage(d) => {
                let n = self.docs.len() as i32;
                self.cur = ((self.cur as i32 + d).rem_euclid(n)) as usize;
            }
            Cmd::SwitchImage(i) => {
                if i < self.docs.len() {
                    self.cur = i;
                }
            }
            _ => {}
        }
    }

    fn paste_origin(&self, w: u32, h: u32) -> (i32, i32) {
        let doc = &self.docs[self.cur];
        let tl = self.to_image(self.canvas_rect, self.canvas_rect.min);
        let ox = (tl.x.floor() as i32).min(doc.width as i32 - w as i32).max(0);
        let oy = (tl.y.floor() as i32).min(doc.height as i32 - h as i32).max(0);
        (ox, oy)
    }

    pub fn clipboard_image(&mut self) -> Option<tiny_skia::Pixmap> {
        let img = self.clipboard()?.get_image().ok()?;
        if img.width == 0 || img.height == 0 {
            return None;
        }
        Some(raster::pixmap_from_straight(&img.bytes, img.width as u32, img.height as u32))
    }

    pub fn start_filter(&mut self, f: Filter) {
        if f.has_dialog() {
            let doc = &self.docs[self.cur];
            self.dialog = Some(Dialog::filter(doc, f));
        } else {
            self.apply_filter_now(&f);
        }
    }

    pub fn apply_filter_now(&mut self, f: &Filter) {
        let doc = &mut self.docs[self.cur];
        let li = doc.active;
        let before = doc.snapshot_layer(li);
        let base = doc.layers[li].pixmap.clone();
        let adj = raster::apply_filter(f, &base, doc.selection.as_ref());
        let sel = doc.selection.clone();
        raster::masked_mix(&base, &adj, sel.as_ref(), &mut doc.layers[li].pixmap);
        doc.push_history(f.name(), before);
        if f.is_effect() {
            self.last_effect = Some(f.clone());
        }
    }

    pub fn save(&mut self, idx: usize, save_as: bool) -> bool {
        let path = match (&self.docs[idx].path, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => {
                let doc = &self.docs[idx];
                let stem = doc.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or(doc.name.clone());
                let default = if doc.layers.len() > 1 { format!("{stem}.ferrum") } else { format!("{stem}.png") };
                rfd::FileDialog::new()
                    .set_file_name(default)
                    .add_filter("PNG", &["png"])
                    .add_filter("Ferrum Paint image (layers)", &["ferrum"])
                    .add_filter("JPEG", &["jpg", "jpeg"])
                    .add_filter("BMP", &["bmp"])
                    .add_filter("TGA", &["tga"])
                    .add_filter("TIFF", &["tif", "tiff"])
                    .add_filter("WebP (lossless)", &["webp"])
                    .save_file()
            }
        };
        let Some(path) = path else { return false };
        match fileio::save(&self.docs[idx], &path) {
            Ok(()) => {
                let doc = &mut self.docs[idx];
                doc.name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                doc.path = Some(path);
                doc.mark_saved();
                true
            }
            Err(e) => {
                self.msg(format!("Could not save:\n{e}"));
                false
            }
        }
    }

    pub fn request_close(&mut self, idx: usize) {
        if idx >= self.docs.len() {
            return;
        }
        if self.docs[idx].is_dirty() {
            self.dialog = Some(Dialog::ConfirmClose { uid: self.docs[idx].uid });
        } else {
            self.close_doc(idx);
        }
    }

    pub fn close_doc(&mut self, idx: usize) {
        if idx == self.cur {
            self.pending = Pending::None;
            self.drag = None;
        }
        self.docs.remove(idx);
        if self.cur >= self.docs.len() {
            self.cur = self.docs.len().saturating_sub(1);
        } else if idx < self.cur {
            self.cur -= 1;
        }
    }

    // ------------------------------------------------------------------------------------------
    // Keyboard

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return;
        }
        let typing_widget = ctx.wants_keyboard_input();
        let events = ctx.input(|i| i.events.clone());
        let mut cmds = vec![];
        let text_mode = matches!(self.pending, Pending::Text(_));
        for ev in events {
            // Text tool editing captures keyboard input.
            if text_mode && !typing_widget {
                if self.text_key(&ev) {
                    continue;
                }
            }
            let (key, m) = match ev {
                egui::Event::Key { key, pressed: true, modifiers, .. } => (key, modifiers),
                egui::Event::Copy => (Key::C, ctx.input(|i| i.modifiers)),
                egui::Event::Cut => (Key::X, ctx.input(|i| i.modifiers)),
                egui::Event::Paste(_) if !typing_widget => (Key::V, ctx.input(|i| i.modifiers)),
                _ => continue,
            };
            if typing_widget && !m.ctrl {
                continue;
            }
            if let Some(c) = self.map_shortcut(key, m, text_mode) {
                cmds.push(c);
            }
        }
        cmds.dedup();
        for c in cmds {
            self.run(ctx, c);
        }
    }

    pub(crate) fn map_shortcut(&mut self, key: Key, m: Modifiers, text_mode: bool) -> Option<Cmd> {
        let (c, s, a) = (m.ctrl || m.command, m.shift, m.alt);
        use Cmd::*;
        let cmd = match (c, s, a, key) {
            (true, false, false, Key::N) => New,
            (true, false, false, Key::O) => Open,
            (true, false, false, Key::S) => Save,
            (true, true, false, Key::S) => SaveAs,
            (true, false, true, Key::S) => SaveAll,
            (true, false, false, Key::W) => Close,
            (true, false, false, Key::F4) => Close,
            (true, false, false, Key::Q) => Exit,
            (true, false, false, Key::Z) => Undo,
            (true, false, false, Key::Y) => Redo,
            (true, false, false, Key::X) => Cut,
            (false, true, false, Key::Delete) => Cut,
            (true, false, false, Key::C) => Copy,
            (true, false, false, Key::Insert) => Copy,
            (true, true, false, Key::C) => CopyMerged,
            (true, false, false, Key::V) => Paste,
            (false, true, false, Key::Insert) => Paste,
            (true, true, false, Key::V) => PasteNewLayer,
            (true, false, true, Key::V) => PasteNewImage,
            (false, false, false, Key::Delete) => EraseSelection,
            (false, false, false, Key::Backspace) => FillSelection(false),
            (false, true, false, Key::Backspace) => FillSelection(true),
            (true, false, false, Key::I) => InvertSelection,
            (true, false, false, Key::A) => SelectAll,
            (true, false, false, Key::D) => Deselect,
            (false, false, false, Key::Enter) => Finish,
            (false, false, false, Key::Escape) => {
                if self.pending.is_editable() {
                    Finish
                } else {
                    return None;
                }
            }
            (true, _, false, Key::Plus) | (true, _, false, Key::Equals) => ZoomIn,
            (true, false, false, Key::Minus) => ZoomOut,
            (true, false, false, Key::B) => ZoomWindow,
            (true, true, false, Key::B) => ZoomSelection,
            (true, false, false, Key::Num0) => ActualSize,
            (true, false, false, Key::Quote) => ToggleGrid,
            (true, true, false, Key::X) => Crop,
            (true, false, false, Key::R) => Resize,
            (true, true, false, Key::R) => CanvasSize,
            (true, false, false, Key::H) => Cmd::Orient(raster::Orient::Rot90Cw),
            (true, false, false, Key::G) => Cmd::Orient(raster::Orient::Rot90Ccw),
            (true, false, false, Key::J) => Cmd::Orient(raster::Orient::Rot180),
            (true, true, false, Key::F) => Flatten,
            (true, true, false, Key::N) => AddLayer,
            (true, true, false, Key::Delete) => DeleteLayer,
            (true, true, false, Key::D) => DuplicateLayer,
            (true, false, false, Key::M) => MergeDown,
            (true, false, false, Key::Comma) => ToggleLayerVisibility,
            (true, true, false, Key::Z) => RotateZoom,
            (false, false, true, Key::PageUp) => SelectLayer(1),
            (false, false, true, Key::PageDown) => SelectLayer(-1),
            (true, false, true, Key::PageUp) => SelectLayer(i32::MAX),
            (true, false, true, Key::PageDown) => SelectLayer(i32::MIN),
            (false, true, true, Key::PageUp) => MoveLayer(1),
            (false, true, true, Key::PageDown) => MoveLayer(-1),
            (true, true, true, Key::PageUp) => MoveLayer(i32::MAX),
            (true, true, true, Key::PageDown) => MoveLayer(i32::MIN),
            (false, false, false, Key::F4) => LayerProperties,
            (true, true, false, Key::L) => Filter(raster::Filter::AutoLevel),
            (true, true, false, Key::G) => Filter(raster::Filter::BlackAndWhite),
            (true, true, false, Key::T) => Filter(raster::Filter::BrightnessContrast { brightness: 0, contrast: 0 }),
            (true, true, false, Key::M) => Filter(raster::Filter::Curves { points: vec![(0, 0), (255, 255)], luminosity: true }),
            (true, true, false, Key::U) => Filter(raster::Filter::HueSaturation { hue: 0, saturation: 100, lightness: 0 }),
            (true, false, true, Key::I) => Filter(raster::Filter::InvertAlpha),
            (true, true, false, Key::I) => Filter(raster::Filter::InvertColors),
            (true, false, false, Key::L) => Filter(raster::Filter::Levels { in_lo: 0, in_hi: 255, gamma: 1.0, out_lo: 0, out_hi: 255 }),
            (true, true, false, Key::P) => Filter(raster::Filter::Posterize { levels: 16 }),
            (true, true, false, Key::E) => Filter(raster::Filter::Sepia),
            (true, false, false, Key::F) => RepeatEffect,
            (false, false, false, Key::F1) => Shortcuts,
            (false, false, false, Key::F5) => ToggleWindow(0),
            (false, false, false, Key::F6) => ToggleWindow(1),
            (false, false, false, Key::F7) => ToggleWindow(2),
            (false, false, false, Key::F8) => ToggleWindow(3),
            (true, true, false, Key::F5) => ResetWindow(0),
            (true, true, false, Key::F6) => ResetWindow(1),
            (true, true, false, Key::F7) => ResetWindow(2),
            (true, true, false, Key::F8) => ResetWindow(3),
            (true, false, false, Key::Tab) | (true, false, false, Key::PageDown) => NextImage(1),
            (true, true, false, Key::Tab) | (true, false, false, Key::PageUp) => NextImage(-1),
            (true, false, false, k) | (false, false, true, k) if num_key(k).is_some() => SwitchImage(num_key(k).unwrap() - 1),
            _ => {
                if text_mode {
                    return None;
                }
                return self.single_key(key, m);
            }
        };
        Some(cmd)
    }

    /// Unmodified single-key shortcuts: tools, brush width, colour swap, shape cycling, nudging.
    fn single_key(&mut self, key: Key, m: Modifiers) -> Option<Cmd> {
        let (c, s, a) = (m.ctrl || m.command, m.shift, m.alt);
        // arrow nudging (Ctrl = 10px)
        let arrow = match key {
            Key::ArrowLeft => Some(egui::vec2(-1.0, 0.0)),
            Key::ArrowRight => Some(egui::vec2(1.0, 0.0)),
            Key::ArrowUp => Some(egui::vec2(0.0, -1.0)),
            Key::ArrowDown => Some(egui::vec2(0.0, 1.0)),
            _ => None,
        };
        if let Some(d) = arrow {
            if !a {
                let d = if c { d * 10.0 } else { d };
                if self.space_down || !self.nudge(d) {
                    // pan the canvas instead
                    if let Some(doc) = self.doc_mut() {
                        doc.view.center += d * 20.0 / doc.view.zoom;
                        App::clamp_center(doc);
                    }
                }
            }
            return None;
        }
        match key {
            Key::OpenBracket | Key::CloseBracket => {
                let step = if c { 5.0 } else { 1.0 };
                let dir = if key == Key::OpenBracket { -1.0 } else { 1.0 };
                self.ts.brush_width = (self.ts.brush_width + dir * step).clamp(1.0, 2000.0);
                return None;
            }
            Key::PageUp | Key::PageDown if !c && !a => {
                if let Some(doc) = self.doc_mut() {
                    let dy = doc.view.zoom.recip() * 200.0 * if key == Key::PageUp { -1.0 } else { 1.0 };
                    if s {
                        doc.view.center.x += dy;
                    } else {
                        doc.view.center.y += dy;
                    }
                    App::clamp_center(doc);
                }
                return None;
            }
            Key::Home | Key::End if c => {
                if let Some(doc) = self.doc_mut() {
                    doc.view.center = if key == Key::Home { egui::Vec2::ZERO } else { egui::vec2(doc.width as f32, doc.height as f32) };
                }
                return None;
            }
            _ => {}
        }
        if c || a {
            return None;
        }
        // Shapes tool: A cycles shapes. Line/Curve: , . / cycle caps & style.
        if self.tool == Tool::Shapes && key == Key::A {
            self.ts.shape = self.ts.shape.cycle(if s { -1 } else { 1 });
            return None;
        }
        if self.tool == Tool::LineCurve || self.tool == Tool::Shapes {
            match key {
                Key::Comma => {
                    self.ts.start_cap = self.ts.start_cap.cycle();
                    return None;
                }
                Key::Period => {
                    self.ts.dash = self.ts.dash.cycle();
                    return None;
                }
                Key::Slash => {
                    self.ts.end_cap = self.ts.end_cap.cycle();
                    return None;
                }
                _ => {}
            }
        }
        match key {
            Key::X => return Some(Cmd::SwapColors),
            Key::C => {
                self.active_slot_secondary = !self.active_slot_secondary;
                return None;
            }
            _ => {}
        }
        let letter = key_letter(key)?;
        let group = Tool::group(letter);
        if group.is_empty() {
            return None;
        }
        let next = match group.iter().position(|&t| t == self.tool) {
            Some(i) => {
                let n = group.len() as i32;
                group[((i as i32 + if s { -1 } else { 1 }).rem_euclid(n)) as usize]
            }
            None => {
                if s {
                    *group.last().unwrap()
                } else {
                    group[0]
                }
            }
        };
        self.set_tool(next);
        None
    }

    /// Keyboard handling while the Text tool is editing. Returns true when consumed.
    pub(crate) fn text_key(&mut self, ev: &egui::Event) -> bool {
        let Pending::Text(t) = &mut self.pending else { return false };
        let byte_idx = |s: &str, ci: usize| s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len());
        match ev {
            egui::Event::Text(s) => {
                let b = byte_idx(&t.text, t.cursor);
                t.text.insert_str(b, s);
                t.cursor += s.chars().count();
            }
            egui::Event::Paste(s) => {
                let b = byte_idx(&t.text, t.cursor);
                t.text.insert_str(b, s);
                t.cursor += s.chars().count();
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                if modifiers.ctrl && !matches!(key, Key::ArrowLeft | Key::ArrowRight) {
                    return false;
                }
                let n = t.text.chars().count();
                match key {
                    Key::Backspace => {
                        if t.cursor > 0 {
                            let b = byte_idx(&t.text, t.cursor - 1);
                            t.text.remove(b);
                            t.cursor -= 1;
                        }
                    }
                    Key::Delete => {
                        if t.cursor < n {
                            let b = byte_idx(&t.text, t.cursor);
                            t.text.remove(b);
                        }
                    }
                    Key::Enter => {
                        let b = byte_idx(&t.text, t.cursor);
                        t.text.insert(b, '\n');
                        t.cursor += 1;
                    }
                    Key::ArrowLeft => t.cursor = t.cursor.saturating_sub(1),
                    Key::ArrowRight => t.cursor = (t.cursor + 1).min(n),
                    Key::Home => {
                        let chars: Vec<char> = t.text.chars().collect();
                        while t.cursor > 0 && chars[t.cursor - 1] != '\n' {
                            t.cursor -= 1;
                        }
                    }
                    Key::End => {
                        let chars: Vec<char> = t.text.chars().collect();
                        while t.cursor < n && chars[t.cursor] != '\n' {
                            t.cursor += 1;
                        }
                    }
                    Key::Escape => {
                        self.commit_pending();
                        return true;
                    }
                    Key::Space | Key::Tab => {}
                    k if key_letter(*k).is_some() || num_key(*k).is_some() => {}
                    Key::Comma | Key::Period | Key::Slash | Key::Minus | Key::Equals | Key::Plus | Key::OpenBracket | Key::CloseBracket
                    | Key::Semicolon | Key::Quote | Key::Backslash | Key::Backtick => {}
                    _ => return false,
                }
            }
            _ => return false,
        }
        self.pending_dirty = true;
        true
    }

    // ------------------------------------------------------------------------------------------
    // Frame

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.docs.get(self.cur) {
            Some(d) => format!("{}{} - {}", d.name, if d.is_dirty() { "*" } else { "" }, APP_NAME),
            None => APP_NAME.to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            self.commit_pending();
            if self.docs.iter().any(|d| d.is_dirty()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.quitting = true;
            }
        }
        if self.quitting && self.dialog.is_none() {
            if let Some(d) = self.docs.iter().find(|d| d.is_dirty()) {
                self.dialog = Some(Dialog::ConfirmClose { uid: d.uid });
            } else {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for f in files {
            self.open_path(&f);
        }
    }

    pub fn sync_hsv(&mut self) {
        let c = if self.active_slot_secondary { self.secondary } else { self.primary };
        if c != self.hsv_for {
            let (h, s, v) = raster::rgb_to_hsv(c.r, c.g, c.b);
            // keep hue/saturation stable for greys and black
            let h = if s == 0.0 || v == 0.0 { self.hsv.0 } else { h };
            let s = if v == 0.0 { self.hsv.1 } else { s };
            self.hsv = (h, s, v);
            self.hsv_for = c;
            self.hex_edit = c.to_hex();
        }
    }
}

fn key_letter(k: Key) -> Option<char> {
    let s = k.name();
    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase());
        }
    }
    None
}

fn num_key(k: Key) -> Option<usize> {
    Some(match k {
        Key::Num1 => 1,
        Key::Num2 => 2,
        Key::Num3 => 3,
        Key::Num4 => 4,
        Key::Num5 => 5,
        Key::Num6 => 6,
        Key::Num7 => 7,
        Key::Num8 => 8,
        Key::Num9 => 9,
        _ => return None,
    })
}

fn default_palette() -> Vec<Col> {
    let mut v = vec![];
    // greys
    for i in 0..8 {
        let g = (i as f32 / 7.0 * 255.0).round() as u8;
        v.push(Col::rgba(g, g, g, 255));
    }
    // 12 hues in 3 shades + 2 tints
    for &(sat, val) in &[(1.0f32, 1.0f32), (1.0, 0.6), (0.45, 1.0), (1.0, 0.35)] {
        for i in 0..12 {
            let (r, g, b) = raster::hsv_to_rgb(i as f32 * 30.0, sat, val);
            v.push(Col::rgba(r, g, b, 255));
        }
    }
    v
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ctx);
        self.handle_keys(ctx);
        self.sync_hsv();
        self.update_thumbnails(ctx);
        self.top_panels(ctx);
        self.status_bar(ctx);
        egui::CentralPanel::default().frame(egui::Frame::default()).show(ctx, |ui| {
            self.canvas_ui(ui);
        });
        self.floating_windows(ctx);
        self.show_dialog(ctx);
        self.handle_close_request(ctx);
        self.update_title(ctx);
    }
}

pub fn brush_presets() -> &'static [f32] {
    &BRUSH_PRESETS
}

/// egui's default UI font lacks several arrows and symbols used in labels
/// (→ ← ↑ ↓ ⇄ ● ▸ …). Add the bundled monospace font as a final fallback for
/// proportional text so they never render as boxes.
pub fn ui_fonts() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    let mono = defs.families.get(&egui::FontFamily::Monospace).cloned().unwrap_or_default();
    let prop = defs.families.entry(egui::FontFamily::Proportional).or_default();
    for name in mono {
        if !prop.contains(&name) {
            prop.push(name);
        }
    }
    defs
}