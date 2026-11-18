//! QA acceptance and edge cases for joints, constraint status/DOF, Drive for relationships, Ground at origin,
//! Place at Component Origin, tool mounting and suppression of bodies and robots (assembly parity P1-9..11).
//! Inventor 2026 help:
//! - joints: To Define and Manage Joint Relationships (A30 in docs/ux/inventor-assembly-parity.md; the document
//!   lists only a truncated id for it)
//! - constraint status dots: GUID-A0573060-3778-4D6F-9431-28587D837CC7
//! - drive: GUID-267E6CE6-7B8D-4972-A20A-350477A447DF, GUID-B78E59C9-F9FB-4ED5-B60E-C9198770B1AF
//! - ground and root: GUID-98FA88B2-AA32-4A27-946A-DCECB36A6696; place at origin: GUID-E7A45820-E297-449D-AFBF-3C8D8EAE8A0E
//!
//! All under <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use std::path::Path;

use gc_core::{AnyId, Id};
use gc_doc::{
    AxisSense, Body, ComponentStatus, Geometry, Occurrence, Placement, RelationKind, RelationStatus,
    Relationship, Robot, Selection, Solid,
};
use gc_math::DVec3;
use gc_session::{CellItem, Session, library};

fn placed(xyz: [f64; 3]) -> Placement {
    Placement { xyz, rpy: [0.0; 3] }
}

fn sel(item: AnyId, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(item), geometry }
}

fn top(h: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, h / 2.0], normal: [0.0, 0.0, 1.0] }
}

fn bottom(h: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, -h / 2.0], normal: [0.0, 0.0, -1.0] }
}

fn ring(z: f64, down: bool) -> Geometry {
    Geometry::Circle { centre: [0.0, 0.0, z], normal: [0.0, 0.0, if down { -1.0 } else { 1.0 }] }
}

fn relation(kind: RelationKind, first: Selection, second: Selection) -> Relationship {
    Relationship::new("R", kind, first, second)
}

fn table_and_block() -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    // The first component is grounded.
    let table = s.add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, placed([0.0, 0.0, 0.5])).unwrap();
    let block = s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.3, 0.2, 1.6])).unwrap();
    (s, table, block)
}

fn pose_of(s: &Session, body: Id<Body>) -> Placement {
    s.doc().get(body).unwrap().placement
}

// ---- constraint status and degrees of freedom ----

/// Grounded: nothing left, black dot. Free: six, hollow dot. One Mate (face to face): two slides and a turn
/// about the normal remain. A Rigid joint: nothing left. A Rotational joint: one turn. Undo returns each.
#[test]
fn degrees_of_freedom_follow_the_relationships() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let dof = |s: &Session, id: AnyId| s.component_dof(id).unwrap();
    assert_eq!((dof(&s, t).total, s.component_status(t)), (0, Some(ComponentStatus::Full)), "grounded");
    assert_eq!((dof(&s, b).total, dof(&s, b).translation), (6, 3));
    assert_eq!(s.component_status(b), Some(ComponentStatus::Under));

    let mate = relation(
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        sel(b, bottom(0.2)),
        sel(t, top(1.0)),
    );
    let id = s.add_relationship(mate).unwrap();
    assert_eq!(
        (dof(&s, b).total, dof(&s, b).translation),
        (3, 2),
        "slides in the plane + a turn about its normal"
    );
    assert_eq!(s.component_status(b), Some(ComponentStatus::Under));
    assert_eq!(s.relationship_status(id), Some(RelationStatus::Under));

    s.delete_relationship(id).unwrap();
    let rigid = relation(
        RelationKind::Rigid { flip: false },
        sel(b, Geometry::Point([0.0; 3])),
        sel(t, Geometry::Point([0.0, 0.0, 0.5])),
    );
    let id = s.add_relationship(rigid).unwrap();
    assert_eq!(dof(&s, b).total, 0, "a rigid joint removes everything");
    assert_eq!(s.component_status(b), Some(ComponentStatus::Full));
    assert_eq!(s.relationship_status(id), Some(RelationStatus::Full));

    s.delete_relationship(id).unwrap();
    let hinge =
        relation(RelationKind::Rotational { flip: false }, sel(b, ring(0.0, true)), sel(t, ring(0.5, false)));
    s.add_relationship(hinge).unwrap();
    assert_eq!((dof(&s, b).total, dof(&s, b).rotation()), (1, 1), "a turn about Z remains");
}

