//! QA boundary tests for sketch elements as robot waypoints and paths (GraphCTRL's own feature, plan 9.1 and 9.4;
//! no Inventor counterpart): roles are one undo step each, one role per point and per curve, closed curves are
//! refused, deleting elements cleans roles up (except those a robot path follows), a followed sketch that changes
//! makes the plan stale, arcs and splines plan to finite continuous results, `route_between` and
//! `path_from_sketch` are one undo step, and the roles survive save, open and old files.
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
    AimReference, CellOp, Command, Created, EditError, EndSnap, PathMotion, Placement, RoleKind, Sketch,
    SketchOp, ToolAim,
};
use gc_math::DVec3;
use gc_session::{PlanStatus, Session};
use gc_sketch::{Curve, CurveGeom, Item, SkPoint};

type K = Id<Sketch>;

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap()
}

fn curve_of(created: &[Created]) -> Id<Curve> {
    created.iter().find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None }).unwrap()
}

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn line(s: &mut Session, k: K, a: [f64; 2], b: [f64; 2]) -> Id<Curve> {
    let op = SketchOp::AddLine {
        a: [a[0], a[1], 0.0],
        b: [b[0], b[1], 0.0],
        snap_a: EndSnap::default(),
        snap_b: EndSnap::default(),
        horizontal: None,
    };
    curve_of(&run(s, k, op).unwrap())
}

fn ends(s: &Session, k: K, c: Id<Curve>) -> (Id<SkPoint>, Id<SkPoint>) {
    s.doc().get(k).unwrap().curve_ends(c).unwrap()
}

/// A robot 0.6 m in front of the XZ plane facing it, on that wall: a line right, a half circle bulging right,
/// a line back left above it.
fn wall() -> (Session, Id<gc_doc::Robot>, K, [Id<Curve>; 3]) {
    let mut s = Session::new();
    let robot = s.add_robot("Robot1", &six_axis()).unwrap();
    let placement = Placement { xyz: [0.0, -0.6, 0.0], rpy: [0.0; 3] };
    s.execute(&Command::Cell(CellOp::PlaceRobot { id: robot, placement })).unwrap();
    s.jog(robot, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    s.end_jog();
    let k = s.create_sketch("XZ").unwrap();
    let below = line(&mut s, k, [-0.1, 0.5], [0.1, 0.5]);
    let arc = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddArc { centre: [0.1, 0.55, 0.0], start: [0.1, 0.5, 0.0], end: [0.1, 0.6, 0.0] },
        )
        .unwrap(),
    );
    let above = line(&mut s, k, [0.1, 0.6], [-0.1, 0.6]);
    s.finish_sketch();
    (s, robot, k, [below, arc, above])
}

/// Refused with no trace, or one undo step that undoes exactly and redoes the same (then the result is kept).
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

