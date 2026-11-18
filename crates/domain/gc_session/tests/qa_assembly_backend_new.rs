//! QA boundary tests for the newest assembly backend: Replace / Replace All, Mirror with Mirror/Reuse states and
//! where mirror-image meshes are written, Tangent against a cone, the Planar joint, and the edits refused while a
//! positional representation other than [Primary] is active. Inventor 2026 help: Replace assembly components
//! GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD, Mirror Components: Reference GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336,
//! Positional representations in assemblies GUID-C728DFAC-1B45-4879-A004-45A8DC991954, To Place Tangent
//! Constraints GUID-FF270566-CA9E-420A-A00A-F52DD0FDFD8E. The Planar joint has no Inventor source we read.
//! Refused edits leave the document and the history untouched; accepted ones are one undo step that restores
//! exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use std::fmt::Debug;

use gc_core::{AnyId, Id};
use gc_doc::{
    AxisSense, Body, CellOp, Command, EditError, Geometry, Occurrence, Placement, RelationKind, Relationship,
    Replacement, Selection, Solid,
};
use gc_math::DVec3;
use gc_session::{MirrorState, Session};

fn step<T, E: Debug>(s: &mut Session, what: &str, f: impl FnOnce(&mut Session) -> Result<T, E>) -> Option<T> {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match f(s) {
        Err(_) => {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
            None
        }
        Ok(v) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one undo step");
            let after = s.doc().clone();
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "{what}: undo restores");
            assert!(s.redo());
            assert!(s.doc().same_content(&after), "{what}: redo repeats");
            Some(v)
        }
    }
}

fn urdf(name: &str) -> String {
    let path = format!("{}/../../../fixtures/urdf/{name}.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn at(x: f64, y: f64, z: f64) -> Placement {
    Placement { xyz: [x, y, z], rpy: [0.0; 3] }
}

fn on(body: Id<Body>, geometry: Geometry) -> Selection {
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

fn plane(z: f64, up: f64) -> Geometry {
    Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, up] }
}

// ---- Replace ----

#[test]
fn replacing_a_robot_that_cannot_be_replaced_leaves_no_trace() {
    let mut s = Session::new();
    let a = s.add_robot("Arm", &urdf("six_axis")).unwrap();
    let body = s.add_body(Solid::Box { size: [0.1; 3] }, at(1.0, 0.0, 0.0)).unwrap();
    // Text that is no description, no text, a different root element.
    for bad in ["", "   ", "<robot", "<robot name='x'/>", "not xml at all", "<html></html>"] {
        assert!(step(&mut s, bad, |s| s.replace_robot(a, "New", bad, "", false)).is_none(), "{bad:?}");
    }
    // A body is not a robot: replacing it with a robot, a robot with a solid, and a missing id.
    let record = |s: &mut Session| gc_doc::Replacement::Robot(Box::new(s.doc().get(a).unwrap().clone()));
    let robot = record(&mut s);
    assert!(
        step(&mut s, "body by robot", |s| s.replace_component(body.erase(), robot.clone(), false)).is_none()
    );
    let solid = Replacement::Solid(Solid::Box { size: [0.2; 3] });
    assert!(
        step(&mut s, "robot by solid", |s| s.replace_component(a.erase(), solid.clone(), false)).is_none()
    );
    let gone = s.add_body(Solid::Box { size: [0.1; 3] }, at(5.0, 0.0, 0.0)).unwrap();
    s.execute(&Command::Cell(CellOp::DeleteBody(gone))).unwrap();
    assert!(step(&mut s, "deleted", |s| s.replace_component(gone.erase(), solid.clone(), false)).is_none());
    let sketch = s.create_sketch("XY").unwrap().erase();
    assert!(step(&mut s, "sketch", |s| s.replace_component(sketch, solid.clone(), false)).is_none());
    // A solid that is no solid.
    for bad in [
        Solid::Box { size: [0.0, 0.1, 0.1] },
        Solid::Box { size: [-1.0, 0.1, 0.1] },
        Solid::Box { size: [f64::NAN, 0.1, 0.1] },
        Solid::Cylinder { radius: 0.0, length: 0.1 },
        Solid::Sphere { radius: f64::INFINITY },
    ] {
        let r = Replacement::Solid(bad.clone());
        assert!(
            step(&mut s, "bad solid", |s| s.replace_component(body.erase(), r.clone(), false)).is_none(),
            "{bad:?}"
        );
    }
}

