//! Colours and widget style.

/// The palette. Sketch colours follow the convention CAD users know: blue geometry can still move, black
/// geometry is fully constrained.
pub mod colors {
    use std::cell::Cell;

    use egui::Color32;

    thread_local! {
        // The interface is drawn on one thread, and per thread keeps tests that run side by side apart.
        static DARK: Cell<bool> = const { Cell::new(false) };
    }

    /// Switches the palette to the dark or the light scheme. [`super::install`] calls it.
    pub fn set_dark(dark: bool) {
        DARK.set(dark);
    }

    /// Whether the dark scheme is in use.
    pub fn is_dark() -> bool {
        DARK.get()
    }

    fn themed(light: Color32, dark: Color32) -> Color32 {
        if is_dark() { dark } else { light }
    }

    /// Sketch geometry that can still move.
    pub const FREE: Color32 = Color32::from_rgb(0x10, 0x40, 0xE0);
    /// The tool-tip trace of a planned path that is up to date.
    pub const TRACE: Color32 = Color32::from_rgb(0x00, 0x8A, 0x4B);
    /// Waypoint markers and their names.
    pub const WAYPOINT: Color32 = Color32::from_rgb(0xB0, 0x1E, 0x8C);
    /// Construction geometry.
    pub const CONSTRUCTION: Color32 = Color32::from_rgb(0xB0, 0x70, 0x20);
    /// Selected items.
    pub const SELECTED: Color32 = Color32::from_rgb(0x1E, 0x90, 0xFF);
    /// Tool previews and snap markers.
    pub const PREVIEW: Color32 = Color32::from_rgb(0xFF, 0x80, 0x00);
    /// Constraint glyphs.
    pub const GLYPH: Color32 = Color32::from_rgb(0x1B, 0x7F, 0x3B);
    /// Errors in the status bar.
    pub const ERROR: Color32 = Color32::from_rgb(0xC0, 0x20, 0x20);
    /// The accent: active tabs, section bars, focused fields.
    pub const ACCENT: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    /// Sketch geometry that is fully constrained.
    pub fn constrained() -> Color32 {
        themed(Color32::from_rgb(0x10, 0x10, 0x10), Color32::from_rgb(0xF0, 0xF0, 0xF0))
    }
    /// Geometry of sketches that are not being edited.
    pub fn inactive() -> Color32 {
        themed(Color32::from_rgb(0x60, 0x68, 0x72), Color32::from_rgb(0x8A, 0x92, 0x9C))
    }
    /// A face of the view cube.
    pub fn cube_face() -> Color32 {
        themed(Color32::from_rgb(0xF4, 0xF5, 0xF7), Color32::from_rgb(0x3A, 0x40, 0x48))
    }
    /// The part of the view cube under the cursor.
    pub fn cube_hover() -> Color32 {
        themed(Color32::from_rgb(0xB5, 0xD8, 0xFF), Color32::from_rgb(0x3E, 0x6F, 0xA8))
    }
    /// Edges and labels of the view cube.
    pub fn cube_edge() -> Color32 {
        themed(Color32::from_rgb(0x5E, 0x66, 0x72), Color32::from_rgb(0xA8, 0xB0, 0xBA))
    }
    /// Driving dimensions.
    pub fn dimension() -> Color32 {
        themed(Color32::from_rgb(0x20, 0x20, 0x20), Color32::from_rgb(0xE0, 0xE0, 0xE0))
    }
    /// Reference dimensions.
    pub fn reference() -> Color32 {
        themed(Color32::from_rgb(0x80, 0x80, 0x80), Color32::from_rgb(0xA0, 0xA0, 0xA0))
    }
    /// Top and bottom of the viewport background gradient.
    pub fn background_top() -> Color32 {
        themed(Color32::from_rgb(0xE6, 0xEA, 0xEE), Color32::from_rgb(0x3A, 0x40, 0x48))
    }
    /// Bottom of the viewport background gradient.
    pub fn background_bottom() -> Color32 {
        themed(Color32::from_rgb(0xC3, 0xCA, 0xD3), Color32::from_rgb(0x22, 0x26, 0x2B))
    }
    /// Minor grid lines.
    pub fn grid() -> Color32 {
        themed(Color32::from_rgb(0xB4, 0xBC, 0xC6), Color32::from_rgb(0x4A, 0x52, 0x5C))
    }
    /// Major grid lines.
    pub fn grid_major() -> Color32 {
        themed(Color32::from_rgb(0x98, 0xA2, 0xAE), Color32::from_rgb(0x5C, 0x66, 0x72))
    }
    /// The ribbon's background.
    pub fn ribbon() -> Color32 {
        themed(Color32::from_rgb(0xFF, 0xFF, 0xFF), Color32::from_rgb(0x2B, 0x30, 0x36))
    }
    /// The side panels' background.
    pub fn surface() -> Color32 {
        themed(Color32::from_rgb(0xF6, 0xF7, 0xF9), Color32::from_rgb(0x23, 0x27, 0x2C))
    }
    /// Text fields, cards and other raised surfaces.
    pub fn card() -> Color32 {
        themed(Color32::from_rgb(0xFF, 0xFF, 0xFF), Color32::from_rgb(0x2B, 0x30, 0x36))
    }
    /// Hairlines and widget borders.
    pub fn border() -> Color32 {
        themed(Color32::from_rgb(0xDF, 0xE3, 0xE8), Color32::from_rgb(0x3A, 0x40, 0x48))
    }
    /// Body text.
    pub fn text() -> Color32 {
        themed(Color32::from_rgb(0x1F, 0x23, 0x28), Color32::from_rgb(0xE6, 0xE8, 0xEB))
    }
    /// Labels and notes.
    pub fn muted() -> Color32 {
        themed(Color32::from_rgb(0x6B, 0x72, 0x80), Color32::from_rgb(0x9A, 0xA3, 0xAF))
    }
    /// Opacity of the floating side panels when they are see-through (0.84 of 255).
    pub const PANEL_ALPHA: u8 = 214;
    /// Space between a floating panel and the edge of the viewport, in points.
    pub const PANEL_MARGIN: f32 = 8.0;
}

