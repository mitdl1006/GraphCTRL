//! Ribbon widgets: tabs, titled groups and large icon buttons.

use egui::{Response, Role, Sense, Ui, WidgetInfo, vec2};

use crate::icons::Icon;
use crate::theme::colors;

const BUTTON_SIZE: egui::Vec2 = vec2(58.0, 58.0);

/// A tab of the ribbon: its name, underlined in the accent colour when it is the open one. Returns
/// true when it was clicked.
pub fn ribbon_tab(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let color = if selected { colors::ACCENT } else { colors::muted() };
    let text = egui::RichText::new(label).size(13.0).strong().color(color);
    let response = ui.add(egui::Button::selectable(selected, text).frame_when_inactive(false));
    if selected {
        let rect = response.rect;
        let underline = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 4.0, rect.bottom() - 2.0),
            egui::pos2(rect.right() - 4.0, rect.bottom()),
        );
        ui.painter().rect_filled(underline, 1.0, colors::ACCENT);
    }
    response.clicked()
}

/// A large button: icon above, label below. `selected` marks the active tool.
pub fn ribbon_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    // Wide enough for the label: labels in some languages are longer than the icon is wide.
    let font = egui::FontId::proportional(11.5);
    let text_width = ui
        .fonts_mut(|fonts| fonts.layout_no_wrap(label.to_owned(), font.clone(), egui::Color32::BLACK))
        .size()
        .x;
    let size = vec2(BUTTON_SIZE.x.max(text_width + 10.0), BUTTON_SIZE.y);
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, selected, label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let fill = if selected {
            visuals.selection.bg_fill
        } else if response.hovered() && enabled {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            egui::Color32::TRANSPARENT
        };
        // Disabled buttons stay legible: lighter, not faint.
        let color = if selected {
            colors::ACCENT
        } else if enabled {
            colors::text()
        } else {
            egui::Color32::from_rgb(0xA3, 0xA9, 0xB2)
        };
        ui.painter().rect_filled(rect, 4.0, fill);
        let icon_rect = egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 20.0), vec2(26.0, 26.0));
        icon.paint(ui.painter(), icon_rect, color);
        ui.painter().text(
            rect.center_bottom() - vec2(0.0, 4.0),
            egui::Align2::CENTER_BOTTOM,
            label,
            font,
            color,
        );
    }
    response
}

/// A group of ribbon buttons with its title underneath and a separator after it.
pub fn ribbon_group(ui: &mut Ui, title: &str, add_buttons: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        let row = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            add_buttons(ui);
        });
        // The title is centred under the buttons without claiming more width than they have.
        let (rect, _) = ui.allocate_exact_size(vec2(row.response.rect.width(), 14.0), Sense::hover());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(10.5),
            colors::muted(),
        );
    });
    ui.separator();
}
