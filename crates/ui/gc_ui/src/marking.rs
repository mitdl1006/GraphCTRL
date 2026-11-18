//! The marking menu, as Inventor's (About Marking Menus,
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3.htm):
//! a right-click in the graphics window opens a radial menu centred on the cursor, each command in a
//! sector of the circle (menu mode); drawing a mark toward a command and quickly releasing the button
//! picks it without showing the menu (mark mode); a short context menu (the overflow menu) appears above
//! or below the radial menu, depending on where the cursor is.
//!
//! The caller owns a [`MarkingMenu`], calls [`MarkingMenu::press`] when the right button goes down in its
//! area and [`MarkingMenu::show`] every frame; the result says which command was picked.

/// Sectors of the radial menu, clockwise from the top: north, north-east, east, … north-west.
pub const SECTORS: usize = 8;
/// Distance from the centre at which a mark counts and the commands are placed (points).
const RADIUS: f32 = 92.0;
/// A mark shorter than this is no mark: releasing opens the menu instead.
const MARK_LENGTH: f32 = 24.0;
/// A mark must be released within this time (seconds) to pick a command; holding longer shows the menu.
const MARK_TIME: f64 = 0.6;

/// A command of the menu.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkingItem {
    /// What the command is called.
    pub label: String,
    /// False to show it greyed out.
    pub enabled: bool,
}

impl MarkingItem {
    /// An enabled command.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), enabled: true }
    }
}

/// What the menu did this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkingPick {
    /// The radial command in this sector (0 = north, clockwise).
    Radial(usize),
    /// The overflow command at this index.
    Overflow(usize),
}

/// The state of an open marking menu.
#[derive(Clone, Debug, Default)]
pub struct MarkingMenu {
    /// Where it opened, and when.
    open: Option<(egui::Pos2, f64)>,
    /// True while the right button is still held from the press that opened it (mark mode).
    marking: bool,
    /// The command the Up and Down keys moved to (a click confirms it).
    keyed: Option<usize>,
}

/// The sector a direction points into (0 = north, clockwise), or `None` for a direction too short to
/// tell.
pub fn sector_of(delta: egui::Vec2, minimum: f32) -> Option<usize> {
    if delta.length() < minimum {
        return None;
    }
    // Screen y grows downward: north is −y. Angle clockwise from north.
    // Screen geometry in points: f32 is plenty, and the result is a sector, not a coordinate.
    let angle = delta.x.atan2(-delta.y);
    let turn = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "0..=8, wrapped"
    )]
    let sector = (turn * SECTORS as f32).round() as usize % SECTORS;
    Some(sector)
}

/// The centre of a sector's command, relative to the menu centre.
fn sector_offset(sector: usize) -> egui::Vec2 {
    #[allow(clippy::cast_precision_loss, reason = "eight sectors")]
    let angle = sector as f32 / SECTORS as f32 * std::f32::consts::TAU;
    egui::vec2(angle.sin(), -angle.cos()) * RADIUS
}

impl MarkingMenu {
    /// True while the menu is open (or a mark is being drawn).
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Opens the menu at `at`; the right button is held, so a mark may follow.
    pub fn press(&mut self, at: egui::Pos2, time: f64) {
        self.open = Some((at, time));
        self.marking = true;
        self.keyed = None;
    }

    /// Closes the menu.
    pub fn close(&mut self) {
        self.open = None;
        self.marking = false;
        self.keyed = None;
    }

