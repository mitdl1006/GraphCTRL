//! QA boundary tests for the Assemble command's offered solutions (`possible_relations`), joint Lock and
//! Protect, and the new work axes (On Line, Parallel Through Point, Intersection of Two Planes, Through
//! Center of Circular Shape). Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: Assemble command
//! GUID-C1E8D955-784D-47DE-9DD4-C9C695051320, joint Lock and Protect GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6,
//! work axes GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385.
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
    AssembleSolution, Body, CellOp, Command, Created, CurveRef, EditError, EndSnap, Geometry, Occurrence,
    OriginPlane, Placement, PlaneRef, PointRef, RelationKind, Relationship, Selection, SketchOp, Solid,
    WorkFeature, WorkGeometry, WorkShape,
};
use gc_session::Session;

fn placed(xyz: [f64; 3]) -> Placement {
    Placement { xyz, rpy: [0.0; 3] }
}

fn sel(item: AnyId, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(item), geometry }
}

fn ring(z: f64, down: bool) -> Geometry {
    Geometry::Circle { centre: [0.0, 0.0, z], normal: [0.0, 0.0, if down { -1.0 } else { 1.0 }] }
}

fn relation(kind: RelationKind, first: Selection, second: Selection) -> Relationship {
    Relationship::new("R", kind, first, second)
}

fn table_and_block() -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let table = s.add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, placed([0.0, 0.0, 0.5])).unwrap();
    let block = s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.3, 0.2, 1.6])).unwrap();
    (s, table, block)
}

fn refused_with<T: std::fmt::Debug>(
    s: &mut Session,
    what: &str,
    f: impl FnOnce(&mut Session) -> Result<T, EditError>,
) -> EditError {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let result = f(s);
    assert!(result.is_err(), "{what} was accepted: {result:?}");
    assert!(s.doc().same_content(&before), "{what} changed the document");
    assert_eq!(s.history().undo_depth(), depth, "{what}");
    result.unwrap_err()
}

// ---- Assemble: possible_relations ----

fn geometries() -> Vec<(&'static str, Geometry)> {
    vec![
        ("plane", Geometry::Plane { point: [0.0, 0.0, 0.1], normal: [0.0, 0.0, 1.0] }),
        ("axis", Geometry::Axis { point: [0.0; 3], direction: [0.0, 0.0, 1.0] }),
        ("point", Geometry::Point([0.0, 0.0, 0.1])),
        ("circle", ring(0.1, false)),
        ("cylinder", Geometry::Cylinder { point: [0.0; 3], direction: [0.0, 0.0, 1.0], radius: 0.05 }),
        ("sphere", Geometry::Sphere { centre: [0.0; 3], radius: 0.05 }),
        ("frame", Geometry::Frame { origin: [0.0; 3], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] }),
    ]
}

/// Over every pair of geometry kinds on two different components: only solutions that make a valid
/// relationship are offered, none twice; Aligned only for two linear geometries; Tangent only when one is
/// round. Each offered solution adds as one undo step or is refused without a trace — never a panic.
#[test]
fn assemble_offers_only_valid_solutions_and_each_one_applies_or_is_refused_cleanly() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let linear = |name: &str| matches!(name, "axis" | "cylinder" | "circle");
    let round = |name: &str| matches!(name, "cylinder" | "sphere");
    for (n1, g1) in geometries() {
        for (n2, g2) in geometries() {
            let (first, second) = (sel(b, g1), sel(t, g2));
            let offered = s.possible_relations(&first, &second);
            let mut seen = Vec::new();
            for solution in &offered {
                assert!(!seen.contains(solution), "{n1}/{n2}: {solution:?} offered twice");
                seen.push(*solution);
                assert!(
                    Relationship::new("", solution.kind(0.0), first, second).is_valid(),
                    "{n1}/{n2}: {solution:?} offered but invalid"
                );
                match solution {
                    AssembleSolution::MateAligned => assert!(linear(n1) && linear(n2), "{n1}/{n2}: aligned"),
                    AssembleSolution::TangentInside | AssembleSolution::TangentOutside => {
                        assert!(round(n1) || round(n2), "{n1}/{n2}: tangent");
                    }
                    _ => {}
                }
                let before = s.doc().clone();
                let depth = s.history().undo_depth();
                match s.add_relationship(relation(solution.kind(0.0), first, second)) {
                    Ok(_) => {
                        assert_eq!(s.history().undo_depth(), depth + 1, "{n1}/{n2} {solution:?}: one step");
                        assert!(s.undo());
                        assert!(s.doc().same_content(&before), "{n1}/{n2} {solution:?}: undo restores");
                    }
                    Err(_) => {
                        assert!(
                            s.doc().same_content(&before),
                            "{n1}/{n2} {solution:?}: refused with a trace"
                        );
                        assert_eq!(s.history().undo_depth(), depth);
                    }
                }
            }
        }
    }
}