#[test]
fn replace_all_is_all_or_nothing_and_one_undo_step_for_every_robot() {
    let mut s = Session::new();
    let a = s.add_robot("A", &urdf("six_axis")).unwrap();
    let b = s.add_robot("B", &urdf("six_axis")).unwrap();
    let c = s.add_robot("C", &urdf("six_axis")).unwrap();
    s.execute(&Command::Cell(CellOp::PlaceRobot { id: b, placement: at(0.5, 0.0, 0.0) })).unwrap();
    s.execute(&Command::Cell(CellOp::PlaceRobot {
        id: c,
        placement: Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0, 0.0, 1.0] },
    }))
    .unwrap();
    s.jog(b, &[0.3, -0.2, 0.1, 0.0, 0.4, 0.0]);
    s.end_jog();
    // One of the three has a waypoint in joint values: a seven-joint replacement is refused for all of them.
    s.add_waypoint(c, gc_doc::WaypointTarget::Joints(vec![0.1; 6])).unwrap();
    assert!(
        step(&mut s, "replace all by seven", |s| s.replace_robot(a, "Seven", &urdf("seven_axis"), "", true))
            .is_none()
    );
    for id in [a, b, c] {
        assert_eq!(
            s.doc().get(id).unwrap().joints.len(),
            6,
            "a robot changed although the replacement was refused"
        );
    }
    // The same-joint replacement is accepted for all three in one step; each keeps its place and its joints.
    let before: Vec<_> = [a, b, c].iter().map(|id| s.doc().get(*id).unwrap().clone()).collect();
    assert!(
        step(&mut s, "replace all", |s| s.replace_robot(a, "UR5e", &urdf("ur5e_kinematics"), "", true))
            .is_some()
    );
    for (id, old) in [a, b, c].iter().zip(&before) {
        let now = s.doc().get(*id).unwrap();
        assert_eq!(now.name, "UR5e");
        assert_eq!(now.placement, old.placement);
        assert_eq!(now.joints, old.joints);
        assert_eq!(now.tcp, old.tcp);
    }
    // Replacing one robot with Replace (not All) leaves the others alone.
    assert!(
        step(&mut s, "replace one", |s| s.replace_robot(b, "Mine", &urdf("six_axis"), "", false)).is_some()
    );
    assert_eq!(s.doc().get(b).unwrap().name, "Mine");
    assert_eq!(s.doc().get(a).unwrap().name, "UR5e");
    // Saved and opened: the replaced description is what is stored.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
}

#[test]
fn replacing_a_body_by_a_shape_keeps_its_name_and_place_and_replace_all_takes_the_same_source_only() {
    let mut s = Session::new();
    let place = Placement { xyz: [0.2, 0.1, 0.3], rpy: [0.1, 0.2, 0.3] };
    let a = s.add_body(Solid::Box { size: [0.1; 3] }, place).unwrap();
    let b = s.add_body(Solid::Box { size: [0.1; 3] }, at(1.0, 0.0, 0.0)).unwrap();
    let other = s.add_body(Solid::Box { size: [0.2; 3] }, at(2.0, 0.0, 0.0)).unwrap();
    s.rename(a.erase(), "Keep me").unwrap();
    let with = Replacement::Solid(Solid::Cylinder { radius: 0.05, length: 0.2 });
    assert!(step(&mut s, "replace one", |s| s.replace_component(a.erase(), with.clone(), false)).is_some());
    let now = s.doc().get(a).unwrap();
    assert_eq!((now.name.as_str(), now.placement), ("Keep me", place));
    assert!(matches!(now.solid, Solid::Cylinder { .. }));
    assert!(matches!(s.doc().get(b).unwrap().solid, Solid::Box { .. }));
    // Replace All on a box takes the other same-size box and not the bigger one (a is a cylinder now).
    assert!(step(&mut s, "replace all", |s| s.replace_component(b.erase(), with.clone(), true)).is_some());
    assert!(matches!(s.doc().get(b).unwrap().solid, Solid::Cylinder { .. }));
    assert!(matches!(s.doc().get(other).unwrap().solid, Solid::Box { .. }), "a different source stayed");
    // Replacing with the same shape is allowed or refused, never half done.
    let same = Replacement::Solid(Solid::Box { size: [0.2; 3] });
    step(&mut s, "replace by the same", |s| s.replace_component(other.erase(), same.clone(), false));
}

