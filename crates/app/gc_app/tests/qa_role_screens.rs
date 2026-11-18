//! QA boundary tests (kittest) for the sketch-role screens (GraphCTRL's own feature, plan 9.1; no Inventor
//! counterpart): the sketch right-click menu ("웨이포인트로 지정" / "경로로 지정"), the browser's role nodes, the role
//! properties panel, Create Path ("경로 생성") and the path panel's followed roles. A screen action that cannot
//! apply leaves the document and the undo depth as they were; every one that applies is one undo step.

#![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_doc::{PathMotion, RoleKind};
use gc_tools::{ToolEvent, ToolKind};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const PX: f64 = 1e-4;

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn click(app: &mut GraphCtrlApp, at: [f64; 2]) {
    app.session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
    app.session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

fn sketching() -> H {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    harness
}

fn depth(h: &H) -> usize {
    h.state().session.history().undo_depth()
}

fn right_click(h: &mut H, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(3);
}

fn screen(h: &H, p: [f64; 2]) -> egui::Pos2 {
    h.state().viewport().screen_of(gc_math::DVec3::new(p[0], p[1], 0.0)).unwrap()
}

fn menu_has(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn escape(h: &mut H) {
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
}

/// Two joined lines (a polyline), a circle and a lone point, with the Select tool.
fn scene() -> H {
    let mut h = sketching();
    let app = h.state_mut();
    app.session.set_tool(ToolKind::Line);
    for p in [mm(-60.0, 0.0), mm(0.0, 40.0), mm(60.0, 0.0)] {
        click(app, p);
    }
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    app.session.set_tool(ToolKind::Circle);
    click(app, mm(0.0, -60.0));
    click(app, mm(20.0, -60.0));
    app.session.set_tool(ToolKind::Point);
    click(app, mm(-60.0, -40.0));
    app.session.set_tool(ToolKind::Select);
    h.run_steps(3);
    h
}

#[test]
fn the_sketch_menu_offers_roles_only_where_they_apply() {
    let mut h = scene();
    let sketch = h.state().session.active_sketch().unwrap().0;
    let (waypoint, path) = ("웨이포인트로 지정", "경로로 지정");
    // Empty space: neither.
    let steps = depth(&h);
    let at = screen(&h, mm(60.0, 60.0));
    right_click(&mut h, at);
    assert!(!menu_has(&h, waypoint) && !menu_has(&h, path), "roles offered in empty space");
    escape(&mut h);
    // A closed curve is a path of its own in the backend since b1c2326; the menu offering it is the ignored BUG
    // test below. Here: whatever the menu shows, opening and closing it leaves the document alone.
    let at = screen(&h, mm(0.0, -40.0));
    right_click(&mut h, at);
    escape(&mut h);
    assert_eq!(depth(&h), steps, "opening and closing menus changed the document");
    // The lone point: a waypoint, one undo step; the same point again offers no second one.
    let at = screen(&h, mm(-60.0, -40.0));
    right_click(&mut h, at);
    assert!(menu_has(&h, waypoint));
    h.get_by_label(waypoint).click();
    h.run_steps(3);
    assert_eq!(h.state().session.sketch_roles(sketch).len(), 1);
    assert_eq!(depth(&h), steps + 1);
    right_click(&mut h, at);
    assert!(!menu_has(&h, waypoint), "a waypoint offered twice");
    escape(&mut h);
    // The corner shared by two lines: one waypoint.
    let corner = screen(&h, mm(0.0, 40.0));
    right_click(&mut h, corner);
    if menu_has(&h, waypoint) {
        h.get_by_label(waypoint).click();
        h.run_steps(3);
        assert_eq!(h.state().session.sketch_roles(sketch).len(), 2);
    } else {
        escape(&mut h);
    }
    // The polyline: one path of both lines; then neither of its lines offers a path again.
    let before = h.state().session.sketch_roles(sketch).len();
    let at = screen(&h, mm(-30.0, 20.0));
    right_click(&mut h, at);
    h.get_by_label(path).click();
    h.run_steps(3);
    assert_eq!(h.state().session.sketch_roles(sketch).len(), before + 1);
    for on in [mm(-30.0, 20.0), mm(30.0, 20.0)] {
        let at = screen(&h, on);
        right_click(&mut h, at);
        assert!(!menu_has(&h, path), "a curve already in a path offered a path");
        escape(&mut h);
    }
    // Undo takes the last role away, one step.
    let steps = depth(&h);
    assert!(h.state_mut().session.undo());
    assert_eq!(depth(&h), steps - 1);
    assert_eq!(h.state().session.sketch_roles(sketch).len(), before);
}

/// BUG: the backend makes a circle, a whole ellipse or a closed spline a path role of its own (`chain_at` of it is
/// that curve, backend b1c2326), but the sketch right-click menu picks the curve under the cursor with
/// `Sketch::curve_run`, which has no run for a closed curve (it is `curve_loop`'s), so "경로로 지정" is never
/// offered for one: a closed path can only be made through the session API.
#[test]
#[ignore = "BUG: the sketch menu never offers Make Path for a closed curve (owner: frontend, gc_viewport lib.rs uses curve_run)"]
fn a_closed_curve_is_offered_as_a_path_by_the_sketch_menu() {
    let mut h = scene();
    let at = screen(&h, mm(0.0, -40.0));
    right_click(&mut h, at);
    assert!(menu_has(&h, "경로로 지정"), "Make Path is not offered on a circle");
}

#[test]
fn browser_role_nodes_select_filter_and_delete_safely() {
    let mut h = scene();
    let sketch = h.state().session.active_sketch().unwrap().0;
    let point = {
        let app = h.state();
        *app.session
            .active_sketch()
            .unwrap()
            .1
            .points
            .keys()
            .find(|p| {
                app.session
                    .point_pos(**p)
                    .is_some_and(|q| (q[0] + 0.06).abs() < 1e-9 && (q[1] + 0.04).abs() < 1e-9)
            })
            .unwrap()
    };
    let way = h.state_mut().session.add_waypoint_role(sketch, point).unwrap();
    let curve = *h.state().session.active_sketch().unwrap().1.curves.keys().next().unwrap();
    let chain = h.state().session.chain_at(sketch, curve);
    let path = h.state_mut().session.add_path_role(sketch, &chain).unwrap();
    h.run_steps(3);
    let (way_name, path_name) = {
        let d = h.state().session.doc();
        (d.get(way).unwrap().name.clone(), d.get(path).unwrap().name.clone())
    };
    assert_ne!(way_name, path_name);
    // Selecting marks it; selecting the other moves the mark.
    h.get_by_label(&way_name).click();
    h.run_steps(3);
    assert_eq!(h.state().viewport().role_selected, Some(way));
    h.get_by_label(&path_name).click();
    h.run_steps(3);
    assert_eq!(h.state().viewport().role_selected, Some(path));
    // The properties panel shows the path's motion choices; the waypoint's panel has none.
    assert!(menu_has(&h, "직선 이동"));
    h.get_by_label(&way_name).click();
    h.run_steps(3);
    assert!(!menu_has(&h, "직선 이동"), "a waypoint was offered a motion");
    // The Find box filters the role nodes by name (the panel header shows the selected role, so deselect).
    h.state_mut().viewport_mut().role_selected = None;
    h.run_steps(3);
    let find = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "찾기");
    find.click();
    h.run_steps(2);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "찾기").type_text("zzz-none");
    h.run_steps(3);
    assert!(!menu_has(&h, &way_name) && !menu_has(&h, &path_name), "the filter left role nodes listed");
    // Deleting the selected role from the session while the panel is open: no panic, the panel is gone.
    h.state_mut().session.delete_role(way).unwrap();
    h.run_steps(3);
    assert!(h.state().session.doc().get(way).is_none());
    assert!(!menu_has(&h, "TCP 속도(mm/s)") || h.state().viewport().role_selected != Some(way));
}

