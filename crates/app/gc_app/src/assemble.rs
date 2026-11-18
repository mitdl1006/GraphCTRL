//! The Assemble command (Inventor's Assemble tab, Relationships panel, Assemble): pick geometry on the
//! component that moves, then on the component that stays; the geometry decides which constraint types
//! are offered, one of which is chosen with its offset or angle. Apply keeps going, OK ends.
//! Reference: Inventor 2026 help, Create assembly constraints using the Assemble command
//! (GUID-C1E8D955-784D-47DE-9DD4-C9C695051320).

use gc_doc::{AssembleSolution, Geometry, Occurrence, OriginElement, Relationship, Selection};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// The dialog's state.
pub(crate) struct Assemble {
    pub(crate) open: bool,
    first: Option<Selection>,
    second: Option<Selection>,
    /// The selection the next pick fills: 0 first, 1 second.
    slot: usize,
    solution: Option<AssembleSolution>,
    /// Millimetres, or degrees for an angle.
    value: String,
    /// A relationship that could not be satisfied: the app opens the constraint error dialog for it.
    pub(crate) conflict: Option<crate::conflict::Conflict>,
}

impl Default for Assemble {
    fn default() -> Self {
        Self {
            open: false,
            first: None,
            second: None,
            slot: 0,
            solution: None,
            value: "0".to_owned(),
            conflict: None,
        }
    }
}

fn is_angle(solution: AssembleSolution) -> bool {
    matches!(solution, AssembleSolution::AngleDirected)
}

/// Alt + drag: a Mate of the dragged component's geometry on what it was dropped on, with the first
/// solution the geometry offers and no offset (Inventor 2026 help, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E).
pub(crate) fn quick_mate(
    session: &mut Session,
    first: Selection,
    second: Selection,
    conflict: &mut Option<crate::conflict::Conflict>,
) {
    let Some(solution) = session.possible_relations(&first, &second).first().copied() else { return };
    let name = format!("{}:{}", solution_word(solution), session.relationships().len() + 1);
    let relationship = Relationship::new(name, solution.kind(0.0), first, second);
    crate::conflict::try_add(session, relationship, conflict);
}

impl Assemble {
    /// Opens the command with nothing picked.
    pub(crate) fn start(&mut self) {
        *self = Self { open: true, ..Self::default() };
    }

    /// A pick in the view: the first selection, then the second.
    pub(crate) fn pick(&mut self, session: &Session, occurrence: Occurrence, element: OriginElement) {
        let selection = Selection { occurrence, geometry: Geometry::origin(element) };
        self.pick_selection(session, selection);
    }

    /// A pick of any geometry: the first selection, then the second.
    pub(crate) fn pick_selection(&mut self, session: &Session, selection: Selection) {
        if self.slot == 0 {
            self.first = Some(selection);
            self.slot = 1;
        } else {
            self.second = Some(selection);
        }
        // The geometry decides what is offered; the first solution is the default.
        self.refresh(session);
    }

    fn refresh(&mut self, session: &Session) {
        let offered = self.offered(session);
        if !self.solution.is_some_and(|s| offered.contains(&s)) {
            self.solution = offered.first().copied();
        }
    }

    fn offered(&self, session: &Session) -> Vec<AssembleSolution> {
        match (&self.first, &self.second) {
            (Some(first), Some(second)) => session.possible_relations(first, second),
            _ => Vec::new(),
        }
    }

    fn relationship(&self, session: &Session) -> Option<Relationship> {
        let (first, second, solution) = (self.first?, self.second?, self.solution?);
        let typed = self.value.trim().parse::<f64>().ok()?;
        let value = if is_angle(solution) { typed.to_radians() } else { typed / 1000.0 };
        let kind = solution.kind(value);
        let name = format!("{}:{}", solution_word(solution), session.relationships().len() + 1);
        Some(Relationship::new(name, kind, first, second))
    }

    /// Whether the view is picking for the command.
    pub(crate) fn picking(&self) -> bool {
        self.open
    }