// ---- Mirror ----

fn mirror_plane(body: Id<Body>) -> Selection {
    on(body, Geometry::Plane { point: [0.0; 3], normal: [1.0, 0.0, 0.0] })
}

#[test]
fn mirror_states_mix_in_one_step_and_a_robot_is_reported_not_reflected() {
    let mut s = Session::new();
    let floor = s.add_body(Solid::Box { size: [4.0, 4.0, 0.1] }, at(0.0, 0.0, -0.05)).unwrap();
    let a = s.add_body(Solid::Box { size: [0.1; 3] }, at(0.5, 0.2, 0.05)).unwrap();
    let b = s.add_body(Solid::Cylinder { radius: 0.05, length: 0.2 }, at(0.8, -0.2, 0.1)).unwrap();
    let robot = s.add_robot("Arm", &urdf("six_axis")).unwrap();
    s.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement: at(1.0, 0.0, 0.0) })).unwrap();
    let plane = mirror_plane(floor);
    let bodies = |s: &Session| s.doc().bodies().iter().count();
    let robots = |s: &Session| s.doc().robots().iter().count();
    let (nb, nr) = (bodies(&s), robots(&s));
    let items = [
        (a.erase(), MirrorState::Mirror),
        (b.erase(), MirrorState::Reuse),
        (robot.erase(), MirrorState::Mirror),
    ];
    let outcome =
        step(&mut s, "mixed mirror", |s| s.mirror_components_mixed(&items, plane, false, false)).unwrap();
    assert_eq!(bodies(&s), nb + 2);
    assert_eq!(robots(&s), nr + 1);
    assert_eq!(outcome.not_reflected, [robot.erase()], "a robot cannot be mirrored as an image");
    // The same list twice in the call, an empty list, a plane that is no plane, a sketch id, a deleted id.
    let twice = [(a.erase(), MirrorState::Reuse), (a.erase(), MirrorState::Reuse)];
    step(&mut s, "same item twice", |s| s.mirror_components_mixed(&twice, plane, false, false));
    assert!(step(&mut s, "empty", |s| s.mirror_components_mixed(&[], plane, false, false)).is_none());
    let not_plane = on(floor, Geometry::Axis { point: [0.0; 3], direction: [1.0, 0.0, 0.0] });
    let one = [(a.erase(), MirrorState::Reuse)];
    assert!(
        step(&mut s, "axis as plane", |s| s.mirror_components_mixed(&one, not_plane, false, false)).is_none()
    );
    let nan_plane = on(floor, Geometry::Plane { point: [0.0; 3], normal: [f64::NAN, 0.0, 0.0] });
    assert!(
        step(&mut s, "NaN plane", |s| s.mirror_components_mixed(&one, nan_plane, false, false)).is_none()
    );
    let zero_plane = on(floor, Geometry::Plane { point: [0.0; 3], normal: [0.0; 3] });
    assert!(
        step(&mut s, "zero normal", |s| s.mirror_components_mixed(&one, zero_plane, false, false)).is_none()
    );
    let sketch = s.create_sketch("XY").unwrap().erase();
    let bad = [(sketch, MirrorState::Reuse)];
    assert!(step(&mut s, "sketch", |s| s.mirror_components_mixed(&bad, plane, false, false)).is_none());
    // A list with one good and one bad item is all or nothing.
    let gone: AnyId = {
        let g = s.add_body(Solid::Box { size: [0.1; 3] }, at(9.0, 0.0, 0.0)).unwrap();
        s.execute(&Command::Cell(CellOp::DeleteBody(g))).unwrap();
        g.erase()
    };
    let half = [(a.erase(), MirrorState::Reuse), (gone, MirrorState::Reuse)];
    assert!(
        step(&mut s, "good and bad", |s| s.mirror_components_mixed(&half, plane, false, false)).is_none()
    );
}

