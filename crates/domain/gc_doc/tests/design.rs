//! The design side of the model tree: work features, extrusions, imported assemblies, and the tree's
//! order with suppression and the rollback marker.

#![allow(clippy::float_cmp, clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{
    Assembly, AssemblyMesh, AssemblyPart, CellOp, Command, Created, Document, EditError, EndSnap, Extent,
    Extrude, Feature, FeatureKind, History, OriginPlane, Placement, PlaneRef, PointRef, SketchOp,
    WorkFeature, WorkGeometry, apply,
};

fn run(doc: &mut Document, history: &mut History, command: Command) -> Result<Vec<Created>, EditError> {
    let applied = apply(doc, &command)?;
    history.record(applied.label, applied.delta);
    Ok(applied.created)
}

fn cell(doc: &mut Document, history: &mut History, op: CellOp) -> Result<Vec<Created>, EditError> {
    run(doc, history, Command::Cell(op))
}

/// A sketch on XY with a rectangle; returns the sketch and its curves.
fn sketch_with_rectangle(
    doc: &mut Document,
    history: &mut History,
) -> (gc_core::Id<gc_doc::Sketch>, Vec<gc_core::Id<gc_sketch::Curve>>) {
    let frame = doc.origin_plane("XY").unwrap();
    let Created::Sketch(sketch) =
        run(doc, history, Command::CreateSketch { name: "Sketch1".into(), frame, spatial: false }).unwrap()
            [0]
    else {
        panic!()
    };
    let op = SketchOp::AddRectangle { a: [0.0, 0.0], b: [0.4, 0.2], snap_a: EndSnap::default() };
    run(doc, history, Command::Sketch { sketch, op }).unwrap();
    let curves = doc.get(sketch).unwrap().curves.keys().copied().collect();
    (sketch, curves)
}

fn extrude(
    sketch: gc_core::Id<gc_doc::Sketch>,
    profile: Vec<gc_core::Id<gc_sketch::Curve>>,
    extent: Extent,
) -> Feature {
    Feature {
        name: "Extrusion1".into(),
        kind: FeatureKind::Extrude(Extrude { sketch, profile, extent, reversed: false }),
    }
}

fn work(name: &str, geometry: WorkGeometry) -> WorkFeature {
    WorkFeature { name: name.into(), geometry, visible: true }
}

#[test]
fn work_features_and_an_extrusion_are_added_checked_and_undone() {
    let mut doc = Document::new();
    let mut history = History::default();
    let empty = doc.clone();
    let (sketch, curves) = sketch_with_rectangle(&mut doc, &mut history);
    assert_eq!(curves.len(), 4);

    let offset =
        work("Plane1", WorkGeometry::PlaneOffset { base: PlaneRef::Origin(OriginPlane::Xy), distance: 0.3 });
    let Created::WorkFeature(plane) =
        cell(&mut doc, &mut history, CellOp::AddWorkFeature(offset)).unwrap()[0]
    else {
        panic!()
    };
    let point_id = *doc.get(sketch).unwrap().points.keys().next().unwrap();
    let corner =
        work("Point1", WorkGeometry::Point { at: PointRef::SketchPoint { sketch, point: point_id } });
    let Created::WorkFeature(point) =
        cell(&mut doc, &mut history, CellOp::AddWorkFeature(corner)).unwrap()[0]
    else {
        panic!()
    };
    let axis = work(
        "Axis1",
        WorkGeometry::AxisNormalToPlane { plane: PlaneRef::Work(plane), through: PointRef::Work(point) },
    );
    cell(&mut doc, &mut history, CellOp::AddWorkFeature(axis)).unwrap();

    // References of the wrong kind, a point twice, or non-finite numbers are refused.
    let wrong = work("Bad", WorkGeometry::PlaneOffset { base: PlaneRef::Work(point), distance: 0.1 });
    assert!(cell(&mut doc, &mut history, CellOp::AddWorkFeature(wrong)).is_err());
    let twice = work("Bad", WorkGeometry::AxisThroughPoints { points: [PointRef::Origin, PointRef::Origin] });
    assert!(cell(&mut doc, &mut history, CellOp::AddWorkFeature(twice)).is_err());
    let nan = work("Bad", WorkGeometry::Point { at: PointRef::Fixed([f64::NAN, 0.0, 0.0]) });
    assert!(cell(&mut doc, &mut history, CellOp::AddWorkFeature(nan)).is_err());

    let Created::Feature(feature) = cell(
        &mut doc,
        &mut history,
        CellOp::AddFeature(extrude(sketch, curves.clone(), Extent::Symmetric(0.5))),
    )
    .unwrap()[0] else {
        panic!()
    };
    assert_eq!(doc.get(feature).unwrap().sketch(), sketch);
    assert_eq!(Extent::Symmetric(0.5).range(false), Some((-0.25, 0.25)));
    assert_eq!(Extent::Distance(0.2).range(true), Some((-0.2, 0.0)));
    assert!(
        cell(
            &mut doc,
            &mut history,
            CellOp::AddFeature(extrude(sketch, curves.clone(), Extent::Distance(0.0)))
        )
        .is_err()
    );

    // The sketch and the plane have dependents: they cannot be deleted before them.
    assert!(run(&mut doc, &mut history, Command::DeleteSketch { id: sketch }).is_err());
    assert!(cell(&mut doc, &mut history, CellOp::DeleteWorkFeature(plane)).is_err());

    // The tree order: sketch, plane, point, axis, extrusion (oldest first, nothing stored yet).
    let order = doc.model_order();
    assert_eq!(order.len(), 5);
    assert_eq!(order[0], sketch.erase());
    assert_eq!(order[4], feature.erase());

    let built = doc.clone();
    while history.undo(&mut doc).is_some() {}
    assert!(doc.same_content(&empty));
    while history.redo(&mut doc).is_some() {}
    assert!(doc.same_content(&built));
}