/// Unknown items give no status; a deleted component too; Sick means "not satisfied where it stands".
#[test]
fn status_of_nothing_is_none_and_a_pinned_pair_is_sick_or_refused() {
    let (mut s, table, block) = table_and_block();
    let ghost = Body {
        name: "x".into(),
        placement: placed([0.0; 3]),
        solid: Solid::Box { size: [1.0; 3] },
        look: Default::default(),
    };
    drop(ghost);
    let gone = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert!(s.component_dof(gone).is_none() && s.component_status(gone).is_none());
    assert!(s.relationship_status(Id::from_raw(std::num::NonZeroU64::new(9999).unwrap())).is_none());
    // Both ends grounded, apart: a mate between them cannot be satisfied. It is refused (never stored sick).
    s.set_component(Occurrence::Item(block.erase()), gc_doc::ComponentFlag::Grounded, true).unwrap();
    let before = s.doc().clone();
    let result = s.add_relationship(relation(
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        sel(block.erase(), bottom(0.2)),
        sel(table.erase(), top(1.0)),
    ));
    assert!(result.is_err());
    assert!(s.doc().same_content(&before));
}

// ---- joints ----

/// Automatic picks Rotational for two circular origins and Rigid otherwise (help: Joint, Automatic type).
#[test]
fn automatic_joint_picks_the_type_from_the_origins() {
    let (s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    assert!(matches!(
        s.automatic_joint(&sel(b, ring(0.0, true)), &sel(t, ring(0.5, false))),
        RelationKind::Rotational { .. }
    ));
    for (first, second) in [
        (Geometry::Point([0.0; 3]), ring(0.5, false)),
        (ring(0.0, true), Geometry::Point([0.0; 3])),
        (top(0.2), top(1.0)),
        (Geometry::Point([0.0; 3]), Geometry::Point([0.0; 3])),
    ] {
        assert!(
            matches!(s.automatic_joint(&sel(b, first), &sel(t, second)), RelationKind::Rigid { .. }),
            "{first:?} {second:?}"
        );
    }
}

/// A rigid joint carries the follower along when the grounded one moves; a rotational joint lets it turn
/// about the joint axis only; both are one undo step and restore exactly.
#[test]
fn rigid_and_rotational_joints_hold_and_release_the_right_freedoms() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let depth = s.history().undo_depth();
    s.add_relationship(relation(
        RelationKind::Rigid { flip: false },
        sel(b, Geometry::Point([0.0; 3])),
        sel(t, Geometry::Point([0.0, 0.0, 0.5])),
    ))
    .unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let p = pose_of(&s, block);
    assert!(
        DVec3::from_array(p.xyz).distance(DVec3::new(0.0, 0.0, 1.0)) < 1e-7,
        "origin on the table-local point (world z 1.0): {p:?}"
    );
    let mut moved = s.doc().get(table).unwrap().clone();
    moved.placement.xyz = [1.0, 2.0, 0.5];
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: table, body: moved })).unwrap();
    let p = pose_of(&s, block);
    assert!(DVec3::from_array(p.xyz).distance(DVec3::new(1.0, 2.0, 1.0)) < 1e-7, "{p:?}");
    assert!(s.undo() && s.undo());
    assert_eq!(pose_of(&s, block).xyz, [0.3, 0.2, 1.6]);
}

/// Joints and constraints with nonsense input are refused with nothing changed.
#[test]
fn malformed_joints_are_refused() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let nan = Geometry::Point([f64::NAN, 0.0, 0.0]);
    for bad in [
        relation(RelationKind::Rigid { flip: false }, sel(b, nan), sel(t, Geometry::Point([0.0; 3]))),
        relation(
            RelationKind::Rigid { flip: false },
            sel(b, Geometry::Point([0.0; 3])),
            sel(b, Geometry::Point([0.0; 3])),
        ),
        relation(
            RelationKind::Rotational { flip: false },
            sel(b, ring(f64::INFINITY, true)),
            sel(t, ring(0.5, false)),
        ),
    ] {
        let result = s.add_relationship(bad.clone());
        assert!(result.is_err(), "{:?} accepted", bad.kind);
        assert!(s.doc().same_content(&before));
        assert_eq!(s.history().undo_depth(), depth);
    }
}