fn wedge_file(dir: &std::path::Path) -> std::path::PathBuf {
    // A small solid wedge, not its own mirror image.
    let positions = vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]];
    let indices = vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
    let mesh = gc_mesh::MeshData::new(positions, indices).unwrap();
    let path = dir.join("wedge.stl");
    std::fs::write(&path, gc_mesh::stl::to_binary(&mesh)).unwrap();
    path
}

#[test]
fn a_mirror_image_goes_into_the_projects_files_folder_once_and_travels_with_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let source = wedge_file(dir.path());
    let mut s = Session::new();
    let floor = s.add_body(Solid::Box { size: [4.0, 4.0, 0.1] }, at(0.0, 0.0, -0.05)).unwrap();
    let part = s
        .add_body(Solid::Mesh { file: source.to_str().unwrap().to_owned(), scale: 0.1 }, at(0.5, 0.2, 0.0))
        .unwrap();
    let cache = dir.path().join("cache");
    s.set_asset_cache(Some(cache.clone()));
    // Saved first: the mirror image is written next to the project, not into the cache.
    let project = dir.path().join("work");
    std::fs::create_dir_all(&project).unwrap();
    let path = project.join("cell.gctrl");
    s.save_as(&path).unwrap();
    let plane = mirror_plane(floor);
    s.mirror_components_reflected(&[part.erase()], plane, false, false).unwrap();
    assert!(!cache.exists() || std::fs::read_dir(&cache).unwrap().count() == 0, "nothing goes to the cache");
    let mirrored_files = |root: &std::path::Path| -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.file_name().unwrap().to_string_lossy().contains(".mirror-") {
                    found.push(p);
                }
            }
        }
        found
    };
    let first = mirrored_files(&project);
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(first[0].to_string_lossy().contains("cell_files"), "{:?}", first[0]);
    // Asking again (and after Undo) writes no second file.
    assert!(s.undo());
    s.mirror_components_reflected(&[part.erase()], plane, false, false).unwrap();
    s.mirror_components_reflected(&[part.erase()], plane, false, false).unwrap();
    assert_eq!(mirrored_files(&project).len(), 1, "one mirror image, found again by its name");
    // The same part across another plane is another image, named with its own axis.
    let up = on(floor, Geometry::Plane { point: [0.0; 3], normal: [0.0, 1.0, 0.0] });
    s.mirror_components_reflected(&[part.erase()], up, false, false).unwrap();
    assert_eq!(mirrored_files(&project).len(), 2);
    // Save, move the whole project folder, open: the mesh files come along (paths are relative).
    s.save().unwrap();
    let moved = dir.path().join("moved");
    std::fs::rename(&project, &moved).unwrap();
    let mut t = Session::new();
    t.open(&moved.join("cell.gctrl")).unwrap_or_else(|e| panic!("a moved project must open: {e:?}"));
    let meshes = t.doc().bodies().iter().filter(|(_, b)| matches!(b.solid, Solid::Mesh { .. })).count();
    assert_eq!(meshes, 4, "the source and three copies");
    for (_, b) in t.doc().bodies().iter() {
        if let Solid::Mesh { file, .. } = &b.solid {
            assert!(std::path::Path::new(file).exists() || moved.join(file).exists(), "missing {file}");
        }
    }
}

// ---- Tangent against a cone ----

fn cone_sel(apex_z: f64, down: f64, half: f64) -> Geometry {
    Geometry::Cone { apex: [0.0, 0.0, apex_z], direction: [0.0, 0.0, down], half_angle: half }
}

fn world_cone(s: &Session, sel: &Selection) -> (DVec3, DVec3) {
    s.doc().selection_axis(sel).unwrap()
}

