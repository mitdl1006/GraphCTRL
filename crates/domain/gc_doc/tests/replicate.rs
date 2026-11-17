//! Mirror, copy and replace components (Inventor 2026 help: About Mirrored Assembly Components,
//! GUID-66AA4A06-F63F-49C3-8F39-42FE4999F1FC; About Copied Assembly Components,
//! GUID-0E21F4A0-DE77-469D-9381-9F55CFA6BF49; Replace assembly components,
//! GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD).

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::{AnyId, Id};
use gc_doc::{
    AxisSense, Body, CellOp, Command, ComponentFlag, Created, Document, EditError, Geometry, History,
    Occurrence, OriginElement, OriginPlane, Placement, RelationKind, Relationship, Replacement, Selection,
    Solid, apply,
};
use gc_math::{DPose3, DVec3};

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

    fn relate(&mut self, relationship: Relationship) -> Id<Relationship> {
        let Created::Relationship(id) = self.cell(CellOp::AddRelationship(relationship)).unwrap()[0] else {
            panic!()
        };
        id
    }

    fn pose(&self, body: Id<Body>) -> DPose3 {
        let p = self.doc.get(body).unwrap().placement;
        gc_math::pose_from_xyz_rpy(p.xyz, p.rpy)
    }

    /// A box body's eight corners in the world, sorted.
    fn corners(&self, body: Id<Body>) -> Vec<DVec3> {
        let Solid::Box { size } = self.doc.get(body).unwrap().solid else { panic!() };
        let pose = self.pose(body);
        let mut out: Vec<DVec3> = (0..8)
            .map(|i| {
                let s = |k: usize| if i & (1 << k) == 0 { -0.5 } else { 0.5 };
                let local = DVec3::new(s(0) * size[0], s(1) * size[1], s(2) * size[2]);
                pose.translation + pose.rotation * local
            })
            .collect();
        out.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
        out
    }

    fn inactive(&self, id: Id<Relationship>) -> bool {
        self.doc.is_inactive(id.erase())
    }
}

fn on(body: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

fn plane(point: [f64; 3], normal: [f64; 3]) -> Geometry {
    Geometry::Plane { point, normal }
}

fn same(a: &[DVec3], b: &[DVec3]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (*p - *q).length() < 1e-7)
}

fn bodies(created: &[Created]) -> Vec<Id<Body>> {
    created.iter().filter_map(|c| if let Created::Body(id) = c { Some(*id) } else { None }).collect()
}

fn relationships(created: &[Created]) -> Vec<Id<Relationship>> {
    created.iter().filter_map(|c| if let Created::Relationship(id) = c { Some(*id) } else { None }).collect()
}

/// A grounded table 1 m high; a block sitting on it off to +X, turned about Z; a pin on the block.
fn bracket(bench: &mut Bench) -> (Id<Body>, Id<Body>, Id<Body>) {
    let table = bench.body("Table", [3.0, 1.0, 1.0], Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] });
    let block =
        bench.body("Block", [0.4, 0.2, 0.2], Placement { xyz: [0.8, 0.1, 1.1], rpy: [0.0, 0.0, 0.3] });
    let pin = bench.body("Pin", [0.05, 0.05, 0.3], Placement { xyz: [0.9, 0.1, 1.5], rpy: [0.0; 3] });
    bench.relate(Relationship::new(
        "Mate:1",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(block, plane([0.0, 0.0, -0.1], [0.0, 0.0, -1.0])),
        on(table, plane([0.0, 0.0, 0.5], [0.0, 0.0, 1.0])),
    ));
    // The pin stands on the block's top, its own XZ plane flush with the block's.
    bench.relate(Relationship::new(
        "Mate:2",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(pin, plane([0.0, 0.0, -0.15], [0.0, 0.0, -1.0])),
        on(block, plane([0.0, 0.0, 0.1], [0.0, 0.0, 1.0])),
    ));
    let xz = Geometry::origin(OriginElement::Plane(OriginPlane::Xz));
    bench.relate(Relationship::new(
        "Flush:1",
        RelationKind::Flush { offset: 0.0 },
        on(pin, xz),
        on(block, xz),
    ));
    (table, block, pin)
}