// ---- Drive for a relationship ----

/// Drive shows the motion without touching the document or the history; ending it shows the document
/// again; a Rigid joint, NaN and unreachable values are refused and the shown pose stays.
#[test]
fn driving_a_relationship_previews_without_editing() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let id = s
        .add_relationship(relation(
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            sel(b, bottom(0.2)),
            sel(t, top(1.0)),
        ))
        .unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(s.driven_placement(b).is_none(), "not driving yet");
    s.drive_relationship(id, 0.3).unwrap();
    let shown = s.driven_placement(b).unwrap();
    assert!((shown.xyz[2] - 1.4).abs() < 1e-7, "0.3 m above the table top: {shown:?}");
    assert!(s.doc().same_content(&before), "the document is untouched");
    assert_eq!(s.history().undo_depth(), depth);
    // Driving again moves the preview; end_drive restores the document's pose.
    s.drive_relationship(id, 0.05).unwrap();
    assert!((s.driven_placement(b).unwrap().xyz[2] - 1.15).abs() < 1e-7);
    // Back to the value the document has: nothing moves, so nothing is shown moved (the old preview is gone).
    s.drive_relationship(id, 0.0).unwrap();
    assert!(s.driven_placement(b).is_none(), "{:?}", s.driven_placement(b));
    s.drive_relationship(id, 0.05).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let shown_before = s.driven_placement(b);
        assert!(s.drive_relationship(id, bad).is_err(), "{bad}");
        assert_eq!(s.driven_placement(b), shown_before, "a refused value leaves the preview where it was");
    }
    s.end_drive_relationship();
    assert!(s.driven_placement(b).is_none());
    assert!(s.doc().same_content(&before));
    // Ending twice is harmless; driving a relationship that is gone is refused.
    s.end_drive_relationship();
    s.delete_relationship(id).unwrap();
    assert!(s.drive_relationship(id, 0.1).is_err());
    // A rigid joint has nothing to drive.
    let rigid = s
        .add_relationship(relation(
            RelationKind::Rigid { flip: false },
            sel(b, Geometry::Point([0.0; 3])),
            sel(t, Geometry::Point([0.0, 0.0, 0.5])),
        ))
        .unwrap();
    assert!(s.drive_relationship(rigid, 0.1).is_err());
}

/// A rotational joint driven through a full turn comes back to where it began; a half turn flips it.
#[test]
fn driving_a_rotational_joint_by_turns() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let id = s
        .add_relationship(relation(
            RelationKind::Rotational { flip: false },
            sel(b, ring(0.0, true)),
            sel(t, ring(0.5, false)),
        ))
        .unwrap();
    let start = pose_of(&s, block);
    s.drive_relationship(id, std::f64::consts::TAU).unwrap();
    let full = s.driven_placement(b).unwrap();
    assert!(
        DVec3::from_array(full.xyz).distance(DVec3::from_array(start.xyz)) < 1e-6,
        "position on the axis"
    );
    s.drive_relationship(id, std::f64::consts::PI).unwrap();
    let half = s.driven_placement(b).unwrap();
    assert!(DVec3::from_array(half.xyz).distance(DVec3::from_array(start.xyz)) < 1e-6, "still on the axis");
    s.end_drive_relationship();
}

// ---- Ground and Root, Place at Component Origin ----

#[test]
fn ground_at_origin_moves_aligns_and_grounds() {
    let (mut s, table, block) = table_and_block();
    let b = block.erase();
    let depth = s.history().undo_depth();
    s.ground_at_origin(&[b]).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let p = pose_of(&s, block);
    assert_eq!((p.xyz, p.rpy), ([0.0; 3], [0.0; 3]));
    assert!(s.components().iter().find(|n| n.occurrence == Occurrence::Item(b)).unwrap().grounded);
    // Again: harmless. Nothing, or something that is not a component: refused with the document untouched.
    s.ground_at_origin(&[b]).unwrap();
    let before = s.doc().clone();
    assert!(s.ground_at_origin(&[]).is_err());
    let gone = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert!(s.ground_at_origin(&[b, gone]).is_err());
    assert!(s.doc().same_content(&before), "one bad item spoils the whole request");
    let _ = table;
}

