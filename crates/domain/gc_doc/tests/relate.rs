//! Assembly constraints (docs/ux/inventor-assembly-parity.md, P1-9): Mate, Flush and Insert move the
//! components that are not grounded; the grounded one stays; one undo step each.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, CellOp, Command, ComponentFlag, Created, Document, EditError, Geometry, History,
    Occurrence, OriginElement, OriginPlane, Placement, RelationKind, Relationship, Selection, Solid, apply,
};
use gc_math::DVec3;

struct Bench {
    doc: Document,
    history: History,
}

impl Bench {
    fn new() -> Self {
        Self { doc: Document::new(), history: History::default() }
    }

    fn cell(&mut self, op: CellOp) -> Result<Vec<Created>, EditError> {
        let applied = apply(&mut self.doc, &Command::Cell(op))?;
        self.history.record(applied.label, applied.delta);
        Ok(applied.created)
    }

    fn body(&mut self, name: &str, size: [f64; 3], placement: Placement) -> Id<Body> {
        let body =
            Body { name: name.into(), placement, solid: Solid::Box { size }, look: Default::default() };
        let Created::Body(id) = self.cell(CellOp::AddBody(body)).unwrap()[0] else { panic!() };
        id
    }

    fn relate(&mut self, relationship: Relationship) -> Result<Id<Relationship>, EditError> {
        let created = self.cell(CellOp::AddRelationship(relationship))?;
        let Created::Relationship(id) = created[0] else { panic!() };
        Ok(id)
    }

    fn pose(&self, body: Id<Body>) -> gc_math::DPose3 {
        let p = self.doc.get(body).unwrap().placement;
        gc_math::pose_from_xyz_rpy(p.xyz, p.rpy)
    }
}

fn on(body: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

fn top(height: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, height / 2.0], normal: [0.0, 0.0, 1.0] }
}

fn bottom(height: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, -height / 2.0], normal: [0.0, 0.0, -1.0] }
}

fn close(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-7
}

/// A table (first, so grounded) 1 m high, and a box lying tilted somewhere above it.
fn table_and_box(bench: &mut Bench) -> (Id<Body>, Id<Body>) {
    let table = bench.body("Table", [2.0, 1.0, 1.0], Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] });
    let tilted = Placement { xyz: [0.3, 0.2, 1.4], rpy: [0.4, -0.3, 0.2] };
    let block = bench.body("Block", [0.2, 0.2, 0.2], tilted);
    (table, block)
}

#[test]
fn mate_puts_the_box_face_to_face_on_the_table_and_flush_flips_it() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    assert!(bench.doc.component_flag(Occurrence::Item(table.erase()), ComponentFlag::Grounded));
    let table_pose = bench.pose(table);
    let depth = bench.history.undo_depth();
    let mate = Relationship {
        limits: None,
        name: "Mate:1".into(),
        kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        first: on(block, bottom(0.2)),
        second: on(table, top(1.0)),
    };
    let id = bench.relate(mate.clone()).unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1, "one undo step");
    // The table stays; the box stands on it, upright, its bottom face at the table top (z = 1).
    assert_eq!(bench.pose(table), table_pose);
    let pose = bench.pose(block);
    assert!(close(pose.rotation * DVec3::Z, DVec3::Z), "upright: {:?}", pose.rotation * DVec3::Z);
    assert!((pose.translation.z - 1.1).abs() < 1e-7, "{}", pose.translation.z);
    // Only what the mate needs moved: the box is still about where it was across the table.
    assert!((pose.translation.x - 0.3).abs() < 0.1 && (pose.translation.y - 0.2).abs() < 0.1);

    // An offset lifts it; Flush turns it upside down beside the face.
    let lifted =
        Relationship { kind: RelationKind::Mate { offset: 0.05, sense: AxisSense::Opposed }, ..mate.clone() };
    bench.cell(CellOp::SetRelationship { id, relationship: lifted }).unwrap();
    assert!((bench.pose(block).translation.z - 1.15).abs() < 1e-7);
    let flush = Relationship { kind: RelationKind::Flush { offset: 0.0 }, ..mate };
    bench.cell(CellOp::SetRelationship { id, relationship: flush }).unwrap();
    let pose = bench.pose(block);
    assert!(close(pose.rotation * DVec3::Z, -DVec3::Z), "flipped");
    assert!(
        (pose.translation.z - 0.9).abs() < 1e-7,
        "its bottom face (now facing up) at z = 1: {}",
        pose.translation.z
    );

    // Undo all the way: the box is back where it was and the constraint is gone.
    for _ in 0..3 {
        bench.history.undo(&mut bench.doc).unwrap();
    }
    assert!(bench.doc.relationships().is_empty());
    assert_eq!(bench.doc.get(block).unwrap().placement.xyz, [0.3, 0.2, 1.4]);
}

#[test]
fn a_constrained_box_follows_the_grounded_table_and_returns_when_dragged_off() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    bench
        .relate(Relationship {
            limits: None,
            name: "Mate:1".into(),
            kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            first: on(block, bottom(0.2)),
            second: on(table, top(1.0)),
        })
        .unwrap();
    // The table moves up 0.5 m: the box goes with it.
    let mut moved = bench.doc.get(table).unwrap().clone();
    moved.placement.xyz = [0.0, 0.0, 1.0];
    bench.cell(CellOp::SetBody { id: table, body: moved }).unwrap();
    assert!((bench.pose(block).translation.z - 1.6).abs() < 1e-7);
    // Dragged up off the table, the box comes back down (Inventor: Update returns it), but a slide
    // along the table top stays.
    let mut lifted = bench.doc.get(block).unwrap().clone();
    lifted.placement.xyz = [0.5, -0.1, 2.0];
    bench.cell(CellOp::SetBody { id: block, body: lifted }).unwrap();
    let pose = bench.pose(block);
    assert!((pose.translation.z - 1.6).abs() < 1e-7);
    assert!((pose.translation.x - 0.5).abs() < 1e-6 && (pose.translation.y + 0.1).abs() < 1e-6);
}

#[test]
fn insert_puts_a_pin_in_a_hole_and_leaves_its_turn_free() {
    let mut bench = Bench::new();
    let plate = bench.body("Plate", [0.4, 0.4, 0.02], Placement::default());
    let pin = bench.body("Pin", [0.02, 0.02, 0.1], Placement { xyz: [0.5, 0.5, 0.5], rpy: [1.0, 0.5, 0.0] });
    // The hole's rim on the top face at (0.1, 0.1); the pin's head underside at its local z = 0.03.
    let hole = Geometry::Circle { centre: [0.1, 0.1, 0.01], normal: [0.0, 0.0, 1.0] };
    let head = Geometry::Circle { centre: [0.0, 0.0, 0.03], normal: [0.0, 0.0, -1.0] };
    bench
        .relate(Relationship {
            limits: None,
            name: "Insert:1".into(),
            kind: RelationKind::Insert { offset: 0.0, aligned: false },
            first: on(pin, head),
            second: on(plate, hole),
        })
        .unwrap();
    let pose = bench.pose(pin);
    // The pin's axis is the hole's axis, pointing down into it, the head on the face.
    assert!(close(pose.rotation * DVec3::Z, DVec3::Z));
    let head_world = pose.translation + pose.rotation * DVec3::new(0.0, 0.0, 0.03);
    assert!(close(head_world, DVec3::new(0.1, 0.1, 0.01)), "{head_world:?}");
}

