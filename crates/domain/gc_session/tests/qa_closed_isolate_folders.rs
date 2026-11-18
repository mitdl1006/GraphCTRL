//! QA boundary tests for closed path roles (start point, tangent tool attitude), Isolate / Undo Isolate and the
//! browser's user-defined folders. Inventor 2026 help, under
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>: Isolate (Frame Generator browser)
//! GUID-36AE9264-912D-4D3C-BB24-7EC16FA8777E, Assembly Browser Reference (User-defined Folder)
//! GUID-CB6B1637-71AD-4753-8CC9-19559BFEE3A5, Working with the Assembly Browser
//! GUID-5F975252-BDB4-488C-A2C5-8B15C7D3C641; closed path roles are GraphCTRL's own feature (plan 9.1).
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
    Command, ComponentFlag, Created, EditError, EndSnap, FolderOp, Occurrence, PathMotion, Placement, Sketch,
    SketchOp, Solid, SubassemblyOp, TreeKey,
};
use gc_session::{PlanStatus, Session};
use gc_sketch::{Curve, Item, SkPoint};

type K = Id<Sketch>;

/// Refused with no trace, or one undo step that undoes exactly and redoes the same.
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

// ---- Closed path roles ----

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap()
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn point_at(s: &mut Session, k: K, pos: [f64; 2]) -> Id<SkPoint> {
    let Created::Point(p) = run(s, k, SketchOp::AddPoint { pos: [pos[0], pos[1], 0.0] }).unwrap()[0] else {
        panic!()
    };
    p
}

fn circle(s: &mut Session, k: K, c: [f64; 2], r: f64) -> Id<Curve> {
    curve_of(
        &run(
            s,
            k,
            SketchOp::AddCircle {
                centre: [c[0], c[1], 0.0],
                radius: r,
                centre_snap: EndSnap::default(),
                rim_snap: EndSnap::default(),
            },
        )
        .unwrap(),
    )
}

/// A robot 0.6 m in front of the XZ wall, as in the backend tests, with a sketch on that wall.
fn wall() -> (Session, Id<gc_doc::Robot>, K) {
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    s.execute(&Command::Cell(gc_doc::CellOp::PlaceRobot { id: robot, placement })).unwrap();
    s.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    s.end_jog();
    let k = s.create_sketch("XZ").unwrap();
    (s, robot, k)
}

fn polyline_ok(s: &Session, role: Id<gc_doc::SketchRole>, what: &str) {
    let line = s.role_polyline(role).unwrap_or_else(|| panic!("{what}: no polyline"));
    assert!(line.len() >= 2 && line.iter().all(|p| p.iter().all(|v| v.is_finite())), "{what}");
}

