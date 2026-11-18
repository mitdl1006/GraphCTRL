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
    Angle,
    Tangent,
    Insert,
    Symmetry,
}

/// The solutions of an Angle constraint (Inventor 2026 help, To Place Angle Constraints in Assemblies,
/// GUID-AD4679FD-EE9B-47C0-9E32-04B990BDD29E).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Solution {
    Directed,
    Undirected,
    Explicit,
}

/// The dialog's state.
pub(crate) struct Constrain {
    pub(crate) open: bool,
    kind: Type,
    first: Option<Selection>,
    second: Option<Selection>,
    /// The third selection: an Angle's Explicit Reference Vector, or the plane of a Symmetry.
    third: Option<Selection>,
    solution: Solution,
    /// Tangent: the first inside the second.
    inside: bool,
    /// The selection the next pick fills: 0 first, 1 second, 2 third.
    slot: usize,
    offset: String,
    predict: bool,
    sense: AxisSense,
    aligned: bool,
    name: String,
    /// The More section: Maximum and Minimum with their values (mm) and Use Offset As Resting Position.
    max_on: bool,
    max: String,
    min_on: bool,
    min: String,
    resting: bool,
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
            third: None,
            solution: Solution::Directed,
            inside: false,
            slot: 0,
            offset: "0".to_owned(),
            predict: false,
            sense: AxisSense::Opposed,
            aligned: false,
            name: String::new(),
            max_on: false,
            max: "0".to_owned(),
            min_on: false,
            min: "0".to_owned(),
            resting: false,
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
        self.pick_selection(session, selection);
    }

    /// A pick of any geometry: the first selection, then the second.
    pub(crate) fn pick_selection(&mut self, session: &Session, selection: Selection) {
        match self.slot {
            0 => {
                self.first = Some(selection);
                self.slot = 1;
            }
            1 => {
                self.second = Some(selection);
                if self.needs_third() {
                    self.slot = 2;
                }
            }
            _ => self.third = Some(selection),
        }
        if self.predict
            && let (Some(first), Some(second)) = (&self.first, &self.second)
            && let Some(kind) = session.predict_relationship(first, second)
        {
            self.apply_prediction(kind);
        }
    }

    /// Whether the constraint takes a third selection.
    fn needs_third(&self) -> bool {
        self.kind == Type::Symmetry || (self.kind == Type::Angle && self.solution == Solution::Explicit)
    }

    /// The unit a bare number in the value box means.
    fn unit(&self) -> gc_doc::Unit {
        if self.kind == Type::Angle { gc_doc::Unit::Deg } else { gc_doc::Unit::Mm }
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
            RelationKind::Angle { angle, .. } => {
                self.kind = Type::Angle;
                self.offset = format!("{:.3}", angle.to_degrees());
            }
            RelationKind::Tangent { offset, inside } => {
                self.kind = Type::Tangent;
                self.inside = inside;
                self.offset = format!("{:.3}", offset * 1000.0);
            }
            // Joints are placed with the Joint command, not this dialog.
            RelationKind::Rigid { .. } | RelationKind::Rotational { .. } => {}
            // Symmetry needs its plane from a third pick; the other joints are placed with Joint.
            #[allow(unreachable_patterns, reason = "room for relationship kinds still to come")]
            _ => {}
        }
    }

    /// The constraint the dialog describes, or `None` while a selection or the offset is missing.
    fn relationship(&self, session: &Session) -> Option<Relationship> {
        let (first, second) = (self.first?, self.second?);
        // A number (mm, or degrees for an angle), or what an expression with units and parameter names gives
        // (metres, radians). Symmetry has no value.
        let value = if self.kind == Type::Symmetry {
            0.0
        } else {
            gc_doc::evaluate_value(session.doc(), &self.offset, self.unit())?
        };
        let kind = match self.kind {
            Type::Mate => RelationKind::Mate { offset: value, sense: self.sense },
            Type::Flush => RelationKind::Flush { offset: value },
            Type::Insert => RelationKind::Insert { offset: value, aligned: self.aligned },
            Type::Tangent => RelationKind::Tangent { offset: value, inside: self.inside },
            Type::Angle => RelationKind::Angle {
                angle: value,
                solution: match self.solution {
                    Solution::Directed => gc_doc::AngleSolution::Directed,
                    Solution::Undirected => gc_doc::AngleSolution::Undirected,
                    Solution::Explicit => gc_doc::AngleSolution::Explicit(self.third?),
                },
            },
            Type::Symmetry => RelationKind::Symmetry { plane: self.third? },
        };
        let name = if self.name.trim().is_empty() {
            let word = match self.kind {
                Type::Mate => "Mate",
                Type::Flush => "Flush",
                Type::Insert => "Insert",
                Type::Angle => "Angle",
                Type::Tangent => "Tangent",
                Type::Symmetry => "Symmetry",
            };
            format!("{word}:{}", session.relationships().len() + 1)
        } else {
            self.name.trim().to_owned()
        };
        // Limits (Inventor 2026 help, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E: "Check Maximum ... Check
        // Minimum ... Use Offset As Resting Position"): millimetres here, metres in the document.
        let value = |on: bool, text: &str| -> Result<Option<f64>, ()> {
            if !on {
                return Ok(None);
            }
            text.trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| Some(v / 1000.0)).ok_or(())
        };
        let limited = matches!(self.kind, Type::Mate | Type::Flush | Type::Insert);
        let (min, max) = if limited {
            (value(self.min_on, &self.min).ok()?, value(self.max_on, &self.max).ok()?)
        } else {
            (None, None)
        };
        if let (Some(min), Some(max)) = (min, max)
            && min > max
        {
            return None;
        }
        let mut relationship = Relationship::new(name, kind, first, second);
        if limited && (min.is_some() || max.is_some() || self.resting) {
            relationship.limits = Some(gc_doc::Limits { min, max, resting: self.resting });
        }
        Some(relationship)
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
                        (Type::Angle, tr!(lang, ConstrainAngle)),
                        (Type::Tangent, tr!(lang, ConstrainTangent)),
                        (Type::Insert, tr!(lang, ConstrainInsert)),
                        (Type::Symmetry, tr!(lang, ConstrainSymmetry)),
                    ] {
                        if ui.selectable_label(self.kind == kind, name).clicked() {
                            self.kind = kind;
                            if self.slot == 2 && !self.needs_third() {
                                self.slot = 1;
                            }
                        }
                    }
                });
                ui.checkbox(&mut self.predict, tr!(lang, ConstrainPredict));
                let mut rows = vec![
                    (0, tr!(lang, ConstrainFirst), self.first),
                    (1, tr!(lang, ConstrainSecond), self.second),
                ];
                if self.needs_third() {
                    let label = if self.kind == Type::Symmetry {
                        tr!(lang, ConstrainSymmetryPlane)
                    } else {
                        tr!(lang, ConstrainThird)
                    };
                    rows.push((2, label, self.third));
                }
                for (slot, label, selection) in rows {
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
                if self.kind == Type::Tangent {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.inside, true, tr!(lang, ConstrainInside));
                        ui.selectable_value(&mut self.inside, false, tr!(lang, ConstrainOutside));
                    });
                }
                if self.kind == Type::Angle {
                    ui.horizontal(|ui| {
                        for (solution, name) in [
                            (Solution::Directed, tr!(lang, ConstrainDirected)),
                            (Solution::Undirected, tr!(lang, ConstrainUndirectedAngle)),
                            (Solution::Explicit, tr!(lang, ConstrainExplicit)),
                        ] {
                            if ui.selectable_label(self.solution == solution, name).clicked() {
                                self.solution = solution;
                                if !self.needs_third() && self.slot == 2 {
                                    self.slot = 1;
                                }
                            }
                        }
                    });
                }
                if self.kind != Type::Symmetry {
                    ui.horizontal(|ui| {
                        let angle = self.kind == Type::Angle;
                        let label = ui.label(if angle {
                            tr!(lang, ConstrainAngleValue)
                        } else {
                            tr!(lang, ConstrainOffset)
                        });
                        let field = ui.add(egui::TextEdit::singleline(&mut self.offset).desired_width(80.0));
                        field.labelled_by(label.id);
                        ui.label(if angle { "deg" } else { "mm" });
                    });
                    if gc_doc::evaluate_value(session.doc(), &self.offset, self.unit()).is_none() {
                        ui.colored_label(ui.visuals().error_fg_color, tr!(lang, ErrExpression));
                    }
                }
                let limits = matches!(self.kind, Type::Mate | Type::Flush | Type::Insert);
                // More: the name and the limits.
                egui::CollapsingHeader::new(tr!(lang, ConstrainMore)).id_salt("gc_constrain_more").show(
                    ui,
                    |ui| {
                        ui.horizontal(|ui| {
                            let label = ui.label(tr!(lang, ConstrainName));
                            ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(140.0))
                                .labelled_by(label.id);
                        });
                        // Limits are for the offsets (Mate, Flush, Insert); an angle's are not offered here.
                        if limits {
                            for (on, text, label) in [
                                (&mut self.max_on, &mut self.max, tr!(lang, ConstrainMax)),
                                (&mut self.min_on, &mut self.min, tr!(lang, ConstrainMin)),
                            ] {
                                ui.horizontal(|ui| {
                                    ui.checkbox(on, label);
                                    let field = ui.add_enabled(
                                        *on,
                                        egui::TextEdit::singleline(text).desired_width(80.0),
                                    );
                                    field.widget_info(|| {
                                        egui::WidgetInfo::labeled(egui::Role::TextInput, *on, label)
                                    });
                                    ui.label("mm");
                                });
                            }
                            ui.checkbox(&mut self.resting, tr!(lang, ConstrainResting));
                        }
                    },
                );
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
                // An expression is kept as typed: the constraint's model parameter takes it (Inventor 2026 help,
                // About Parameters in Models, GUID-8E5C63EA-0540-4EBE-9B17-B8159C7FE40C).
                if self.kind != Type::Symmetry
                    && self.offset.trim().parse::<f64>().is_err()
                    && let Some(id) = session.relationships().last().map(|(id, _)| *id)
                {
                    drop(session.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetRelationValue {
                        id,
                        source: self.offset.clone(),
                    })));
                }
                // Apply goes on with the next constraint, OK closes.
                self.first = None;
                self.second = None;
                self.third = None;
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
    let work = session.doc().work_features().iter().find_map(|(id, feature)| {
        (Occurrence::Item(id.erase()) == selection.occurrence).then(|| feature.name.clone())
    });
    let name = work.unwrap_or_else(|| {
        session
            .components()
            .iter()
            .find(|node| node.occurrence == selection.occurrence)
            .map_or_else(|| "?".to_owned(), |node| format!("{}:1", node.name))
    });
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
    /// The tab shown: the joint itself or its Limits.
    tab: JointTab,
    /// The joint being edited (its Limits), when the dialog was opened on one that exists.
    editing: Option<gc_core::Id<Relationship>>,
    /// The Limits tab's values.
    limits: crate::joint_limits::LimitsEdit,
    /// A relationship that could not be satisfied (see `Constrain::conflict`).
    pub(crate) conflict: Option<crate::conflict::Conflict>,
}

