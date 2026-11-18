//! Mirror Components with true mirror images (Inventor 2026 help, Mirror Components: Reference,
//! GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336, status Mirror): a mesh body's copy points to a reflected
//! mesh file written next to the source.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_doc::{Geometry, Occurrence, Placement, Selection, Solid};
use gc_math::{DVec3, pose_from_xyz_rpy};
use gc_session::Session;

/// A wedge with no symmetry: corners at the origin, 3 along X, 2 along Y, 1 along Z.
fn wedge() -> gc_mesh::MeshData {
    let positions = vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]];
    // Outward windings.
    let indices = vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
    gc_mesh::MeshData::new(positions, indices).unwrap()
}

/// Six times the signed volume: positive when the triangles face outward.
fn volume(mesh: &gc_mesh::MeshData) -> f64 {
    mesh.triangles().map(|[a, b, c]| a.dot(b.cross(c))).sum()
}

fn world_corners(session: &Session, body: gc_core::Id<gc_doc::Body>) -> Vec<DVec3> {
    let body = session.doc().get(body).unwrap();
    let Solid::Mesh { file, scale } = &body.solid else { panic!() };
    let mesh = gc_mesh::stl::parse(&std::fs::read(file).unwrap()).unwrap();
    let pose = pose_from_xyz_rpy(body.placement.xyz, body.placement.rpy);
    let mut corners: Vec<DVec3> = mesh
        .positions()
        .iter()
        .map(|p| {
            let local = DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])) * *scale;
            pose.translation + pose.rotation * local
        })
        .collect();
    corners.sort_by(|a, b| a.to_array().partial_cmp(&b.to_array()).unwrap());
    corners
}

#[test]
fn a_mirrored_mesh_body_is_a_true_mirror_image() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("mirror_reflected");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("wedge.stl");
    std::fs::write(&source, gc_mesh::stl::to_binary(&wedge())).unwrap();

    let mut session = Session::new();
    let floor = session
        .add_body(Solid::Box { size: [4.0, 4.0, 0.1] }, Placement { xyz: [0.0, 0.0, -0.05], rpy: [0.0; 3] })
        .unwrap();
    let solid = Solid::Mesh { file: source.to_str().unwrap().to_owned(), scale: 0.1 };
    let placement = Placement { xyz: [0.5, 0.2, 0.0], rpy: [0.0, 0.0, 0.3] };
    let part = session.add_body(solid, placement).unwrap();
    let plane = Selection {
        occurrence: Occurrence::Item(floor.erase()),
        geometry: Geometry::Plane { point: [0.0; 3], normal: [1.0, 0.0, 0.0] },
    };
    let bodies = |session: &Session| -> Vec<gc_core::Id<gc_doc::Body>> {
        session.doc().bodies().iter().map(|(id, _)| id).collect()
    };
    let depth = session.history().undo_depth();
    let outcome =
        session.mirror_components_reflected(&[part.erase(), floor.erase()], plane, false, false).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1);
    assert!(outcome.not_reflected.is_empty() && outcome.suppressed.is_empty());
    let copy = bodies(&session)
        .into_iter()
        .find(|id| *id != part && matches!(session.doc().get(*id).unwrap().solid, Solid::Mesh { .. }));
    let copy = copy.unwrap();
    let Solid::Mesh { file, scale } = session.doc().get(copy).unwrap().solid.clone() else { panic!() };
    assert_eq!(scale, 0.1);
    assert!(file.contains("wedge.mirror-") && file.ends_with(".stl"), "{file}");
    // The file is the reflected mesh: its triangles still face outward.
    let reflected = gc_mesh::stl::parse(&std::fs::read(&file).unwrap()).unwrap();
    assert!(volume(&wedge()) > 0.0 && volume(&reflected) > 0.0);
    // Every corner of the copy is the mirror image, across x = 0, of a corner of the source.
    let mut expected: Vec<DVec3> =
        world_corners(&session, part).into_iter().map(|p| DVec3::new(-p.x, p.y, p.z)).collect();
    expected.sort_by(|a, b| a.to_array().partial_cmp(&b.to_array()).unwrap());
    let got = world_corners(&session, copy);
    assert_eq!(got.len(), 4);
    for (a, b) in got.iter().zip(&expected) {
        assert!((*a - *b).length() < 1e-6, "{got:?} vs {expected:?}");
    }
    // The plain mirror (the same shape turned round) is not a mirror image of this wedge.
    assert!(session.undo());
    let plain = session.mirror_components(&[part.erase()], plane, false, false);
    assert!(plain.is_ok());
    let turned = bodies(&session).into_iter().rfind(|id| *id != part && *id != floor).unwrap();
    let off = world_corners(&session, turned).iter().zip(&expected).any(|(a, b)| (*a - *b).length() > 1e-3);
    assert!(off);
    // Asking again writes no second file.
    assert!(session.undo());
    session.mirror_components_reflected(&[part.erase()], plane, false, false).unwrap();
    let files = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(files, 2, "the source and one mirror image");

    // A body whose file is gone is placed like the plain mirror and reported.
    let lost = Solid::Mesh { file: dir.join("gone.stl").to_str().unwrap().to_owned(), scale: 1.0 };
    let ghost = session.add_body(lost, Placement { xyz: [1.0, 1.0, 0.0], rpy: [0.0; 3] }).unwrap();
    let outcome = session.mirror_components_reflected(&[ghost.erase()], plane, false, false).unwrap();
    assert_eq!(outcome.not_reflected, [ghost.erase()]);
}
