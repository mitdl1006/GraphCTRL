//! The Work Plane, Work Axis and Work Point commands (Inventor's 3D Model tab, Work Features panel): choose
//! the way the feature is defined and what it is built on, then create it; the same dialog edits an existing
//! one. The types are those the document knows; in an assembly Inventor offers a subset (parts only:
//! Normal to Plane Through Point), see docs/handoff/frontend.md.
//! Reference: Inventor 2026 help, To Create Work Features (GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385).

use gc_core::Id;
use gc_doc::{
    CellOp, Command, CurveRef, LineRef, OriginAxis, OriginPlane, PlaneRef, PointRef, WorkFeature,
    WorkGeometry,
};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// What kind of feature the dialog makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Category {
    Plane,
    Axis,
    Point,
}

/// How the feature is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    PlaneOffset,
    PlaneThreePoints,
    AxisOnLine,
    AxisParallelThrough,
    AxisTwoPoints,
    AxisTwoPlanes,
    AxisNormalToPlane,
    AxisCircleCentre,
    PlaneParallelThrough,
    PlaneMidplane,
    PlaneAngle,
    PlaneLinePoint,
    PlaneNormalLine,
    PlaneNormalCurve,
    PointAt,
    PointLines,
    PointPlaneLine,
    PointThreePlanes,
    PointCurveCentre,
}

impl Kind {
    fn of(category: Category) -> &'static [Kind] {
        match category {
            Category::Plane => &[
                Kind::PlaneOffset,
                Kind::PlaneThreePoints,
                Kind::PlaneParallelThrough,
                Kind::PlaneMidplane,
                Kind::PlaneAngle,
                Kind::PlaneLinePoint,
                Kind::PlaneNormalLine,
                Kind::PlaneNormalCurve,
            ],
            Category::Axis => &[
                Kind::AxisOnLine,
                Kind::AxisParallelThrough,
                Kind::AxisTwoPoints,
                Kind::AxisTwoPlanes,
                Kind::AxisNormalToPlane,
                Kind::AxisCircleCentre,
            ],
            Category::Point => &[
                Kind::PointAt,
                Kind::PointLines,
                Kind::PointPlaneLine,
                Kind::PointThreePlanes,
                Kind::PointCurveCentre,
            ],
        }
    }
}

/// The dialog's state: the chosen entries are indices into the lists built each frame.
pub(crate) struct WorkFeatureDialog {
    pub(crate) open: bool,
    category: Category,
    /// The feature being edited; `None` makes a new one.
    editing: Option<Id<WorkFeature>>,
    kind: Kind,
    first: usize,
    second: usize,
    third: usize,
    /// Millimetres, of an offset plane.
    distance: String,
    /// Degrees, of a plane turned about a line.
    angle: String,
    /// A point given by coordinates (millimetres) instead of picked.
    typed: bool,
    coordinates: [String; 3],
}

impl Default for WorkFeatureDialog {
    fn default() -> Self {
        Self {
            open: false,
            category: Category::Axis,
            editing: None,
            kind: Kind::AxisOnLine,
            first: 0,
            second: 0,
            third: 0,
            distance: "10".to_owned(),
            angle: "45".to_owned(),
            typed: false,
            coordinates: ["0".to_owned(), "0".to_owned(), "0".to_owned()],
        }
    }
}

/// A sketch curve to build on, with its name.
fn curves(session: &Session, round: bool) -> Vec<(CurveRef, String)> {
    let mut found = Vec::new();
    for (sketch_id, sketch) in session.doc().sketches().iter() {
        for curve in sketch.curves.keys() {
            let fits = if round { sketch.round(*curve).is_some() } else { sketch.line(*curve).is_some() };
            if fits {
                let word = if round { "circle" } else { "line" };
                found.push((
                    CurveRef { sketch: sketch_id, curve: *curve },
                    format!("{} · {word} {}", sketch.name, found.len() + 1),
                ));
            }
        }
    }
    found
}