#[test]
fn origin_planes_can_be_mated_and_impossible_constraints_are_refused() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let xy = Geometry::origin(OriginElement::Plane(OriginPlane::Xy));
    // Flush the box's XY plane with the table's at 0.3 m: the box's origin 0.3 m above the table's.
    bench
        .relate(Relationship {
            limits: None,
            name: "Flush:1".into(),
            kind: RelationKind::Flush { offset: 0.3 },
            first: on(block, xy),
            second: on(table, xy),
        })
        .unwrap();
    let pose = bench.pose(block);
    assert!((pose.translation.z - 0.8).abs() < 1e-7 && close(pose.rotation * DVec3::Z, DVec3::Z));

    // Ground the box too: a second constraint that wants it elsewhere cannot be met; nothing changes.
    let grounded = Command::SetComponent {
        occurrence: Occurrence::Item(block.erase()),
        flag: ComponentFlag::Grounded,
        on: true,
    };
    apply(&mut bench.doc, &grounded).unwrap();
    let before = bench.doc.clone();
    let result = bench.relate(Relationship {
        limits: None,
        name: "Mate:2".into(),
        kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        first: on(block, bottom(0.2)),
        second: on(table, top(1.0)),
    });
    let Err(error @ EditError::Unsolvable { .. }) = result else { panic!("{result:?}") };
    let message = error.to_string();
    assert!(message.contains("Flush:1") || message.contains("Mate:2"), "names the constraints: {message}");
    assert_eq!(bench.doc.relationships(), before.relationships());
    assert_eq!(bench.doc.bodies(), before.bodies());

    // Flush needs two planes, Insert two circles; a constraint on one component with itself is refused.
    let bad = Relationship {
        limits: None,
        name: "Bad".into(),
        kind: RelationKind::Flush { offset: 0.0 },
        first: on(block, Geometry::Point([0.0; 3])),
        second: on(table, xy),
    };
    assert!(matches!(bench.relate(bad.clone()), Err(EditError::BadValue)));
    let itself = Relationship {
        first: on(table, xy),
        kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        ..bad
    };
    assert!(matches!(bench.relate(itself), Err(EditError::BadValue)));
}

#[test]
fn predict_offset_and_orientation_reads_how_the_faces_lie() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let mut level = bench.doc.get(block).unwrap().clone();
    level.placement = Placement { xyz: [0.3, 0.2, 1.25], rpy: [0.0; 3] };
    bench.cell(CellOp::SetBody { id: block, body: level }).unwrap();
    // The box's bottom faces the table top 0.15 m above it: Mate at 0.15.
    let predicted = bench.doc.predict_relationship(&on(block, bottom(0.2)), &on(table, top(1.0))).unwrap();
    let RelationKind::Mate { offset, .. } = predicted else { panic!("{predicted:?}") };
    assert!((offset - 0.15).abs() < 1e-12);
    // Its top looks the same way as the table top: Flush.
    let predicted = bench.doc.predict_relationship(&on(block, top(0.2)), &on(table, top(1.0))).unwrap();
    assert!(matches!(predicted, RelationKind::Flush { offset } if (offset - 0.35).abs() < 1e-12));
}

#[test]
fn a_constrained_component_cannot_be_deleted_while_constrained() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    bench
        .relate(Relationship {
            limits: None,
            name: "Mate:1".into(),
            kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            first: on(block, bottom(0.2)),
            second: on(table, top(1.0)),
        })
        .unwrap();
    // Deleting a constrained component takes nothing with it silently: it is in use.
    assert!(bench.cell(CellOp::DeleteBody(block)).is_err());
}

fn table_frame() -> Geometry {
    // On the table top (local z = 0.5), 0.5 m along X, Z up and X along the table.
    Geometry::Frame { origin: [0.5, 0.0, 0.5], z: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0] }
}

#[test]
fn a_rigid_joint_mounts_a_base_on_the_table_and_it_follows_the_table() {
    let mut bench = Bench::new();
    let (table, base) = table_and_box(&mut bench);
    // The base's own origin on the table frame, Z the same way (flip): standing upright on it.
    bench
        .relate(Relationship {
            limits: None,
            name: "Rigid:1".into(),
            kind: RelationKind::Rigid { flip: true },
            first: on(base, Geometry::origin_frame()),
            second: on(table, table_frame()),
        })
        .unwrap();
    let pose = bench.pose(base);
    assert!(close(pose.translation, DVec3::new(0.5, 0.0, 1.0)), "{:?}", pose.translation);
    assert!(close(pose.rotation * DVec3::Z, DVec3::Z) && close(pose.rotation * DVec3::X, DVec3::X));
    // The table moves and turns: the base goes with it, still on the same spot of the table.
    let mut moved = bench.doc.get(table).unwrap().clone();
    moved.placement = Placement { xyz: [1.0, 2.0, 0.5], rpy: [0.0, 0.0, std::f64::consts::FRAC_PI_2] };
    bench.cell(CellOp::SetBody { id: table, body: moved }).unwrap();
    let pose = bench.pose(base);
    assert!(close(pose.translation, DVec3::new(1.0, 2.5, 1.0)), "{:?}", pose.translation);
    assert!(close(pose.rotation * DVec3::X, DVec3::Y));
    // A rigid joint leaves nothing free: dragging the base anywhere brings it back.
    let mut dragged = bench.doc.get(base).unwrap().clone();
    dragged.placement = Placement { xyz: [3.0, 0.0, 0.0], rpy: [0.0, 0.0, 1.0] };
    bench.cell(CellOp::SetBody { id: base, body: dragged }).unwrap();
    assert!(close(bench.pose(base).translation, DVec3::new(1.0, 2.5, 1.0)));

    // Without flip the Z axes are opposed: the base hangs upside down from the spot.
    let id = bench.doc.relationships().iter().next().unwrap().0;
    let relationship = Relationship {
        kind: RelationKind::Rigid { flip: false },
        ..bench.doc.relationships().get(id).unwrap().clone()
    };
    bench.cell(CellOp::SetRelationship { id, relationship }).unwrap();
    assert!(close(bench.pose(base).rotation * DVec3::Z, -DVec3::Z));
}

#[test]
fn a_rotational_joint_leaves_the_turn_about_its_axis_free() {
    let mut bench = Bench::new();
    let (table, base) = table_and_box(&mut bench);
    bench
        .relate(Relationship {
            limits: None,
            name: "Rotational:1".into(),
            kind: RelationKind::Rotational { flip: true },
            first: on(base, Geometry::origin_frame()),
            second: on(table, table_frame()),
        })
        .unwrap();
    assert!(close(bench.pose(base).translation, DVec3::new(0.5, 0.0, 1.0)));
    // Turned about Z and tipped over: the turn stays, the tip does not.
    let mut turned = bench.doc.get(base).unwrap().clone();
    turned.placement = Placement { xyz: [0.5, 0.0, 1.0], rpy: [0.3, 0.0, 0.7] };
    bench.cell(CellOp::SetBody { id: base, body: turned }).unwrap();
    let pose = bench.pose(base);
    assert!(close(pose.rotation * DVec3::Z, DVec3::Z), "upright again");
    let x = pose.rotation * DVec3::X;
    assert!(
        (gc_math::fm::atan2(x.y, x.x) - 0.7).abs() < 0.05,
        "the turn is kept: {}",
        gc_math::fm::atan2(x.y, x.x)
    );
    // Automatic picks Rotational when both origins are circular, Rigid otherwise.
    let circle = on(base, Geometry::Circle { centre: [0.0; 3], normal: [0.0, 0.0, 1.0] });
    let hole = on(table, Geometry::Circle { centre: [0.5, 0.0, 0.5], normal: [0.0, 0.0, 1.0] });
    assert_eq!(Document::automatic_joint(&circle, &hole), RelationKind::Rotational { flip: false });
    assert_eq!(
        Document::automatic_joint(&circle, &on(table, table_frame())),
        RelationKind::Rigid { flip: false }
    );
    assert_eq!(
        Document::automatic_joint(&on(base, Geometry::origin_frame()), &on(table, table_frame())),
        RelationKind::Rigid { flip: false }
    );
}

