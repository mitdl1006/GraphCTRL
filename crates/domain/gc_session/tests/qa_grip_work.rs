//! QA boundary tests for Grip Snap (`grip_snap_pose`, `grip_allows`, `grip_snap_commit`) and for the work
//! feature definitions the Work Plane / Axis / Point dialog sends. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: Grip Snap
//! GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C, work features GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385.
//! Every refused edit leaves the document and the history untouched; every accepted one is one undo step that
//! restores exactly.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::{AnyId, Id};
use gc_doc::{
    AxisSense, Body, CellOp, Command, ComponentFlag, Created, EditError, Geometry, GripLine, GripMotion,
    Occurrence, OriginPlane, Placement, PlaneRef, PointRef, RelationKind, Relationship, Selection, Solid,
    WorkFeature, WorkGeometry, WorkShape,
};
use gc_math::{DPose3, DQuat, DVec3};
use gc_session::Session;

fn body(s: &mut Session, xyz: [f64; 3]) -> Id<Body> {
    s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz, rpy: [0.0; 3] }).unwrap()
}

fn pose_of(s: &Session, b: Id<Body>) -> DPose3 {
    s.doc().component_origin(Occurrence::Item(b.erase())).unwrap().pose
}

fn grounded(s: &Session, b: Id<Body>) -> bool {
    s.doc().component_flag(Occurrence::Item(b.erase()), ComponentFlag::Grounded)
}

fn x_line() -> GripLine {
    GripLine::through([0.0; 3], [1.0, 0.0, 0.0]).unwrap()
}

fn z_line() -> GripLine {
    GripLine { point: [0.0; 3], direction: [0.0, 0.0, 1.0] }
}

fn same_rotation(a: DQuat, b: DQuat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-9
}

// ---- Pose arithmetic ----

#[test]
fn grip_lines_of_no_direction_or_non_finite_give_nothing() {
    let mut s = Session::new();
    let b = body(&mut s, [0.0, 0.0, 0.5]);
    for (what, line) in [
        ("zero direction", GripLine { point: [0.0; 3], direction: [0.0; 3] }),
        ("tiny direction", GripLine { point: [0.0; 3], direction: [1e-13, 0.0, 0.0] }),
        ("NaN direction", GripLine { point: [0.0; 3], direction: [f64::NAN, 0.0, 0.0] }),
        ("infinite direction", GripLine { point: [0.0; 3], direction: [f64::INFINITY, 0.0, 0.0] }),
        ("NaN point", GripLine { point: [f64::NAN, 0.0, 0.0], direction: [1.0, 0.0, 0.0] }),
    ] {
        for motion in [GripMotion::MoveAlong, GripMotion::RotateAbout] {
            assert!(
                s.grip_snap_pose(b.erase(), motion, line, [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]).is_none(),
                "{what} {motion:?} made a pose"
            );
            assert!(!s.grip_allows(b.erase(), motion, line), "{what} {motion:?} is allowed");
        }
    }
    assert!(GripLine::through([0.0; 3], [f64::NAN, 0.0, 0.0]).is_none());
    assert!(GripLine::through([0.0; 3], [1e-13, 0.0, 0.0]).is_none());
}

