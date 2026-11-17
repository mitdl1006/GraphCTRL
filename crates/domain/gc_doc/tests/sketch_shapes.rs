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

fn dimensions(created: &[Created]) -> Vec<Id<gc_sketch::Dimension>> {
    created.iter().filter_map(|c| if let Created::Dimension(id) = c { Some(*id) } else { None }).collect()
}

fn centre_of(bench: &Bench, circle: Id<Curve>) -> [f64; 2] {
    let (c, _) = bench.sketch().round(circle).unwrap();
    let p = bench.sketch().pos(c).unwrap();
    [p[0], p[1]]
}

/// Inventor 2026 help, To Create and Edit Sketch Patterns (GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE):
/// Rectangular: geometry, Direction 1 and 2 with Count and Spacing (an equation), Fitted; "Patterned
/// geometry is fully constrained".
#[test]
fn rectangular_patterns_are_fully_constrained_and_follow_their_spacing() {
    use gc_doc::SketchPatternDirection;
    let mut bench = Bench::new();
    let along_x = add_line(&mut bench, [0.0, -0.5], [1.0, -0.5]);
    let along_y = add_line(&mut bench, [-0.5, 0.0], [-0.5, 1.0]);
    let hole = Bench::curves(
        &bench
            .op(SketchOp::AddCircle {
                centre: [0.0, 0.0, 0.0],
                radius: 0.01,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            })
            .unwrap(),
    )[0];
    let held = bench.sketch().round(hole).unwrap().0;
    bench.op(SketchOp::AddConstraint(GeoConstraint::Fix(held))).unwrap();
    let dof = bench.dof();
    let direction = |line, count, spacing: &str| SketchPatternDirection {
        line,
        count,
        spacing: spacing.into(),
        flip: false,
    };
    let created = bench
        .op(SketchOp::RectangularPattern {
            curves: vec![hole],
            first: direction(along_x, 3, "50"),
            second: Some(direction(along_y, 2, "30")),
            fitted: false,
        })
        .unwrap();
    assert_eq!(bench.dof(), dof, "fully constrained to the original");
    let circles: Vec<Id<Curve>> =
        Bench::curves(&created).into_iter().filter(|c| bench.sketch().round(*c).is_some()).collect();
    assert_eq!(circles.len(), 5, "3 × 2, less the original");
    let mut centres: Vec<[f64; 2]> = circles.iter().map(|c| centre_of(&bench, *c)).collect();
    centres.sort_by(|a, b| a[1].total_cmp(&b[1]).then(a[0].total_cmp(&b[0])));
    let expected = [[0.05, 0.0], [0.1, 0.0], [0.0, 0.03], [0.05, 0.03], [0.1, 0.03]];
    for (c, e) in centres.iter().zip(expected) {
        assert!((c[0] - e[0]).abs() < 1e-9 && (c[1] - e[1]).abs() < 1e-9, "{centres:?}");
    }
    // One spacing dimension per direction; editing the first moves the columns.
    let dims = dimensions(&created);
    assert_eq!(dims.len(), 2);
    bench.op(SketchOp::SetDimension { id: dims[0], source: "80".into() }).unwrap();
    let mut xs: Vec<f64> = circles.iter().map(|c| centre_of(&bench, *c)[0]).collect();
    xs.sort_by(f64::total_cmp);
    assert!((xs[4] - 0.16).abs() < 1e-7, "{xs:?}");
    // Fitted: 100 mm overall for three elements is 50 mm apart; flip goes the other way.
    let mut bench2 = Bench::new();
    let along = add_line(&mut bench2, [0.0, -0.5], [1.0, -0.5]);
    let dot = Bench::curves(
        &bench2
            .op(SketchOp::AddCircle {
                centre: [0.0; 3],
                radius: 0.01,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            })
            .unwrap(),
    )[0];
    let created = bench2
        .op(SketchOp::RectangularPattern {
            curves: vec![dot],
            first: SketchPatternDirection { line: along, count: 3, spacing: "100".into(), flip: true },
            second: None,
            fitted: true,
        })
        .unwrap();
    let mut xs: Vec<f64> = Bench::curves(&created)
        .into_iter()
        .filter_map(|c| bench2.sketch().round(c).map(|_| centre_of(&bench2, c)[0]))
        .collect();
    xs.sort_by(f64::total_cmp);
    assert!((xs[0] + 0.1).abs() < 1e-9 && (xs[1] + 0.05).abs() < 1e-9, "{xs:?}");
    // Refused: one element, the direction among the curves, a bad spacing.
    assert!(
        bench2
            .op(SketchOp::RectangularPattern {
                curves: vec![dot],
                first: direction(along, 1, "10"),
                second: None,
                fitted: false
            })
            .is_err()
    );
    assert!(
        bench2
            .op(SketchOp::RectangularPattern {
                curves: vec![along],
                first: direction(along, 2, "10"),
                second: None,
                fitted: false
            })
            .is_err()
    );
    assert!(
        bench2
            .op(SketchOp::RectangularPattern {
                curves: vec![dot],
                first: direction(along, 2, "-5"),
                second: None,
                fitted: false
            })
            .is_err()
    );
}

