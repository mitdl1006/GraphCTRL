//! Mesh files are read in the background (lead's request): the body is in the document at once (one undo
//! step), its mesh pending until the job is collected; then it is drawn, measured and checked.

#![allow(clippy::unwrap_used, reason = "test code")]

use gc_doc::{Placement, Solid};
use gc_session::{CellItem, Session};

#[test]
fn a_mesh_body_is_there_at_once_and_its_mesh_arrives_with_poll_jobs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("part.stl");
    let mesh = gc_mesh::primitives::cylinder(0.2, 0.5, 24).unwrap();
    std::fs::write(&path, gc_mesh::stl::to_binary(&mesh)).unwrap();
    let (executor, queue) = gc_jobs::Executor::manual();
    let mut session = Session::new();
    session.set_executor(executor);
    let depth = session.history().undo_depth();
    let solid = Solid::Mesh { file: path.to_string_lossy().into_owned(), scale: 1.0 };
    let body = session.add_body(solid.clone(), Placement::default()).unwrap();
    assert_eq!(session.history().undo_depth(), depth + 1, "one undo step");
    assert!(session.doc().get(body).is_some());
    assert!(session.mesh_pending(&solid), "placeholder while loading");
    assert_eq!(session.meshes_loading(), 1);
    assert!(session.body_mesh(&solid).is_none());
    assert!(session.item_geometry(CellItem::Body(body)).is_none());

    queue.run_all();
    assert!(session.poll_jobs(), "something arrived");
    assert!(!session.mesh_pending(&solid));
    assert_eq!(session.body_mesh(&solid).unwrap().data.triangle_count(), 96);
    assert!(session.item_geometry(CellItem::Body(body)).is_some());
    assert_eq!(session.history().undo_depth(), depth + 1, "arriving is not an edit");
    assert!(!session.poll_jobs());

    // A file that cannot be read: not pending, and the reason is kept.
    let missing =
        Solid::Mesh { file: directory.path().join("gone.stl").to_string_lossy().into_owned(), scale: 1.0 };
    session.add_body(missing.clone(), Placement::default()).unwrap();
    queue.run_all();
    session.poll_jobs();
    assert!(!session.mesh_pending(&missing));
    let Solid::Mesh { file, .. } = &missing else { unreachable!() };
    assert!(session.mesh_error(file).is_some());
}