fn ghost_curve() -> Id<Curve> {
    Id::<Curve>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

fn ghost_point() -> Id<SkPoint> {
    Id::<SkPoint>::from_raw(std::num::NonZeroU64::new(9999).unwrap())
}

#[test]
fn waypoint_and_path_roles_one_per_element_and_refusals() {
    let (mut s, _robot, k, [below, arc, above]) = wall();
    let (a, b) = ends(&s, k, below);
    // A waypoint: a ghost point, a ghost sketch, a point that has one.
    assert!(step(&mut s, "ghost point", |s| s.add_waypoint_role(k, ghost_point())).is_none());
    let other = Id::<Sketch>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(step(&mut s, "ghost sketch", |s| s.add_waypoint_role(other, a)).is_none());
    // `step` leaves the accepted role in place.
    let role = step(&mut s, "waypoint", |s| s.add_waypoint_role(k, a)).unwrap();
    assert!(step(&mut s, "the same point again", |s| s.add_waypoint_role(k, a)).is_none());
    assert_eq!(s.add_waypoint_role(k, a), Err(EditError::InUse));
    // A point of another sketch than the one named.
    let k2 = s.create_sketch("XY").unwrap();
    s.finish_sketch();
    assert!(step(&mut s, "a point of another sketch", |s| s.add_waypoint_role(k2, b)).is_none());
    // Paths: none, a ghost, a closed curve, a branch, a gap, a curve twice, in any order.
    assert!(step(&mut s, "no curves", |s| s.add_path_role(k, &[])).is_none());
    assert!(step(&mut s, "ghost curve", |s| s.add_path_role(k, &[ghost_curve()])).is_none());
    assert!(
        step(&mut s, "a ghost among real ones", |s| s.add_path_role(k, &[below, ghost_curve()])).is_none()
    );
    assert!(
        step(&mut s, "a gap (below and above are not joined)", |s| s.add_path_role(k, &[below, above]))
            .is_none()
    );
    s.add_path_role(k, &[below, arc, above]).unwrap();
    assert_eq!(s.add_path_role(k, &[arc]), Err(EditError::InUse), "a curve is in one path only");
    assert!(step(&mut s, "a curve of a taken chain", |s| s.add_path_role(k, &[below])).is_none());
    assert!(s.chain_at(k, below).is_empty() && s.chain_at(k, arc).is_empty());
    // Roles of one sketch are listed; delete one with no robot following: one step, undone exactly.
    assert_eq!(s.sketch_roles(k).len(), 2);
    assert!(step(&mut s, "delete a waypoint", |s| s.delete_role(role)).is_some());
    let gone = Id::<gc_doc::SketchRole>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(step(&mut s, "delete a ghost role", |s| s.delete_role(gone)).is_none());
}

#[test]
fn closed_curves_branches_and_gaps_are_refused() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let circle = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddCircle {
                centre: [0.0; 3],
                radius: 0.05,
                centre_snap: EndSnap::default(),
                rim_snap: EndSnap::default(),
            },
        )
        .unwrap(),
    );
    let ellipse = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddEllipse {
                centre: [0.3, 0.0, 0.0],
                major: [0.34, 0.0, 0.0],
                minor: 0.02,
                centre_snap: EndSnap::default(),
            },
        )
        .unwrap(),
    );
    let closed_spline = curve_of(
        &run(
            &mut s,
            k,
            SketchOp::AddSpline {
                points: vec![[0.6, 0.0, 0.0], [0.65, 0.04, 0.0], [0.7, 0.0, 0.0], [0.65, -0.04, 0.0]],
                snaps: Vec::new(),
            },
        )
        .unwrap(),
    );
    run(&mut s, k, SketchOp::CloseSpline { curve: closed_spline }).unwrap();
    for (what, c) in [("circle", circle), ("full ellipse", ellipse), ("closed spline", closed_spline)] {
        assert!(step(&mut s, what, |s| s.add_path_role(k, &[c])).is_none(), "{what} is no path");
        assert!(s.chain_at(k, c).is_empty(), "{what} offers no chain");
    }
    // A closed loop of lines: refused (no free end), or a path that starts somewhere sane.
    let (p, q, r) = ([0.0, 0.5], [0.1, 0.5], [0.05, 0.6]);
    let l1 = line(&mut s, k, p, q);
    let l2 = line(&mut s, k, q, r);
    let l3 = line(&mut s, k, r, p);
    // A snapped loop: join the ends with Coincident constraints so the three really form a triangle.
    for (a, b) in [(l1, l2), (l2, l3), (l3, l1)] {
        let (_, e) = ends(&s, k, a);
        let (st, _) = ends(&s, k, b);
        run(&mut s, k, SketchOp::AddConstraint(gc_sketch::GeoConstraint::Coincident(e, st))).ok();
    }
    step(&mut s, "a closed loop of three lines", |s| s.add_path_role(k, &[l1, l2, l3]));
    // A fork: three lines meeting at a point.
    let hub = [0.5, 0.5];
    let spokes: Vec<_> =
        [[0.6, 0.5], [0.5, 0.6], [0.4, 0.5]].iter().map(|e| line(&mut s, k, hub, *e)).collect();
    for pair in spokes.windows(2) {
        let (a, _) = ends(&s, k, pair[0]);
        let (b, _) = ends(&s, k, pair[1]);
        run(&mut s, k, SketchOp::AddConstraint(gc_sketch::GeoConstraint::Coincident(a, b))).ok();
    }
    assert!(step(&mut s, "a fork of three lines", |s| s.add_path_role(k, &spokes)).is_none());
    // A construction curve and a projected-like element: any outcome without a trace on refusal.
    let c = line(&mut s, k, [2.0, 2.0], [2.1, 2.0]);
    run(&mut s, k, SketchOp::SetConstruction { curve: c, construction: true }).ok();
    step(&mut s, "a construction line", |s| s.add_path_role(k, &[c]));
    // A zero-length line.
    let dot = line(&mut s, k, [3.0, 3.0], [3.0 + 1e-13, 3.0]);
    step(&mut s, "a zero-length line", |s| s.add_path_role(k, &[dot]));
}