    /// Draws the menu and handles the pointer. `radial` has up to [`SECTORS`] commands, in sector order
    /// (`None` leaves a sector empty); `overflow` is the short context menu.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        radial: &[Option<MarkingItem>],
        overflow: &[MarkingItem],
    ) -> Option<MarkingPick> {
        let (centre, opened) = self.open?;
        // Esc closes the menu too (the help dismisses by a click in the centre, and warns that Esc can cancel a
        // command in progress; here the menu's own Esc only closes it, GUID-756BC728-3366-497D-B480-4A06E932ED20).
        let (pointer, released, time, escape, primary, down, up) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.button_released(egui::PointerButton::Secondary),
                i.time,
                i.key_pressed(egui::Key::Escape),
                i.pointer.button_clicked(egui::PointerButton::Primary),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
            )
        });
        if escape {
            self.close();
            return None;
        }
        let item = |sector: usize| radial.get(sector).and_then(Option::as_ref).filter(|item| item.enabled);
        // Up and Down move through the enabled commands (clockwise from north); a click then confirms.
        if down || up {
            let enabled: Vec<usize> = (0..SECTORS).filter(|s| item(*s).is_some()).collect();
            if !enabled.is_empty() {
                let at = self.keyed.and_then(|k| enabled.iter().position(|s| *s == k));
                let next = match (at, down) {
                    (None, true) => 0,
                    (None, false) => enabled.len() - 1,
                    (Some(i), true) => (i + 1) % enabled.len(),
                    (Some(i), false) => (i + enabled.len() - 1) % enabled.len(),
                };
                self.keyed = Some(enabled[next]);
            }
        }
        // Mark mode: a quick stroke toward a command picks it on release.
        if self.marking {
            let direction = pointer.map(|p| p - centre).unwrap_or_default();
            if released {
                self.marking = false;
                if time - opened <= MARK_TIME
                    && let Some(sector) = sector_of(direction, MARK_LENGTH)
                    && item(sector).is_some()
                {
                    self.close();
                    return Some(MarkingPick::Radial(sector));
                }
            }
            // A held button that has not marked yet still shows the menu (menu mode).
        }

        let hovered_sector = if self.keyed.is_some() && !self.marking {
            self.keyed
        } else {
            pointer.and_then(|p| sector_of(p - centre, MARK_LENGTH))
        };
        let mut picked = None;
        let painter =
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("marking_menu")));
        let visuals = ctx.global_style().visuals.clone();
        painter.circle_stroke(centre, 6.0, egui::Stroke::new(1.5, visuals.text_color()));
        let font = egui::FontId::proportional(13.0);
        for (sector, entry) in radial.iter().enumerate().take(SECTORS) {
            let Some(entry) = entry else { continue };
            let at = centre + sector_offset(sector);
            let galley = painter.layout_no_wrap(entry.label.clone(), font.clone(), visuals.text_color());
            let rect = egui::Rect::from_center_size(at, galley.size() + egui::vec2(16.0, 8.0));
            let hot = entry.enabled && hovered_sector == Some(sector);
            let fill = if hot { visuals.selection.bg_fill } else { visuals.window_fill };
            painter.rect(rect, 4.0, fill, visuals.window_stroke, egui::StrokeKind::Outside);
            let colour = if entry.enabled { visuals.text_color() } else { visuals.weak_text_color() };
            painter.galley(rect.center() - galley.size() / 2.0, galley, colour);
            if hot {
                painter.line_segment([centre, at], egui::Stroke::new(1.0, visuals.selection.stroke.color));
            }
        }
        // The overflow menu below the radial one, or above it near the bottom of the screen.
        if !overflow.is_empty() {
            let screen = ctx.content_rect();
            let below = centre.y + RADIUS + 40.0 + 22.0 * overflow.len() as f32 <= screen.bottom();
            let top = if below {
                centre.y + RADIUS + 28.0
            } else {
                centre.y - RADIUS - 28.0 - 22.0 * overflow.len() as f32
            };
            let area = egui::Area::new(egui::Id::unique("marking_overflow"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(centre.x - 80.0, top));
            area.show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(160.0);
                    for (index, entry) in overflow.iter().enumerate() {
                        if ui
                            .add_enabled(entry.enabled, egui::Button::new(&entry.label).frame(false))
                            .clicked()
                        {
                            picked = Some(MarkingPick::Overflow(index));
                        }
                    }
                });
            });
        }
        // The radial commands as an invisible area of buttons, so they can be found and clicked by name.
        egui::Area::new(egui::Id::unique("marking_buttons"))
            .order(egui::Order::Foreground)
            .fixed_pos(centre)
            .show(ctx, |ui| {
                for (sector, entry) in radial.iter().enumerate().take(SECTORS) {
                    let Some(entry) = entry else { continue };
                    let at = centre + sector_offset(sector);
                    let galley =
                        ui.painter().layout_no_wrap(entry.label.clone(), font.clone(), visuals.text_color());
                    let rect = egui::Rect::from_center_size(at, galley.size() + egui::vec2(16.0, 8.0));
                    let response =
                        ui.interact(rect, egui::Id::unique(("marking_item", sector)), egui::Sense::click());
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, entry.enabled, &entry.label)
                    });
                    if response.clicked() && entry.enabled {
                        picked = Some(MarkingPick::Radial(sector));
                    }
                }
            });
        if picked.is_some() {
            self.close();
        } else if primary
            && !self.marking
            && let Some(sector) = self.keyed.filter(|k| item(*k).is_some())
        {
            // The command the keys moved to is confirmed by a click.
            self.close();
            return Some(MarkingPick::Radial(sector));
        } else if primary && !self.marking {
            // A click anywhere else (the centre included) closes the menu, after the area buttons had their chance.
            let on_overflow = ctx
                .layer_id_at(pointer.unwrap_or_default())
                .is_some_and(|l| l.id == egui::Id::unique("marking_overflow"));
            if !on_overflow {
                self.close();
            }
        }
        ctx.request_repaint();
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_fall_into_their_sectors() {
        assert_eq!(sector_of(egui::vec2(0.0, -50.0), 24.0), Some(0)); // north
        assert_eq!(sector_of(egui::vec2(50.0, -50.0), 24.0), Some(1)); // north-east
        assert_eq!(sector_of(egui::vec2(50.0, 0.0), 24.0), Some(2)); // east
        assert_eq!(sector_of(egui::vec2(0.0, 50.0), 24.0), Some(4)); // south
        assert_eq!(sector_of(egui::vec2(-50.0, 0.0), 24.0), Some(6)); // west
        assert_eq!(sector_of(egui::vec2(-50.0, -51.0), 24.0), Some(7)); // north-west
        // Just left of north still rounds to north; a short move is no mark.
        assert_eq!(sector_of(egui::vec2(-5.0, -50.0), 24.0), Some(0));
        assert_eq!(sector_of(egui::vec2(3.0, 4.0), 24.0), None);
    }
}
