//! Dark modern look modelled on the reference screenshots (Logic Pro iPad/Touch-style chrome).
use egui::{Color32, CornerRadius, Stroke};

pub const BG: Color32 = Color32::from_rgb(0x12, 0x13, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x22);
pub const GRID_BG: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x13);
pub const TEXT: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf5);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x8a, 0x8d, 0x98);
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(0x00, 0x7a, 0xff);
pub const NEON_GREEN: Color32 = Color32::from_rgb(0x34, 0xc7, 0x59);
pub const VIBRANT_RED: Color32 = Color32::from_rgb(0xff, 0x3b, 0x30);
pub const PURPLE_BASS: Color32 = Color32::from_rgb(0xaf, 0x52, 0xde);
pub const CYAN_KEYS: Color32 = Color32::from_rgb(0x00, 0xc7, 0xda);
pub const MAGENTA_VOCALS: Color32 = Color32::from_rgb(0xff, 0x2d, 0x55);
pub const ORANGE_FX: Color32 = Color32::from_rgb(0xff, 0x80, 0x00);

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = GRID_BG;
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.inactive.bg_fill = Color32::from_rgb(0x28, 0x29, 0x32);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x28, 0x29, 0x32);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x38, 0x3a, 0x46);
    v.widgets.active.weak_bg_fill = ACCENT_BLUE;
    v.selection.bg_fill = ACCENT_BLUE;
    let r = CornerRadius::same(6);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x18, 0x19, 0x20));
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    });
}