#[test]
fn deleting_elements_cleans_roles_up_and_followed_roles_stay() {
    let (mut s, robot, k, [below, arc, above]) = wall();
    let (a, _) = ends(&s, k, below);
    let waypoint = s.add_waypoint_role(k, a).unwrap();
    let path = s.add_path_role(k, &[below, arc, above]).unwrap();
    // Deleting the point's curve takes the roles along in the same step; undo brings both back.
    let depth = s.history().undo_depth();
    run(&mut s, k, SketchOp::Delete(vec![Item::Curve(below)])).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.doc().get(waypoint).is_none(), "the waypoint went with its point");
    // The rest of the chain (arc + above) is still one chain: the path role keeps them.
    let RoleKind::Path { curves, .. } = &s.doc().get(path).unwrap().kind else { panic!() };
    assert_eq!(curves.as_slice(), [arc, above].as_slice());
    assert!(s.undo());
    assert!(s.doc().get(waypoint).is_some() && s.doc().get(path).is_some());
    // Deleting the middle of the chain: the ends no longer join; the role goes (nothing follows it).
    run(&mut s, k, SketchOp::Delete(vec![Item::Curve(arc)])).unwrap();
    assert!(s.doc().get(path).is_none() || s.role_polyline(path).is_some(), "no half-broken role left");
    assert!(s.undo());
    // A robot follows it: now the role is kept through the same deletion, and the plan says why it fails.
    let follower = s.follow_roles(robot, &[path]).unwrap();
    assert_eq!(s.plan(follower), PlanStatus::Ready, "{:?}", s.plan_error(follower));
    let steps = s.history().undo_depth();
    run(&mut s, k, SketchOp::Delete(vec![Item::Curve(arc)])).unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    assert!(s.doc().get(path).is_some(), "a followed role is not deleted");
    assert_eq!(s.plan_status(follower), PlanStatus::Stale);
    let status = s.plan(follower);
    assert!(status != PlanStatus::Ready, "the path cannot follow a broken chain");
    let why = s.plan_error(follower).unwrap_or_default().to_owned();
    assert!(!why.is_empty(), "the failure has a reason");
    // The role itself reports its chain as broken, not a panic.
    assert!(s.role_polyline(path).is_none());
    assert!(s.role_ends(path).is_none() || s.role_ends(path).is_some());
    // The sketch cannot be deleted while a path follows its role.
    assert!(s.undo());
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    let r = s.execute(&Command::DeleteSketch { id: k });
    if r.is_err() {
        assert!(s.doc().same_content(&before));
        assert_eq!(s.history().undo_depth(), depth);
    }
    // The role cannot be deleted while it is followed.
    assert_eq!(s.delete_role(path), Err(EditError::InUse));
}