/// Every curve of the sketches that has a centre (a circle, an arc or an ellipse), or any curve that a plane
/// can stand square to (those and splines).
fn curves_where(session: &Session, keep: impl Fn(&gc_sketch::CurveGeom) -> bool) -> Vec<(CurveRef, String)> {
    let mut found = Vec::new();
    for (sketch_id, sketch) in session.doc().sketches().iter() {
        for (curve, data) in &sketch.curves {
            if keep(&data.geom) {
                found.push((
                    CurveRef { sketch: sketch_id, curve: *curve },
                    format!("{} · {}", sketch.name, found.len() + 1),
                ));
            }
        }
    }
    found
}

/// The curves a work axis or point can be built on the centre of: circles, arcs and ellipses.
fn centred(session: &Session) -> Vec<(CurveRef, String)> {
    curves_where(session, |geom| {
        matches!(
            geom,
            gc_sketch::CurveGeom::Circle { .. }
                | gc_sketch::CurveGeom::Arc { .. }
                | gc_sketch::CurveGeom::Ellipse { .. }
        )
    })
}

/// The curves a plane can be put square to: arcs, circles, ellipses and splines.
fn squarable(session: &Session) -> Vec<(CurveRef, String)> {
    curves_where(session, |geom| !matches!(geom, gc_sketch::CurveGeom::Line { .. }))
}

/// The lines to build on: sketch lines, work axes, the origin axes of the assembly and of each component.
fn lines(session: &Session) -> Vec<(LineRef, String)> {
    let mut found: Vec<(LineRef, String)> =
        curves(session, false).into_iter().map(|(curve, name)| (LineRef::Sketch(curve), name)).collect();
    for (id, feature) in session.doc().work_features().iter() {
        if feature.geometry.is_axis() {
            found.push((LineRef::Work(id), feature.name.clone()));
        }
    }
    for (axis, name) in [(OriginAxis::X, "X"), (OriginAxis::Y, "Y"), (OriginAxis::Z, "Z")] {
        found.push((LineRef::Origin(axis), format!("{name} axis")));
    }
    for node in session.components() {
        if matches!(node.occurrence, gc_doc::Occurrence::Item(_)) {
            for (axis, name) in [(OriginAxis::X, "X"), (OriginAxis::Y, "Y"), (OriginAxis::Z, "Z")] {
                found.push((
                    LineRef::Component(node.occurrence, axis),
                    format!("{}:1 {name} axis", node.name),
                ));
            }
        }
    }
    found
}

/// The points to build on: the origin, every sketch point and the work points.
fn points(session: &Session) -> Vec<(PointRef, String)> {
    let mut found = vec![(PointRef::Origin, "Origin".to_owned())];
    for (sketch_id, sketch) in session.doc().sketches().iter() {
        for (point, data) in &sketch.points {
            let at = data.pos.map(|v| v * 1000.0);
            found.push((
                PointRef::SketchPoint { sketch: sketch_id, point: *point },
                format!("{} · ({:.0}, {:.0}, {:.0}) mm", sketch.name, at[0], at[1], at[2]),
            ));
        }
    }
    for (id, feature) in session.doc().work_features().iter() {
        if feature.geometry.is_point() {
            found.push((PointRef::Work(id), feature.name.clone()));
        }
    }
    found
}

/// The planes to build on: the origin planes and the work planes.
fn planes(session: &Session) -> Vec<(PlaneRef, String)> {
    let mut found: Vec<(PlaneRef, String)> =
        [(OriginPlane::Xy, "XY"), (OriginPlane::Xz, "XZ"), (OriginPlane::Yz, "YZ")]
            .into_iter()
            .map(|(plane, name)| (PlaneRef::Origin(plane), name.to_owned()))
            .collect();
    for (id, feature) in session.doc().work_features().iter() {
        if feature.geometry.is_plane() {
            found.push((PlaneRef::Work(id), feature.name.clone()));
        }
    }
    found
}