#[test]
fn moving_along_a_ray_has_edge_cases() {
    let mut s = Session::new();
    let b = body(&mut s, [0.1, 0.2, 0.3]);
    let origin = pose_of(&s, b);
    // The two points equal, or square to the line: no motion, the rotation untouched.
    for (from, to) in [([0.5; 3], [0.5; 3]), ([0.0; 3], [0.0, 1.0, 1.0])] {
        let pose = s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), from, to).unwrap();
        assert!((pose.translation - origin.translation).length() < 1e-12);
        assert!(same_rotation(pose.rotation, origin.rotation));
    }
    // Backwards along the line, and by a very large offset: finite.
    let back =
        s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [-0.4, 0.0, 0.0]).unwrap();
    assert!((back.translation.x - (0.1 - 0.4)).abs() < 1e-12);
    let far =
        s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [1e12, 0.0, 0.0]).unwrap();
    assert!(far.translation.is_finite());
    // A line whose direction is long (any length but zero) takes the same offset.
    let long = GripLine { point: [0.0; 3], direction: [1000.0, 0.0, 0.0] };
    let pose = s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, long, [0.0; 3], [0.3, 0.0, 0.0]).unwrap();
    assert!((pose.translation.x - 0.4).abs() < 1e-12);
    // Non-finite points: nothing.
    for bad in [[f64::NAN, 0.0, 0.0], [0.0, f64::INFINITY, 0.0]] {
        assert!(s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), bad, [0.0; 3]).is_none());
        assert!(s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], bad).is_none());
    }
}