#[test]
fn a_cone_tangent_to_a_plane_touches_it_along_a_line_wherever_the_parts_stand() {
    for rpy in [[0.0; 3], [0.4, -0.3, 0.9], [1.2, 0.5, -2.0]] {
        let mut s = Session::new();
        let table = s.add_body(Solid::Box { size: [2.0, 2.0, 0.2] }, at(0.0, 0.0, 0.0)).unwrap();
        let cone =
            s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.3, 0.1, 0.8], rpy }).unwrap();
        let half = 0.35;
        let first = on(cone, cone_sel(0.0, 1.0, half));
        let second = on(table, plane(0.1, 1.0));
        let rel =
            Relationship::new("Tangent", RelationKind::Tangent { offset: 0.0, inside: false }, first, second);
        let id = step(&mut s, "cone tangent", |s| s.add_relationship(rel.clone()));
        let Some(id) = id else { continue };
        let (apex, axis) = world_cone(&s, &first);
        let (_, normal) = s.doc().selection_axis(&second).unwrap();
        let apex = s.doc().selection_point(&first).unwrap_or(apex);
        let height = (apex - DVec3::new(0.0, 0.0, 0.1)).dot(normal);
        assert!(height.abs() < 1e-6, "rpy {rpy:?}: the apex is {height} off the plane");
        let tilt = axis.dot(normal).abs();
        assert!((tilt - half.sin()).abs() < 1e-5, "rpy {rpy:?}: the axis is {tilt} against {}", half.sin());
        assert!(s.doc().get(id).is_some());
    }
}

#[test]
fn a_cone_that_is_no_cone_makes_no_relationship() {
    let mut s = Session::new();
    let table = s.add_body(Solid::Box { size: [2.0, 2.0, 0.2] }, at(0.0, 0.0, 0.0)).unwrap();
    let part = s.add_body(Solid::Box { size: [0.1; 3] }, at(0.3, 0.1, 0.8)).unwrap();
    let pi = std::f64::consts::PI;
    for (half, dir) in [
        (0.0, 1.0),
        (-0.3, 1.0),
        (pi / 2.0, 1.0),
        (pi, 1.0),
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (0.3, 0.0),
        (0.3, f64::NAN),
    ] {
        let rel = Relationship::new(
            "Tangent",
            RelationKind::Tangent { offset: 0.0, inside: false },
            on(part, cone_sel(0.0, dir, half)),
            on(table, plane(0.1, 1.0)),
        );
        assert!(
            step(&mut s, "bad cone", |s| s.add_relationship(rel.clone())).is_none(),
            "half {half} dir {dir}"
        );
    }
    // A cone against a cone, or a cone with a plane through Mate: each is placed or refused whole.
    let other = s.add_body(Solid::Box { size: [0.1; 3] }, at(0.0, 1.0, 0.5)).unwrap();
    let both = Relationship::new(
        "Tangent",
        RelationKind::Tangent { offset: 0.0, inside: false },
        on(part, cone_sel(0.0, 1.0, 0.3)),
        on(other, cone_sel(0.0, -1.0, 0.3)),
    );
    step(&mut s, "cone against cone", |s| s.add_relationship(both.clone()));
    let mate = Relationship::new(
        "Mate",
        RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
        on(part, cone_sel(0.0, 1.0, 0.3)),
        on(table, plane(0.1, 1.0)),
    );
    step(&mut s, "cone mated to a plane", |s| s.add_relationship(mate.clone()));
}

#[test]
fn a_cone_tangent_to_a_sphere_stays_a_sphere_radius_from_the_surface() {
    let mut s = Session::new();
    let ball = s.add_body(Solid::Sphere { radius: 0.1 }, at(0.0, 0.0, 0.0)).unwrap();
    let part = s
        .add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [0.5, 0.2, 0.6], rpy: [0.3, 0.2, 0.1] })
        .unwrap();
    let half = 0.4;
    let cone = on(part, cone_sel(0.0, 1.0, half));
    let sphere = on(ball, Geometry::Sphere { centre: [0.0; 3], radius: 0.1 });
    let rel =
        Relationship::new("Tangent", RelationKind::Tangent { offset: 0.0, inside: false }, cone, sphere);
    if step(&mut s, "cone tangent to sphere", |s| s.add_relationship(rel.clone())).is_some() {
        let (axis_point, axis) = s.doc().selection_axis(&cone).unwrap();
        let apex = s.doc().selection_point(&cone).unwrap_or(axis_point);
        let centre = s.doc().selection_point(&sphere).unwrap();
        let v = centre - apex;
        let along = v.dot(axis);
        let perp = (v - axis * along).length();
        let distance = (perp * half.cos() - along * half.sin()).abs();
        assert!((distance - 0.1).abs() < 1e-5, "the sphere is {distance} from the cone's surface");
    }
}