    /// The command's window. Applies and closes as the buttons say.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let (mut ok, mut apply, mut cancel, mut undo) = (false, false, false, false);
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 330.0, screen.top() + 200.0);
        let offered = self.offered(session);
        egui::Window::new(tr!(lang, AssembleTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(300.0)
            .show(ctx, |ui| {
                for (slot, label, selection) in
                    [(0, tr!(lang, ConstrainFirst), self.first), (1, tr!(lang, ConstrainSecond), self.second)]
                {
                    let text = match selection {
                        Some(selection) => crate::constrain::describe(session, &selection, lang),
                        None => tr!(lang, ConstrainPick).to_owned(),
                    };
                    let name = format!("{label}: {text}");
                    let button = ui.selectable_label(self.slot == slot, name.clone());
                    button.widget_info(|| {
                        egui::WidgetInfo::selected(egui::Role::Button, true, self.slot == slot, &name)
                    });
                    if button.clicked() {
                        self.slot = slot;
                    }
                }
                ui.separator();
                if self.first.is_some() && self.second.is_some() {
                    if offered.is_empty() {
                        ui.label(tr!(lang, AssembleNone));
                    }
                    // The solutions this geometry allows, as the mini toolbar of Inventor's command.
                    ui.horizontal_wrapped(|ui| {
                        for solution in &offered {
                            if ui
                                .selectable_label(
                                    self.solution == Some(*solution),
                                    solution_name(*solution, lang),
                                )
                                .clicked()
                            {
                                self.solution = Some(*solution);
                            }
                        }
                    });
                    if let Some(solution) = self.solution {
                        ui.horizontal(|ui| {
                            let label = ui.label(if is_angle(solution) {
                                tr!(lang, AssembleAngle)
                            } else {
                                tr!(lang, ConstrainOffset)
                            });
                            let field =
                                ui.add(egui::TextEdit::singleline(&mut self.value).desired_width(80.0));
                            field.labelled_by(label.id);
                            ui.label(if is_angle(solution) { "°" } else { "mm" });
                        });
                    }
                }
                let ready = self.relationship(session).is_some();
                ui.horizontal(|ui| {
                    ok = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
                    apply = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainApply))).clicked();
                    undo = ui.button(tr!(lang, AssembleUndo)).clicked();
                    cancel = ui.button(tr!(lang, ConstrainCancel)).clicked();
                });
            });
        if undo {
            // Deletes the selections and goes on defining constraints.
            self.first = None;
            self.second = None;
            self.slot = 0;
            self.solution = None;
        }
        if (ok || apply)
            && let Some(relationship) = self.relationship(session)
            && crate::conflict::try_add(session, relationship, &mut self.conflict)
        {
            self.first = None;
            self.second = None;
            self.slot = 0;
            self.solution = None;
            if ok {
                open = false;
            }
        }
        if cancel {
            open = false;
        }
        self.open = open;
    }
}

/// The word that starts the relationship's name.
fn solution_word(solution: AssembleSolution) -> &'static str {
    match solution {
        AssembleSolution::MateMate | AssembleSolution::MateAligned => "Mate",
        AssembleSolution::MateFlush => "Flush",
        AssembleSolution::AngleDirected => "Angle",
        AssembleSolution::TangentInside | AssembleSolution::TangentOutside => "Tangent",
        AssembleSolution::InsertOpposed | AssembleSolution::InsertAligned => "Insert",
    }
}

fn solution_name(solution: AssembleSolution, lang: Lang) -> &'static str {
    match solution {
        AssembleSolution::MateMate => tr!(lang, SolMateMate),
        AssembleSolution::MateFlush => tr!(lang, SolMateFlush),
        AssembleSolution::MateAligned => tr!(lang, SolMateAligned),
        AssembleSolution::AngleDirected => tr!(lang, SolAngleDirected),
        AssembleSolution::TangentInside => tr!(lang, SolTangentInside),
        AssembleSolution::TangentOutside => tr!(lang, SolTangentOutside),
        AssembleSolution::InsertOpposed => tr!(lang, SolInsertOpposed),
        AssembleSolution::InsertAligned => tr!(lang, SolInsertAligned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gc_doc::{OriginPlane, Placement, Solid};

    /// Two origin planes offer Mate, Flush and Angle; the chosen one is placed with its offset and the
    /// moving component ends up there.
    #[test]
    fn two_planes_offer_mate_flush_and_angle_and_the_chosen_one_is_placed() {
        let mut session = Session::new();
        let solid = Solid::Box { size: [0.05; 3] };
        let fixed = session.add_body(solid.clone(), Placement::default()).unwrap();
        let moving = session.add_body(solid, Placement { xyz: [0.0, 0.0, 0.3], rpy: [0.0; 3] }).unwrap();
        let plane = OriginElement::Plane(OriginPlane::Xy);

        let mut command = Assemble::default();
        command.start();
        command.pick(&session, Occurrence::Item(moving.erase()), plane);
        assert!(command.offered(&session).is_empty(), "one pick offers nothing yet");
        command.pick(&session, Occurrence::Item(fixed.erase()), plane);
        let offered = command.offered(&session);
        for wanted in
            [AssembleSolution::MateMate, AssembleSolution::MateFlush, AssembleSolution::AngleDirected]
        {
            assert!(offered.contains(&wanted), "{wanted:?} in {offered:?}");
        }
        assert_eq!(command.solution, Some(AssembleSolution::MateMate), "the first solution is the default");

        command.solution = Some(AssembleSolution::MateFlush);
        command.value = "20".to_owned();
        let relationship = command.relationship(&session).expect("ready");
        assert!(relationship.name.starts_with("Flush"));
        let mut refused = None;
        assert!(crate::conflict::try_add(&mut session, relationship, &mut refused));
        let z = session.doc().get(moving).unwrap().placement.xyz[2];
        assert!((z - 0.02).abs() < 1e-6, "flush at 20 mm: {z}");
    }
}