/// Circular: an axis (point), Count and Angle (an equation), Fitted.
#[test]
fn circular_patterns_turn_round_the_axis_and_stay_constrained() {
    let mut bench = Bench::new();
    let centre = Bench::points(&bench.op(SketchOp::PlacePoint { pos: [0.0; 3], center: true }).unwrap())[0];
    bench.op(SketchOp::AddConstraint(GeoConstraint::Fix(centre))).unwrap();
    // A short radial line and a small circle at 0.1 on X.
    let spoke = add_line(&mut bench, [0.05, 0.0], [0.08, 0.0]);
    let hole = Bench::curves(
        &bench
            .op(SketchOp::AddCircle {
                centre: [0.1, 0.0, 0.0],
                radius: 0.005,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            })
            .unwrap(),
    )[0];
    let dof = bench.dof();
    let created = bench
        .op(SketchOp::CircularPattern {
            curves: vec![spoke, hole],
            axis: centre,
            count: 6,
            angle: "360 deg".into(),
            flip: false,
            fitted: true,
        })
        .unwrap();
    assert_eq!(bench.dof(), dof, "fully constrained");
    let circles: Vec<Id<Curve>> =
        Bench::curves(&created).into_iter().filter(|c| bench.sketch().round(*c).is_some()).collect();
    assert_eq!(circles.len(), 5);
    let mut angles: Vec<f64> = circles
        .iter()
        .map(|c| {
            let p = centre_of(&bench, *c);
            p[1].atan2(p[0]).to_degrees().rem_euclid(360.0)
        })
        .collect();
    angles.sort_by(f64::total_cmp);
    for (a, e) in angles.iter().zip([60.0, 120.0, 180.0, 240.0, 300.0]) {
        assert!((a - e).abs() < 1e-7, "{angles:?}");
    }
    assert!(circles.iter().all(|c| {
        let p = centre_of(&bench, *c);
        (p[0].hypot(p[1]) - 0.1).abs() < 1e-9
    }));
    // Clockwise, 30° apart, not fitted.
    let created = bench
        .op(SketchOp::CircularPattern {
            curves: vec![hole],
            axis: centre,
            count: 2,
            angle: "30 deg".into(),
            flip: true,
            fitted: false,
        })
        .unwrap();
    let copy = Bench::curves(&created).into_iter().find(|c| bench.sketch().round(*c).is_some()).unwrap();
    let p = centre_of(&bench, copy);
    assert!((p[1].atan2(p[0]).to_degrees() + 30.0).abs() < 1e-7, "{p:?}");
    assert!(
        bench
            .op(SketchOp::CircularPattern {
                curves: vec![hole],
                axis: centre,
                count: 1,
                angle: "30 deg".into(),
                flip: false,
                fitted: false
            })
            .is_err()
    );
    assert!(
        bench
            .op(SketchOp::CircularPattern {
                curves: vec![hole],
                axis: centre,
                count: 3,
                angle: "0 deg".into(),
                flip: false,
                fitted: false
            })
            .is_err()
    );
}

