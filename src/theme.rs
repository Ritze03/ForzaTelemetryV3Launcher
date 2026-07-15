//! "Graphite" theme — the subset of ForzaTelemetryV3's styling the launcher uses.
//! Colours are referenced by role token (ACCENT, PANEL, DIM…), never hard-coded at
//! call sites. See ForzaTelemetryV3/docs/ui/STYLING-GUIDE.md for the full system.

use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextStyle};

// ---- Brand / chrome ------------------------------------------------------
pub const ACCENT: Color32 = Color32::from_rgb(0x5B, 0x8B, 0xF0);
/// Title-bar / top-panel background (darker than panels).
pub const HEAD: Color32 = Color32::from_rgb(0x14, 0x17, 0x1A);
pub const PANEL: Color32 = Color32::from_rgb(0x1E, 0x21, 0x25);
pub const BORDER: Color32 = Color32::from_rgb(0x2C, 0x30, 0x36);
pub const TEXT: Color32 = Color32::from_rgb(0xE7, 0xE9, 0xEC);
pub const DIM: Color32 = Color32::from_rgb(0x96, 0x9C, 0xA6);
pub const FIELD: Color32 = Color32::from_rgb(0x15, 0x18, 0x1B);
pub const BTN: Color32 = Color32::from_rgb(0x26, 0x2A, 0x30);
pub const BTNBD: Color32 = Color32::from_rgb(0x34, 0x39, 0x41);
pub const PRIMARY_TEXT: Color32 = Color32::from_rgb(0x0B, 0x12, 0x22);
pub const DANGER: Color32 = Color32::from_rgb(0xE1, 0x55, 0x54);

// Selection / hover tints (premultiplied — const-friendly).
const SEL: Color32 = Color32::from_rgba_premultiplied(0x0F, 0x16, 0x27, 0x29);
const SELBD: Color32 = Color32::from_rgba_premultiplied(0x26, 0x3A, 0x65, 0x6B);
const HOV: Color32 = Color32::from_rgba_premultiplied(0x0D, 0x0D, 0x0D, 0x0D);

/// An UPPERCASE section label in the accent colour.
pub fn section_label(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).color(ACCENT).size(12.0).strong()
}

/// A bordered card with a blue [`section_label`] title, then a uniform 8px gap.
/// Callers stacking cards must zero `item_spacing.y` first (the card owns the gap).
pub fn card(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(section_label(title));
        ui.add_space(4.0);
        body(ui);
    });
    ui.add_space(8.0);
}

/// Primary action: solid accent fill, dark bold text.
pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).color(PRIMARY_TEXT).strong())
        .fill(ACCENT)
        .stroke(Stroke::new(1.0, ACCENT))
}

/// Destructive: transparent fill, red text + faint red border.
pub fn danger_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).color(DANGER))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(0xE1, 0x55, 0x54, 82)))
}

/// Install the Geist font family. Geist Mono (TTF) for text, Nerd Font (OTF) for
/// icon glyphs only. Licences live in assets/fonts/. Call once at startup.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "geist_mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/GeistMono-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "geist_icons".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/GeistMonoNerdFont-Regular.otf"))
            .into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let fam = fonts.families.entry(family).or_default();
        fam.insert(0, "geist_mono".to_owned());
        fam.push("geist_icons".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Install the Graphite visuals + type scale. Call once at startup.
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = HOV;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = SEL;
    v.selection.stroke = Stroke::new(1.0, SELBD);

    let round = egui::Rounding::same(7.0);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, DIM);
    v.widgets.noninteractive.rounding = round;
    v.widgets.inactive.bg_fill = BTN;
    v.widgets.inactive.weak_bg_fill = BTN;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BTNBD);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.rounding = round;
    v.widgets.hovered.bg_fill = BTN;
    v.widgets.hovered.weak_bg_fill = HOV;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.rounding = round;
    v.widgets.active.bg_fill = BTN;
    v.widgets.active.weak_bg_fill = HOV;
    v.widgets.active.bg_stroke = Stroke::new(1.0, SELBD);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.rounding = round;
    v.widgets.open.bg_fill = FIELD;
    v.widgets.open.weak_bg_fill = FIELD;
    v.widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.open.rounding = round;
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        use egui::FontFamily::{Monospace, Proportional};
        s.text_styles = [
            (TextStyle::Heading, FontId::new(18.0, Proportional)),
            (TextStyle::Body, FontId::new(13.0, Proportional)),
            (TextStyle::Button, FontId::new(13.0, Proportional)),
            (TextStyle::Small, FontId::new(11.0, Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, Monospace)),
        ]
        .into();
        s.spacing.button_padding = egui::vec2(9.0, 5.0);
        s.spacing.interact_size.y = 22.0;
    });
}