#[test]
fn place_at_component_origin_puts_one_component_on_anothers_origin() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let depth = s.history().undo_depth();
    s.place_at_component_origin(b, Occurrence::Item(t)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one undo step");
    let p = pose_of(&s, block);
    assert!(DVec3::from_array(p.xyz).distance(DVec3::new(0.0, 0.0, 0.5)) < 1e-7, "{p:?}");
    assert_eq!(s.relationships().len(), 3, "three flush constraints, XY, XZ and YZ");
    assert_eq!(s.component_dof(b).unwrap().total, 0, "fully constrained there");
    // A component on its own origin, a missing target: refused, nothing changes.
    let before = s.doc().clone();
    assert!(s.place_at_component_origin(b, Occurrence::Item(b)).is_err());
    let gone = Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase();
    assert!(s.place_at_component_origin(b, Occurrence::Item(gone)).is_err());
    assert!(s.doc().same_content(&before));
    assert!(s.undo());
    assert_eq!(s.relationships().len(), 0);
    assert_eq!(pose_of(&s, block).xyz, [0.3, 0.2, 1.6]);
}

// ---- suppression of bodies and robots ----

/// A suppressed body is neither measured against nor part of interference; showing it again brings it back
/// (help: suppressed components are not computed).
#[test]
fn a_suppressed_body_drops_out_of_interference_and_measure() {
    let mut s = Session::new();
    let a = CellItem::Body(s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.0; 3])).unwrap());
    let CellItem::Body(bid) =
        CellItem::Body(s.add_body(Solid::Box { size: [0.2; 3] }, placed([0.1, 0.0, 0.0])).unwrap())
    else {
        panic!()
    };
    let b = CellItem::Body(bid);
    assert_eq!(s.interference(&[a, b], &[]).len(), 1);
    let depth = s.history().undo_depth();
    s.set_component(Occurrence::Item(bid.erase()), gc_doc::ComponentFlag::Suppressed, true).unwrap();
    assert!(s.doc().is_component_suppressed(bid.erase()));
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.interference(&[a, b], &[]).is_empty(), "a suppressed body does not interfere");
    assert!(s.undo());
    assert!(!s.doc().is_component_suppressed(bid.erase()));
    assert_eq!(s.interference(&[a, b], &[]).len(), 1);
}

/// A suppressed robot or body is not an obstacle: the robot's pose that hit a pillar is clear when the
/// pillar is suppressed.
#[test]
fn a_suppressed_obstacle_no_longer_blocks_the_robot() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    let robot: Id<Robot> =
        s.add_library_robot(packages.iter().find(|p| p.info.name == "UR5e").unwrap()).unwrap();
    s.jog(robot, &[0.0, -1.9, 1.7, -1.37, -1.57, 0.0]);
    s.end_jog();
    // A pillar through the robot's own base.
    let pillar = s.add_body(Solid::Box { size: [0.3, 0.3, 0.6] }, placed([0.0, 0.0, 0.3])).unwrap();
    assert!(s.robot_problem(robot).is_some(), "the pillar collides with the robot");
    s.set_component(Occurrence::Item(pillar.erase()), gc_doc::ComponentFlag::Suppressed, true).unwrap();
    assert!(s.robot_problem(robot).is_none(), "{:?}", s.robot_problem(robot));
    assert!(s.undo());
    assert!(s.robot_problem(robot).is_some());
}

/// A robot's links are components of their own (a robot link has an origin like any part).
#[test]
fn a_robots_links_are_listed_as_components() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    let robot = s.add_library_robot(packages.iter().find(|p| p.info.name == "UR5e").unwrap()).unwrap();
    let links = s.robot_links(robot);
    assert!(links.len() >= 7, "base link and six more: {}", links.len());
    let mut names: Vec<_> = links.iter().map(|n| n.name.clone()).collect();
    names.sort();
    let unique = names.len();
    names.dedup();
    assert_eq!(names.len(), unique, "link names are unique in the tree");
    assert!(links.iter().all(|n| s.component_origin(n.occurrence).is_some()), "every link has an origin");
    let ghost = Id::<Robot>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(s.robot_links(ghost).is_empty());
}

// ---- mounting a tool ----

