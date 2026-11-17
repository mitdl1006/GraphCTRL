//! Sketch commands of the ribbon's "planned" group (docs/ux/inventor-sketch-parity.md §2.2–2.5): Point,
//! Polygon, Slot, Mirror and patterns, Offset. Each checks the geometry and the constraints Inventor's help
//! describes, against the original pages (cited per test).

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: checks results with std math"
)]

use gc_core::Id;
use gc_doc::{Command, Created, Document, EditError, History, SketchOp, apply};
use gc_sketch::{Curve, GeoConstraint, SkPoint, Sketch};

struct Bench {
    doc: Document,
    history: History,
    sketch: Id<Sketch>,
}

impl Bench {
    fn new() -> Self {
        let mut doc = Document::new();
        let frame = doc.origin_plane("XY").unwrap();
        let applied =
            apply(&mut doc, &Command::CreateSketch { name: "S".into(), frame, spatial: false }).unwrap();
        let Created::Sketch(sketch) = applied.created[0] else { panic!() };
        Self { doc, history: History::default(), sketch }
    }

    fn op(&mut self, op: SketchOp) -> Result<Vec<Created>, EditError> {
        let applied = apply(&mut self.doc, &Command::Sketch { sketch: self.sketch, op })?;
        self.history.record(applied.label, applied.delta);
        Ok(applied.created)
    }

    fn sketch(&self) -> &Sketch {
        self.doc.get(self.sketch).unwrap()
    }

    fn points(created: &[Created]) -> Vec<Id<SkPoint>> {
        created.iter().filter_map(|c| if let Created::Point(id) = c { Some(*id) } else { None }).collect()
    }

    #[allow(dead_code, reason = "used by later items")]
    fn curves(created: &[Created]) -> Vec<Id<Curve>> {
        created.iter().filter_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).collect()
    }

    #[allow(dead_code, reason = "used by later items")]
    fn constraints(&self, created: &[Created]) -> Vec<GeoConstraint> {
        created
            .iter()
            .filter_map(|c| {
                if let Created::Constraint(id) = c {
                    self.sketch().constraints.get(id).copied()
                } else {
                    None
                }
            })
            .collect()
    }

    fn dof(&self) -> usize {
        self.sketch().clone().solve().unwrap().dof
    }
}

/// Inventor 2026 help, To Work with Points (GUID-D4A4288F-32F1-4E23-A382-930EBA152E87): points are centre
/// points by default (crosses) or sketch points (dots), "created without constraints"; the style is toggled
/// on points (Format panel, or right-click).
#[test]
fn points_are_placed_without_constraints_as_centre_or_sketch_points() {
    let mut bench = Bench::new();
    let dof = bench.dof();
    let created = bench.op(SketchOp::PlacePoint { pos: [0.1, 0.2, 0.0], center: true }).unwrap();
    let [point] = Bench::points(&created)[..] else { panic!("{created:?}") };
    assert_eq!(created.len(), 1, "no constraints");
    assert!(bench.sketch().points[&point].center);
    assert_eq!(bench.dof(), dof + 2, "free in the plane");
    let created = bench.op(SketchOp::PlacePoint { pos: [0.3, 0.2, 0.0], center: false }).unwrap();
    let plain = Bench::points(&created)[0];
    assert!(!bench.sketch().points[&plain].center);

    // The toggle on both; one undo step.
    let depth = bench.history.undo_depth();
    bench.op(SketchOp::SetCenterPoint { points: vec![point, plain], center: false }).unwrap();
    assert!(!bench.sketch().points[&point].center);
    assert_eq!(bench.history.undo_depth(), depth + 1);
    bench.history.undo(&mut bench.doc).unwrap();
    assert!(bench.sketch().points[&point].center);

    // A line's end is not a standalone point; a point that is not a number is refused.
    let line = bench
        .op(SketchOp::AddLine {
            a: [0.0; 3],
            b: [0.1, 0.0, 0.0],
            snap_a: Default::default(),
            snap_b: Default::default(),
            horizontal: None,
        })
        .unwrap();
    let end = bench.sketch().line(Bench::curves(&line)[0]).unwrap().0;
    assert!(bench.op(SketchOp::SetCenterPoint { points: vec![end], center: true }).is_err());
    assert!(bench.op(SketchOp::PlacePoint { pos: [f64::NAN, 0.0, 0.0], center: true }).is_err());
}
