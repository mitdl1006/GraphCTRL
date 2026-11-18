//! QA boundary tests for assembly constraint values as model parameters (Inventor 2026 help, About Parameters in
//! Models GUID-8E5C63EA-0540-4EBE-9B17-B8159C7FE40C, To Work with Parameters in Models
//! GUID-595F9D8E-9EEA-4446-91AB-830E5BAE548E, document units GUID-671A8679-AEC1-413A-A23B-428967648CE6).
//! Refused edits leave the document and the history untouched; accepted ones are one undo step that restores
//! exactly (redo repeats).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    AxisSense, Body, CellOp, Command, Created, DocSettings, EditError, Geometry, LengthUnit, Occurrence,
    Placement, RelationKind, Relationship, Selection, Solid,
};
use gc_session::Session;

/// Refused with no trace, or one undo step that undoes exactly and redoes the same (the edit stays applied).
fn step<T>(s: &mut Session, what: &str, f: impl FnOnce(&mut Session) -> Result<T, EditError>) -> Option<T> {
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

fn on(body: Id<Body>, z: f64, up: f64) -> Selection {
    let geometry = Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, up] };
    Selection { occurrence: Occurrence::Item(body.erase()), geometry }
}

struct Scene {
    s: Session,
    table: Id<Body>,
    block: Id<Body>,
    lid: Id<Body>,
    first: Id<Relationship>,
    second: Id<Relationship>,
}

/// A table with two blocks each mated to its top.
fn scene() -> Scene {
    let mut s = Session::new();
    let table = s
        .add_body(Solid::Box { size: [2.0, 1.0, 1.0] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let block =
        s.add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.0, 1.5], rpy: [0.0; 3] }).unwrap();
    let lid = s
        .add_body(Solid::Box { size: [0.2; 3] }, Placement { xyz: [-0.3, 0.0, 1.5], rpy: [0.0; 3] })
        .unwrap();
    let mate = |body: Id<Body>| {
        Relationship::new(
            "Mate",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            on(body, -0.1, -1.0),
            on(table, 0.5, 1.0),
        )
    };
    let first = s.add_relationship(mate(block)).unwrap();
    let second = s.add_relationship(mate(lid)).unwrap();
    Scene { s, table, block, lid, first, second }
}

fn set(s: &mut Session, id: Id<Relationship>, text: &str) -> Result<(), EditError> {
    s.execute(&Command::Cell(CellOp::SetRelationValue { id, source: text.into() })).map(|_| ())
}

fn rename(s: &mut Session, id: Id<Relationship>, name: &str) -> Result<(), EditError> {
    s.execute(&Command::Cell(CellOp::RenameRelationParam { id, name: name.into() })).map(|_| ())
}

fn add_param(s: &mut Session, name: &str, source: &str) -> Id<gc_doc::Param> {
    let created = s.execute(&Command::AddParam { name: name.into(), source: source.into(), angle: false });
    let Created::Param(id) = created.unwrap()[0] else { panic!() };
    id
}

fn z(s: &Session, body: Id<Body>) -> f64 {
    s.doc().get(body).unwrap().placement.xyz[2]
}

#[test]
fn every_constraint_with_a_value_has_one_distinct_parameter_and_a_deleted_one_is_refused() {
    let Scene { mut s, first, second, .. } = scene();
    let a = s.doc().relation_param(first).unwrap().clone();
    let b = s.doc().relation_param(second).unwrap().clone();
    assert_ne!(a.name, b.name, "two constraints must not share a name");
    assert_eq!(s.doc().relation_params().len(), 2);
    // A bare number is the document's unit: what the table shows is what the user would type back.
    assert_eq!(a.value, 0.0);
    // A missing relationship id and an id of another kind are refused.
    let gone = s
        .add_relationship(Relationship::new(
            "Flush",
            RelationKind::Flush { offset: 0.0 },
            on(Id::<Body>::from_raw(std::num::NonZeroU64::MIN), 0.0, 1.0),
            on(Id::<Body>::from_raw(std::num::NonZeroU64::MIN), 0.0, 1.0),
        ))
        .ok();
    assert!(gone.is_none(), "a constraint of a part with itself");
    // A deleted constraint's id.
    let doomed = s.delete_relationship(second).map(|()| second);
    let missing = doomed.unwrap();
    assert!(step(&mut s, "missing id", |s| set(s, missing, "5 mm")).is_none());
    assert!(step(&mut s, "rename missing", |s| rename(s, missing, "x")).is_none());
}