fn index_of<T: PartialEq + Copy>(list: &[(T, String)], wanted: T) -> usize {
    list.iter().position(|(item, _)| *item == wanted).unwrap_or(0)
}

impl WorkFeatureDialog {
    /// Opens the dialog to make a feature of a category.
    pub(crate) fn start(&mut self, category: Category) {
        let kind = Kind::of(category)[0];
        *self = Self { open: true, category, kind, ..Self::default() };
    }

    /// Opens the dialog on an existing feature, to change it.
    pub(crate) fn edit(&mut self, session: &Session, id: Id<WorkFeature>) {
        let Some(feature) = session.doc().work_features().get(id) else { return };
        let mut dialog = Self { open: true, editing: Some(id), ..Self::default() };
        let all_points = points(session);
        let all_planes = planes(session);
        match feature.geometry {
            WorkGeometry::PlaneOffset { base, distance } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneOffset;
                dialog.first = index_of(&all_planes, base);
                dialog.distance = format!("{}", distance * 1000.0);
            }
            WorkGeometry::PlaneThroughPoints { points: p } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneThreePoints;
                dialog.first = index_of(&all_points, p[0]);
                dialog.second = index_of(&all_points, p[1]);
                dialog.third = index_of(&all_points, p[2]);
            }
            WorkGeometry::AxisThroughPoints { points: p } => {
                dialog.kind = Kind::AxisTwoPoints;
                dialog.first = index_of(&all_points, p[0]);
                dialog.second = index_of(&all_points, p[1]);
            }
            WorkGeometry::AxisNormalToPlane { plane, through } => {
                dialog.kind = Kind::AxisNormalToPlane;
                dialog.first = index_of(&all_planes, plane);
                dialog.second = index_of(&all_points, through);
            }
            WorkGeometry::AxisOnLine { line } => {
                dialog.kind = Kind::AxisOnLine;
                dialog.first = index_of(&curves(session, false), line);
            }
            WorkGeometry::AxisParallelThrough { line, through } => {
                dialog.kind = Kind::AxisParallelThrough;
                dialog.first = index_of(&curves(session, false), line);
                dialog.second = index_of(&all_points, through);
            }
            WorkGeometry::AxisPlanesIntersection { planes: p } => {
                dialog.kind = Kind::AxisTwoPlanes;
                dialog.first = index_of(&all_planes, p[0]);
                dialog.second = index_of(&all_planes, p[1]);
            }
            WorkGeometry::AxisThroughCircle { curve } => {
                dialog.kind = Kind::AxisCircleCentre;
                dialog.first = index_of(&centred(session), curve);
            }
            WorkGeometry::Point { at } => {
                dialog.category = Category::Point;
                dialog.kind = Kind::PointAt;
                match at {
                    PointRef::Fixed(xyz) => {
                        dialog.typed = true;
                        dialog.coordinates = xyz.map(|v| format!("{}", v * 1000.0));
                    }
                    other => dialog.first = index_of(&all_points, other),
                }
            }
            WorkGeometry::PlaneParallelThrough { base, through } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneParallelThrough;
                dialog.first = index_of(&all_planes, base);
                dialog.second = index_of(&all_points, through);
            }
            WorkGeometry::PlaneMidplane { planes: p } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneMidplane;
                dialog.first = index_of(&all_planes, p[0]);
                dialog.second = index_of(&all_planes, p[1]);
            }
            WorkGeometry::PlaneAngleToPlane { base, line, angle } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneAngle;
                dialog.first = index_of(&all_planes, base);
                dialog.second = index_of(&lines(session), line);
                dialog.angle = format!("{}", angle.to_degrees());
            }
            WorkGeometry::PlaneLineAndPoint { line, through } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneLinePoint;
                dialog.first = index_of(&lines(session), line);
                dialog.second = index_of(&all_points, through);
            }
            WorkGeometry::PlaneNormalToLine { line, through } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneNormalLine;
                dialog.first = index_of(&lines(session), line);
                dialog.second = index_of(&all_points, through);
            }
            WorkGeometry::PlaneNormalToCurve { curve, at } => {
                dialog.category = Category::Plane;
                dialog.kind = Kind::PlaneNormalCurve;
                dialog.first = index_of(&squarable(session), curve);
                dialog.second = index_of(&all_points, at);
            }
            WorkGeometry::PointLinesIntersection { lines: l } => {
                dialog.category = Category::Point;
                dialog.kind = Kind::PointLines;
                let all_lines = lines(session);
                dialog.first = index_of(&all_lines, l[0]);
                dialog.second = index_of(&all_lines, l[1]);
            }
            WorkGeometry::PointPlaneLine { plane, line } => {
                dialog.category = Category::Point;
                dialog.kind = Kind::PointPlaneLine;
                dialog.first = index_of(&all_planes, plane);
                dialog.second = index_of(&lines(session), line);
            }
            WorkGeometry::PointThreePlanes { planes: p } => {
                dialog.category = Category::Point;
                dialog.kind = Kind::PointThreePlanes;
                dialog.first = index_of(&all_planes, p[0]);
                dialog.second = index_of(&all_planes, p[1]);
                dialog.third = index_of(&all_planes, p[2]);
            }
            WorkGeometry::PointCurveCentre { curve } => {
                dialog.category = Category::Point;
                dialog.kind = Kind::PointCurveCentre;
                dialog.first = index_of(&centred(session), curve);
            }
        }
        *self = dialog;
    }

    /// The definition the choices make, if they are complete.
    fn geometry(&self, session: &Session) -> Option<WorkGeometry> {
        let all_points = points(session);
        let all_planes = planes(session);
        let point = |index: usize| all_points.get(index).map(|p| p.0);
        let plane = |index: usize| all_planes.get(index).map(|p| p.0);
        match self.kind {
            Kind::PlaneOffset => {
                let distance = self.distance.trim().parse::<f64>().ok()? / 1000.0;
                Some(WorkGeometry::PlaneOffset { base: plane(self.first)?, distance })
            }
            Kind::PlaneThreePoints => Some(WorkGeometry::PlaneThroughPoints {
                points: [point(self.first)?, point(self.second)?, point(self.third)?],
            }),
            Kind::AxisOnLine => {
                let line = curves(session, false).get(self.first)?.0;
                Some(WorkGeometry::AxisOnLine { line })
            }
            Kind::AxisParallelThrough => {
                let line = curves(session, false).get(self.first)?.0;
                Some(WorkGeometry::AxisParallelThrough { line, through: point(self.second)? })
            }
            Kind::AxisTwoPoints => {
                Some(WorkGeometry::AxisThroughPoints { points: [point(self.first)?, point(self.second)?] })
            }
            Kind::AxisTwoPlanes => Some(WorkGeometry::AxisPlanesIntersection {
                planes: [plane(self.first)?, plane(self.second)?],
            }),
            Kind::AxisNormalToPlane => Some(WorkGeometry::AxisNormalToPlane {
                plane: plane(self.first)?,
                through: point(self.second)?,
            }),
            Kind::AxisCircleCentre => {
                let curve = centred(session).get(self.first)?.0;
                Some(WorkGeometry::AxisThroughCircle { curve })
            }
            Kind::PlaneParallelThrough => Some(WorkGeometry::PlaneParallelThrough {
                base: plane(self.first)?,
                through: point(self.second)?,
            }),
            Kind::PlaneMidplane => {
                Some(WorkGeometry::PlaneMidplane { planes: [plane(self.first)?, plane(self.second)?] })
            }
            Kind::PlaneAngle => {
                let angle = self.angle.trim().parse::<f64>().ok()?.to_radians();
                let line = lines(session).get(self.second)?.0;
                Some(WorkGeometry::PlaneAngleToPlane { base: plane(self.first)?, line, angle })
            }
            Kind::PlaneLinePoint => {
                let line = lines(session).get(self.first)?.0;
                Some(WorkGeometry::PlaneLineAndPoint { line, through: point(self.second)? })
            }
            Kind::PlaneNormalLine => {
                let line = lines(session).get(self.first)?.0;
                Some(WorkGeometry::PlaneNormalToLine { line, through: point(self.second)? })
            }
            Kind::PlaneNormalCurve => {
                let curve = squarable(session).get(self.first)?.0;
                Some(WorkGeometry::PlaneNormalToCurve { curve, at: point(self.second)? })
            }
            Kind::PointLines => {
                let all = lines(session);
                Some(WorkGeometry::PointLinesIntersection {
                    lines: [all.get(self.first)?.0, all.get(self.second)?.0],
                })
            }
            Kind::PointPlaneLine => {
                let line = lines(session).get(self.second)?.0;
                Some(WorkGeometry::PointPlaneLine { plane: plane(self.first)?, line })
            }
            Kind::PointThreePlanes => Some(WorkGeometry::PointThreePlanes {
                planes: [plane(self.first)?, plane(self.second)?, plane(self.third)?],
            }),
            Kind::PointCurveCentre => {
                let curve = centred(session).get(self.first)?.0;
                Some(WorkGeometry::PointCurveCentre { curve })
            }
            Kind::PointAt => {
                let at = if self.typed {
                    let mut xyz = [0.0; 3];
                    for (value, text) in xyz.iter_mut().zip(&self.coordinates) {
                        *value = text.trim().parse::<f64>().ok()? / 1000.0;
                    }
                    PointRef::Fixed(xyz)
                } else {
                    point(self.first)?
                };
                Some(WorkGeometry::Point { at })
            }
        }
    }

    /// The dialog window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut create = false;
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 340.0, screen.top() + 200.0);
        let title = match self.category {
            Category::Plane => tr!(lang, WorkPlaneTitle),
            Category::Axis => tr!(lang, WorkAxisTitle),
            Category::Point => tr!(lang, WorkPointTitle),
        };
        egui::Window::new(title).open(&mut open).default_pos(start).default_width(320.0).show(ctx, |ui| {
            if self.editing.is_none() {
                for kind in Kind::of(self.category) {
                    if ui.selectable_label(self.kind == *kind, kind_name(*kind, lang)).clicked() {
                        self.kind = *kind;
                        self.first = 0;
                        self.second = 0;
                        self.third = 0;
                    }
                }
                ui.separator();
            }
            let names = |list: Vec<String>| list;
            let point_names = names(points(session).into_iter().map(|p| p.1).collect());
            let plane_names = names(planes(session).into_iter().map(|p| p.1).collect());
            let line_names = names(curves(session, false).into_iter().map(|c| c.1).collect());
            let all_line_names = names(lines(session).into_iter().map(|c| c.1).collect());
            let square_names = names(squarable(session).into_iter().map(|c| c.1).collect());
            let centre_names = names(centred(session).into_iter().map(|c| c.1).collect());
            match self.kind {
                Kind::PlaneOffset => {
                    choose(ui, tr!(lang, WaBasePlane), &plane_names, &mut self.first);
                    labelled(ui, tr!(lang, WaDistance), &mut self.distance);
                }
                Kind::PlaneThreePoints => {
                    choose(ui, tr!(lang, WaFirstPoint), &point_names, &mut self.first);
                    choose(ui, tr!(lang, WaSecondPoint), &point_names, &mut self.second);
                    choose(ui, tr!(lang, WaThirdPoint), &point_names, &mut self.third);
                }
                Kind::AxisOnLine => choose(ui, tr!(lang, WaLine), &line_names, &mut self.first),
                Kind::AxisParallelThrough => {
                    choose(ui, tr!(lang, WaLine), &line_names, &mut self.first);
                    choose(ui, tr!(lang, WaPoint), &point_names, &mut self.second);
                }
                Kind::AxisTwoPoints => {
                    choose(ui, tr!(lang, WaFirstPoint), &point_names, &mut self.first);
                    choose(ui, tr!(lang, WaSecondPoint), &point_names, &mut self.second);
                }
                Kind::AxisTwoPlanes => {
                    choose(ui, tr!(lang, WaFirstPlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaSecondPlane), &plane_names, &mut self.second);
                }
                Kind::AxisNormalToPlane => {
                    choose(ui, tr!(lang, WaBasePlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaPoint), &point_names, &mut self.second);
                }
                Kind::AxisCircleCentre => choose(ui, tr!(lang, WaCircleOf), &centre_names, &mut self.first),
                Kind::PlaneParallelThrough => {
                    choose(ui, tr!(lang, WaBasePlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaPoint), &point_names, &mut self.second);
                }
                Kind::PlaneMidplane => {
                    choose(ui, tr!(lang, WaFirstPlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaSecondPlane), &plane_names, &mut self.second);
                }
                Kind::PlaneAngle => {
                    choose(ui, tr!(lang, WaBasePlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaLine), &all_line_names, &mut self.second);
                    labelled(ui, tr!(lang, WaAngleDeg), &mut self.angle);
                }
                Kind::PlaneLinePoint | Kind::PlaneNormalLine => {
                    choose(ui, tr!(lang, WaLine), &all_line_names, &mut self.first);
                    choose(ui, tr!(lang, WaPoint), &point_names, &mut self.second);
                }
                Kind::PlaneNormalCurve => {
                    choose(ui, tr!(lang, WaCurve), &square_names, &mut self.first);
                    choose(ui, tr!(lang, WaPoint), &point_names, &mut self.second);
                }
                Kind::PointLines => {
                    choose(ui, tr!(lang, WaFirstLine), &all_line_names, &mut self.first);
                    choose(ui, tr!(lang, WaSecondLine), &all_line_names, &mut self.second);
                }
                Kind::PointPlaneLine => {
                    choose(ui, tr!(lang, WaBasePlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaLine), &all_line_names, &mut self.second);
                }
                Kind::PointThreePlanes => {
                    choose(ui, tr!(lang, WaFirstPlane), &plane_names, &mut self.first);
                    choose(ui, tr!(lang, WaSecondPlane), &plane_names, &mut self.second);
                    choose(ui, tr!(lang, WaThirdPlane), &plane_names, &mut self.third);
                }
                Kind::PointCurveCentre => choose(ui, tr!(lang, WaCurve), &centre_names, &mut self.first),
                Kind::PointAt => {
                    ui.checkbox(&mut self.typed, tr!(lang, WaTypedPoint));
                    if self.typed {
                        for (text, axis) in self.coordinates.iter_mut().zip(["X", "Y", "Z"]) {
                            labelled(ui, &format!("{axis} (mm)"), text);
                        }
                    } else {
                        choose(ui, tr!(lang, WaPoint), &point_names, &mut self.first);
                    }
                }
            }
            let ready = self.geometry(session).is_some();
            create = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
        });
        if create && let Some(geometry) = self.geometry(session) {
            let op = match self.editing {
                Some(id) => session.doc().work_features().get(id).map(|old| CellOp::SetWorkFeature {
                    id,
                    feature: WorkFeature { geometry, ..old.clone() },
                }),
                None => {
                    let word = match self.category {
                        Category::Plane => "Plane",
                        Category::Axis => "Axis",
                        Category::Point => "Point",
                    };
                    let count = session
                        .doc()
                        .work_features()
                        .iter()
                        .filter(|(_, f)| match self.category {
                            Category::Plane => f.geometry.is_plane(),
                            Category::Point => f.geometry.is_point(),
                            Category::Axis => !f.geometry.is_plane() && !f.geometry.is_point(),
                        })
                        .count();
                    Some(CellOp::AddWorkFeature(WorkFeature {
                        name: format!("{word}{}", count + 1),
                        geometry,
                        visible: true,
                    }))
                }
            };
            // A definition that does not work out (parallel planes, a line of no length) is refused and
            // explained in the status bar.
            if let Some(op) = op
                && session.execute(&Command::Cell(op)).is_ok()
            {
                open = false;
            }
        }
        self.open = open;
    }
}