/// Both picks on one component, a component that does not exist and a non-finite geometry offer nothing
/// (or something valid) and never panic.
#[test]
fn assemble_offers_nothing_for_one_component_or_broken_picks() {
    let (s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let plane = Geometry::Plane { point: [0.0; 3], normal: [0.0, 0.0, 1.0] };
    assert!(s.possible_relations(&sel(t, plane), &sel(t, plane)).is_empty(), "one component twice");
    let ghost = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    drop(s.possible_relations(&sel(ghost, plane), &sel(t, plane)));
    for bad in [
        Geometry::Plane { point: [0.0; 3], normal: [0.0; 3] },
        Geometry::Plane { point: [f64::NAN, 0.0, 0.0], normal: [0.0, 0.0, 1.0] },
        Geometry::Axis { point: [0.0; 3], direction: [0.0; 3] },
        Geometry::Axis { point: [0.0; 3], direction: [f64::INFINITY, 0.0, 0.0] },
        Geometry::Cylinder { point: [0.0; 3], direction: [0.0, 0.0, 1.0], radius: -1.0 },
        Geometry::Cylinder { point: [0.0; 3], direction: [0.0, 0.0, 1.0], radius: f64::NAN },
        Geometry::Sphere { centre: [0.0; 3], radius: 0.0 },
        Geometry::Circle { centre: [0.0; 3], normal: [0.0; 3] },
        Geometry::Frame { origin: [0.0; 3], z: [0.0; 3], x: [0.0; 3] },
    ] {
        for (first, second) in [(sel(b, bad), sel(t, plane)), (sel(t, plane), sel(b, bad))] {
            let offered = s.possible_relations(&first, &second);
            for solution in offered {
                assert!(
                    Relationship::new("", solution.kind(0.0), first, second).is_valid(),
                    "{bad:?}: {solution:?}"
                );
            }
        }
    }
}

/// Applying with broken values (NaN or infinite offset or angle) is refused without a trace.
#[test]
fn applying_an_assemble_solution_with_a_bad_value_is_refused() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let first = sel(b, Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] });
    let second = sel(t, Geometry::Plane { point: [0.0, 0.0, 0.5], normal: [0.0, 0.0, 1.0] });
    for solution in s.possible_relations(&first, &second) {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            refused_with(&mut s, &format!("{solution:?} {value}"), |s| {
                s.add_relationship(relation(solution.kind(value), first, second))
            });
        }
    }
}

// ---- Joint Lock and Protect ----

fn rotational(s: &mut Session, table: Id<Body>, block: Id<Body>) -> Id<gc_doc::Relationship> {
    s.add_relationship(relation(
        RelationKind::Rotational { flip: false },
        sel(block.erase(), ring(-0.1, true)),
        sel(table.erase(), ring(0.5, false)),
    ))
    .unwrap()
}

fn mate(s: &mut Session, table: Id<Body>, block: Id<Body>) -> Id<gc_doc::Relationship> {
    s.add_relationship(relation(
        RelationKind::Mate { offset: 0.0, sense: gc_doc::AxisSense::Opposed },
        sel(block.erase(), Geometry::Plane { point: [0.0, 0.0, -0.1], normal: [0.0, 0.0, -1.0] }),
        sel(table.erase(), Geometry::Plane { point: [0.0, 0.0, 0.5], normal: [0.0, 0.0, 1.0] }),
    ))
    .unwrap()
}

