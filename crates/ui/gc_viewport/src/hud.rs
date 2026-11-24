//! The heads-up display next to the cursor (Inventor's dynamic input): while a drawing tool waits for
//! its next point, floating boxes show the length, angle, radius and so on the cursor gives. Typing a
//! number goes into the highlighted box, Tab locks it and moves on, Enter completes the shape with the
//! locked values, Esc quits.
//! Reference: Inventor 2026 help, "To Draw 2D Geometry Using Dynamic Input"
//! (GUID-33A8B11D-A04C-4809-BC82-C0419215915F).

use egui::{Key, Pos2, vec2};
use gc_i18n::{Lang, tr};
use gc_session::Session;
use gc_tools::{HudField, HudSlot, Key as ToolKey, ToolEvent};
use gc_ui::colors;

use crate::paint::format_length;

/// What the display remembers between frames.
#[derive(Default)]
pub(crate) struct Hud {
    /// The box that has the typed input.
    focus: usize,
    /// What was typed in each box, not sent yet.
    typed: Vec<String>,
    /// The boxes the display showed last frame, to notice when the tool moves on to other boxes.
    fields: Vec<HudField>,
    /// Typing is going into a box (the keyboard focus is ours).
    typing: bool,
    /// A box that is to take the keyboard focus: the one Tab moved to, or the first digit typed.
    take_focus: Option<usize>,
}

impl Hud {
    /// Shows the boxes near the cursor and carries out what was typed. Returns true while the display is
    /// up, so that the caller knows the keyboard may be in use.
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session,
        cursor: Option<Pos2>,
        lang: Lang,
    ) -> bool {
        let slots = if session.active_sketch().is_some() { session.tool_hud() } else { Vec::new() };
        let fields: Vec<HudField> = slots.iter().map(|s| s.field).collect();
        if fields != self.fields {
            // A new step of the tool, or none: start over.
            self.fields = fields;
            self.typed = vec![String::new(); slots.len()];
            self.focus = 0;
            self.typing = false;
            self.take_focus = None;
        }
        if slots.is_empty() {
            return false;
        }
        let Some(cursor) = cursor else { return true };
        // Digits typed with no box in use start the highlighted one, as in Inventor.
        if !self.typing && !ctx.egui_wants_keyboard_input() {
            let first = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Text(text) if text.chars().all(|c| "0123456789.-+".contains(c)) => {
                        Some(text.clone())
                    }
                    _ => None,
                })
            });
            if let Some(text) = first {
                self.typed[self.focus] = text;
                self.typing = true;
                self.take_focus = Some(self.focus);
            }
        }
        let count = slots.len();
        let mut send: Vec<(HudField, Option<String>)> = Vec::new();
        let mut finish = false;
        let mut move_to = None;
        let area = egui::Area::new(egui::Id::unique("gc_hud"))
            .fixed_pos(cursor + vec2(20.0, 20.0))
            .order(egui::Order::Tooltip)
            .interactable(true);
        area.show(ctx, |ui| {
            egui::Frame::new()
                .fill(gc_ui::panel_fill(false))
                .stroke(egui::Stroke::new(1.0, colors::border()))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    for (index, slot) in slots.iter().enumerate() {
                        let name = field_name(lang, slot.field);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).size(11.0).color(colors::muted()));
                            let highlighted = index == self.focus;
                            let hint = measured_text(slot);
                            let text = &mut self.typed[index];
                            let edit = egui::TextEdit::singleline(text)
                                .hint_text(hint)
                                .desired_width(72.0)
                                .font(egui::FontId::monospace(12.0));
                            let response = ui.add(edit);
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::TextInput, true, format!("HUD {name}"))
                            });
                            if self.take_focus == Some(index) {
                                response.request_focus();
                            }
                            if response.has_focus() && self.take_focus.is_none() {
                                // Tab is ours: it locks the box instead of moving the keyboard focus.
                                ui.memory_mut(|memory| {
                                    memory.set_focus_lock_filter(
                                        response.id,
                                        egui::EventFilter {
                                            tab: true,
                                            horizontal_arrows: true,
                                            vertical_arrows: false,
                                            escape: false,
                                        },
                                    );
                                });
                                self.focus = index;
                                self.typing = true;
                            }
                            if highlighted {
                                ui.painter().rect_stroke(
                                    response.rect.expand(1.0),
                                    3.0,
                                    egui::Stroke::new(1.5, colors::ACCENT),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if slot.locked.is_some() {
                                lock_icon(ui);
                            }
                        });
                    }
                });
        });
        if self.typing {
            // Tab locks the box and moves to the next one; Shift+Tab goes back.
            let (tab, back, enter) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, Key::Tab),
                    i.consume_key(egui::Modifiers::SHIFT, Key::Tab),
                    i.key_pressed(Key::Enter),
                )
            });
            if tab || back || enter {
                let text = self.typed[self.focus].trim().to_owned();
                if !text.is_empty() {
                    send.push((slots[self.focus].field, Some(text)));
                }
            }
            if enter {
                // The other boxes keep what was typed in them: send those too.
                for (index, text) in self.typed.iter().enumerate() {
                    if index != self.focus && !text.trim().is_empty() {
                        send.push((slots[index].field, Some(text.trim().to_owned())));
                    }
                }
                finish = true;
            } else if tab {
                move_to = Some((self.focus + 1) % count);
            } else if back {
                move_to = Some((self.focus + count - 1) % count);
            }
        }
        for (field, value) in send {
            session.tool_event(ToolEvent::Hud { field, value });
        }
        if finish {
            session.tool_event(ToolEvent::Key(ToolKey::Enter));
            self.fields.clear();
            self.typing = false;
            self.take_focus = None;
        } else if let Some(next) = move_to {
            self.focus = next;
            self.take_focus = Some(next);
        } else {
            self.take_focus = None;
        }
        true
    }

    /// Esc quits the display: nothing typed is kept.
    pub(crate) fn cancel(&mut self) {
        self.fields.clear();
        self.typed.clear();
        self.typing = false;
        self.take_focus = None;
    }
}