#[test]
fn a_tool_is_mounted_by_its_joint_origin_on_the_flange() {
    // The tool's mounting face: 5 cm below its own origin, facing down (out of the tool, towards the flange).
    let face = Geometry::Frame { origin: [0.0, 0.0, -0.05], z: [0.0, 0.0, -1.0], x: [1.0, 0.0, 0.0] };
    let placement = face.rigid_mount(false);
    // The face's centre lands on the flange origin and faces against the flange's Z: the tool sits
    // 5 cm out along the flange's Z, upright.
    assert!(close(placement.translation, DVec3::new(0.0, 0.0, 0.05)), "{:?}", placement.translation);
    assert!(close(placement.rotation * DVec3::Z, DVec3::Z));
    let flipped = face.rigid_mount(true);
    assert!(close(flipped.rotation * DVec3::Z, -DVec3::Z));
    assert!(close(flipped.translation + flipped.rotation * DVec3::new(0.0, 0.0, -0.05), DVec3::ZERO));
}

#[test]
fn remaining_freedom_and_status_points_follow_the_relationships() {
    use gc_doc::{Dof, RelationStatus};
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let item = |b: Id<Body>| b.erase();
    assert_eq!(bench.doc.component_dof(item(table)), Some(Dof { total: 0, translation: 0 }), "grounded");
    assert_eq!(bench.doc.component_dof(item(block)), Some(Dof { total: 6, translation: 3 }), "free");
    // A face mate leaves sliding on the table (2) and turning about its normal (1).
    let mate = bench
        .relate(Relationship {
            limits: None,
            name: "Mate:1".into(),
            kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            first: on(block, bottom(0.2)),
            second: on(table, top(1.0)),
        })
        .unwrap();
    let dof = bench.doc.component_dof(item(block)).unwrap();
    assert_eq!((dof.total, dof.translation, dof.rotation()), (3, 2, 1));
    assert_eq!(bench.doc.relationship_status(mate), Some(RelationStatus::Under));
    // Turned into a rotational joint, one turn is left; as a rigid joint nothing is, and the point is full.
    let rotational = Relationship {
        limits: None,
        name: "Joint".into(),
        kind: RelationKind::Rotational { flip: true },
        first: on(block, Geometry::origin_frame()),
        second: on(table, table_frame()),
    };
    bench.cell(CellOp::SetRelationship { id: mate, relationship: rotational.clone() }).unwrap();
    let dof = bench.doc.component_dof(item(block)).unwrap();
    assert_eq!((dof.total, dof.translation), (1, 0));
    let rigid = Relationship { kind: RelationKind::Rigid { flip: true }, ..rotational };
    bench.cell(CellOp::SetRelationship { id: mate, relationship: rigid }).unwrap();
    assert_eq!(bench.doc.component_dof(item(block)).unwrap().total, 0);
    assert_eq!(bench.doc.relationship_status(mate), Some(RelationStatus::Full));
    use gc_doc::ComponentStatus;
    assert_eq!(bench.doc.component_status(item(block)), Some(ComponentStatus::Full));
    assert_eq!(bench.doc.component_status(item(table)), Some(ComponentStatus::Full), "grounded");
    bench.history.undo(&mut bench.doc).unwrap();
    assert_eq!(bench.doc.component_status(item(block)), Some(ComponentStatus::Under));
}

#[test]
fn ground_at_origin_moves_aligns_and_grounds() {
    let mut bench = Bench::new();
    let (_, block) = table_and_box(&mut bench);
    bench.cell(CellOp::GroundAtOrigin(vec![block.erase()])).unwrap();
    assert_eq!(bench.doc.get(block).unwrap().placement, Placement::default());
    assert!(bench.doc.component_flag(Occurrence::Item(block.erase()), ComponentFlag::Grounded));
    // One undo step puts it back, free.
    bench.history.undo(&mut bench.doc).unwrap();
    assert_eq!(bench.doc.get(block).unwrap().placement.xyz, [0.3, 0.2, 1.4]);
    assert!(!bench.doc.component_flag(Occurrence::Item(block.erase()), ComponentFlag::Grounded));
}

#[test]
fn place_at_component_origin_aligns_and_constrains_to_the_other_origin() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let created = bench
        .cell(CellOp::PlaceAtComponentOrigin { item: block.erase(), target: Occurrence::Item(table.erase()) })
        .unwrap();
    assert_eq!(created.len(), 3, "three flush constraints");
    assert!(close(bench.pose(block).translation, bench.pose(table).translation));
    assert_eq!(bench.doc.component_dof(block.erase()).unwrap().total, 0, "fully constrained");
    // It follows the table.
    let mut moved = bench.doc.get(table).unwrap().clone();
    moved.placement.xyz = [1.0, 1.0, 0.5];
    bench.cell(CellOp::SetBody { id: table, body: moved }).unwrap();
    assert!(close(bench.pose(block).translation, DVec3::new(1.0, 1.0, 0.5)));
}

fn xz() -> Geometry {
    Geometry::origin(OriginElement::Plane(OriginPlane::Xz))
}

fn yaw(bench: &Bench, body: Id<Body>) -> f64 {
    let x = bench.pose(body).rotation * DVec3::X;
    gc_math::fm::atan2(x.y, x.x)
}

#[test]
fn angle_constraints_turn_by_the_angle_with_the_chosen_sense() {
    use gc_doc::AngleSolution;
    let deg30 = 30f64.to_radians();
    for (solution, expected) in [
        (AngleSolution::Directed, None),
        (AngleSolution::Undirected, None),
        // Measured from the box's normal to the table's about the table's +Z: the box turns by −30°.
        (
            AngleSolution::Explicit(Selection {
                occurrence: Occurrence::Item(
                    Id::<Body>::from_raw(std::num::NonZeroU64::new(5).unwrap()).erase(),
                ),
                geometry: Geometry::origin(OriginElement::Plane(OriginPlane::Xy)),
            }),
            Some(-deg30),
        ),
    ] {
        let mut bench = Bench::new();
        let (table, block) = table_and_box(&mut bench);
        assert_eq!(table.erase().raw.get(), 5, "the explicit reference above names the table");
        bench
            .relate(Relationship {
                limits: None,
                name: "Mate:1".into(),
                kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
                first: on(block, bottom(0.2)),
                second: on(table, top(1.0)),
            })
            .unwrap();
        bench
            .relate(Relationship {
                limits: None,
                name: "Angle:1".into(),
                kind: RelationKind::Angle { angle: deg30, solution },
                first: on(block, xz()),
                second: on(table, xz()),
            })
            .unwrap();
        let turned = yaw(&bench, block);
        assert!((turned.abs() - deg30).abs() < 1e-6, "{solution:?}: {}", turned.to_degrees());
        if let Some(expected) = expected {
            assert!((turned - expected).abs() < 1e-6, "{solution:?}: {}", turned.to_degrees());
        }
        // Mate (3 left) and an angle: sliding on the table is all that is left.
        assert_eq!(bench.doc.component_dof(block.erase()).unwrap(), gc_doc::Dof { total: 2, translation: 2 });
    }
}