#[test]
fn the_tree_can_be_reordered_suppressed_and_rolled_back() {
    let mut doc = Document::new();
    let mut history = History::default();
    let (sketch, curves) = sketch_with_rectangle(&mut doc, &mut history);
    let Created::WorkFeature(plane) = cell(
        &mut doc,
        &mut history,
        CellOp::AddWorkFeature(work(
            "Plane1",
            WorkGeometry::PlaneOffset { base: PlaneRef::Origin(OriginPlane::Yz), distance: 1.0 },
        )),
    )
    .unwrap()[0] else {
        panic!()
    };
    let Created::Feature(feature) =
        cell(&mut doc, &mut history, CellOp::AddFeature(extrude(sketch, curves, Extent::Distance(0.1))))
            .unwrap()[0]
    else {
        panic!()
    };
    let (s, p, f) = (sketch.erase(), plane.erase(), feature.erase());
    assert_eq!(doc.model_order(), vec![s, p, f]);

    // The plane can go first; the extrusion cannot come before its sketch.
    cell(&mut doc, &mut history, CellOp::MoveInTree { item: p, before: Some(s) }).unwrap();
    assert_eq!(doc.model_order(), vec![p, s, f]);
    assert!(matches!(
        cell(&mut doc, &mut history, CellOp::MoveInTree { item: f, before: Some(s) }),
        Err(EditError::InUse)
    ));
    assert!(matches!(
        cell(&mut doc, &mut history, CellOp::MoveInTree { item: s, before: None }),
        Err(EditError::InUse)
    ));

    // Suppressed and rolled-back items are kept but inactive.
    cell(&mut doc, &mut history, CellOp::SetSuppressed { item: p, suppressed: true }).unwrap();
    assert!(doc.is_inactive(p) && !doc.is_inactive(s));
    cell(&mut doc, &mut history, CellOp::SetRollback { after: Some(s) }).unwrap();
    assert!(doc.is_inactive(f) && !doc.is_inactive(s));
    // A new item goes right after the marker, and the marker moves past it.
    let Created::WorkFeature(later) = cell(
        &mut doc,
        &mut history,
        CellOp::AddWorkFeature(work("Point1", WorkGeometry::Point { at: PointRef::Fixed([0.0, 0.0, 1.0]) })),
    )
    .unwrap()[0] else {
        panic!()
    };
    assert_eq!(doc.model_order(), vec![p, s, later.erase(), f]);
    assert!(!doc.is_inactive(later.erase()) && doc.is_inactive(f));
    cell(&mut doc, &mut history, CellOp::SetRollback { after: None }).unwrap();
    assert!(!doc.is_inactive(f));

    // Deleting an item just drops it from the order.
    cell(&mut doc, &mut history, CellOp::DeleteFeature(feature)).unwrap();
    assert_eq!(doc.model_order(), vec![p, s, later.erase()]);
    // Undo brings everything back as it was.
    assert!(history.undo(&mut doc).is_some());
    assert_eq!(doc.model_order(), vec![p, s, later.erase(), f]);
}

