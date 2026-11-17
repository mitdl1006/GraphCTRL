//! Building blocks of the side panels: a title for the selected item, section headers, property rows.

use egui::{RichText, Sense, Ui, vec2};

use crate::theme::colors;

/// The width of the label column of property rows, so that values line up.
pub const LABEL_WIDTH: f32 = 72.0;

/// The selected item's name, large, with what kind of item it is above it in small muted letters.
pub fn title(ui: &mut Ui, kind: &str, name: &str) {
    ui.add_space(4.0);
    // The kind is painted, not a label: it repeats words the browser and ribbon use as their own names.
    if !kind.is_empty() {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            kind,
            egui::FontId::proportional(11.0),
            colors::MUTED,
        );
    }
    ui.label(RichText::new(name).size(18.0).strong().color(colors::TEXT));
    ui.add_space(2.0);
}

/// A section header: a short accent bar, the title, and a hairline across the panel.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(vec2(3.0, 14.0), Sense::hover());
        ui.painter().rect_filled(bar, 1.5, colors::ACCENT);
        ui.label(RichText::new(title).size(12.5).strong().color(colors::TEXT));
    });
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, colors::BORDER);
    ui.add_space(4.0);
}

/// The label of a property row: muted, in a column of fixed width.
pub fn label(ui: &mut Ui, text: &str) -> egui::Response {
    fixed_label(ui, LABEL_WIDTH, text)
}

/// A muted label in a column of the given width, left-aligned and cut short with "…" when too long.
pub fn fixed_label(ui: &mut Ui, width: f32, text: &str) -> egui::Response {
    let size = vec2(width, ui.spacing().interact_size.y);
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        ui.set_min_size(size);
        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(colors::MUTED)).truncate())
    })
    .inner
}

/// A small muted note under a value.
pub fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(colors::MUTED));
}

/// The header of a side panel: its name, and a hairline under it.
pub fn header(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).size(14.0).strong().color(colors::TEXT));
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, colors::BORDER);
    ui.add_space(2.0);
}

/// A card: a white, rounded box with a hairline border, for grouped read-outs such as plan results.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(colors::CARD)
        .stroke(egui::Stroke::new(1.0, colors::BORDER))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A status chip: short text on a tinted, rounded background.
pub fn chip(ui: &mut Ui, text: &str, color: egui::Color32) {
    let tint = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28);
    egui::Frame::new().fill(tint).corner_radius(10).inner_margin(egui::Margin::symmetric(8, 2)).show(
        ui,
        |ui| {
            ui.label(RichText::new(text).size(12.0).strong().color(color));
        },
    );
}