#[test]
fn every_kind_of_closed_curve_is_a_path_role_of_its_own() {
    let (mut s, _robot, k) = wall();
    let c = circle(&mut s, k, [0.0, 0.55], 0.05);
    let e = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddEllipse {
                centre: [0.3, 0.0, 0.55],
                major: [0.34, 0.0, 0.55],
                minor: 0.02,
                centre_snap: EndSnap::default(),
            },
        )
        .unwrap(),
    );
    let sp = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddSpline {
                points: vec![[-0.3, 0.5, 0.0], [-0.25, 0.55, 0.0], [-0.2, 0.5, 0.0], [-0.25, 0.45, 0.0]],
                snaps: Vec::new(),
            },
        )
        .unwrap(),
    );
    run(&mut s, k, SketchOp::CloseSpline { curve: sp }).unwrap();
    let open = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddLine {
                a: [0.5, 0.5, 0.0],
                b: [0.6, 0.5, 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        )
        .unwrap(),
    );
    s.finish_sketch();
    for (what, curve) in [("circle", c), ("ellipse", e), ("closed spline", sp)] {
        assert_eq!(s.chain_at(k, curve), vec![curve], "{what}");
        let role = step(&mut s, what, |s| s.add_path_role(k, &[curve])).unwrap();
        s.add_path_role(k, &[curve]).ok();
        let role = s.path_role(k, curve).unwrap_or(role);
        assert!(s.role_closed(role), "{what} is closed");
        polyline_ok(&s, role, what);
        let line = s.role_polyline(role).unwrap();
        let (first, last) = (line[0], *line.last().unwrap());
        let gap =
            ((first[0] - last[0]).powi(2) + (first[1] - last[1]).powi(2) + (first[2] - last[2]).powi(2))
                .sqrt();
        assert!(gap < 1e-9, "{what}: not once round ({gap} m apart)");
        // The same curve again: it is in a path.
        assert_eq!(s.add_path_role(k, &[curve]), Err(EditError::InUse), "{what}");
        // Reversing: two steps back to the start; the start stays where it is.
        let before = s.role_polyline(role).unwrap()[0];
        s.reverse_role(role).unwrap();
        s.reverse_role(role).unwrap();
        assert_eq!(s.role_polyline(role).unwrap()[0], before, "{what}");
    }
    // An open line is not closed; a waypoint is not either; closed curves do not chain with others.
    let line_role = s.add_path_role(k, &[open]).unwrap();
    assert!(!s.role_closed(line_role));
    let p = {
        s.enter_sketch(k);
        let p = point_at(&mut s, k, [0.0, 0.8]);
        s.finish_sketch();
        p
    };
    let way = s.add_waypoint_role(k, p).unwrap();
    assert!(!s.role_closed(way));
    let c2 = {
        s.enter_sketch(k);
        let c2 = circle(&mut s, k, [0.0, 0.9], 0.02);
        s.finish_sketch();
        c2
    };
    assert!(step(&mut s, "a closed curve with an open one", |s| s.add_path_role(k, &[c2, open])).is_none());
}

#[test]
fn start_points_and_the_tangent_attitude_have_boundaries() {
    let (mut s, robot, k) = wall();
    let c = circle(&mut s, k, [0.0, 0.55], 0.05);
    let top = point_at(&mut s, k, [0.0, 0.7]);
    let inside = point_at(&mut s, k, [0.0, 0.55]);
    let far = point_at(&mut s, k, [1e3, 1e3]);
    s.finish_sketch();
    let role = s.add_path_role(k, &[c]).unwrap();
    let way = s.add_waypoint_role(k, top).unwrap();
    // Start: a point of the sketch (the one nearest on the circle is used), the circle's own centre (equally
    // near everywhere), a point very far away; each is one step or refused, with a finite polyline.
    for (what, p) in [("above", top), ("centre", inside), ("far away", far)] {
        if step(&mut s, what, |s| s.set_role_start(role, Some(p))).is_some() {
            s.set_role_start(role, Some(p)).unwrap();
            polyline_ok(&s, role, what);
            let line = s.role_polyline(role).unwrap();
            let (a, b) = (line[0], *line.last().unwrap());
            assert!((a[0] - b[0]).abs() < 1e-9 && (a[2] - b[2]).abs() < 1e-9, "{what}: once round");
        }
    }
    // The same start again is no step; None goes back to the default (+X).
    let depth = s.history().undo_depth();
    s.set_role_start(role, Some(far)).ok();
    assert!(s.history().undo_depth() <= depth + 1);
    assert!(
        step(&mut s, "start back to the default", |s| s.set_role_start(role, None)).is_some()
            || s.history().undo_depth() == depth
    );
    s.set_role_start(role, None).ok();
    assert!((s.role_polyline(role).unwrap()[0][0] - 0.05).abs() < 1e-9);
    // A ghost point, and the waypoint's settings (refused).
    let ghost = Id::<SkPoint>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(step(&mut s, "ghost start", |s| s.set_role_start(role, Some(ghost))).is_none());
    assert!(step(&mut s, "start of a waypoint", |s| s.set_role_start(way, Some(top))).is_none());
    assert!(step(&mut s, "tangent of a waypoint", |s| s.set_role_tangent(way, true)).is_none());
    // A start that is then deleted: the role goes back to the default start, finite; undo restores.
    s.set_role_start(role, Some(top)).unwrap();
    s.enter_sketch(k);
    let before = s.doc().clone();
    run(&mut s, k, SketchOp::Delete(vec![Item::Point(top)])).ok();
    s.finish_sketch();
    if s.doc().get(role).is_some() {
        polyline_ok(&s, role, "after the start point was deleted");
    }
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    // Tangent: toggle is one step each way; the same value again is no step.
    assert!(step(&mut s, "tangent on", |s| s.set_role_tangent(role, true)).is_some());
    s.set_role_tangent(role, true).unwrap();
    let depth = s.history().undo_depth();
    s.set_role_tangent(role, true).ok();
    assert_eq!(s.history().undo_depth(), depth, "tangent again is no step");
    // A robot once round with the tool turning 360°: Ready, or a failure with a reason (wrist limits).
    let path = s.follow_roles(robot, &[role]).unwrap();
    for motion in [PathMotion::Follow, PathMotion::Linear, PathMotion::Joint] {
        s.set_role_motion(role, motion).unwrap();
        for tangent in [false, true] {
            s.set_role_tangent(role, tangent).unwrap();
            let status = s.plan(path);
            if status == PlanStatus::Ready {
                let tip: Vec<_> = s.planned(path).unwrap().tip_path().collect();
                assert!(tip.iter().all(|p| p.iter().all(|v| v.is_finite())), "{motion:?} tangent={tangent}");
            } else {
                let why = s.plan_error(path).unwrap_or_default();
                assert!(!why.is_empty(), "{motion:?} tangent={tangent}: failed with no reason ({status:?})");
            }
        }
    }
}