#[test]
fn an_imported_assembly_is_a_fixed_reference_that_can_be_placed() {
    let mut doc = Document::new();
    let mut history = History::default();
    let quarter_turn =
        [[0.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 0.0, 0.0, 1.0]];
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.5, 1.0]];
    let assembly = Assembly {
        name: "Machine".into(),
        file: "machine.glb".into(),
        placement: Placement::default(),
        parts: vec![
            AssemblyPart { name: "Frame".into(), parent: None, matrix: quarter_turn, meshes: vec![0] },
            AssemblyPart { name: "Door".into(), parent: Some(0), matrix: identity, meshes: vec![0] },
        ],
        meshes: vec![AssemblyMesh { name: "Plate".into(), color: [0.5, 0.5, 0.5, 1.0], triangles: 12 }],
        look: Default::default(),
    };
    assert!(assembly.is_valid());
    // The door: half a metre up in the frame, which stands 1 m along X.
    let door = assembly.part_matrix(1).unwrap();
    assert_eq!([door[3][0], door[3][1], door[3][2]], [1.0, 0.0, 0.5]);
    let Created::Assembly(id) =
        cell(&mut doc, &mut history, CellOp::AddAssembly(assembly.clone())).unwrap()[0]
    else {
        panic!()
    };
    let placement = Placement { xyz: [2.0, 0.0, 0.0], rpy: [0.0; 3] };
    cell(&mut doc, &mut history, CellOp::PlaceAssembly { id, placement }).unwrap();
    assert_eq!(doc.get(id).unwrap().placement, placement);
    assert!(doc.model_order().contains(&id.erase()));

    // A child before its parent, or a missing mesh, is refused.
    let mut broken = assembly.clone();
    broken.parts[0].parent = Some(1);
    assert!(cell(&mut doc, &mut history, CellOp::AddAssembly(broken)).is_err());
    let mut broken = assembly;
    broken.parts[1].meshes = vec![3];
    assert!(cell(&mut doc, &mut history, CellOp::AddAssembly(broken)).is_err());
}

#[test]
fn items_are_renamed_shown_hidden_and_their_dependents_listed() {
    let mut doc = Document::new();
    let mut history = History::default();
    let (sketch, curves) = sketch_with_rectangle(&mut doc, &mut history);
    let point_id = *doc.get(sketch).unwrap().points.keys().next().unwrap();
    let Created::WorkFeature(point) = cell(
        &mut doc,
        &mut history,
        CellOp::AddWorkFeature(work(
            "Point1",
            WorkGeometry::Point { at: PointRef::SketchPoint { sketch, point: point_id } },
        )),
    )
    .unwrap()[0] else {
        panic!()
    };
    let Created::WorkFeature(axis) = cell(
        &mut doc,
        &mut history,
        CellOp::AddWorkFeature(work(
            "Axis1",
            WorkGeometry::AxisNormalToPlane {
                plane: PlaneRef::Origin(OriginPlane::Xy),
                through: PointRef::Work(point),
            },
        )),
    )
    .unwrap()[0] else {
        panic!()
    };
    let Created::Feature(feature) =
        cell(&mut doc, &mut history, CellOp::AddFeature(extrude(sketch, curves, Extent::Distance(0.1))))
            .unwrap()[0]
    else {
        panic!()
    };

    // Rename: trimmed, one undo step; empty or taken names are refused.
    run(&mut doc, &mut history, Command::Rename { item: point.erase(), name: "  Corner ".into() }).unwrap();
    assert_eq!(doc.get(point).unwrap().name, "Corner");
    assert!(matches!(
        run(&mut doc, &mut history, Command::Rename { item: axis.erase(), name: "Corner".into() }),
        Err(EditError::NameTaken)
    ));
    assert!(run(&mut doc, &mut history, Command::Rename { item: axis.erase(), name: "   ".into() }).is_err());
    run(&mut doc, &mut history, Command::Rename { item: sketch.erase(), name: "Profile".into() }).unwrap();
    assert_eq!(doc.get(sketch).unwrap().name, "Profile");
    assert!(history.undo(&mut doc).is_some());
    assert_eq!(doc.get(sketch).unwrap().name, "Sketch1");

    // Visibility of any item is stored and read back the same way.
    assert!(doc.is_visible(sketch.erase()));
    run(&mut doc, &mut history, Command::SetVisible { item: sketch.erase(), visible: false }).unwrap();
    assert!(!doc.is_visible(sketch.erase()) && doc.is_visible(feature.erase()));
    run(&mut doc, &mut history, Command::SetVisible { item: sketch.erase(), visible: true }).unwrap();
    assert!(doc.is_visible(sketch.erase()));

    // What depends on the sketch: the point on it and the extrusion directly, the axis through the point.
    let dependents = doc.dependents_of(sketch.erase());
    assert_eq!(dependents.len(), 3);
    assert!(dependents.contains(&point.erase()) && dependents.contains(&feature.erase()));
    assert_eq!(*dependents.last().unwrap(), axis.erase(), "indirect dependents come after direct ones");
    assert!(doc.dependents_of(axis.erase()).is_empty());

    // The rollback marker's position in the tree order.
    assert_eq!(doc.rollback_index(), None);
    cell(&mut doc, &mut history, CellOp::SetRollback { after: Some(point.erase()) }).unwrap();
    assert_eq!(doc.rollback_index(), Some(1));
}