fn pose(s: &Session, body: Id<Body>) -> Placement {
    s.shown_body(body).unwrap().placement
}

/// Constraints and relationships that do not exist have neither Lock nor Protect, with no trace.
#[test]
fn lock_and_protect_refuse_constraints_and_missing_relationships() {
    let (mut s, table, block) = table_and_block();
    let constraint = mate(&mut s, table, block);
    let ghost = Id::<gc_doc::Relationship>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    for id in [constraint, ghost] {
        for on in [true, false] {
            refused_with(&mut s, &format!("lock {id:?} {on}"), |s| s.set_joint_lock(id, on));
            refused_with(&mut s, &format!("protect {id:?} {on}"), |s| s.set_joint_protect(id, on));
        }
    }
    assert_eq!(s.joint_condition(ghost), gc_doc::JointCondition::default());
}

/// Lock is one undo step that undoes exactly; locking twice or unlocking an unlocked joint leaves no stray
/// step with a different result; the lock and protect flags are independent.
#[test]
fn lock_is_one_undo_step_and_independent_of_protect() {
    let (mut s, table, block) = table_and_block();
    let joint = rotational(&mut s, table, block);
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.set_joint_lock(joint, true).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.joint_condition(joint).locked.is_some() && !s.joint_condition(joint).protected);
    // Locking again keeps it locked (no change of the pose), whatever the step count.
    let locked_once = s.joint_condition(joint).locked;
    drop(s.set_joint_lock(joint, true));
    assert_eq!(s.joint_condition(joint).locked, locked_once, "locking again moved the lock");
    // Protect leaves the lock alone and the other way round.
    s.set_joint_protect(joint, true).unwrap();
    assert!(s.joint_condition(joint).locked.is_some() && s.joint_condition(joint).protected);
    s.set_joint_lock(joint, false).unwrap();
    assert!(s.joint_condition(joint).locked.is_none() && s.joint_condition(joint).protected);
    s.set_joint_protect(joint, false).unwrap();
    assert_eq!(s.joint_condition(joint), gc_doc::JointCondition::default());
    // Unlocking a joint that is not locked is harmless.
    drop(s.set_joint_lock(joint, false));
    assert_eq!(s.joint_condition(joint), gc_doc::JointCondition::default());
    while s.history().undo_depth() > depth {
        assert!(s.undo());
    }
    assert!(s.doc().same_content(&before));
}