#[test]
fn tangent_puts_a_ball_and_a_roller_on_the_table_inside_or_outside() {
    let mut bench = Bench::new();
    let table = bench.body("Table", [2.0, 1.0, 1.0], Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] });
    let ball = bench.body("Ball", [0.2; 3], Placement { xyz: [0.2, 0.1, 2.0], rpy: [0.0; 3] });
    let roller = bench.body("Roller", [0.2; 3], Placement { xyz: [-0.3, 0.0, 1.7], rpy: [0.3, 0.2, 0.0] });
    let sphere = Geometry::Sphere { centre: [0.0; 3], radius: 0.1 };
    bench
        .relate(Relationship {
            limits: None,
            name: "Tangent:1".into(),
            kind: RelationKind::Tangent { offset: 0.0, inside: false },
            first: on(ball, sphere),
            second: on(table, top(1.0)),
        })
        .unwrap();
    assert!((bench.pose(ball).translation.z - 1.1).abs() < 1e-7, "resting on the top");
    let cylinder = Geometry::Cylinder { point: [0.0; 3], direction: [1.0, 0.0, 0.0], radius: 0.05 };
    bench
        .relate(Relationship {
            limits: None,
            name: "Tangent:2".into(),
            kind: RelationKind::Tangent { offset: 0.0, inside: false },
            first: on(roller, cylinder),
            second: on(table, top(1.0)),
        })
        .unwrap();
    let pose = bench.pose(roller);
    assert!((pose.translation.z - 1.05).abs() < 1e-7);
    assert!((pose.rotation * DVec3::X).z.abs() < 1e-7, "its axis lies flat");
    // Inside: the ball under the table top plane.
    let id = bench.doc.relationships().iter().next().unwrap().0;
    let inside = Relationship {
        limits: None,
        name: "Tangent:1".into(),
        kind: RelationKind::Tangent { offset: 0.0, inside: true },
        first: on(ball, sphere),
        second: on(table, top(1.0)),
    };
    bench.cell(CellOp::SetRelationship { id, relationship: inside }).unwrap();
    assert!((bench.pose(ball).translation.z - 0.9).abs() < 1e-7);
}

#[test]
fn symmetry_mirrors_one_component_across_a_plane() {
    let mut bench = Bench::new();
    let table = bench.body("Table", [2.0, 1.0, 1.0], Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] });
    let left = bench.body("Left", [0.1; 3], Placement { xyz: [-0.4, 0.2, 1.2], rpy: [0.0; 3] });
    let right = bench.body("Right", [0.1; 3], Placement { xyz: [0.7, -0.1, 1.0], rpy: [0.0; 3] });
    let ground = Command::SetComponent {
        occurrence: Occurrence::Item(left.erase()),
        flag: ComponentFlag::Grounded,
        on: true,
    };
    apply(&mut bench.doc, &ground).unwrap();
    bench
        .relate(Relationship {
            limits: None,
            name: "Symmetry:1".into(),
            kind: RelationKind::Symmetry {
                plane: on(table, Geometry::origin(OriginElement::Plane(OriginPlane::Yz))),
            },
            first: on(left, Geometry::Point([0.0; 3])),
            second: on(right, Geometry::Point([0.0; 3])),
        })
        .unwrap();
    assert!(
        close(bench.pose(right).translation, DVec3::new(0.4, 0.2, 1.2)),
        "{:?}",
        bench.pose(right).translation
    );
    // The plane's component cannot be deleted while it is used.
    assert!(bench.cell(CellOp::DeleteBody(table)).is_err());
}

#[test]
fn slider_cylindrical_and_ball_joints_leave_their_freedoms() {
    use gc_doc::Dof;
    for (kind, dof) in [
        (RelationKind::Slider { flip: true }, Dof { total: 1, translation: 1 }),
        (RelationKind::Cylindrical { flip: true }, Dof { total: 2, translation: 1 }),
        (RelationKind::Ball, Dof { total: 3, translation: 0 }),
    ] {
        let mut bench = Bench::new();
        let (table, block) = table_and_box(&mut bench);
        bench
            .relate(Relationship {
                limits: None,
                name: "Joint".into(),
                kind,
                first: on(block, Geometry::origin_frame()),
                second: on(table, table_frame()),
            })
            .unwrap();
        assert_eq!(bench.doc.component_dof(block.erase()).unwrap(), dof, "{kind:?}");
        let (pose, frame) = (bench.pose(block), DVec3::new(0.5, 0.0, 1.0));
        match kind {
            RelationKind::Ball => assert!(close(pose.translation, frame)),
            _ => {
                // On the vertical line through the joint origin, upright.
                assert!((pose.translation.x - 0.5).abs() < 1e-7 && pose.translation.y.abs() < 1e-7);
                assert!(close(pose.rotation * DVec3::Z, DVec3::Z));
            }
        }
    }
}

#[test]
fn limits_let_the_offset_move_within_them_and_drive_stays_inside() {
    use gc_doc::Limits;
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let mut mate = Relationship::new(
        "Mate:1",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(block, bottom(0.2)),
        on(table, top(1.0)),
    );
    mate.limits = Some(Limits { min: Some(0.0), max: Some(0.3), resting: false });
    let id = bench.relate(mate.clone()).unwrap();
    // Without a resting position the box stays at any height within the limits: it was 0.3 m above the
    // top, the nearest allowed (it starts tilted at 1.4, its face ends at 0.3 above the top after turning).
    let gap = |bench: &Bench| bench.pose(block).translation.z - 0.1 - 1.0;
    assert!((0.0..=0.3 + 1e-7).contains(&gap(&bench)), "{}", gap(&bench));
    // Dragged a little within the limits, it stays; dragged beyond, it stops at the limit.
    for (z, expected) in [(1.25, 0.15), (2.0, 0.3)] {
        let mut moved = bench.doc.get(block).unwrap().clone();
        moved.placement.xyz[2] = z;
        bench.cell(CellOp::SetBody { id: block, body: moved }).unwrap();
        assert!((gap(&bench) - expected).abs() < 1e-6, "{z}: {}", gap(&bench));
    }
    // Drive cannot leave the limits.
    let driven = bench.doc.driven(id, 0.5).unwrap();
    let (_, pose) = driven
        .iter()
        .find(|(item, _)| *item == block.erase())
        .copied()
        .unwrap_or((block.erase(), bench.pose(block)));
    assert!((pose.translation.z - 1.1 - 0.3).abs() < 1e-6, "{}", pose.translation.z);
    // With a resting position the offset holds.
    mate.limits = Some(Limits { min: Some(0.0), max: Some(0.3), resting: true });
    bench.cell(CellOp::SetRelationship { id, relationship: mate.clone() }).unwrap();
    assert!(gap(&bench).abs() < 1e-7);
    // Limits on a joint, or upside down, are refused.
    mate.limits = Some(Limits { min: Some(0.3), max: Some(0.0), resting: false });
    assert!(bench.cell(CellOp::SetRelationship { id, relationship: mate }).is_err());
}

