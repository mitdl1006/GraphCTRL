//! Isolate and Undo Isolate (Inventor 2026 help, Frame Generator browser,
//! GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E: "Isolate turns off visibility of components … Undo Isolate,
//! instead of Undo, restores visibility of components to the pre-isolation state. It also keeps intact
//! those operations performed in the isolated state").

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{ComponentFlag, EditError, Occurrence, Placement, Solid};
use gc_session::Session;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn isolate_hides_the_rest_and_undo_isolate_brings_it_back() {
    let mut s = Session::new();
    let body = |s: &mut Session, x: f64| {
        let placement = Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] };
        Occurrence::Item(s.add_body(Solid::Box { size: [0.1; 3] }, placement).unwrap().erase())
    };
    let (a, b, c) = (body(&mut s, 1.0), body(&mut s, 2.0), body(&mut s, 3.0));
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    let robot_item = Occurrence::Item(robot.erase());
    let visible = |s: &Session, o: Occurrence| s.doc().component_flag(o, ComponentFlag::Visible);

    // Nothing to keep, nothing isolated yet.
    assert_eq!(s.isolate(&[]), Err(EditError::BadValue));
    assert_eq!(s.undo_isolate(), Err(EditError::BadValue));
    // C was hidden by hand before: Undo Isolate must not show it.
    s.set_component(c, ComponentFlag::Visible, false).unwrap();

    let depth = s.history().undo_depth();
    s.isolate(&[a]).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.is_isolated());
    assert!(visible(&s, a) && !visible(&s, b) && !visible(&s, c) && !visible(&s, robot_item));

    // What is done while isolated stays: B is shown by hand, A is moved.
    s.set_component(b, ComponentFlag::Visible, true).unwrap();
    s.undo_isolate().unwrap();
    assert!(!s.is_isolated());
    assert!(visible(&s, a) && visible(&s, b) && visible(&s, robot_item));
    assert!(!visible(&s, c), "hidden before the isolate, hidden after");
    // Plain undo steps back through it like any edit.
    assert!(s.undo());
    assert!(s.is_isolated() && !visible(&s, robot_item));
    assert!(s.redo());

    // Keeping a robot link keeps the robot and the links around and inside it; other components go.
    let links = s.robot_links(robot);
    let last = links.last().unwrap().occurrence;
    s.isolate(&[last]).unwrap();
    assert!(visible(&s, robot_item) && visible(&s, last) && !visible(&s, a) && !visible(&s, b));
    // Saved with the document: Undo Isolate still works after reading it back.
    let text = gc_persist::to_text(s.doc()).unwrap();
    assert!(gc_persist::from_text(&text).unwrap().doc.same_content(s.doc()));
    s.undo_isolate().unwrap();
    assert!(visible(&s, a) && visible(&s, b));
}