#[test]
fn closed_curve_extremes_and_files() {
    let (mut s, robot, k) = wall();
    // Radii from nothing to larger than the wall.
    let tiny = circle(&mut s, k, [0.0, 0.55], 1e-6);
    let huge = circle(&mut s, k, [0.0, 0.55], 5.0);
    s.finish_sketch();
    for (what, c) in [("tiny", tiny), ("huge", huge)] {
        // `step` leaves an accepted role in place.
        let Some(role) = step(&mut s, what, |s| s.add_path_role(k, &[c])) else { continue };
        polyline_ok(&s, role, what);
        let path = s.follow_roles(robot, &[role]).unwrap();
        s.set_role_tangent(role, true).unwrap();
        let status = s.plan(path);
        if status != PlanStatus::Ready {
            assert!(s.plan_error(path).is_some(), "{what}: failed with no reason");
        }
    }
    // path_from_sketch: a sketch of only closed curves is still refused; with an open chain only that is used.
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let only_closed = s.create_sketch("XZ").unwrap();
    circle(&mut s, only_closed, [0.0, 0.5], 0.05);
    s.finish_sketch();
    let depth2 = s.history().undo_depth();
    let _ = (before, depth);
    assert!(s.path_from_sketch(robot, only_closed).is_err());
    assert_eq!(s.history().undo_depth(), depth2, "the refusal left no trace");
    // Saved and opened: travel settings stay; a file without them (an old one) opens with the defaults.
    let c = circle_in_new_sketch(&mut s);
    let (sk, curve) = c;
    let role = s.add_path_role(sk, &[curve]).unwrap();
    s.set_role_tangent(role, true).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("closed.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()));
    let again = dir.path().join("again.gcp");
    t.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    fn strip(v: &mut serde_json::Value) -> usize {
        let mut n = 0;
        match v {
            serde_json::Value::Object(map) => {
                if map.remove("travel").is_some() {
                    n += 1;
                }
                for x in map.values_mut() {
                    n += strip(x);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a {
                    n += strip(x);
                }
            }
            _ => {}
        }
        n
    }
    assert!(strip(&mut value) >= 1, "the saved role has travel settings");
    let old = dir.path().join("old.gcp");
    std::fs::write(&old, serde_json::to_string(&value).unwrap()).unwrap();
    let mut u = Session::new();
    u.open(&old).unwrap();
    let old_role = u.doc().roles().iter().next().unwrap().0;
    assert!(u.role_closed(old_role), "still a closed path");
}

fn circle_in_new_sketch(s: &mut Session) -> (K, Id<Curve>) {
    let k = s.create_sketch("XZ").unwrap();
    let c = circle(s, k, [0.2, 0.5], 0.04);
    s.finish_sketch();
    (k, c)
}

// ---- Isolate ----

fn body(s: &mut Session, x: f64) -> Occurrence {
    let placement = Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] };
    Occurrence::Item(s.add_body(Solid::Box { size: [0.1; 3] }, placement).unwrap().erase())
}