/// Inventor 2026 help, To Stretch, Scale, or Offset 2D Sketch Geometry
/// (GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4): "By default, Inventor selects loops ... and constrains the
/// offset geometry to be equidistant from the original curve"; Loop Select and Constrain Offset can be
/// turned off.
#[test]
fn offset_takes_whole_loops_and_keeps_one_distance() {
    let rectangle = |bench: &mut Bench| -> Vec<Id<Curve>> {
        let created = bench
            .op(SketchOp::AddRectangle { a: [0.0, 0.0], b: [0.2, 0.1], snap_a: Default::default() })
            .unwrap();
        Bench::curves(&created)
    };
    // One side picked; the loop is taken; the offset is outside and 10 mm away everywhere.
    let mut bench = Bench::new();
    let sides = rectangle(&mut bench);
    let dof = bench.dof();
    let created = bench
        .op(SketchOp::Offset { curves: vec![sides[0]], distance: 0.01, loop_select: true, constrain: true })
        .unwrap();
    let made: Vec<Id<Curve>> =
        Bench::curves(&created).into_iter().filter(|c| !bench.sketch().curves[c].construction).collect();
    assert_eq!(made.len(), 4);
    assert_eq!(bench.dof(), dof + 1, "the offset distance alone");
    let corners: Vec<[f64; 3]> = made
        .iter()
        .flat_map(|c| {
            let (a, b) = bench.sketch().line(*c).unwrap();
            [bench.sketch().pos(a).unwrap(), bench.sketch().pos(b).unwrap()]
        })
        .collect();
    let (min_x, max_x) =
        corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let (min_y, max_y) =
        corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
    assert!(
        (min_x + 0.01).abs() < 1e-9
            && (max_x - 0.21).abs() < 1e-9
            && (min_y + 0.01).abs() < 1e-9
            && (max_y - 0.11).abs() < 1e-9,
        "{corners:?}"
    );
    // A negative distance goes inside.
    let created = bench
        .op(SketchOp::Offset { curves: vec![sides[1]], distance: -0.02, loop_select: true, constrain: true })
        .unwrap();
    let inner: Vec<[f64; 3]> = Bench::curves(&created)
        .into_iter()
        .filter(|c| !bench.sketch().curves[c].construction)
        .flat_map(|c| {
            let (a, _) = bench.sketch().line(c).unwrap();
            [bench.sketch().pos(a).unwrap()]
        })
        .collect();
    assert!(inner.iter().all(|p| p[0] > 0.0 && p[0] < 0.2 && p[1] > 0.0 && p[1] < 0.1), "{inner:?}");

    // Constrain Offset off: each side keeps its own distance (four degrees of freedom).
    let mut bench = Bench::new();
    let sides = rectangle(&mut bench);
    let dof = bench.dof();
    bench
        .op(SketchOp::Offset { curves: vec![sides[0]], distance: 0.01, loop_select: true, constrain: false })
        .unwrap();
    assert_eq!(bench.dof(), dof + 4);
    // Loop Select off: only the picked side.
    let created = bench
        .op(SketchOp::Offset { curves: vec![sides[0]], distance: 0.01, loop_select: false, constrain: true })
        .unwrap();
    assert_eq!(
        Bench::curves(&created).into_iter().filter(|c| !bench.sketch().curves[c].construction).count(),
        1
    );

    // A slot (lines and arcs meeting tangentially) and a circle.
    let mut bench = Bench::new();
    let slot = Bench::curves(
        &bench
            .op(SketchOp::AddSlot {
                a: [0.0; 3],
                b: [0.2, 0.0, 0.0],
                width: 0.1,
                kind: gc_doc::SlotKind::CenterToCenter,
            })
            .unwrap(),
    );
    let dof = bench.dof();
    let created = bench
        .op(SketchOp::Offset { curves: vec![slot[0]], distance: 0.01, loop_select: true, constrain: true })
        .unwrap();
    assert_eq!(bench.dof(), dof + 1, "slot offset: its distance");
    let arcs: Vec<f64> =
        Bench::curves(&created).into_iter().filter_map(|c| bench.sketch().round(c).map(|(_, r)| r)).collect();
    assert_eq!(arcs.len(), 2);
    assert!(arcs.iter().all(|r| (r - 0.06).abs() < 1e-9), "{arcs:?}");
    let circle = Bench::curves(
        &bench
            .op(SketchOp::AddCircle {
                centre: [1.0, 0.0, 0.0],
                radius: 0.05,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            })
            .unwrap(),
    )[0];
    let created = bench
        .op(SketchOp::Offset { curves: vec![circle], distance: 0.02, loop_select: true, constrain: true })
        .unwrap();
    let r =
        Bench::curves(&created).into_iter().find_map(|c| bench.sketch().round(c).map(|(_, r)| r)).unwrap();
    assert!((r - 0.07).abs() < 1e-12);
    // Refused: nothing, no distance, a circle offset to nothing.
    assert!(
        bench
            .op(SketchOp::Offset { curves: vec![], distance: 0.01, loop_select: true, constrain: true })
            .is_err()
    );
    assert!(
        bench
            .op(SketchOp::Offset { curves: vec![circle], distance: 0.0, loop_select: true, constrain: true })
            .is_err()
    );
    assert!(
        bench
            .op(SketchOp::Offset {
                curves: vec![circle],
                distance: -0.06,
                loop_select: true,
                constrain: true
            })
            .is_err()
    );
}