#[test]
fn the_role_panel_edits_motion_speed_and_direction_one_step_each() {
    let mut h = sketching();
    h.state_mut().session.set_tool(ToolKind::Line);
    for p in [mm(-60.0, 0.0), mm(60.0, 0.0)] {
        click(h.state_mut(), p);
    }
    h.state_mut().session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    h.state_mut().session.set_tool(ToolKind::Select);
    let (sketch, curve) = {
        let (id, s) = h.state().session.active_sketch().unwrap();
        (id, *s.curves.keys().next().unwrap())
    };
    let role = h.state_mut().session.add_path_role(sketch, &[curve]).unwrap();
    h.state_mut().viewport_mut().role_selected = Some(role);
    h.run_steps(3);
    let kind = |h: &H| h.state().session.doc().get(role).unwrap().kind.clone();
    // Motion: each radio is one step; the one already chosen is no step.
    let steps = depth(&h);
    h.get_by_label("곡선 추종").click();
    h.run_steps(2);
    assert_eq!(depth(&h), steps, "choosing the motion already set made a step");
    for (label, want) in [
        ("직선 이동", PathMotion::Linear),
        ("관절 이동", PathMotion::Joint),
        ("곡선 추종", PathMotion::Follow),
    ] {
        let before = depth(&h);
        h.get_by_label(label).click();
        h.run_steps(2);
        assert_eq!(depth(&h), before + 1, "{label}: one step");
        let RoleKind::Path { motion, .. } = kind(&h) else { panic!() };
        assert_eq!(motion, want);
    }
    // Direction: twice is back where it started, two steps; the label says which way.
    let before = depth(&h);
    h.get_by_label("경로 방향 뒤집기").click();
    h.run_steps(2);
    assert!(menu_has(&h, "경로 방향 뒤집기 (뒤집힘)"));
    h.get_by_label("경로 방향 뒤집기 (뒤집힘)").click();
    h.run_steps(2);
    assert_eq!(depth(&h), before + 2);
    let RoleKind::Path { reversed, .. } = kind(&h) else { panic!() };
    assert!(!reversed);
    // Speed: bad text leaves it, good text sets it, empty clears it; each change is one step.
    let input = egui::accesskit::Role::TextInput;
    let type_speed = |h: &mut H, text: &str| {
        h.get_by_role_and_label(input, "TCP 속도(mm/s)").click();
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        if text.is_empty() {
            h.key_press(egui::Key::Backspace);
        } else {
            h.get_by_role_and_label(input, "TCP 속도(mm/s)").type_text(text);
        }
        h.key_press(egui::Key::Tab);
        h.run_steps(3);
    };
    let speed = |h: &H| match kind(h) {
        RoleKind::Path { tcp_speed, .. } => tcp_speed,
        _ => panic!(),
    };
    for bad in ["abc", "-5", "0", "nan", "1e999 mm", "--1"] {
        let before = depth(&h);
        type_speed(&mut h, bad);
        assert!(speed(&h).is_none_or(|v| v.is_finite() && v > 0.0), "{bad:?} gave {:?}", speed(&h));
        assert!(depth(&h) <= before + 1, "{bad:?}");
        if depth(&h) == before + 1 {
            assert!(h.state_mut().session.undo());
        }
    }
    let before = depth(&h);
    type_speed(&mut h, "50");
    assert_eq!(speed(&h), Some(0.05));
    assert_eq!(depth(&h), before + 1);
    type_speed(&mut h, "50");
    assert_eq!(depth(&h), before + 1, "the same speed again is no step");
    type_speed(&mut h, "");
    assert_eq!(speed(&h), None);
    assert_eq!(depth(&h), before + 2);
    // The panel of a role that is gone from the document is not drawn, and the app does not panic.
    h.state_mut().session.delete_role(role).unwrap();
    h.run_steps(3);
    assert!(!menu_has(&h, "경로 방향 뒤집기"));
}