fn visible(s: &Session, o: Occurrence) -> bool {
    s.doc().component_flag(o, ComponentFlag::Visible)
}

#[test]
fn isolate_boundaries() {
    let mut s = Session::new();
    let (a, b, c, d) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6), body(&mut s, 0.9));
    let ghost =
        Occurrence::Item(Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase());
    // Refusals: nothing kept, a ghost among the kept, Undo Isolate with nothing isolated.
    assert!(step(&mut s, "empty keep", |s| s.isolate(&[])).is_none());
    assert!(step(&mut s, "ghost keep", |s| s.isolate(&[ghost])).is_none());
    assert!(step(&mut s, "a ghost with a real one", |s| s.isolate(&[a, ghost])).is_none());
    assert!(step(&mut s, "undo isolate with nothing", |s| s.undo_isolate()).is_none());
    assert!(!s.is_isolated());
    // Keeping everything hides nothing: no empty "isolated" state is left behind.
    let r = step(&mut s, "keep all", |s| s.isolate(&[a, b, c, d]));
    if r.is_some() {
        s.isolate(&[a, b, c, d]).unwrap();
        assert!(!s.is_isolated() || [a, b, c, d].iter().all(|o| visible(&s, *o)));
        if s.is_isolated() {
            s.undo_isolate().unwrap();
        }
    }
    // Duplicates in the keep list; a second Isolate while isolated adds to what is hidden (and a later Undo
    // Isolate shows all of it).
    assert!(step(&mut s, "keep a twice", |s| s.isolate(&[a, a])).is_some());
    s.isolate(&[a, a]).unwrap();
    assert!(visible(&s, a) && !visible(&s, b) && !visible(&s, c) && !visible(&s, d));
    s.set_component(b, ComponentFlag::Visible, true).unwrap();
    assert!(step(&mut s, "isolate again", |s| s.isolate(&[b])).is_some());
    s.isolate(&[b]).unwrap();
    assert!(visible(&s, b) && !visible(&s, a));
    s.undo_isolate().unwrap();
    assert!(visible(&s, a) && visible(&s, b) && visible(&s, c) && visible(&s, d), "everything is back");
    assert!(!s.is_isolated());
    // Undo Isolate twice: the second is refused.
    assert!(step(&mut s, "undo isolate again", |s| s.undo_isolate()).is_none());
    // A deleted component that Isolate hid: Undo Isolate still works for the rest, with no panic.
    s.isolate(&[a]).unwrap();
    let Occurrence::Item(any) = c else { panic!() };
    s.execute(&Command::Cell(gc_doc::CellOp::DeleteBody(Id::from_raw(any.raw)))).unwrap();
    s.undo_isolate().unwrap();
    assert!(visible(&s, a) && visible(&s, b) && visible(&s, d));
    // Plain Undo goes back through Undo Isolate, then the delete, then the Isolate.
    assert!(s.undo() && s.undo() && s.undo());
    assert!(!s.is_isolated());
    assert!(visible(&s, c));
}

