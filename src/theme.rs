//! Look and feel: light and dark themes, UI font selection, compact tool-window headers and
//! the small persisted preferences file.

use egui::{pos2, vec2, Color32, CornerRadius, Margin, Rect, RichText, Sense, Shadow, Stroke, Ui};

/// Colours used by custom-painted widgets, derived from the current theme.
#[derive(Clone, Copy)]
pub struct Pal {
    pub dark: bool,
    pub panel: Color32,
    pub bar: Color32,
    pub window: Color32,
    pub header: Color32,
    pub border: Color32,
    pub canvas: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub hover: Color32,
    pub field: Color32,
}

pub fn pal(dark: bool) -> Pal {
    if dark {
        Pal {
            dark,
            panel: Color32::from_rgb(43, 45, 49),
            bar: Color32::from_rgb(37, 39, 43),
            window: Color32::from_rgb(50, 52, 57),
            header: Color32::from_rgb(40, 42, 46),
            border: Color32::from_rgb(28, 29, 32),
            canvas: Color32::from_rgb(30, 31, 34),
            text: Color32::from_rgb(224, 226, 230),
            weak: Color32::from_rgb(150, 154, 162),
            accent: Color32::from_rgb(77, 144, 254),
            accent_soft: Color32::from_rgb(44, 72, 112),
            hover: Color32::from_rgb(64, 67, 74),
            field: Color32::from_rgb(33, 35, 38),
        }
    } else {
        Pal {
            dark,
            panel: Color32::from_rgb(240, 242, 246),
            bar: Color32::from_rgb(248, 249, 251),
            window: Color32::from_rgb(252, 252, 253),
            header: Color32::from_rgb(234, 237, 242),
            border: Color32::from_rgb(196, 201, 210),
            canvas: Color32::from_rgb(196, 200, 208),
            text: Color32::from_rgb(28, 30, 34),
            weak: Color32::from_rgb(110, 115, 124),
            accent: Color32::from_rgb(38, 117, 220),
            accent_soft: Color32::from_rgb(207, 225, 250),
            hover: Color32::from_rgb(225, 232, 243),
            field: Color32::from_rgb(255, 255, 255),
        }
    }
}

/// Current palette for a UI.
pub fn ui_pal(ui: &Ui) -> Pal {
    pal(ui.visuals().dark_mode)
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    let p = pal(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let r4 = CornerRadius::same(4);
    v.panel_fill = p.panel;
    v.window_fill = p.window;
    v.faint_bg_color = if dark { Color32::from_rgb(47, 49, 54) } else { Color32::from_rgb(245, 246, 249) };
    v.extreme_bg_color = p.field;
    v.code_bg_color = p.header;
    v.window_stroke = Stroke::new(1.0f32, p.border);
    v.window_corner_radius = CornerRadius::same(7);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_shadow = Shadow { offset: [0, 4], blur: 18, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 38 }) };
    v.popup_shadow = Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 40 }) };
    v.window_highlight_topmost = false;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0f32, if dark { Color32::from_rgb(220, 232, 255) } else { Color32::from_rgb(12, 52, 110) });
    v.hyperlink_color = p.accent;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.resize_corner_size = 10.0;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.window;
    w.noninteractive.weak_bg_fill = p.window;
    w.noninteractive.bg_stroke = Stroke::new(1.0f32, if dark { Color32::from_rgb(62, 65, 71) } else { Color32::from_rgb(218, 222, 229) });
    w.noninteractive.fg_stroke = Stroke::new(1.0f32, p.text);
    w.noninteractive.corner_radius = r4;

    let (inactive, inactive_stroke) =
        if dark { (Color32::from_rgb(58, 61, 67), Color32::from_rgb(70, 73, 80)) } else { (Color32::from_rgb(232, 235, 240), Color32::from_rgb(205, 210, 218)) };
    w.inactive.bg_fill = inactive;
    w.inactive.weak_bg_fill = inactive;
    w.inactive.bg_stroke = Stroke::new(1.0f32, inactive_stroke);
    w.inactive.fg_stroke = Stroke::new(1.0f32, p.text);
    w.inactive.corner_radius = r4;

    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.hover;
    w.hovered.bg_stroke = Stroke::new(1.0f32, p.accent.gamma_multiply(0.7));
    w.hovered.fg_stroke = Stroke::new(1.5f32, p.text);
    w.hovered.corner_radius = r4;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.accent_soft;
    w.active.weak_bg_fill = p.accent_soft;
    w.active.bg_stroke = Stroke::new(1.0f32, p.accent);
    w.active.fg_stroke = Stroke::new(2.0f32, p.text);
    w.active.corner_radius = r4;
    w.active.expansion = 0.0;

    w.open.bg_fill = p.accent_soft;
    w.open.weak_bg_fill = p.accent_soft;
    w.open.bg_stroke = Stroke::new(1.0f32, p.accent);
    w.open.fg_stroke = Stroke::new(1.0f32, p.text);
    w.open.corner_radius = r4;

    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        use egui::{FontFamily::Proportional, FontId, TextStyle};
        s.text_styles = [
            (TextStyle::Small, FontId::new(11.0, Proportional)),
            (TextStyle::Body, FontId::new(13.0, Proportional)),
            (TextStyle::Button, FontId::new(13.0, Proportional)),
            (TextStyle::Heading, FontId::new(15.0, Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, egui::FontFamily::Monospace)),
        ]
        .into();
        s.spacing.item_spacing = vec2(6.0, 5.0);
        s.spacing.button_padding = vec2(7.0, 3.0);
        s.spacing.interact_size = vec2(36.0, 22.0);
        s.spacing.menu_margin = Margin::symmetric(6, 5);
        s.spacing.window_margin = Margin::same(8);
        s.spacing.slider_width = 170.0;
        s.spacing.combo_height = 320.0;
        s.interaction.selectable_labels = false;
    });
}

