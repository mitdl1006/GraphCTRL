//! Sketch-level acceptance tests (G1-G12 in docs/PLAN.md, appendix C).

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: exact values are compared and failures should panic"
)]

use std::num::NonZeroU64;

use gc_core::{AnyId, Id, IdAlloc, Kind};
use gc_sketch::{
    AddResult, Curve, DimKind, DragRefused, ExternalRef, FrameOutcome, GeoConstraint, Item, SkPoint, Sketch,
    SketchError, SketchGrab, Space, Status,
};

fn frame() -> AnyId {
    AnyId { kind: Kind::Frame, raw: NonZeroU64::new(1).unwrap() }
}

struct Fixture {
    sketch: Sketch,
    alloc: IdAlloc,
}

impl Fixture {
    fn planar() -> Self {
        let mut alloc = IdAlloc::new();
        alloc.reserve_through(NonZeroU64::new(1).unwrap());
        Self { sketch: Sketch::new(Space::Planar(frame())), alloc }
    }

    fn line(&mut self, a: [f64; 2], b: [f64; 2]) -> (Id<Curve>, Id<SkPoint>, Id<SkPoint>) {
        let id = self.sketch.add_line(&mut self.alloc, [a[0], a[1], 0.0], [b[0], b[1], 0.0]);
        let (p, q) = self.sketch.line(id).unwrap();
        (id, p, q)
    }

    fn constrain(&mut self, c: GeoConstraint) -> AddResult<GeoConstraint> {
        self.sketch.try_add_constraint(&mut self.alloc, c).unwrap()
    }

    fn must(&mut self, c: GeoConstraint) -> usize {
        match self.constrain(c) {
            AddResult::Added { report, .. } => report.dof,
            other => panic!("{c:?} was not added: {other:?}"),
        }
    }

    fn dimension(&mut self, kind: DimKind, value: f64) -> AddResult<gc_sketch::Dimension> {
        self.sketch.try_add_dimension(&mut self.alloc, kind, &value.to_string(), value, [0.0, 0.0]).unwrap()
    }

    fn xy(&self, p: Id<SkPoint>) -> [f64; 2] {
        let pos = self.sketch.pos(p).unwrap();
        [pos[0], pos[1]]
    }

    /// The rectangle tool's output: four lines, four coincident corners, H, V, H, V.
    fn rectangle(&mut self, w: f64, h: f64) -> [(Id<Curve>, Id<SkPoint>, Id<SkPoint>); 4] {
        let sides = [
            self.line([0.1, 0.2], [w, 0.0]),
            self.line([w, 0.1], [w + 0.2, h]),
            self.line([w, h], [0.0, h + 0.1]),
            self.line([0.1, h], [0.0, 0.0]),
        ];
        for i in 0..4 {
            self.must(GeoConstraint::Coincident(sides[i].2, sides[(i + 1) % 4].1));
        }
        self.must(GeoConstraint::Horizontal(sides[0].1, sides[0].2));
        self.must(GeoConstraint::Vertical(sides[1].1, sides[1].2));
        self.must(GeoConstraint::Horizontal(sides[2].1, sides[2].2));
        self.must(GeoConstraint::Vertical(sides[3].1, sides[3].2));
        sides
    }
}

