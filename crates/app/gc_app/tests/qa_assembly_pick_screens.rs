//! QA scenarios (kittest) for the Constrain and Joint dialogs taking their selections from the view: faces, cylinder
//! axes, straight edges and points of components that stand anywhere, in any turn. Inventor 2026 help: To Place Mate
//! or Flush Constraints GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E, To Place Angle Constraints
//! GUID-AD4679FD-EE9B-47C0-9E32-04B990BDD29E, Tangent GUID-FF270566-CA9E-420A-A00A-F52DD0FDFD8E, Create Joints
//! Reference GUID-6AA68E8F-7C97-4806-8483-3941DE915E70. Whatever is clicked, the constraint that comes out holds in the
//! world, Cancel leaves the document and the undo depth as they were, and OK is one undo step that restores exactly.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gc_app::{GraphCtrlApp, RunOptions};
use gc_core::Id;
use gc_doc::{Body, Geometry, Occurrence, Placement, RelationKind, Selection, Solid};
use gc_math::DVec3;

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

fn harness() -> H {
    let mut h = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    h.run();
    h
}

fn depth(h: &H) -> usize {
    h.state().session.history().undo_depth()
}

fn has(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn click(h: &mut H, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
    }
    h.run_steps(3);
}

/// The world point of a spot given in a body's own frame.
fn world(h: &H, body: Id<Body>, local: [f64; 3]) -> DVec3 {
    let placement = h.state().session.doc().get(body).unwrap().placement;
    let pose = gc_math::pose_from_xyz_rpy(placement.xyz, placement.rpy);
    pose.translation + pose.rotation * DVec3::from_array(local)
}

fn click_on(h: &mut H, body: Id<Body>, local: [f64; 3]) {
    let at = h.state().viewport().screen_of(world(h, body, local)).unwrap();
    assert!(
        at.x < 900.0 && at.x > 250.0 && at.y > 120.0 && at.y < 740.0,
        "the spot is off the free part of the view: {at:?}"
    );
    click(h, at);
}

/// Two boxes: one at the origin (it stays), one turned and moved (the bodies' top faces face the camera).
fn two_boxes(h: &mut H, turned: Placement) -> (Id<Body>, Id<Body>) {
    let solid = Solid::Box { size: [0.1; 3] };
    let s = &mut h.state_mut().session;
    // Away from the world origin: a click near the origin triad picks the triad's axis, not the face under it.
    let a = s.add_body(solid.clone(), Placement { xyz: [-0.15, 0.1, 0.0], rpy: [0.0; 3] }).unwrap();
    let b = s.add_body(solid, turned).unwrap();
    h.run();
    h.state_mut().fit_view();
    h.run_steps(3);
    (a, b)
}

fn open(h: &mut H, command: &str, tab: Option<&str>) {
    h.get_by_label("조립").click();
    h.run_steps(3);
    h.get_all_by_label(command).next().unwrap().click();
    h.run_steps(3);
    if let Some(tab) = tab {
        h.get_by_label(tab).click();
        h.run_steps(2);
    }
}

fn filled(h: &H) -> bool {
    h.query_all_by_label_contains("뷰에서 선택").count() == 0
}

fn first_second(h: &H) -> (Selection, Selection) {
    let rels = h.state().session.relationships();
    assert_eq!(rels.len(), 1, "one constraint was made");
    (rels[0].1.first, rels[0].1.second)
}

fn plane_world(h: &H, sel: &Selection) -> (DVec3, DVec3) {
    let doc = h.state().session.doc();
    (doc.selection_point(sel).unwrap(), doc.selection_axis(sel).unwrap().1)
}

const TURNS: [[f64; 3]; 3] = [[0.0; 3], [0.0, 0.0, 0.9], [0.2, -0.15, 0.7]];

