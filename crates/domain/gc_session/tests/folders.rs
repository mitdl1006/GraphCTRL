//! User-defined browser folders (Inventor 2026 help, Assembly Browser Reference,
//! GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5: "User-defined Folder groups assembly nodes for better browser
//! organization").

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{EditError, FolderOp, Occurrence, Placement, Solid, SubassemblyOp, TreeKey};
use gc_session::Session;

#[test]
fn folders_arrange_the_browser_without_changing_the_assembly() {
    let mut s = Session::new();
    let body = |s: &mut Session, x: f64| {
        let placement = Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] };
        let id = s.add_body(Solid::Box { size: [0.1; 3] }, placement).unwrap();
        (id, TreeKey::Component(Occurrence::Item(id.erase())))
    };
    let ((a_id, a), (_, b), (_, c)) = (body(&mut s, 1.0), body(&mut s, 2.0), body(&mut s, 3.0));
    let tree = s.component_tree();

    // Add to New Folder: one undo step, a free name, the rows in the order given.
    let depth = s.history().undo_depth();
    let first = s.edit_folders(FolderOp::Add { name: String::new(), members: vec![b, a] }).unwrap().unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    let folders = s.folders();
    assert_eq!((folders[0].name.as_str(), folders[0].members.as_slice()), ("Folder:1", [b, a].as_slice()));
    assert_eq!((s.folder_of(a), s.folder_of(c)), (Some(first), None));
    assert_eq!(s.component_tree(), tree, "the assembly itself is as it was");

    // A row is in one folder: putting it in another takes it out of the first.
    let second =
        s.edit_folders(FolderOp::Add { name: "Fixtures".into(), members: vec![a, c] }).unwrap().unwrap();
    assert_ne!(first, second);
    assert_eq!(s.folders()[0].members, vec![b]);
    assert_eq!(s.folder_of(a), Some(second));
    // Names: not empty, not taken.
    assert_eq!(
        s.edit_folders(FolderOp::Rename { id: second, name: "Folder:1".into() }),
        Err(EditError::NameTaken)
    );
    assert_eq!(s.edit_folders(FolderOp::Rename { id: second, name: "  ".into() }), Err(EditError::BadValue));
    s.edit_folders(FolderOp::Rename { id: second, name: "Jigs".into() }).unwrap();
    assert_eq!(s.folders()[1].name, "Jigs");

    // Dragging out and in.
    s.edit_folders(FolderOp::Move { members: vec![c], into: None }).unwrap();
    assert_eq!(s.folder_of(c), None);
    s.edit_folders(FolderOp::Move { members: vec![c], into: Some(first) }).unwrap();
    assert_eq!(s.folders()[0].members, vec![b, c]);
    assert_eq!(s.edit_folders(FolderOp::Move { members: vec![c], into: Some(99) }), Err(EditError::BadValue));

    // A folder holds rows of one level: a component inside a subassembly cannot join top-level rows.
    s.edit_subassemblies(SubassemblyOp::Demote { members: vec![a], name: String::new() }).unwrap();
    assert_eq!(s.folder_of(a), Some(second), "alone in its folder, the folder is at its level now");
    assert_eq!(
        s.edit_folders(FolderOp::Move { members: vec![a], into: Some(first) }),
        Err(EditError::BadValue)
    );
    assert_eq!(
        s.edit_folders(FolderOp::Add { name: String::new(), members: vec![a, b] }),
        Err(EditError::BadValue)
    );

    // Saved with the document.
    let text = gc_persist::to_text(s.doc()).unwrap();
    assert_eq!(gc_persist::from_text(&text).unwrap().doc.folders(), s.folders());

    // A deleted component leaves its folder; deleting a folder keeps what it held; ids are not reused.
    s.execute(&gc_doc::Command::Cell(gc_doc::CellOp::DeleteBody(a_id))).unwrap();
    s.edit_folders(FolderOp::Delete(first)).unwrap();
    assert!(s.component_tree().iter().any(|row| row.key == b));
    assert_eq!(s.edit_folders(FolderOp::Delete(first)), Err(EditError::BadValue));
    let third = s.edit_folders(FolderOp::Add { name: String::new(), members: Vec::new() }).unwrap().unwrap();
    assert!(third > second);
    assert!(s.undo());
    assert_eq!(s.folders().len(), 1);
}