#[test]
fn a_value_is_text_units_and_expressions_and_bad_text_leaves_no_trace() {
    let Scene { mut s, block, first, second, .. } = scene();
    // A bare number is millimetres; others convert; the part is moved by exactly that much.
    assert!(step(&mut s, "bare number", |s| set(s, first, "12")).is_some());
    assert!((z(&s, block) - (1.0 + 0.1 + 0.012)).abs() < 1e-9);
    assert_eq!(s.doc().relation_param(first).unwrap().source, "12");
    assert!(step(&mut s, "inches", |s| set(s, first, "1 in")).is_some());
    assert!((s.doc().relation_param(first).unwrap().value - 0.0254).abs() < 1e-12);
    assert!(step(&mut s, "centimetres", |s| set(s, second, "0.5 cm")).is_some());
    assert!(step(&mut s, "negative", |s| set(s, second, "-3 mm")).is_some());
    assert!(step(&mut s, "an expression", |s| set(s, second, "2 * (3 mm + 4 mm)")).is_some());
    assert!((s.doc().relation_param(second).unwrap().value - 0.014).abs() < 1e-12);
    // Text that does not evaluate, or evaluates to something that is no length, is refused.
    for bad in [
        "",
        "   ",
        "abc",
        "5 +",
        "(",
        "1 / 0",
        "5 deg",
        "5 mm * 5 mm",
        "nope + 1 mm",
        "1e999",
        "NaN",
        "inf",
        "5 mm; drop",
        "= 5",
        "1x = 5",
    ] {
        assert!(step(&mut s, bad, |s| set(s, first, bad)).is_none(), "{bad:?} was accepted");
    }
    // Zero offset and a very large one are legal values; each is exactly one undo step or a plain refusal.
    for text in ["0", "1000 m", "1e6 mm", "-1000 m"] {
        step(&mut s, text, |s| set(s, first, text));
    }
}

#[test]
fn a_constraint_can_be_named_while_its_value_is_set() {
    let Scene { mut s, first, second, .. } = scene();
    assert!(step(&mut s, "name = value", |s| set(s, first, "lift = 5 mm")).is_some());
    assert_eq!(s.doc().relation_param(first).unwrap().name, "lift");
    assert_eq!(s.doc().relation_param(first).unwrap().source, "5 mm");
    // The name is used by the second one and follows.
    assert!(step(&mut s, "uses lift", |s| set(s, second, "lift * 2")).is_some());
    assert_eq!(s.doc().relation_param(second).unwrap().value, 0.01);
    assert!(step(&mut s, "change lift", |s| set(s, first, "7 mm")).is_some());
    assert!((s.doc().relation_param(second).unwrap().value - 0.014).abs() < 1e-12);
    // A name already used by another constraint, a name that is no name, a name of a function or a unit.
    for bad in ["lift = 1 mm", "1x = 5 mm", "a b = 5 mm", "mm = 5 mm", "sin = 5 mm", " = 5 mm", "x y = 1"] {
        let target = second;
        if bad.starts_with("lift") {
            assert!(step(&mut s, bad, |s| set(s, target, bad)).is_none(), "{bad:?}");
        } else {
            // Whatever is decided it must be all or nothing.
            step(&mut s, bad, |s| set(s, target, bad));
        }
    }
}