/// Inventor 2026 help, To Create 2D Shape Geometry (GUID-D489CE6D-7299-4211-A43A-F3580A4BA357), Slots:
/// the arc slots are a centre arc, concentric outer and inner arcs and half-round ends.
#[test]
fn arc_slots_keep_centre_radius_angles_and_width_with_six_degrees_of_freedom() {
    let mut bench = Bench::new();
    let dof = bench.dof();
    let created = bench
        .op(SketchOp::AddArcSlot {
            centre: [0.0, 0.0, 0.0],
            start: [0.1, 0.0, 0.0],
            end: [0.0, 0.3, 0.0],
            width: 0.02,
        })
        .unwrap();
    let sketch = bench.sketch();
    let curves = Bench::curves(&created);
    let drawn: Vec<_> = curves.iter().filter(|c| !sketch.curves[c].construction).collect();
    assert_eq!(drawn.len(), 4, "outer, inner and two ends");
    let mut radii: Vec<f64> = drawn.iter().filter_map(|c| sketch.round(**c).map(|(_, r)| r)).collect();
    radii.sort_by(f64::total_cmp);
    for (got, want) in radii.iter().zip([0.01, 0.01, 0.09, 0.11]) {
        assert!((got - want).abs() < 1e-9, "{radii:?}");
    }
    assert_eq!(bench.dof(), dof + 6, "centre, radius, two angles and the width");
    let bad = |centre: [f64; 3], start: [f64; 3], end: [f64; 3], width| SketchOp::AddArcSlot {
        centre,
        start,
        end,
        width,
    };
    assert!(bench.op(bad([0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0], 0.01)).is_err(), "no sweep");
    assert!(bench.op(bad([0.0; 3], [0.1, 0.0, 0.0], [0.0, 0.1, 0.0], 0.2)).is_err(), "wider than the arc");
    assert!(bench.op(bad([0.0; 3], [0.0; 3], [0.0, 0.1, 0.0], 0.01)).is_err(), "no radius");
}