#[test]
fn mirror_places_reflected_copies_and_re_makes_the_relationships_between_them() {
    let mut bench = Bench::new();
    let (table, block, pin) = bracket(&mut bench);
    let depth = bench.history.undo_depth();
    let relationships_before = bench.doc.relationships().len();
    // Across the table's YZ plane (the world's, x = 0).
    let yz = on(table, Geometry::origin(OriginElement::Plane(OriginPlane::Yz)));
    let created = bench
        .cell(CellOp::MirrorComponents {
            items: vec![block.erase(), pin.erase()],
            plane: yz,
            relationships: true,
            ground: false,
        })
        .unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1, "one undo step");
    let copies = bodies(&created);
    assert_eq!(copies.len(), 2);
    let reflect = |p: DVec3| DVec3::new(-p.x, p.y, p.z);
    for (source, copy) in [block, pin].into_iter().zip(&copies) {
        let mut mirrored: Vec<DVec3> = bench.corners(source).into_iter().map(reflect).collect();
        mirrored.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
        assert!(same(&bench.corners(*copy), &mirrored), "the copy is the source's mirror image");
        // A proper rotation: no reflection in the pose itself.
        assert!((bench.pose(*copy).rotation.length() - 1.0).abs() < 1e-9);
    }
    // Only the relationships with both references in the set (the pin's two to the block) are re-made.
    let made = relationships(&created);
    assert_eq!(made.len(), 2);
    assert_eq!(bench.doc.relationships().len(), relationships_before + 2);
    assert!(made.iter().all(|id| !bench.inactive(*id)), "both solve as they are");
    let names: Vec<String> = made.iter().map(|id| bench.doc.get(*id).unwrap().name.clone()).collect();
    assert_eq!(names, ["Mate:3", "Flush:2"]);
    let block_copy = Occurrence::Item(copies[0].erase());
    assert!(
        bench
            .doc
            .relationships()
            .iter()
            .all(|(_, r)| r.first.occurrence != block_copy || r.second.occurrence.owner() != table.erase())
    );
    assert_eq!(bench.doc.get(copies[0]).unwrap().name, "Block:1");

    // Moving the copied block takes the copied pin along (the relationships are live).
    let moved = Placement { xyz: [-1.0, 0.2, 1.1], rpy: [0.0; 3] };
    let mut body = bench.doc.get(copies[0]).unwrap().clone();
    body.placement = moved;
    bench.cell(CellOp::SetBody { id: copies[0], body }).unwrap();
    // Both copies are free; whatever the solver moved, the pin still stands on the block's top.
    let (pin_at, block_at) = (bench.pose(copies[1]).translation, bench.pose(copies[0]).translation);
    assert!((pin_at.z - block_at.z - 0.25).abs() < 1e-4, "{pin_at} {block_at}");
    assert!(bench.doc.solve_relationships().is_ok());

    // Undo takes all of it away.
    bench.history.undo(&mut bench.doc).unwrap();
    bench.history.undo(&mut bench.doc).unwrap();
    assert!(bench.doc.get(copies[0]).is_none());
    assert_eq!(bench.doc.relationships().len(), relationships_before);
}

#[test]
fn mirror_suppresses_relationships_whose_orientation_is_ambiguous_and_grounds_when_asked() {
    let mut bench = Bench::new();
    let (table, block, pin) = bracket(&mut bench);
    // A symmetry about a plane outside the set: which way round is ambiguous for the mirror image.
    let side = on(table, Geometry::origin(OriginElement::Plane(OriginPlane::Xz)));
    let xz = Geometry::origin(OriginElement::Plane(OriginPlane::Xz));
    let symmetry = Relationship::new(
        "Symmetry:1",
        RelationKind::Symmetry { plane: side },
        on(pin, plane([0.0, 0.025, 0.0], [0.0, 1.0, 0.0])),
        on(block, xz),
    );
    bench.relate(symmetry);
    let plane = on(table, Geometry::origin(OriginElement::Plane(OriginPlane::Yz)));
    let created = bench
        .cell(CellOp::MirrorComponents {
            items: vec![block.erase(), pin.erase()],
            plane,
            relationships: true,
            ground: true,
        })
        .unwrap();
    let copies = bodies(&created);
    for copy in &copies {
        assert!(bench.doc.component_flag(Occurrence::Item(copy.erase()), ComponentFlag::Grounded));
    }
    let copied = relationships(&created)
        .into_iter()
        .find(|id| matches!(bench.doc.get(*id).unwrap().kind, RelationKind::Symmetry { .. }))
        .unwrap();
    assert!(bench.inactive(copied), "ambiguous: made suppressed");
    // Without Mirror Relationships nothing is re-made.
    let plane = on(table, Geometry::origin(OriginElement::Plane(OriginPlane::Yz)));
    let created = bench
        .cell(CellOp::MirrorComponents {
            items: vec![block.erase()],
            plane,
            relationships: false,
            ground: false,
        })
        .unwrap();
    assert!(relationships(&created).is_empty());
    // The mirror plane must be a plane.
    let axis = on(table, Geometry::origin(OriginElement::Axis(0)));
    assert!(
        bench
            .cell(CellOp::MirrorComponents {
                items: vec![block.erase()],
                plane: axis,
                relationships: true,
                ground: false
            })
            .is_err()
    );
}