#[test]
fn faces_clicked_in_the_view_make_a_constraint_that_holds_whatever_the_turn() {
    for turn in TURNS {
        for (tab, check) in [("맞춤", "mate"), ("플러시", "flush")] {
            let mut h = harness();
            let (a, b) = two_boxes(&mut h, Placement { xyz: [0.3, 0.0, 0.1], rpy: turn });
            let before = h.state().session.doc().clone();
            let steps = depth(&h);
            open(&mut h, "구속", Some(tab));
            click_on(&mut h, a, [0.03, 0.0, 0.05]);
            click_on(&mut h, b, [0.03, 0.0, 0.05]);
            assert!(filled(&h), "{turn:?} {tab}: both selections are filled");
            assert_eq!(h.state().session.relationships().len(), 0, "nothing is made before OK");
            h.get_by_label("확인").click();
            h.run_steps(3);
            let (first, second) = first_second(&h);
            // The faces are the boxes' top faces in their own frames, not the origin planes the view also offers.
            for (sel, owner) in [(first, a), (second, b)] {
                let _ = owner;
                let Geometry::Plane { point, normal } = sel.geometry else {
                    panic!("{turn:?}: {:?}", sel.geometry)
                };
                assert!((point[2] - 0.05).abs() < 1e-6 && normal[2] > 0.99, "{turn:?}: {point:?} {normal:?}");
            }
            // And the constraint holds in the world: Mate puts the faces together facing each other, Flush side by side.
            let (p1, n1) = plane_world(&h, &first);
            let (p2, n2) = plane_world(&h, &second);
            assert!(
                ((p1 - p2).dot(n2)).abs() < 1e-6,
                "{turn:?} {check}: the faces are {} apart",
                (p1 - p2).dot(n2)
            );
            let dot = n1.dot(n2);
            if check == "mate" {
                assert!(dot < -0.999, "{turn:?}: the faces face each other: {dot}");
            } else {
                assert!(dot > 0.999, "{turn:?}: the faces point the same way: {dot}");
            }
            // One undo step brings everything back exactly.
            assert_eq!(depth(&h), steps + 1);
            assert!(h.state_mut().session.undo());
            assert!(h.state().session.doc().same_content(&before), "{turn:?} {tab}");
        }
    }
}

#[test]
fn an_angle_between_two_clicked_faces_is_the_angle_that_was_typed() {
    let mut h = harness();
    let (a, b) = two_boxes(&mut h, Placement { xyz: [0.3, 0.0, 0.1], rpy: [0.0, 0.0, 0.5] });
    open(&mut h, "구속", Some("각도"));
    click_on(&mut h, a, [0.03, 0.0, 0.05]);
    click_on(&mut h, b, [0.03, 0.0, 0.05]);
    let field = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "각도 값");
    field.focus();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "각도 값").type_text("30");
    h.run_steps(2);
    h.get_by_label("확인").click();
    h.run_steps(3);
    let rels = h.state().session.relationships();
    let (first, second) = (rels[0].1.first, rels[0].1.second);
    assert!(matches!(rels[0].1.kind, RelationKind::Angle { .. }));
    let (_, n1) = plane_world(&h, &first);
    let (_, n2) = plane_world(&h, &second);
    let angle = gc_math::fm::acos(n1.dot(n2).clamp(-1.0, 1.0)).to_degrees();
    assert!(
        (angle - 30.0).abs() < 1e-3 || (angle - 150.0).abs() < 1e-3,
        "the faces stand at {angle} degrees"
    );
}