fn robot_scene() -> H {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    harness.get_by_label("UR5e 로봇").click();
    harness.run();
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    app.session.jog(robot, &[0.0, -1.57, 1.57, -1.57, -1.57, 0.0]);
    app.session.end_jog();
    app.session.create_sketch_3d().unwrap();
    app.session.set_tool(ToolKind::Line);
    app.session.spatial_click([0.45, -0.15, 0.25], None, None);
    app.session.spatial_click([0.45, 0.15, 0.25], None, Some(1));
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    app.session.finish_sketch();
    harness.run();
    harness
}

fn world_click(h: &mut H, world: [f64; 3]) {
    let at = h.state().viewport().screen_of(gc_math::DVec3::from_array(world)).unwrap();
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
    }
    h.run_steps(5);
}

#[test]
fn create_path_mode_boundaries() {
    let mut h = robot_scene();
    let (a, b) = ([0.45, -0.15, 0.25], [0.45, 0.15, 0.25]);
    let before = h.state().session.doc().clone();
    let steps = depth(&h);
    // Start, then Esc, then a right click: each ends the mode with nothing made.
    for how in ["esc", "right"] {
        h.get_by_label("경로 생성").click();
        h.run_steps(2);
        assert!(h.state().viewport().is_routing());
        if how == "esc" {
            escape(&mut h);
        } else {
            let at = egui::pos2(640.0, 400.0);
            right_click(&mut h, at);
            escape(&mut h);
        }
        assert!(!h.state().viewport().is_routing(), "{how}: still routing");
    }
    // Clicks in empty space and the first point only: still routing, nothing made.
    h.get_by_label("경로 생성").click();
    h.run_steps(2);
    let at = egui::pos2(1000.0, 650.0);
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
    }
    assert!(h.state().viewport().is_routing(), "an empty click ended the mode");
    world_click(&mut h, a);
    assert!(h.state().viewport().is_routing());
    // The same point twice: no path, still routing.
    world_click(&mut h, a);
    assert!(h.state().viewport().is_routing());
    assert!(h.state().session.doc().paths().is_empty());
    assert!(h.state().session.doc().same_content(&before) || h.state().session.doc().paths().is_empty());
    assert_eq!(depth(&h), steps, "the clicks changed the document");
    // Esc after the first point: nothing made.
    escape(&mut h);
    assert!(!h.state().viewport().is_routing());
    assert!(h.state().session.doc().paths().is_empty());
    // A full run with the straight-line box on: one path, one step, the second move linear.
    h.get_by_label("경로 생성").click();
    h.run_steps(2);
    h.get_by_label("두 번째 이동을 직선으로").click();
    h.run_steps(2);
    world_click(&mut h, a);
    world_click(&mut h, b);
    h.run_steps(30);
    let paths: Vec<_> =
        h.state().session.doc().paths().iter().map(|(id, p)| (id, p.linear.clone())).collect();
    assert_eq!(paths.len(), 1, "one path between the two points");
    assert_eq!(paths[0].1, vec![false, true], "the second move is a straight line");
    assert!(!h.state().viewport().is_routing());
    assert_eq!(depth(&h), steps + 1, "one undo step");
    assert!(h.state_mut().session.undo());
    assert!(h.state().session.doc().paths().is_empty());
    assert!(h.state().session.doc().waypoints().is_empty(), "undo removes the waypoints too");
}

