//! More work plane, axis and point definitions (Inventor 2026 help, To Create Work Features,
//! GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385): Parallel to Plane Through Point, Midplane Between Two
//! Planes, Angle to Plane Around Edge, a line and a point, Normal to Axis Through Point, Normal to Curve
//! at Point; Intersection of Two Lines, of Plane and Line, of Three Planes, a curve's centre; an axis
//! through an ellipse's centre.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{
    CellOp, Command, Created, CurveRef, EndSnap, LineRef, OriginAxis, OriginPlane, PlaneRef, PointRef,
    SketchOp, WorkFeature, WorkGeometry, WorkShape,
};
use gc_math::DVec3;
use gc_session::Session;

fn work(s: &mut Session, geometry: WorkGeometry) -> Id<WorkFeature> {
    let feature = WorkFeature { name: "W".into(), geometry, visible: true };
    match s.execute(&Command::Cell(CellOp::AddWorkFeature(feature))).unwrap()[..] {
        [Created::WorkFeature(id)] => id,
        ref other => panic!("{other:?}"),
    }
}

/// Where a new work feature is; `None` when the definition does not work out (then it is refused and
/// nothing is added).
fn shape(s: &mut Session, geometry: WorkGeometry) -> Option<WorkShape> {
    let feature = WorkFeature { name: "W".into(), geometry, visible: true };
    let before = s.doc().clone();
    match s.execute(&Command::Cell(CellOp::AddWorkFeature(feature))) {
        Ok(created) => match created[..] {
            [Created::WorkFeature(id)] => s.doc().work_shape(id),
            ref other => panic!("{other:?}"),
        },
        Err(_) => {
            assert!(s.doc().same_content(&before), "a refused work feature leaves no trace");
            None
        }
    }
}

fn plane(s: &mut Session, geometry: WorkGeometry) -> (DVec3, DVec3, DVec3) {
    match shape(s, geometry) {
        Some(WorkShape::Plane(pose)) => {
            (pose.translation, pose.rotation * DVec3::Z, pose.rotation * DVec3::X)
        }
        other => panic!("{other:?}"),
    }
}

fn point(s: &mut Session, geometry: WorkGeometry) -> DVec3 {
    match shape(s, geometry) {
        Some(WorkShape::Point(p)) => p,
        other => panic!("{other:?}"),
    }
}

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

fn parallel(a: DVec3, b: DVec3) -> bool {
    a.cross(b).length() < 1e-9
}

fn xy() -> PlaneRef {
    PlaneRef::Origin(OriginPlane::Xy)
}

fn lifted(s: &mut Session, z: f64) -> PlaneRef {
    PlaneRef::Work(work(s, WorkGeometry::PlaneOffset { base: xy(), distance: z }))
}

fn sketch_curve(s: &mut Session, op: SketchOp) -> CurveRef {
    let sketch = match s.active_sketch() {
        Some((id, _)) => id,
        None => s.create_sketch("XY").unwrap(),
    };
    let created = s.execute(&Command::Sketch { sketch, op }).unwrap();
    let curve =
        created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap();
    CurveRef { sketch, curve }
}