#[test]
fn rotating_about_a_line_has_edge_cases() {
    let mut s = Session::new();
    let b = body(&mut s, [0.5, 0.0, 0.2]);
    let origin = pose_of(&s, b);
    // A point on the axis has no angle; so has the target point.
    assert!(
        s.grip_snap_pose(b.erase(), GripMotion::RotateAbout, z_line(), [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])
            .is_none()
    );
    assert!(
        s.grip_snap_pose(b.erase(), GripMotion::RotateAbout, z_line(), [1.0, 0.0, 0.0], [0.0, 0.0, 3.0])
            .is_none()
    );
    // A point so near the axis that it is on it.
    assert!(
        s.grip_snap_pose(b.erase(), GripMotion::RotateAbout, z_line(), [1e-14, 0.0, 0.0], [1.0, 0.0, 0.0])
            .is_none()
    );
    // Straight across (half a turn) and the same point (no turn).
    let half = s
        .grip_snap_pose(b.erase(), GripMotion::RotateAbout, z_line(), [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0])
        .unwrap();
    assert!((half.translation - DVec3::new(-0.5, 0.0, 0.2)).length() < 1e-12, "{:?}", half.translation);
    assert!((half.rotation * DVec3::X + DVec3::X).length() < 1e-12, "turned a half turn");
    let none = s
        .grip_snap_pose(b.erase(), GripMotion::RotateAbout, z_line(), [1.0, 0.0, 0.0], [2.0, 0.0, 0.0])
        .unwrap();
    assert!(
        (none.translation - origin.translation).length() < 1e-12
            && same_rotation(none.rotation, origin.rotation)
    );
    // Turning by an angle and back (swap the two points) is the identity, and the rotation stays a unit
    // quaternion however many times it is turned.
    let mut pose = origin;
    let mut s2 = Session::new();
    let c = body(&mut s2, origin.translation.to_array());
    let step = |p: DPose3, from: [f64; 3], to: [f64; 3]| -> DPose3 {
        let mut t = Session::new();
        let d = t
            .add_body(
                Solid::Box { size: [0.2; 3] },
                Placement { xyz: p.translation.to_array(), rpy: gc_math::rpy_from_quat(p.rotation) },
            )
            .unwrap();
        t.grip_snap_pose(d.erase(), GripMotion::RotateAbout, z_line(), from, to).unwrap()
    };
    let _ = c;
    let (a, b2) = ([1.0, 0.0, 0.0], [0.6, 0.8, 0.0]);
    pose = step(pose, a, b2);
    pose = step(pose, b2, a);
    assert!((pose.translation - origin.translation).length() < 1e-9, "{:?}", pose.translation);
    assert!(same_rotation(pose.rotation, origin.rotation));
    for _ in 0..1000 {
        pose = DPose3::from_parts(pose.translation, pose.rotation);
    }
    assert!((pose.rotation.length() - 1.0).abs() < 1e-9);
    // Reversing the line's direction:
    let down = GripLine { point: [0.0; 3], direction: [0.0, 0.0, -1.0] };
    let up = s
        .grip_snap_pose(b.erase(), GripMotion::RotateAbout, z_line(), [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        .unwrap();
    let dn =
        s.grip_snap_pose(b.erase(), GripMotion::RotateAbout, down, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    // The angle is the one between the two points seen along the line, so reversing the line gives the same snap.
    assert!((up.rotation * DVec3::X - DVec3::Y).length() < 1e-9, "a quarter turn from X to Y");
    assert!((up.translation - dn.translation).length() < 1e-9 && same_rotation(up.rotation, dn.rotation));
}

/// Not a component: nothing, for every kind of id that is not a body, robot or assembly, and for deleted ones.
#[test]
fn grip_snap_of_what_is_not_a_component_gives_nothing() {
    let mut s = Session::new();
    let b = body(&mut s, [0.0, 0.0, 0.5]);
    let sketch = s.create_sketch("XY").unwrap();
    s.finish_sketch();
    let ghost: AnyId = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    for (what, item) in [("sketch", sketch.erase()), ("ghost", ghost)] {
        assert!(
            s.grip_snap_pose(item, GripMotion::MoveAlong, x_line(), [0.0; 3], [1.0, 0.0, 0.0]).is_none(),
            "{what}"
        );
        assert!(!s.grip_allows(item, GripMotion::MoveAlong, x_line()), "{what}");
    }
    let extra = body(&mut s, [1.0, 0.0, 0.5]);
    s.delete_item(gc_session::CellItem::Body(extra)).unwrap();
    assert!(
        s.grip_snap_pose(extra.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [1.0, 0.0, 0.0]).is_none()
    );
    assert!(!s.grip_allows(extra.erase(), GripMotion::MoveAlong, x_line()));
    let _ = b;
}

// ---- Freedom ----

#[test]
fn the_freedom_report_follows_joints_suppression_and_grounding() {
    let mut s = Session::new();
    let floor = body(&mut s, [0.0, 0.0, -0.1]);
    let b = body(&mut s, [0.3, 0.0, 0.5]);
    assert!(grounded(&s, floor) && !grounded(&s, b));
    for motion in [GripMotion::MoveAlong, GripMotion::RotateAbout] {
        assert!(!s.grip_allows(floor.erase(), motion, x_line()), "a grounded component never moves");
        assert!(s.grip_allows(b.erase(), motion, x_line()), "a free one moves any way");
    }
    // A rigid joint to the floor takes everything away.
    let frame = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] },
    };
    s.add_relationship(Relationship::new(
        "Rigid",
        RelationKind::Rigid { flip: false },
        frame(b, -0.1, -1.0),
        frame(floor, 0.1, 1.0),
    ))
    .unwrap();
    for motion in [GripMotion::MoveAlong, GripMotion::RotateAbout] {
        for line in [x_line(), z_line(), GripLine { point: [0.2, 0.3, 0.1], direction: [1.0, 1.0, 1.0] }] {
            assert!(!s.grip_allows(b.erase(), motion, line), "a rigid component {motion:?}");
        }
    }
    // Suppressing the relationship frees it again (the report follows the document).
    let joint = s.relationships().into_iter().next().map(|(id, _)| id).unwrap();
    s.set_relationship_suppressed(joint, true).unwrap();
    assert!(s.grip_allows(b.erase(), GripMotion::MoveAlong, x_line()));
    // A grounded flag toggled off on the floor frees it; toggled on, takes it away.
    s.set_component(Occurrence::Item(b.erase()), ComponentFlag::Grounded, true).unwrap();
    assert!(!s.grip_allows(b.erase(), GripMotion::MoveAlong, x_line()));
}

// ---- Commit ----

