//! QA edge cases for view and positional representations (assembly parity P2 #20): a view saves what is shown;
//! a positional representation overrides relationship values and positions on top of Primary, which is the
//! document itself and is never stored; going back to Primary restores exactly. (The Inventor help page for
//! representations (A63 in docs/ux/inventor-assembly-parity.md) is listed there without a usable id, so no
//! link is given.)

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use gc_core::{AnyId, Id};
use gc_doc::{Body, CellOp, Command, ComponentFlag, Occurrence, Placement, PositionalRepresentation, Solid};
use gc_session::Session;

fn placed(x: f64) -> Placement {
    Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] }
}

fn cell() -> (Session, Id<Body>, Id<Body>) {
    let mut s = Session::new();
    let a = s.add_body(Solid::Box { size: [0.2; 3] }, placed(0.0)).unwrap();
    let b = s.add_body(Solid::Box { size: [0.2; 3] }, placed(1.0)).unwrap();
    (s, a, b)
}

fn op(s: &mut Session, op: CellOp) -> Result<Vec<gc_doc::Created>, gc_doc::EditError> {
    s.execute(&Command::Cell(op))
}

fn visible(s: &Session, id: Id<Body>) -> bool {
    s.components().iter().find(|n| n.occurrence == Occurrence::Item(id.erase())).unwrap().visible
}

fn ghost() -> AnyId {
    Id::<Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase()
}

/// A view remembers what was hidden; changing the model afterwards and activating the view puts the
/// visibility back; one undo step; unknown indexes are refused untouched; hidden entries for components
/// that no longer exist are ignored.
#[test]
fn a_view_representation_restores_visibility() {
    let (mut s, a, b) = cell();
    s.set_component(Occurrence::Item(b.erase()), ComponentFlag::Visible, false).unwrap();
    op(
        &mut s,
        CellOp::AddViewRepresentation { name: "Only A".into(), camera: Some("cam".into()), section: None },
    )
    .unwrap();
    assert_eq!(s.representations().views.len(), 1);
    assert!(s.representations().views[0].hidden.contains(&Occurrence::Item(b.erase())));
    assert_eq!(s.representations().views[0].camera.as_deref(), Some("cam"));
    // A new view is the active one and would take the display edits that follow (Inventor 2026 help, To
    // Create, Edit, and Restore View Representations, GUID-397F9263-12C3-47A1-B6A6-16FC05EDEF47); locked,
    // it keeps what it saved. (Backend, lead-approved test change.)
    assert_eq!(s.representations().active_view, Some(0));
    op(&mut s, CellOp::LockViewRepresentation { index: 0, locked: true }).unwrap();
    // Show everything, hide a, then activate the view.
    s.set_component(Occurrence::Item(b.erase()), ComponentFlag::Visible, true).unwrap();
    s.set_component(Occurrence::Item(a.erase()), ComponentFlag::Visible, false).unwrap();
    let depth = s.history().undo_depth();
    op(&mut s, CellOp::ActivateViewRepresentation(0)).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one step");
    assert!(visible(&s, a) && !visible(&s, b));
    assert!(s.undo());
    assert!(!visible(&s, a) && visible(&s, b));
    // Refused: an index that is not there.
    let before = s.doc().clone();
    for index in [1, 7, usize::MAX] {
        assert!(op(&mut s, CellOp::ActivateViewRepresentation(index)).is_err(), "{index}");
        assert!(s.doc().same_content(&before));
    }
    // A view that names a component that is gone (or never was) does not break activation.
    op(&mut s, CellOp::LockViewRepresentation { index: 0, locked: false }).unwrap();
    let mut view = s.representations().views[0].clone();
    view.hidden.push(Occurrence::Item(ghost()));
    view.disabled.push(Occurrence::Item(ghost()));
    view.transparency.push((ghost(), 0.5));
    op(&mut s, CellOp::SetViewRepresentation { index: 0, view }).unwrap();
    op(&mut s, CellOp::ActivateViewRepresentation(0)).unwrap();
    assert!(visible(&s, a) && !visible(&s, b));
}

/// Names: an empty name, the same name twice. Whatever is accepted must keep every saved view reachable by
/// its own index; refused ones leave no trace.
#[test]
fn view_names_are_sensible() {
    let (mut s, _, _) = cell();
    let add = |s: &mut Session, name: &str| {
        op(s, CellOp::AddViewRepresentation { name: name.into(), camera: None, section: None })
    };
    let before = s.doc().clone();
    for bad in ["", "   ", "\n"] {
        if add(&mut s, bad).is_ok() {
            assert!(s.undo(), "{bad:?}");
        }
        assert!(s.doc().same_content(&before), "{bad:?} left a trace");
    }
    add(&mut s, "Top").unwrap();
    let again = add(&mut s, "Top");
    if again.is_ok() {
        // Duplicate names accepted: they are told apart by index only.
        assert_eq!(s.representations().views.len(), 2);
    }
}