#[test]
fn a_conflict_names_the_relationship_in_the_way_and_suppressing_it_recovers() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let mate = bench
        .relate(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(block, bottom(0.2)),
            on(table, top(1.0)),
        ))
        .unwrap();
    let xy = Geometry::origin(OriginElement::Plane(OriginPlane::Xy));
    let flush =
        Relationship::new("Flush:1", RelationKind::Flush { offset: 0.3 }, on(block, xy), on(table, xy));
    assert!(matches!(bench.relate(flush.clone()), Err(EditError::Unsolvable { .. })));
    assert_eq!(bench.doc.conflicting_relationships(Some(&flush)), vec![mate]);
    assert!(bench.doc.conflicting_relationships(None).is_empty(), "as they are, all is well");
    // Suppress the mate: the flush fits, and the mate is kept for later.
    bench.cell(CellOp::SetSuppressed { item: mate.erase(), suppressed: true }).unwrap();
    bench.relate(flush).unwrap();
    assert!((bench.pose(block).translation.z - 0.8).abs() < 1e-7);
    assert!(bench.doc.get(mate).is_some());
    // Bringing the mate back conflicts again and is refused.
    assert!(bench.cell(CellOp::SetSuppressed { item: mate.erase(), suppressed: false }).is_err());
}

#[test]
fn view_representations_restore_what_is_shown() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let hide = Command::SetComponent {
        occurrence: Occurrence::Item(block.erase()),
        flag: ComponentFlag::Visible,
        on: false,
    };
    apply(&mut bench.doc, &hide).unwrap();
    let disable = Command::SetComponent {
        occurrence: Occurrence::Item(table.erase()),
        flag: ComponentFlag::Enabled,
        on: false,
    };
    apply(&mut bench.doc, &disable).unwrap();
    let mut look = bench.doc.get(table).unwrap().look;
    look.transparency = 0.5;
    bench.cell(CellOp::SetBodyLook { id: table, look }).unwrap();
    bench
        .cell(CellOp::AddViewRepresentation {
            name: "No box".into(),
            camera: Some("cam".into()),
            section: None,
        })
        .unwrap();
    let show = Command::SetComponent {
        occurrence: Occurrence::Item(block.erase()),
        flag: ComponentFlag::Visible,
        on: true,
    };
    apply(&mut bench.doc, &show).unwrap();
    let enable = Command::SetComponent {
        occurrence: Occurrence::Item(table.erase()),
        flag: ComponentFlag::Enabled,
        on: true,
    };
    apply(&mut bench.doc, &enable).unwrap();
    look.transparency = 0.0;
    bench.cell(CellOp::SetBodyLook { id: table, look }).unwrap();
    assert!(bench.doc.component_flag(Occurrence::Item(block.erase()), ComponentFlag::Visible));
    bench.cell(CellOp::ActivateViewRepresentation(0)).unwrap();
    // Enabled state and transparency come back with the view (About View Representations).
    assert!(!bench.doc.component_flag(Occurrence::Item(table.erase()), ComponentFlag::Enabled));
    assert!((bench.doc.get(table).unwrap().look.transparency - 0.5).abs() < 1e-12);
    assert!(!bench.doc.component_flag(Occurrence::Item(block.erase()), ComponentFlag::Visible));
    assert!(bench.doc.component_flag(Occurrence::Item(table.erase()), ComponentFlag::Visible));
    assert_eq!(bench.doc.representations().views[0].camera.as_deref(), Some("cam"));
    assert!(
        bench
            .cell(CellOp::AddViewRepresentation { name: "No box".into(), camera: None, section: None })
            .is_err()
    );
}

#[test]
fn positional_representations_override_values_and_go_back_to_primary() {
    use gc_doc::PositionalRepresentation;
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let mate = bench
        .relate(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(block, bottom(0.2)),
            on(table, top(1.0)),
        ))
        .unwrap();
    let primary_z = bench.pose(block).translation.z;
    bench.cell(CellOp::AddPositionalRepresentation("Lifted".into())).unwrap();
    let lifted =
        PositionalRepresentation { name: "Lifted".into(), values: vec![(mate, 0.2)], ..Default::default() };
    bench.cell(CellOp::SetPositionalRepresentation { index: 0, position: lifted }).unwrap();
    bench.cell(CellOp::ActivatePositionalRepresentation(Some(0))).unwrap();
    assert!((bench.pose(block).translation.z - primary_z - 0.2).abs() < 1e-7, "lifted in this position");
    assert_eq!(
        bench.doc.get(mate).unwrap().kind,
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        "Primary unchanged"
    );
    bench.cell(CellOp::ActivatePositionalRepresentation(None)).unwrap();
    assert!((bench.pose(block).translation.z - primary_z).abs() < 1e-7, "back where Primary has it");

    // Suppressing the mate and placing the box elsewhere in another position.
    bench.cell(CellOp::AddPositionalRepresentation("Aside".into())).unwrap();
    let aside = PositionalRepresentation {
        name: "Aside".into(),
        suppression: vec![(mate, true)],
        placements: vec![(block.erase(), Placement { xyz: [3.0, 0.0, 0.1], rpy: [0.0; 3] })],
        ..Default::default()
    };
    bench.cell(CellOp::SetPositionalRepresentation { index: 1, position: aside }).unwrap();
    bench.cell(CellOp::ActivatePositionalRepresentation(Some(1))).unwrap();
    assert!(close(bench.pose(block).translation, DVec3::new(3.0, 0.0, 0.1)));
    bench.cell(CellOp::ActivatePositionalRepresentation(None)).unwrap();
    assert!((bench.pose(block).translation.z - primary_z).abs() < 1e-7);
    assert!(
        bench.cell(CellOp::AddPositionalRepresentation("Primary".into())).is_err(),
        "[Primary] is reserved"
    );
}

#[test]
fn assemble_offers_only_the_solutions_the_picked_geometry_allows() {
    use gc_doc::{AssembleSolution as S, possible_relations};
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let plane = Geometry::Plane { point: [0.0; 3], normal: [0.0, 0.0, 1.0] };
    let circle = Geometry::Circle { centre: [0.0; 3], normal: [0.0, 0.0, 1.0] };
    let cylinder = Geometry::Cylinder { point: [0.0; 3], direction: [0.0, 0.0, 1.0], radius: 0.05 };
    let sphere = Geometry::Sphere { centre: [0.0; 3], radius: 0.05 };
    let axis = Geometry::Axis { point: [0.0; 3], direction: [1.0, 0.0, 0.0] };
    let offer = |a: Geometry, b: Geometry| possible_relations(&on(block, a), &on(table, b));
    // Two planes: Mate-Mate, Mate-Flush, Angle-Directed (the help's own example).
    assert_eq!(offer(plane, plane), [S::MateMate, S::MateFlush, S::AngleDirected]);
    // Two circular edges: Insert offered, opposed and aligned.
    let circles = offer(circle, circle);
    assert!(circles.contains(&S::InsertOpposed) && circles.contains(&S::InsertAligned));
    assert!(!circles.contains(&S::MateFlush));
    // A sphere on a plane: tangent, inside or outside, or mate of its centre; no angle.
    assert_eq!(offer(sphere, plane), [S::MateMate, S::TangentInside, S::TangentOutside]);
    // Two axes: mate either way and angle; a cylinder on a plane may also be tangent.
    assert_eq!(offer(axis, axis), [S::MateMate, S::MateAligned, S::AngleDirected]);
    assert!(offer(cylinder, plane).contains(&S::TangentOutside));
    assert!(!offer(cylinder, axis).contains(&S::TangentOutside), "the solver has no line tangency");
    // Both picks on one component, or a joint origin: nothing.
    assert!(possible_relations(&on(block, plane), &on(block, plane)).is_empty());
    assert!(offer(Geometry::origin_frame(), plane).is_empty());
    // A solution gives a kind the solver takes.
    bench
        .relate(Relationship::new("Mate:1", S::MateMate.kind(0.0), on(block, top(0.2)), on(table, top(1.0))))
        .unwrap();
}