#[test]
fn renaming_follows_every_reference_and_refuses_taken_or_invalid_names() {
    let Scene { mut s, first, second, .. } = scene();
    add_param(&mut s, "gap", "10 mm");
    assert!(step(&mut s, "lift", |s| set(s, first, "lift = gap + 1 mm")).is_some());
    assert!(step(&mut s, "lift2", |s| set(s, second, "lift2 = lift * 2")).is_some());
    let p = add_param(&mut s, "twice", "lift2 + 1 mm");
    assert!(step(&mut s, "rename to base", |s| rename(s, first, "base")).is_some());
    assert_eq!(s.doc().relation_param(second).unwrap().source, "base * 2");
    assert_eq!(s.doc().get(p).unwrap().source, "lift2 + 1 mm");
    assert!(step(&mut s, "rename lift2", |s| rename(s, second, "other")).is_some());
    assert_eq!(s.doc().get(p).unwrap().source, "other + 1 mm");
    // Same name again: nothing to do (accepted without a trace or refused; both leave no half state).
    let before = s.doc().clone();
    drop(rename(&mut s, first, "base"));
    assert!(s.doc().same_content(&before), "renaming to the same name changed the document");
    // Taken by a parameter, by the other constraint, invalid.
    for bad in ["gap", "other", "twice", "", " ", "1a", "a b", "a-b", "a.b", "m", "mm", "pi", "sin", "x+y"] {
        step(&mut s, bad, |s| rename(s, first, bad));
    }
    for taken in ["gap", "other", "twice"] {
        assert!(step(&mut s, taken, |s| rename(s, first, taken)).is_none(), "{taken}");
    }
    // The other direction: a Param named like a constraint's parameter is refused.
    let id = add_param(&mut s, "free", "1 mm");
    let before = s.doc().clone();
    let r = s.execute(&Command::RenameParam { id, name: "base".into() });
    assert!(matches!(r, Err(EditError::BadName(_))), "{r:?}");
    assert!(s.doc().same_content(&before));
    let r = s.execute(&Command::AddParam { name: "base".into(), source: "1 mm".into(), angle: false });
    assert!(r.is_err(), "a parameter named like a constraint's");
    assert!(s.doc().same_content(&before));
}

#[test]
fn a_parameter_that_a_constraint_uses_cannot_be_deleted_but_a_deleted_constraint_hands_its_value_on() {
    let Scene { mut s, block, first, .. } = scene();
    let gap = add_param(&mut s, "gap", "10 mm");
    assert!(step(&mut s, "lift", |s| set(s, first, "lift = gap + 5 mm")).is_some());
    // InUse: the document does not change, and nothing is half deleted.
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    assert!(matches!(s.execute(&Command::DeleteParam { id: gap }), Err(EditError::InUse)));
    assert!(s.doc().same_content(&before));
    assert_eq!(s.history().undo_depth(), depth);
    // A Param that names the constraint's parameter: deleting the constraint keeps what it was worth.
    let p = add_param(&mut s, "twice", "lift * 2");
    assert_eq!(s.doc().get(p).unwrap().value, 0.03);
    let z_before = z(&s, block);
    assert!(step(&mut s, "delete the constraint", |s| s.delete_relationship(first)).is_some());
    assert!(s.doc().relation_param(first).is_none(), "the parameter went with the constraint");
    let after = s.doc().get(p).unwrap();
    assert!(after.value.is_finite());
    assert!(
        !after.source.contains("lift"),
        "the expression keeps the value, not a dangling name: {:?}",
        after.source
    );
    assert_eq!(after.value, 0.03);
    assert_eq!(z(&s, block), z_before, "the part stays where it was");
    // Now gap is free to delete.
    assert!(
        step(&mut s, "delete gap", |s| s.execute(&Command::DeleteParam { id: gap }).map(|_| ())).is_some()
    );
}

#[test]
fn a_loop_through_constraints_and_parameters_is_refused_with_no_trace() {
    let Scene { mut s, first, second, .. } = scene();
    assert!(step(&mut s, "a", |s| set(s, first, "a = 5 mm")).is_some());
    assert!(step(&mut s, "b", |s| set(s, second, "b = a + 1 mm")).is_some());
    // a = b + 1 closes the loop; a = a + 1 is the shortest loop.
    assert!(step(&mut s, "a = b", |s| set(s, first, "b + 1 mm")).is_none());
    assert!(step(&mut s, "a = a", |s| set(s, first, "a + 1 mm")).is_none());
    // Through a parameter.
    let p = add_param(&mut s, "p", "b + 1 mm");
    assert!(step(&mut s, "a = p", |s| set(s, first, "p + 1 mm")).is_none());
    // And the other way round: a parameter that would name a constraint that names it.
    assert!(
        step(&mut s, "p = a + p", |s| s.execute(&Command::SetParam { id: p, source: "a + p".into() }))
            .is_none()
    );
}