/// Inventor 2026 help, To Create and Edit Sketch Patterns (GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE),
/// Edit Rectangular and Circular Patterns: Edit Pattern (change the values), Delete Pattern (the entire
/// pattern), Suppress Element(s) (dashed, relationships kept).
#[test]
fn sketch_patterns_can_be_edited_suppressed_and_deleted_as_a_whole() {
    use gc_sketch::PatternDefinition;
    let mut bench = Bench::new();
    let hole = Bench::curves(
        &bench
            .op(SketchOp::AddCircle {
                centre: [0.05, 0.0, 0.0],
                radius: 0.01,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            })
            .unwrap(),
    )[0];
    let axis =
        bench.sketch().points.keys().copied().find(|p| bench.sketch().pos(*p) == Some([0.0; 3])).unwrap();
    let before = bench.sketch().clone();
    let drawn = |bench: &Bench| bench.sketch().curves.values().filter(|c| !c.construction).count();
    bench
        .op(SketchOp::CircularPattern {
            curves: vec![hole],
            axis,
            count: 4,
            angle: "360 deg".into(),
            flip: false,
            fitted: true,
        })
        .unwrap();
    assert_eq!(drawn(&bench), 4);
    let record = &bench.sketch().patterns[0];
    assert_eq!(record.elements.len(), 3);
    let copy = record.elements[1][0];
    assert_eq!(bench.sketch().pattern_of(hole), Some(0));
    assert_eq!(bench.sketch().pattern_element_of(copy), Some((0, 1)));

    // Suppress: the element turns dashed (construction), the others stay; and back.
    bench.op(SketchOp::SuppressPatternElements { curves: vec![copy], suppress: true }).unwrap();
    assert!(bench.sketch().curves[&copy].construction);
    assert_eq!(drawn(&bench), 3);
    assert_eq!(bench.sketch().patterns[0].suppressed, vec![1]);
    bench.op(SketchOp::SuppressPatternElements { curves: vec![copy], suppress: false }).unwrap();
    assert!(!bench.sketch().curves[&copy].construction);

    // Edit: six around, the second element suppressed; through a copy or the original alike.
    let definition = PatternDefinition::Circular {
        curves: vec![hole],
        axis,
        count: 6,
        angle: "360 deg".into(),
        flip: false,
        fitted: true,
    };
    bench.op(SketchOp::EditSketchPattern { element: copy, definition, suppressed: vec![1] }).unwrap();
    assert_eq!(bench.sketch().patterns.len(), 1);
    assert_eq!(bench.sketch().patterns[0].elements.len(), 5);
    assert_eq!(drawn(&bench), 5, "six less the suppressed one");
    assert!(!bench.sketch().curves.contains_key(&copy), "the old copies are gone");

    // Delete Pattern: back to the original alone, nothing left over.
    let element = bench.sketch().patterns[0].elements[0][0];
    bench.op(SketchOp::DeleteSketchPattern { element }).unwrap();
    let after = bench.sketch();
    assert!(after.patterns.is_empty());
    assert_eq!(after.curves.len(), before.curves.len());
    assert_eq!(after.points.len(), before.points.len());
    assert_eq!(after.constraints.len(), before.constraints.len());
    assert_eq!(after.dimensions.len(), before.dimensions.len());

    // Deleting the original forgets the pattern.
    bench
        .op(SketchOp::CircularPattern {
            curves: vec![hole],
            axis,
            count: 3,
            angle: "360 deg".into(),
            flip: false,
            fitted: true,
        })
        .unwrap();
    // Delete leaves pattern copies alone ("removes nonpattern geometry"); an original takes its pattern.
    let copy = bench.sketch().patterns[0].elements[0][0];
    bench.op(SketchOp::Delete(vec![gc_sketch::Item::Curve(copy)])).unwrap();
    assert!(bench.sketch().curves.contains_key(&copy));
    bench.op(SketchOp::Delete(vec![gc_sketch::Item::Curve(hole)])).unwrap();
    assert!(bench.sketch().patterns.is_empty());
    assert!(!bench.sketch().curves.contains_key(&copy));
    assert_eq!(bench.sketch().curves.len(), before.curves.len() - 1);
    assert!(bench.op(SketchOp::DeleteSketchPattern { element: hole }).is_err());
}
