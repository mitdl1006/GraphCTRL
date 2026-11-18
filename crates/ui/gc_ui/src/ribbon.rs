//! Ribbon widgets: tabs, titled groups, large icon buttons and split buttons.

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

/// A small ribbon button: the icon alone, the name as a tooltip and as its accessible name. Inventor packs
/// panels such as Constrain with these, in rows of small icons.
pub fn ribbon_small_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), sense);
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
        let color = if selected {
            colors::ACCENT
        } else if enabled {
            colors::text()
        } else {
            egui::Color32::from_rgb(0xA3, 0xA9, 0xB2)
        };
        ui.painter().rect_filled(rect, 4.0, fill);
        icon.paint(ui.painter(), rect.shrink(5.0), color);
    }
    response.on_hover_text(label)
}

/// One row of a [`ribbon_stack`].
pub struct StackRow<'a> {
    /// Its icon.
    pub icon: Icon,
    /// Its name.
    pub label: &'a str,
    /// Whether the command is running now.
    pub selected: bool,
    /// Whether it can be used.
    pub enabled: bool,
}

/// Up to three small buttons one above the other, each an icon with its name beside it, as Inventor packs
/// the less used commands of a panel next to the large ones. Returns the index of the row clicked.
pub fn ribbon_stack(ui: &mut Ui, rows: &[StackRow<'_>]) -> Option<usize> {
    ribbon_stack_responses(ui, rows).iter().position(|response| response.clicked())
}

/// Like [`ribbon_stack`], but returns every row's response (for a menu that opens from a row).
pub fn ribbon_stack_responses(ui: &mut Ui, rows: &[StackRow<'_>]) -> Vec<Response> {
    const ROW_HEIGHT: f32 = 24.0;
    let font = egui::FontId::proportional(11.5);
    let widest = rows
        .iter()
        .map(|row| {
            ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(row.label.to_owned(), font.clone(), egui::Color32::BLACK)
            })
            .size()
            .x
        })
        .fold(0.0_f32, f32::max);
    let width = widest + 34.0;
    let mut responses = Vec::new();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        // The rows are centred in the height of a large button.
        let spare = (BUTTON_SIZE.y - ROW_HEIGHT * rows.len() as f32).max(0.0);
        ui.add_space(spare / 2.0);
        for row in rows {
            let sense = if row.enabled { Sense::click() } else { Sense::hover() };
            let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), sense);
            response.widget_info(|| WidgetInfo::selected(Role::Button, row.enabled, row.selected, row.label));
            if ui.is_rect_visible(rect) {
                let visuals = ui.visuals();
                let fill = if row.selected {
                    visuals.selection.bg_fill
                } else if response.hovered() && row.enabled {
                    visuals.widgets.hovered.weak_bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                };
                let color = if row.selected {
                    colors::ACCENT
                } else if row.enabled {
                    colors::text()
                } else {
                    egui::Color32::from_rgb(0xA3, 0xA9, 0xB2)
                };
                ui.painter().rect_filled(rect, 4.0, fill);
                let icon_rect =
                    egui::Rect::from_center_size(rect.left_center() + vec2(15.0, 0.0), vec2(18.0, 18.0));
                row.icon.paint(ui.painter(), icon_rect, color);
                ui.painter().text(
                    rect.left_center() + vec2(30.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    row.label,
                    font.clone(),
                    color,
                );
            }
            responses.push(response);
        }
    });
    responses
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

/// One variant of a split button.
pub struct SplitItem<'a> {
    /// Its icon.
    pub icon: Icon,
    /// Its name: the button shows it while it is the current variant.
    pub label: &'a str,
    /// Whether the command is running now (the tool is the active one).
    pub active: bool,
}

/// An Inventor-style split button: the large body runs the variant last used and shows its icon and name;
/// the arrow under it opens a list to choose another variant, which then becomes the body. Returns the index
/// of the variant to run when the body was clicked or a variant was chosen. The choice is remembered while
/// the program runs, under `id_salt`.
///
/// Accessible names: the body is the current variant's name, the arrow is `more` ("<group> more").
pub fn ribbon_split_button(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    more: &str,
    items: &[SplitItem<'_>],
    enabled: bool,
) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let id = egui::Id::unique("gc_split_button").with(id_salt);
    let remembered = ui.data(|data| data.get_temp::<usize>(id)).unwrap_or(0);
    // The variant that is running wins, so the button follows keys and menus that start a tool.
    let current = items.iter().position(|item| item.active).unwrap_or(remembered.min(items.len() - 1));
    let item = &items[current];
    let selected = items.iter().any(|item| item.active);

    let font = egui::FontId::proportional(11.5);
    // As wide as the current name needs: the button changes width with the variant, as Inventor's does.
    let widest = ui
        .fonts_mut(|fonts| fonts.layout_no_wrap(item.label.to_owned(), font.clone(), egui::Color32::BLACK))
        .size()
        .x;
    let size = vec2(BUTTON_SIZE.x.max(widest + 10.0), BUTTON_SIZE.y);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, body) = ui.allocate_exact_size(size, sense);
    body.widget_info(|| WidgetInfo::selected(Role::Button, enabled, selected, item.label));
    // The arrow strip along the bottom edge, over the body.
    let arrow_rect =
        egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 15.0), rect.right_bottom());
    let arrow = ui.interact(arrow_rect, id.with("arrow"), sense);
    arrow.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, more));

    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let fill = if selected {
            visuals.selection.bg_fill
        } else if (body.hovered() || arrow.hovered()) && enabled {
            visuals.widgets.hovered.weak_bg_fill
        } else {
            egui::Color32::TRANSPARENT
        };
        let color = if selected {
            colors::ACCENT
        } else if enabled {
            colors::text()
        } else {
            egui::Color32::from_rgb(0xA3, 0xA9, 0xB2)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, fill);
        if arrow.hovered() && enabled {
            painter.rect_filled(arrow_rect, 4.0, visuals.widgets.hovered.bg_fill);
        }
        // The icon over the name over the arrow strip: 58 px high in all.
        let icon_rect = egui::Rect::from_center_size(rect.center_top() + vec2(0.0, 15.0), vec2(22.0, 22.0));
        item.icon.paint(painter, icon_rect, color);
        painter.text(
            egui::pos2(rect.center().x, rect.bottom() - 16.0),
            egui::Align2::CENTER_BOTTOM,
            item.label,
            font,
            color,
        );
        // A small downward triangle.
        let c = arrow_rect.center();
        painter.add(egui::Shape::convex_polygon(
            vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 2.5)],
            color,
            egui::Stroke::NONE,
        ));
    }

    let mut run = body.clicked().then_some(current);
    egui::Popup::menu(&arrow).show(|ui| {
        for (index, item) in items.iter().enumerate() {
            let label = ui.selectable_label(index == current, item.label);
            if label.clicked() {
                run = Some(index);
                ui.close();
            }
        }
    });
    if let Some(index) = run {
        ui.data_mut(|data| data.insert_temp(id, index));
    }
    run
}

/// A large ribbon button that opens a drop-list (Inventor's Object Visibility): `add_items` fills the list,
/// which stays open while check boxes in it are clicked and closes with a click outside.
pub fn ribbon_drop_button(ui: &mut Ui, icon: Icon, label: &str, add_items: impl FnOnce(&mut Ui)) -> Response {
    let button = ribbon_button(ui, icon, label, false, true);
    egui::Popup::menu(&button).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(add_items);
    button
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
