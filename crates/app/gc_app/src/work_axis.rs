//! The Work Axis command (Inventor's 3D Model tab, Work Features panel, Axis): choose the way the axis is
//! defined and what it is built on, then create it. The types are those of an assembly: On Line, Parallel to
//! Line Through Point, Through Two Points, Intersection of Two Planes, Through Center of Circular Shape.
//! Normal to Plane Through Point is for parts only and Revolved Face needs model faces we do not have.
//! Reference: Inventor 2026 help, To Create Work Features (GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385).

use gc_doc::{CellOp, Command, CurveRef, OriginPlane, PlaneRef, PointRef, WorkFeature, WorkGeometry};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// How the axis is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    OnLine,
    ParallelThrough,
    TwoPoints,
    TwoPlanes,
    CircleCentre,
}

/// The dialog's state: the chosen entries are indices into the lists built each frame.
pub(crate) struct WorkAxis {
    pub(crate) open: bool,
    kind: Kind,
    first: usize,
    second: usize,
}

impl Default for WorkAxis {
    fn default() -> Self {
        Self { open: false, kind: Kind::OnLine, first: 0, second: 0 }
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

/// The points to build on: the origin and every sketch point.
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
    found
}

/// The origin planes.
fn planes() -> Vec<(PlaneRef, String)> {
    [(OriginPlane::Xy, "XY"), (OriginPlane::Xz, "XZ"), (OriginPlane::Yz, "YZ")]
        .into_iter()
        .map(|(plane, name)| (PlaneRef::Origin(plane), name.to_owned()))
        .collect()
}

impl WorkAxis {
    /// Opens the dialog.
    pub(crate) fn start(&mut self) {
        *self = Self { open: true, ..Self::default() };
    }

    /// The definition the choices make, if they are complete.
    fn geometry(&self, session: &Session) -> Option<WorkGeometry> {
        let pick = |list_len: usize, index: usize| (index < list_len).then_some(index);
        match self.kind {
            Kind::OnLine => {
                let lines = curves(session, false);
                let line = lines.get(pick(lines.len(), self.first)?)?.0;
                Some(WorkGeometry::AxisOnLine { line })
            }
            Kind::ParallelThrough => {
                let lines = curves(session, false);
                let all_points = points(session);
                let line = lines.get(pick(lines.len(), self.first)?)?.0;
                let through = all_points.get(pick(all_points.len(), self.second)?)?.0;
                Some(WorkGeometry::AxisParallelThrough { line, through })
            }
            Kind::TwoPoints => {
                let all_points = points(session);
                let a = all_points.get(pick(all_points.len(), self.first)?)?.0;
                let b = all_points.get(pick(all_points.len(), self.second)?)?.0;
                Some(WorkGeometry::AxisThroughPoints { points: [a, b] })
            }
            Kind::TwoPlanes => {
                let all = planes();
                let a = all.get(self.first)?.0;
                let b = all.get(self.second)?.0;
                Some(WorkGeometry::AxisPlanesIntersection { planes: [a, b] })
            }
            Kind::CircleCentre => {
                let rounds = curves(session, true);
                let curve = rounds.get(pick(rounds.len(), self.first)?)?.0;
                Some(WorkGeometry::AxisThroughCircle { curve })
            }
        }
    }

    /// The dialog window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut create = false;
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 340.0, screen.top() + 200.0);
        egui::Window::new(tr!(lang, WorkAxisTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(320.0)
            .show(ctx, |ui| {
                for (kind, name) in [
                    (Kind::OnLine, tr!(lang, WaOnLine)),
                    (Kind::ParallelThrough, tr!(lang, WaParallel)),
                    (Kind::TwoPoints, tr!(lang, WaTwoPoints)),
                    (Kind::TwoPlanes, tr!(lang, WaTwoPlanes)),
                    (Kind::CircleCentre, tr!(lang, WaCircle)),
                ] {
                    if ui.selectable_label(self.kind == kind, name).clicked() {
                        self.kind = kind;
                        self.first = 0;
                        self.second = 0;
                    }
                }
                ui.separator();
                match self.kind {
                    Kind::OnLine => {
                        let names: Vec<String> = curves(session, false).into_iter().map(|c| c.1).collect();
                        choose(ui, tr!(lang, WaLine), &names, &mut self.first);
                    }
                    Kind::ParallelThrough => {
                        let lines: Vec<String> = curves(session, false).into_iter().map(|c| c.1).collect();
                        choose(ui, tr!(lang, WaLine), &lines, &mut self.first);
                        let all: Vec<String> = points(session).into_iter().map(|p| p.1).collect();
                        choose(ui, tr!(lang, WaPoint), &all, &mut self.second);
                    }
                    Kind::TwoPoints => {
                        let all: Vec<String> = points(session).into_iter().map(|p| p.1).collect();
                        choose(ui, tr!(lang, WaFirstPoint), &all, &mut self.first);
                        choose(ui, tr!(lang, WaSecondPoint), &all, &mut self.second);
                    }
                    Kind::TwoPlanes => {
                        let all: Vec<String> = planes().into_iter().map(|p| p.1).collect();
                        choose(ui, tr!(lang, WaFirstPlane), &all, &mut self.first);
                        choose(ui, tr!(lang, WaSecondPlane), &all, &mut self.second);
                    }
                    Kind::CircleCentre => {
                        let names: Vec<String> = curves(session, true).into_iter().map(|c| c.1).collect();
                        choose(ui, tr!(lang, WaCircleOf), &names, &mut self.first);
                    }
                }
                let ready = self.geometry(session).is_some();
                create = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
            });
        if create && let Some(geometry) = self.geometry(session) {
            let name = format!("Axis{}", session.doc().work_features().iter().count() + 1);
            let op = CellOp::AddWorkFeature(WorkFeature { name, geometry, visible: true });
            // A definition that does not work out (parallel planes, a line of no length) is refused and
            // explained in the status bar.
            if session.execute(&Command::Cell(op)).is_ok() {
                open = false;
            }
        }
        self.open = open;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Two origin planes make an axis; the same plane twice is refused and nothing is added.
    #[test]
    fn two_planes_make_an_axis_and_the_same_plane_twice_is_refused() {
        let mut session = Session::new();
        let mut dialog = WorkAxis::default();
        dialog.start();
        dialog.kind = Kind::TwoPlanes;
        dialog.first = 0;
        dialog.second = 1;
        let geometry = dialog.geometry(&session).expect("two planes");
        let op = CellOp::AddWorkFeature(WorkFeature { name: "Axis1".into(), geometry, visible: true });
        session.execute(&Command::Cell(op)).unwrap();
        assert_eq!(session.doc().work_features().iter().count(), 1);

        dialog.second = 0;
        let same = dialog.geometry(&session).expect("it is a definition");
        let op = CellOp::AddWorkFeature(WorkFeature { name: "Axis2".into(), geometry: same, visible: true });
        assert!(session.execute(&Command::Cell(op)).is_err());
        assert_eq!(session.doc().work_features().iter().count(), 1);
    }
}