/// Joint origins (Inventor 2026 help, To Define and Manage Joint Relationships,
/// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6): Align 1 and Align 2 give the alignment directions, Invert
/// alignment turns it round, Offset Origin and Between Two Faces place the origin.
#[test]
fn joint_origins_take_alignment_an_offset_and_the_middle_of_two_faces() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let axis = |direction: [f64; 3]| Geometry::Axis { point: [0.0; 3], direction };
    // The block's bottom, its X edge as Align 1; the table top, its Y edge as Align 2.
    let moving = |doc: &Document, invert: bool| {
        doc.joint_origin_aligned(&on(block, bottom(0.2)), &on(block, axis([1.0, 0.0, 0.0])), invert).unwrap()
    };
    let fixed = bench
        .doc
        .joint_origin_aligned(&on(table, top(1.0)), &on(table, axis([0.0, 1.0, 0.0])), false)
        .unwrap();
    let joint = Relationship {
        limits: None,
        name: "Rigid:1".into(),
        kind: RelationKind::Rigid { flip: false },
        first: moving(&bench.doc, false),
        second: fixed,
    };
    let id = bench.relate(joint.clone()).unwrap();
    let pose = bench.pose(block);
    assert!(close(pose.rotation * DVec3::X, DVec3::Y), "{:?}", pose.rotation * DVec3::X);
    assert!(close(pose.translation, DVec3::new(0.0, 0.0, 1.1)), "{:?}", pose.translation);
    // Invert alignment: the same edge, the other way.
    let inverted = Relationship { first: moving(&bench.doc, true), ..joint.clone() };
    bench.cell(CellOp::SetRelationship { id, relationship: inverted }).unwrap();
    assert!(close(bench.pose(block).rotation * DVec3::X, -DVec3::Y));
    // A point as the alignment: the direction from the origin to it.
    let towards = on(table, Geometry::Point([-3.0, 0.0, 0.5]));
    let by_point = bench.doc.joint_origin_aligned(&on(table, top(1.0)), &towards, false).unwrap();
    let Geometry::Frame { x, .. } = by_point.geometry else { panic!() };
    assert!(close(DVec3::from_array(x), -DVec3::X));
    // A direction along the origin's axis aligns nothing.
    let along = on(table, axis([0.0, 0.0, 2.0]));
    assert!(bench.doc.joint_origin_aligned(&on(table, top(1.0)), &along, false).is_none());

    // Offset Origin: 0.3 m along the origin's X (the table's Y), 0.05 m up.
    let Geometry::Frame { origin, z, x } = fixed.geometry.offset_origin([0.3, 0.0, 0.05]) else { panic!() };
    assert!(close(DVec3::from_array(origin), DVec3::new(0.0, 0.3, 0.55)));
    assert!(close(DVec3::from_array(z), DVec3::Z) && close(DVec3::from_array(x), DVec3::Y));
    let offset = Relationship {
        second: Selection { geometry: fixed.geometry.offset_origin([0.3, 0.0, 0.05]), ..fixed },
        ..joint
    };
    bench.cell(CellOp::SetRelationship { id, relationship: offset }).unwrap();
    assert!(close(bench.pose(block).translation, DVec3::new(0.0, 0.3, 1.15)));

    // Between Two Faces: half way between the table's left and right sides, below the picked point.
    let side = |x: f64| Geometry::Plane { point: [x, 0.0, 0.0], normal: [x.signum(), 0.0, 0.0] };
    let Some(Geometry::Frame { origin, z, .. }) =
        Geometry::between_faces(&side(-1.0), &side(1.0), [0.7, 0.2, 0.4])
    else {
        panic!()
    };
    assert!(close(DVec3::from_array(origin), DVec3::new(0.0, 0.2, 0.4)));
    assert!(close(DVec3::from_array(z), -DVec3::X));
    assert!(Geometry::between_faces(&side(1.0), &top(1.0), [0.0; 3]).is_none());
}

/// Free Move and Update (Inventor 2026 help, Move assembly components,
/// GUID-9796C22B-23C1-4D69-BC2E-54056E46F255; Rotate assembly components,
/// GUID-7EAE9DFD-899A-4FD1-B63F-92DD41D0A916): a dropped component stays until Update, then snaps back;
/// a grounded one stays and what is related to it follows.
#[test]
fn free_move_drops_a_component_until_the_assembly_is_updated() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let mate = Relationship {
        limits: None,
        name: "Mate:1".into(),
        kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        first: on(block, bottom(0.2)),
        second: on(table, top(1.0)),
    };
    let id = bench.relate(mate).unwrap();
    assert!(!bench.doc.needs_update());
    let rest = bench.pose(block);

    // Dragged up and turned: it stays there, the band shows the relationship does not hold.
    let depth = bench.history.undo_depth();
    let dropped = Placement { xyz: [0.6, 0.1, 1.6], rpy: [0.5, 0.0, 0.0] };
    bench.cell(CellOp::FreeMove { item: block.erase(), placement: dropped }).unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1);
    assert_eq!(bench.doc.get(block).unwrap().placement, dropped);
    assert!(bench.doc.needs_update());
    let bands = bench.doc.relationship_bands();
    assert_eq!(bands.len(), 1);
    assert!(bands[0].id == id && !bands[0].satisfied);
    assert!(close(bands[0].ends[1], DVec3::new(0.0, 0.0, 1.0)), "{:?}", bands[0].ends);
    assert!(bands[0].ends[0].z > 1.3);
    // Update: back on the table, upright, and still where it was dropped across the table.
    bench.cell(CellOp::UpdateAssembly).unwrap();
    assert!(!bench.doc.needs_update() && bench.doc.relationship_bands()[0].satisfied);
    let pose = bench.pose(block);
    assert!(
        (pose.translation.z - rest.translation.z).abs() < 1e-7 && close(pose.rotation * DVec3::Z, DVec3::Z)
    );
    assert!((pose.translation.x - 0.6).abs() < 0.05 && (pose.translation.y - 0.1).abs() < 0.05);

    // The grounded table is dropped lower: it stays, and on Update the block follows it.
    let lower = Placement { xyz: [0.0, 0.0, 0.3], rpy: [0.0; 3] };
    bench.cell(CellOp::FreeMove { item: table.erase(), placement: lower }).unwrap();
    assert!(bench.doc.needs_update());
    bench.cell(CellOp::UpdateAssembly).unwrap();
    assert_eq!(bench.doc.get(table).unwrap().placement, lower);
    assert!((bench.pose(block).translation.z - 0.9).abs() < 1e-7);
    // Undo takes the Update and the drop back one at a time.
    bench.history.undo(&mut bench.doc).unwrap();
    assert!(bench.doc.needs_update());
    bench.history.undo(&mut bench.doc).unwrap();
    assert!(!bench.doc.needs_update());
    // Values that are not numbers and things that are not components are refused.
    let nowhere = Placement { xyz: [f64::NAN, 0.0, 0.0], rpy: [0.0; 3] };
    assert_eq!(
        bench.cell(CellOp::FreeMove { item: block.erase(), placement: nowhere }),
        Err(EditError::BadValue)
    );
    assert_eq!(bench.cell(CellOp::FreeMove { item: id.erase(), placement: lower }), Err(EditError::BadValue));
}