/// The tabs of the Joint dialog (Inventor 2026 help, GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JointTab {
    Joint,
    Limits,
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
            tab: JointTab::Joint,
            editing: None,
            limits: crate::joint_limits::LimitsEdit::default(),
            conflict: None,
        }
    }
}

impl Joint {
    /// Opens the dialog with nothing picked.
    pub(crate) fn start(&mut self) {
        *self = Self { open: true, ..Self::default() };
    }

    /// Opens the dialog on a joint that exists, at its Limits tab (the browser's "한계…").
    pub(crate) fn edit_limits(&mut self, session: &Session, id: gc_core::Id<Relationship>) {
        let Some(relationship) = session.doc().get(id) else { return };
        *self = Self {
            open: true,
            first: Some(relationship.first),
            second: Some(relationship.second),
            tab: JointTab::Limits,
            editing: Some(id),
            limits: crate::joint_limits::LimitsEdit::from(session.joint_limits(id)),
            ..Self::default()
        };
    }

    /// A pick in the view: the origin of the moving component first, then the stationary one.
    pub(crate) fn pick(&mut self, occurrence: Occurrence, element: OriginElement) {
        // A joint that exists keeps its origins; only its Limits are edited.
        if self.editing.is_some() {
            return;
        }
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
                // Two tabs: the joint and its Limits.
                ui.horizontal(|ui| {
                    for (tab, name) in [
                        (JointTab::Joint, tr!(lang, JointTabPlacement)),
                        (JointTab::Limits, tr!(lang, JointTabLimits)),
                    ] {
                        if ui.selectable_label(self.tab == tab, name).clicked() {
                            self.tab = tab;
                        }
                    }
                });
                ui.separator();
                // What kind of joint the limits are for: the one being edited, or the one the picks give.
                let kind = match self.editing {
                    Some(id) => session.doc().get(id).map(|r| r.kind),
                    None => self.relationship(session).map(|r| r.kind),
                };
                let angular = kind.is_some_and(|k| gc_doc::takes_limits(&k, true));
                let linear = kind.is_some_and(|k| gc_doc::takes_limits(&k, false));
                match self.tab {
                    JointTab::Joint => {
                        let editing = self.editing.is_some();
                        ui.add_enabled_ui(!editing, |ui| {
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
                        });
                        for (slot, label, selection) in
                            [(0, tr!(lang, JointFirst), self.first), (1, tr!(lang, JointSecond), self.second)]
                        {
                            let text = match selection {
                                Some(selection) => describe(session, &selection, lang),
                                None => tr!(lang, ConstrainPick).to_owned(),
                            };
                            let name = format!("{label}: {text}");
                            if editing {
                                ui.label(name);
                                continue;
                            }
                            let button = ui.selectable_label(self.slot == slot, name.clone());
                            button.widget_info(|| {
                                egui::WidgetInfo::selected(egui::Role::Button, true, self.slot == slot, &name)
                            });
                            if button.clicked() {
                                self.slot = slot;
                            }
                        }
                        ui.add_enabled(!editing, egui::Checkbox::new(&mut self.flip, tr!(lang, JointFlip)));
                    }
                    JointTab::Limits => {
                        let position = self.editing.and_then(|id| session.joint_position(id));
                        self.limits.ui(ui, angular, linear, position, lang);
                    }
                }
                let ready = (self.editing.is_some() || self.relationship(session).is_some())
                    && self.limits.valid(angular, linear);
                ui.horizontal(|ui| {
                    ok = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
                    apply = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainApply))).clicked();
                    cancel = ui.button(tr!(lang, ConstrainCancel)).clicked();
                });
            });
        if ok || apply {
            if let Some(id) = self.editing {
                // The joint exists: only its Limits change. A refusal is explained in the status bar.
                if let Some(kind) = session.doc().get(id).map(|r| r.kind) {
                    let (angular, linear) = self
                        .limits
                        .ranges(gc_doc::takes_limits(&kind, true), gc_doc::takes_limits(&kind, false));
                    drop(session.set_joint_limits(id, angular, linear));
                }
                if ok {
                    open = false;
                }
            } else if let Some(relationship) = self.relationship(session) {
                let kind = relationship.kind;
                if crate::conflict::try_add(session, relationship, &mut self.conflict) {
                    // The Limits tab's values go with the new joint.
                    let (angular, linear) = self
                        .limits
                        .ranges(gc_doc::takes_limits(&kind, true), gc_doc::takes_limits(&kind, false));
                    if (angular.is_some() || linear.is_some())
                        && let Some((id, _)) = session.relationships().into_iter().last()
                    {
                        drop(session.set_joint_limits(id, angular, linear));
                    }
                    self.first = None;
                    self.second = None;
                    self.slot = 0;
                    self.name.clear();
                    self.limits = crate::joint_limits::LimitsEdit::default();
                    if ok {
                        open = false;
                    }
                }
            }
        }
        if cancel {
            open = false;
        }
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gc_doc::{OriginPlane, Placement, Solid};

    fn pick(session: &Session, dialog: &mut Constrain, body: gc_core::AnyId, plane: OriginPlane) {
        dialog.pick(session, Occurrence::Item(body), OriginElement::Plane(plane));
    }

    /// Angle takes degrees and its solution (Explicit needs a third selection); Tangent takes the side; Symmetry
    /// takes a plane as its third selection (Inventor 2026 help, GUID-AD4679FD-EE9B-47C0-9E32-04B990BDD29E).
    #[test]
    fn angle_tangent_and_symmetry_are_described_by_the_dialog() {
        let mut session = Session::new();
        let solid = Solid::Box { size: [0.05; 3] };
        let a = session.add_body(solid.clone(), Placement::default()).unwrap().erase();
        let b = session.add_body(solid, Placement { xyz: [0.2, 0.0, 0.0], rpy: [0.0; 3] }).unwrap().erase();
        let mut dialog = Constrain::default();
        dialog.start();
        pick(&session, &mut dialog, a, OriginPlane::Xy);
        pick(&session, &mut dialog, b, OriginPlane::Xy);

        dialog.kind = Type::Angle;
        dialog.offset = "30".to_owned();
        let angle = dialog.relationship(&session).expect("an angle");
        match angle.kind {
            RelationKind::Angle { angle, solution } => {
                assert!((angle - 30.0_f64.to_radians()).abs() < 1e-9);
                assert_eq!(solution, gc_doc::AngleSolution::Directed);
            }
            other => panic!("{other:?}"),
        }
        // Explicit Reference Vector waits for the third selection, which the next pick gives.
        dialog.solution = Solution::Explicit;
        assert!(dialog.relationship(&session).is_none());
        dialog.slot = 2;
        pick(&session, &mut dialog, a, OriginPlane::Yz);
        assert!(matches!(
            dialog.relationship(&session).map(|r| r.kind),
            Some(RelationKind::Angle { solution: gc_doc::AngleSolution::Explicit(_), .. })
        ));

        dialog.kind = Type::Tangent;
        dialog.inside = true;
        dialog.offset = "5".to_owned();
        assert!(matches!(
            dialog.relationship(&session).map(|r| r.kind),
            Some(RelationKind::Tangent { inside: true, offset }) if (offset - 0.005).abs() < 1e-9
        ));

        dialog.kind = Type::Symmetry;
        assert!(matches!(dialog.relationship(&session).map(|r| r.kind), Some(RelationKind::Symmetry { .. })));
        dialog.third = None;
        assert!(dialog.relationship(&session).is_none(), "a symmetry needs its plane");
    }
}