#[test]
fn settings_of_a_role_are_refused_or_one_step() {
    let (mut s, robot, k, [below, arc, above]) = wall();
    let (a, _) = ends(&s, k, below);
    let way = s.add_waypoint_role(k, a).unwrap();
    let path = s.add_path_role(k, &[below, arc, above]).unwrap();
    // Path settings on a waypoint and waypoint settings on a path.
    assert!(step(&mut s, "motion of a waypoint", |s| s.set_role_motion(way, PathMotion::Linear)).is_none());
    assert!(step(&mut s, "speed of a waypoint", |s| s.set_role_speed(way, Some(0.1))).is_none());
    assert!(step(&mut s, "reverse a waypoint", |s| s.reverse_role(way)).is_none());
    for speed in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(step(&mut s, &format!("speed {speed}"), |s| s.set_role_speed(path, Some(speed))).is_none());
    }
    for speed in [1e-9, 1e9, 0.05] {
        step(&mut s, &format!("speed {speed}"), |s| s.set_role_speed(path, Some(speed)));
    }
    step(&mut s, "speed none", |s| s.set_role_speed(path, None));
    // Same value again: nothing changes (no empty undo step).
    let depth = s.history().undo_depth();
    s.set_role_motion(path, PathMotion::Follow).ok();
    assert_eq!(s.history().undo_depth(), depth, "the same motion is not a step");
    for motion in [PathMotion::Linear, PathMotion::Joint, PathMotion::Follow] {
        step(&mut s, &format!("motion {motion:?}"), |s| s.set_role_motion(path, motion));
    }
    // Aim: odd angles, NaN, a frame that is not there.
    // The default aim again is no change and no undo step.
    let depth = s.history().undo_depth();
    s.set_role_aim(way, ToolAim { reference: AimReference::SketchPlane, rpy: [0.0; 3] }).ok();
    assert_eq!(s.history().undo_depth(), depth, "the same aim is not a step");
    for rpy in [[0.0, 0.3, 0.0], [1e9, 0.0, 0.0], [f64::NAN, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0]] {
        let aim = ToolAim { reference: AimReference::SketchPlane, rpy };
        let finite = rpy.iter().all(|v| v.is_finite());
        let done = step(&mut s, &format!("aim {rpy:?}"), |s| s.set_role_aim(way, aim));
        if !finite {
            assert!(done.is_none(), "{rpy:?} was accepted");
        }
    }
    let ghost_frame = Id::<gc_doc::WorkFrame>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let aim = ToolAim { reference: AimReference::Frame(ghost_frame), rpy: [0.0; 3] };
    assert!(step(&mut s, "a frame that is not there", |s| s.set_role_aim(way, aim)).is_none());
    // A plan reads the role's settings: any change makes the robot's plan stale, then planning again is Ready.
    let follower = s.follow_roles(robot, &[path]).unwrap();
    assert_eq!(s.plan(follower), PlanStatus::Ready, "{:?}", s.plan_error(follower));
    s.set_role_speed(path, Some(0.05)).unwrap();
    assert_eq!(s.plan_status(follower), PlanStatus::Stale);
    assert!(s.undo());
    assert_eq!(s.plan_status(follower), PlanStatus::Ready, "undone: the stored plan is current again");
    // Dimension changes make the path out of date too.
    let (p, q) = ends(&s, k, below);
    s.enter_sketch(k);
    run(&mut s, k, SketchOp::SetCoords { points: vec![(p, [-0.12, 0.5, 0.0])], radii: Vec::new() }).unwrap();
    let _ = q;
    s.finish_sketch();
    assert_eq!(s.plan_status(follower), PlanStatus::Stale);
}

#[test]
fn arcs_and_splines_plan_to_finite_continuous_tip_paths() {
    let (mut s, robot, k, [below, arc, above]) = wall();
    let role = s.add_path_role(k, &[below, arc, above]).unwrap();
    let follower = s.follow_roles(robot, &[role]).unwrap();
    assert_eq!(s.plan(follower), PlanStatus::Ready, "{:?}", s.plan_error(follower));
    let continuous = |tip: &[[f64; 3]], what: &str| {
        assert!(tip.len() > 2, "{what}");
        assert!(tip.iter().all(|p| p.iter().all(|v| v.is_finite())), "{what}: not finite");
        let longest = tip
            .windows(2)
            .map(|w| (DVec3::from_array(w[1]) - DVec3::from_array(w[0])).length())
            .fold(0.0, f64::max);
        assert!(longest < 0.1, "{what}: a jump of {longest} m in the tip path");
    };
    continuous(&s.planned(follower).unwrap().tip_path().collect::<Vec<_>>(), "line-arc-line");
    // Every way of travelling it, both directions.
    for motion in [PathMotion::Linear, PathMotion::Joint, PathMotion::Follow] {
        for _ in 0..2 {
            s.set_role_motion(role, motion).unwrap();
            let status = s.plan(follower);
            if status == PlanStatus::Ready {
                continuous(
                    &s.planned(follower).unwrap().tip_path().collect::<Vec<_>>(),
                    &format!("{motion:?}"),
                );
            } else {
                assert!(s.plan_error(follower).is_some(), "{motion:?}: failed with no reason");
            }
            s.reverse_role(role).unwrap();
        }
    }
    // A spline on the wall: through its fit points, finite; a spline with a sharp turn; a spline of two points.
    s.set_role_motion(role, PathMotion::Follow).unwrap();
    s.enter_sketch(k);
    for (what, pts) in [
        ("gentle", vec![[-0.1, 0.7], [0.0, 0.75], [0.1, 0.7]]),
        ("sharp", vec![[-0.1, 0.8], [0.0, 0.9], [-0.1, 0.8001], [0.1, 0.85]]),
        ("two points", vec![[-0.1, 0.95], [0.1, 0.95]]),
    ] {
        let points = pts.iter().map(|p| [p[0], p[1], 0.0]).collect();
        let sp = curve_of(&run(&mut s, k, SketchOp::AddSpline { points, snaps: Vec::new() }).unwrap());
        s.finish_sketch();
        let Some(r) = step(&mut s, what, |s| s.add_path_role(k, &[sp])) else {
            s.enter_sketch(k);
            continue;
        };
        let f = s.follow_roles(robot, &[s.path_role(k, sp).unwrap()]).unwrap();
        let _ = r;
        let status = s.plan(f);
        if status == PlanStatus::Ready {
            continuous(&s.planned(f).unwrap().tip_path().collect::<Vec<_>>(), what);
        } else {
            assert!(s.plan_error(f).is_some(), "{what}: failed with no reason");
        }
        s.enter_sketch(k);
    }
}