#[test]
fn g1_rectangle_tool_output_and_full_constraint() {
    let mut f = Fixture::planar();
    let sides = f.rectangle(10.0, 5.0);
    let report = f.sketch.solve().unwrap();
    assert_eq!((report.status, report.dof), (Status::Solved, 4));
    assert!(report.curve_is_free(&f.sketch, sides[0].0));
    assert_eq!(f.xy(sides[0].2), f.xy(sides[1].1), "coincident corners are bit-identical");

    let origin = f.sketch.add_external(&mut f.alloc, ExternalRef::FrameOrigin(frame()), [0.0; 3]);
    assert_eq!(f.must(GeoConstraint::Coincident(sides[0].1, origin)), 2);
    let width = DimKind::Distance { p: sides[0].1, q: sides[0].2 };
    let height = DimKind::Distance { p: sides[1].1, q: sides[1].2 };
    let AddResult::Added { report, .. } = f.dimension(width, 12.0) else { panic!("width") };
    assert_eq!(report.dof, 1);
    let AddResult::Added { report, .. } = f.dimension(height, 7.0) else { panic!("height") };
    assert_eq!(report.dof, 0);
    assert!(!report.curve_is_free(&f.sketch, sides[2].0));
    assert_eq!(f.xy(sides[0].1), [0.0, 0.0]);
    let corner = f.xy(sides[1].2);
    assert!((corner[0] - 12.0).abs() < 1e-9 && (corner[1] - 7.0).abs() < 1e-9, "{corner:?}");

    // The diagonal follows from width and height: refused as redundant, offered as a reference dimension.
    let diagonal = DimKind::Distance { p: sides[0].1, q: sides[1].2 };
    let before = f.sketch.clone();
    assert!(matches!(f.dimension(diagonal, 193.0_f64.sqrt()), AddResult::Redundant(_)));
    assert!(matches!(f.dimension(diagonal, 20.0), AddResult::Conflicting(_)));
    assert_eq!(f.sketch, before, "a refused dimension changes nothing");
    let reference = f.sketch.add_reference_dimension(&mut f.alloc, diagonal, [6.0, 4.0]);
    let shown = f.sketch.dimensions[&reference].display_value(&f.sketch).unwrap();
    assert!((shown - 193.0_f64.sqrt()).abs() < 1e-9);
}

#[test]
fn g2_a_conflicting_dimension_names_what_it_conflicts_with() {
    let mut f = Fixture::planar();
    let (_, a, b) = f.line([0.0, 0.0], [9.0, 0.5]);
    f.must(GeoConstraint::Horizontal(a, b));
    let AddResult::Added { id: length, .. } = f.dimension(DimKind::Distance { p: a, q: b }, 10.0) else {
        panic!("length")
    };
    let AddResult::Conflicting(with) = f.dimension(DimKind::HDistance { p: a, q: b, sign: 1.0 }, 7.0) else {
        panic!("should conflict")
    };
    assert!(with.contains(&Item::Dimension(length)), "{with:?}");
}

#[test]
fn width_between_two_horizontal_lines_is_a_valid_dimension() {
    let mut f = Fixture::planar();
    let (bottom, a, b) = f.line([0.0, 0.0], [10.0, 0.2]);
    let (_, c, d) = f.line([0.0, 5.0], [10.0, 5.3]);
    f.must(GeoConstraint::Horizontal(a, b));
    f.must(GeoConstraint::Horizontal(c, d));
    let before = f.sketch.solve().unwrap().dof;
    let AddResult::Added { report, .. } =
        f.dimension(DimKind::PointLine { p: c, line: bottom, side: 1.0 }, 6.0)
    else {
        panic!("the width dimension must be accepted")
    };
    assert_eq!(report.dof, before - 1);
    assert!((f.xy(c)[1] - f.xy(a)[1] - 6.0).abs() < 1e-9);
}

#[test]
fn g3_a_line_tangent_to_a_circle_stays_on_its_side() {
    let mut f = Fixture::planar();
    let (line, a, b) = f.line([0.0, 0.0], [10.0, 0.0]);
    let circle = f.sketch.add_circle(&mut f.alloc, [4.0, 3.2, 0.0], 3.0).unwrap();
    f.must(GeoConstraint::Fix(a));
    f.must(GeoConstraint::Fix(b));
    f.must(GeoConstraint::Tangent { a: line, b: circle, side: 1.0, external: true });
    let (centre, radius) = f.sketch.round(circle).unwrap();
    assert!((f.xy(centre)[1] - radius).abs() < 1e-9, "the circle touches the line from above");
    let AddResult::Added { report, .. } = f.dimension(DimKind::Radius(circle), 2.0) else { panic!("radius") };
    assert_eq!(report.dof, 1, "only sliding along the line is left");
    assert!((f.xy(centre)[1] - 2.0).abs() < 1e-9);
    assert!(matches!(
        f.sketch.try_add_constraint(&mut f.alloc, GeoConstraint::Parallel(line, circle)),
        Err(SketchError::WrongCurveKind)
    ));
}

