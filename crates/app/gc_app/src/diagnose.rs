//! The Relationship Conflict Analysis dialog (Inventor 2026 help, Relationship Conflict Analysis Reference,
//! GUID-528B2CD0-93FD-45BC-AB69-6C5A559BB8E8): the relationships and components related to a conflict, each
//! relationship Keep or Break, each component's grounding turned round or not, Check for the solve result, and
//! Delete or Suppress the broken ones.

use gc_core::Id;
use gc_doc::{Occurrence, Relationship};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// The dialog's state.
#[derive(Default)]
pub(crate) struct Diagnose {
    pub(crate) open: bool,
    /// The relationships of the conflict, each with "break" on or off.
    relationships: Vec<(Id<Relationship>, bool)>,
    /// The components of the conflict, each with its grounding turned round or not.
    components: Vec<(Occurrence, bool)>,
    /// The result of the last Check.
    result: Option<bool>,
}

impl Diagnose {
    /// Opens the dialog for the conflict of a relationship.
    pub(crate) fn start(&mut self, session: &Session, id: Id<Relationship>) {
        let (blocking, keep) = session.related_failed(id);
        let mut relationships: Vec<(Id<Relationship>, bool)> = vec![(id, false)];
        relationships.extend(blocking.into_iter().filter(|other| *other != id).map(|other| (other, false)));
        *self = Self {
            open: true,
            relationships,
            components: keep.into_iter().map(|c| (c, false)).collect(),
            result: None,
        };
    }

    fn broken(&self) -> Vec<Id<Relationship>> {
        self.relationships.iter().filter(|(_, b)| *b).map(|(id, _)| *id).collect()
    }

    /// The dialog window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut check = false;
        let mut delete = false;
        let mut suppress = false;
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 360.0, screen.top() + 160.0);
        let names: Vec<(Id<Relationship>, String)> =
            session.relationships().into_iter().map(|(id, r)| (id, r.name.clone())).collect();
        let components: Vec<(Occurrence, String)> =
            session.components().iter().map(|n| (n.occurrence, n.name.clone())).collect();
        egui::Window::new(tr!(lang, DiagnoseTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(tr!(lang, DiagnoseRelationships)).strong());
                for (id, broken) in &mut self.relationships {
                    let name = names.iter().find(|(other, _)| other == id).map_or("?", |(_, n)| n.as_str());
                    ui.horizontal(|ui| {
                        ui.label(name);
                        ui.selectable_value(broken, false, tr!(lang, DiagnoseKeep));
                        ui.selectable_value(broken, true, tr!(lang, DiagnoseBreak));
                    });
                }
                ui.separator();
                ui.label(egui::RichText::new(tr!(lang, DiagnoseComponents)).strong());
                for (occurrence, toggled) in &mut self.components {
                    let name =
                        components.iter().find(|(o, _)| o == occurrence).map_or("?", |(_, n)| n.as_str());
                    let grounded = session.doc().component_flag(*occurrence, gc_doc::ComponentFlag::Grounded);
                    // The grounding as it will be after Check: turned round when toggled.
                    let label = if grounded != *toggled {
                        format!("{name} ({})", tr!(lang, DiagnoseGrounded))
                    } else {
                        name.to_owned()
                    };
                    ui.checkbox(toggled, label);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    check = ui.button(tr!(lang, DiagnoseCheck)).clicked();
                    match self.result {
                        Some(true) => ui.label(tr!(lang, DiagnoseSolved)),
                        Some(false) => {
                            ui.colored_label(ui.visuals().error_fg_color, tr!(lang, DiagnoseConflicts))
                        }
                        None => ui.weak(tr!(lang, DiagnoseNotChecked)),
                    };
                });
                let any = !self.broken().is_empty();
                ui.horizontal(|ui| {
                    suppress =
                        ui.add_enabled(any, egui::Button::new(tr!(lang, DiagnoseSuppressBroken))).clicked();
                    delete =
                        ui.add_enabled(any, egui::Button::new(tr!(lang, DiagnoseDeleteBroken))).clicked();
                });
            });
        let toggled: Vec<Occurrence> = self.components.iter().filter(|(_, t)| *t).map(|(o, _)| *o).collect();
        if check {
            self.result = Some(session.conflict_check(&self.broken(), &toggled));
        }
        if suppress || delete {
            for id in self.broken() {
                if suppress {
                    drop(session.set_relationship_suppressed(id, true));
                } else {
                    drop(session.delete_relationship(id));
                }
            }
            open = false;
        }
        self.open = open;
    }
}