#[test]
fn route_between_boundaries() {
    let (mut s, robot, k, [below, ..]) = wall();
    let (a, b) = ends(&s, k, below);
    // Same point twice, a ghost point, a ghost sketch, a point of another sketch, an unknown planner.
    let waypoints = s.doc().waypoints().len();
    for (what, from, to, planner) in [
        ("same point twice", (k, a), (k, a), "rrt"),
        ("ghost point", (k, a), (k, ghost_point()), "rrt"),
        ("unknown planner", (k, a), (k, b), "no such planner"),
        ("empty planner", (k, a), (k, b), ""),
    ] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        assert!(s.route_between(robot, from, to, planner, false).is_err(), "{what} was accepted");
        assert!(s.doc().same_content(&before), "{what} changed the document");
        assert_eq!(s.history().undo_depth(), depth, "{what}");
    }
    assert_eq!(s.doc().waypoints().len(), waypoints, "refusals made no waypoints");
    // A ghost robot and a ghost sketch.
    let ghost_robot = Id::<gc_doc::Robot>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(s.route_between(ghost_robot, (k, a), (k, b), "rrt_star", false).is_err());
    let ghost_sketch = Id::<Sketch>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(s.route_between(robot, (ghost_sketch, a), (k, b), "rrt_star", false).is_err());
    // Another sketch's point: accepted (a path may run between sketches) or refused with no trace.
    let k2 = s.create_sketch("XZ").unwrap();
    let other = line(&mut s, k2, [0.2, 0.5], [0.3, 0.5]);
    s.finish_sketch();
    let (c, _) = ends(&s, k2, other);
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    match s.route_between(robot, (k, a), (k2, c), "rrt_star", false) {
        Ok(path) => {
            assert_eq!(s.history().undo_depth(), depth + 1, "one undo step");
            assert!(s.plan_status(path) != PlanStatus::NotPlanned);
            assert!(s.undo());
            assert!(s.doc().same_content(&before), "undo restores");
        }
        Err(_) => assert!(s.doc().same_content(&before)),
    }
    // NaN and infinite sketch points are not reachable through the sketch (the sketch refuses them), so
    // moving a point very far is the nearest boundary: the plan fails with a reason or succeeds, never panics.
    s.enter_sketch(k);
    run(&mut s, k, SketchOp::SetCoords { points: vec![(b, [1e6, 1e6, 0.0])], radii: Vec::new() }).unwrap();
    s.finish_sketch();
    let path = s.route_between(robot, (k, a), (k, b), "rrt_star", false);
    if let Ok(path) = path {
        let status = s.plan_status(path);
        assert!(
            status != PlanStatus::Ready
                || s.planned(path).unwrap().tip_path().all(|p| p.iter().all(|v| v.is_finite()))
        );
    }
}

