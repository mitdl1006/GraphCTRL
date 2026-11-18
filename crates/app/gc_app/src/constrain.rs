//! The Constrain dialog (Inventor's Assemble tab, Relationships panel, Constrain): pick the first and
//! second geometry in the view, choose Mate, Flush or Insert, set the offset, press Apply or OK.
//! Reference: Inventor 2026 help, To Place Mate or Flush Constraints in Assemblies
//! (GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E).

use gc_doc::{AxisSense, Geometry, Occurrence, OriginElement, RelationKind, Relationship, Selection};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// Which kind of constraint is being placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Type {
    Mate,
    Flush,
    Insert,
}

/// The dialog's state.
pub(crate) struct Constrain {
    pub(crate) open: bool,
    kind: Type,
    first: Option<Selection>,
    second: Option<Selection>,
    /// The selection the next pick fills: 0 first, 1 second.
    slot: usize,
    offset: String,
    predict: bool,
    sense: AxisSense,
    aligned: bool,
    name: String,
    /// A relationship that could not be satisfied: the app opens the constraint error dialog for it.
    pub(crate) conflict: Option<crate::conflict::Conflict>,
}

impl Default for Constrain {
    fn default() -> Self {
        Self {
            open: false,
            kind: Type::Mate,
            first: None,
            second: None,
            slot: 0,
            offset: "0".to_owned(),
            predict: false,
            sense: AxisSense::Opposed,
            aligned: false,
            name: String::new(),
            conflict: None,
        }
    }
}

impl Constrain {
    /// Opens the dialog with nothing picked.
    pub(crate) fn start(&mut self) {
        *self = Self { open: true, ..Self::default() };
    }

    /// A pick in the view: it fills the active selection, then the other one.
    pub(crate) fn pick(&mut self, session: &Session, occurrence: Occurrence, element: OriginElement) {
        let selection = Selection { occurrence, geometry: Geometry::origin(element) };
        if self.slot == 0 {
            self.first = Some(selection);
            self.slot = 1;
        } else {
            self.second = Some(selection);
        }
        if self.predict
            && let (Some(first), Some(second)) = (&self.first, &self.second)
            && let Some(kind) = session.predict_relationship(first, second)
        {
            self.apply_prediction(kind);
        }
    }

    /// Predict Offset and Orientation: the current distance and the solution the layout suggests.
    fn apply_prediction(&mut self, kind: RelationKind) {
        match kind {
            RelationKind::Mate { offset, sense } => {
                self.kind = Type::Mate;
                self.sense = sense;
                self.offset = format!("{:.3}", offset * 1000.0);
            }
            RelationKind::Flush { offset } => {
                self.kind = Type::Flush;
                self.offset = format!("{:.3}", offset * 1000.0);
            }
            RelationKind::Insert { offset, aligned } => {
                self.kind = Type::Insert;
                self.aligned = aligned;
                self.offset = format!("{:.3}", offset * 1000.0);
            }
            // Joints are placed with the Joint command, not this dialog.
            RelationKind::Rigid { .. } | RelationKind::Rotational { .. } => {}
            // Kinds the backend adds later (angle, tangent, symmetry, more joints) predict nothing here.
            #[allow(unreachable_patterns, reason = "room for relationship kinds still to come")]
            _ => {}
        }
    }

    /// The constraint the dialog describes, or `None` while a selection or the offset is missing.
    fn relationship(&self, session: &Session) -> Option<Relationship> {
        let (first, second) = (self.first?, self.second?);
        let offset = self.offset.trim().parse::<f64>().ok()? / 1000.0;
        let kind = match self.kind {
            Type::Mate => RelationKind::Mate { offset, sense: self.sense },
            Type::Flush => RelationKind::Flush { offset },
            Type::Insert => RelationKind::Insert { offset, aligned: self.aligned },
        };
        let name = if self.name.trim().is_empty() {
            let word = match self.kind {
                Type::Mate => "Mate",
                Type::Flush => "Flush",
                Type::Insert => "Insert",
            };
            format!("{word}:{}", session.relationships().len() + 1)
        } else {
            self.name.trim().to_owned()
        };
        Some(Relationship::new(name, kind, first, second))
    }

