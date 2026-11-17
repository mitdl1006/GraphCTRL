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
        name: "Mate:2".into(),
        kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        first: on(block, bottom(0.2)),
        second: on(table, top(1.0)),
    });
    assert!(matches!(result, Err(EditError::Unsolvable(_))), "{result:?}");
    assert_eq!(bench.doc.relationships(), before.relationships());
    assert_eq!(bench.doc.bodies(), before.bodies());

    // Flush needs two planes, Insert two circles; a constraint on one component with itself is refused.
    let bad = Relationship {
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
            name: "Mate:1".into(),
            kind: RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            first: on(block, bottom(0.2)),
            second: on(table, top(1.0)),
        })
        .unwrap();
    // Deleting a constrained component takes nothing with it silently: it is in use.
    assert!(bench.cell(CellOp::DeleteBody(block)).is_err());
}