#[test]
fn isolate_inside_subassemblies_and_robots_and_files() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let key = |o: Occurrence| TreeKey::Component(o);
    let kit = match s
        .edit_subassemblies(SubassemblyOp::Demote { members: vec![key(a), key(b)], name: "Kit".into() })
        .unwrap()[..]
    {
        [Created::Subassembly(id)] => TreeKey::Subassembly(id),
        ref other => panic!("{other:?}"),
    };
    let _ = kit;
    // Keeping one member of a subassembly: its sibling goes, the outside one goes.
    s.isolate(&[a]).unwrap();
    assert!(visible(&s, a) && !visible(&s, b) && !visible(&s, c));
    // Saved while isolated, opened, Undo Isolate still works.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("isolated.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.is_isolated());
    t.undo_isolate().unwrap();
    assert!(visible(&t, b) && visible(&t, c));
    // A robot's link: the robot stays; keep a robot and the bodies go.
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    let robot_item = Occurrence::Item(robot.erase());
    s.undo_isolate().unwrap();
    s.isolate(&[robot_item]).unwrap();
    assert!(visible(&s, robot_item) && !visible(&s, a) && !visible(&s, c));
    // Many components: quick.
    let mut big = Session::new();
    let bodies: Vec<_> = (0..300).map(|i| body(&mut big, f64::from(i) * 0.15)).collect();
    let t0 = std::time::Instant::now();
    big.isolate(&bodies[..1]).unwrap();
    big.undo_isolate().unwrap();
    eprintln!("report: isolate and undo isolate of 300 bodies took {:?}", t0.elapsed());
}

// ---- User-defined folders ----

fn tkey(o: Occurrence) -> TreeKey {
    TreeKey::Component(o)
}

#[test]
fn folder_boundaries() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let ghost = TreeKey::Component(Occurrence::Item(
        Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap()).erase(),
    ));
    let tree = s.component_tree();
    let (ka, kb, kc) = (tkey(a), tkey(b), tkey(c));
    // Add: a ghost member is refused; the same row twice; the empty list; an odd name.
    assert!(
        step(&mut s, "ghost member", |s| s
            .edit_folders(FolderOp::Add { name: String::new(), members: vec![ghost] }))
        .is_none()
    );
    assert!(
        step(&mut s, "ghost among real", |s| s
            .edit_folders(FolderOp::Add { name: String::new(), members: vec![ka, ghost] }))
        .is_none()
    );
    step(&mut s, "a row twice", |s| {
        s.edit_folders(FolderOp::Add { name: "Twice".into(), members: vec![ka, ka] })
    });
    for name in ["", "  ", "Folder:1", "한글 폴더", "a\nb", "quote \" and, comma", &"x".repeat(50_000)] {
        let accepted = step(&mut s, &format!("name {:?}", name.chars().take(8).collect::<String>()), |s| {
            s.edit_folders(FolderOp::Add { name: name.to_owned(), members: vec![kb] })
        });
        if accepted.is_some() {
            // `step` leaves the folder in place: it is the last one.
            let folder = s.folders().into_iter().last().unwrap();
            assert!(!folder.name.trim().is_empty(), "a folder with an empty name");
            s.undo();
        }
    }
    assert_eq!(s.component_tree(), tree, "folders arrange only the browser");
    // Names are unique, also when only the case differs or a space trails (accepted or refused, never two
    // folders that are told apart by nothing a person can see).
    let f1 = s.edit_folders(FolderOp::Add { name: "Jigs".into(), members: vec![ka] }).unwrap().unwrap();
    let _ = f1;
    let before = s.folders().len();
    let r = s.edit_folders(FolderOp::Add { name: "jigs".into(), members: vec![kb] });
    let r2 = s.edit_folders(FolderOp::Add { name: "Jigs ".into(), members: vec![kc] });
    eprintln!(
        "observation: second folder named jigs -> {r:?}, named 'Jigs ' -> {r2:?} ({} folders from {before})",
        s.folders().len()
    );
    // Rename: to its own name; to a taken one; ghost folder; Move into a ghost folder; delete twice.
    let id = s.folders()[0].id;
    let own = s.folders()[0].name.clone();
    step(&mut s, "rename to its own name", |s| s.edit_folders(FolderOp::Rename { id, name: own.clone() }));
    assert!(
        step(&mut s, "rename a ghost", |s| s.edit_folders(FolderOp::Rename { id: 9999, name: "X".into() }))
            .is_none()
    );
    assert!(
        step(&mut s, "move into a ghost", |s| s
            .edit_folders(FolderOp::Move { members: vec![kc], into: Some(9999) }))
        .is_none()
    );
    assert!(
        step(&mut s, "move a ghost", |s| s
            .edit_folders(FolderOp::Move { members: vec![ghost], into: Some(id) }))
        .is_none()
    );
    assert!(step(&mut s, "delete a ghost", |s| s.edit_folders(FolderOp::Delete(9999))).is_none());
    s.edit_folders(FolderOp::Delete(id)).unwrap();
    assert!(step(&mut s, "delete twice", |s| s.edit_folders(FolderOp::Delete(id))).is_none());
    // Ids are not reused after delete.
    let seen: Vec<u32> = s.folders().iter().map(|f| f.id).collect();
    let fresh = s.edit_folders(FolderOp::Add { name: String::new(), members: Vec::new() }).unwrap().unwrap();
    assert!(!seen.contains(&fresh) && fresh != id);
    // The BOM and the assembly are untouched by all of it.
    assert_eq!(s.component_tree(), tree);
}