#[test]
fn g8_three_dimensional_points() {
    let mut alloc = IdAlloc::new();
    alloc.reserve_through(NonZeroU64::new(1).unwrap());
    let mut sketch = Sketch::new(Space::Spatial(frame()));
    let origin = sketch.add_external(&mut alloc, ExternalRef::FrameOrigin(frame()), [0.0; 3]);
    let p = sketch.add_point(&mut alloc, [1.0, 2.0, 2.5]);
    assert_eq!(sketch.solve().unwrap().dof, 3);
    let distance = DimKind::Distance { p: origin, q: p };
    let AddResult::Added { report, .. } =
        sketch.try_add_dimension(&mut alloc, distance, "2", 2.0, [0.0; 2]).unwrap()
    else {
        panic!("distance")
    };
    assert_eq!(report.dof, 2);
    let mut on_plane = sketch.clone();
    let AddResult::Added { report, .. } =
        on_plane.try_add_constraint(&mut alloc, GeoConstraint::OnPlane { p, axis: 2 }).unwrap()
    else {
        panic!("on plane")
    };
    assert_eq!(report.dof, 1);
    assert_eq!(on_plane.pos(p).unwrap()[2], 0.0);
    let AddResult::Added { report, .. } =
        sketch.try_add_constraint(&mut alloc, GeoConstraint::AlongAxis { p: origin, q: p, axis: 0 }).unwrap()
    else {
        panic!("along axis")
    };
    assert_eq!(report.dof, 0);
    let pos = sketch.pos(p).unwrap();
    assert!((pos[0] - 2.0).abs() < 1e-9 && pos[1] == 0.0 && pos[2] == 0.0);
    assert_eq!(sketch.add_circle(&mut alloc, [0.0; 3], 1.0), Err(SketchError::WrongSpace));
}

#[test]
fn g12_dimension_edits_never_flip_and_reject_bad_values() {
    let mut f = Fixture::planar();
    let sides = f.rectangle(10.0, 5.0);
    let origin = f.sketch.add_external(&mut f.alloc, ExternalRef::FrameOrigin(frame()), [0.0; 3]);
    f.must(GeoConstraint::Coincident(sides[0].1, origin));
    let AddResult::Added { id: width, .. } =
        f.dimension(DimKind::Distance { p: sides[0].1, q: sides[0].2 }, 10.0)
    else {
        panic!("width")
    };
    for value in [0.001, 1000.0, 10.0] {
        let report = f.sketch.set_dimension(width, &value.to_string(), value).unwrap();
        assert_eq!(report.status, Status::Solved);
        let end = f.xy(sides[0].2);
        assert!((end[0] - value).abs() < 1e-7 * value.max(1.0), "width {value}: {end:?}");
        assert!(end[0] > 0.0, "the rectangle never mirrors");
    }
    let before = f.sketch.clone();
    assert_eq!(f.sketch.set_dimension(width, "-5", -5.0), Err(SketchError::InvalidValue));
    assert_eq!(f.sketch.set_dimension(width, "0", 0.0), Err(SketchError::InvalidValue));
    assert_eq!(f.sketch, before, "a rejected value changes nothing");
    assert_eq!(f.sketch.dimensions[&width].source, "10");
}

#[test]
fn an_unreachable_dimension_value_leaves_the_sketch_untouched() {
    let mut f = Fixture::planar();
    let (_, a, b) = f.line([0.0, 0.0], [10.0, 0.0]);
    let (_, c, d) = f.line([0.0, 0.0], [4.0, 3.0]);
    let (_, e, g) = f.line([4.0, 3.0], [10.0, 0.0]);
    f.must(GeoConstraint::Fix(a));
    f.must(GeoConstraint::Fix(b));
    f.must(GeoConstraint::Coincident(a, c));
    f.must(GeoConstraint::Coincident(d, e));
    f.must(GeoConstraint::Coincident(g, b));
    let AddResult::Added { id: left, .. } = f.dimension(DimKind::Distance { p: c, q: d }, 5.0) else {
        panic!()
    };
    let AddResult::Added { .. } = f.dimension(DimKind::Distance { p: e, q: g }, 6.5) else { panic!() };
    let before = f.sketch.clone();
    // 1 + 6.5 < 10: the triangle cannot close.
    assert_eq!(f.sketch.set_dimension(left, "1", 1.0), Err(SketchError::Unsolved(Status::Inconsistent)));
    assert_eq!(f.sketch, before);
    assert!(f.sketch.violation().unwrap() <= 1.0);
}