#[test]
fn work_planes_parallel_midplane_angle_line_and_normals() {
    let mut s = Session::new();
    // Parallel to Plane Through Point.
    let (o, n, _) = plane(
        &mut s,
        WorkGeometry::PlaneParallelThrough { base: xy(), through: PointRef::Fixed([0.1, 0.2, 0.3]) },
    );
    assert!(near(o, DVec3::new(0.1, 0.2, 0.3)) && near(n, DVec3::Z));
    // Midplane: halfway, with the first plane's normal; two planes that are not parallel have none.
    let (a, b) = (lifted(&mut s, 0.1), lifted(&mut s, 0.5));
    let (o, n, _) = plane(&mut s, WorkGeometry::PlaneMidplane { planes: [a, b] });
    assert!((o.z - 0.3).abs() < 1e-9 && near(n, DVec3::Z));
    assert!(
        shape(&mut s, WorkGeometry::PlaneMidplane { planes: [xy(), PlaneRef::Origin(OriginPlane::Xz)] })
            .is_none()
    );
    // Angle to Plane Around Edge: about the X axis (in the XY plane): 0 lies in it, 90° stands square.
    let x = LineRef::Origin(OriginAxis::X);
    let (_, n, _) = plane(&mut s, WorkGeometry::PlaneAngleToPlane { base: xy(), line: x, angle: 0.0 });
    assert!(parallel(n, DVec3::Z));
    let right = std::f64::consts::FRAC_PI_2;
    let (o, n, ax) = plane(&mut s, WorkGeometry::PlaneAngleToPlane { base: xy(), line: x, angle: right });
    assert!(parallel(n, DVec3::Y) && parallel(ax, DVec3::X) && o.length() < 1e-9);
    // A line not parallel to the plane: no plane.
    let z = LineRef::Origin(OriginAxis::Z);
    assert!(shape(&mut s, WorkGeometry::PlaneAngleToPlane { base: xy(), line: z, angle: right }).is_none());
    // A line and a point.
    let (_, n, _) =
        plane(&mut s, WorkGeometry::PlaneLineAndPoint { line: x, through: PointRef::Fixed([0.5, 1.0, 0.0]) });
    assert!(parallel(n, DVec3::Z));
    assert!(
        shape(&mut s, WorkGeometry::PlaneLineAndPoint { line: x, through: PointRef::Fixed([2.0, 0.0, 0.0]) })
            .is_none()
    );
    // Normal to Axis Through Point: origin where the axis meets it, X towards the point.
    let (o, n, ax) =
        plane(&mut s, WorkGeometry::PlaneNormalToLine { line: z, through: PointRef::Fixed([1.0, 0.0, 2.0]) });
    assert!(near(o, DVec3::new(0.0, 0.0, 2.0)) && near(n, DVec3::Z) && near(ax, DVec3::X));
    // A work axis serves as the line too.
    let axis = work(
        &mut s,
        WorkGeometry::AxisThroughPoints { points: [PointRef::Origin, PointRef::Fixed([0.0, 1.0, 0.0])] },
    );
    let (_, n, _) = plane(
        &mut s,
        WorkGeometry::PlaneNormalToLine { line: LineRef::Work(axis), through: PointRef::Origin },
    );
    assert!(near(n, DVec3::Y));
}