#[test]
fn committing_nothing_or_bad_poses_is_refused_without_a_trace() {
    let mut s = Session::new();
    let _first = body(&mut s, [0.0, 1.0, 0.0]);
    let b = body(&mut s, [0.0; 3]);
    let ghost: AnyId = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    let good =
        s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [0.4, 0.0, 0.0]).unwrap();
    let nan = DPose3::from_parts(DVec3::new(f64::NAN, 0.0, 0.0), DQuat::IDENTITY);
    let inf = DPose3::from_parts(DVec3::new(0.0, f64::INFINITY, 0.0), DQuat::IDENTITY);
    let huge = DPose3::from_parts(DVec3::new(1e300, 0.0, 0.0), DQuat::IDENTITY);
    let bad_rotation = DPose3::from_parts(DVec3::ZERO, DQuat::from_xyzw(f64::NAN, 0.0, 0.0, 1.0));
    for (what, moves) in [
        ("empty", vec![]),
        ("ghost", vec![(ghost, good)]),
        ("ghost among real", vec![(b.erase(), good), (ghost, good)]),
        ("NaN position", vec![(b.erase(), nan)]),
        ("infinite position", vec![(b.erase(), inf)]),
        ("NaN rotation", vec![(b.erase(), bad_rotation)]),
    ] {
        for ground in [false, true] {
            let before = s.doc().clone();
            let depth = s.history().undo_depth();
            let result = s.grip_snap_commit(&moves, ground);
            assert!(result.is_err(), "{what} ground {ground} was accepted");
            assert!(s.doc().same_content(&before), "{what} ground {ground} left a trace");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
        }
    }
    // Huge but finite: refused with no trace, or accepted and finite (and one undo step).
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.grip_snap_commit(&[(b.erase(), huge)], false) {
        Err(_) => assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth),
        Ok(()) => {
            assert_eq!(s.history().undo_depth(), depth + 1);
            assert!(pose_of(&s, b).translation.is_finite());
            assert!(s.undo() && s.doc().same_content(&before));
        }
    }
}

/// Commit and Ground grounds what moved: an already grounded component stays grounded (no toggle), several
/// at once are all grounded in one undo step, and a component listed twice ends where its last pose put it.
#[test]
fn commit_and_ground_grounds_without_toggling_and_handles_several_components() {
    let mut s = Session::new();
    let first = body(&mut s, [0.0, 1.0, 0.0]);
    let (b, c) = (body(&mut s, [0.0; 3]), body(&mut s, [1.0, 0.0, 0.0]));
    assert!(grounded(&s, first) && !grounded(&s, b) && !grounded(&s, c));
    let move_to = |s: &Session, item: Id<Body>, dx: f64| {
        s.grip_snap_pose(item.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [dx, 0.0, 0.0]).unwrap()
    };
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    // First (already grounded), b and c together.
    let moves = [
        (first.erase(), move_to(&s, first, 0.1)),
        (b.erase(), move_to(&s, b, 0.2)),
        (c.erase(), move_to(&s, c, 0.3)),
    ];
    s.grip_snap_commit(&moves, true).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step for all");
    assert!(grounded(&s, first), "Commit and Ground un-grounded a grounded component");
    assert!(grounded(&s, b) && grounded(&s, c));
    assert!(
        (pose_of(&s, b).translation.x - 0.2).abs() < 1e-9
            && (pose_of(&s, c).translation.x - 1.3).abs() < 1e-9
    );
    assert!(s.undo());
    assert!(s.doc().same_content(&before), "undo of a multi-component commit is not exact");
    assert!(grounded(&s, first) && !grounded(&s, b) && !grounded(&s, c));
    // A component listed twice: accepted (the last pose wins) or refused whole; never half-moved.
    let twice = [(b.erase(), move_to(&s, b, 0.2)), (b.erase(), move_to(&s, b, 0.5))];
    match s.grip_snap_commit(&twice, false) {
        Ok(()) => {
            let x = pose_of(&s, b).translation.x;
            assert!((x - 0.2).abs() < 1e-9 || (x - 0.5).abs() < 1e-9, "{x}");
            assert!(s.undo() && s.doc().same_content(&before));
        }
        Err(_) => assert!(s.doc().same_content(&before)),
    }
}