fn kind_name(kind: Kind, lang: Lang) -> &'static str {
    match kind {
        Kind::PlaneOffset => tr!(lang, WpOffset),
        Kind::PlaneThreePoints => tr!(lang, WpThreePoints),
        Kind::AxisOnLine => tr!(lang, WaOnLine),
        Kind::AxisParallelThrough => tr!(lang, WaParallel),
        Kind::AxisTwoPoints => tr!(lang, WaTwoPoints),
        Kind::AxisTwoPlanes => tr!(lang, WaTwoPlanes),
        Kind::AxisNormalToPlane => tr!(lang, WaNormal),
        Kind::AxisCircleCentre => tr!(lang, WaCircle),
        Kind::PointAt => tr!(lang, WorkPointTitle),
        Kind::PlaneParallelThrough => tr!(lang, WpParallelThrough),
        Kind::PlaneMidplane => tr!(lang, WpMidplane),
        Kind::PlaneAngle => tr!(lang, WpAngle),
        Kind::PlaneLinePoint => tr!(lang, WpLinePoint),
        Kind::PlaneNormalLine => tr!(lang, WpNormalLine),
        Kind::PlaneNormalCurve => tr!(lang, WpNormalCurve),
        Kind::PointLines => tr!(lang, WpPointLines),
        Kind::PointPlaneLine => tr!(lang, WpPointPlaneLine),
        Kind::PointThreePlanes => tr!(lang, WpThreePlanes),
        Kind::PointCurveCentre => tr!(lang, WpCurveCentre),
    }
}