    /// Whether the view is picking for the dialog.
    pub(crate) fn picking(&self) -> bool {
        self.open
    }

    /// The dialog window. Applies and closes as the buttons say.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut apply = false;
        let mut ok = false;
        let mut cancel = false;
        // Over the properties panel at the right: out of the way of what is being picked in the view.
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 330.0, screen.top() + 200.0);
        egui::Window::new(tr!(lang, ConstrainTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (kind, name) in [
                        (Type::Mate, tr!(lang, ConstrainMate)),
                        (Type::Flush, tr!(lang, ConstrainFlush)),
                        (Type::Insert, tr!(lang, ConstrainInsert)),
                    ] {
                        if ui.selectable_label(self.kind == kind, name).clicked() {
                            self.kind = kind;
                        }
                    }
                });
                ui.checkbox(&mut self.predict, tr!(lang, ConstrainPredict));
                for (slot, label, selection) in
                    [(0, tr!(lang, ConstrainFirst), self.first), (1, tr!(lang, ConstrainSecond), self.second)]
                {
                    ui.horizontal(|ui| {
                        let text = match selection {
                            Some(selection) => describe(session, &selection, lang),
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
                    });
                }
                if self.kind == Type::Mate {
                    ui.horizontal(|ui| {
                        for (sense, name) in [
                            (AxisSense::Opposed, tr!(lang, ConstrainOpposed)),
                            (AxisSense::Aligned, tr!(lang, ConstrainAligned)),
                            (AxisSense::Undirected, tr!(lang, ConstrainUndirected)),
                        ] {
                            if ui.selectable_label(self.sense == sense, name).clicked() {
                                self.sense = sense;
                            }
                        }
                    });
                }
                if self.kind == Type::Insert {
                    ui.checkbox(&mut self.aligned, tr!(lang, ConstrainAligned));
                }
                ui.horizontal(|ui| {
                    let label = ui.label(tr!(lang, ConstrainOffset));
                    let field = ui.add(egui::TextEdit::singleline(&mut self.offset).desired_width(80.0));
                    field.labelled_by(label.id);
                    ui.label("mm");
                });
                let ready = self.relationship(session).is_some();
                ui.horizontal(|ui| {
                    if ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked() {
                        ok = true;
                    }
                    if ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainApply))).clicked() {
                        apply = true;
                    }
                    if ui.button(tr!(lang, ConstrainCancel)).clicked() {
                        cancel = true;
                    }
                });
            });
        if (ok || apply)
            && let Some(relationship) = self.relationship(session)
        {
            // A constraint that cannot be solved is refused; the status bar says why.
            if crate::conflict::try_add(session, relationship, &mut self.conflict) {
                // Apply goes on with the next constraint, OK closes.
                self.first = None;
                self.second = None;
                self.slot = 0;
                self.name.clear();
                if ok {
                    open = false;
                }
            }
        }
        if cancel {
            open = false;
        }
        self.open = open;
    }
}

/// "Body1:1 · XY plane" for a selection.
pub(crate) fn describe(session: &Session, selection: &Selection, lang: Lang) -> String {
    let name = session
        .components()
        .iter()
        .find(|node| node.occurrence == selection.occurrence)
        .map_or_else(|| "?".to_owned(), |node| format!("{}:1", node.name));
    let what = match selection.geometry {
        Geometry::Plane { .. } => tr!(lang, ConstrainPlane),
        Geometry::Axis { .. } => tr!(lang, ConstrainAxis),
        Geometry::Point(_) => tr!(lang, ConstrainPoint),
        Geometry::Circle { .. } => tr!(lang, ConstrainCircle),
        Geometry::Frame { .. } => tr!(lang, ConstrainFrame),
        // Geometry the backend adds later (cylinders, spheres) until it has its own words.
        #[allow(unreachable_patterns, reason = "room for selection geometry still to come")]
        _ => tr!(lang, ConstrainGeometry),
    };
    format!("{name} · {what}")
}