/// What is tied to a committed component follows: a body mated to another that is snapped away moves with it
/// (relationships stay satisfied), and the whole thing undoes in one step.
#[test]
fn what_is_tied_to_a_committed_component_follows() {
    let mut s = Session::new();
    let floor = body(&mut s, [0.0, 0.0, -0.1]);
    let (b, c) = (body(&mut s, [0.5, 0.0, 0.1]), body(&mut s, [0.5, 0.0, 0.3]));
    let on = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
    };
    let frame = |item: Id<Body>, z: f64, nz: f64| Selection {
        occurrence: Occurrence::Item(item.erase()),
        geometry: Geometry::Frame { origin: [0.0, 0.0, z], z: [0.0, 0.0, nz], x: [1.0, 0.0, 0.0] },
    };
    let mate = RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed };
    s.add_relationship(Relationship::new("b on floor", mate, on(b, -0.1, -1.0), on(floor, 0.1, 1.0)))
        .unwrap();
    // c is joined rigidly to b, so it goes wherever b goes (a plain mate would let it stay and slide).
    s.add_relationship(Relationship::new(
        "c fixed to b",
        RelationKind::Rigid { flip: false },
        frame(c, -0.1, -1.0),
        frame(b, 0.1, 1.0),
    ))
    .unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let c_before = pose_of(&s, c).translation;
    let pose =
        s.grip_snap_pose(b.erase(), GripMotion::MoveAlong, x_line(), [0.0; 3], [0.4, 0.0, 0.0]).unwrap();
    s.grip_snap_commit(&[(b.erase(), pose)], false).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!((pose_of(&s, b).translation.x - 0.9).abs() < 1e-6);
    assert!(
        (pose_of(&s, c).translation.x - (c_before.x + 0.4)).abs() < 1e-6,
        "the body on top did not follow"
    );
    assert!(s.doc().solve_relationships().is_ok());
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

/// A rotation that ends at gimbal lock (pitch of 90 degrees) must survive the trip through the document's
/// roll-pitch-yaw placement: the pose after commit is the pose that was snapped.
#[test]
fn a_snapped_rotation_at_gimbal_lock_is_kept() {
    let mut s = Session::new();
    let _first = body(&mut s, [0.0, 1.0, 0.0]);
    let b = body(&mut s, [0.0; 3]);
    for turn in [
        DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
        DQuat::from_rotation_y(-std::f64::consts::FRAC_PI_2),
        DQuat::from_rotation_z(0.3)
            * DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2)
            * DQuat::from_rotation_x(0.7),
    ] {
        let pose = DPose3::from_parts(DVec3::new(0.1, 0.2, 0.3), turn.normalize());
        s.grip_snap_commit(&[(b.erase(), pose)], false).unwrap();
        let got = pose_of(&s, b);
        assert!((got.translation - pose.translation).length() < 1e-9);
        assert!(
            same_rotation(got.rotation, pose.rotation),
            "gimbal lock lost the rotation: {:?} vs {:?}",
            got.rotation,
            pose.rotation
        );
    }
}

// ---- Work feature definitions as the dialog sends them ----

fn add_feature(s: &mut Session, geometry: WorkGeometry) -> Result<Id<WorkFeature>, EditError> {
    let created = s.execute(&Command::Cell(CellOp::AddWorkFeature(WorkFeature {
        name: "Feature".into(),
        geometry,
        visible: true,
    })))?;
    created
        .iter()
        .find_map(|c| if let Created::WorkFeature(id) = c { Some(*id) } else { None })
        .ok_or(EditError::BadValue)
}

fn refused_feature(s: &mut Session, what: &str, geometry: WorkGeometry) {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = add_feature(s, geometry);
    assert!(result.is_err(), "{what} was accepted: {result:?}");
    assert!(s.doc().same_content(&before), "{what} left a trace");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
}