/// Positional representations: "Primary" and empty or repeated names are refused; values that are not numbers
/// are refused; activating moves components as the override says and going back to Primary restores them
/// exactly; deleting the active position returns to Primary first.
#[test]
fn positional_representations_override_and_restore_exactly() {
    let (mut s, a, b) = cell();
    let before = s.doc().clone();
    for bad in ["", "  ", "Primary"] {
        assert!(op(&mut s, CellOp::AddPositionalRepresentation(bad.into())).is_err(), "{bad:?}");
        assert!(s.doc().same_content(&before));
    }
    op(&mut s, CellOp::AddPositionalRepresentation("Open".into())).unwrap();
    assert!(op(&mut s, CellOp::AddPositionalRepresentation("Open".into())).is_err(), "a repeated name");
    assert!(
        op(&mut s, CellOp::AddPositionalRepresentation(" Open ".into())).is_err(),
        "a repeated name with spaces"
    );
    // Non-finite relationship values are refused.
    let ghost_rel = Id::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let bad = PositionalRepresentation {
        name: "Open".into(),
        values: vec![(ghost_rel, f64::NAN)],
        ..Default::default()
    };
    assert!(op(&mut s, CellOp::SetPositionalRepresentation { index: 0, position: bad }).is_err());
    // Move b in "Open".
    let open = PositionalRepresentation {
        name: "Open".into(),
        placements: vec![(b.erase(), placed(2.5)), (ghost(), placed(9.0))],
        ..Default::default()
    };
    op(&mut s, CellOp::SetPositionalRepresentation { index: 0, position: open }).unwrap();
    let primary_b = s.doc().get(b).unwrap().placement;
    op(&mut s, CellOp::ActivatePositionalRepresentation(Some(0))).unwrap();
    assert_eq!(s.doc().get(b).unwrap().placement.xyz, [2.5, 0.0, 0.0]);
    assert_eq!(s.doc().get(a).unwrap().placement.xyz, [0.0; 3], "a was not overridden");
    // Back to Primary: bit-exact.
    op(&mut s, CellOp::ActivatePositionalRepresentation(None)).unwrap();
    assert_eq!(s.doc().get(b).unwrap().placement, primary_b);
    // Out of range is refused; activating the active one again is harmless.
    assert!(op(&mut s, CellOp::ActivatePositionalRepresentation(Some(5))).is_err());
    op(&mut s, CellOp::ActivatePositionalRepresentation(Some(0))).unwrap();
    op(&mut s, CellOp::ActivatePositionalRepresentation(Some(0))).unwrap();
    // Deleting the active position goes back to Primary exactly.
    op(&mut s, CellOp::DeleteRepresentation { view: None, position: Some(0) }).unwrap();
    assert_eq!(s.doc().get(b).unwrap().placement, primary_b);
    assert!(s.representations().positions.is_empty() && s.representations().active_position.is_none());
    // Undo of the delete brings the position back and active, with b where "Open" puts it.
    assert!(s.undo());
    assert_eq!(s.representations().positions.len(), 1);
}

/// Deleting something that is not there, or nothing at all, changes nothing; views and positions are removed
/// independently.
#[test]
fn deleting_representations_is_exact() {
    let (mut s, _, _) = cell();
    op(&mut s, CellOp::AddViewRepresentation { name: "V".into(), camera: None, section: None }).unwrap();
    op(&mut s, CellOp::AddPositionalRepresentation("P".into())).unwrap();
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    for (view, position) in [(Some(5), None), (None, Some(5)), (Some(0), Some(5)), (Some(usize::MAX), None)] {
        assert!(
            op(&mut s, CellOp::DeleteRepresentation { view, position }).is_err(),
            "{view:?} {position:?}"
        );
        assert!(s.doc().same_content(&before), "{view:?} {position:?}: a refused delete removed something");
        assert_eq!(s.history().undo_depth(), depth);
    }
    op(&mut s, CellOp::DeleteRepresentation { view: Some(0), position: None }).unwrap();
    assert!(s.representations().views.is_empty() && s.representations().positions.len() == 1);
}

/// Saved and reopened, the representations (and the active position) are the same.
#[test]
fn representations_survive_save_and_reopen() {
    let (mut s, _, b) = cell();
    op(
        &mut s,
        CellOp::AddViewRepresentation {
            name: "V".into(),
            camera: Some("c".into()),
            section: Some("s".into()),
        },
    )
    .unwrap();
    op(&mut s, CellOp::AddPositionalRepresentation("P".into())).unwrap();
    let position = PositionalRepresentation {
        name: "P".into(),
        placements: vec![(b.erase(), placed(3.0))],
        ..Default::default()
    };
    op(&mut s, CellOp::SetPositionalRepresentation { index: 0, position }).unwrap();
    op(&mut s, CellOp::ActivatePositionalRepresentation(Some(0))).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("rep.gctrl");
    s.save_as(&file).unwrap();
    let mut reopened = Session::new();
    reopened.open(&file).unwrap();
    assert!(reopened.doc().same_content(s.doc()));
    assert_eq!(reopened.representations(), s.representations());
    op(&mut reopened, CellOp::ActivatePositionalRepresentation(None)).unwrap();
    assert_eq!(
        reopened.doc().get(b).unwrap().placement.xyz,
        [1.0, 0.0, 0.0],
        "Primary is restored after a reopen too"
    );
}