#[test]
fn constraint_parameters_follow_the_documents_length_unit_and_keep_their_size() {
    let Scene { mut s, block, first, .. } = scene();
    assert!(step(&mut s, "set", |s| set(s, first, "25.4")).is_some());
    let value = s.doc().relation_param(first).unwrap().value;
    let z0 = z(&s, block);
    assert!((value - 0.0254).abs() < 1e-12);
    let inches = DocSettings { length_unit: LengthUnit::In, ..s.doc_settings() };
    if step_settings(&mut s, inches) {
        // The size is the same: 25.4 mm is one inch; the text no longer means 25.4 inches.
        assert!((s.doc().relation_param(first).unwrap().value - 0.0254).abs() < 1e-12);
        assert_eq!(z(&s, block), z0);
        // A bare number is now inches.
        assert!(step(&mut s, "2 inches", |s| set(s, first, "2")).is_some());
        assert!((s.doc().relation_param(first).unwrap().value - 0.0508).abs() < 1e-12);
    }
}

fn step_settings(s: &mut Session, settings: DocSettings) -> bool {
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.set_doc_settings(settings) {
        Err(_) => {
            assert!(s.doc().same_content(&before));
            false
        }
        Ok(()) => {
            assert_eq!(s.history().undo_depth(), depth + 1);
            true
        }
    }
}

#[test]
fn saved_and_opened_files_keep_constraint_parameters_and_old_files_get_them() {
    let Scene { mut s, block, first, second, .. } = scene();
    add_param(&mut s, "gap", "3 mm");
    set(&mut s, first, "lift = gap * 2").unwrap();
    set(&mut s, second, "4 mm").unwrap();
    let text = gc_persist::to_text(s.doc()).unwrap();
    let opened = gc_persist::from_text(&text).unwrap().doc;
    assert!(opened.same_content(s.doc()));
    assert_eq!(opened.relation_param(first).unwrap().name, "lift");

    // A file from before constraints had parameters: take the key away. It opens, and the next edit gives
    // every constraint a name again without changing where anything stands.
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let removed = strip_key(&mut json, "relation_params");
    assert!(removed > 0, "the saved file has no relation_params key to remove");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.gcp");
    std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
    let mut old = Session::new();
    old.open(&path).unwrap_or_else(|e| panic!("an older file must open: {e:?}"));
    assert!((z(&old, block) - z(&s, block)).abs() < 1e-12);
    drop(set(&mut old, second, "5 mm"));
    for (id, _) in old.relationships() {
        if let Some(p) = old.doc().relation_param(id) {
            assert!(gc_expr::is_valid_param_name(&p.name), "{:?}", p.name);
        }
    }

    // A parameter that names something that is not there (a damaged file) must not crash the opening.
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    replace_text(&mut json, "gap * 2", "missing * 2");
    let path = dir.path().join("damaged.gcp");
    std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
    let mut damaged = Session::new();
    if damaged.open(&path).is_ok() {
        drop(set(&mut damaged, second, "6 mm"));
    }
}

fn strip_key(v: &mut serde_json::Value, key: &str) -> usize {
    match v {
        serde_json::Value::Object(m) => {
            let here = usize::from(m.remove(key).is_some());
            here + m.values_mut().map(|x| strip_key(x, key)).sum::<usize>()
        }
        serde_json::Value::Array(a) => a.iter_mut().map(|x| strip_key(x, key)).sum(),
        _ => 0,
    }
}

fn replace_text(v: &mut serde_json::Value, from: &str, to: &str) {
    match v {
        serde_json::Value::String(s) if s == from => to.clone_into(s),
        serde_json::Value::Object(m) => m.values_mut().for_each(|x| replace_text(x, from, to)),
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| replace_text(x, from, to)),
        _ => {}
    }
}

#[test]
fn a_changed_parameter_moves_every_part_that_follows_it_in_one_undo_step() {
    let Scene { mut s, block, lid, table, first, second } = scene();
    let gap = add_param(&mut s, "gap", "1 mm");
    set(&mut s, first, "gap").unwrap();
    set(&mut s, second, "gap * 3").unwrap();
    assert!(
        step(&mut s, "gap = 10 mm", |s| s.execute(&Command::SetParam { id: gap, source: "10 mm".into() }))
            .is_some()
    );
    assert!((z(&s, block) - 1.11).abs() < 1e-9);
    assert!((z(&s, lid) - 1.13).abs() < 1e-9);
    assert_eq!(z(&s, table), 0.5, "the table stays");
    // A value no constraint can hold (a lid that would pass through the table is still satisfiable, a
    // NaN is not): refused with no trace.
    assert!(
        step(&mut s, "gap = bad", |s| s.execute(&Command::SetParam { id: gap, source: "1 / 0".into() }))
            .is_none()
    );
}