#[test]
fn a_plane_normal_to_a_circle_ellipse_or_spline_at_a_point_on_it() {
    let mut s = Session::new();
    let circle = sketch_curve(
        &mut s,
        SketchOp::AddCircle {
            centre: [0.0, 0.0, 0.0],
            radius: 0.1,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    );
    let (o, n, _) = plane(
        &mut s,
        WorkGeometry::PlaneNormalToCurve { curve: circle, at: PointRef::Fixed([0.1, 0.0, 0.0]) },
    );
    assert!(near(o, DVec3::new(0.1, 0.0, 0.0)) && parallel(n, DVec3::Y));
    // Off the curve: none.
    assert!(
        shape(
            &mut s,
            WorkGeometry::PlaneNormalToCurve { curve: circle, at: PointRef::Fixed([0.2, 0.0, 0.0]) }
        )
        .is_none()
    );
    let ellipse = sketch_curve(
        &mut s,
        SketchOp::AddEllipse {
            centre: [1.0, 0.0, 0.0],
            major: [1.2, 0.0, 0.0],
            minor: 0.1,
            centre_snap: EndSnap::default(),
        },
    );
    // At the end of the minor axis the tangent runs along the major axis.
    let (_, n, _) = plane(
        &mut s,
        WorkGeometry::PlaneNormalToCurve { curve: ellipse, at: PointRef::Fixed([1.0, 0.1, 0.0]) },
    );
    assert!(parallel(n, DVec3::X), "{n:?}");
    let spline = sketch_curve(
        &mut s,
        SketchOp::AddSpline {
            points: vec![[0.0, 1.0, 0.0], [0.5, 1.0, 0.0], [1.0, 1.0, 0.0]],
            snaps: Vec::new(),
        },
    );
    let (_, n, _) = plane(
        &mut s,
        WorkGeometry::PlaneNormalToCurve { curve: spline, at: PointRef::Fixed([0.5, 1.0, 0.0]) },
    );
    assert!(parallel(n, DVec3::X), "{n:?}");
    // A line is not a curve for this.
    let line = sketch_curve(
        &mut s,
        SketchOp::AddLine {
            a: [0.0, 2.0, 0.0],
            b: [1.0, 2.0, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    );
    let feature = WorkFeature {
        name: "W".into(),
        geometry: WorkGeometry::PlaneNormalToCurve { curve: line, at: PointRef::Fixed([0.5, 2.0, 0.0]) },
        visible: true,
    };
    assert!(s.execute(&Command::Cell(CellOp::AddWorkFeature(feature))).is_err());
    // Centres and an axis through the ellipse's centre.
    assert!(near(
        point(&mut s, WorkGeometry::PointCurveCentre { curve: ellipse }),
        DVec3::new(1.0, 0.0, 0.0)
    ));
    assert!(near(point(&mut s, WorkGeometry::PointCurveCentre { curve: circle }), DVec3::ZERO));
    match shape(&mut s, WorkGeometry::AxisThroughCircle { curve: ellipse }) {
        Some(WorkShape::Axis { point, direction }) => {
            assert!(near(point, DVec3::new(1.0, 0.0, 0.0)) && parallel(direction, DVec3::Z));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn work_points_where_lines_and_planes_meet() {
    let mut s = Session::new();
    let line = sketch_curve(
        &mut s,
        SketchOp::AddLine {
            a: [0.2, -1.0, 0.0],
            b: [0.2, 1.0, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: None,
        },
    );
    let (x, z) = (LineRef::Origin(OriginAxis::X), LineRef::Origin(OriginAxis::Z));
    let p = point(&mut s, WorkGeometry::PointLinesIntersection { lines: [x, LineRef::Sketch(line)] });
    assert!(near(p, DVec3::new(0.2, 0.0, 0.0)));
    // Skew or parallel lines do not meet.
    let lifted_axis = work(
        &mut s,
        WorkGeometry::AxisThroughPoints {
            points: [PointRef::Fixed([0.0, 0.0, 1.0]), PointRef::Fixed([0.0, 1.0, 1.0])],
        },
    );
    assert!(
        shape(&mut s, WorkGeometry::PointLinesIntersection { lines: [x, LineRef::Work(lifted_axis)] })
            .is_none()
    );
    // Plane and line.
    let up = lifted(&mut s, 0.4);
    assert!(near(
        point(&mut s, WorkGeometry::PointPlaneLine { plane: up, line: z }),
        DVec3::new(0.0, 0.0, 0.4)
    ));
    assert!(shape(&mut s, WorkGeometry::PointPlaneLine { plane: up, line: x }).is_none());
    // Three planes.
    let side = PlaneRef::Work(work(
        &mut s,
        WorkGeometry::PlaneParallelThrough {
            base: PlaneRef::Origin(OriginPlane::Yz),
            through: PointRef::Fixed([0.3, 0.0, 0.0]),
        },
    ));
    let p = point(
        &mut s,
        WorkGeometry::PointThreePlanes { planes: [up, PlaneRef::Origin(OriginPlane::Xz), side] },
    );
    assert!(near(p, DVec3::new(0.3, 0.0, 0.4)), "{p:?}");
    // A work axis used by another feature cannot be deleted out from under it; saved and loaded.
    work(
        &mut s,
        WorkGeometry::PlaneNormalToLine { line: LineRef::Work(lifted_axis), through: PointRef::Origin },
    );
    assert!(s.execute(&Command::Cell(CellOp::DeleteWorkFeature(lifted_axis))).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.doc().work_features().iter().count(), s.doc().work_features().iter().count());
}