/// The dialog parses its fields with `str::parse::<f64>`, which accepts `NaN`, `inf` and `1e999`; the document
/// must not.
#[test]
fn work_features_with_non_finite_numbers_are_refused() {
    let mut s = Session::new();
    let xy = PlaneRef::Origin(OriginPlane::Xy);
    for text in ["NaN", "inf", "-inf", "1e999"] {
        let value: f64 = text.parse().unwrap();
        refused_feature(
            &mut s,
            &format!("offset {text}"),
            WorkGeometry::PlaneOffset { base: xy, distance: value / 1000.0 },
        );
        refused_feature(
            &mut s,
            &format!("typed point {text}"),
            WorkGeometry::Point { at: PointRef::Fixed([value / 1000.0, 0.0, 0.0]) },
        );
        refused_feature(
            &mut s,
            &format!("through point {text}"),
            WorkGeometry::AxisNormalToPlane { plane: xy, through: PointRef::Fixed([0.0, value, 0.0]) },
        );
    }
}

/// Zero, negative and huge offsets, and the typed point at the origin or far away.
#[test]
fn work_plane_offsets_and_typed_points_at_the_extremes() {
    let mut s = Session::new();
    let xy = PlaneRef::Origin(OriginPlane::Xy);
    for distance in [0.0, 0.01, -0.01, 1e6, -1e6, 1e-12] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        match add_feature(&mut s, WorkGeometry::PlaneOffset { base: xy, distance }) {
            Ok(id) => {
                assert_eq!(s.history().undo_depth(), depth + 1, "offset {distance}");
                let Some(WorkShape::Plane(pose)) = s.work_shape(id) else {
                    panic!("offset {distance} has no plane")
                };
                assert!(
                    (pose.translation.z - distance).abs() < 1e-9 * distance.abs().max(1.0),
                    "offset {distance}: {:?}",
                    pose.translation
                );
                assert!(s.undo() && s.doc().same_content(&before));
            }
            Err(_) => assert!(
                s.doc().same_content(&before) && s.history().undo_depth() == depth,
                "offset {distance}"
            ),
        }
    }
    for at in [[0.0; 3], [1e-3, -2e-3, 3e-3], [1e6, 1e6, 1e6], [-1e6, 0.0, 0.0]] {
        let id = add_feature(&mut s, WorkGeometry::Point { at: PointRef::Fixed(at) }).unwrap();
        let Some(WorkShape::Point(p)) = s.work_shape(id) else { panic!("no point") };
        assert!((p - DVec3::from(at)).length() < 1e-6);
    }
    // 1e300 mm: refused or finite.
    let before = s.doc().clone();
    match add_feature(&mut s, WorkGeometry::Point { at: PointRef::Fixed([1e300, 0.0, 0.0]) }) {
        Ok(id) => assert!(
            matches!(s.work_shape(id), Some(WorkShape::Point(p)) if p.is_finite())
                || s.work_shape(id).is_none()
        ),
        Err(_) => assert!(s.doc().same_content(&before)),
    }
}

/// Three points: collinear or repeated points make no plane; two equal points make no axis.
#[test]
fn work_planes_and_axes_through_degenerate_points_are_refused() {
    let mut s = Session::new();
    let at = |x: f64, y: f64, z: f64| PointRef::Fixed([x, y, z]);
    refused_feature(
        &mut s,
        "collinear points",
        WorkGeometry::PlaneThroughPoints {
            points: [at(0.0, 0.0, 0.0), at(1.0, 0.0, 0.0), at(2.0, 0.0, 0.0)],
        },
    );
    refused_feature(
        &mut s,
        "repeated point",
        WorkGeometry::PlaneThroughPoints {
            points: [at(0.0, 0.0, 0.0), at(0.0, 0.0, 0.0), at(2.0, 1.0, 0.0)],
        },
    );
    refused_feature(
        &mut s,
        "all one point",
        WorkGeometry::PlaneThroughPoints { points: [at(1.0, 1.0, 1.0); 3] },
    );
    refused_feature(
        &mut s,
        "an axis through one point",
        WorkGeometry::AxisThroughPoints { points: [at(1.0, 1.0, 1.0); 2] },
    );
    refused_feature(
        &mut s,
        "an axis through two points a hair apart",
        WorkGeometry::AxisThroughPoints { points: [at(0.0, 0.0, 0.0), at(1e-15, 0.0, 0.0)] },
    );
    // A real triangle works and the plane has a real normal.
    let id = add_feature(
        &mut s,
        WorkGeometry::PlaneThroughPoints {
            points: [at(0.0, 0.0, 0.0), at(1.0, 0.0, 0.0), at(0.0, 1.0, 0.0)],
        },
    )
    .unwrap();
    assert!(matches!(s.work_shape(id), Some(WorkShape::Plane(_))));
}