/// The fill of a floating side panel: the surface colour, see-through unless `opaque`.
pub fn panel_fill(opaque: bool) -> egui::Color32 {
    let surface = colors::surface();
    let alpha = if opaque { 255 } else { colors::PANEL_ALPHA };
    egui::Color32::from_rgba_unmultiplied(surface.r(), surface.g(), surface.b(), alpha)
}

/// The frame of a floating side panel: rounded, hairline border, a soft shadow.
pub fn panel_frame(opaque: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(panel_fill(opaque))
        .stroke(egui::Stroke::new(1.0, colors::border()))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(8))
        .shadow(egui::Shadow {
            offset: [0, 2],
            blur: 12,
            spread: 0,
            color: egui::Color32::from_black_alpha(40),
        })
}

/// Applies the light theme: white raised surfaces on a light grey ground, hairline borders, rounded
/// corners, one blue accent.
pub fn install(ctx: &egui::Context, dark: bool) {
    use egui::{Color32, CornerRadius, Stroke};
    colors::set_dark(dark);
    let pick = |light: Color32, dark_color: Color32| if dark { dark_color } else { light };
    let mut visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    visuals.panel_fill = colors::surface();
    visuals.window_fill = colors::card();
    visuals.extreme_bg_color = colors::card();
    visuals.faint_bg_color = pick(Color32::from_rgb(0xF0, 0xF2, 0xF5), Color32::from_rgb(0x2F, 0x34, 0x3B));
    visuals.override_text_color = Some(colors::text());
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.window_stroke = Stroke::new(1.0, colors::border());
    visuals.selection.bg_fill =
        pick(Color32::from_rgb(0xDB, 0xE8, 0xFD), Color32::from_rgb(0x2A, 0x45, 0x75));
    visuals.selection.stroke = Stroke::new(1.0, colors::ACCENT);
    let radius = CornerRadius::same(6);
    let idle = pick(Color32::from_rgb(0xF1, 0xF3, 0xF6), Color32::from_rgb(0x33, 0x39, 0x40));
    let hover = pick(Color32::from_rgb(0xE8, 0xEE, 0xF8), Color32::from_rgb(0x3A, 0x44, 0x55));
    let pressed = pick(Color32::from_rgb(0xDB, 0xE8, 0xFD), Color32::from_rgb(0x2A, 0x45, 0x75));
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors::border());
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, colors::text());
    widgets.noninteractive.corner_radius = radius;
    widgets.inactive.bg_fill = idle;
    widgets.inactive.weak_bg_fill = idle;
    widgets.inactive.bg_stroke = Stroke::new(1.0, colors::border());
    widgets.inactive.fg_stroke = Stroke::new(1.0, colors::text());
    widgets.inactive.corner_radius = radius;
    widgets.hovered.bg_fill = hover;
    widgets.hovered.weak_bg_fill = hover;
    widgets.hovered.bg_stroke =
        Stroke::new(1.0, pick(Color32::from_rgb(0xA9, 0xC1, 0xF5), Color32::from_rgb(0x5B, 0x7F, 0xC7)));
    widgets.hovered.fg_stroke = Stroke::new(1.5, colors::text());
    widgets.hovered.corner_radius = radius;
    widgets.active.bg_fill = pressed;
    widgets.active.weak_bg_fill = pressed;
    widgets.active.bg_stroke = Stroke::new(1.0, colors::ACCENT);
    widgets.active.fg_stroke = Stroke::new(1.5, colors::text());
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
