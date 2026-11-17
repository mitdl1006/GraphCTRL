//! Sketch edits behind the Inventor-style tools (docs/ux/inventor-sketch-parity.md, P1): tangent arcs,
//! three-point arcs and rectangles, construction geometry, driven dimensions. Scenario tests in the style
//! of plan appendix C: build, check the geometry and the remaining degrees of freedom.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "test code: checks results with std math"
)]

use gc_core::Id;
use gc_doc::{Command, Created, Document, EditError, EndSnap, History, SketchOp, apply};
use gc_sketch::{Curve, CurveGeom, DimKind, GeoConstraint, Sketch};

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

    fn curves(created: &[Created]) -> Vec<Id<Curve>> {
        created.iter().filter_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).collect()
    }

    fn xy(&self, point: Id<gc_sketch::SkPoint>) -> [f64; 2] {
        let p = self.sketch().pos(point).unwrap();
        [p[0], p[1]]
    }

    fn dof(&self) -> usize {
        self.sketch().clone().solve().unwrap().dof
    }
}

fn line(bench: &mut Bench, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let created = bench
        .op(SketchOp::AddLine {
            a: [a[0], a[1], 0.0],
            b: [b[0], b[1], 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        })
        .unwrap();
    Bench::curves(&created)[0]
}

#[test]
fn a_tangent_arc_continues_a_line_smoothly_on_either_side() {
    for (end, left) in [([2.0, 1.0], true), ([2.0, -1.0], false)] {
        let mut bench = Bench::new();
        let first = line(&mut bench, [0.0, 0.0], [1.0, 0.0]);
        let (_, joint) = bench.sketch().line(first).unwrap();
        let created = bench
            .op(SketchOp::AddTangentArc {
                from: joint,
                curve: first,
                end: [end[0], end[1], 0.0],
                snap_end: EndSnap::default(),
            })
            .unwrap();
        let arc = Bench::curves(&created)[0];
        let CurveGeom::Arc { c, s, e } = bench.sketch().curves[&arc].geom else { panic!() };
        // The arc starts where the line ends and leaves in the line's direction (+X): its centre is
        // straight above or below the joint.
        let (centre, start_or_end) = (bench.xy(c), if left { bench.xy(s) } else { bench.xy(e) });
        assert!((start_or_end[0] - 1.0).abs() < 1e-9 && start_or_end[1].abs() < 1e-9, "{start_or_end:?}");
        assert!((centre[0] - 1.0).abs() < 1e-9, "centre {centre:?}");
        assert_eq!(centre[1] > 0.0, left, "turns {}", if left { "left" } else { "right" });
        // The far end is where it was asked to be.
        let far = if left { bench.xy(e) } else { bench.xy(s) };
        assert!((far[0] - end[0]).hypot(far[1] - end[1]) < 1e-9, "{far:?}");
        assert!(bench.sketch().constraints.values().any(|k| matches!(k, GeoConstraint::Tangent { .. })));
        // Line (4) + arc (5: centre, two ends on the circle) − joint (2) − tangent (1) = 6.
        assert_eq!(bench.dof(), 6);
    }
}

#[test]
fn a_three_point_arc_passes_through_its_middle_point() {
    let mut bench = Bench::new();
    for through in [[0.5, 0.4], [0.5, -0.4]] {
        let created = bench
            .op(SketchOp::AddArcThrough {
                a: [0.0, 0.0, 0.0],
                b: [1.0, 0.0, 0.0],
                through: [through[0], through[1], 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
            })
            .unwrap();
        let arc = Bench::curves(&created)[0];
        let CurveGeom::Arc { c, s, e } = bench.sketch().curves[&arc].geom else { panic!() };
        let (centre, start, end) = (bench.xy(c), bench.xy(s), bench.xy(e));
        let r = (start[0] - centre[0]).hypot(start[1] - centre[1]);
        assert!(((through[0] - centre[0]).hypot(through[1] - centre[1]) - r).abs() < 1e-9);
        // The middle point lies on the stored (counter-clockwise) arc.
        let angle = |p: [f64; 2]| (p[1] - centre[1]).atan2(p[0] - centre[0]);
        let ccw = |from: f64, to: f64| (to - from).rem_euclid(std::f64::consts::TAU);
        assert!(ccw(angle(start), angle(through)) < ccw(angle(start), angle(end)));
    }
    // Three points on a line make no arc.
    assert!(
        bench
            .op(SketchOp::AddArcThrough {
                a: [0.0, 0.0, 0.0],
                b: [1.0, 0.0, 0.0],
                through: [0.5, 0.0, 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
            })
            .is_err()
    );
}

#[test]
fn a_three_point_rectangle_is_square_at_any_angle_with_five_freedoms() {
    let mut bench = Bench::new();
    let created = bench
        .op(SketchOp::AddRectangle3 {
            a: [0.0, 0.0, 0.0],
            b: [0.3, 0.4, 0.0],
            c: [0.0, 0.8, 0.0],
            snap_a: EndSnap::default(),
        })
        .unwrap();
    let lines = Bench::curves(&created);
    assert_eq!(lines.len(), 4);
    let direction = |l: Id<Curve>| {
        let (a, b) = bench.sketch().line(l).unwrap();
        let (a, b) = (bench.xy(a), bench.xy(b));
        [b[0] - a[0], b[1] - a[1]]
    };
    let dot = |u: [f64; 2], v: [f64; 2]| u[0] * v[0] + u[1] * v[1];
    for i in 0..4 {
        assert!(
            dot(direction(lines[i]), direction(lines[(i + 1) % 4])).abs() < 1e-9,
            "right angle at corner {i}"
        );
    }
    // First side 0.5 long; the width is the projection of b→c on the side's normal.
    assert!((direction(lines[0])[0].hypot(direction(lines[0])[1]) - 0.5).abs() < 1e-9);
    // Position (2), angle (1), length and width (2).
    assert_eq!(bench.dof(), 5);
    // A third point on the first side's line makes no rectangle.
    assert!(
        bench
            .op(SketchOp::AddRectangle3 {
                a: [0.0, 0.0, 0.0],
                b: [0.3, 0.4, 0.0],
                c: [0.6, 0.8, 0.0],
                snap_a: EndSnap::default()
            })
            .is_err()
    );
}

#[test]
fn geometry_drawn_as_construction_is_construction_and_one_undo_step() {
    let mut bench = Bench::new();
    let depth = bench.history.undo_depth();
    let created = bench
        .op(SketchOp::AsConstruction(Box::new(SketchOp::AddRectangle {
            a: [0.0, 0.0],
            b: [0.2, 0.1],
            snap_a: EndSnap::default(),
        })))
        .unwrap();
    let lines = Bench::curves(&created);
    assert_eq!(lines.len(), 4);
    assert!(lines.iter().all(|l| bench.sketch().curves[l].construction));
    assert_eq!(bench.history.undo_depth(), depth + 1);
}

#[test]
fn a_dimension_can_be_made_driven_and_driving_again_unless_it_is_redundant() {
    let mut bench = Bench::new();
    let l = line(&mut bench, [0.0, 0.0], [0.1, 0.0]);
    let (a, b) = bench.sketch().line(l).unwrap();
    let kind = DimKind::Distance { p: a, q: b };
    let created =
        bench.op(SketchOp::AddDimension { kind, source: "100 mm".into(), text_pos: [0.0; 2] }).unwrap();
    let Created::Dimension(dim) = created[0] else { panic!() };
    let constrained = bench.dof();
    bench.op(SketchOp::SetDriving { id: dim, driving: false }).unwrap();
    assert!(!bench.sketch().dimensions[&dim].driving);
    assert_eq!(bench.dof(), constrained + 1, "a driven dimension constrains nothing");
    bench.op(SketchOp::SetDriving { id: dim, driving: true }).unwrap();
    assert!(bench.sketch().dimensions[&dim].driving);
    assert_eq!(bench.sketch().dimensions[&dim].source, "100 mm");

    // Both ends fixed: the length is set already, so the dimension can only be driven.
    bench.op(SketchOp::SetDriving { id: dim, driving: false }).unwrap();
    bench.op(SketchOp::AddConstraint(GeoConstraint::Fix(a))).unwrap();
    bench.op(SketchOp::AddConstraint(GeoConstraint::Fix(b))).unwrap();
    assert!(matches!(
        bench.op(SketchOp::SetDriving { id: dim, driving: true }),
        Err(EditError::Redundant(_))
    ));
    assert!(!bench.sketch().dimensions[&dim].driving, "unchanged");
}