/// Editing a work feature (the dialog's Edit): to something invalid is refused with no trace, to something
/// valid is one undo step, and renaming/hiding keeps the definition.
#[test]
fn editing_a_work_feature_to_something_invalid_is_refused() {
    let mut s = Session::new();
    let xy = PlaneRef::Origin(OriginPlane::Xy);
    let id = add_feature(&mut s, WorkGeometry::PlaneOffset { base: xy, distance: 0.01 }).unwrap();
    let old = s.doc().work_features().get(id).unwrap().clone();
    let set = |geometry| CellOp::SetWorkFeature { id, feature: WorkFeature { geometry, ..old.clone() } };
    for (what, geometry) in [
        ("NaN offset", WorkGeometry::PlaneOffset { base: xy, distance: f64::NAN }),
        ("itself as the base", WorkGeometry::PlaneOffset { base: PlaneRef::Work(id), distance: 0.02 }),
        ("an axis between the same plane", WorkGeometry::AxisPlanesIntersection { planes: [xy, xy] }),
    ] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        let result = s.execute(&Command::Cell(set(geometry)));
        match result {
            Err(_) => {
                assert!(s.doc().same_content(&before), "{what} left a trace");
                assert_eq!(s.history().undo_depth(), depth, "{what}");
            }
            Ok(_) => panic!("{what} was accepted"),
        }
    }
    // A good edit: one undo step; undo restores exactly.
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.execute(&Command::Cell(set(WorkGeometry::PlaneOffset { base: xy, distance: 0.05 }))).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.undo() && s.doc().same_content(&before));
}

/// A work feature built on another that is deleted or edited into a cycle: the dependent is protected.
#[test]
fn work_features_that_depend_on_each_other_are_protected() {
    let mut s = Session::new();
    let xy = PlaneRef::Origin(OriginPlane::Xy);
    let base = add_feature(&mut s, WorkGeometry::PlaneOffset { base: xy, distance: 0.1 }).unwrap();
    let top =
        add_feature(&mut s, WorkGeometry::PlaneOffset { base: PlaneRef::Work(base), distance: 0.1 }).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    // The base cannot go while the other stands on it.
    assert!(
        s.execute(&Command::Cell(CellOp::DeleteWorkFeature(base))).is_err(),
        "deleted a feature that is in use"
    );
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    // The base standing on the one above it would be a cycle.
    let old = s.doc().work_features().get(base).unwrap().clone();
    let cycle = CellOp::SetWorkFeature {
        id: base,
        feature: WorkFeature {
            geometry: WorkGeometry::PlaneOffset { base: PlaneRef::Work(top), distance: 0.1 },
            ..old
        },
    };
    assert!(s.execute(&Command::Cell(cycle)).is_err(), "a cycle of work planes was accepted");
    assert!(s.doc().same_content(&before) && s.history().undo_depth() == depth);
    // The one on top can go; then the base.
    s.execute(&Command::Cell(CellOp::DeleteWorkFeature(top))).unwrap();
    s.execute(&Command::Cell(CellOp::DeleteWorkFeature(base))).unwrap();
    assert!(s.undo() && s.undo());
    assert!(s.doc().same_content(&before));
}
