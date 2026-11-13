//! Modern Dark look modelled on BandLab Studio design tokens.
use egui::{Color32, CornerRadius, Stroke};

// BandLab Surfaces
pub const BG: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);         // Canvas base
pub const PANEL: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1a);      // Sidebars, mixer, containers
pub const CARD_BG: Color32 = Color32::from_rgb(0x22, 0x22, 0x24);    // Elevated cards
pub const GRID_BG: Color32 = Color32::from_rgb(0x16, 0x16, 0x18);    // Arranger timeline background
pub const TEXT: Color32 = Color32::from_rgb(0xe5, 0xe5, 0xe5);       // High emphasis text
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xa0, 0xa0, 0xa0); // Secondary text/icons

// BandLab Brand & Transport Accents
pub const BANDLAB_RED: Color32 = Color32::from_rgb(0xf1, 0x2c, 0x18); // Record / Primary Brand CTA
pub const PLAY_GREEN: Color32 = Color32::from_rgb(0x00, 0xc3, 0x7d);  // BandLab Mint Green (Play active)
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(0x2f, 0x93, 0xf5); // BandLab Electric Blue

// BandLab Official Track Preset Palette
pub const TRACK_BLUE: Color32 = Color32::from_rgb(0x2f, 0x93, 0xf5);   // Drums
pub const TRACK_PURPLE: Color32 = Color32::from_rgb(0xb3, 0x01, 0xff); // Bass
pub const TRACK_GREEN: Color32 = Color32::from_rgb(0x00, 0xc3, 0x7d);  // Guitars
pub const TRACK_CYAN: Color32 = Color32::from_rgb(0x00, 0xc6, 0xd1);   // Keys / Synth
pub const TRACK_RED: Color32 = Color32::from_rgb(0xf1, 0x2d, 0x19);    // Vocals
pub const TRACK_ORANGE: Color32 = Color32::from_rgb(0xfe, 0x69, 0x0d); // FX
pub const TRACK_GOLD: Color32 = Color32::from_rgb(0xff, 0xaf, 0x14);   // Aux / Bus

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = GRID_BG;
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.inactive.bg_fill = CARD_BG;
    v.widgets.inactive.weak_bg_fill = CARD_BG;
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x32, 0x32, 0x38);
    v.widgets.active.weak_bg_fill = ACCENT_BLUE;
    v.selection.bg_fill = ACCENT_BLUE;
    let r = CornerRadius::same(6);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x28, 0x28, 0x2e));
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