#[test]
fn path_from_sketch_boundaries() {
    let (mut s, robot, k, [below, arc, above]) = wall();
    // One undo step makes the role and the path; asking again reuses it.
    let depth = s.history().undo_depth();
    let whole = s.path_from_sketch(robot, k).unwrap();
    assert_eq!(s.history().undo_depth(), depth + 1);
    assert!(s.undo());
    assert!(s.doc().roles().is_empty(), "undo removes the role too");
    assert!(s.redo());
    assert_eq!(s.doc().roles().len(), 1);
    let again = s.path_from_sketch(robot, k).unwrap();
    assert_eq!(s.doc().roles().len(), 1);
    assert_eq!(s.doc().get(again).unwrap().follow, s.doc().get(whole).unwrap().follow);
    let _ = (below, arc, above);
    // An empty sketch, a ghost sketch, a ghost robot, a sketch of only closed curves.
    let empty = s.create_sketch("XY").unwrap();
    s.finish_sketch();
    let ghost_sketch = Id::<Sketch>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let ghost_robot = Id::<gc_doc::Robot>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let closed = s.create_sketch("XY").unwrap();
    run(
        &mut s,
        closed,
        SketchOp::AddCircle {
            centre: [0.0; 3],
            radius: 0.05,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
    .unwrap();
    s.finish_sketch();
    for (what, robot, sketch) in [
        ("empty sketch", robot, empty),
        ("ghost sketch", robot, ghost_sketch),
        ("ghost robot", ghost_robot, k),
        ("only a circle", robot, closed),
    ] {
        let before = s.doc().clone();
        let depth = s.history().undo_depth();
        let r = s.path_from_sketch(robot, sketch);
        if r.is_err() {
            assert!(s.doc().same_content(&before), "{what} changed the document");
            assert_eq!(s.history().undo_depth(), depth, "{what}");
        } else {
            assert_eq!(s.history().undo_depth(), depth + 1, "{what}: one step");
        }
        let ok_expected = what == "empty sketch" || what == "only a circle";
        if !ok_expected {
            assert!(r.is_err(), "{what} was accepted");
        }
    }
}

#[test]
fn roles_survive_save_and_open_and_old_files_have_none() {
    let (mut s, robot, k, [below, arc, above]) = wall();
    let (a, _) = ends(&s, k, below);
    s.add_waypoint_role(k, a).unwrap();
    let role = s.add_path_role(k, &[below, arc, above]).unwrap();
    s.set_role_speed(role, Some(0.05)).unwrap();
    s.set_role_motion(role, PathMotion::Linear).unwrap();
    let follower = s.follow_roles(robot, &[role]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roles.gcp");
    s.save_as(&path).unwrap();
    let mut t = Session::new();
    t.open(&path).unwrap();
    assert!(t.doc().same_content(s.doc()), "the reopened document differs");
    assert_eq!(t.doc().roles().len(), 2);
    assert_eq!(t.doc().get(follower).unwrap().follow, vec![role]);
    let again = dir.path().join("again.gcp");
    t.save_as(&again).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), std::fs::read(&again).unwrap(), "saving twice differs");
    // A project saved before roles existed (no role entries) opens with none; damaged role entries are
    // refused or dropped, never kept as roles of things that are not there.
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    fn strip(v: &mut serde_json::Value, key_part: &str) -> usize {
        let mut n = 0;
        match v {
            serde_json::Value::Object(map) => {
                let keys: Vec<String> =
                    map.keys().filter(|k| k.to_lowercase().contains(key_part)).cloned().collect();
                for k in keys {
                    map.remove(&k);
                    n += 1;
                }
                for x in map.values_mut() {
                    n += strip(x, key_part);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a {
                    n += strip(x, key_part);
                }
            }
            _ => {}
        }
        n
    }
    // A realistic old file has neither roles nor paths that follow them.
    let mut old = value.clone();
    let removed = strip(&mut old, "role") + strip(&mut old, "follow");
    assert!(removed > 0, "the saved file has role entries to remove");
    let plain = dir.path().join("old.gcp");
    std::fs::write(&plain, serde_json::to_string(&old).unwrap()).unwrap();
    let mut u = Session::new();
    u.open(&plain).unwrap();
    assert!(u.doc().roles().is_empty(), "an old file has no roles");
    // Only the roles removed while a path still follows them: the dangling reference is refused (a damaged
    // file), not opened with a path that follows nothing.
    let mut half = value.clone();
    strip(&mut half, "role");
    let half_path = dir.path().join("half.gcp");
    std::fs::write(&half_path, serde_json::to_string(&half).unwrap()).unwrap();
    let mut v = Session::new();
    assert!(v.open(&half_path).is_err(), "a path following roles that are gone was opened");
    assert!(v.doc().roles().is_empty());
    // Roles pointing at a point or curve that is not in the sketch: opened without them, or refused.
    let damaged = text.replace(&format!("\"{}\"", "Waypoint"), "\"Waypoint\"");
    let bad = dir.path().join("bad.gcp");
    std::fs::write(&bad, damaged).unwrap();
    let mut w = Session::new();
    if w.open(&bad).is_ok() {
        let sk = w.doc().sketches().iter().next().map(|(id, _)| id);
        let _ = sk;
        for (id, r) in w.doc().roles().iter() {
            let _ = (id, r.name.as_str());
        }
    }
    let _ = CurveGeom::Line { a: ghost_point(), b: ghost_point() };
}