fn machine() -> Assembly {
    let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    Assembly {
        name: "Machine".into(),
        file: "machine.glb".into(),
        placement: Placement::default(),
        parts: vec![
            AssemblyPart { name: "Frame".into(), parent: None, matrix: identity, meshes: vec![0] },
            AssemblyPart { name: "Door".into(), parent: Some(0), matrix: identity, meshes: vec![0] },
            AssemblyPart { name: "Table".into(), parent: None, matrix: identity, meshes: vec![0] },
        ],
        meshes: vec![AssemblyMesh { name: "Plate".into(), color: [0.5, 0.5, 0.5, 1.0], triangles: 12 }],
        look: Default::default(),
    }
}

#[test]
fn the_first_component_is_grounded_and_component_states_switch_in_one_undo_step() {
    use gc_doc::{ComponentFlag, Occurrence};
    let mut doc = Document::new();
    let mut history = History::default();
    let Created::Assembly(first) = cell(&mut doc, &mut history, CellOp::AddAssembly(machine())).unwrap()[0]
    else {
        panic!()
    };
    let Created::Assembly(second) = cell(&mut doc, &mut history, CellOp::AddAssembly(machine())).unwrap()[0]
    else {
        panic!()
    };
    let item = |id: gc_core::Id<Assembly>| Occurrence::Item(id.erase());
    assert!(doc.component_flag(item(first), ComponentFlag::Grounded), "the first component is grounded");
    assert!(!doc.component_flag(item(second), ComponentFlag::Grounded), "the others are free");

    // The tree: each assembly followed by its parts, the door under the frame.
    let tree = doc.components();
    assert_eq!(tree.len(), 8);
    let door = Occurrence::Part { assembly: first, index: 1 };
    let frame = Occurrence::Part { assembly: first, index: 0 };
    let node = tree.iter().find(|n| n.occurrence == door).unwrap();
    assert_eq!((node.name.as_str(), node.parent), ("Door", Some(frame)));
    assert!(node.visible && node.enabled && !node.suppressed && !node.grounded);

    // Each switch is one undo step.
    for flag in
        [ComponentFlag::Grounded, ComponentFlag::Enabled, ComponentFlag::Visible, ComponentFlag::Suppressed]
    {
        let before = doc.component_flag(door, flag);
        let depth = history.undo_depth();
        run(&mut doc, &mut history, Command::SetComponent { occurrence: door, flag, on: !before }).unwrap();
        assert_eq!(doc.component_flag(door, flag), !before, "{flag:?}");
        assert_eq!(history.undo_depth(), depth + 1);
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.component_flag(door, flag), before, "{flag:?} undone");
    }

    // A hidden frame hides its door from the drawing but both stay obstacles; suppressed, neither counts.
    run(
        &mut doc,
        &mut history,
        Command::SetComponent { occurrence: frame, flag: ComponentFlag::Visible, on: false },
    )
    .unwrap();
    assert_eq!(doc.parts_shown(first, false), vec![false, false, true]);
    assert_eq!(doc.parts_shown(first, true), vec![true, true, true]);
    run(
        &mut doc,
        &mut history,
        Command::SetComponent { occurrence: frame, flag: ComponentFlag::Suppressed, on: true },
    )
    .unwrap();
    assert_eq!(doc.parts_shown(first, true), vec![false, false, true]);

    // Whole assemblies are shown, hidden and suppressed through their own states.
    run(
        &mut doc,
        &mut history,
        Command::SetComponent { occurrence: item(second), flag: ComponentFlag::Suppressed, on: true },
    )
    .unwrap();
    assert!(doc.is_inactive(second.erase()));
    // A part that does not exist is refused.
    let missing = Occurrence::Part { assembly: first, index: 9 };
    assert!(
        run(
            &mut doc,
            &mut history,
            Command::SetComponent { occurrence: missing, flag: ComponentFlag::Grounded, on: true }
        )
        .is_err()
    );

    // Undoing the first import takes its grounding with it.
    let mut doc = Document::new();
    let mut history = History::default();
    cell(&mut doc, &mut history, CellOp::AddAssembly(machine())).unwrap();
    history.undo(&mut doc).unwrap();
    assert!(doc.timeline().is_none_or(|(_, t)| t.components.grounded.is_empty()));
}

