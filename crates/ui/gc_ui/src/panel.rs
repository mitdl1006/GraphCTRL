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
            colors::muted(),
        );
    }
    ui.label(RichText::new(name).size(18.0).strong().color(colors::text()));
    ui.add_space(2.0);
}

/// A section header: a short accent bar, the title, and a hairline across the panel.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(vec2(3.0, 14.0), Sense::hover());
        ui.painter().rect_filled(bar, 1.5, colors::ACCENT);
        ui.label(RichText::new(title).size(12.5).strong().color(colors::text()));
    });
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, colors::border());
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
        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(colors::muted())).truncate())
    })
    .inner
}

/// A small muted note under a value.
pub fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(colors::muted()));
}

/// The header of a side panel: its name, and a hairline under it.
pub fn header(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).size(14.0).strong().color(colors::text()));
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, colors::border());
    ui.add_space(2.0);
}

/// A card: a white, rounded box with a hairline border, for grouped read-outs such as plan results.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(colors::card())
        .stroke(egui::Stroke::new(1.0, colors::border()))
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

/// What a [`tree_row`] shows.
pub struct TreeRow<'a> {
    /// The icon of the kind of item.
    pub icon: crate::icons::Icon,
    /// The text of the row.
    pub text: &'a str,
    /// Whether the item is selected.
    pub selected: bool,
    /// A visibility check box: its state and the accessible name.
    pub visible: Option<(&'a mut bool, &'a str)>,
    /// The accessible name of the delete button, when the item can be deleted.
    pub delete: Option<&'a str>,
}

/// What happened to a [`tree_row`].
pub struct TreeRowResult {
    /// The row itself: click to select, double-click to open, secondary click for the menu.
    pub row: egui::Response,
    /// The delete button was pressed.
    pub deleted: bool,
    /// The visibility check box changed.
    pub toggled: bool,
}

/// One row of the browser tree: a kind icon, the text, an optional visibility check box and an optional
/// delete button. The delete button is drawn only while the row is hovered or selected; it is always
/// there for assistive technology.
pub fn tree_row(ui: &mut Ui, row: TreeRow<'_>) -> TreeRowResult {
    const HEIGHT: f32 = 22.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let painter = ui.painter();
    if row.selected {
        painter.rect_filled(rect, 4.0, with_alpha(colors::ACCENT, 34));
    } else if hovered {
        painter.rect_filled(rect, 4.0, with_alpha(colors::text(), 14));
    }
    let mut x = rect.left() + 4.0;
    let mut toggled = false;
    if let Some((state, name)) = row.visible {
        let box_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top()), vec2(HEIGHT, HEIGHT));
        let check = ui.put(box_rect, egui::Checkbox::without_text(state)).on_hover_text(name);
        check.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *state, name));
        toggled = check.changed();
        x += HEIGHT + 2.0;
    }
    let tint = if row.selected { colors::ACCENT } else { colors::muted() };
    let icon_rect = egui::Rect::from_center_size(egui::pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0));
    row.icon.paint(ui.painter(), icon_rect, tint);
    x += 22.0;
    let right = rect.right() - if row.delete.is_some() { HEIGHT + 2.0 } else { 4.0 };
    let clip = egui::Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(right, rect.bottom()));
    ui.painter().with_clip_rect(clip).text(
        egui::pos2(x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        row.text,
        egui::FontId::proportional(13.0),
        colors::text(),
    );
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, row.selected, row.text));
    let mut deleted = false;
    if let Some(name) = row.delete {
        let cross_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - HEIGHT - 2.0, rect.top()),
            vec2(HEIGHT, HEIGHT),
        );
        let shown = hovered || row.selected;
        let color = if shown { colors::muted() } else { egui::Color32::TRANSPARENT };
        let cross = egui::Button::new(RichText::new("×").size(14.0).color(color)).frame(false);
        let button = ui.put(cross_rect, cross);
        button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
        let button = if shown { button.on_hover_text(name) } else { button };
        deleted = button.clicked();
    }
    TreeRowResult { row: response, deleted, toggled }
}

fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

/// A property row with a drop-down: the muted label in the label column, the combo box filling the rest.
/// The label is also the combo box's accessible name.
pub fn combo_row<R>(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
    selected: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) {
    ui.horizontal(|ui| {
        let name = label(ui, text);
        let combo = egui::ComboBox::from_id_salt(salt)
            .selected_text(selected)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, add_contents);
        combo.response.labelled_by(name.id);
    });
}
