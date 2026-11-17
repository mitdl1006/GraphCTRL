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

fn length(sketch: &Sketch, line: Id<Curve>) -> f64 {
    let (a, b) = sketch.line(line).unwrap();
    let (p, q) = (sketch.pos(a).unwrap(), sketch.pos(b).unwrap());
    (p[0] - q[0]).hypot(p[1] - q[1])
}

/// Inventor 2026 help, To Create 2D Shape Geometry (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357), Polygons:
/// up to 120 sides; Inscribed uses a vertex for size and orientation, Circumscribed the midpoint of an edge;
/// click the centre, then drag.
#[test]
fn polygons_are_regular_with_four_degrees_of_freedom_inscribed_or_circumscribed() {
    for circumscribed in [false, true] {
        for sides in [3u32, 4, 5, 6, 7, 8, 12] {
            let mut bench = Bench::new();
            let dof = bench.dof();
            let created = bench
                .op(SketchOp::AddPolygon {
                    centre: [0.1, 0.2, 0.0],
                    at: [0.1, 0.3, 0.0],
                    sides,
                    circumscribed,
                })
                .unwrap_or_else(|e| panic!("{sides} {circumscribed}: {e}"));
            let curves = Bench::curves(&created);
            let (lines, construction) = curves.split_at(sides as usize);
            assert!(!construction.is_empty());
            assert!(construction.iter().all(|c| bench.sketch().curves[c].construction));
            assert_eq!(bench.dof(), dof + 4, "{sides} {circumscribed}: centre, size and turn");
            let mut solved = bench.sketch().clone();
            solved.solve().unwrap();
            let first = length(&solved, lines[0]);
            assert!(lines.iter().all(|l| (length(&solved, *l) - first).abs() < 1e-9), "equal sides");
            // Size: the corner (inscribed) or the edge midpoint (circumscribed) is 0.1 from the centre.
            let expected = if circumscribed {
                0.2 * (std::f64::consts::PI / f64::from(sides)).tan()
            } else {
                0.2 * (std::f64::consts::PI / f64::from(sides)).sin()
            };
            assert!((first - expected).abs() < 1e-9, "{sides} {circumscribed}: {first} vs {expected}");
        }
    }
    let mut bench = Bench::new();
    for bad in [2, 121] {
        assert!(
            bench
                .op(SketchOp::AddPolygon {
                    centre: [0.0; 3],
                    at: [0.1, 0.0, 0.0],
                    sides: bad,
                    circumscribed: false
                })
                .is_err()
        );
    }
    assert!(
        bench
            .op(SketchOp::AddPolygon { centre: [0.0; 3], at: [0.0; 3], sides: 6, circumscribed: false })
            .is_err()
    );
    let created = bench
        .op(SketchOp::AddPolygon { centre: [0.0; 3], at: [0.1, 0.0, 0.0], sides: 120, circumscribed: true })
        .unwrap();
    assert_eq!(Bench::curves(&created).len(), 122, "120 sides, the circle and the radius");
}

/// Inventor 2026 help, To Create 2D Shape Geometry (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357), Slots:
/// Center to Center (arc centres, then width), Overall (start and end of the centre line, then width),
/// Center Point (slot centre, an arc centre, then width).
#[test]
fn straight_slots_keep_their_centres_and_width_with_five_degrees_of_freedom() {
    use gc_doc::SlotKind;
    for (kind, a, b) in [
        (SlotKind::CenterToCenter, [0.0, 0.0], [0.2, 0.1]),
        (
            SlotKind::Overall,
            [-0.05 * 0.894_427_190_999_915_9, -0.05 * 0.447_213_595_499_957_9],
            [0.2 + 0.05 * 0.894_427_190_999_915_9, 0.1 + 0.05 * 0.447_213_595_499_957_9],
        ),
        (SlotKind::CenterPoint, [0.1, 0.05], [0.2, 0.1]),
    ] {
        let mut bench = Bench::new();
        let dof = bench.dof();
        let created = bench
            .op(SketchOp::AddSlot { a: [a[0], a[1], 0.0], b: [b[0], b[1], 0.0], width: 0.1, kind })
            .unwrap();
        assert_eq!(bench.dof(), dof + 5, "{kind:?}: two centres and the width");
        let curves = Bench::curves(&created);
        assert_eq!(curves.len(), 5);
        let mut solved = bench.sketch().clone();
        solved.solve().unwrap();
        // The arc centres at (0, 0) and (0.2, 0.1) whichever way they were given; radius = width / 2.
        let centres: Vec<[f64; 3]> =
            curves.iter().filter_map(|c| solved.round(*c)).map(|(c, _)| solved.pos(c).unwrap()).collect();
        let near = |p: [f64; 3], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-9;
        assert!(
            centres.iter().any(|c| near(*c, [0.0, 0.0])) && centres.iter().any(|c| near(*c, [0.2, 0.1])),
            "{kind:?} {centres:?}"
        );
        assert!(curves.iter().filter_map(|c| solved.round(*c)).all(|(_, r)| (r - 0.05).abs() < 1e-9));
        // The sides are the centre line's length, and the centre line is construction.
        let sides: Vec<f64> = curves
            .iter()
            .filter(|c| !solved.curves[c].construction)
            .filter_map(|c| solved.line(*c).map(|_| length(&solved, *c)))
            .collect();
        assert_eq!(sides.len(), 2);
        assert!(sides.iter().all(|l| (l - 0.05f64.hypot(0.1) * 2.0).abs() < 1e-9), "{sides:?}");
    }
    let mut bench = Bench::new();
    let bad = |kind, a: [f64; 3], b: [f64; 3], width| SketchOp::AddSlot { a, b, width, kind };
    assert!(bench.op(bad(SlotKind::CenterToCenter, [0.0; 3], [0.0; 3], 0.1)).is_err(), "no length");
    assert!(bench.op(bad(SlotKind::CenterToCenter, [0.0; 3], [0.1, 0.0, 0.0], 0.0)).is_err(), "no width");
    assert!(bench.op(bad(SlotKind::Overall, [0.0; 3], [0.1, 0.0, 0.0], 0.1)).is_err(), "shorter than wide");
    assert!(bench.op(bad(SlotKind::CenterPoint, [0.0; 3], [f64::NAN, 0.0, 0.0], 0.1)).is_err());
}