/// A labelled list to choose from.
fn choose(ui: &mut egui::Ui, label: &str, names: &[String], index: &mut usize) {
    let current = names.get(*index).map_or("-", String::as_str);
    // The label belongs to the list: it is its accessible name.
    egui::ComboBox::from_label(label).selected_text(current).show_ui(ui, |ui| {
        for (i, name) in names.iter().enumerate() {
            ui.selectable_value(index, i, name);
        }
    });
}

/// A labelled text field.
fn labelled(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(text).desired_width(80.0))
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(session: &mut Session, name: &str, geometry: WorkGeometry) -> Result<(), gc_doc::EditError> {
        let op = CellOp::AddWorkFeature(WorkFeature { name: name.into(), geometry, visible: true });
        session.execute(&Command::Cell(op)).map(|_| ())
    }

    /// Two origin planes make an axis; the same plane twice is refused and nothing is added.
    #[test]
    fn two_planes_make_an_axis_and_the_same_plane_twice_is_refused() {
        let mut session = Session::new();
        let mut dialog = WorkFeatureDialog::default();
        dialog.start(Category::Axis);
        dialog.kind = Kind::AxisTwoPlanes;
        dialog.first = 0;
        dialog.second = 1;
        let geometry = dialog.geometry(&session).expect("two planes");
        add(&mut session, "Axis1", geometry).unwrap();
        assert_eq!(session.doc().work_features().iter().count(), 1);

        dialog.second = 0;
        let same = dialog.geometry(&session).expect("it is a definition");
        assert!(add(&mut session, "Axis2", same).is_err());
        assert_eq!(session.doc().work_features().iter().count(), 1);
    }

    /// An offset plane, a typed point, three points making a plane through them, and editing the offset.
    #[test]
    fn planes_and_points_are_made_and_an_offset_plane_is_edited() {
        let mut session = Session::new();
        let mut dialog = WorkFeatureDialog::default();
        dialog.start(Category::Plane);
        dialog.distance = "25".to_owned();
        let geometry = dialog.geometry(&session).expect("offset plane");
        add(&mut session, "Plane1", geometry).unwrap();
        let (id, _) = session.doc().work_features().iter().next().map(|(id, f)| (id, f.clone())).unwrap();

        dialog.start(Category::Point);
        dialog.typed = true;
        dialog.coordinates = ["10".to_owned(), "20".to_owned(), "30".to_owned()];
        let at = dialog.geometry(&session).expect("typed point");
        assert_eq!(at, WorkGeometry::Point { at: PointRef::Fixed([0.01, 0.02, 0.03]) });
        add(&mut session, "Point1", at).unwrap();

        // Editing reads the definition back into the fields.
        let mut again = WorkFeatureDialog::default();
        again.edit(&session, id);
        assert_eq!(again.distance, "25");
        again.distance = "40".to_owned();
        let changed = again.geometry(&session).unwrap();
        let old = session.doc().work_features().get(id).unwrap().clone();
        let op = CellOp::SetWorkFeature { id, feature: WorkFeature { geometry: changed, ..old } };
        session.execute(&Command::Cell(op)).unwrap();
        let moved = session.work_shape(id).unwrap();
        assert!(matches!(moved, gc_doc::WorkShape::Plane(pose) if (pose.translation.z - 0.04).abs() < 1e-9));
    }

    /// The newer kinds: a midplane, a point where two origin axes meet, three planes meeting in a point, a
    /// plane through a line and a point; each is read back by the dialog when it is edited.
    #[test]
    fn midplanes_and_intersection_points_are_made_and_read_back() {
        let mut session = Session::new();
        let mut dialog = WorkFeatureDialog::default();

        // A plane 40 mm above XY, then the midplane between them.
        dialog.start(Category::Plane);
        dialog.distance = "40".to_owned();
        let above = dialog.geometry(&session).unwrap();
        add(&mut session, "Plane1", above).unwrap();
        dialog.kind = Kind::PlaneMidplane;
        dialog.first = 0; // XY
        dialog.second = 3; // Plane1
        let middle = dialog.geometry(&session).expect("midplane");
        assert!(matches!(middle, WorkGeometry::PlaneMidplane { .. }));
        add(&mut session, "Plane2", middle).unwrap();
        let (id, _) = session.doc().work_features().iter().find(|(_, f)| f.name == "Plane2").unwrap();
        assert!(
            matches!(session.work_shape(id), Some(gc_doc::WorkShape::Plane(p)) if (p.translation.z - 0.02).abs() < 1e-9)
        );
        let mut again = WorkFeatureDialog::default();
        again.edit(&session, id);
        assert_eq!(again.kind, Kind::PlaneMidplane);
        assert_eq!((again.first, again.second), (0, 3));

        // Two origin axes meet at the origin; three origin planes meet there too.
        dialog.start(Category::Point);
        dialog.kind = Kind::PointLines;
        dialog.first = 0; // X axis
        dialog.second = 1; // Y axis
        let meet = dialog.geometry(&session).expect("two lines");
        add(&mut session, "Point1", meet).unwrap();
        dialog.kind = Kind::PointThreePlanes;
        dialog.first = 0;
        dialog.second = 1;
        dialog.third = 2;
        let three = dialog.geometry(&session).expect("three planes");
        add(&mut session, "Point2", three).unwrap();
        for name in ["Point1", "Point2"] {
            let (id, _) = session.doc().work_features().iter().find(|(_, f)| f.name == name).unwrap();
            assert!(
                matches!(session.work_shape(id), Some(gc_doc::WorkShape::Point(p)) if p.length() < 1e-9),
                "{name}"
            );
        }

        // A plane through the X axis and a point above the origin.
        dialog.start(Category::Plane);
        dialog.kind = Kind::PlaneLinePoint;
        dialog.first = 0; // X axis
        let origin_index = points(&session).iter().position(|(p, _)| *p == PointRef::Origin).unwrap();
        dialog.second = origin_index;
        // The origin lies on the X axis: no plane results, and the definition is refused by the document.
        let degenerate = dialog.geometry(&session).expect("it is a definition");
        assert!(add(&mut session, "Plane3", degenerate).is_err());
    }
}