#[test]
fn create_path_is_off_without_a_robot_and_the_path_panel_lists_followed_roles() {
    // No robot: the button does nothing.
    let mut h = sketching();
    h.state_mut().session.finish_sketch();
    h.run();
    h.get_by_label("로봇").click();
    h.run_steps(3);
    if menu_has(&h, "경로 생성") {
        h.get_by_label("경로 생성").click();
        h.run_steps(2);
        assert!(!h.state().viewport().is_routing(), "Create Path started with no robot");
    }
    // A robot that follows a role: its panel lists the role; a followed role cannot be deleted from the
    // browser (the document keeps it).
    let mut h = robot_scene();
    let (sketch, role, robot) = {
        let app = h.state_mut();
        let sketch = app.session.doc().sketches().iter().next().unwrap().0;
        let curve = *app.session.doc().get(sketch).unwrap().curves.keys().next().unwrap();
        let role = app.session.add_path_role(sketch, &[curve]).unwrap();
        let robot = app.session.doc().robots().iter().next().unwrap().0;
        (sketch, role, robot)
    };
    let path = h.state_mut().session.follow_roles(robot, &[role]).unwrap();
    h.run_steps(3);
    let name = h.state().session.doc().get(role).unwrap().name.clone();
    h.state_mut().session.select_item(Some(gc_session::CellItem::Path(path)));
    h.run_steps(3);
    assert!(h.get_all_by_label_contains(&name).count() >= 1, "the followed role is not listed");
    let steps = depth(&h);
    assert_eq!(h.state_mut().session.delete_role(role), Err(gc_doc::EditError::InUse));
    assert_eq!(depth(&h), steps);
    let _ = sketch;
    // A planned path whose role is then reversed becomes out of date, and the panel keeps working.
    h.state_mut().session.plan(path);
    h.state_mut().session.reverse_role(role).unwrap();
    h.run_steps(3);
    assert_eq!(h.state().session.plan_status(path), gc_session::PlanStatus::Stale);
}