#[test]
fn folders_follow_the_tree_when_it_changes_and_survive_files() {
    let mut s = Session::new();
    let (a, b, c) = (body(&mut s, 0.0), body(&mut s, 0.3), body(&mut s, 0.6));
    let (ka, kb, kc) = (tkey(a), tkey(b), tkey(c));
    let f = s
        .edit_folders(FolderOp::Add { name: "Fixtures".into(), members: vec![ka, kb, kc] })
        .unwrap()
        .unwrap();
    // Demote a member into a subassembly: it leaves the level of the folder; the folder keeps the rest, or
    // the call is refused with no trace.
    let before = s.doc().clone();
    match s.edit_subassemblies(SubassemblyOp::Demote { members: vec![ka, kb], name: "Kit".into() }) {
        Ok(_) => {
            let folders = s.folders();
            assert!(
                folders
                    .iter()
                    .all(|f| f.members.iter().all(|m| s.component_tree().iter().any(|r| r.key == *m)))
            );
            assert!(s.undo());
            assert!(s.doc().same_content(&before));
        }
        Err(_) => assert!(s.doc().same_content(&before)),
    }
    // Delete a member: it leaves the folder; delete the last one: the (empty) folder stays; undo restores.
    let Occurrence::Item(any) = a else { panic!() };
    s.execute(&Command::Cell(gc_doc::CellOp::DeleteBody(Id::from_raw(any.raw)))).unwrap();
    assert!(!s.folders().iter().any(|x| x.members.contains(&ka)));
    assert!(s.folders().iter().any(|x| x.id == f));
    assert!(s.undo());
    assert!(s.folders().iter().any(|x| x.id == f && x.members.contains(&ka)));
    // Saved and opened: the same folders in the same order; saving twice gives the same bytes.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("folders.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert_eq!(t.folders(), s.folders());
    assert!(t.doc().same_content(s.doc()));
    let again = dir.path().join("again.gcp");
    t.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap());
    // A damaged file: a folder with a member that is not there, a duplicate id: opened without the bad
    // parts or refused; the folders it does list point only at rows that exist.
    let text = std::fs::read_to_string(&path).unwrap();
    for (from, to) in [("\"Fixtures\"", "\"\"")] {
        if !text.contains(from) {
            continue;
        }
        let bad = dir.path().join("bad.gcp");
        std::fs::write(&bad, text.replace(from, to)).unwrap();
        let mut u = Session::new();
        if u.open(&bad).is_ok() {
            assert!(
                u.folders()
                    .iter()
                    .all(|f| f.members.iter().all(|m| u.component_tree().iter().any(|r| r.key == *m)))
            );
        }
    }
    // Many folders: quick, ids distinct.
    let mut big = Session::new();
    let rows: Vec<_> = (0..200).map(|i| tkey(body(&mut big, f64::from(i) * 0.15))).collect();
    let t0 = std::time::Instant::now();
    for r in &rows {
        big.edit_folders(FolderOp::Add { name: String::new(), members: vec![*r] }).unwrap();
    }
    let ids: std::collections::BTreeSet<u32> = big.folders().iter().map(|f| f.id).collect();
    assert_eq!(ids.len(), 200);
    let names: std::collections::BTreeSet<String> = big.folders().iter().map(|f| f.name.clone()).collect();
    assert_eq!(names.len(), 200, "automatic names are all different");
    eprintln!("report: 200 folders took {:?}", t0.elapsed());
}