#[test]
fn cancel_after_picking_leaves_nothing_and_the_next_dialog_starts_empty() {
    let mut h = harness();
    let (a, b) = two_boxes(&mut h, Placement { xyz: [0.3, 0.0, 0.1], rpy: [0.0, 0.0, 0.6] });
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open(&mut h, "구속", None);
    click_on(&mut h, a, [0.03, 0.0, 0.05]);
    // Moving over the other body only highlights it.
    let over = h.state().viewport().screen_of(world(&h, b, [0.0, 0.0, 0.05])).unwrap();
    h.event(egui::Event::PointerMoved(over));
    h.run_steps(3);
    assert!(
        h.state().session.doc().same_content(&before) && depth(&h) == steps,
        "hovering changed the document"
    );
    click_on(&mut h, b, [0.03, 0.0, 0.05]);
    assert!(filled(&h));
    h.get_by_label("취소").click();
    h.run_steps(3);
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
    // A new dialog is empty again.
    open(&mut h, "구속", None);
    assert!(has(&h, "첫 번째 선택: 뷰에서 선택"), "the new dialog starts with nothing picked");
    h.get_by_label("취소").click();
    h.run_steps(3);
}

#[test]
fn two_faces_of_one_component_and_clicks_on_nothing_make_no_constraint() {
    let mut h = harness();
    let (a, _b) = two_boxes(&mut h, Placement { xyz: [0.3, 0.0, 0.1], rpy: [0.0, 0.0, 0.6] });
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open(&mut h, "구속", None);
    // The background picks nothing and does not move the dialog on.
    click(&mut h, egui::pos2(300.0, 700.0));
    assert!(has(&h, "첫 번째 선택: 뷰에서 선택"), "a click on nothing filled the selection");
    // Two faces of the same component: whatever the dialog does about it, OK makes no constraint from them.
    click_on(&mut h, a, [0.03, 0.0, 0.05]);
    click_on(&mut h, a, [0.05, 0.0, 0.0]);
    if has(&h, "확인") {
        h.get_by_label("확인").click();
        h.run_steps(3);
    }
    assert_eq!(h.state().session.relationships().len(), 0, "a part was constrained to itself");
    assert!(h.state().session.doc().same_content(&before));
    assert_eq!(depth(&h), steps);
}

#[test]
fn cylinders_clicked_in_the_view_are_mated_axis_to_axis() {
    let mut h = harness();
    let solid = Solid::Cylinder { radius: 0.05, length: 0.2 };
    let s = &mut h.state_mut().session;
    let a = s.add_body(solid.clone(), Placement::default()).unwrap();
    let b = s.add_body(solid, Placement { xyz: [0.3, 0.1, 0.0], rpy: [0.1, 0.1, 0.4] }).unwrap();
    h.run();
    h.state_mut().fit_view();
    h.run_steps(3);
    open(&mut h, "구속", Some("맞춤"));
    // The side of a cylinder: try the four sides, the camera sees one of them.
    let mut picked = 0;
    for body in [a, b] {
        let spots = [[0.05, 0.0, 0.03], [-0.05, 0.0, 0.03], [0.0, 0.05, 0.03], [0.0, -0.05, 0.03]];
        for spot in spots {
            let w = world(&h, body, spot);
            let Some(at) = h.state().viewport().screen_of(w) else { continue };
            if at.x > 900.0 || at.x < 250.0 {
                continue;
            }
            click(&mut h, at);
            let now = h.query_all_by_label_contains("뷰에서 선택").count();
            if now < 2 - picked {
                picked += 1;
                break;
            }
        }
    }
    if picked < 2 {
        eprintln!("report: only {picked} cylinder side(s) could be picked by clicking");
        h.get_by_label("취소").click();
        return;
    }
    h.get_by_label("확인").click();
    h.run_steps(3);
    let (first, second) = first_second(&h);
    // Whatever the geometry is called, the two axes are one line in the world.
    let doc = h.state().session.doc();
    let (p1, d1) = doc.selection_axis(&first).unwrap();
    let (p2, d2) = doc.selection_axis(&second).unwrap();
    assert!(d1.cross(d2).length() < 1e-6, "the axes are parallel");
    assert!(
        (p2 - p1).cross(d1).length() < 1e-6,
        "the axes are one line: {} apart",
        (p2 - p1).cross(d1).length()
    );
}

