//! The constraint error dialog (Inventor's Place/Edit Connection Error and Design Doctor flow): a
//! relationship that cannot be satisfied together with the others is refused; the dialog names the
//! relationships that stand in the way and offers to suppress or delete them, then places the new
//! relationship again.
//! Reference: Inventor 2026 help, Place/Edit Connection Error Reference and Use Design Doctor to repair
//! relationship errors (full GUIDs not recorded in docs/ux/inventor-assembly-parity.md, A34/A36).

use gc_core::Id;
use gc_doc::{EditError, Relationship};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// A relationship that was refused, with what the solver said.
pub(crate) struct Conflict {
    pub(crate) candidate: Relationship,
    pub(crate) names: Vec<String>,
}

/// Adds the relationship. When it cannot be satisfied, `conflict` receives it for the dialog.
/// Returns whether it was added.
pub(crate) fn try_add(
    session: &mut Session,
    relationship: Relationship,
    conflict: &mut Option<Conflict>,
) -> bool {
    match session.add_relationship(relationship.clone()) {
        Ok(_) => true,
        Err(EditError::Unsolvable { names, .. }) => {
            *conflict = Some(Conflict { candidate: relationship, names });
            false
        }
        Err(_) => false,
    }
}

/// The dialog's state.
#[derive(Default)]
pub(crate) struct ConstraintError {
    pending: Option<Conflict>,
    /// The existing relationships each of which alone stands in the way.
    culprits: Vec<Id<Relationship>>,
}

impl ConstraintError {
    /// Shows the dialog for a refused relationship.
    pub(crate) fn show_for(&mut self, session: &Session, conflict: Conflict) {
        self.culprits = session.conflicting_relationships(Some(&conflict.candidate));
        self.pending = Some(conflict);
    }

    /// The dialog window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let Some(conflict) = &self.pending else { return };
        let message = tr!(lang, ConflictMessage, conflict.names.join(", "));
        let mut suppress = None;
        let mut delete = None;
        let mut close = false;
        egui::Window::new(tr!(lang, ConflictTitle))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(message);
                ui.add_space(6.0);
                if self.culprits.is_empty() {
                    ui.label(tr!(lang, ConflictNoSingle));
                } else {
                    ui.label(egui::RichText::new(tr!(lang, ConflictCulprits)).strong());
                    for id in &self.culprits {
                        let name = session
                            .relationships()
                            .into_iter()
                            .find(|(other, _)| other == id)
                            .map_or_else(String::new, |(_, r)| r.name.clone());
                        ui.horizontal(|ui| {
                            ui.label(&name);
                            if ui.button(tr!(lang, CmdSuppress)).on_hover_text(&name).clicked() {
                                suppress = Some(*id);
                            }
                            if ui.button(tr!(lang, ConflictDelete)).on_hover_text(&name).clicked() {
                                delete = Some(*id);
                            }
                        });
                    }
                }
                ui.add_space(6.0);
                if ui.button(tr!(lang, ConstrainCancel)).clicked() {
                    close = true;
                }
            });
        if let Some(id) = suppress {
            drop(session.set_relationship_suppressed(id, true));
            self.retry(session);
        } else if let Some(id) = delete {
            drop(session.delete_relationship(id));
            self.retry(session);
        } else if close {
            self.pending = None;
        }
    }

    /// Places the refused relationship again; the dialog closes when it succeeds and shows the
    /// remaining culprits when it does not.
    fn retry(&mut self, session: &mut Session) {
        let Some(conflict) = self.pending.take() else { return };
        let mut next = None;
        if !try_add(session, conflict.candidate, &mut next) {
            if let Some(next) = next {
                self.show_for(session, next);
            }
        } else {
            self.culprits.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gc_doc::{AxisSense, Geometry, Occurrence, OriginElement, OriginPlane, RelationKind, Selection};

    fn mate(name: &str, offset: f64, moving: gc_core::AnyId, fixed: gc_core::AnyId) -> Relationship {
        let plane = Geometry::origin(OriginElement::Plane(OriginPlane::Xy));
        let select = |item| Selection { occurrence: Occurrence::Item(item), geometry: plane };
        Relationship::new(
            name,
            RelationKind::Mate { offset, sense: AxisSense::Opposed },
            select(moving),
            select(fixed),
        )
    }

    /// Two mates of the same planes with different offsets cannot both hold: the second is refused, the
    /// dialog names the first, and suppressing it lets the second in.
    #[test]
    fn a_refused_mate_is_placed_after_the_culprit_is_suppressed() {
        let mut session = Session::new();
        let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
        let fixed = session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
        let moving =
            session.add_body(solid, gc_doc::Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] }).unwrap();
        session.add_relationship(mate("Mate:1", 0.0, moving.erase(), fixed.erase())).unwrap();
        let first = session.relationships()[0].0;

        let mut refused = None;
        let candidate = mate("Mate:2", 0.1, moving.erase(), fixed.erase());
        assert!(!try_add(&mut session, candidate, &mut refused));
        let conflict = refused.expect("the solver refuses it");
        let mut dialog = ConstraintError::default();
        dialog.show_for(&session, conflict);
        assert_eq!(dialog.culprits, vec![first]);

        session.set_relationship_suppressed(first, true).unwrap();
        dialog.retry(&mut session);
        assert!(dialog.pending.is_none(), "the retry went through");
        assert_eq!(session.relationships().len(), 2);
    }
}