/// View representations (Inventor 2026 help, To Create, Edit, and Restore View Representations,
/// GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47): the active one takes display edits, Lock stops that and
/// hides components added later, Copy makes an active duplicate. Positional representations copy too
/// (GUID-34C28C35-45CA-4476-9336-30146F7C9471).
#[test]
fn the_active_view_follows_the_display_until_it_is_locked() {
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let shown = |bench: &mut Bench, body: Id<Body>, on: bool| {
        let occurrence = Occurrence::Item(body.erase());
        let command = Command::SetComponent { occurrence, flag: ComponentFlag::Visible, on };
        let applied = apply(&mut bench.doc, &command).unwrap();
        bench.history.record(applied.label, applied.delta);
    };
    let visible = |bench: &Bench, body: Id<Body>| {
        bench.doc.component_flag(Occurrence::Item(body.erase()), ComponentFlag::Visible)
    };
    let hidden = |bench: &Bench, view: usize| bench.doc.representations().views[view].hidden.clone();
    bench.cell(CellOp::AddViewRepresentation { name: "View1".into(), camera: None, section: None }).unwrap();
    // Not active yet: display edits leave it alone.
    shown(&mut bench, block, false);
    assert!(hidden(&bench, 0).is_empty());
    shown(&mut bench, block, true);
    // Active: hiding the block is saved in it, in the same undo step.
    bench.cell(CellOp::ActivateViewRepresentation(0)).unwrap();
    let depth = bench.history.undo_depth();
    shown(&mut bench, block, false);
    assert_eq!(bench.history.undo_depth(), depth + 1);
    assert_eq!(hidden(&bench, 0), [Occurrence::Item(block.erase())]);
    bench.history.undo(&mut bench.doc).unwrap();
    assert!(hidden(&bench, 0).is_empty() && visible(&bench, block));
    shown(&mut bench, block, false);

    // Copy: "View2", active, the same display; the original keeps what it had.
    bench.cell(CellOp::CopyViewRepresentation(0)).unwrap();
    let reps = bench.doc.representations();
    assert_eq!((reps.views[1].name.as_str(), reps.active_view), ("View2", Some(1)));
    assert_eq!(reps.views[1].hidden, reps.views[0].hidden);
    shown(&mut bench, block, true);
    assert!(hidden(&bench, 1).is_empty());
    assert_eq!(hidden(&bench, 0), [Occurrence::Item(block.erase())]);

    // Locked: display edits are not saved, a replacement keeps the display (the name may change), and a
    // component added while it is active is hidden.
    bench.cell(CellOp::LockViewRepresentation { index: 1, locked: true }).unwrap();
    shown(&mut bench, table, false);
    assert!(hidden(&bench, 1).is_empty());
    shown(&mut bench, table, true);
    let mut renamed = bench.doc.representations().views[1].clone();
    renamed.name = "Assembly step".into();
    renamed.hidden.push(Occurrence::Item(table.erase()));
    bench.cell(CellOp::SetViewRepresentation { index: 1, view: renamed }).unwrap();
    let view = bench.doc.representations().views[1].clone();
    assert!(view.name == "Assembly step" && view.hidden.is_empty() && view.locked);
    let late = bench.body("Late", [0.1, 0.1, 0.1], Placement { xyz: [1.5, 0.0, 0.05], rpy: [0.0; 3] });
    assert!(!visible(&bench, late) && visible(&bench, block));
    assert_eq!(hidden(&bench, 1), [Occurrence::Item(late.erase())]);
    // It can be shown by hand; the locked view still does not take that.
    shown(&mut bench, late, true);
    assert!(visible(&bench, late));
    assert_eq!(hidden(&bench, 1), [Occurrence::Item(late.erase())]);
    // Unlocked, it follows again.
    bench.cell(CellOp::LockViewRepresentation { index: 1, locked: false }).unwrap();
    shown(&mut bench, late, false);
    shown(&mut bench, late, true);
    assert!(hidden(&bench, 1).is_empty());
    assert!(bench.cell(CellOp::LockViewRepresentation { index: 9, locked: true }).is_err());
    assert!(bench.cell(CellOp::CopyViewRepresentation(9)).is_err());

    // A positional representation copies with its overrides under a numbered name; Primary stays active.
    bench.cell(CellOp::AddPositionalRepresentation("Open".into())).unwrap();
    let mut open = bench.doc.representations().positions[0].clone();
    open.placements.push((block.erase(), Placement { xyz: [0.0, 0.0, 2.0], rpy: [0.0; 3] }));
    bench.cell(CellOp::SetPositionalRepresentation { index: 0, position: open.clone() }).unwrap();
    bench.cell(CellOp::CopyPositionalRepresentation(0)).unwrap();
    let reps = bench.doc.representations();
    assert_eq!(reps.positions[1].name, "Open1");
    assert_eq!(reps.positions[1].placements, open.placements);
    assert_eq!(reps.active_position, None);
    assert!(bench.cell(CellOp::CopyPositionalRepresentation(5)).is_err());
}

/// Accept the relationship (Inventor 2026 help, Place/Edit Connection Error Reference,
/// GUID-23D0D333-AD6A-4CB6-9786-6966DFE8FDFF) and Recover (Use Design Doctor to repair relationship
/// errors, GUID-8C95B0AB-4505-4092-8C37-E60C68DCF4A0; Relationship Conflict Analysis Reference,
/// GUID-528B2CD0-93FD-45BC-AB69-6C5A559BB8E8).
#[test]
fn a_relationship_that_cannot_be_solved_can_be_accepted_with_an_error() {
    use gc_doc::{ComponentStatus, RelationStatus};
    let mut bench = Bench::new();
    let (table, block) = table_and_box(&mut bench);
    let mate = |offset: f64| Relationship {
        limits: None,
        name: format!("Mate {offset}"),
        kind: RelationKind::Mate { offset, sense: AxisSense::Opposed },
        first: on(block, bottom(0.2)),
        second: on(table, top(1.0)),
    };
    let first = bench.relate(mate(0.0)).unwrap();
    let rest = bench.pose(block);
    // The same faces 0.3 m apart as well: refused as before, and nothing is left behind.
    assert!(matches!(bench.relate(mate(0.3)), Err(EditError::Unsolvable { .. })));
    assert_eq!(bench.doc.relationships().iter().count(), 1);
    // Accepted: it is there with an error, the block stays where the first mate holds it.
    let created = bench.cell(CellOp::AcceptRelationship(mate(0.3))).unwrap();
    let Created::Relationship(second) = created[0] else { panic!() };
    assert_eq!(bench.doc.sick_relationships(), [second]);
    assert_eq!(bench.doc.relationship_status(second), Some(RelationStatus::Sick));
    assert_ne!(bench.doc.relationship_status(first), Some(RelationStatus::Sick));
    assert_eq!(bench.pose(block), rest);
    assert_eq!(bench.doc.component_status(block.erase()), Some(ComponentStatus::Unknown));
    // Other commands keep working, the error stays.
    let moved = Placement { xyz: [0.2, 0.1, 0.5], rpy: [0.0; 3] };
    let mut body = bench.doc.get(table).unwrap().clone();
    body.placement = moved;
    bench.cell(CellOp::SetBody { id: table, body }).unwrap();
    assert!(bench.doc.is_sick(second));

    // Isolate Related Failed: the first mate stands in its way; both components take part.
    let (culprits, components) = bench.doc.related_failed(second);
    assert_eq!(culprits, [first]);
    assert_eq!(components.len(), 2);
    assert!(bench.doc.related_failed(first).0.is_empty());
    // Conflict analysis: still in conflict as it is; fine once either mate is set to Break.
    assert!(!bench.doc.conflict_check(&[], &[]));
    assert!(bench.doc.conflict_check(&[first], &[]) && bench.doc.conflict_check(&[second], &[]));
    let before = bench.doc.clone();
    assert!(bench.doc.same_content(&before));

    // Treatments. Suppressing the first mate lets the accepted one be satisfied: the mark goes and the
    // block moves to 0.3 m above the table.
    bench.cell(CellOp::SetSuppressed { item: first.erase(), suppressed: true }).unwrap();
    bench.cell(CellOp::UpdateAssembly).unwrap();
    assert!(bench.doc.sick_relationships().is_empty());
    assert!(
        (bench.pose(block).translation.z - bench.pose(table).translation.z - 0.5 - 0.3 - 0.1).abs() < 1e-7
    );
}

