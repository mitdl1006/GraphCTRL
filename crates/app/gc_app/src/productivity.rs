//! The Assemble tab's Productivity panel: Ground and Root, and Place at Component Origin.
//! Reference: Inventor 2026 help, Ground and Root (GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696) and Place at
//! Component Origin (GUID-E7A45820-E297-449D-AFBF-3C8D8EAE8A0E).

use gc_core::AnyId;
use gc_doc::Occurrence;
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// The Ground and Root Components dialog: the components, and what to do with them. Only "Ground at origin" is
/// there in the backend; the other two options are listed, disabled, with the reason.
#[derive(Default)]
pub(crate) struct GroundRoot {
    pub(crate) open: bool,
    items: Vec<AnyId>,
}

impl GroundRoot {
    /// Opens the dialog with these components chosen (the marked rows of the browser, if any).
    pub(crate) fn start(&mut self, items: Vec<AnyId>) {
        *self = Self { open: true, items };
    }

    /// The dialog window; the error of a refused run, if any.
    pub(crate) fn window(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session,
        lang: Lang,
    ) -> Option<String> {
        let mut open = self.open;
        let mut ok = false;
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 330.0, screen.top() + 200.0);
        let components: Vec<(AnyId, String)> = session
            .components()
            .iter()
            .filter_map(|node| match node.occurrence {
                Occurrence::Item(id) => Some((id, node.name.clone())),
                _ => None,
            })
            .collect();
        egui::Window::new(tr!(lang, GroundRootTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(tr!(lang, ReplicateComponents)).strong());
                for (id, name) in &components {
                    let mut chosen = self.items.contains(id);
                    if ui.checkbox(&mut chosen, name).changed() {
                        if chosen {
                            self.items.push(*id);
                        } else {
                            self.items.retain(|item| item != id);
                        }
                    }
                }
                ui.separator();
                let mut on = true;
                ui.checkbox(&mut on, tr!(lang, GroundAtOrigin));
                let unavailable = tr!(lang, GroundRootUnavailable);
                for label in [tr!(lang, GroundFlushConstraints), tr!(lang, GroundRepositionTop)] {
                    let mut off = false;
                    ui.add_enabled(false, egui::Checkbox::new(&mut off, label))
                        .on_disabled_hover_text(unavailable);
                }
                ok = ui
                    .add_enabled(!self.items.is_empty(), egui::Button::new(tr!(lang, ConstrainOk)))
                    .clicked();
            });
        let mut error = None;
        if ok {
            match session.ground_at_origin(&self.items) {
                Ok(()) => open = false,
                Err(e) => error = Some(e.to_string()),
            }
        }
        self.open = open;
        error
    }
}
