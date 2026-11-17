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
    /// The tool-tip trace of a planned path that is up to date.
    pub const TRACE: Color32 = Color32::from_rgb(0x00, 0x8A, 0x4B);
    /// Waypoint markers and their names.
    pub const WAYPOINT: Color32 = Color32::from_rgb(0xB0, 0x1E, 0x8C);
    /// A face of the view cube.
    pub const CUBE_FACE: Color32 = Color32::from_rgb(0xF4, 0xF5, 0xF7);
    /// The part of the view cube under the cursor.
    pub const CUBE_HOVER: Color32 = Color32::from_rgb(0xB5, 0xD8, 0xFF);
    /// Edges and labels of the view cube.
    pub const CUBE_EDGE: Color32 = Color32::from_rgb(0x5E, 0x66, 0x72);
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
    pub const RIBBON: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// The side panels' background.
    pub const SURFACE: Color32 = Color32::from_rgb(0xF6, 0xF7, 0xF9);
    /// Text fields, cards and other raised surfaces.
    pub const CARD: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Hairlines and widget borders.
    pub const BORDER: Color32 = Color32::from_rgb(0xDF, 0xE3, 0xE8);
    /// The accent: active tabs, section bars, focused fields.
    pub const ACCENT: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    /// Body text.
    pub const TEXT: Color32 = Color32::from_rgb(0x1F, 0x23, 0x28);
    /// Labels and notes.
    pub const MUTED: Color32 = Color32::from_rgb(0x6B, 0x72, 0x80);
}

/// Applies the light theme: white raised surfaces on a light grey ground, hairline borders, rounded
/// corners, one blue accent.
pub fn install(ctx: &egui::Context) {
    use egui::{Color32, CornerRadius, Stroke};
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = colors::SURFACE;
    visuals.window_fill = colors::CARD;
    visuals.extreme_bg_color = colors::CARD;
    visuals.faint_bg_color = Color32::from_rgb(0xF0, 0xF2, 0xF5);
    visuals.override_text_color = Some(colors::TEXT);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.window_stroke = Stroke::new(1.0, colors::BORDER);
    visuals.selection.bg_fill = Color32::from_rgb(0xDB, 0xE8, 0xFD);
    visuals.selection.stroke = Stroke::new(1.0, colors::ACCENT);
    let radius = CornerRadius::same(6);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors::BORDER);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, colors::TEXT);
    widgets.noninteractive.corner_radius = radius;
    widgets.inactive.bg_fill = Color32::from_rgb(0xF1, 0xF3, 0xF6);
    widgets.inactive.weak_bg_fill = Color32::from_rgb(0xF1, 0xF3, 0xF6);
    widgets.inactive.bg_stroke = Stroke::new(1.0, colors::BORDER);
    widgets.inactive.fg_stroke = Stroke::new(1.0, colors::TEXT);
    widgets.inactive.corner_radius = radius;
    widgets.hovered.bg_fill = Color32::from_rgb(0xE8, 0xEE, 0xF8);
    widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE8, 0xEE, 0xF8);
    widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xA9, 0xC1, 0xF5));
    widgets.hovered.fg_stroke = Stroke::new(1.5, colors::TEXT);
    widgets.hovered.corner_radius = radius;
    widgets.active.bg_fill = Color32::from_rgb(0xDB, 0xE8, 0xFD);
    widgets.active.weak_bg_fill = Color32::from_rgb(0xDB, 0xE8, 0xFD);
    widgets.active.bg_stroke = Stroke::new(1.0, colors::ACCENT);
    widgets.active.fg_stroke = Stroke::new(1.5, colors::TEXT);
    widgets.active.corner_radius = radius;
    widgets.open = widgets.active;
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 5.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.interact_size.y = 22.0;
        style.spacing.indent = 14.0;
        style.spacing.window_margin = egui::Margin::same(12);
    });
}