/// Motion constraints (Inventor 2026 help, Place Motion constraints between assembly components,
/// GUID-493DEAF8-D124-47BE-AFF2-B0535E9005C5; Place Constraint Reference - Motion Tab,
/// GUID-0293C151-565F-4980-AA95-DD76C54AB538): Rotation ties the turning of two components by a ratio,
/// Rotation-Translation the travel of one to the turning of the other; they never stand in the way of
/// positional constraints or move grounded components.
#[test]
fn motion_constraints_tie_turning_and_travel_together() {
    use gc_doc::AngleSolution;
    let mut bench = Bench::new();
    let table = bench.body("Table", [2.0, 1.0, 1.0], Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] });
    let frame = |origin: [f64; 3], z: [f64; 3], x: [f64; 3]| Geometry::Frame { origin, z, x };
    let axis = |direction: [f64; 3]| Geometry::Axis { point: [0.0; 3], direction };
    // Two gears lying on the table, each on a rotational joint about the vertical.
    let gear = |bench: &mut Bench, name: &str, x: f64| {
        let body = bench.body(name, [0.2, 0.2, 0.1], Placement { xyz: [x, 0.0, 1.3], rpy: [0.0; 3] });
        let joint = Relationship::new(
            format!("Rotational {name}"),
            RelationKind::Rotational { flip: false },
            on(body, frame([0.0, 0.0, -0.05], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0])),
            on(table, frame([x, 0.0, 0.5], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])),
        );
        bench.relate(joint).unwrap();
        body
    };
    let (driver, driven) = (gear(&mut bench, "Driver", -0.3), gear(&mut bench, "Driven", 0.3));
    // A rack on a slider along the table's X.
    let rack = bench.body("Rack", [0.4, 0.05, 0.05], Placement { xyz: [0.0, 0.4, 1.3], rpy: [0.0; 3] });
    let slide = Relationship::new(
        "Slider",
        RelationKind::Slider { flip: true },
        on(rack, frame([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])),
        on(table, frame([0.0, 0.4, 0.525], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])),
    );
    bench.relate(slide).unwrap();
    // The driver's angle to the table is held by an angle constraint about the vertical.
    let side = |body: Id<Body>| on(body, Geometry::Plane { point: [0.1, 0.0, 0.0], normal: [1.0, 0.0, 0.0] });
    let angle = |value: f64| Relationship {
        limits: None,
        name: "Angle".into(),
        kind: RelationKind::Angle { angle: value, solution: AngleSolution::Explicit(on(table, top(1.0))) },
        first: side(table),
        second: side(driver),
    };
    let held = bench.relate(angle(0.0)).unwrap();
    let yaw = |bench: &Bench, body: Id<Body>| {
        let x = bench.pose(body).rotation * DVec3::X;
        gc_math::fm::atan2(x.y, x.x)
    };
    // Gears: the driven one turns twice as far, the other way. Rack: 0.1 m a turn.
    let gears = Relationship::new(
        "Rotation",
        RelationKind::Rotation { ratio: 2.0, reverse: true },
        on(driver, axis([0.0, 0.0, 1.0])),
        on(driven, axis([0.0, 0.0, 1.0])),
    );
    bench.relate(gears).unwrap();
    let pinion = Relationship::new(
        "Rotation-Translation",
        RelationKind::RotationTranslation { distance: 0.1, reverse: false },
        on(driver, axis([0.0, 0.0, 1.0])),
        on(rack, axis([1.0, 0.0, 0.0])),
    );
    let pinion = bench.relate(pinion).unwrap();
    // Placing them moves nothing.
    assert!(yaw(&bench, driver).abs() < 1e-9 && yaw(&bench, driven).abs() < 1e-9);
    let rack_at = bench.pose(rack).translation.x;

    let mut turned = 0.0;
    for step in [0.3, 0.5, -0.2] {
        let before = (yaw(&bench, driver), yaw(&bench, driven), bench.pose(rack).translation.x);
        bench.cell(CellOp::SetRelationship { id: held, relationship: angle(step) }).unwrap();
        let by = yaw(&bench, driver) - before.0;
        assert!(by.abs() > 0.1, "the driver turned: {by}");
        turned += by;
        assert!((yaw(&bench, driven) - before.1 + 2.0 * by).abs() < 1e-6, "{step}");
        let travel = bench.pose(rack).translation.x - before.2;
        assert!((travel - 0.1 * by / std::f64::consts::TAU).abs() < 1e-7, "{step}: {travel}");
    }
    assert!((bench.pose(rack).translation.x - rack_at - 0.1 * turned / std::f64::consts::TAU).abs() < 1e-6);
    // Undo takes the driver, the driven gear and the rack back together.
    let driven_now = yaw(&bench, driven);
    bench.history.undo(&mut bench.doc).unwrap();
    assert!((yaw(&bench, driven) - driven_now).abs() > 0.1);
    bench.history.redo(&mut bench.doc).unwrap();
    assert!((yaw(&bench, driven) - driven_now).abs() < 1e-12);

    // A grounded component is not moved, and the motion constraint does not make the edit fail.
    let command = Command::SetComponent {
        occurrence: Occurrence::Item(driven.erase()),
        flag: ComponentFlag::Grounded,
        on: true,
    };
    apply(&mut bench.doc, &command).unwrap();
    let fixed = bench.pose(driven);
    bench.cell(CellOp::SetRelationship { id: held, relationship: angle(0.6) }).unwrap();
    assert_eq!(bench.pose(driven), fixed);
    // Not for Drive, no zero ratio, and the dialog's defaults.
    assert!(bench.doc.driven(pinion, 1.0).is_err());
    let zero = Relationship::new(
        "Zero",
        RelationKind::Rotation { ratio: 0.0, reverse: false },
        on(driver, axis([0.0, 0.0, 1.0])),
        on(driven, axis([0.0, 0.0, 1.0])),
    );
    assert_eq!(bench.relate(zero), Err(EditError::BadValue));
    let cylinder = |radius: f64| Geometry::Cylinder { point: [0.0; 3], direction: [0.0, 0.0, 1.0], radius };
    let (small, large) = (on(driver, cylinder(0.02)), on(driven, cylinder(0.08)));
    assert_eq!(Document::motion_default(&small, &large, false), Some(0.25));
    assert_eq!(
        Document::motion_default(&small, &on(rack, axis([1.0, 0.0, 0.0])), true),
        Some(std::f64::consts::TAU * 0.02)
    );
    assert_eq!(Document::motion_default(&on(driver, axis([0.0, 0.0, 1.0])), &large, false), Some(1.0));
}