// ---- Planar joint ----

fn frame(origin: [f64; 3], z: [f64; 3]) -> Geometry {
    Geometry::Frame { origin, z, x: [1.0, 0.0, 0.0] }
}

#[test]
fn a_planar_joint_holds_the_height_and_the_tilt_and_leaves_the_slide_and_the_turn_free() {
    for flip in [false, true] {
        let mut s = Session::new();
        let table = s.add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, at(0.0, 0.0, 0.5)).unwrap();
        let tilted = Placement { xyz: [0.3, 0.2, 1.4], rpy: [0.3, -0.2, 0.7] };
        let block = s.add_body(Solid::Box { size: [0.2, 0.2, 0.1] }, tilted).unwrap();
        let z_block = if flip { 1.0 } else { -1.0 };
        let rel = Relationship::new(
            "Planar",
            RelationKind::Planar { flip },
            on(block, frame([0.0, 0.0, -0.05], [0.0, 0.0, z_block])),
            on(table, frame([0.0, 0.0, 0.5], [0.0, 0.0, 1.0])),
        );
        let id = step(&mut s, "planar", |s| s.add_relationship(rel.clone())).unwrap();
        let first = Selection {
            occurrence: Occurrence::Item(block.erase()),
            geometry: frame([0.0, 0.0, -0.05], [0.0, 0.0, z_block]),
        };
        let (origin, normal) = s.doc().selection_axis(&first).unwrap();
        let _ = origin;
        let point = s.doc().selection_point(&first).unwrap();
        assert!((point.z - 1.0).abs() < 1e-6, "flip {flip}: the joint origin is {} above nothing", point.z);
        assert!((normal.z.abs() - 1.0).abs() < 1e-6, "flip {flip}: the tilt is held");
        assert_eq!(normal.z > 0.0, flip, "the axes are opposed unless flipped");
        // Sliding and turning stay free: the block did not jump sideways when the joint was placed.
        let placed = s.doc().get(block).unwrap().placement;
        assert!((placed.xyz[0] - 0.3).abs() < 0.2 && (placed.xyz[1] - 0.2).abs() < 0.2, "{placed:?}");
        // Free Move slides it on the table and Update keeps it there (it is still satisfied).
        s.free_move(
            block.erase(),
            Placement { xyz: [placed.xyz[0] + 0.3, placed.xyz[1] - 0.1, placed.xyz[2]], rpy: placed.rpy },
        )
        .unwrap();
        assert!(!s.needs_update(), "a slide along the plane keeps the joint satisfied");
        // Limits, which no plane joint has, are refused with no trace.
        let limits = gc_doc::JointRange { start: -1.0, end: 1.0, current: None };
        assert!(
            step(&mut s, "planar limits", |s| {
                s.execute(&Command::Cell(CellOp::SetJointLimits { id, angular: Some(limits), linear: None }))
                    .map(|_| ())
            })
            .is_none()
        );
        // Saved and opened.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("planar.gcp");
        s.save_as(&path).unwrap();
        let mut t = Session::new();
        t.open(&path).unwrap();
        assert!(t.doc().same_content(s.doc()));
        assert!(matches!(t.doc().get(id).unwrap().kind, RelationKind::Planar { flip: f } if f == flip));
    }
}

// ---- Primary only ----

fn into_position(s: &mut Session) {
    s.add_positional_representation("Open").unwrap();
    s.activate_positional_representation(Some(0)).unwrap();
}