// ----------------------------------------------------------------------------------------------
// Fonts

/// Native UI fonts at regular weight (Segoe UI on Windows, Ubuntu on Ubuntu), falling back to
/// the bundled fonts. The bundled monospace font is appended so arrows and symbols always render.
pub fn ui_fonts() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    let candidates: &[&str] = &[
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        "/usr/share/fonts/truetype/ubuntu/Ubuntu[wdth,wght].ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    if let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) {
        defs.font_data.insert("system-ui".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        defs.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "system-ui".into());
    }
    let mono = defs.families.get(&egui::FontFamily::Monospace).cloned().unwrap_or_default();
    let prop = defs.families.entry(egui::FontFamily::Proportional).or_default();
    for name in mono {
        if !prop.contains(&name) {
            prop.push(name);
        }
    }
    defs
}

// ----------------------------------------------------------------------------------------------
// Preferences

fn settings_path() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from).map(|p| p.join("Ferrum Paint"));
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|p| p.join("ferrum-paint"));
    base.map(|b| b.join("settings.ini"))
}

/// Saved theme preference (`None` = not set yet).
pub fn load_dark_pref() -> Option<bool> {
    let text = std::fs::read_to_string(settings_path()?).ok()?;
    text.lines().find_map(|l| l.strip_prefix("theme=")).map(|v| v.trim() == "dark")
}

pub fn save_dark_pref(dark: bool) {
    if let Some(p) = settings_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("theme={}\n", if dark { "dark" } else { "light" }));
    }
}

// ----------------------------------------------------------------------------------------------
// Tool windows

/// Show a tool window with a compact left-aligned header and close button, like a docked
/// palette. The window can be dragged by its header. Returns false when the user closes it.
pub fn tool_window(ctx: &egui::Context, id: egui::Id, title: &str, cfg: impl FnOnce(egui::Window) -> egui::Window, add: impl FnOnce(&mut Ui)) -> bool {
    let p = pal(ctx.style().visuals.dark_mode);
    let frame = egui::Frame::window(&ctx.style()).inner_margin(Margin { left: 7, right: 7, top: 0, bottom: 7 });
    let mut keep = true;
    let win = egui::Window::new(title).id(id).title_bar(false).collapsible(false).frame(frame);
    cfg(win).show(ctx, |ui| {
        let bg = ui.painter().add(egui::Shape::Noop);
        let hr = ui
            .horizontal(|ui| {
                ui.set_height(25.0);
                ui.add(egui::Label::new(RichText::new(title).size(12.5).strong().color(p.text)).selectable(false));
            })
            .response
            .rect;
        ui.add_space(3.0);
        add(ui);
        let full = ui.min_rect();
        let header = Rect::from_min_max(pos2(full.min.x - 7.0, hr.min.y), pos2(full.max.x + 7.0, hr.max.y));
        ui.painter().set(bg, egui::epaint::RectShape::filled(header, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, p.header));
        ui.painter().hline(header.x_range(), header.max.y, Stroke::new(1.0f32, ui.visuals().widgets.noninteractive.bg_stroke.color));
        let cb = Rect::from_center_size(pos2(header.max.x - 15.0, header.center().y), vec2(18.0, 18.0));
        let r = ui.interact(cb, id.with("close"), Sense::click());
        if r.hovered() {
            ui.painter().rect_filled(cb, 4.0, if p.dark { Color32::from_rgb(196, 64, 64) } else { Color32::from_rgb(232, 90, 90) });
        }
        let c = if r.hovered() { Color32::WHITE } else { p.weak };
        let d = 4.0;
        ui.painter().line_segment([cb.center() + vec2(-d, -d), cb.center() + vec2(d, d)], Stroke::new(1.5f32, c));
        ui.painter().line_segment([cb.center() + vec2(d, -d), cb.center() + vec2(-d, d)], Stroke::new(1.5f32, c));
        if r.on_hover_text("Close").clicked() {
            keep = false;
        }
    });
    keep
}
