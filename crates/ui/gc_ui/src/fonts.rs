//! Bundled UI fonts.
//!
//! Jetendard (JetBrains Mono + Pretendard, OFL 1.1) is embedded unmodified and registered first in
//! both the proportional and monospace families, so Latin and Hangul render identically on every OS.
//! egui's own fonts and the system fonts remain as fallbacks for glyphs Jetendard lacks.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

static REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/Jetendard-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../../../../assets/fonts/Jetendard-Bold.ttf");

/// Name of the font family used for headings and emphasis.
pub const BOLD_FAMILY: &str = "gc-bold";

/// The bold family as an egui value.
pub fn bold_family() -> FontFamily {
    FontFamily::Name(BOLD_FAMILY.into())
}

/// Font definitions with Jetendard as the first choice everywhere.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("Jetendard".to_owned(), Arc::new(FontData::from_static(REGULAR)));
    fonts.font_data.insert("Jetendard-Bold".to_owned(), Arc::new(FontData::from_static(BOLD)));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "Jetendard".to_owned());
    }
    fonts.families.insert(bold_family(), vec!["Jetendard-Bold".to_owned(), "Jetendard".to_owned()]);
    fonts
}

/// Installs the bundled fonts into an egui context.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}