/// The type of joint: Automatic picks Rigid or Rotational from the two origins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JointType {
    Automatic,
    Rigid,
    Rotational,
}

/// The Joint dialog (Inventor's Assemble tab, Relationships panel, Joint): pick the origin on the
/// component that moves and the origin on the one that stays, choose the type, flip the component if
/// needed, press Apply or OK. A joint origin is a point, axis or plane of the component's origin folder;
/// the centre point stands for the component's own origin frame.
/// Reference: Inventor 2026 help, Create Joints Reference (GUID-6AA68E8F-7C97-4806-8483-3941DE915E70) and
/// To Define and Manage Joint Relationships (GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6).
pub(crate) struct Joint {
    pub(crate) open: bool,
    kind: JointType,
    first: Option<Selection>,
    second: Option<Selection>,
    slot: usize,
    flip: bool,
    name: String,
    /// A relationship that could not be satisfied (see `Constrain::conflict`).
    pub(crate) conflict: Option<crate::conflict::Conflict>,
}

impl Default for Joint {
    fn default() -> Self {
        Self {
            open: false,
            kind: JointType::Automatic,
            first: None,
            second: None,
            slot: 0,
            flip: false,
            name: String::new(),
            conflict: None,
        }
    }
}

impl Joint {
    /// Opens the dialog with nothing picked.
    pub(crate) fn start(&mut self) {
        *self = Self { open: true, ..Self::default() };
    }

    /// A pick in the view: the origin of the moving component first, then the stationary one.
    pub(crate) fn pick(&mut self, occurrence: Occurrence, element: OriginElement) {
        let geometry = match element {
            OriginElement::Point => Geometry::origin_frame(),
            other => Geometry::origin(other),
        };
        let selection = Selection { occurrence, geometry };
        if self.slot == 0 {
            self.first = Some(selection);
            self.slot = 1;
        } else {
            self.second = Some(selection);
        }
    }

    fn relationship(&self, session: &Session) -> Option<Relationship> {
        let (first, second) = (self.first?, self.second?);
        let kind = match self.kind {
            JointType::Automatic => match session.automatic_joint(&first, &second) {
                RelationKind::Rotational { .. } => RelationKind::Rotational { flip: self.flip },
                _ => RelationKind::Rigid { flip: self.flip },
            },
            JointType::Rigid => RelationKind::Rigid { flip: self.flip },
            JointType::Rotational => RelationKind::Rotational { flip: self.flip },
        };
        let name = if self.name.trim().is_empty() {
            let word = if matches!(kind, RelationKind::Rotational { .. }) { "Rotational" } else { "Rigid" };
            format!("{word}:{}", session.relationships().len() + 1)
        } else {
            self.name.trim().to_owned()
        };
        Some(Relationship::new(name, kind, first, second))
    }

    /// The dialog window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let (mut apply, mut ok, mut cancel) = (false, false, false);
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 330.0, screen.top() + 200.0);
        egui::Window::new(tr!(lang, JointTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (kind, name) in [
                        (JointType::Automatic, tr!(lang, JointAutomatic)),
                        (JointType::Rigid, tr!(lang, JointRigid)),
                        (JointType::Rotational, tr!(lang, JointRotational)),
                    ] {
                        if ui.selectable_label(self.kind == kind, name).clicked() {
                            self.kind = kind;
                        }
                    }
                });
                for (slot, label, selection) in
                    [(0, tr!(lang, JointFirst), self.first), (1, tr!(lang, JointSecond), self.second)]
                {
                    let text = match selection {
                        Some(selection) => describe(session, &selection, lang),
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
                ui.checkbox(&mut self.flip, tr!(lang, JointFlip));
                let ready = self.relationship(session).is_some();
                ui.horizontal(|ui| {
                    ok = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
                    apply = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainApply))).clicked();
                    cancel = ui.button(tr!(lang, ConstrainCancel)).clicked();
                });
            });
        if (ok || apply)
            && let Some(relationship) = self.relationship(session)
            && crate::conflict::try_add(session, relationship, &mut self.conflict)
        {
            self.first = None;
            self.second = None;
            self.slot = 0;
            self.name.clear();
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
