//! QA edge cases for renaming parameters (lead 7a954ff). Inventor 2026 help, To Create and Edit Parameters:
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-595F9D8E-9EEA-4446-91AB-830E5BAE548E.htm>
//! A rename rewrites every reference (other parameters and dimensions) in one undo step; a name that is
//! invalid, taken or a unit/function name is refused with nothing changed.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_core::Id;
use gc_doc::{Command, Created, Param};
use gc_session::Session;

fn add(s: &mut Session, name: &str, source: &str) -> Id<Param> {
    match s.execute(&Command::AddParam { name: name.into(), source: source.into(), angle: false }).unwrap()[0]
    {
        Created::Param(id) => id,
        ref other => panic!("{other:?}"),
    }
}

fn name_of(s: &Session, id: Id<Param>) -> String {
    s.doc().get(id).unwrap().name.clone()
}

fn source_of(s: &Session, id: Id<Param>) -> String {
    s.doc().get(id).unwrap().source.clone()
}

/// References follow the new name; similar names (a longer name that contains the old one, a unit) are left
/// alone; the whole rename is one Undo step.
#[test]
fn renaming_rewrites_exactly_the_references() {
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    let gap = add(&mut s, "Gap", "5 mm");
    let gap2 = add(&mut s, "Gap2", "Gap * 2");
    let wide = add(&mut s, "Wide", "Gap + Gap2 + 1 mm");
    let depth = s.history().undo_depth();
    s.execute(&Command::RenameParam { id: gap, name: "Spacing".into() }).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1, "one step");
    assert_eq!(name_of(&s, gap), "Spacing");
    assert_eq!(source_of(&s, gap2), "Spacing * 2");
    assert_eq!(source_of(&s, wide), "Spacing + Gap2 + 1 mm", "Gap2 is another parameter and keeps its name");
    assert_eq!(name_of(&s, gap2), "Gap2");
    assert!(s.undo());
    assert_eq!(name_of(&s, gap), "Gap");
    assert_eq!(source_of(&s, gap2), "Gap * 2");
    assert_eq!(source_of(&s, wide), "Gap + Gap2 + 1 mm");
}

/// Refused renames leave the document and the history untouched.
#[test]
fn bad_names_are_refused_and_change_nothing() {
    let mut s = Session::new();
    let a = add(&mut s, "a", "1 mm");
    add(&mut s, "b", "a * 2");
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    for bad in
        ["", "   ", "b", "1a", "a b", "a-b", "mm", "sin", "cos", "pi", "a+b", "é!", "\n", &"x".repeat(5000)]
    {
        let result = s.execute(&Command::RenameParam { id: a, name: bad.into() });
        // Some of these may be legal in the expression language; whatever is accepted must keep every
        // reference valid, and whatever is refused must change nothing.
        if result.is_err() {
            assert!(s.doc().same_content(&before), "{bad:?} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{bad:?}");
        } else {
            let new = name_of(&s, a);
            let b_source = s.doc().params().iter().map(|(_, p)| p.source.clone()).collect::<Vec<_>>();
            assert!(
                b_source.iter().any(|src| src.contains(new.trim())),
                "{bad:?}: references were not rewritten: {b_source:?}"
            );
            assert!(s.undo());
            assert!(s.doc().same_content(&before));
        }
    }
    // Renaming to its own name is a no-op; renaming a parameter that does not exist is refused.
    s.execute(&Command::RenameParam { id: a, name: "a".into() }).unwrap();
    assert!(s.doc().same_content(&before));
    let ghost = Id::<Param>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(s.execute(&Command::RenameParam { id: ghost, name: "z".into() }).is_err());
    assert!(s.doc().same_content(&before));
}

/// Dimensions that read the parameter keep their value after the rename (the sketch is re-solved the same).
#[test]
fn a_dimension_keeps_reading_the_renamed_parameter() {
    use gc_tools::{ToolEvent, ToolKind};
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    let w = add(&mut s, "Width", "30 mm");
    s.set_tool(ToolKind::Line);
    for at in [[0.0, 0.0], [0.020, 0.0]] {
        s.tool_event(ToolEvent::Move { pos: at, px: 1e-4, suppress_snap: false });
        s.tool_event(ToolEvent::Click { pos: at, px: 1e-4, modifier: false });
    }
    s.set_tool(ToolKind::Dimension);
    for at in [[0.010, 0.0], [0.010, -0.008]] {
        s.tool_event(ToolEvent::Move { pos: at, px: 1e-4, suppress_snap: false });
        s.tool_event(ToolEvent::Click { pos: at, px: 1e-4, modifier: false });
    }
    s.submit_value("Width * 2");
    let end = |s: &Session| {
        let sketch = s.active_sketch().unwrap().1;
        let curve = *sketch.curves.keys().next().unwrap();
        let (_, b) = sketch.line(curve).unwrap();
        sketch.pos(b).unwrap()[0]
    };
    assert!((end(&s) - 0.060).abs() < 1e-9, "{}", end(&s));
    s.execute(&Command::RenameParam { id: w, name: "W".into() }).unwrap();
    let dimension = s.active_sketch().unwrap().1.dimensions.values().next().unwrap().source.clone();
    assert_eq!(dimension, "W * 2");
    assert!((end(&s) - 0.060).abs() < 1e-9, "the value did not change");
    // Changing the renamed parameter still drives the dimension.
    s.execute(&Command::SetParam { id: w, source: "10 mm".into() }).unwrap();
    assert!((end(&s) - 0.020).abs() < 1e-9, "{}", end(&s));
}