/// The name of a box.
fn field_name(lang: Lang, field: HudField) -> &'static str {
    match field {
        HudField::Length => tr!(lang, HudLength),
        HudField::Angle => tr!(lang, HudAngle),
        HudField::Radius => tr!(lang, HudRadius),
        HudField::Diameter => tr!(lang, HudDiameter),
        HudField::Width => tr!(lang, HudWidth),
        HudField::Height => tr!(lang, HudHeight),
        HudField::Distance => tr!(lang, HudDistance),
        HudField::X => tr!(lang, HudX),
        HudField::Y => tr!(lang, HudY),
        HudField::Scale => tr!(lang, HudScale),
        HudField::DeltaX => tr!(lang, HudDeltaX),
        HudField::DeltaY => tr!(lang, HudDeltaY),
    }
}

/// What the cursor gives now, or the locked value, as shown in an empty box.
fn measured_text(slot: &HudSlot) -> String {
    if let Some(locked) = &slot.locked {
        return locked.clone();
    }
    match slot.field {
        HudField::Angle => format!("{:.1}°", slot.measured.to_degrees()),
        HudField::Scale => format!("{:.3}", slot.measured),
        _ => format_length(slot.measured),
    }
}

/// A small padlock, drawn: it says the value typed in the box holds the cursor.
fn lock_icon(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 14.0), egui::Sense::hover());
    let painter = ui.painter();
    let color = colors::ACCENT;
    let body = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 1.0, rect.center().y),
        egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
    );
    painter.rect_filled(body, 2.0, color);
    painter.circle_stroke(
        egui::pos2(rect.center().x, rect.center().y - 1.0),
        3.0,
        egui::Stroke::new(1.5, color),
    );
}
