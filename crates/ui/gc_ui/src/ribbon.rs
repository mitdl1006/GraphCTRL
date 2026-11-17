//! Ribbon widgets: tabs, titled groups and large icon buttons.

use egui::{Response, Role, Sense, Ui, WidgetInfo, vec2};

use crate::icons::Icon;

const BUTTON_SIZE: egui::Vec2 = vec2(58.0, 58.0);

/// A tab of the ribbon. Returns true when it was clicked.
pub fn ribbon_tab(ui: &mut Ui, label: &str, selected: bool) -> bool {
    ui.selectable_label(selected, egui::RichText::new(label).strong()).clicked()
}

/// A large button: icon above, label below. `selected` marks the active tool.
pub fn ribbon_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(BUTTON_SIZE, sense);
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
        let color = if enabled { visuals.strong_text_color() } else { visuals.weak_text_color() };
        ui.painter().rect_filled(rect, 4.0, fill);
        let icon_rect = egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 20.0), vec2(26.0, 26.0));
        icon.paint(ui.painter(), icon_rect, color);
        ui.painter().text(
            rect.center_bottom() - vec2(0.0, 4.0),
            egui::Align2::CENTER_BOTTOM,
            label,
            egui::FontId::proportional(11.5),
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
            ui.visuals().weak_text_color(),
        );
    });
    ui.separator();
}