#[test]
fn deleting_a_curve_takes_its_points_constraints_and_dimensions_along() {
    let mut f = Fixture::planar();
    let sides = f.rectangle(10.0, 5.0);
    let AddResult::Added { id: width, .. } =
        f.dimension(DimKind::Distance { p: sides[0].1, q: sides[0].2 }, 10.0)
    else {
        panic!()
    };
    assert_eq!((f.sketch.curves.len(), f.sketch.points.len(), f.sketch.constraints.len()), (4, 8, 8));
    f.sketch.delete(&[Item::Curve(sides[0].0)]);
    assert_eq!((f.sketch.curves.len(), f.sketch.points.len()), (3, 6));
    // Gone: the bottom's Horizontal and the two Coincident constraints at its ends.
    assert_eq!(f.sketch.constraints.len(), 5);
    assert!(!f.sketch.dimensions.contains_key(&width));
    assert_eq!(f.sketch.solve().unwrap().status, Status::Solved);
    // A point owned by a curve cannot be deleted on its own.
    f.sketch.delete(&[Item::Point(sides[1].1)]);
    assert_eq!(f.sketch.points.len(), 6);
}

#[test]
fn dragging_previews_without_touching_the_sketch_until_applied() {
    let mut f = Fixture::planar();
    let sides = f.rectangle(10.0, 5.0);
    f.sketch.solve().unwrap();
    let before = f.sketch.clone();
    let corner = sides[1].2;
    let start = f.xy(corner);
    let mut drag = f.sketch.begin_drag(SketchGrab::Point(corner), start).unwrap();
    assert_eq!(drag.update([start[0] + 3.0, start[1] + 2.0]), FrameOutcome::Moved);
    assert_eq!(f.sketch, before, "the sketch is unchanged while dragging");
    let preview = drag.pos(corner).unwrap();
    assert!((preview[0] - start[0] - 3.0).abs() < 1e-7 && (preview[1] - start[1] - 2.0).abs() < 1e-7);
    assert!(drag.apply(&mut f.sketch));
    // Still a rectangle: the neighbours followed.
    assert_eq!(f.xy(sides[1].1)[0], f.xy(corner)[0]);
    assert_eq!(f.xy(sides[2].2)[1], f.xy(corner)[1]);
    assert!(f.sketch.violation().unwrap() <= 1.0);
}

#[test]
fn fully_constrained_geometry_refuses_to_be_dragged() {
    let mut f = Fixture::planar();
    let (line, a, b) = f.line([0.0, 0.0], [10.0, 0.0]);
    f.must(GeoConstraint::Fix(a));
    f.must(GeoConstraint::Fix(b));
    assert_eq!(
        f.sketch.begin_drag(SketchGrab::Point(b), [10.0, 0.0]).err(),
        Some(DragRefused::NothingMobile)
    );
    assert_eq!(
        f.sketch.begin_drag(SketchGrab::Curve(line), [5.0, 0.0]).err(),
        Some(DragRefused::NothingMobile)
    );
}

#[test]
fn measurements_use_the_signs_of_their_driving_form() {
    let mut f = Fixture::planar();
    let (l1, a, b) = f.line([0.0, 0.0], [10.0, 0.0]);
    let (l2, _, d) = f.line([0.0, 0.0], [0.0, 4.0]);
    let measure = |kind| f.sketch.measure(&kind).unwrap();
    assert_eq!(measure(DimKind::Distance { p: a, q: b }), 10.0);
    assert_eq!(measure(DimKind::HDistance { p: b, q: a, sign: -1.0 }), 10.0);
    assert_eq!(measure(DimKind::PointLine { p: d, line: l1, side: 1.0 }), 4.0);
    let quarter = std::f64::consts::FRAC_PI_2;
    assert!((measure(DimKind::Angle { l1, rev1: false, l2, rev2: false }) - quarter).abs() < 1e-12);
    assert!(
        (measure(DimKind::Angle { l1: l2, rev1: false, l2: l1, rev2: false }) - 3.0 * quarter).abs() < 1e-12
    );
}