#[test]
fn while_a_positional_representation_is_active_the_component_list_cannot_change_but_moving_still_can() {
    let mut s = Session::new();
    let floor = s.add_body(Solid::Box { size: [4.0, 4.0, 0.1] }, at(0.0, 0.0, -0.05)).unwrap();
    let a = s.add_body(Solid::Box { size: [0.1; 3] }, at(0.5, 0.2, 0.05)).unwrap();
    let robot = s.add_robot("Arm", &urdf("six_axis")).unwrap();
    let pattern_layout = gc_doc::PatternLayout::Rectangular {
        column: gc_doc::PatternDirection {
            along: on(floor, Geometry::origin(gc_doc::OriginElement::Axis(0))),
            flip: false,
            count: 3,
            spacing: 0.3,
        },
        row: None,
    };
    s.add_pattern("Row", &[a.erase()], pattern_layout).unwrap();
    let feature = || gc_doc::WorkFeature {
        name: "WP".into(),
        geometry: gc_doc::WorkGeometry::PlaneOffset {
            base: gc_doc::PlaneRef::Origin(gc_doc::OriginPlane::Xy),
            distance: 0.1,
        },
        visible: true,
    };
    into_position(&mut s);
    let primary_only = |r: Result<(), EditError>, what: &str| {
        assert!(matches!(r, Err(EditError::PrimaryOnly)), "{what}: {r:?}");
    };
    macro_rules! refused {
        ($what:expr, $call:expr) => {{
            let before = s.doc().clone();
            let depth = s.history().undo_depth();
            primary_only($call, $what);
            assert!(s.doc().same_content(&before), "{} changed the document", $what);
            assert_eq!(s.history().undo_depth(), depth, "{}", $what);
        }};
    }
    refused!("add body", s.add_body(Solid::Box { size: [0.1; 3] }, at(0.0, 0.0, 0.0)).map(|_| ()));
    refused!("delete body", s.execute(&Command::Cell(CellOp::DeleteBody(a))).map(|_| ()));
    refused!("delete robot", s.execute(&Command::Cell(CellOp::DeleteRobot(robot))).map(|_| ()));
    refused!("work feature", s.execute(&Command::Cell(CellOp::AddWorkFeature(feature()))).map(|_| ()));
    refused!("pattern", s.add_pattern("Two", &[a.erase()], pattern_layout));
    refused!("mirror", s.mirror_components(&[a.erase()], mirror_plane(floor), false, false).map(|_| ()));
    refused!("copy", s.copy_components(&[a.erase()], at(0.5, 0.0, 0.0), false, false).map(|_| ()));
    refused!(
        "replace",
        s.replace_component(a.erase(), Replacement::Solid(Solid::Box { size: [0.2; 3] }), false).map(|_| ())
    );
    refused!(
        "delete the pattern",
        s.execute(&Command::Cell(CellOp::DeletePattern { index: 0, keep_copies: true })).map(|_| ())
    );
    let bigger = gc_doc::PatternLayout::Rectangular {
        column: gc_doc::PatternDirection {
            along: on(floor, Geometry::origin(gc_doc::OriginElement::Axis(0))),
            flip: false,
            count: 5,
            spacing: 0.3,
        },
        row: None,
    };
    refused!(
        "a pattern with more copies",
        s.execute(&Command::Cell(CellOp::SetPattern { index: 0, layout: bigger })).map(|_| ())
    );
    // What does not change the list still works: moving, naming, hiding, grounding, a pattern's spacing.
    assert!(s.free_move(a.erase(), at(0.6, 0.2, 0.05)).is_ok());
    assert!(s.rename(a.erase(), "Renamed").is_ok());
    assert!(s.set_visible(a.erase(), false).is_ok());
    assert!(s.set_component(Occurrence::Item(a.erase()), gc_doc::ComponentFlag::Grounded, true).is_ok());
    let wider = gc_doc::PatternLayout::Rectangular {
        column: gc_doc::PatternDirection {
            along: on(floor, Geometry::origin(gc_doc::OriginElement::Axis(0))),
            flip: false,
            count: 3,
            spacing: 0.4,
        },
        row: None,
    };
    assert!(s.execute(&Command::Cell(CellOp::SetPattern { index: 0, layout: wider })).is_ok());
    // Back to [Primary]: every one of them works again.
    s.activate_positional_representation(None).unwrap();
    assert!(s.add_body(Solid::Box { size: [0.1; 3] }, at(0.0, 0.0, 0.0)).is_ok());
    assert!(s.execute(&Command::Cell(CellOp::AddWorkFeature(feature()))).is_ok());
    assert!(s.copy_components(&[a.erase()], at(0.5, 0.0, 0.0), false, false).is_ok());
    // Undo across the switch works and never leaves the representation active with an edit half done.
    while s.undo() {}
    while s.redo() {}
}