#[test]
fn copy_moves_the_copies_and_copies_their_relationships() {
    let mut bench = Bench::new();
    let (_table, block, pin) = bracket(&mut bench);
    let offset = Placement { xyz: [0.0, -0.4, 0.0], rpy: [0.0; 3] };
    let created = bench
        .cell(CellOp::CopyComponents {
            items: vec![block.erase(), pin.erase()],
            offset,
            relationships: true,
            ground: false,
        })
        .unwrap();
    let copies = bodies(&created);
    for (source, copy) in [block, pin].into_iter().zip(&copies) {
        let shifted: Vec<DVec3> =
            bench.corners(source).into_iter().map(|p| p + DVec3::new(0.0, -0.4, 0.0)).collect();
        assert!(same(&bench.corners(*copy), &shifted));
        assert_eq!(bench.doc.get(*copy).unwrap().look, bench.doc.get(source).unwrap().look);
    }
    assert_eq!(relationships(&created).len(), 2);
    assert!(relationships(&created).iter().all(|id| !bench.inactive(*id)));
    // Copying copies again names on from the highest.
    let created = bench
        .cell(CellOp::CopyComponents {
            items: vec![block.erase()],
            offset,
            relationships: false,
            ground: false,
        })
        .unwrap();
    assert_eq!(bench.doc.get(bodies(&created)[0]).unwrap().name, "Block:2");
    assert!(
        bench
            .cell(CellOp::CopyComponents { items: vec![], offset, relationships: true, ground: false })
            .is_err()
    );
}

#[test]
fn replace_keeps_the_place_and_the_relationships_whose_geometry_is_still_there() {
    let mut bench = Bench::new();
    let (table, block, pin) = bracket(&mut bench);
    let before = bench.pose(block);
    let pin_mate =
        bench.doc.relationships().iter().find(|(_, r)| r.name == "Mate:2").map(|(id, _)| id).unwrap();
    let flush =
        bench.doc.relationships().iter().find(|(_, r)| r.name == "Flush:1").map(|(id, _)| id).unwrap();
    let table_mate =
        bench.doc.relationships().iter().find(|(_, r)| r.name == "Mate:1").map(|(id, _)| id).unwrap();
    // Same footprint, taller: the bottom face moves, so the mate on it is lost; the top face too.
    let taller = Solid::Box { size: [0.4, 0.2, 0.4] };
    let depth = bench.history.undo_depth();
    bench
        .cell(CellOp::ReplaceComponents {
            items: vec![block.erase()],
            with: Replacement::Solid(taller.clone()),
        })
        .unwrap();
    assert_eq!(bench.history.undo_depth(), depth + 1);
    assert_eq!(bench.doc.get(block).unwrap().solid, taller);
    assert_eq!(bench.doc.get(block).unwrap().name, "Block");
    let after = bench.pose(block);
    assert!((after.translation - before.translation).length() < 1e-9, "origins coincident");
    assert!(bench.inactive(table_mate) && bench.inactive(pin_mate), "faces gone: suppressed");
    assert!(!bench.inactive(flush), "origin planes are always there");
    let _ = table;

    // Back, then to a box whose faces are where they were: everything is kept.
    bench.history.undo(&mut bench.doc).unwrap();
    let wider = Solid::Box { size: [0.6, 0.2, 0.2] };
    bench
        .cell(CellOp::ReplaceComponents { items: vec![block.erase()], with: Replacement::Solid(wider) })
        .unwrap();
    assert!(!bench.inactive(table_mate) && !bench.inactive(pin_mate) && !bench.inactive(flush));

    // A body cannot take an assembly, nor a pin a missing shape.
    let bad = Solid::Box { size: [0.0, 1.0, 1.0] };
    assert!(
        bench
            .cell(CellOp::ReplaceComponents { items: vec![pin.erase()], with: Replacement::Solid(bad) })
            .is_err()
    );
}

#[test]
fn replace_all_finds_every_component_of_the_same_source() {
    let mut bench = Bench::new();
    let size = [0.1, 0.1, 0.1];
    let a = bench.body("Bolt", size, Placement { xyz: [0.0, 0.0, 0.0], rpy: [0.0; 3] });
    let b = bench.body("Bolt:1", size, Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] });
    let other = bench.body("Nut", [0.2, 0.1, 0.1], Placement { xyz: [2.0, 0.0, 0.0], rpy: [0.0; 3] });
    let all: Vec<AnyId> = bench.doc.same_source(a.erase());
    assert_eq!(all, vec![a.erase(), b.erase()]);
    let screw = Solid::Mesh { file: "screw.stl".into(), scale: 0.001 };
    bench.cell(CellOp::ReplaceComponents { items: all, with: Replacement::Solid(screw.clone()) }).unwrap();
    assert_eq!(bench.doc.get(a).unwrap().solid, screw);
    assert_eq!(bench.doc.get(b).unwrap().solid, screw);
    assert_ne!(bench.doc.get(other).unwrap().solid, screw);
}