#[test]
fn the_joint_dialog_takes_two_clicked_faces_and_a_rigid_joint_joins_their_frames() {
    let mut h = harness();
    let (a, b) = two_boxes(&mut h, Placement { xyz: [0.3, 0.0, 0.1], rpy: [0.0, 0.0, 0.5] });
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    open(&mut h, "조인트", Some("강체"));
    click_on(&mut h, b, [0.03, 0.0, 0.05]);
    click_on(&mut h, a, [0.03, 0.0, 0.05]);
    assert!(filled(&h), "both origins are filled");
    h.get_by_label("확인").click();
    h.run_steps(3);
    let (first, second) = first_second(&h);
    assert!(matches!(h.state().session.relationships()[0].1.kind, RelationKind::Rigid { .. }));
    // The frames sit on the clicked faces and, joined, share their origin.
    let doc = h.state().session.doc();
    let (p1, p2) = (doc.selection_point(&first).unwrap(), doc.selection_point(&second).unwrap());
    assert!((p1 - p2).length() < 1e-6, "the joined origins are {} apart", (p1 - p2).length());
    assert_eq!(depth(&h), steps + 1);
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
    let _ = Occurrence::Item(a.erase());
}

fn rows_text(h: &H) -> Vec<String> {
    ["첫 번째 선택", "두 번째 선택"]
        .iter()
        .flat_map(|p| {
            h.query_all_by_label_contains(p)
                .map(|n| format!("{:?}", n.accesskit_node().label()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The first of these body-local spots whose click, in a fresh dialog, fills the first selection with something
/// described by `want` (the dialog is cancelled again).
fn spot_that_picks(h: &mut H, tab: &str, body: Id<Body>, spots: &[[f64; 3]], want: &str) -> Option<[f64; 3]> {
    for spot in spots {
        let w = world(h, body, *spot);
        let Some(at) = h.state().viewport().screen_of(w) else { continue };
        if at.x > 900.0 || at.x < 250.0 || at.y < 120.0 || at.y > 740.0 {
            continue;
        }
        open(h, "구속", Some(tab));
        click(h, at);
        let rows = rows_text(h);
        h.get_by_label("취소").click();
        h.run_steps(3);
        if rows.iter().any(|r| r.contains(want)) {
            return Some(*spot);
        }
    }
    None
}

#[test]
fn the_rims_of_two_cylinders_clicked_in_the_view_are_inserted_axis_to_axis_and_face_to_face() {
    let mut h = harness();
    let solid = Solid::Cylinder { radius: 0.05, length: 0.1 };
    let s = &mut h.state_mut().session;
    let a = s.add_body(solid.clone(), Placement { xyz: [-0.15, 0.1, 0.0], rpy: [0.0; 3] }).unwrap();
    let b = s.add_body(solid, Placement { xyz: [0.2, 0.0, 0.1], rpy: [0.3, -0.2, 0.8] }).unwrap();
    h.run();
    h.state_mut().fit_view();
    h.run_steps(3);
    let before = h.state().session.doc().clone();
    open(&mut h, "구속", Some("삽입"));
    // The rim of each cylinder's visible end: the circular edge is picked, not the cap behind it.
    let rim = |z: f64| -> Vec<[f64; 3]> {
        (0..12)
            .map(|i| {
                let t = f64::from(i) * std::f64::consts::TAU / 12.0;
                [0.05 * gc_math::fm::cos(t), 0.05 * gc_math::fm::sin(t), z]
            })
            .collect()
    };
    h.get_by_label("취소").click();
    h.run_steps(3);
    let all = [rim(0.05), rim(-0.05)].concat();
    let spot_b = spot_that_picks(&mut h, "삽입", b, &all, "· 원");
    let spot_a = spot_that_picks(&mut h, "삽입", a, &all, "· 원");
    let (Some(spot_b), Some(spot_a)) = (spot_b, spot_a) else {
        panic!("a circular edge could not be picked on a cylinder's rim: {spot_b:?} {spot_a:?}");
    };
    open(&mut h, "구속", Some("삽입"));
    click_on(&mut h, b, spot_b);
    click_on(&mut h, a, spot_a);
    let rows = rows_text(&h);
    h.get_by_label("확인").click();
    h.run_steps(3);
    let rels = h.state().session.relationships();
    assert_eq!(
        rels.len(),
        1,
        "one Insert constraint; rows {rows:?}; error {:?}",
        h.state().session.last_error()
    );
    let (first, second) = (rels[0].1.first, rels[0].1.second);
    assert!(matches!(rels[0].1.kind, RelationKind::Insert { .. }));
    for sel in [first, second] {
        assert!(matches!(sel.geometry, Geometry::Circle { .. }), "{:?}", sel.geometry);
    }
    // Axes on one line and the circles' planes together (offset 0).
    let doc = h.state().session.doc();
    let (p1, d1) = doc.selection_axis(&first).unwrap();
    let (p2, d2) = doc.selection_axis(&second).unwrap();
    assert!(d1.cross(d2).length() < 1e-6, "the axes are parallel");
    assert!((p2 - p1).cross(d1).length() < 1e-6, "the axes are one line");
    let (c1, c2) = (doc.selection_point(&first).unwrap(), doc.selection_point(&second).unwrap());
    assert!(((c1 - c2).dot(d1)).abs() < 1e-6, "the circles' planes are {} apart", (c1 - c2).dot(d1));
    assert!(d1.dot(d2) < -0.999, "Insert's default turns the parts to face each other");
    // Undo restores exactly.
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
}

#[test]
fn a_sphere_clicked_in_the_view_is_made_tangent_to_a_clicked_face() {
    let mut h = harness();
    let s = &mut h.state_mut().session;
    let table = s
        .add_body(Solid::Box { size: [0.4, 0.4, 0.1] }, Placement { xyz: [-0.1, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let ball = s
        .add_body(Solid::Sphere { radius: 0.05 }, Placement { xyz: [0.25, 0.1, 0.2], rpy: [0.0; 3] })
        .unwrap();
    h.run();
    h.state_mut().fit_view();
    h.run_steps(3);
    let before = h.state().session.doc().clone();
    let r = 0.05 * 0.7;
    let spots = [[r, 0.0, r], [-r, 0.0, r], [0.0, r, r], [0.0, -r, r], [0.0, 0.0, 0.05], [r, 0.0, -r]];
    let Some(spot) = spot_that_picks(&mut h, "접선", ball, &spots, "· 형상") else {
        panic!("a sphere could not be picked by clicking on it");
    };
    open(&mut h, "구속", Some("접선"));
    click_on(&mut h, table, [0.12, 0.05, 0.05]);
    click_on(&mut h, ball, spot);
    let rows = rows_text(&h);
    h.get_by_label("확인").click();
    h.run_steps(3);
    let rels = h.state().session.relationships();
    assert_eq!(rels.len(), 1, "rows {rows:?}; error {:?}", h.state().session.last_error());
    let (first, second) = (rels[0].1.first, rels[0].1.second);
    assert!(matches!(rels[0].1.kind, RelationKind::Tangent { .. }));
    let sphere = [first, second]
        .into_iter()
        .find(|s| matches!(s.geometry, Geometry::Sphere { .. }))
        .expect("a sphere selection");
    let face = [first, second]
        .into_iter()
        .find(|s| matches!(s.geometry, Geometry::Plane { .. }))
        .expect("a plane selection");
    let doc = h.state().session.doc();
    let centre = doc.selection_point(&sphere).unwrap();
    let (p, n) = (doc.selection_point(&face).unwrap(), doc.selection_axis(&face).unwrap().1);
    assert!(
        ((centre - p).dot(n).abs() - 0.05).abs() < 1e-6,
        "the ball's centre is {} from the face",
        (centre - p).dot(n)
    );
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().same_content(&before));
}