/// Work axes (Inventor 2026 help, To Create Work Features, GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385): On
/// Line or Edge, Parallel to Line Through Point, Intersection of Two Planes, Through Center of Circular
/// Shape — here on sketch lines and circles.
#[test]
fn work_axes_on_a_line_parallel_through_a_point_between_planes_and_through_a_circle() {
    use gc_doc::{CurveRef, WorkShape};
    use gc_math::DVec3;
    let mut doc = Document::new();
    let mut history = History::default();
    let (sketch, curves) = sketch_with_rectangle(&mut doc, &mut history);
    let circle = run(
        &mut doc,
        &mut history,
        Command::Sketch {
            sketch,
            op: SketchOp::AddCircle {
                centre: [0.6, 0.1, 0.0],
                radius: 0.05,
                centre_snap: Default::default(),
                rim_snap: Default::default(),
            },
        },
    )
    .unwrap()
    .into_iter()
    .find_map(|c| if let Created::Curve(id) = c { Some(id) } else { None })
    .unwrap();
    let bottom = CurveRef { sketch, curve: curves[0] };
    let add = |doc: &mut Document, history: &mut History, geometry| {
        let Created::WorkFeature(id) =
            cell(doc, history, CellOp::AddWorkFeature(work("Axis", geometry))).unwrap()[0]
        else {
            panic!()
        };
        doc.work_shape(id).unwrap()
    };
    let axis = |shape: WorkShape| match shape {
        WorkShape::Axis { point, direction } => (point, direction),
        other => panic!("{other:?}"),
    };
    let near = |a: DVec3, b: DVec3| (a - b).length() < 1e-9;

    let (p, d) = axis(add(&mut doc, &mut history, WorkGeometry::AxisOnLine { line: bottom }));
    assert!(near(d.abs(), DVec3::X) && p.y.abs() < 1e-9 && p.z.abs() < 1e-9);
    let through =
        WorkGeometry::AxisParallelThrough { line: bottom, through: PointRef::Fixed([0.0, 1.0, 2.0]) };
    let (p, d) = axis(add(&mut doc, &mut history, through));
    assert!(near(p, DVec3::new(0.0, 1.0, 2.0)) && near(d.abs(), DVec3::X));
    let planes = WorkGeometry::AxisPlanesIntersection {
        planes: [PlaneRef::Origin(OriginPlane::Xy), PlaneRef::Origin(OriginPlane::Xz)],
    };
    let (p, d) = axis(add(&mut doc, &mut history, planes));
    assert!(near(d.abs(), DVec3::X) && p.y.abs() < 1e-9 && p.z.abs() < 1e-9, "the X axis: {p:?} {d:?}");
    let (p, d) = axis(add(
        &mut doc,
        &mut history,
        WorkGeometry::AxisThroughCircle { curve: CurveRef { sketch, curve: circle } },
    ));
    assert!(near(p, DVec3::new(0.6, 0.1, 0.0)) && near(d.abs(), DVec3::Z));

    // A line where a circle is wanted (and the other way), the same plane twice: refused.
    let bad = [
        WorkGeometry::AxisOnLine { line: CurveRef { sketch, curve: circle } },
        WorkGeometry::AxisThroughCircle { curve: bottom },
        WorkGeometry::AxisPlanesIntersection { planes: [PlaneRef::Origin(OriginPlane::Xy); 2] },
    ];
    for geometry in bad {
        assert!(cell(&mut doc, &mut history, CellOp::AddWorkFeature(work("Bad", geometry))).is_err());
    }
    // The sketch has dependents now.
    assert!(run(&mut doc, &mut history, Command::DeleteSketch { id: sketch }).is_err());
}