/// A locked joint moves with what it is joined to (help: Lock removes the motion, the component still
/// follows), and the move is one undo step that restores both.
#[test]
fn a_locked_joint_follows_the_other_component_and_undo_restores_both() {
    let (mut s, table, block) = table_and_block();
    let joint = rotational(&mut s, table, block);
    s.set_joint_lock(joint, true).unwrap();
    let (t0, b0) = (pose(&s, table), pose(&s, block));
    let before = s.doc().clone();
    let mut moved = s.doc().get(table).unwrap().clone();
    moved.placement.xyz = [0.7, -0.3, 0.9];
    s.execute(&Command::Cell(CellOp::SetBody { id: table, body: moved })).unwrap();
    let (t1, b1) = (pose(&s, table), pose(&s, block));
    for k in 0..3 {
        let (dt, db) = (t1.xyz[k] - t0.xyz[k], b1.xyz[k] - b0.xyz[k]);
        assert!((dt - db).abs() < 1e-7, "axis {k}: the table moved {dt}, the locked block {db}");
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    assert_eq!(pose(&s, block), b0);
}

/// Driving a locked joint cannot move the block (Lock removes all motion): refused, or nothing moves.
#[test]
fn driving_a_locked_joint_does_not_move_it() {
    let (mut s, table, block) = table_and_block();
    let joint = rotational(&mut s, table, block);
    s.set_joint_lock(joint, true).unwrap();
    let before = s.doc().clone();
    let b0 = pose(&s, block);
    let result = s.drive_relationship(joint, 1.0);
    if result.is_err() {
        assert!(s.doc().same_content(&before));
    }
    let b1 = pose(&s, block);
    let moved = (0..3).any(|k| (b1.xyz[k] - b0.xyz[k]).abs() > 1e-9)
        || (0..3).any(|k| (b1.rpy[k] - b0.rpy[k]).abs() > 1e-9);
    assert!(!moved, "a locked joint turned when driven: {b0:?} -> {b1:?}");
}

/// Protect: a relationship that takes freedom away from the protected joint is refused with
/// `EditError::Protected`, no trace; one that leaves its freedom is accepted; with Protect off the first
/// one goes in. Deleting the joint clears its condition.
#[test]
fn protect_refuses_freedom_taking_relationships_without_a_trace() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let joint = rotational(&mut s, table, block);
    s.set_joint_protect(joint, true).unwrap();
    let stop = relation(
        RelationKind::Angle {
            angle: std::f64::consts::FRAC_PI_2,
            solution: gc_doc::AngleSolution::Undirected,
        },
        sel(b, Geometry::Plane { point: [0.1, 0.0, 0.0], normal: [1.0, 0.0, 0.0] }),
        sel(t, Geometry::Plane { point: [0.0, 0.5, 0.0], normal: [0.0, 1.0, 0.0] }),
    );
    let error = refused_with(&mut s, "freedom-taking angle", |s| s.add_relationship(stop.clone()));
    assert!(matches!(error, EditError::Protected(_)), "{error:?}");
    // A second try, and a second protected-joint refusal, are the same.
    let error = refused_with(&mut s, "again", |s| s.add_relationship(stop.clone()));
    assert!(matches!(error, EditError::Protected(_)));
    // Unprotected: it goes in, as one undo step.
    s.set_joint_protect(joint, false).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    s.add_relationship(stop).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    // Deleting a protected, locked joint clears what was kept about it; undo brings it all back.
    s.set_joint_protect(joint, true).unwrap();
    s.set_joint_lock(joint, true).unwrap();
    let with_joint = s.doc().clone();
    let condition = s.joint_condition(joint);
    s.execute(&Command::Cell(CellOp::DeleteRelationship(joint))).unwrap();
    assert_eq!(
        s.joint_condition(joint),
        gc_doc::JointCondition::default(),
        "the condition outlived the joint"
    );
    assert!(s.undo());
    assert!(s.doc().same_content(&with_joint));
    assert_eq!(s.joint_condition(joint), condition);
}

// ---- Work axes ----

type K = Id<gc_doc::Sketch>;