fn add_line(bench: &mut Bench, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let created = bench
        .op(SketchOp::AddLine {
            a: [a[0], a[1], 0.0],
            b: [b[0], b[1], 0.0],
            snap_a: Default::default(),
            snap_b: Default::default(),
            horizontal: None,
        })
        .unwrap();
    Bench::curves(&created)[0]
}

/// Inventor 2026 help, To Create and Edit Sketch Patterns (GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE),
/// Create Mirror Patterns: select geometry, then the mirror line.
#[test]
fn mirror_copies_are_symmetric_about_the_line_and_add_no_freedom() {
    let mut bench = Bench::new();
    let axis = add_line(&mut bench, [0.0, -1.0], [0.0, 1.0]);
    // A line touching the axis, a circle and an arc on the right.
    let touching = add_line(&mut bench, [0.0, 0.2], [0.3, 0.4]);
    let circle = Bench::curves(
        &bench
            .op(SketchOp::AddCircle {
                centre: [0.5, 0.0, 0.0],
                radius: 0.1,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            })
            .unwrap(),
    )[0];
    let arc = Bench::curves(
        &bench
            .op(SketchOp::AddArc { centre: [0.4, -0.4, 0.0], start: [0.5, -0.4, 0.0], end: [0.4, -0.3, 0.0] })
            .unwrap(),
    )[0];
    // The line's end on the axis is held there.
    let end = bench.sketch().line(touching).unwrap().0;
    bench.op(SketchOp::AddConstraint(GeoConstraint::PointOnCurve(end, axis))).unwrap();
    let dof = bench.dof();
    let depth = bench.history.undo_depth();
    let created = bench.op(SketchOp::Mirror { curves: vec![touching, circle, arc], axis }).unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1);
    assert_eq!(bench.dof(), dof, "the copies follow their originals");
    let copies = Bench::curves(&created);
    assert_eq!(copies.len(), 3);
    let sketch = bench.sketch();
    // The touching line shares its end on the axis with its copy.
    assert_eq!(sketch.line(copies[0]).unwrap().0, end);
    let other = sketch.pos(sketch.line(copies[0]).unwrap().1).unwrap();
    assert!((other[0] + 0.3).abs() < 1e-12 && (other[1] - 0.4).abs() < 1e-12);
    // The circle's copy at (-0.5, 0), same radius; the arc's copy runs the other way round.
    let (centre, radius) = sketch.round(copies[1]).unwrap();
    assert!((sketch.pos(centre).unwrap()[0] + 0.5).abs() < 1e-12 && (radius - 0.1).abs() < 1e-12);
    let gc_sketch::CurveGeom::Arc { s, e, .. } = sketch.curves[&copies[2]].geom else { panic!() };
    assert!(
        (sketch.pos(s).unwrap()[0] + 0.4).abs() < 1e-12 && (sketch.pos(e).unwrap()[0] + 0.5).abs() < 1e-12
    );

    // Editing the original moves the copy: the circle grows, so does its mirror image.
    bench
        .op(SketchOp::AddDimension {
            kind: gc_sketch::DimKind::Radius(circle),
            source: "150".into(),
            text_pos: [0.0; 2],
        })
        .unwrap();
    let (_, r) = bench.sketch().round(copies[1]).unwrap();
    assert!((r - 0.15).abs() < 1e-9, "{r}");
    // Refused: not a line as the axis, the axis among the curves, nothing to mirror.
    assert!(bench.op(SketchOp::Mirror { curves: vec![touching], axis: circle }).is_err());
    assert!(bench.op(SketchOp::Mirror { curves: vec![axis], axis }).is_err());
    assert!(bench.op(SketchOp::Mirror { curves: vec![], axis }).is_err());
}