#[test]
fn a_tool_is_mounted_on_the_flange_or_the_mount_is_refused() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots");
    let packages = library::scan(&dir);
    let mut s = Session::new();
    s.set_robot_library(Some(dir));
    let robot = s.add_library_robot(packages.iter().find(|p| p.info.name == "UR5e").unwrap()).unwrap();
    let origin = Geometry::Circle { centre: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let before = s.doc().clone();
    // No tool yet: nothing to mount.
    assert!(s.mount_tool(robot, origin, false).is_err());
    assert!(s.doc().same_content(&before));
    // With a tool the mount sets its placement; flipping turns it over (Z axes the same way).
    let tool = gc_doc::Tool {
        solid: Solid::Cylinder { radius: 0.02, length: 0.2 },
        placement: Placement::default(),
    };
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetTool { id: robot, tool: Some(tool) })).unwrap();
    s.mount_tool(robot, origin, false).unwrap();
    let plain = s.doc().get(robot).unwrap().tool.as_deref().unwrap().placement;
    s.mount_tool(robot, origin, true).unwrap();
    let flipped = s.doc().get(robot).unwrap().tool.as_deref().unwrap().placement;
    assert_ne!(plain, flipped, "Flip component changes the mount");
    // A mount origin that is not finite is refused.
    let nan = Geometry::Circle { centre: [f64::NAN, 0.0, 0.0], normal: [0.0, 0.0, 1.0] };
    let tool_before = s.doc().get(robot).unwrap().tool.clone();
    assert!(s.mount_tool(robot, nan, false).is_err());
    assert_eq!(s.doc().get(robot).unwrap().tool, tool_before);
    // Undo walks back: flipped -> plain -> before.
    assert!(s.undo());
    assert_eq!(s.doc().get(robot).unwrap().tool.as_deref().unwrap().placement, plain);
}

/// Lock and Protect (Inventor 2026 help, To Define and Manage Joint Relationships,
/// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6): Lock removes all motion but the component still follows
/// what it is joined to; Protect refuses relationships that take the joint's freedom away; neither is
/// for constraints.
#[test]
fn lock_and_protect_a_joint() {
    let (mut s, table, block) = table_and_block();
    let (t, b) = (table.erase(), block.erase());
    let joint = s
        .add_relationship(relation(
            RelationKind::Rotational { flip: false },
            sel(b, ring(-0.1, true)),
            sel(t, ring(0.5, false)),
        ))
        .unwrap();
    assert_eq!(s.component_dof(b).unwrap().total, 1, "the turn");
    s.set_joint_lock(joint, true).unwrap();
    assert!(s.joint_condition(joint).locked.is_some());
    assert_eq!(s.component_dof(b).unwrap().total, 0, "locked: no motion");
    // The table moves: the locked block goes with it.
    let mut moved = s.doc().get(table).unwrap().clone();
    moved.placement.xyz = [1.0, 0.0, 0.5];
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::SetBody { id: table, body: moved })).unwrap();
    assert!((pose_of(&s, block).xyz[0] - 1.0).abs() < 1e-7);
    s.set_joint_lock(joint, false).unwrap();
    assert_eq!(s.component_dof(b).unwrap().total, 1, "free to turn again");

    // Protected: an angle that would stop the turn is refused.
    s.set_joint_protect(joint, true).unwrap();
    // The block's +X face at a right angle to the table's +Y face (as they stand): it stops the turn.
    let angle = relation(
        RelationKind::Angle {
            angle: std::f64::consts::FRAC_PI_2,
            solution: gc_doc::AngleSolution::Undirected,
        },
        sel(b, Geometry::Plane { point: [0.1, 0.0, 0.0], normal: [1.0, 0.0, 0.0] }),
        sel(t, Geometry::Plane { point: [0.0, 0.5, 0.0], normal: [0.0, 1.0, 0.0] }),
    );
    let result = s.add_relationship(angle.clone());
    assert!(
        matches!(result, Err(gc_doc::EditError::Protected(_))),
        "{result:?} dof {:?}",
        s.component_dof(b)
    );
    s.set_joint_protect(joint, false).unwrap();
    s.add_relationship(angle).unwrap();

    // A constraint has neither.
    let mate = s.relationships().into_iter().find(|(id, _)| *id != joint).map(|(id, _)| id).unwrap();
    assert!(s.set_joint_lock(mate, true).is_err());
}
