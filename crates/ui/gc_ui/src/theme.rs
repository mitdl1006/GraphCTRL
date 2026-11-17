//! Colours and widget style.

/// The palette. Sketch colours follow the convention CAD users know: blue geometry can still move, black
/// geometry is fully constrained.
pub mod colors {
    use egui::Color32;

    /// Sketch geometry that can still move.
    pub const FREE: Color32 = Color32::from_rgb(0x10, 0x40, 0xE0);
    /// Sketch geometry that is fully constrained.
    pub const CONSTRAINED: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
    /// Geometry of sketches that are not being edited.
    pub const INACTIVE: Color32 = Color32::from_rgb(0x60, 0x68, 0x72);
    /// Construction geometry.
    pub const CONSTRUCTION: Color32 = Color32::from_rgb(0xB0, 0x70, 0x20);
    /// Selected items.
    pub const SELECTED: Color32 = Color32::from_rgb(0x1E, 0x90, 0xFF);
    /// Tool previews and snap markers.
    pub const PREVIEW: Color32 = Color32::from_rgb(0xFF, 0x80, 0x00);
    /// Driving dimensions.
    pub const DIMENSION: Color32 = Color32::from_rgb(0x20, 0x20, 0x20);
    /// Reference dimensions.
    pub const REFERENCE: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
    /// Constraint glyphs.
    pub const GLYPH: Color32 = Color32::from_rgb(0x1B, 0x7F, 0x3B);
    /// Errors in the status bar.
    pub const ERROR: Color32 = Color32::from_rgb(0xC0, 0x20, 0x20);
    /// Top and bottom of the viewport background gradient.
    pub const BACKGROUND_TOP: Color32 = Color32::from_rgb(0xE6, 0xEA, 0xEE);
    /// Bottom of the viewport background gradient.
    pub const BACKGROUND_BOTTOM: Color32 = Color32::from_rgb(0xC3, 0xCA, 0xD3);
    /// Minor grid lines.
    pub const GRID: Color32 = Color32::from_rgb(0xB4, 0xBC, 0xC6);
    /// Major grid lines.
    pub const GRID_MAJOR: Color32 = Color32::from_rgb(0x98, 0xA2, 0xAE);
    /// The ribbon's background.
    pub const RIBBON: Color32 = Color32::from_rgb(0xF3, 0xF4, 0xF6);
}

/// Applies the light theme.
pub fn install(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = colors::RIBBON;
    visuals.selection.bg_fill = egui::Color32::from_rgb(0xCF, 0xE4, 0xFA);
    visuals.selection.stroke = egui::Stroke::new(1.0, colors::SELECTED);
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
    });
}