fn sketch_run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn line(s: &mut Session, k: K, a: [f64; 2], b: [f64; 2]) -> Id<gc_sketch::Curve> {
    let op = SketchOp::AddLine {
        a: [a[0], a[1], 0.0],
        b: [b[0], b[1], 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    sketch_run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
}

fn circle(s: &mut Session, k: K) -> Id<gc_sketch::Curve> {
    sketch_run(
        s,
        k,
        SketchOp::AddCircle {
            centre: [0.1, 0.1, 0.0],
            radius: 0.02,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
    .unwrap()
    .iter()
    .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
    .unwrap()
}

fn add_axis(s: &mut Session, geometry: WorkGeometry) -> Result<Id<WorkFeature>, EditError> {
    let created = s.execute(&Command::Cell(CellOp::AddWorkFeature(WorkFeature {
        name: "Axis".into(),
        geometry,
        visible: true,
    })))?;
    created
        .iter()
        .find_map(|c| if let Created::WorkFeature(id) = c { Some(*id) } else { None })
        .ok_or(EditError::BadValue)
}

fn offset_plane(s: &mut Session, distance: f64) -> Id<WorkFeature> {
    add_axis(s, WorkGeometry::PlaneOffset { base: PlaneRef::Origin(OriginPlane::Xy), distance }).unwrap()
}

/// The same plane twice, a plane that does not exist: refused at creation with no trace; two planes that do
/// meet give a real line in one undo step.
#[test]
fn an_axis_between_the_same_or_missing_planes_is_refused_and_a_real_pair_works() {
    let mut s = Session::new();
    let xy = PlaneRef::Origin(OriginPlane::Xy);
    refused_with(&mut s, "same plane twice", |s| {
        add_axis(s, WorkGeometry::AxisPlanesIntersection { planes: [xy, xy] })
    });
    let ghost = Id::<WorkFeature>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    refused_with(&mut s, "ghost plane", |s| {
        add_axis(s, WorkGeometry::AxisPlanesIntersection { planes: [xy, PlaneRef::Work(ghost)] })
    });
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let id = add_axis(
        &mut s,
        WorkGeometry::AxisPlanesIntersection { planes: [xy, PlaneRef::Origin(OriginPlane::Yz)] },
    )
    .unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    match s.work_shape(id) {
        Some(WorkShape::Axis { direction, .. }) => assert!((direction.length() - 1.0).abs() < 1e-9),
        other => panic!("{other:?}"),
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
}

/// Two planes that never meet (parallel and apart, or the same plane by two different work planes) have no
/// line of intersection: the axis must be refused at creation, not accepted as a feature with no shape.
#[test]
#[ignore = "BUG: an axis between two parallel planes is accepted although it has no line (work_shape is None); only identical references are refused (owner: backend, gc_doc work_feature_ok)"]
fn an_axis_between_parallel_planes_is_refused() {
    let mut s = Session::new();
    let xy = PlaneRef::Origin(OriginPlane::Xy);
    let lifted = PlaneRef::Work(offset_plane(&mut s, 0.1));
    let lifted_again = PlaneRef::Work(offset_plane(&mut s, 0.3));
    let same = PlaneRef::Work(offset_plane(&mut s, 0.0));
    for (what, planes) in [
        ("parallel, apart", [xy, lifted]),
        ("parallel, both lifted", [lifted, lifted_again]),
        ("coincident in another guise", [xy, same]),
    ] {
        refused_with(&mut s, what, |s| add_axis(s, WorkGeometry::AxisPlanesIntersection { planes }));
    }
}

/// A work axis on (or parallel to) a line of zero length has no direction, so it must be refused.
/// (A zero-length line cannot be drawn, but a projected reference line from a point to itself is accepted.)
#[test]
#[ignore = "BUG: a work axis on a zero-length line is accepted although it has no direction (work_shape is None) (owner: backend, gc_doc work_feature_ok should check the shape exists)"]
fn an_axis_on_a_zero_length_line_is_refused() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    sketch_run(&mut s, k, SketchOp::ProjectedLines { lines: vec![([0.0; 3], [0.0; 3])] }).unwrap();
    let sketch = s.active_sketch().unwrap().1;
    let zero = sketch
        .curves
        .keys()
        .copied()
        .find(|c| {
            sketch.line(*c).is_some_and(|(a, b)| {
                let (a, b) = (sketch.pos(a).unwrap(), sketch.pos(b).unwrap());
                (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-12
            })
        })
        .unwrap();
    for geometry in [
        WorkGeometry::AxisOnLine { line: CurveRef { sketch: k, curve: zero } },
        WorkGeometry::AxisParallelThrough {
            line: CurveRef { sketch: k, curve: zero },
            through: PointRef::Fixed([0.0, 1.0, 0.0]),
        },
    ] {
        refused_with(&mut s, &format!("{geometry:?}"), |s| add_axis(s, geometry.clone()));
    }
}

/// Wrong kinds of curve, ghosts, a point equal to nothing: all refused.
#[test]
fn axes_on_the_wrong_kind_of_curve_or_missing_curve_are_refused() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, [0.0, 0.0], [0.05, 0.0]);
    let c = circle(&mut s, k);
    let ghost_curve = Id::<gc_sketch::Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let ghost_sketch = Id::<gc_doc::Sketch>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let cr = |curve| CurveRef { sketch: k, curve };
    for (what, geometry) in [
        ("line axis on a circle", WorkGeometry::AxisOnLine { line: cr(c) }),
        ("circle axis on a line", WorkGeometry::AxisThroughCircle { curve: cr(l) }),
        ("ghost curve", WorkGeometry::AxisOnLine { line: cr(ghost_curve) }),
        ("ghost sketch", WorkGeometry::AxisOnLine { line: CurveRef { sketch: ghost_sketch, curve: l } }),
        (
            "parallel on a circle",
            WorkGeometry::AxisParallelThrough { line: cr(c), through: PointRef::Origin },
        ),
        (
            "parallel through a ghost work point",
            WorkGeometry::AxisParallelThrough {
                line: cr(l),
                through: PointRef::Work(Id::from_raw(std::num::NonZeroU64::new(9999).unwrap())),
            },
        ),
        (
            "parallel through a NaN point",
            WorkGeometry::AxisParallelThrough { line: cr(l), through: PointRef::Fixed([f64::NAN, 0.0, 0.0]) },
        ),
    ] {
        refused_with(&mut s, what, |s| add_axis(s, geometry));
    }
    // The good ones go in, each one undo step, each with a real direction.
    for geometry in [
        WorkGeometry::AxisOnLine { line: cr(l) },
        WorkGeometry::AxisThroughCircle { curve: cr(c) },
        WorkGeometry::AxisParallelThrough { line: cr(l), through: PointRef::Fixed([0.0, 0.2, 0.3]) },
    ] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        let id = add_axis(&mut s, geometry.clone()).unwrap();
        assert_eq!(s.history().undo_depth(), depth + 1, "{geometry:?}");
        assert!(matches!(s.work_shape(id), Some(WorkShape::Axis { .. })), "{geometry:?}");
        assert!(s.undo());
        assert!(s.doc().same_content(&before));
    }
}

/// The reference sketch of a work axis cannot be deleted (help: the feature depends on it); refused with no
/// trace. Deleting the curve it is built on must not leave an axis that silently has no shape.
#[test]
fn the_sketch_a_work_axis_refers_to_cannot_be_deleted() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, [0.0, 0.0], [0.05, 0.0]);
    let c = circle(&mut s, k);
    for geometry in [
        WorkGeometry::AxisOnLine { line: CurveRef { sketch: k, curve: l } },
        WorkGeometry::AxisThroughCircle { curve: CurveRef { sketch: k, curve: c } },
    ] {
        let axis = add_axis(&mut s, geometry.clone()).unwrap();
        s.finish_sketch();
        refused_with(&mut s, &format!("delete the sketch of {geometry:?}"), |s| {
            s.execute(&Command::DeleteSketch { id: k })
        });
        // With the axis gone (one undo step) the sketch can be deleted, and the delete undoes exactly.
        s.execute(&Command::Cell(CellOp::DeleteWorkFeature(axis))).unwrap();
        let before = s.doc().clone();
        s.execute(&Command::DeleteSketch { id: k }).unwrap();
        assert!(s.undo());
        assert!(s.doc().same_content(&before));
        s.enter_sketch(k);
    }
}

/// Deleting the curve an axis is built on (inside the sketch) is refused, or takes the axis along; it must not
/// leave an axis pointing at nothing.
#[test]
#[ignore = "BUG: deleting the sketch line a work axis follows is accepted and leaves the axis with no shape (the sketch itself is protected, its curves are not) (owner: backend, gc_doc sketch Delete should refuse or take the axis along)"]
fn deleting_the_curve_of_a_work_axis_leaves_no_dangling_axis() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let l = line(&mut s, k, [0.0, 0.0], [0.05, 0.0]);
    let axis = add_axis(&mut s, WorkGeometry::AxisOnLine { line: CurveRef { sketch: k, curve: l } }).unwrap();
    let result = sketch_run(&mut s, k, SketchOp::Delete(vec![gc_sketch::Item::Curve(l)]));
    if result.is_ok() {
        let alive = s.doc().work_features().iter().any(|(id, _)| id == axis);
        assert!(!alive || s.work_shape(axis).is_some(), "the axis is left with no shape");
    }
}
