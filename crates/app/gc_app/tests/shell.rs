//! The application shell through egui_kittest: it must work without a GPU, and with one it is rendered to
//! PNG files that serve as evidence of what the user sees.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    reason = "test code: exact values are compared and failures should panic"
)]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_tools::{ConstraintKind, ToolEvent, ToolKind};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
/// One pixel in sketch units for the scripted input below: 0.1 mm.
const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn click(app: &mut GraphCtrlApp, at: [f64; 2]) {
    app.session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
    app.session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

/// Draws a small part: a rectangle on the origin with width and height, and a circle tangent to its side.
fn draw_sample(app: &mut GraphCtrlApp) {
    app.session.set_tool(ToolKind::Rectangle);
    click(app, mm(0.0, 0.0));
    click(app, mm(60.0, 35.0));
    app.session.set_tool(ToolKind::Dimension);
    click(app, mm(30.0, 0.0));
    click(app, mm(30.0, -10.0));
    app.session.submit_value("60 mm");
    click(app, mm(60.0, 17.0));
    click(app, mm(72.0, 17.0));
    app.session.submit_value("35 mm");
    app.session.set_tool(ToolKind::Circle);
    click(app, mm(95.0, 20.0));
    click(app, mm(107.0, 20.0));
    app.session.set_tool(ToolKind::Constraint(ConstraintKind::Tangent));
    click(app, mm(60.0, 30.0));
    click(app, mm(95.0, 32.0));
    app.session.set_tool(ToolKind::Line);
    click(app, mm(0.0, 35.0));
    click(app, mm(30.0, 55.0));
    app.session.tool_event(ToolEvent::Move { pos: mm(60.2, 35.3), px: PX, suppress_snap: false });
}

/// The app must work without any GPU: ribbon, panels, tools and commands do not depend on the renderer.
#[test]
fn shell_runs_without_gpu() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일");
    harness.get_by_label("준비");
    harness.get_by_label("GPU 없음");
    assert!(harness.state().session.active_sketch().is_none());

    // Starting a sketch from the ribbon switches to the sketch tab and its tools.
    harness.get_by_label("XY 스케치").click();
    harness.run();
    assert!(harness.state().session.active_sketch().is_some());
    harness.get_by_label("Sketch1");
    harness.get_by_label("선").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Line);
    harness.get_by_label("선의 시작점을 클릭");

    draw_sample(harness.state_mut());
    harness.run();
    assert_eq!(harness.state().session.active_sketch().unwrap().1.curves.len(), 6);
    harness.get_by_label("스케치 마무리").click();
    harness.run();
    assert!(harness.state().session.active_sketch().is_none());
    // The tab that was open before the sketch (the model tab) is back.
    harness.get_by_label("XY 스케치");
}

/// With a GPU the whole window is rendered; the PNG is the UI checkpoint shown to the user.
#[test]
fn shell_renders_with_gpu() {
    if gc_test_support::gpu_or_skip("shell_renders_with_gpu").is_none() {
        return;
    }
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("GPU 사용 중");
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    harness.run();
    let image = harness.render().unwrap();
    assert_eq!(image.dimensions(), (1280, 800));
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    image.save(out.join("stage4-sketch-ui.png")).unwrap();
    // The viewport background must be the light clear colour, not black.
    let px = image.get_pixel(1000, 700);
    assert!(px[0] > 150 && px[1] > 150 && px[2] > 150, "viewport pixel was {px:?}");
}

/// Renders the main states of the window to PNG files for review.
#[test]
fn screenshots_for_review() {
    if gc_test_support::gpu_or_skip("screenshots_for_review").is_none() {
        return;
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    let shot = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, name: &str| {
        harness.run();
        harness.render().unwrap().save(out.join(name)).unwrap();
    };
    shot(&mut harness, "review-1-start.png");

    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    // A dimension being placed: the value box is open on the circle's diameter.
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Dimension);
    click(app, mm(107.0, 20.0));
    click(app, mm(112.0, 42.0));
    shot(&mut harness, "review-2-value-box.png");

    // The same width a second time: the reference-dimension question.
    let app = harness.state_mut();
    app.session.submit_value("24 mm");
    click(app, mm(30.0, 0.0));
    click(app, mm(30.0, -20.0));
    app.session.submit_value("60 mm");
    shot(&mut harness, "review-3-reference-question.png");

    // The finished sketch seen from the home view.
    let app = harness.state_mut();
    app.session.answer_reference_offer(true);
    app.session.set_tool(ToolKind::Select);
    harness.run();
    harness.get_by_label("스케치 마무리").click();
    harness.run();
    harness.get_by_label("뷰").click();
    harness.run();
    harness.get_by_label("홈").click();
    shot(&mut harness, "review-4-home-view.png");
}

const LEFT: [f64; 6] = [-1.0, 0.2, 0.3, 0.0, 0.5, 0.0];
const RIGHT: [f64; 6] = [1.0, 0.2, 0.3, 0.0, 0.5, 0.0];

/// Clicks a node with the mouse (a click through the accessibility tree does not reach widgets that are
/// drawn by the view itself, such as the marking menu's commands).
/// Chooses a variant of a split button of the ribbon: the arrow ("<group> 더 보기") opens the list, the
/// variant's name picks it. (The arrow is drawn with `ui.interact`, so it takes pointer events.)
fn pick_variant(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, group: &str, variant: &str) {
    let at = harness.get_by_label(&format!("{group} 더 보기")).rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    harness.get_by_label(variant).click();
    harness.run_steps(3);
}

fn mouse_click_label(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, label: &str) {
    let at = harness.get_by_label(label).rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
}

/// Builds a small cell through the ribbon: a robot, a pillar in its way, a waypoint on the other side
/// and a planned path around the pillar.
fn build_cell(
    harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>,
    robot: &str,
    from: [f64; 6],
    to: [f64; 6],
) {
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    harness.get_by_label(robot).click();
    harness.run();
    let robot = harness.state().session.doc().robots().iter().next().unwrap().0;
    let app = harness.state_mut();
    app.session.jog(robot, &to);
    app.session.end_jog();
    harness.run();
    harness.get_by_label("현재 자세").click();
    harness.run();
    let app = harness.state_mut();
    app.session.jog(robot, &from);
    app.session.end_jog();
    harness.get_by_label("상자").click();
    harness.run();
    harness.get_by_label("경로 만들기").click();
    harness.run();
    harness.get_by_label("계획").click();
    harness.run();
}

/// The robot workflow works without a GPU: add, jog, waypoint, obstacle, path, plan, play.
#[test]
fn a_robot_path_is_planned_through_the_ribbon() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    build_cell(&mut harness, "6R 로봇", LEFT, RIGHT);
    let session = &harness.state().session;
    let path = session.doc().paths().iter().next().unwrap().0;
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(!session.planned(path).unwrap().segments[0].straight, "the box is in the way");
    harness.get_by_label("Path1 (계획됨)");
    harness.get_by_label("Path1 (계획됨)");
    assert!(harness.query_all_by_label("joint_1").count() >= 1, "the jog sliders are shown");
    // The result card states what the checks guarantee, and the ACM list can be edited.
    assert!(
        harness.query_by_label_contains("검사 간격").is_some()
            || harness.query_by_label_contains("검사한 상태에서만").is_some()
    );
    harness.get_by_label("제외: 검사하지 않는 쌍(ACM), 닫힌 메시 안에서 시작한 바디, 컨트롤러 추종 오차");
    harness.get_by_label("충돌 검사 쌍(ACM)").click();
    harness.run();
    let pair = harness
        .state()
        .session
        .link_pairs(harness.state().session.doc().robots().iter().next().unwrap().0)
        .unwrap();
    let checked = pair.iter().find(|p| p.checked).unwrap();
    harness.get_by_label(&format!("{} - {}", checked.a, checked.b)).click();
    harness.run();
    harness.get_by_label("Path1 (오래됨(다시 계획 필요))");
    harness.get_by_label(&format!("{} - {}", checked.a, checked.b)).click();
    harness.run();
    harness.get_by_label("Path1 (계획됨)");
    harness.get_by_label("접촉 허용 추가").scroll_to_me();
    harness.run();
    harness.get_by_label("접촉 허용 추가").click();
    harness.run();
    let path_id = harness.state().session.doc().paths().iter().next().unwrap().0;
    assert_eq!(
        harness.state().session.doc().get(path_id).unwrap().contacts.len(),
        1,
        "{:?}",
        harness.state().session.last_error()
    );
    harness.get_by_label("Path1 (오래됨(다시 계획 필요))");
    harness.get_by_label(&format!("{} 삭제", "1번째 이동: base_link - Body1")).click();
    harness.run();
    assert!(harness.state().session.doc().get(path_id).unwrap().contacts.is_empty());
    harness.get_by_label("계획").click();
    harness.run();
    harness.get_by_label("Path1 (계획됨)");

    // Jogged into the box, the panel says what collides; back out, the message is gone.
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    app.session.jog(robot, &[0.0, 0.2, 0.3, 0.0, 0.5, 0.0]);
    harness.run();
    assert_eq!(harness.query_all_by_label_contains("충돌:").count(), 1);
    harness.state_mut().session.jog(robot, &LEFT);
    harness.run();
    assert_eq!(harness.query_all_by_label_contains("충돌:").count(), 0);
    harness.state_mut().session.end_jog();

    // The comparison runs every planner and lists them.
    harness.get_by_label("알고리즘 비교").scroll_to_me();
    harness.run();
    harness.get_by_label("알고리즘 비교").click();
    harness.run();
    let (rows, current) = harness.state().session.comparison(path).unwrap();
    assert!(current && rows.len() == 5 && rows[0].successes == rows[0].runs);
    harness.get_by_label("Informed RRT*");

    // The motion to a waypoint is chosen with its checkbox; that is an edit, so the plan goes stale.
    harness.get_by_label("P1까지 직선(LIN)").click();
    harness.run();
    assert_eq!(harness.state().session.doc().get(path).unwrap().linear, vec![true]);
    harness.get_by_label("Path1 (오래됨(다시 계획 필요))");
    harness.get_by_label("P1까지 직선(LIN)").click();
    harness.run();
    harness.get_by_label("Path1 (계획됨)");

    // Waypoints are added to the path from a list, and taken out again with their buttons.
    harness.get_by_label("웨이포인트 추가").click();
    harness.run();
    harness.query_all_by_label("P1").last().unwrap().click();
    harness.run();
    let p1 = harness.state().session.doc().get(path).unwrap().waypoints[0];
    assert_eq!(harness.state().session.doc().get(path).unwrap().waypoints, vec![p1, p1]);
    assert_eq!(harness.query_all_by_label("↑").count(), 2, "one per waypoint");
    harness.query_all_by_label("빼기").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().session.doc().get(path).unwrap().waypoints, vec![p1]);
    harness.get_by_label("Path1 (계획됨)");

    // The tool speed on straight moves is limited with a checkbox; 250 mm/s is offered first.
    harness.get_by_label("직선 이동 TCP 속도 제한").click();
    harness.run();
    assert_eq!(harness.state().session.doc().get(path).unwrap().tcp_speed, Some(0.25));
    harness.get_by_label("직선 이동 TCP 속도 제한").click();
    harness.run();
    assert_eq!(harness.state().session.doc().get(path).unwrap().tcp_speed, None);
    assert!(harness.query_by_label_contains("최대 TCP 속도").is_some());

    // Playing advances the shown robot along the motion without touching the document.
    let revision = harness.state().session.doc().revision();
    harness.get_by_label("재생").click();
    harness.run_steps(8);
    let (_, time) = harness.state().session.playback().unwrap();
    assert!(time > 0.0);
    harness.get_by_label("정지").click();
    harness.run_steps(2);
    assert_eq!(harness.state().session.doc().revision(), revision);

    // The waypoint is used by the path and stays; the status bar says why.
    harness.get_by_label("P1 삭제").click();
    harness.run();
    assert_eq!(harness.state().session.doc().waypoints().len(), 1);
    harness.get_by_label("아직 사용 중이라 삭제할 수 없습니다");

    // Moving the box makes the plan stale: a live preview first, one undo step when it ends.
    let app = harness.state_mut();
    app.session.set_playback(None);
    let (id, body) = app.session.doc().bodies().iter().next().map(|(id, b)| (id, b.clone())).unwrap();
    let depth = app.session.history().undo_depth();
    for step in 1..=5 {
        let mut moved = body.clone();
        moved.placement.xyz[1] = 0.1 * f64::from(step);
        app.session.preview_body(id, moved);
        assert_eq!(app.session.doc().revision(), revision);
    }
    assert_eq!(app.session.shown_body(id).unwrap().placement.xyz[1], 0.5);
    app.session.end_body_preview();
    assert_eq!(app.session.history().undo_depth(), depth + 1);
    harness.run();
    harness.get_by_label("Path1 (오래됨(다시 계획 필요))");

    // Path, then waypoint, then robot can be deleted from the browser; each is one undo step.
    for (label, left) in [("Path1 삭제", (1, 1, 0)), ("P1 삭제", (1, 0, 0)), ("6R_1 삭제", (0, 0, 0))] {
        harness.get_by_label(label).click();
        harness.run();
        let doc = harness.state().session.doc();
        assert_eq!((doc.robots().len(), doc.waypoints().len(), doc.paths().len()), left, "{label}");
    }
    assert!(harness.state_mut().session.undo());
    assert_eq!(harness.state().session.doc().robots().len(), 1);
}

/// The robot cell rendered: the 3D solids come from the GPU, the trace and waypoints are drawn over them.
#[test]
fn robot_cell_renders_with_gpu() {
    if gc_test_support::gpu_or_skip("robot_cell_renders_with_gpu").is_none() {
        return;
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    build_cell(&mut harness, "6R 로봇", LEFT, RIGHT);
    let image = harness.render().unwrap();
    image.save(out.join("review-5-robot-path.png")).unwrap();
    // Somewhere in the viewport the dark and the steel-grey links of the robot are visible (the steel is
    // bluer and darker than the background).
    let viewport = (220..1000).flat_map(|x| (130..760).map(move |y| (x, y)));
    let (mut dark, mut steel) = (0, 0);
    for (x, y) in viewport {
        let px = image.get_pixel(x, y);
        dark += u32::from(px[0] < 110 && px[1] < 110 && px[2] < 120 && px[0].abs_diff(px[2]) < 40);
        steel += u32::from(px[0] > 110 && px[0] < 175 && px[2] > px[0].saturating_add(15));
    }
    assert!(dark > 500 && steel > 500, "dark {dark}, steel {steel}");

    // Half-way through the motion, with the planner comparison in the side panel.
    harness.get_by_label("알고리즘 비교").click();
    harness.run();
    let app = harness.state_mut();
    let path = app.session.doc().paths().iter().next().unwrap().0;
    let duration = app.session.planned(path).unwrap().duration();
    app.session.set_playback(Some((path, duration * 0.5)));
    harness.run();
    harness.render().unwrap().save(out.join("review-6-playback.png")).unwrap();
}

/// The packaged UR5e: its meshes are drawn and a path around the pillar is planned with them.
#[test]
fn ur5e_cell_renders_with_gpu() {
    if gc_test_support::gpu_or_skip("ur5e_cell_renders_with_gpu").is_none() {
        return;
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    // Elbow up on one side of the pillar, then on the other.
    let from = [-0.9, -1.3, 1.5, -1.8, -1.57, 0.0];
    let to = [0.9, -1.3, 1.5, -1.8, -1.57, 0.0];
    build_cell(&mut harness, "UR5e 로봇", from, to);
    let session = &harness.state().session;
    let (robot, _) = session.doc().robots().iter().next().unwrap();
    let loaded = session.robot_model(robot).unwrap();
    assert!(loaded.missing.is_empty(), "{:?}", loaded.missing);
    let path = session.doc().paths().iter().next().unwrap().0;
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(!session.planned(path).unwrap().segments[0].straight, "the pillar is in the way");
    // The arm as it really looks: twenty coloured parts of the seven links, the pillar, and one call
    // for all the lines of the trace, one for all the edges.
    let stats = harness.state().viewport().background().stats();
    assert_eq!(stats.draw_calls, 23);
    assert!(stats.triangles > 120_000, "{stats:?}");
    harness.render().unwrap().save(out.join("review-7-ur5e-path.png")).unwrap();
    let app = harness.state_mut();
    let duration = app.session.planned(path).unwrap().duration();
    app.session.set_playback(Some((path, duration * 0.5)));
    harness.run();
    harness.render().unwrap().save(out.join("review-8-ur5e-playback.png")).unwrap();
}

/// A close look at the packaged UR5e as it is drawn: the manufacturer's visual meshes in their own colours.
#[test]
fn ur5e_close_up_renders_with_gpu() {
    if gc_test_support::gpu_or_skip("ur5e_close_up_renders_with_gpu").is_none() {
        return;
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .wgpu()
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
    app.session.jog(robot, &[0.6, -1.9, 1.7, -1.4, -1.57, 0.3]);
    app.session.end_jog();
    harness.get_by_label("뷰").click();
    harness.run();
    harness.get_by_label("전체 보기").click();
    harness.run();
    let image = harness.render().unwrap();
    image.save(out.join("review-9-ur5e-close-up.png")).unwrap();
    // The blue caps of the real arm are visible in the viewport.
    let blue = (220..1000)
        .flat_map(|x| (130..760).map(move |y| (x, y)))
        .filter(|(x, y)| {
            let px = image.get_pixel(*x, *y);
            px[2] > 120 && px[2] > px[0].saturating_add(40) && px[1] > px[0]
        })
        .count();
    assert!(blue > 200, "{blue} blue pixels");

    // A cylinder tool on the flange, chosen in the panel, and the TCP moved to its far end.
    harness.get_by_label("툴").click();
    harness.run();
    harness.get_by_label("툴 형상").click();
    harness.run();
    harness.query_all_by_label("원기둥").last().unwrap().click();
    harness.run();
    harness.get_by_label("TCP를 툴 끝으로").scroll_to_me();
    harness.run();
    harness.get_by_label("TCP를 툴 끝으로").click();
    harness.run();
    let stored = harness.state().session.doc().get(robot).unwrap();
    assert!(matches!(stored.tool.as_ref().unwrap().solid, gc_doc::Solid::Cylinder { .. }));
    assert!((stored.tcp.xyz[2] - 0.15).abs() < 1e-12, "{:?}", stored.tcp);
    harness.render().unwrap().save(out.join("review-18-tool.png")).unwrap();
}

/// The idea of the product in one test: draw a polyline in a 3D sketch, make it a path, and the UR5e's
/// tool follows the lines; change a dimension and the path follows the sketch.
#[test]
fn the_ur5e_follows_a_sketched_polyline() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    harness.get_by_label("UR5e 로봇").click();
    harness.run();
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    // Elbow up, tool pointing down.
    app.session.jog(robot, &[0.0, -1.57, 1.57, -1.57, -1.57, 0.0]);
    app.session.end_jog();

    // An L in the air, 250 mm above the floor: 300 mm along Y, then 200 mm along X.
    let sketch = app.session.create_sketch_3d().unwrap();
    app.session.set_tool(ToolKind::Line);
    app.session.spatial_click([0.45, -0.15, 0.25], None, None);
    app.session.spatial_click([0.45, 0.15, 0.25], None, Some(1));
    app.session.spatial_click([0.65, 0.15, 0.25], None, Some(0));
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    let first = *app.session.active_sketch().unwrap().1.curves.keys().next().unwrap();
    app.session.set_tool(ToolKind::Dimension);
    let pick = |item| ToolEvent::ClickItem { item, pos: [0.0; 2], modifier: false };
    app.session.tool_event(pick(Some(gc_sketch::Item::Curve(first))));
    app.session.tool_event(pick(None));
    app.session.submit_value("300 mm");
    let length = *app.session.active_sketch().unwrap().1.dimensions.keys().next().unwrap();
    app.session.finish_sketch();
    harness.run();

    harness.get_by_label("스케치 경로").click();
    harness.run();
    harness.get_by_label("계획").click();
    harness.run();
    let session = &harness.state().session;
    let path = session.doc().paths().iter().next().unwrap().0;
    assert_eq!(session.doc().get(path).unwrap().linear, vec![false, true, true]);
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    // The tool passes through the three sketch points, and between them stays on the lines' height.
    let tips: Vec<[f64; 3]> = session.planned(path).unwrap().tip_path().collect();
    for corner in [[0.45, -0.15, 0.25], [0.45, 0.15, 0.25], [0.65, 0.15, 0.25]] {
        let nearest = tips
            .iter()
            .map(|t| {
                ((t[0] - corner[0]).powi(2) + (t[1] - corner[1]).powi(2) + (t[2] - corner[2]).powi(2)).sqrt()
            })
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 0.01, "{corner:?}: {nearest}");
    }
    let on_lines = tips.iter().skip_while(|t| (t[2] - 0.25).abs() > 1e-3).all(|t| (t[2] - 0.25).abs() < 1e-3);
    assert!(on_lines, "once on the sketch, the tool stays at its height");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        let app = harness.state_mut();
        let duration = app.session.planned(path).unwrap().duration();
        app.session.set_playback(Some((path, duration * 0.7)));
        harness.get_by_label("뷰").click();
        harness.run();
        harness.get_by_label("전체 보기").click();
        harness.run();
        harness.render().unwrap().save(out.join("review-11-ur5e-follows-sketch.png")).unwrap();
        harness.state_mut().session.set_playback(None);
    }

    // The first line 300 mm -> 200 mm: the sketch changes, the path is out of date, and planning again
    // follows the new corner.
    let app = harness.state_mut();
    app.session.enter_sketch(sketch);
    app.session.edit_dimension(length, "200 mm").unwrap();
    app.session.finish_sketch();
    assert_eq!(app.session.plan_status(path), gc_session::PlanStatus::Stale);
    assert_eq!(app.session.plan(path), gc_session::PlanStatus::Ready, "{:?}", app.session.plan_error(path));
    let tips: Vec<[f64; 3]> = app.session.planned(path).unwrap().tip_path().collect();
    let corner = [0.45, 0.05, 0.25];
    let nearest = tips
        .iter()
        .map(|t| {
            ((t[0] - corner[0]).powi(2) + (t[1] - corner[1]).powi(2) + (t[2] - corner[2]).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min);
    assert!(nearest < 0.01, "the corner moved with the dimension: {nearest}");
}

/// A work frame on a fully constrained sketch point, waypoints taught in it, and the whole path moving
/// when the origin's dimension changes.
#[test]
fn waypoints_follow_a_work_frame() {
    use gc_sketch::DimKind;
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    harness.get_by_label("UR5e 로봇").click();
    harness.run();

    // A point on the floor, 500 mm in front of the robot and 200 mm to the side, selected.
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    let sketch = app.session.create_sketch("XY").unwrap();
    let origin = *app.session.active_sketch().unwrap().1.points.keys().next().unwrap();
    let created = app
        .session
        .execute(&gc_doc::Command::Sketch { sketch, op: gc_doc::SketchOp::AddPoint { pos: [0.5, 0.2, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
    app.session.set_tool(ToolKind::Select);
    click(app, [0.5, 0.2]);
    assert_eq!(app.session.selection().len(), 1);
    harness.run();

    // Still free: the frame is refused and the status bar says why.
    harness.get_by_label("좌표계").click();
    harness.run();
    assert!(harness.state().session.doc().work_frames().is_empty());
    assert_eq!(harness.query_all_by_label_contains("완전히 구속되지 않았습니다").count(), 1);

    let app = harness.state_mut();
    let add = |kind, source: &str| gc_doc::Command::Sketch {
        sketch,
        op: gc_doc::SketchOp::AddDimension { kind, source: source.to_owned(), text_pos: [0.0; 2] },
    };
    let created =
        app.session.execute(&add(DimKind::HDistance { p: origin, q: point, sign: 1.0 }, "500 mm")).unwrap();
    let gc_doc::Created::Dimension(along_x) = created[0] else { panic!("no dimension") };
    app.session.execute(&add(DimKind::VDistance { p: origin, q: point, sign: 1.0 }, "200 mm")).unwrap();
    harness.run();
    harness.get_by_label("좌표계").click();
    harness.run();
    let frame = harness.state().session.doc().work_frames().iter().next().unwrap().0;
    assert!(harness.query_all_by_label("Frame1").count() >= 1, "listed in the browser");

    // Two waypoints taught relative to the frame: tool down, 300 mm and then 250 mm above the floor.
    let app = harness.state_mut();
    app.session.finish_sketch();
    app.session.jog(robot, &[0.4, -1.3, 1.6, -1.87, -1.57, 0.0]);
    app.session.end_jog();
    harness.run();
    harness.get_by_label("좌표계 기준").click();
    harness.run();
    let app = harness.state_mut();
    app.session.jog(robot, &[0.0, -1.2, 1.6, -1.97, -1.57, 0.0]);
    app.session.end_jog();
    harness.run();
    harness.get_by_label("좌표계 기준").click();
    harness.run();
    harness.get_by_label("P1 (기준: Frame1)");
    let session = &harness.state().session;
    let waypoints: Vec<_> = session.doc().waypoints().iter().map(|(id, _)| id).collect();
    let before: Vec<_> = waypoints.iter().map(|w| session.waypoint_pose(*w).unwrap().translation).collect();
    harness.get_by_label("경로 만들기").click();
    harness.run();
    harness.get_by_label("계획").click();
    harness.run();
    let path = harness.state().session.doc().paths().iter().next().unwrap().0;
    let session = &harness.state().session;
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));

    // The origin moves 100 mm along X: every waypoint in the frame moves by the same, and the path is out
    // of date until it is planned again.
    let app = harness.state_mut();
    app.session.enter_sketch(sketch);
    app.session.edit_dimension(along_x, "600 mm").unwrap();
    app.session.finish_sketch();
    for (waypoint, was) in waypoints.iter().zip(&before) {
        let now = app.session.waypoint_pose(*waypoint).unwrap().translation;
        assert!((now - *was - gc_math::DVec3::new(0.1, 0.0, 0.0)).length() < 1e-9);
    }
    assert_eq!(app.session.plan_status(path), gc_session::PlanStatus::Stale);
    harness.run();
    harness.get_by_label("계획").click();
    harness.run();
    let session = &harness.state().session;
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    let end = gc_math::DVec3::from_array(session.planned(path).unwrap().tip_path().last().unwrap());
    let target = session.waypoint_pose(*waypoints.last().unwrap()).unwrap().translation;
    assert!((end - target).length() < 1e-4);
    assert!(session.work_frame_pose(frame).is_some());
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.get_by_label("뷰").click();
        harness.run();
        harness.get_by_label("홈").click();
        harness.run();
        harness.get_by_label("전체 보기").click();
        harness.run();
        harness.render().unwrap().save(out.join("review-12-work-frame.png")).unwrap();
    }
    // Selecting a waypoint shows how the arm would stand there, see-through, and says it is reached.
    let first = waypoints[0];
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Waypoint(first)));
    harness.run();
    let ghost = harness.state().session.shown_ghost().unwrap();
    assert!(ghost.joints.is_some() && ghost.problem.is_none(), "{ghost:?}");
    assert!(harness.query_by_label_contains("로봇이 도달합니다").is_some());
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-17-ghost.png")).unwrap();
    }
}

/// A frame from two points picked in order: origin, then the point the X axis points to. The panel shows
/// its six numbers, in the world and in the robot's base frame.
#[test]
fn a_frame_from_picked_points_shows_its_six_numbers() {
    use gc_sketch::GeoConstraint;
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    harness.get_by_label("UR5e 로봇").click();
    harness.run();
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    // The robot stands 100 mm to the side, so that base and world numbers differ.
    let placement = gc_doc::Placement { xyz: [0.0, -0.1, 0.0], rpy: [0.0; 3] };
    app.session.execute(&gc_doc::Command::Cell(gc_doc::CellOp::PlaceRobot { id: robot, placement })).unwrap();
    let sketch = app.session.create_sketch("XY").unwrap();
    for pos in [[0.4, 0.1, 0.0], [0.6, 0.3, 0.0]] {
        let created = app
            .session
            .execute(&gc_doc::Command::Sketch { sketch, op: gc_doc::SketchOp::AddPoint { pos } })
            .unwrap();
        let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
        let fix = gc_doc::SketchOp::AddConstraint(GeoConstraint::Fix(point));
        app.session.execute(&gc_doc::Command::Sketch { sketch, op: fix }).unwrap();
    }
    app.session.set_tool(ToolKind::Select);
    click(app, [0.4, 0.1]);
    app.session.tool_event(ToolEvent::Move { pos: [0.6, 0.3], px: PX, suppress_snap: false });
    app.session.tool_event(ToolEvent::Click { pos: [0.6, 0.3], px: PX, modifier: true });
    harness.run();
    harness.get_by_label("좌표계").click();
    harness.run();
    let session = &harness.state().session;
    let (_, frame) = session.doc().work_frames().iter().next().unwrap();
    assert!(frame.x_point.is_some() && frame.xy_point.is_none());

    // Open the frame in the panel: X axis at 45 degrees, origin at (400, 100) in the world.
    let header = harness.query_all_by_label("Frame1").last().unwrap();
    header.click();
    harness.run();
    harness.get_by_label("X 400.000  Y 100.000  Z 0.000 mm");
    harness.get_by_label("R 0.000°  P 0.000°  Y 45.000°");
    // From the UR5e's base frame (its `base` link, half a turn about Z from base_link), as the teach
    // pendant shows it.
    harness.get_by_label("X -400.000  Y -200.000  Z 0.000 mm");
    harness.get_by_label("R 0.000°  P 0.000°  Y -135.000°");
    harness.get_by_label("X축 방향 점");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-13-frame-axes.png")).unwrap();
    }
}

/// Equipment can be hidden or made see-through; it stays an obstacle, plans stay current, and the look is
/// saved with the project.
#[test]
fn equipment_can_be_hidden_or_made_see_through() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    build_cell(
        &mut harness,
        "UR5e 로봇",
        [-0.9, -1.3, 1.5, -1.8, -1.57, 0.0],
        [0.9, -1.3, 1.5, -1.8, -1.57, 0.0],
    );
    let session = &harness.state().session;
    let path = session.doc().paths().iter().next().unwrap().0;
    let (body, _) = session.doc().bodies().iter().next().unwrap();
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    let solids = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        gc_scene::Scene::of(&harness.state().session).solids
    };
    let pillar_alpha = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        solids(harness).iter().find(|s| matches!(s.shape, gc_scene::Shape3::Primitive(_))).map(|s| s.color[3])
    };
    assert_eq!(pillar_alpha(&harness), Some(1.0));

    // 60 % see-through: previewed while the slider moves, one undo step when it is let go.
    let app = harness.state_mut();
    let depth = app.session.history().undo_depth();
    let shown = gc_session::Shown::Body(body);
    for percent in [20.0, 40.0, 60.0] {
        app.session.preview_look(shown, gc_doc::Look { hidden: false, transparency: percent / 100.0 });
    }
    app.session.end_look_preview();
    assert_eq!(app.session.history().undo_depth(), depth + 1);
    assert!((pillar_alpha(&harness).unwrap() - 0.4).abs() < 1e-6);
    assert_eq!(
        harness.state().session.plan_status(path),
        gc_session::PlanStatus::Ready,
        "looks do not touch plans"
    );
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.run();
        harness.render().unwrap().save(out.join("review-14-see-through.png")).unwrap();
    }

    // Hidden from the browser: not drawn, still an obstacle.
    harness.get_by_label("Body1 보이기").click();
    harness.run();
    assert_eq!(pillar_alpha(&harness), None);
    let session = &harness.state().session;
    assert!(session.doc().get(body).unwrap().look.hidden);
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready);
    let robot = session.doc().robots().iter().next().unwrap().0;
    let app = harness.state_mut();
    app.session.jog(robot, &[0.0, -1.3, 1.5, -1.8, -1.57, 0.0]);
    assert!(app.session.robot_problem(robot).is_some(), "the hidden pillar is still in the way");
    app.session.jog(robot, &[-0.9, -1.3, 1.5, -1.8, -1.57, 0.0]);
    app.session.end_jog();
    // The robot can be hidden too.
    harness.get_by_label("UR5e_1 보이기").click();
    harness.run();
    assert!(solids(&harness).is_empty());

    // Saved and opened again, the looks are kept.
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("looks.gctrl");
    harness.state_mut().session.save_as(&file).unwrap();
    let mut reopened = gc_session::Session::new();
    reopened.open(&file).unwrap();
    assert_eq!(reopened.doc().get(body).unwrap().look, gc_doc::Look { hidden: true, transparency: 0.6 });
    assert!(reopened.doc().get(robot).unwrap().look.hidden);
}

/// Selecting as in CAD programs: the item under the cursor is highlighted, a click selects it in the view
/// and in the browser, the panel shows only it, a right click offers its commands, Esc and Delete work.
#[test]
fn items_are_highlighted_selected_and_offer_their_commands() {
    use gc_session::CellItem;
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    build_cell(
        &mut harness,
        "UR5e 로봇",
        [-0.9, -1.3, 1.5, -1.8, -1.57, 0.0],
        [0.9, -1.3, 1.5, -1.8, -1.57, 0.0],
    );
    let session = &harness.state().session;
    let (pillar, body) = session.doc().bodies().iter().next().map(|(id, b)| (id, b.clone())).unwrap();
    let robot = session.doc().robots().iter().next().unwrap().0;
    let centre = gc_math::DVec3::from_array(body.placement.xyz) + gc_math::DVec3::new(0.0, 0.0, 0.3);
    let on_pillar = harness.state().viewport().screen_of(centre).unwrap();
    let move_to = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2| {
        harness.event(egui::Event::PointerMoved(at));
        harness.run();
    };
    let press = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2, button| {
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run_steps(3);
        }
    };

    // The pillar under the cursor is highlighted; clicking selects it and the panel shows only it.
    move_to(&mut harness, on_pillar);
    assert_eq!(harness.state().session.hovered_item(), Some(CellItem::Body(pillar)));
    press(&mut harness, on_pillar, egui::PointerButton::Primary);
    assert_eq!(harness.state().session.selected_item(), Some(CellItem::Body(pillar)));
    assert_eq!(harness.query_all_by_label("joint_1").count(), 0, "the robot's sliders are not shown");
    assert!(harness.query_all_by_label("투명도").count() >= 1, "the body's own settings are shown");
    // Clicking the robot in the browser selects it there and in the view.
    harness.get_by_label("UR5e_1:1").click();
    harness.run();
    assert_eq!(harness.state().session.selected_item(), Some(CellItem::Robot(robot)));
    assert!(harness.query_all_by_label("shoulder_pan_joint").count() >= 1, "the robot's sliders are shown");
    if gpu {
        // The robot selected (blue) and the pillar under the cursor (orange).
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        move_to(&mut harness, on_pillar);
        harness.render().unwrap().save(out.join("review-15-selection.png")).unwrap();
    }

    // A right click on the pillar offers to hide it.
    press(&mut harness, on_pillar, egui::PointerButton::Secondary);
    assert_eq!(harness.state().session.selected_item(), Some(CellItem::Body(pillar)));
    mouse_click_label(&mut harness, "숨기기");
    assert!(harness.state().session.doc().get(pillar).unwrap().look.hidden);

    // Esc clears the selection; Delete deletes what is selected, unless it is still in use.
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert_eq!(harness.state().session.selected_item(), None);
    harness.get_by_label("P1").click();
    harness.run();
    harness.key_press(egui::Key::Delete);
    harness.run();
    assert_eq!(harness.state().session.doc().waypoints().len(), 1, "the path still uses it");
    harness.get_by_label("Path1 (계획됨)").click();
    harness.run();
    harness.key_press(egui::Key::Delete);
    harness.run();
    assert!(harness.state().session.doc().paths().is_empty());
    assert_eq!(harness.state().session.selected_item(), None);
}

/// The move and turn handles: dragging the X arrow of a selected body moves it along X in whole
/// millimetres, the ring turns it about the vertical, and each drag is one undo step.
#[test]
fn a_selected_body_is_moved_and_turned_with_its_handles() {
    use gc_session::CellItem;
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("상자").click();
    harness.run();
    let (body, start) = {
        let session = &harness.state().session;
        let (id, b) = session.doc().bodies().iter().next().unwrap();
        (id, b.placement)
    };
    harness.state_mut().session.select_item(Some(CellItem::Body(body)));
    harness.run();
    let origin = gc_math::DVec3::from_array(start.xyz);
    let screen = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, w| {
        harness.state().viewport().screen_of(w).unwrap()
    };
    let drag = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, from: egui::Pos2, to: egui::Pos2| {
        harness.event(egui::Event::PointerMoved(from));
        harness.run();
        let button = |pressed| egui::Event::PointerButton {
            pos: if pressed { from } else { to },
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(button(true));
        harness.step();
        for k in 1..=6 {
            harness.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
            harness.step();
        }
        harness.event(button(false));
        harness.run();
    };
    let depth = harness.state().session.history().undo_depth();

    // Along the X arrow: grab it half-way along, drag to where 100 mm further along X appears.
    let tip = screen(&harness, origin + gc_math::DVec3::X * 0.05);
    let unit = screen(&harness, origin + gc_math::DVec3::X * 0.15)
        - screen(&harness, origin + gc_math::DVec3::X * 0.05);
    let grab = harness.state().viewport().screen_of(origin).unwrap()
        + (tip - harness.state().viewport().screen_of(origin).unwrap()).normalized() * 40.0;
    drag(&mut harness, grab, grab + unit);
    let moved = harness.state().session.doc().get(body).unwrap().placement;
    assert!((moved.xyz[0] - start.xyz[0] - 0.1).abs() < 0.0051, "{:?} -> {:?}", start.xyz, moved.xyz);
    assert_eq!((moved.xyz[1], moved.xyz[2]), (start.xyz[1], start.xyz[2]), "only X changes");
    let mm = moved.xyz[0] * 1000.0;
    assert!((mm - mm.round()).abs() < 1e-6, "whole millimetres: {mm}");
    assert_eq!(harness.state().session.history().undo_depth(), depth + 1);
    assert_eq!(harness.state().session.selected_item(), Some(CellItem::Body(body)), "still selected");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-16-gizmo.png")).unwrap();
    }

    // The ring: from its +X side round to its +Y side is a quarter turn about Z.
    let origin = gc_math::DVec3::from_array(moved.xyz);
    let px = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        // Along the screen's own right the projection is not foreshortened.
        let right = harness.state().viewport().camera.right();
        screen(harness, origin + right * 0.1).distance(screen(harness, origin)) / 0.1
    };
    let radius = 60.0 / px(&harness) as f64;
    let on_ring = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, angle: f64| {
        screen(
            harness,
            origin + gc_math::DVec3::new(gc_math::fm::cos(angle), gc_math::fm::sin(angle), 0.0) * radius,
        )
    };
    let (from, to) = (on_ring(&harness, 0.3), on_ring(&harness, 0.3 + std::f64::consts::FRAC_PI_2));
    drag(&mut harness, from, to);
    let turned = harness.state().session.doc().get(body).unwrap().placement;
    assert!(
        (turned.rpy[2] - start.rpy[2] - std::f64::consts::FRAC_PI_2).abs() < 0.0175 + 1e-9,
        "{:?}",
        turned.rpy
    );
    assert_eq!(turned.xyz, moved.xyz, "turning does not move it");
    assert!(harness.state_mut().session.undo());
    assert_eq!(harness.state().session.doc().get(body).unwrap().placement, moved);

    // The X ring tips it over: from the ring's +Y side to its +Z side is a quarter turn about X.
    let on_x_ring = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, angle: f64| {
        let (c, s) = (gc_math::fm::cos(angle), gc_math::fm::sin(angle));
        screen(harness, origin + gc_math::DVec3::new(0.0, c, s) * radius)
    };
    let (from, to) = (on_x_ring(&harness, 0.4), on_x_ring(&harness, 0.4 + std::f64::consts::FRAC_PI_2));
    drag(&mut harness, from, to);
    let tipped = harness.state().session.doc().get(body).unwrap().placement;
    let rotation = gc_math::quat_from_rpy(tipped.rpy[0], tipped.rpy[1], tipped.rpy[2]);
    let up = rotation * gc_math::DVec3::Z;
    assert!(
        (up - gc_math::DVec3::new(0.0, -1.0, 0.0)).length() < 0.03,
        "the box's Z now points along -Y: {up:?}"
    );
    assert_eq!(tipped.xyz, moved.xyz);
}

/// The handles of a waypoint given in a work frame move it in the world and store the result in the
/// frame; a work frame's own handles only turn it, its origin being a constrained sketch point.
#[test]
fn waypoints_and_frames_have_handles_too() {
    use gc_session::CellItem;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    harness.get_by_label("6R 로봇").click();
    harness.run();
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    let sketch = app.session.create_sketch("XY").unwrap();
    let created = app
        .session
        .execute(&gc_doc::Command::Sketch { sketch, op: gc_doc::SketchOp::AddPoint { pos: [0.4, 0.0, 0.0] } })
        .unwrap();
    let gc_doc::Created::Point(point) = created[0] else { panic!("no point") };
    let fix = gc_doc::SketchOp::AddConstraint(gc_sketch::GeoConstraint::Fix(point));
    app.session.execute(&gc_doc::Command::Sketch { sketch, op: fix }).unwrap();
    let frame = app.session.add_work_frame(sketch, point).unwrap();
    app.session.finish_sketch();
    let offset = gc_doc::Placement { xyz: [0.0, 0.0, 0.3], rpy: [std::f64::consts::PI, 0.0, 0.0] };
    let waypoint =
        app.session.add_waypoint(robot, gc_doc::WaypointTarget::InFrame { frame, offset }).unwrap();
    app.session.select_item(Some(CellItem::Waypoint(waypoint)));
    harness.run();

    let screen = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, w| {
        harness.state().viewport().screen_of(w).unwrap()
    };
    let drag = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, from: egui::Pos2, to: egui::Pos2| {
        harness.event(egui::Event::PointerMoved(from));
        harness.run();
        let button = |pressed: bool, at| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(button(true, from));
        harness.step();
        for k in 1..=6 {
            harness.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
            harness.step();
        }
        harness.event(button(false, to));
        harness.run();
    };
    // Up the Z arrow by 50 mm: the stored offset in the frame goes from 300 to 350 mm.
    let at = harness.state().session.waypoint_pose(waypoint).unwrap().translation;
    let base = screen(&harness, at);
    let up = (screen(&harness, at + gc_math::DVec3::Z * 0.05) - base).normalized();
    let per_metre = screen(&harness, at + gc_math::DVec3::Z * 0.1).distance(base) / 0.1;
    let grab = base + up * 40.0;
    drag(&mut harness, grab, grab + up * per_metre * 0.05);
    let Some(gc_doc::WaypointTarget::InFrame { offset: now, .. }) =
        harness.state().session.doc().get(waypoint).map(|w| w.target.clone())
    else {
        panic!("not in a frame")
    };
    assert!((now.xyz[2] - 0.35).abs() < 0.0051, "{:?}", now.xyz);
    assert!(now.xyz[0].abs() < 1e-9 && now.xyz[1].abs() < 1e-9);

    // The frame: no arrows where they would be, and its ring turns it about its origin.
    let app = harness.state_mut();
    app.session.select_item(Some(CellItem::Frame(frame)));
    harness.run();
    let origin = harness.state().session.work_frame_pose(frame).unwrap().translation;
    let right = harness.state().viewport().camera.right();
    let radius =
        60.0 / f64::from(screen(&harness, origin + right * 0.1).distance(screen(&harness, origin)) / 0.1);
    let on_ring = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, angle: f64| {
        let (c, s) = (gc_math::fm::cos(angle), gc_math::fm::sin(angle));
        screen(harness, origin + gc_math::DVec3::new(c, s, 0.0) * radius)
    };
    let before = harness.state().session.waypoint_pose(waypoint).unwrap().translation;
    let (from, to) = (on_ring(&harness, 0.3), on_ring(&harness, 0.3 + std::f64::consts::FRAC_PI_2));
    drag(&mut harness, from, to);
    let rpy = harness.state().session.doc().get(frame).unwrap().rpy;
    assert!((rpy[2] - std::f64::consts::FRAC_PI_2).abs() < 0.0175 + 1e-9, "{rpy:?}");
    assert!((harness.state().session.work_frame_pose(frame).unwrap().translation - origin).length() < 1e-9);
    // The waypoint in it stays where it is relative to the frame: straight above the origin, so in place.
    let after = harness.state().session.waypoint_pose(waypoint).unwrap().translation;
    assert!((after - before).length() < 1e-6);
}

/// With worker threads, planning does not finish inside the click: the window shows "planning" and picks
/// the result up when the job reports back.
#[test]
fn planning_on_a_worker_thread_reports_back() {
    let options = RunOptions { background_jobs: true, ..RunOptions::default() };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, options));
    harness.run();
    build_cell(&mut harness, "6R 로봇", LEFT, RIGHT);
    let path = harness.state().session.doc().paths().iter().next().unwrap().0;
    // The job runs on its own thread; give it up to ten seconds of real time.
    for _ in 0..1000 {
        if harness.state().session.plan_status(path) != gc_session::PlanStatus::Planning {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.step();
    }
    let session = &harness.state().session;
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(!session.is_planning());
}

/// A dimension is edited where it stands: the box opens with its expression, Enter applies a new one, an
/// expression that cannot be applied keeps the box open.
#[test]
fn a_dimension_is_edited_in_place() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Select);
    let (_, sketch) = app.session.active_sketch().unwrap();
    let width = *sketch.dimensions.iter().find(|(_, d)| d.source == "60 mm").unwrap().0;
    app.edit_dimension(width);
    harness.run();

    let source = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        harness.state().session.active_sketch().unwrap().1.dimensions[&width].source.clone()
    };
    let retype = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, text: &str| {
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness.get_by_label("dimension expression").type_text(text);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
    };
    // Nonsense is refused: the dimension keeps its value and the box stays for another try.
    retype(&mut harness, "60 mm +");
    assert_eq!(source(&harness), "60 mm");
    harness.get_by_label("dimension expression");
    retype(&mut harness, "80 mm");
    assert_eq!(source(&harness), "80 mm");
    assert_eq!(harness.query_all_by_label("dimension expression").count(), 0, "the box closed");
}

/// A named parameter is added in the browser, drives a dimension, and changing it changes the sketch.
#[test]
fn a_parameter_drives_a_dimension() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.run();
    let fill = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, field: &str, text: &str| {
        harness.get_by_label(field).click();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness.get_by_label(field).type_text(text);
        harness.run();
    };
    fill(&mut harness, "parameter name", "w");
    fill(&mut harness, "parameter value", "80 mm");
    harness.get_by_label("추가").click();
    harness.run();
    harness.get_by_label("w = 80 mm");

    // The width dimension now reads the parameter.
    let app = harness.state_mut();
    let (_, sketch) = app.session.active_sketch().unwrap();
    let width = *sketch.dimensions.iter().find(|(_, d)| d.source == "60 mm").unwrap().0;
    app.session.edit_dimension(width, "w").unwrap();
    let measured = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        let (_, sketch) = harness.state().session.active_sketch().unwrap();
        sketch.measure(&sketch.dimensions[&width].kind).unwrap()
    };
    harness.run();
    assert!((measured(&harness) - 0.080).abs() < 1e-9);

    // Clicking the parameter loads it into the editor; a new value is applied to it and to the sketch.
    harness.get_by_label("w = 80 mm").click();
    harness.run();
    fill(&mut harness, "parameter value", "100 mm");
    harness.get_by_label("적용").click();
    harness.run();
    harness.get_by_label("w = 100 mm");
    assert!((measured(&harness) - 0.100).abs() < 1e-9);

    // In use, it cannot be deleted.
    harness.get_by_label("w 삭제").click();
    harness.run();
    assert_eq!(harness.state().session.doc().params().len(), 1);
    harness.get_by_label("아직 사용 중이라 삭제할 수 없습니다");
}

/// A 3D sketch through the window: the line tool places points with the mouse, Tab switches the working
/// plane, and with a GPU the result is rendered for review.
#[test]
fn a_3d_sketch_is_drawn_in_the_viewport() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    pick_variant(&mut harness, "스케치", "3D 스케치");
    harness.run();
    assert!(harness.state().session.sketch_is_spatial());
    harness.get_by_label("작업 평면 XY (Tab으로 전환)");
    harness.get_by_label("선").click();
    harness.run();

    // A click in the viewport starts a line on the working plane (z = 0 on the XY plane).
    let at = egui::pos2(520.0, 520.0);
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    let start = harness.state().session.spatial_anchor().unwrap();
    assert!(start[2].abs() < 1e-12, "{start:?}");
    // Moving the mouse shows the rubber band from there.
    harness.event(egui::Event::PointerMoved(egui::pos2(640.0, 470.0)));
    harness.run();
    let (from, to, _) = harness.state().session.spatial_preview().unwrap();
    assert!(from == start && to != start && to[2].abs() < 1e-12);
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.get_by_label("작업 평면 YZ (Tab으로 전환)");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().session.spatial_anchor().is_none());

    // The rest is scripted: 150 mm along X from the origin, 200 mm up, then out into space; a length, a
    // height.
    let app = harness.state_mut();
    let origin = *app.session.active_sketch().unwrap().1.points.keys().next().unwrap();
    app.session.spatial_click([0.0; 3], Some(origin), None);
    app.session.spatial_click([0.15, 0.0, 0.0], None, Some(0));
    app.session.spatial_click([0.15, 0.0, 0.2], None, Some(2));
    app.session.spatial_click([0.3, 0.12, 0.26], None, None);
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    let (_, sketch) = app.session.active_sketch().unwrap();
    let lines: Vec<_> = sketch.curves.keys().copied().collect();
    let top = sketch.line(lines[1]).unwrap().1;
    app.session.set_tool(ToolKind::Dimension);
    let pick = |item| ToolEvent::ClickItem { item, pos: [0.0; 2], modifier: false };
    app.session.tool_event(pick(Some(gc_sketch::Item::Curve(lines[0]))));
    app.session.tool_event(pick(None));
    app.session.submit_value("150 mm");
    app.session.tool_event(pick(Some(gc_sketch::Item::Point(top))));
    app.session.tool_event(pick(None));
    app.session.submit_value("200 mm");
    app.session.set_tool(ToolKind::Line);
    app.session.spatial_click([0.3, 0.12, 0.26], None, None);
    app.session.spatial_hover(Some(([0.3, 0.12, 0.4], Some(2))));
    harness.run();
    harness.get_by_label("3개 치수 필요");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-10-3d-sketch.png")).unwrap();
    }

    // The constraint glyphs of a 3D sketch can be clicked and deleted.
    let app = harness.state_mut();
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    app.session.set_tool(ToolKind::Select);
    let (_, sketch) = app.session.active_sketch().unwrap();
    let before = sketch.constraints.len();
    let (id, q) = sketch
        .constraints
        .iter()
        .find_map(|(id, c)| match c {
            gc_sketch::GeoConstraint::AlongAxis { q, axis: 0, .. } => Some((*id, *q)),
            _ => None,
        })
        .unwrap();
    let world = app.session.active_sketch().map(|_| app.session.point_pos(q).unwrap()).unwrap();
    // The panels float over the view now: move the drawing out from under the properties panel.
    app.viewport_mut().camera.pan([0.0, 0.0], [-300.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let at = harness.state().viewport().screen_of(gc_math::DVec3::from_array(world)).unwrap();
    // The letters sit up and to the right of the point.
    let glyph = at + egui::vec2(8.0 + 14.0, -14.5);
    harness.event(egui::Event::PointerMoved(glyph));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: glyph,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    let selection = harness.state().session.selection().iter().copied().collect::<Vec<_>>();
    assert_eq!(selection, [gc_sketch::Item::Constraint(id)]);
    harness.key_press(egui::Key::Delete);
    harness.run();
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    assert_eq!(sketch.constraints.len(), before - 1);
}

/// The view cube turns the view: its right edge, seen from the front, looks from between front and right.
#[test]
fn clicking_the_view_cube_turns_the_view() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("뷰").click();
    harness.run();
    harness.get_by_label("정면").click();
    harness.run();
    let forward = harness.state().viewport().camera.forward();
    assert!((forward - gc_math::DVec3::Y).length() < 1e-9);

    let (centre, half) = harness.state().viewport().view_cube_placement().unwrap();
    let at = centre + egui::vec2(0.9 * half, 0.0);
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    let forward = harness.state().viewport().camera.forward();
    let expected = gc_math::DVec3::new(-1.0, 1.0, 0.0).normalize();
    assert!((forward - expected).length() < 1e-9, "{forward:?}");
}

/// Recent projects in the File menu, and unsaved work recovered after the program did not close normally.
#[test]
fn recent_projects_and_recovery_after_a_crash() {
    let directory = tempfile::tempdir().unwrap();
    let settings = directory.path().join("settings");
    let project = directory.path().join("cell.gctrl");
    let options = RunOptions { settings_dir: Some(settings.clone()), ..RunOptions::default() };
    let start = |options: RunOptions, dialogs: Scripted| {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(SIZE)
            .with_pixels_per_point(1.0)
            .build_eframe(move |cc| GraphCtrlApp::new(cc, options).with_dialogs(Box::new(dialogs)));
        harness.run();
        harness
    };

    // Saved once: listed under recent projects.
    let mut harness = start(options.clone(), Scripted(project.clone()));
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    harness.run();
    assert!(harness.state_mut().save(false));
    assert_eq!(harness.state().recent().len(), 1);
    let saved = harness.state().session.doc().clone();

    // More work, saved automatically, then the program "crashes" (the harness is dropped).
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Circle);
    click(app, mm(150.0, 20.0));
    click(app, mm(160.0, 20.0));
    assert!(app.session.is_dirty());
    app.autosave();
    let unsaved = app.session.doc().clone();
    drop(harness);

    // Started again: the recovery question; recovering gives the unsaved work, still to be saved.
    let mut harness = start(options.clone(), Scripted(project.clone()));
    harness.get_by_label("복구").click();
    harness.run();
    let session = &harness.state().session;
    assert!(session.doc().same_content(&unsaved));
    assert!(session.is_dirty());
    assert_eq!(session.file_path(), Some(project.as_path()));
    // Saving keeps it and removes the automatic copy.
    assert!(harness.state_mut().save(false));
    harness.run();
    assert!(gc_persist::places::pending_autosave(&settings).is_none());
    drop(harness);

    // Started again: no question; the project opens from the recent list.
    let mut harness = start(options, Scripted(project.clone()));
    assert_eq!(harness.query_all_by_label("복구").count(), 0);
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("최근 프로젝트").hover();
    harness.run();
    harness.get_by_label("cell.gctrl").click();
    harness.run();
    let session = &harness.state().session;
    assert!(session.doc().same_content(&unsaved) && !session.doc().same_content(&saved));
    assert!(!session.is_dirty());
}

/// File dialogs that always answer with one path.
struct Scripted(std::path::PathBuf);

impl gc_app::FileDialogs for Scripted {
    fn open(&mut self) -> Option<std::path::PathBuf> {
        Some(self.0.clone())
    }
    fn save(&mut self, _: &str) -> Option<std::path::PathBuf> {
        Some(self.0.clone())
    }
    fn open_mesh(&mut self) -> Option<std::path::PathBuf> {
        Some(self.0.clone())
    }
}

/// "Import Mesh" asks for a file and adds it as a body; a file that cannot be read is reported.
#[test]
fn a_mesh_file_is_imported_through_the_ribbon() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.stl");
    let mut harness =
        egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0).build_eframe(|cc| {
            GraphCtrlApp::new(cc, RunOptions::default()).with_dialogs(Box::new(Scripted(path.clone())))
        });
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    // The file does not exist yet: the status bar says so and nothing is added.
    harness.get_by_label("메시 불러오기").click();
    harness.run();
    assert!(harness.state().session.doc().bodies().is_empty());
    assert!(harness.query_all_by_label_contains("fixture.stl").count() >= 1);

    let mesh = gc_mesh::primitives::cylinder(0.2, 0.5, 24).unwrap();
    std::fs::write(&path, gc_mesh::stl::to_binary(&mesh)).unwrap();
    harness.get_by_label("메시 불러오기").click();
    harness.run();
    let session = &harness.state().session;
    let (_, body) = session.doc().bodies().iter().next().unwrap();
    assert_eq!(body.name, "fixture");
    assert_eq!(session.body_mesh(&body.solid).unwrap().data.triangle_count(), 96);
    // Listed in the browser and in the placement panel.
    assert_eq!(harness.query_all_by_label("fixture:1").count(), 1);
    assert_eq!(harness.query_all_by_label("fixture").count(), 1);
}

/// Save, start a new document, open the file again: the sketch is back and the window is not "dirty".
#[test]
fn save_new_and_open_through_the_file_menu() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.gctrl");
    let dialogs = Scripted(path.clone());
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()).with_dialogs(Box::new(dialogs)));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    harness.run();
    assert!(harness.state().session.is_dirty());
    let before = harness.state().session.doc().clone();

    assert!(harness.state_mut().save(false));
    harness.run();
    assert!(!harness.state().session.is_dirty());
    assert!(path.exists());

    // With nothing unsaved, "New" does not ask.
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("새로 만들기").click();
    harness.run();
    assert_eq!(harness.state().session.doc().sketches().len(), 0);

    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("열기...").click();
    harness.run();
    assert!(harness.state().session.doc().same_content(&before));
    assert!(!harness.state().session.is_dirty());
    harness.get_by_label("Sketch1");

    // An edit makes the document dirty again, and "New" then asks before discarding it.
    harness.state_mut().session.undo();
    let app = harness.state_mut();
    let first = app.session.doc().sketches().iter().next().map(|(id, _)| id).unwrap();
    app.session.enter_sketch(first);
    app.session.set_tool(ToolKind::Circle);
    click(app, mm(-30.0, -30.0));
    click(app, mm(-20.0, -30.0));
    harness.run();
    assert!(harness.state().session.is_dirty());
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("새로 만들기").click();
    harness.run();
    harness.get_by_label("변경 내용을 저장할까요?");
    harness.get_by_label("취소").click();
    harness.run();
    assert_eq!(harness.state().session.doc().sketches().len(), 1, "cancelling keeps the document");
}

/// File > Open example: a UR5e cell with a pillar, a work frame and a planned path around the pillar.
#[test]
fn the_example_cell_opens_planned() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    let session = &harness.state().session;
    assert_eq!(session.doc().robots().len(), 1);
    assert_eq!(session.doc().work_frames().len(), 1);
    let path = session.doc().paths().iter().next().unwrap().0;
    assert_eq!(session.plan_status(path), gc_session::PlanStatus::Ready, "{:?}", session.plan_error(path));
    assert!(session.planned(path).unwrap().tip_path().count() > 20);
    assert!(!session.is_dirty() || session.history().undo_depth() > 0);
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-19-example.png")).unwrap();
    }
    // Clicking the trace selects the path: drawn thicker, its waypoints numbered in order.
    let session = &harness.state().session;
    let points: Vec<_> = session.planned(path).unwrap().tip_path().collect();
    let middle = gc_math::DVec3::from_array(points[points.len() * 3 / 4]);
    let on_trace = harness.state().viewport().screen_of(middle).unwrap();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: on_trace,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    let session = &harness.state().session;
    assert_eq!(session.selected_item(), Some(gc_session::CellItem::Path(path)));
    let scene = gc_scene::Scene::of(session);
    assert_eq!(scene.traces[0].highlight, gc_scene::Highlight::Selected);
    let mut names: Vec<_> = scene.waypoints.iter().map(|w| w.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["1·P1", "2·P2"]);
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-20-path-selected.png")).unwrap();
    }

    // A waypoint taught as the robot's pose: its joint values are edited in the panel, and it can be
    // taught again from the robot's current pose.
    let robot = harness.state().session.doc().robots().iter().next().unwrap().0;
    let taught = harness.state_mut().session.add_waypoint_here(robot).unwrap();
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Waypoint(taught)));
    harness.run();
    harness.get_by_label("관절값 (현재 자세로 저장된 웨이포인트)");
    let now = [0.3, -1.6, 1.5, -1.4, -1.57, 0.2];
    harness.state_mut().session.jog(robot, &now);
    harness.state_mut().session.end_jog();
    harness.get_by_label("지금 로봇 자세로 다시 저장").scroll_to_me();
    harness.run();
    harness.get_by_label("지금 로봇 자세로 다시 저장").click();
    harness.run();
    let target = harness.state().session.doc().get(taught).unwrap().target.clone();
    assert_eq!(target, gc_doc::WaypointTarget::Joints(now.to_vec()));

    // Help > Licences lists what ships with the application.
    harness.get_by_label("도움말").click();
    harness.run();
    harness.get_by_label("라이선스").click();
    harness.run();
    harness.run_steps(3);
    harness.get_by_label("Jetendard font (SIL Open Font License 1.1)");
    // The UR packages bring their licence from the robot library.
    assert!(harness.query_by_label_contains("Universal_Robots_ROS2_Description").is_some());
}

/// The robot library lists the four UR e-Series models and the sample arm; a UR10e is drawn with its
/// collision meshes.
#[test]
fn the_library_offers_the_ur_e_series() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("로봇").click();
    harness.run();
    harness.get_by_label("로봇 추가").click();
    harness.run();
    for name in ["UR3e 로봇", "UR5e 로봇", "UR10e 로봇", "UR16e 로봇", "6R 로봇"] {
        harness.get_by_label(name);
    }
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-21-library.png")).unwrap();
    }
    harness.get_by_label("UR10e 로봇").click();
    harness.run();
    let app = harness.state_mut();
    let robot = app.session.doc().robots().iter().next().unwrap().0;
    let loaded = app.session.robot_model(robot).unwrap();
    assert!(loaded.missing.is_empty(), "{:?}", loaded.missing);
    app.session.jog(robot, &[0.6, -1.9, 1.7, -1.4, -1.57, 0.3]);
    app.session.end_jog();
    // Reaching 1.3 m: the flange is farther out than a UR5e's would be.
    let tcp = app.session.tcp_pose(robot).unwrap().translation;
    assert!(tcp.truncate().length() > 0.5, "{tcp:?}");
    harness.get_by_label("뷰").click();
    harness.run();
    harness.get_by_label("전체 보기").click();
    harness.run();
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-22-ur10e.png")).unwrap();
    }
}

/// A waypoint's numbers are measured from the frame the user picks (world, the robot's base as on the UR
/// teach pendant, a work frame) without moving it; the pose calculator composes and re-expresses poses.
#[test]
fn waypoint_references_and_the_pose_calculator() {
    use gc_session::{CellItem, Reference};
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    let session = &harness.state().session;
    let p1 = session.doc().waypoints().iter().next().unwrap().0;
    let robot = session.doc().robots().iter().next().unwrap().0;
    let world = session.waypoint_pose(p1).unwrap();
    // The UR5e's base frame is its `base` link: base_link turned half a turn about Z.
    let base = session.robot_base_frame(robot).unwrap();
    assert!((base.rotation * gc_math::DVec3::X + gc_math::DVec3::X).length() < 1e-12);

    harness.state_mut().session.select_item(Some(CellItem::Waypoint(p1)));
    harness.run();
    harness.get_by_label("P1 (기준: Frame1)");
    // The UR5e's configurations come from its closed form: several, all within the joint limits.
    let ghost = harness.state().session.waypoint_ghost(p1).unwrap();
    assert_eq!(ghost.analytic.as_deref(), Some("ur6r"));
    assert!(ghost.solutions >= 2 && ghost.joints.is_some(), "{ghost:?}");
    assert!(harness.query_by_label_contains("IK: 해석적 (ur6r)").is_some());
    harness.get_by_label("기준 좌표계").click();
    harness.run();
    harness.query_all_by_label("로봇 베이스").last().unwrap().click();
    harness.run();
    let session = &harness.state().session;
    assert_eq!(session.waypoint_reference(p1), Some(Reference::Base));
    assert!((session.waypoint_pose(p1).unwrap().translation - world.translation).length() < 1e-9);
    harness.get_by_label("P1 (기준: 로봇 베이스)");
    harness.get_by_label("다른 좌표계에서 본 값").click();
    harness.get_by_label("변환 행렬 (이동: mm)").click();
    harness.run_steps(8);
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-23-reference.png")).unwrap();
    }

    // The calculator takes the waypoint as pose A and shows it in another frame.
    harness.get_by_label("도구").click();
    harness.run();
    harness.get_by_label("포즈 변환 계산기").click();
    harness.run_steps(8);
    harness.get_by_label("선택한 웨이포인트 → A").click();
    harness.run_steps(8);
    harness.get_by_label("A · B");
    harness.get_by_label("A: 월드 기준 → 월드 기준");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-24-pose-calculator.png")).unwrap();
    }
}

/// The language is remembered between runs in the settings file, and Help > About says where it is.
#[test]
fn the_language_is_remembered() {
    let directory = tempfile::tempdir().unwrap();
    let settings = directory.path().join("settings");
    let start = |dir: std::path::PathBuf| {
        let options = RunOptions { settings_dir: Some(dir), ..RunOptions::default() };
        let mut harness = egui_kittest::Harness::builder()
            .with_size(SIZE)
            .with_pixels_per_point(1.0)
            .build_eframe(move |cc| GraphCtrlApp::new(cc, options));
        harness.run();
        harness
    };
    let mut harness = start(settings.clone());
    harness.get_by_label("도구").click();
    harness.run();
    harness.get_by_label("언어").click();
    harness.run();
    harness.get_by_label("English").click();
    harness.run();
    harness.get_by_label("File");
    assert!(settings.join("settings.json").is_file());
    drop(harness);

    // The next run starts in English; About shows the settings file.
    let mut harness = start(settings.clone());
    harness.get_by_label("File");
    harness.get_by_label("Help").click();
    harness.run();
    harness.get_by_label("About").click();
    harness.run();
    assert!(harness.query_by_label_contains("settings.json").is_some());
}

/// The performance overlay is switched on in the Tools menu and shows the frame statistics.
#[test]
fn the_performance_overlay_is_switched_in_the_tools_menu() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    assert!(!harness.state().viewport().perf_overlay);
    harness.get_by_label("도구").click();
    harness.run();
    harness.get_by_label("성능 오버레이").click();
    harness.run_steps(3);
    assert!(harness.state().viewport().perf_overlay);
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-28-perf-overlay.png")).unwrap();
    }
}

/// With agent connections allowed in the settings, an agent's MCP call is carried out between frames:
/// the robot appears in the cell, the status bar and the activity log say what the agent did.
#[test]
fn an_agent_adds_a_robot_over_mcp() {
    use std::io::{Read, Write};
    let directory = tempfile::tempdir().unwrap();
    let settings = directory.path().join("settings");
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    std::fs::create_dir_all(&settings).unwrap();
    std::fs::write(
        settings.join("settings.json"),
        format!(r#"{{"agent_enabled": true, "agent_port": {port}, "agent_token": "tok"}}"#),
    )
    .unwrap();
    let options = RunOptions { settings_dir: Some(settings), ..RunOptions::default() };
    let gpu = gc_test_support::gpu_or_skip("an_agent_adds_a_robot_over_mcp (screenshot)").is_some();
    let mut builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    if gpu {
        builder = builder.wgpu();
    }
    let mut harness = builder.build_eframe(move |cc| GraphCtrlApp::new(cc, options));
    harness.run_steps(2);

    let post = move |session: Option<&str>, body: &str| -> String {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30))).unwrap();
        let mut request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nAuthorization: Bearer tok\r\n\
             Content-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        if let Some(session) = session {
            request += &format!("Mcp-Session-Id: {session}\r\nMcp-Protocol-Version: 2025-06-18\r\n");
        }
        request += "\r\n";
        request += body;
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = stream.read(&mut buf) {
            if n == 0 {
                break;
            }
            response.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&response);
            if text.starts_with("HTTP/1.1 202") || (text.contains("\"result\"") && text.contains("\n\n")) {
                break;
            }
        }
        String::from_utf8_lossy(&response).into_owned()
    };
    let client = std::thread::spawn(move || {
        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
        let reply = post(None, init);
        let session = reply
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase().starts_with("mcp-session-id:").then(|| l[15..].trim().to_owned())
            })
            .expect("a session id");
        post(Some(&session), r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        post(
            Some(&session),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add_robot","arguments":{"name":"UR5e"}}}"#,
        )
    });
    // The app carries out the call on one of its frames.
    for _ in 0..400 {
        if client.is_finished() {
            break;
        }
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let reply = client.join().unwrap();
    assert!(reply.contains("UR5e_1"), "{reply}");
    harness.run_steps(2);
    assert_eq!(harness.state().session.doc().robots().len(), 1);
    harness.get_by_label("AI 에이전트: add_robot UR5e");

    harness.get_by_label("도구").click();
    harness.run();
    harness.get_by_label("AI 에이전트 연결").click();
    harness.run_steps(4);
    harness.get_by_label(&format!("주소: http://127.0.0.1:{port}/mcp"));
    assert!(harness.query_by_label_contains("claude mcp add --transport http graphctrl").is_some());
    assert!(harness.query_by_label("add_robot UR5e").is_some());
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-25-agent-connection.png")).unwrap();
    }
}

/// Help > Log shows what was logged.
#[test]
fn the_log_window_shows_the_log() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    gc_app::install_diagnostics(None);
    log::info!("a line for the log window");
    harness.get_by_label("도움말").click();
    harness.run();
    harness.get_by_label("로그").click();
    harness.run();
    assert!(harness.query_by_label_contains("a line for the log window").is_some());
}

/// The browser and the properties float over the 3D view: the view fills the window, the panels can be
/// folded, the mouse does not reach the view through them, and the view's overlays keep clear of them.
#[test]
fn the_side_panels_float_over_the_view() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    harness.get_by_label("뷰").click();
    harness.run();
    harness.get_by_label("전체 보기").click();
    harness.run();

    // The cube sits left of the properties panel (330 wide plus margins), not under it.
    let (cube, half) = harness.state().viewport().view_cube_placement().unwrap();
    assert!(cube.x + half < 1280.0 - 330.0 - 16.0, "{cube:?}");
    // The view fills the window behind the panels.
    let clear = harness.state().viewport().clear.unwrap();
    assert!(clear.left() > 200.0 && clear.right() < 960.0, "{clear:?}");

    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-29-floating-panels.png")).unwrap();
    }

    // A click on the properties panel's empty part is not a click in the view: nothing gets selected.
    harness.state_mut().session.select_item(None);
    let on_panel = egui::pos2(1280.0 - 8.0 - 160.0, 700.0);
    harness.event(egui::Event::PointerMoved(on_panel));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: on_panel,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert!(harness.state().session.selected_item().is_none());

    // Folding the properties gives its room back to the view.
    harness.get_by_label("속성 접기").click();
    harness.run();
    let clear = harness.state().viewport().clear.unwrap();
    assert!(clear.right() > 1200.0, "{clear:?}");
    harness.get_by_label("속성 펼치기").click();
    harness.run();
    assert!(harness.state().viewport().clear.unwrap().right() < 960.0);

    // The see-through option can be switched off in the View tab.
    harness.get_by_label("패널 반투명").click();
    harness.run();
    harness.get_by_label("패널 반투명").click();
    harness.run();
}

/// The dark colour scheme is switched in the View tab and shows in the whole window.
#[test]
fn the_dark_theme_is_switched_in_the_view_tab() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    harness.get_by_label("뷰").click();
    harness.run();
    assert!(!gc_ui::colors::is_dark());
    harness.get_by_label("어두운 테마").click();
    harness.run_steps(3);
    assert!(gc_ui::colors::is_dark());
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        let image = harness.render().unwrap();
        image.save(out.join("review-30-dark.png")).unwrap();
        // The window is dark: the status bar is not the light surface colour.
        let px = image.get_pixel(640, 790);
        assert!(px[0] < 100 && px[1] < 100, "{px:?}");
    }
    harness.get_by_label("어두운 테마").click();
    harness.run_steps(3);
    assert!(!gc_ui::colors::is_dark());
}

/// The browser: a node is renamed in place (F2 or a second click), shown or hidden with its check box,
/// and a hidden waypoint leaves the 3D view.
#[test]
fn browser_nodes_are_renamed_and_hidden() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    let waypoint = harness.state().session.doc().waypoints().iter().next().unwrap().0;
    assert_eq!(harness.state().session.doc().waypoints().iter().next().unwrap().1.name, "P1");

    // The first click selects, the second one edits the name.
    harness.query_all_by_label("P1").next().unwrap().click();
    harness.run();
    harness.query_all_by_label("P1").next().unwrap().click();
    harness.run();
    harness.get_by_label("name").focus();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_label("name").type_text("Pick");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(harness.state().session.doc().get(waypoint).unwrap().name, "Pick");
    // A name another waypoint has is refused and explained.
    let other = harness.state().session.doc().waypoints().iter().nth(1).unwrap().1.name.clone();
    harness.query_all_by_label("Pick").next().unwrap().click();
    harness.run();
    harness.query_all_by_label("Pick").next().unwrap().click();
    harness.run();
    harness.get_by_label("name").focus();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_label("name").type_text(&other);
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.get_by_label("같은 이름이 이미 있습니다");

    // Hiding a waypoint takes its marker out of the scene.
    let shown = |h: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        gc_scene::Scene::of(&h.state().session).waypoints.iter().any(|m| m.id == waypoint)
    };
    assert!(shown(&harness));
    harness.get_by_label("Pick 보이기").click();
    harness.run();
    assert!(!harness.state().session.doc().is_visible(waypoint.erase()));
    assert!(!shown(&harness));
    harness.get_by_label("Pick 보이기").click();
    harness.run();
    assert!(shown(&harness));
}

/// A point of a 3D sketch is dragged with the mouse: the point follows, the constraints hold, and the
/// drag is one undo step.
#[test]
fn a_3d_sketch_point_is_dragged() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    pick_variant(&mut harness, "스케치", "3D 스케치");
    harness.run();
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Line);
    let origin = *app.session.active_sketch().unwrap().1.points.keys().next().unwrap();
    app.session.spatial_click([0.0; 3], Some(origin), None);
    app.session.spatial_click([0.15, 0.0, 0.0], None, Some(0));
    app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    app.session.set_tool(ToolKind::Select);
    let end = *app.session.active_sketch().unwrap().1.points.keys().last().unwrap();
    let before = app.session.point_pos(end).unwrap();
    // Out from under the floating properties panel.
    app.viewport_mut().camera.pan([0.0, 0.0], [-300.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let from = harness.state().viewport().screen_of(gc_math::DVec3::from_array(before)).unwrap();
    // Along the line (the X axis runs down to the right in the isometric view).
    let to = from + egui::vec2(50.0, 29.0);
    harness.event(egui::Event::PointerMoved(from));
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    for step in 1..=6 {
        harness.event(egui::Event::PointerMoved(from + (to - from) * (step as f32 / 6.0)));
        harness.run_steps(2);
    }
    harness.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let after = harness.state().session.point_pos(end).unwrap();
    let moved = (0..3).map(|k| (after[k] - before[k]).powi(2)).sum::<f64>().sqrt();
    assert!(moved > 0.01, "{before:?} -> {after:?}");
    // The axis constraint of the line holds: the point stays on the X axis through the origin.
    assert!(after[1].abs() < 1e-6 && after[2].abs() < 1e-6, "{after:?}");
}

/// Basic sketch handling: a new sketch starts with the line tool, F8/F9 show and hide the constraint
/// glyphs, a window drag selects what lies wholly inside, a crossing drag what it touches, and the
/// right-click menu ends the sketch.
#[test]
fn sketch_basics_follow_the_cad_conventions() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Line);

    harness.key_press(egui::Key::F9);
    harness.run();
    assert!(!harness.state().viewport().show_constraints);
    harness.key_press(egui::Key::F8);
    harness.run();
    assert!(harness.state().viewport().show_constraints);

    draw_sample(harness.state_mut());
    harness.state_mut().session.set_tool(ToolKind::Select);
    harness.run();
    let drag = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, from: egui::Pos2, to: egui::Pos2| {
        harness.event(egui::Event::PointerMoved(from));
        harness.run();
        harness.event(egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
        for step in 1..=4 {
            harness.event(egui::Event::PointerMoved(from + (to - from) * (step as f32 / 4.0)));
            harness.run_steps(2);
        }
        harness.event(egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    };
    // From the top left to the bottom right of the free area: everything is wholly inside.
    drag(&mut harness, egui::pos2(236.0, 150.0), egui::pos2(930.0, 760.0));
    let all = harness.state().session.selection().len();
    assert!(all >= 6, "{all} items selected by the window");
    // A tiny crossing box (right to left) over empty space selects nothing and clears the selection.
    drag(&mut harness, egui::pos2(932.0, 762.0), egui::pos2(925.0, 755.0));
    assert!(harness.state().session.selection().is_empty());
    // Right to left over everything is a crossing selection: it takes at least as much.
    drag(&mut harness, egui::pos2(930.0, 760.0), egui::pos2(236.0, 150.0));
    assert!(harness.state().session.selection().len() >= all);

    // The right-click menu finishes the sketch.
    harness.event(egui::Event::PointerMoved(egui::pos2(600.0, 700.0)));
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: egui::pos2(600.0, 700.0),
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: egui::pos2(600.0, 700.0),
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(3);
    harness.get_by_label("완료");
    // The sketch's marking menu: Finish Sketch is one of its four commands (the ribbon has one too).
    let finish = harness.query_all_by_label("스케치 마무리").last().unwrap().rect().center();
    harness.event(egui::Event::PointerMoved(finish));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: finish,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    assert!(harness.state().session.active_sketch().is_none());
}

/// Slice Graphics (F7): a body in front of the sketch plane hides it; F7 cuts it away, and finishing the
/// sketch brings it back.
#[test]
fn slice_graphics_reveals_the_sketch_plane() {
    let Some(_) = gc_test_support::gpu() else { return };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    // A big box that covers the origin plane's view from above.
    let solid = gc_doc::Solid::Box { size: [0.6, 0.6, 0.2] };
    let placement = gc_doc::Placement { xyz: [0.0, 0.0, 0.1], rpy: [0.0; 3] };
    harness.state_mut().session.add_body(solid, placement).unwrap();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    harness.state_mut().fit_view();
    harness.run();
    let covered = harness.render().unwrap();
    assert!(!harness.state().viewport().slice);
    harness.key_press(egui::Key::F7);
    harness.run();
    assert!(harness.state().viewport().slice);
    let sliced = harness.render().unwrap();
    // The box that hid the plane is gone from the picture: it differs a lot.
    let centre = |image: &image::RgbaImage| -> Vec<image::Rgba<u8>> {
        (400..700)
            .flat_map(|x| (250..600).map(move |y| (x, y)))
            .map(|(x, y)| *image.get_pixel(x, y))
            .collect()
    };
    let different = centre(&covered).iter().zip(centre(&sliced)).filter(|(a, b)| **a != *b).count();
    assert!(different > 5000, "{different} pixels changed");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    covered.save(out.join("review-31-slice-off.png")).unwrap();
    sliced.save(out.join("review-31-slice-on.png")).unwrap();
    // Finishing the sketch restores it.
    harness.query_all_by_label("스케치 마무리").last().unwrap().click();
    harness.run();
    assert!(!harness.state().viewport().slice);
}

/// Tools > Measure, as Inventor's Measure with component priority (Inventor 2026 help, "To Measure
/// Distance, Length, Angle, Loop, or Area in Models or Drawings":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm):
/// the robot and the pillar give their minimum distance, Center to Center switches the value, clicking
/// a blank area restarts and Done closes the panel.
#[test]
fn measure_gives_the_minimum_distance_between_two_components() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    harness.get_by_label("검사").click();
    harness.run();
    harness.get_by_label("측정").click();
    harness.run();
    harness.get_by_label_contains("측정할 항목을 선택");

    let (robot, pillar) = {
        let doc = harness.state().session.doc();
        (doc.robots().iter().next().unwrap().0, doc.bodies().iter().next().unwrap().0)
    };
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Robot(robot)));
    harness.run();
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Body(pillar)));
    harness.run();
    let expected = harness
        .state()
        .session
        .measure_pair(gc_session::CellItem::Robot(robot), gc_session::CellItem::Body(pillar))
        .unwrap();
    let gap = expected.minimum.unwrap().distance;
    assert!(gap > 0.0);
    harness.get_by_label(&format!("최소 거리: {:.3} mm", gap * 1000.0));
    harness.get_by_label_contains("선택 1: UR5e");
    harness.get_by_label_contains("선택 2:");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-26-measure.png")).unwrap();
    }

    harness.get_by_label("중심 간 거리").click();
    harness.run();
    harness.get_by_label(&format!("중심 간 거리: {:.3} mm", expected.centre_to_centre * 1000.0));
    // Maximum Distance: never below the minimum.
    harness.get_by_label("최대 거리").click();
    harness.run();
    let farthest = expected.maximum.unwrap().distance;
    assert!(farthest > gap);
    harness.get_by_label(&format!("최대 거리: {:.3} mm", farthest * 1000.0));

    // A click on a blank area (nothing selected) restarts.
    harness.state_mut().session.select_item(None);
    harness.run();
    assert!(harness.query_by_label_contains("선택 1:").is_none());
    harness.get_by_label("완료").click();
    harness.run();
    assert!(harness.query_by_label_contains("측정할 항목을 선택").is_none());
}

/// The heads-up display: after the first click of the line tool, typing a length, Tab, an angle and Enter
/// draws that line (Inventor's dynamic input).
#[test]
fn the_heads_up_display_draws_a_line_from_typed_values() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    assert_eq!(harness.state().session.tool(), ToolKind::Line);
    let click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2| {
        harness.event(egui::Event::PointerMoved(at));
        harness.run();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
        }
    };
    click(&mut harness, egui::pos2(520.0, 450.0));
    harness.event(egui::Event::PointerMoved(egui::pos2(640.0, 400.0)));
    harness.run();
    // The boxes are up: the length and the direction.
    harness.get_by_label("HUD 길이");
    harness.get_by_label("HUD 각도");
    for text in ["5", "0"] {
        harness.event(egui::Event::Text(text.into()));
        harness.run();
    }
    harness.key_press(egui::Key::Tab);
    harness.run();
    for text in ["3", "0"] {
        harness.event(egui::Event::Text(text.into()));
        harness.run();
    }
    harness.key_press(egui::Key::Enter);
    harness.run();
    let (_, sketch) = harness.state().session.active_sketch().unwrap();
    let id = *sketch.curves.keys().next().expect("the line was drawn");
    let (a, b) = sketch.line(id).unwrap();
    let (pa, pb) =
        (harness.state().session.point_pos(a).unwrap(), harness.state().session.point_pos(b).unwrap());
    let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
    let length = gc_math::fm::hypot(dx, dy);
    assert!((length - 0.05).abs() < 1e-6, "length {length}");
    let angle = gc_math::fm::atan2(dy, dx).to_degrees();
    assert!((angle - 30.0).abs() < 1e-6, "angle {angle}");
}

/// Measure with the Edges and Faces priority (same Inventor help page as above): clicking a flat face of
/// the pillar and then its top face gives their areas and the 90° between them; a click on a blank area
/// restarts; changing the priority resets the command.
#[test]
fn measure_picks_faces_with_the_edges_and_faces_priority() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    harness.get_by_label("검사").click();
    harness.run();
    harness.get_by_label("측정").click();
    harness.run();
    harness.get_by_label("모서리 및 면 우선").click();
    harness.run();

    let body = harness.state().session.doc().bodies().iter().next().map(|(_, b)| b.clone()).unwrap();
    let at = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, offset: [f64; 3]| {
        let centre = gc_math::DVec3::from_array(body.placement.xyz) + gc_math::DVec3::from_array(offset);
        harness.state().viewport().screen_of(centre).unwrap()
    };
    let click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, pos: egui::Pos2| {
        harness.event(egui::Event::PointerMoved(pos));
        harness.run();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
        }
    };
    // The middle of the +X side face (80 × 900 mm; the box centre seen from the iso view lies on its
    // front edge), then the top face (80 × 80 mm) at its centre.
    let side = at(&harness, [0.04, 0.0, 0.1]);
    click(&mut harness, side);
    harness.get_by_label("선택 1: 면 면적 72000.00 mm²");
    let top = at(&harness, [0.0, 0.0, 0.45]);
    click(&mut harness, top);
    harness.get_by_label("선택 2: 면 면적 6400.00 mm²");
    harness.get_by_label("각도 90.000°");
    // Picking does not select components.
    assert_eq!(harness.state().session.selected_item(), None);
    // Accumulated Properties: add the angle twice and subtract the top face's area from nothing.
    let plus = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, nth: usize| {
        harness.query_all_by_label("+").nth(nth).unwrap().click();
        harness.run();
    };
    plus(&mut harness, 2);
    plus(&mut harness, 2);
    harness.get_by_label_contains("각도 180.000°");
    harness.query_all_by_label("−").nth(1).unwrap().click();
    harness.run();
    harness.get_by_label_contains("면적 -6400.00 mm²");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-27-measure-faces.png")).unwrap();
    }

    // A blank area restarts.
    click(&mut harness, egui::pos2(700.0, 760.0));
    assert!(harness.query_by_label_contains("선택 1:").is_none());

    // Two corners of the top face, then a third with Shift: the angle at the second corner is 90°.
    let corner = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, dx: f64, dy: f64| {
        let c = at(harness, [dx, dy, 0.45]);
        let middle = at(harness, [0.0, 0.0, 0.45]);
        // A few pixels inside the face, so the ray hits it, well within the 8 px of the corner.
        c + (middle - c).normalized() * 3.0
    };
    let (p1, p2, p3) =
        (corner(&harness, -0.04, -0.04), corner(&harness, 0.04, -0.04), corner(&harness, 0.04, 0.04));
    click(&mut harness, p1);
    click(&mut harness, p2);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
    click(&mut harness, p3);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    harness.run();
    harness.get_by_label("각도 90.000°");
    // Back to component priority: clicks select components again.
    harness.get_by_label("컴포넌트 우선").click();
    harness.run();
    click(&mut harness, side);
    assert!(harness.state().session.selected_item().is_some());
}

/// Tools > Analyze Interference (Inventor 2026 help, "To Analyze Interference Among Components":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-80DED0A6-AC65-4C7C-AD85-0144EE6DAB5B.htm):
/// all components in Set #1, OK lists the interference with its volume; Ignore hides it unless
/// "Ignored" is ticked; picking while a set is defined adds and removes components.
#[test]
fn interference_lists_overlaps_and_ignores_them() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    // A block pushed 30 mm into the pillar (pillar 80 × 80 mm at x = 450 mm): overlap 30 × 80 × 200 mm.
    let block = harness
        .state_mut()
        .session
        .add_body(
            gc_doc::Solid::Box { size: [0.2, 0.08, 0.2] },
            gc_doc::Placement { xyz: [0.45 + 0.04 + 0.1 - 0.03, 0.1, 0.5], rpy: [0.0; 3] },
        )
        .unwrap();
    harness.run();
    harness.get_by_label("검사").click();
    harness.run();
    harness.get_by_label("간섭 분석").click();
    harness.run();

    // Picking while Set #1 is defined adds the block; picking it again removes it.
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Body(block)));
    harness.run();
    assert!(harness.query_by_label("  Body2").is_some());
    harness.state_mut().session.select_item(None);
    harness.run();
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Body(block)));
    harness.run();
    assert!(harness.query_by_label("  Body2").is_none());

    harness.get_by_label("모든 컴포넌트를 세트 #1에").click();
    harness.run();
    harness.get_by_label("확인").click();
    harness.run();
    harness.get_by_label("간섭 1개 · 총 체적 480000.0 mm³");
    let row = "Body1 : Body2  체적 480000.0 mm³  중심 (475.0, 100.0, 500.0) mm";
    harness.get_by_label(row);
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-28-interference.png")).unwrap();
    }
    harness.get_by_label(row).click_secondary();
    harness.run();
    harness.get_by_label("무시").click();
    harness.run();
    assert!(harness.query_by_label(row).is_none());
    harness.get_by_label("무시한 항목 표시").click();
    harness.run();
    harness.get_by_label(row);
    harness.get_by_label("다시 분석").click();
    harness.run();
    harness.get_by_label("세트 #2 정의");
}

/// Tools > Drive (Inventor 2026 help, "To Drive Relationships in Assemblies":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B78E59C9-F9FB-4ED5-B60E-C9198770B1AF.htm):
/// End defaults to Start + 10; Go to End shows the joint there; with Collision Detection the drive stops
/// at the first collision; closing the dialog puts the robot back as the document has it.
#[test]
fn drive_moves_a_joint_and_stops_at_a_collision() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    let robot = harness.state().session.doc().robots().iter().next().unwrap().0;
    let stored = harness.state().session.robot_joints(robot).unwrap();
    harness.query_all_by_label("조립").next().unwrap().click();
    harness.run();
    harness.get_by_label("구동").click();
    harness.run();

    harness.get_by_label("끝으로").click();
    harness.run();
    harness.get_by_label("현재 값 10.000°");
    let shown = harness.state().session.robot_joints(robot).unwrap();
    assert!((shown[0] - (stored[0] + 10f64.to_radians())).abs() < 1e-9);
    harness.get_by_label("시작으로").click();
    harness.run();

    // Turn the shoulder toward the pillar with Collision Detection on: the drive stops on the way.
    harness.get_by_label("자세히").click();
    harness.run();
    harness.get_by_label("충돌 감지").click();
    harness.run();
    harness.state_mut().drive_end_for_test(-90.0);
    harness.get_by_label("정방향").click();
    for _ in 0..200 {
        harness.step();
        if harness.query_by_label_contains("충돌 감지:").is_some() {
            break;
        }
    }
    harness.run_steps(2);
    let label = harness.query_by_label_contains("충돌 감지:").map(|n| format!("{n:?}"));
    let at = harness.state().session.robot_joints(robot).unwrap()[0].to_degrees();
    assert!(label.is_some(), "the drive should stop at the pillar; shoulder at {at}°");
    let stopped = harness.state().session.robot_joints(robot).unwrap()[0];
    assert!(stopped < stored[0] && stopped > stored[0] - 90f64.to_radians(), "{stopped}");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        harness.render().unwrap().save(out.join("review-29-drive.png")).unwrap();
    }

    // Closing restores the document's pose; nothing was committed.
    let undo_depth = harness.state().session.history().undo_depth();
    harness.state_mut().close_drive_for_test();
    harness.run();
    assert_eq!(harness.state().session.robot_joints(robot).unwrap(), stored);
    assert_eq!(harness.state().session.history().undo_depth(), undo_depth);
}

/// Measure with the Edges and Faces priority on a cylinder (same Inventor help page): its diameter and
/// radius; against the pillar's face, the gap and the distance to its axis (Center to Center).
#[test]
fn measure_gives_a_cylinders_diameter() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    // An upright shaft of diameter 100 mm in front of the pillar.
    harness
        .state_mut()
        .session
        .add_body(
            gc_doc::Solid::Cylinder { radius: 0.05, length: 0.4 },
            gc_doc::Placement { xyz: [0.45, -0.25, 0.2], rpy: [0.0; 3] },
        )
        .unwrap();
    harness.run();
    harness.get_by_label("검사").click();
    harness.run();
    harness.get_by_label("측정").click();
    harness.run();
    harness.get_by_label("모서리 및 면 우선").click();
    harness.run();
    let on_shaft = harness.state().viewport().screen_of(gc_math::DVec3::new(0.45, -0.25, 0.2)).unwrap();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: on_shaft,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    harness.get_by_label_contains("원통 지름 100.000 mm · 반지름 50.000 mm · 길이 400.000 mm");
}

/// Manage tab > Parameters (Inventor 2026 help, "To Work with Parameters in Models":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-595F9D8E-9EEA-4446-91AB-830E5BAE548E.htm):
/// the sketch dimensions are listed as model parameters; Add Numeric adds a user parameter; a user
/// parameter's equation is changed and a dimension made to use it; search filters the rows; Delete
/// Parameter is refused while a dimension still uses it.
#[test]
fn the_parameters_dialog_lists_and_edits_parameters() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    harness.get_by_label("관리").click();
    harness.run();
    harness.get_by_label("매개변수").click();
    harness.run();
    harness.get_by_label("모델 매개변수");
    let first = "d0 (Sketch1 · 거리 1)";
    assert!(harness.query_all_by_label(first).count() >= 1);
    harness.get_by_label("450");

    let type_into = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, row: &str, text: &str| {
        let field = harness.get_by_role_and_label(egui::accesskit::Role::TextInput, row);
        field.focus();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.get_by_role_and_label(egui::accesskit::Role::TextInput, row).type_text(text);
        harness.key_press(egui::Key::Enter);
        harness.run();
    };
    harness.get_by_label("숫자 추가").click();
    harness.run();
    type_into(&mut harness, "Param", "112.5 mm");
    harness.get_by_label("112.5");
    type_into(&mut harness, first, "Param * 4");
    let sketch = harness.state().session.doc().sketches().iter().next().unwrap().1.clone();
    let dimension = sketch.dimensions.iter().next().unwrap().1.clone();
    assert_eq!(dimension.source, "Param * 4");
    assert!((dimension.value - 0.45).abs() < 1e-12);

    // Search narrows the rows; a parameter in use cannot be deleted (the dialog says why).
    harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "검색").focus();
    harness.run();
    harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "검색").type_text("Param");
    harness.run();
    assert!(harness.query_by_label("d1 (Sketch1 · 거리 2)").is_none());
    harness.get_by_role_and_label(egui::accesskit::Role::Label, "Param").click_secondary();
    harness.run();
    harness.get_by_label("매개변수 삭제").click();
    harness.run();
    assert_eq!(harness.state().session.doc().params().len(), 1);
    // Renaming the parameter renames it in the dimension's equation too.
    harness.get_by_label("Param name").focus();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_label("Param name").type_text("Gap");
    harness.key_press(egui::Key::Enter);
    harness.run();
    let renamed = harness.state().session.doc().sketches().iter().next().unwrap().1.clone();
    assert_eq!(renamed.dimensions.iter().next().unwrap().1.source, "Gap * 4");
    harness.get_by_label("완료").click();
    harness.run();
}

/// A component's origin folder: shown in the browser with a check box each, drawn in the view, picked
/// there (the browser follows), and a sketch is started from its right-click menu.
#[test]
fn a_component_origin_plane_is_shown_picked_and_sketched_on() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let body = harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    let occurrence = gc_doc::Occurrence::Item(body.erase());
    let plane = gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy);
    assert!(!harness.state().session.origin_visible(occurrence, plane), "hidden at first");

    // The folder is listed under the body; its check boxes show the elements.
    harness.query_all_by_label("원점").last().unwrap().click();
    harness.run();
    harness.query_all_by_label("XY 평면 보이기").next().unwrap().click();
    harness.run();
    assert!(harness.state().session.origin_visible(occurrence, plane));

    // A click on the drawn plane selects it, the browser row follows; Escape lets go.
    let on_plane = gc_math::DVec3::new(0.09, 0.09, 0.0);
    // Out from under the floating properties panel.
    harness.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let at = harness.state().viewport().screen_of(on_plane).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert_eq!(harness.state().viewport().origin_selected, Some((occurrence, plane)));
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-32-origin-plane.png")).unwrap();
    }
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().viewport().origin_selected.is_none());

    // The right-click menu of the plane starts a sketch on it.
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run_steps(3);
    mouse_click_label(&mut harness, "스케치 만들기");
    assert!(harness.state().session.active_sketch().is_some());
    assert_eq!(harness.state().session.doc().sketches().len(), 1);
}

/// The navigation bar: a pan tool that stays on, zoom all, and the home button at the view cube; the
/// browser is an assembly tree with the document at its top and the components as "name:1".
#[test]
fn the_navigation_bar_and_the_assembly_tree() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    // The tree: the document, its folders, the components.
    harness.get_by_label("Cell1");
    harness.get_by_label("관계");
    harness.get_by_label("표현");
    harness.get_by_label("UR5e_1:1");
    harness.get_by_label("Body1:1");
    // The status bar counts occurrences.
    harness.get_by_label_contains("구성요소 2");

    let pivot = harness.state().viewport().camera.pivot;
    // The bar's buttons are drawn in the view and answer to the mouse.
    let press = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, label: &str| {
        let at = harness.get_by_label(label).rect().center();
        harness.event(egui::Event::PointerMoved(at));
        harness.run();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
        }
    };
    press(&mut harness, "초점 이동");
    let from = egui::pos2(520.0, 450.0);
    harness.event(egui::Event::PointerMoved(from));
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    for step in 1..=4 {
        harness.event(egui::Event::PointerMoved(from + egui::vec2(15.0 * step as f32, 0.0)));
        harness.run_steps(2);
    }
    harness.event(egui::Event::PointerButton {
        pos: from + egui::vec2(60.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert!((harness.state().viewport().camera.pivot - pivot).length() > 1e-3, "the view was panned");
    if gpu {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        harness.render().unwrap().save(out.join("review-33-nav-bar-tree.png")).unwrap();
    }
    // Esc ends the tool; the home button turns the view to the isometric one.
    harness.key_press(egui::Key::Escape);
    harness.run();
    press(&mut harness, "홈 뷰");
    press(&mut harness, "모두 확대");
}

/// The Constrain dialog: pick the origin planes of two components in the view, press OK, and the first
/// component moves to the second; the constraint is listed in the browser's Relationships folder.
#[test]
fn the_constrain_dialog_mates_two_components() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    let fixed = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let placement = gc_doc::Placement { xyz: [0.12, 0.0, 0.0], rpy: [0.0; 3] };
    let moving = app.session.add_body(solid, placement).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();

    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("구속").click();
    harness.run_steps(5);
    let click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, world: gc_math::DVec3| {
        let at = harness.state().viewport().screen_of(world).unwrap();
        harness.event(egui::Event::PointerMoved(at));
        harness.run_steps(3);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run_steps(3);
        }
    };
    // First the Z axis that moves, then the one that stays.
    click(&mut harness, gc_math::DVec3::new(0.12, 0.0, 0.06));
    harness.get_by_label_contains("첫 번째 선택: Body2:1 · 축");
    click(&mut harness, gc_math::DVec3::new(0.0, 0.0, 0.06));
    harness.get_by_label_contains("두 번째 선택: Body1:1 · 축");
    harness.get_by_label("확인").click();
    harness.run();
    let session = &harness.state().session;
    assert_eq!(session.relationships().len(), 1, "{:?}", session.last_error());
    let sideways = session.doc().get(moving).unwrap().placement.xyz[0];
    assert!(sideways.abs() < 1e-6, "the axes coincide now: {sideways}");
    let _ = fixed;
}

/// Section views (View tab > Appearance): a half section cuts the model by a plane; a quarter section
/// takes a second plane after Continue; Delete brings the model back.
#[test]
fn section_views_cut_the_model() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.4, 0.4, 0.4] };
    let placement = gc_doc::Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] };
    harness.state_mut().session.add_body(solid, placement).unwrap();
    harness.state_mut().fit_view();
    harness.get_by_label("뷰").click();
    harness.run();
    let whole = gpu.then(|| harness.render().unwrap());

    harness.get_by_label("1/2 단면").click();
    harness.run_steps(3);
    let section = harness.state().viewport().section.clone().unwrap();
    assert_eq!(section.planes.len(), 1);
    assert!(section.editing.is_some());
    if let Some(whole) = whole {
        let cut = harness.render().unwrap();
        let different = whole.pixels().zip(cut.pixels()).filter(|(a, b)| a != b).count();
        assert!(different > 5000, "{different} pixels changed");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        cut.save(out.join("review-34-half-section.png")).unwrap();
        // Flipped, the cut face looks at the viewer: the cap is a flat face, not the box's blue.
        harness.get_by_label("뒤집기").click();
        harness.run_steps(3);
        let flipped = harness.render().unwrap();
        flipped.save(out.join("review-41-section-cap.png")).unwrap();
        let at = flipped.get_pixel(520, 420).0;
        assert!(at[0] + 20 >= at[2] && at[0] > 120, "a flat cap, not the box blue: {at:?}");
        harness.get_by_label("뒤집기").click();
        harness.run_steps(3);
    }
    // OK applies it; the toolbar goes away and the cut stays.
    harness.get_by_label("확인").click();
    harness.run_steps(3);
    assert!(harness.state().viewport().section.as_ref().unwrap().editing.is_none());

    // A quarter section goes on to a second plane.
    harness.get_by_label("1/4 단면").click();
    harness.run_steps(3);
    harness.get_by_label("계속").click();
    harness.run_steps(3);
    assert_eq!(harness.state().viewport().section.as_ref().unwrap().planes.len(), 2);
    harness.get_by_label("확인").click();
    harness.run_steps(3);

    harness.get_by_label("단면 삭제").click();
    harness.run_steps(3);
    assert!(harness.state().viewport().section.is_none());
}

/// The marking menu: a quick mark toward a command picks it without showing the menu; a right click
/// without a mark shows the radial menu, whose commands are named.
#[test]
fn the_marking_menu_picks_by_mark_and_by_menu() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let centre = egui::pos2(500.0, 450.0);
    harness.event(egui::Event::PointerMoved(centre));
    harness.run();
    // Mark mode in the assembly environment: north is Constrain.
    harness.event(egui::Event::PointerButton {
        pos: centre,
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerMoved(centre + egui::vec2(0.0, -70.0)));
    harness.event(egui::Event::PointerButton {
        pos: centre + egui::vec2(0.0, -70.0),
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(3);
    harness.get_by_label("첫 번째 선택: 뷰에서 선택");

    // Menu mode: the commands show with their names; the sketch environment has its own four.
    harness.get_by_label("취소").click();
    harness.run_steps(3);
    harness.get_by_label("XY 스케치").click();
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: centre,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run_steps(3);
    for name in ["선", "원", "직사각형"] {
        assert!(harness.query_all_by_label(name).next().is_some(), "{name}");
    }
}

/// Sketch tab > Project Cut Edges (Inventor 2026 help, "To Project Geometry into a 2D Sketch":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm):
/// with the pillar selected, the XY sketch gets the four edges where its plane cuts the pillar, as fixed
/// lines in one undo step.
#[test]
fn project_cut_edges_adds_the_cut_of_the_selected_part() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    // The XY sketch's plane cuts the pillar (80 × 80 mm, standing on the floor at 450, 100 mm) at its foot.
    let (sketch, pillar) = {
        let doc = harness.state().session.doc();
        (doc.sketches().iter().next().unwrap().0, doc.bodies().iter().next().unwrap().0)
    };
    let before = harness.state().session.doc().get(sketch).unwrap().curves.len();
    harness.state_mut().session.enter_sketch(sketch);
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Body(pillar)));
    harness.run();
    // The ribbon's Sketch tab (the browser has a "스케치" folder too; the tab comes first).
    harness.query_all_by_label("스케치").next().unwrap().click();
    harness.run();
    let depth = harness.state().session.history().undo_depth();
    harness.get_by_label("절단 모서리 투영").click();
    harness.run();
    harness.get_by_label("절단 모서리 4개를 투영했습니다.");
    let after = harness.state().session.doc().get(sketch).unwrap().clone();
    assert_eq!(after.curves.len(), before + 4);
    assert_eq!(harness.state().session.history().undo_depth(), depth + 1);
    // The square is 80 mm on a side around (450, 100).
    let xs: Vec<f64> = after.points.values().map(|p| p.pos[0]).filter(|x| (x - 0.45).abs() < 0.05).collect();
    assert!(
        xs.iter().any(|x| (x - 0.41).abs() < 1e-6) && xs.iter().any(|x| (x - 0.49).abs() < 1e-6),
        "{xs:?}"
    );
}

/// The Joint dialog: the centre points (joint origins) of two components, type Automatic gives a rigid
/// joint, and the first component moves onto the second.
#[test]
fn the_joint_dialog_joins_two_components_rigidly() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let placement = gc_doc::Placement { xyz: [0.12, 0.0, 0.0], rpy: [0.0; 3] };
    let moving = app.session.add_body(solid, placement).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();

    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("조인트").click();
    harness.run_steps(5);
    let click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, world: gc_math::DVec3| {
        let at = harness.state().viewport().screen_of(world).unwrap();
        harness.event(egui::Event::PointerMoved(at));
        harness.run_steps(3);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run_steps(3);
        }
    };
    click(&mut harness, gc_math::DVec3::new(0.12, 0.0, 0.0));
    harness.get_by_label_contains("첫 번째 원점: Body2:1");
    click(&mut harness, gc_math::DVec3::new(0.0, 0.0, 0.0));
    harness.get_by_label_contains("두 번째 원점: Body1:1");
    harness.get_by_label("확인").click();
    harness.run();
    let session = &harness.state().session;
    assert_eq!(session.relationships().len(), 1, "{:?}", session.last_error());
    let sideways = session.doc().get(moving).unwrap().placement.xyz[0];
    assert!(sideways.abs() < 1e-6, "the origins coincide now: {sideways}");
}

/// The links of a robot are components below it in the browser, each with an origin folder; a link's
/// origin can be shown like any component's.
#[test]
fn robot_links_are_components_below_the_robot() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("예제 열기 (UR5e 셀)").click();
    harness.run();
    let robot = harness.state().session.doc().robots().iter().next().unwrap().0;
    let links = harness.state().session.robot_links(robot);
    assert!(links.len() >= 6, "{}", links.len());
    let name = format!("{}:1", links[1].name);
    harness.query_all_by_label("링크").next().unwrap().click();
    harness.run();
    harness.get_by_label_contains(&name);
    // A link's origin can be shown through the session like any component's.
    let occurrence = links[1].occurrence;
    harness
        .state_mut()
        .session
        .set_origin_visible(occurrence, &[gc_doc::OriginElement::Point], true)
        .unwrap();
    harness.run();
    assert!(harness.state().session.origin_visible(occurrence, gc_doc::OriginElement::Point));
}

/// Drive on an assembly relationship (Inventor 2026 help, "To Drive Relationships in Assemblies":
/// https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B78E59C9-F9FB-4ED5-B60E-C9198770B1AF.htm):
/// a block mated on top of a base is driven from its offset (0) to 10 mm; Go to End lifts it; with
/// Collision Detection, driving it down into the base stops; closing puts it back.
#[test]
fn drive_moves_a_mated_block_and_stops_at_a_collision() {
    use gc_doc::{AxisSense, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid};
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let session = &mut harness.state_mut().session;
    let base = session
        .add_body(Solid::Box { size: [0.4, 0.4, 0.2] }, Placement { xyz: [0.0, 0.0, 0.1], rpy: [0.0; 3] })
        .unwrap();
    let block = session
        .add_body(Solid::Box { size: [0.1, 0.1, 0.1] }, Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let face = |body: gc_core::Id<gc_doc::Body>, z: f64, nz: f64| Selection {
        occurrence: Occurrence::Item(body.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, nz] },
    };
    let mate = session
        .add_relationship(Relationship::new(
            "Mate:1",
            RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
            face(block, -0.05, -1.0),
            face(base, 0.1, 1.0),
        ))
        .unwrap();
    let rest = harness.state().session.shown_body(block).unwrap().placement;
    assert!((rest.xyz[2] - 0.25).abs() < 1e-6, "mated on top: {rest:?}");

    harness.state_mut().drive_relationship(mate);
    harness.run();
    harness.get_by_label("끝으로").click();
    harness.run();
    harness.get_by_label("현재 값 10.000mm");
    let lifted = harness.state().session.shown_body(block).unwrap().placement;
    assert!((lifted.xyz[2] - 0.26).abs() < 1e-6, "{lifted:?}");

    // Down into the base with Collision Detection: the drive stops on the way.
    harness.get_by_label("시작으로").click();
    harness.run();
    harness.get_by_label("자세히").click();
    harness.run();
    harness.get_by_label("충돌 감지").click();
    harness.run();
    harness.state_mut().drive_end_for_test(-50.0);
    harness.get_by_label("정방향").click();
    for _ in 0..100 {
        harness.step();
        if harness.query_by_label_contains("충돌 감지:").is_some() {
            break;
        }
    }
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("충돌 감지: Body2 ↔ Body1").is_some());

    harness.state_mut().close_drive_for_test();
    harness.run();
    let back = harness.state().session.shown_body(block).unwrap().placement;
    assert!((back.xyz[2] - 0.25).abs() < 1e-6, "{back:?}");
}

/// Free Move and Free Rotate: with the tool on, dragging a component moves it in the plane of the view
/// or turns it about its origin, and each drag is one undo step.
#[test]
fn free_move_and_free_rotate_drag_a_component() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.1, 0.1, 0.1] };
    let body = harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.state_mut().fit_view();
    harness.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let drag = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, from: egui::Pos2, by: egui::Vec2| {
        harness.event(egui::Event::PointerMoved(from));
        harness.run_steps(2);
        harness.event(egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
        for step in 1..=4 {
            harness.event(egui::Event::PointerMoved(from + by * (step as f32 / 4.0)));
            harness.run_steps(2);
        }
        harness.event(egui::Event::PointerButton {
            pos: from + by,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
    };
    let on_body = harness.state().viewport().screen_of(gc_math::DVec3::new(0.0, 0.0, 0.05)).unwrap();
    let depth = harness.state().session.history().undo_depth();

    harness.state_mut().viewport_mut().free_tool = Some(gc_viewport::FreeTool::Move);
    drag(&mut harness, on_body, egui::vec2(60.0, 0.0));
    let placed = harness.state().session.doc().get(body).unwrap().placement;
    let moved = placed.xyz.iter().map(|v| v * v).sum::<f64>().sqrt();
    assert!(moved > 0.01, "the body moved: {placed:?}");
    assert_eq!(harness.state().session.history().undo_depth(), depth + 1, "one undo step");

    harness.state_mut().viewport_mut().free_tool = Some(gc_viewport::FreeTool::Rotate);
    let on_body = harness
        .state()
        .viewport()
        .screen_of(gc_math::DVec3::from_array(placed.xyz) + gc_math::DVec3::new(0.0, 0.0, 0.05))
        .unwrap();
    drag(&mut harness, on_body, egui::vec2(40.0, 0.0));
    let turned = harness.state().session.doc().get(body).unwrap().placement;
    assert!(turned.rpy.iter().any(|a| a.abs() > 0.05), "the body turned: {turned:?}");
    assert_eq!(harness.state().session.history().undo_depth(), depth + 2);
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.state().viewport().free_tool.is_none());
}

/// without a robot once there is such a relationship.
#[test]
fn a_relationship_is_driven_from_the_browser() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    let fixed = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let placement = gc_doc::Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] };
    let moving = app.session.add_body(solid, placement).unwrap();
    let plane = gc_doc::Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy));
    let select = |body: gc_core::Id<gc_doc::Body>| gc_doc::Selection {
        occurrence: gc_doc::Occurrence::Item(body.erase()),
        geometry: plane,
    };
    let relationship = gc_doc::Relationship::new(
        "Mate:1",
        gc_doc::RelationKind::Mate { offset: 0.0, sense: gc_doc::AxisSense::Opposed },
        select(moving),
        select(fixed),
    );
    app.session.add_relationship(relationship).unwrap();
    harness.run();

    // Right click the node in the "관계" folder: the menu offers Drive.
    let at = harness.get_by_label("Mate:1").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    harness.get_by_label("구동").click();
    harness.run_steps(4);
    // The dialog is up: its title and the menu entry's name are the same word.
    assert!(harness.query_all_by_label("구동").count() >= 1);
}

#[test]
fn a_relationship_is_suppressed_from_the_browser() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    let fixed = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let placement = gc_doc::Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] };
    let moving = app.session.add_body(solid, placement).unwrap();
    let plane = gc_doc::Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy));
    let select = |body: gc_core::Id<gc_doc::Body>| gc_doc::Selection {
        occurrence: gc_doc::Occurrence::Item(body.erase()),
        geometry: plane,
    };
    let relationship = gc_doc::Relationship::new(
        "Mate:1",
        gc_doc::RelationKind::Mate { offset: 0.0, sense: gc_doc::AxisSense::Opposed },
        select(moving),
        select(fixed),
    );
    let id = app.session.add_relationship(relationship).unwrap();
    harness.run();
    assert!(!harness.state().session.doc().is_inactive(id.erase()));

    let at = harness.get_by_label("Mate:1").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    harness.get_by_label("억제").click();
    harness.run();
    assert!(harness.state().session.doc().is_inactive(id.erase()));
}

/// The Pattern dialog opens from the ribbon and lists the components to pattern.
#[test]
fn the_pattern_dialog_lists_the_components() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("패턴").click();
    harness.run_steps(3);
    harness.get_by_label("패턴화할 컴포넌트");
    harness.get_by_label("사각형");
    harness.get_by_label("원형").click();
    harness.run_steps(3);
    harness.get_by_label_contains("회전 축");
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-40-assemble-pattern.png")).unwrap();
    }
}

/// View tab > Visibility > Degrees of Freedom shows a symbol at every component that can still move.
#[test]
fn degrees_of_freedom_symbols_are_shown_from_the_view_tab() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.2, 0.2, 0.2] };
    let placement = gc_doc::Placement { xyz: [0.0, 0.0, 0.1], rpy: [0.0; 3] };
    // The first component is grounded; the second one is free.
    harness.state_mut().session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    harness.state_mut().session.add_body(solid, placement).unwrap();
    harness.state_mut().fit_view();
    harness.get_by_label("뷰").click();
    harness.run();
    assert!(harness.state().session.dof_symbols().is_empty());
    harness.get_by_label("자유도").click();
    harness.run_steps(3);
    let symbols = harness.state().session.dof_symbols();
    let counts: Vec<_> = symbols.iter().map(|(_, s)| (s.translations.len(), s.rotations.len())).collect();
    assert!(counts.contains(&(3, 3)) && counts.contains(&(0, 0)), "a free and a grounded body: {counts:?}");
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-42-dof.png")).unwrap();
    }
    harness.get_by_label("자유도").click();
    harness.run_steps(3);
    assert!(harness.state().session.dof_symbols().is_empty());
}

/// Assemble tab > Place with a STEP file: the fixture arrives as an assembly; a broken file is reported in
/// the status bar. Then Grounded on a selected body toggles its grounded state (Inventor: right-click a
/// component > Grounded; https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-2E0B9110-BBAE-482B-A9AF-953BC40E8F2A.htm
/// says the first component placed is grounded).
#[test]
fn place_reads_a_step_assembly_and_ground_toggles_a_component() {
    let directory = tempfile::tempdir().unwrap();
    let broken = directory.path().join("broken.stp");
    std::fs::write(&broken, b"ISO-10303-21; not really").unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/step/two_blocks_ap242.stp");
    let chosen = std::sync::Arc::new(std::sync::Mutex::new(broken.clone()));
    struct Chosen(std::sync::Arc<std::sync::Mutex<std::path::PathBuf>>);
    impl gc_app::FileDialogs for Chosen {
        fn open(&mut self) -> Option<std::path::PathBuf> {
            None
        }
        fn save(&mut self, _: &str) -> Option<std::path::PathBuf> {
            None
        }
        fn open_mesh(&mut self) -> Option<std::path::PathBuf> {
            Some(self.0.lock().unwrap().clone())
        }
    }
    let dialogs = Chosen(std::sync::Arc::clone(&chosen));
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(move |cc| GraphCtrlApp::new(cc, RunOptions::default()).with_dialogs(Box::new(dialogs)));
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("배치").click();
    harness.run_steps(4);
    harness.get_by_label_contains("가져오기 실패: ");
    assert!(harness.state().session.doc().assemblies().is_empty());

    *chosen.lock().unwrap() = fixture;
    harness.get_by_label("배치").click();
    harness.run_steps(4);
    assert_eq!(harness.state().session.doc().assemblies().len(), 1);
    assert!(harness.query_by_label_contains("가져오기 실패").is_none());

    // A body: select it, Grounded toggles.
    let body = harness
        .state_mut()
        .session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    harness.state_mut().session.select_item(Some(gc_session::CellItem::Body(body)));
    harness.run();
    let grounded = |h: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        h.state()
            .session
            .doc()
            .component_flag(gc_doc::Occurrence::Item(body.erase()), gc_doc::ComponentFlag::Grounded)
    };
    let before = grounded(&harness);
    harness.get_by_label("접지").click();
    harness.run();
    assert_ne!(grounded(&harness), before);
    harness.get_by_label("접지").click();
    harness.run();
    assert_eq!(grounded(&harness), before);
}

/// Right-click a component in the browser > Contact Set; Inspect tab > Contact Solver > Contact Set Only.
/// (Inventor 2026: Detect contact between assembly components, GUID-FA1A8282-3A7E-45BE-8DB3-F637FB1C3292.)
#[test]
fn the_contact_set_and_the_contact_solver_are_set_from_the_ui() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let body = harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    assert!(!harness.state().session.doc().in_contact_set(body.erase()));

    let at = harness.get_by_label("Body1:1").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    harness.get_by_label("접촉 세트").click();
    harness.run();
    assert!(harness.state().session.doc().in_contact_set(body.erase()));

    harness.get_by_label("검사").click();
    harness.run();
    harness.get_by_label("접촉 솔버").click();
    harness.run_steps(3);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-43-contact-solver.png")).unwrap();
    }
    harness.get_by_label("접촉 세트만").click();
    harness.run();
    assert_eq!(harness.state().session.doc().contact_solver(), gc_doc::ContactSolver::ContactSet);
}

/// Assemble tab > Copy copies the checked component by the offset; Replace swaps its shape.
#[test]
fn copy_and_replace_work_from_the_assemble_tab() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("복사").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.components().len(), 1);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-44-copy.png")).unwrap();
    }
    harness.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Body1").click();
    harness.run();
    harness.get_by_label("확인").click();
    harness.run();
    assert_eq!(harness.state().session.components().len(), 2, "the copy");

    harness.get_by_label("교체").click();
    harness.run_steps(3);
    harness.get_by_label("구").click();
    harness.run();
    harness.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Body1").click();
    harness.run();
    harness.get_by_label("확인").click();
    harness.run();
    let bodies = harness.state().session.doc().bodies().iter().count();
    assert_eq!(bodies, 2);
}

/// The Representations folder of the browser creates, activates and deletes view and positional
/// representations.
#[test]
fn the_representations_folder_creates_activates_and_deletes() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("표현").click();
    harness.run();
    assert!(harness.state().session.representation_folder().views.is_empty());
    harness.get_by_label("새 뷰 표현").click();
    harness.run();
    let folder = harness.state().session.representation_folder();
    assert_eq!(folder.views.len(), 1);
    harness.get_by_label("새 위치 표현").click();
    harness.run();
    let folder = harness.state().session.representation_folder();
    assert_eq!(folder.positions.len(), 2, "[Primary] and the new one");
    assert!(folder.positions[0].active, "[Primary] stays active until another is chosen");

    // A click on the new position activates it.
    harness.get_by_label("Position:1").click();
    harness.run();
    let folder = harness.state().session.representation_folder();
    assert!(folder.positions[1].active && !folder.positions[0].active);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-45-representations.png")).unwrap();
    }
}

/// A new view representation stores the camera and the section view; activating it brings them back.
#[test]
fn a_view_representation_restores_the_camera_and_the_section() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.state_mut().viewport_mut().camera.distance = 3.0;
    harness.state_mut().viewport_mut().section = Some(gc_viewport::Section {
        kind: gc_viewport::SectionKind::Half,
        planes: vec![gc_viewport::SectionPlane { axis: 1, offset: 0.1, flipped: false }],
        editing: None,
    });
    harness.run();
    harness.get_by_label("표현").click();
    harness.run();
    harness.get_by_label("새 뷰 표현").click();
    harness.run();
    let stored = harness.state().session.representations().views[0].clone();
    assert!(stored.camera.is_some() && stored.section.is_some());

    // Move away: another camera and no section.
    harness.state_mut().viewport_mut().camera.distance = 7.0;
    harness.state_mut().viewport_mut().section = None;
    harness.run();
    harness.get_by_label("View:1").click();
    harness.run();
    let viewport = harness.state().viewport();
    assert!((viewport.camera.distance - 3.0).abs() < 1e-9, "{}", viewport.camera.distance);
    assert_eq!(viewport.section.as_ref().map(|s| s.planes.len()), Some(1));
}

/// At 1280 px the ribbon of a 3D sketch (13 constraints in two rows) still shows "스케치 마무리" inside
/// the window.
#[test]
fn the_3d_sketch_ribbon_keeps_finish_sketch_on_screen() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    pick_variant(&mut harness, "스케치", "3D 스케치");
    harness.run_steps(3);
    let finish = harness.get_by_label("스케치 마무리").rect();
    assert!(finish.right() <= SIZE.x && finish.left() >= 0.0, "{finish:?} outside the {SIZE:?} window");
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-46-3d-sketch-ribbon.png")).unwrap();
    }
    harness.get_by_label("스케치 마무리").click();
    harness.run_steps(3);
}

/// The sketch ribbon's drawing tools are split buttons: the body runs the variant used last, the arrow opens
/// the list, a chosen variant becomes the body; and at 1280 px "스케치 마무리" stays inside the window in
/// 2D and 3D sketches.
#[test]
fn the_sketch_ribbon_has_split_buttons_and_fits_1280_px() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run_steps(3);
    for name in ["스케치 마무리", "스케치 취소"] {
        let rect = harness.get_by_label(name).rect();
        assert!(rect.right() <= SIZE.x, "{name} {rect:?} outside the window");
    }

    // The line tool is current after starting a sketch; the arrow lists the other variants.
    assert_eq!(harness.state().session.tool(), gc_tools::ToolKind::Line);
    mouse_click_label(&mut harness, "직사각형 더 보기");
    harness.run_steps(3);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-47-split-button.png")).unwrap();
    }
    harness.get_by_label("3점 중심 직사각형").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.tool(), gc_tools::ToolKind::CenterRectangle3);
    // It is the body now: another tool, then the body runs it again.
    harness.get_by_label("선택").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.tool(), gc_tools::ToolKind::Select);
    harness.get_by_label("3점 중심 직사각형").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.tool(), gc_tools::ToolKind::CenterRectangle3);

    // A 3D sketch fits as well.
    harness.get_by_label("스케치 마무리").click();
    harness.run_steps(3);
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    pick_variant(&mut harness, "스케치", "3D 스케치");
    harness.run_steps(3);
    let finish = harness.get_by_label("스케치 마무리").rect();
    assert!(finish.right() <= SIZE.x, "{finish:?} outside the window in a 3D sketch");
}

/// A file dialog that always picks the same file.
struct PickFile(std::path::PathBuf);

impl gc_app::FileDialogs for PickFile {
    fn open(&mut self) -> Option<std::path::PathBuf> {
        None
    }
    fn save(&mut self, _: &str) -> Option<std::path::PathBuf> {
        None
    }
    fn open_mesh(&mut self) -> Option<std::path::PathBuf> {
        Some(self.0.clone())
    }
}

/// Assemble tab > Place: the first component stays at the origin; a second body hangs on the cursor over the
/// ground, a click sets it down and attaches a copy, Esc drops that copy (Inventor 2026 help, Place
/// Components, GUID-2E0B9110-BBAE-482B-A9AF-953BC40E8F2A).
#[test]
fn place_hangs_a_body_on_the_cursor_and_sets_it_down_by_clicks() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("plate.stl");
    // A closed box of 40 mm as ASCII STL: twelve triangles.
    let corners: [[f64; 3]; 8] = [
        [0.0, 0.0, 0.0],
        [0.04, 0.0, 0.0],
        [0.04, 0.04, 0.0],
        [0.0, 0.04, 0.0],
        [0.0, 0.0, 0.04],
        [0.04, 0.0, 0.04],
        [0.04, 0.04, 0.04],
        [0.0, 0.04, 0.04],
    ];
    let faces = [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let mut stl = String::from("solid plate\n");
    for face in faces {
        stl.push_str("facet normal 0 0 0\nouter loop\n");
        for index in face {
            let [x, y, z] = corners[index];
            stl.push_str(&format!("vertex {x} {y} {z}\n"));
        }
        stl.push_str("endloop\nendfacet\n");
    }
    stl.push_str("endsolid plate\n");
    std::fs::write(&file, stl).unwrap();

    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(move |cc| {
        GraphCtrlApp::new(cc, RunOptions::default()).with_dialogs(Box::new(PickFile(file)))
    });
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.state_mut().fit_view();
    harness.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("배치").click();
    harness.run_steps(3);
    assert!(harness.state().viewport().is_placing(), "the second component hangs on the cursor");
    let bodies = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        harness.state().session.doc().bodies().iter().count()
    };
    assert_eq!(bodies(&harness), 2);

    // The cursor over the ground at (0, 0.05): the body's origin follows.
    let target = gc_math::DVec3::new(0.0, 0.05, 0.0);
    let at = harness.state().viewport().screen_of(target).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    let hanging = harness.state().session.doc().bodies().iter().last().map(|(id, _)| id).unwrap();
    let shown = harness.state().session.shown_body(hanging).unwrap().placement.xyz;
    assert!(shown[0].abs() < 0.01 && (shown[1] - 0.05).abs() < 0.01, "{shown:?}");

    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-48-place.png")).unwrap();
    }

    // A click sets it down and a copy takes its place on the cursor.
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    assert!(harness.state().viewport().is_placing());
    assert_eq!(bodies(&harness), 3, "a copy hangs now");
    let placed = harness.state().session.doc().get(hanging).unwrap().placement.xyz;
    assert!(placed[0].abs() < 0.01 && (placed[1] - 0.05).abs() < 0.01, "{placed:?}");

    // Esc ends and drops the copy that was still hanging.
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    assert!(!harness.state().viewport().is_placing());
    assert_eq!(bodies(&harness), 2);
}

/// Assemble tab > Assemble opens the command that offers constraint types by the geometry picked
/// (Inventor 2026 help, GUID-C1E8D955-784D-47DE-9DD4-C9C695051320).
#[test]
fn the_assemble_command_opens_from_the_assemble_tab() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    assert_eq!(harness.query_all_by_label("어셈블").count(), 1, "only the ribbon button before");
    harness.get_by_label("어셈블").click();
    harness.run_steps(3);
    assert!(harness.query_all_by_label("어셈블").count() >= 2, "the command window is up");
    harness.get_by_label_contains("첫 번째 선택");
    harness.get_by_label_contains("두 번째 선택");
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-49-assemble.png")).unwrap();
    }
}

/// Right-click a joint in the browser > Lock and Protect (Inventor 2026 help, To Define and Manage Joint
/// Relationships, GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6); a constraint offers neither.
#[test]
fn lock_and_protect_are_set_on_a_joint_from_the_browser() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    let fixed = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let moving =
        app.session.add_body(solid, gc_doc::Placement { xyz: [0.0, 0.0, 0.2], rpy: [0.0; 3] }).unwrap();
    let frame = |body: gc_core::Id<gc_doc::Body>| gc_doc::Selection {
        occurrence: gc_doc::Occurrence::Item(body.erase()),
        geometry: gc_doc::Geometry::origin_frame(),
    };
    let joint = gc_doc::Relationship::new(
        "Rigid:1",
        gc_doc::RelationKind::Rigid { flip: false },
        frame(moving),
        frame(fixed),
    );
    let id = app.session.add_relationship(joint).unwrap();
    harness.run();
    assert!(harness.state().session.joint_condition(id).locked.is_none());

    let right_click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, label: &str| {
        let at = harness.get_by_label(label).rect().center();
        harness.event(egui::Event::PointerMoved(at));
        harness.run();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
        }
    };
    right_click(&mut harness, "Rigid:1");
    harness.get_by_label("잠금").click();
    harness.run();
    assert!(harness.state().session.joint_condition(id).locked.is_some());
    if gpu {
        harness.run_steps(2);
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-50-joint-lock.png")).unwrap();
    }
    right_click(&mut harness, "Rigid:1");
    harness.get_by_label("보호").click();
    harness.run();
    assert!(harness.state().session.joint_condition(id).protected);
    // Clearing the check frees it again.
    right_click(&mut harness, "Rigid:1");
    harness.get_by_label("잠금").click();
    harness.run();
    assert!(harness.state().session.joint_condition(id).locked.is_none());
}

/// File > Application Options > Sketch tab: the autoproject checkbox switches the session setting
/// (Inventor 2026 help, Sketch Tab Reference, GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D).
#[test]
fn the_application_options_switch_autoproject_of_face_edges() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    assert!(harness.state().session.autoproject_face_edges(), "on by default, as in Inventor");
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("응용 프로그램 옵션").click();
    harness.run_steps(3);
    harness.get_by_label("스케치 작성·편집 시 모서리 자동 투영").click();
    harness.run();
    assert!(!harness.state().session.autoproject_face_edges());
    harness.get_by_label("스케치 작성·편집 시 모서리 자동 투영").click();
    harness.run();
    assert!(harness.state().session.autoproject_face_edges());
}

/// The autoproject choice of the Application Options is saved in the settings file and applied at the next
/// start.
#[test]
fn the_autoproject_choice_is_saved_and_read_back() {
    let directory = tempfile::tempdir().unwrap();
    let options =
        || RunOptions { settings_dir: Some(directory.path().to_path_buf()), ..RunOptions::default() };
    let mut harness =
        egui_kittest::Harness::builder().with_size(SIZE).build_eframe(|cc| GraphCtrlApp::new(cc, options()));
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("응용 프로그램 옵션").click();
    harness.run_steps(3);
    harness.get_by_label("스케치 작성·편집 시 모서리 자동 투영").click();
    harness.run();
    assert!(!harness.state().session.autoproject_face_edges());
    assert!(gc_persist::places::load_settings(directory.path()).autoproject_edges_off);

    // The next start reads it.
    let again =
        egui_kittest::Harness::builder().with_size(SIZE).build_eframe(|cc| GraphCtrlApp::new(cc, options()));
    assert!(!again.state().session.autoproject_face_edges());
}

/// Subassemblies in the browser: Demote groups a component into a folder, a double click edits it in place
/// (the rest is out of scope, faded), the ribbon's Return to Top backs out, Dissolve ungroups (Inventor 2026
/// help, Create Subassemblies in Place, GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353; Edit components in
/// place, GUID-C79C36C9-A6DB-46C5-8864-41F66C750231).
#[test]
fn subassemblies_are_made_edited_in_place_and_dissolved_from_the_browser() {
    let gpu = gc_test_support::gpu().is_some();
    let builder =
        egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0).with_step_dt(0.05);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    let first = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let second =
        app.session.add_body(solid, gc_doc::Placement { xyz: [0.2, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    harness.run();

    let right_click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, label: &str| {
        let at = harness.get_by_label(label).rect().center();
        harness.event(egui::Event::PointerMoved(at));
        harness.run();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
        }
    };
    right_click(&mut harness, "Body2:1");
    harness.get_by_label("서브어셈블리로 묶기 (강등)").click();
    harness.run_steps(3);
    let name = egui::accesskit::Role::TextInput;
    harness.get_by_role_and_label(name, "서브어셈블리로 묶기 (강등)").click();
    harness.run();
    harness.get_by_role_and_label(name, "서브어셈블리로 묶기 (강등)").type_text("Group");
    harness.run();
    harness.get_by_label("확인").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.doc().subassemblies().len(), 1);
    assert_eq!(harness.state().session.doc().subassemblies()[0].name, "Group");

    // A double click on the folder edits it in place: the other component is outside, faded.
    let at = harness.get_by_label("Group").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    // Two clicks close in time: a short step on the harness clock.
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
    }
    harness.run_steps(3);
    assert!(harness.state().session.edit_target().is_some());
    assert!(harness.state().session.in_edit_scope(gc_doc::Occurrence::Item(second.erase())));
    assert!(!harness.state().session.in_edit_scope(gc_doc::Occurrence::Item(first.erase())));
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-51-edit-in-place.png")).unwrap();
    }

    // Return to Top on the Assemble tab backs out.
    harness.get_by_label("조립").click();
    harness.run();
    pick_variant(&mut harness, "편집", "최상위로");
    harness.run_steps(3);
    assert!(harness.state().session.edit_target().is_none());

    // Dissolve ungroups.
    right_click(&mut harness, "Group");
    harness.get_by_label("해체").click();
    harness.run_steps(3);
    assert!(harness.state().session.doc().subassemblies().is_empty());
}

/// A sketch made on a component's plane hangs below that component's row in the browser, not in the
/// top-level sketch folder.
#[test]
fn a_component_sketch_hangs_below_its_component_in_the_browser() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let body = harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    let occurrence = gc_doc::Occurrence::Item(body.erase());
    // A sketch made while the component is edited in place belongs to it.
    harness.state_mut().session.edit_component(gc_doc::TreeKey::Component(occurrence)).unwrap();
    harness.state_mut().session.create_sketch_on_component(occurrence, gc_doc::OriginPlane::Xy).unwrap();
    harness.run();
    let sketches = harness.state().session.doc().sketches().iter().map(|(id, _)| id).collect::<Vec<_>>();
    assert_eq!(sketches.len(), 1);
    let session = &harness.state().session;
    assert_eq!(session.sketch_owner(sketches[0]), Some(gc_doc::TreeKey::Component(occurrence)));
    assert!(session.sketches_of(None).is_empty(), "it is not a top-level sketch");
    // Its row is under the component: below "Body1:1" on the screen.
    let body_row = harness.get_by_label("Body1:1").rect().top();
    let sketch_row = harness.get_by_label("Sketch1").rect().top();
    assert!(sketch_row > body_row, "{sketch_row} below {body_row}");
}

/// 3D Model tab > Work Axis opens the dialog; a work axis (intersection of two planes) is listed in the
/// browser's Work Features folder, drawn in the view, hidden by its check box and deleted by its button
/// (Inventor 2026 help, To Create Work Features, GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385).
#[test]
fn a_work_axis_is_made_listed_hidden_and_deleted() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("작업 축").click();
    harness.run_steps(3);
    assert!(harness.query_all_by_label("작업 축").count() >= 2, "the dialog is up");
    harness.get_by_label("두 평면의 교선").click();
    harness.run();
    // The same plane twice is refused (nothing is made); two different planes make the axis.
    let session = &mut harness.state_mut().session;
    let geometry = gc_doc::WorkGeometry::AxisPlanesIntersection {
        planes: [
            gc_doc::PlaneRef::Origin(gc_doc::OriginPlane::Xy),
            gc_doc::PlaneRef::Origin(gc_doc::OriginPlane::Yz),
        ],
    };
    let op = gc_doc::CellOp::AddWorkFeature(gc_doc::WorkFeature {
        name: "Axis1".to_owned(),
        geometry,
        visible: true,
    });
    session.execute(&gc_doc::Command::Cell(op)).unwrap();
    harness.run_steps(3);
    harness.get_by_label("Axis1");
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-52-work-axis.png")).unwrap();
    }
    let id = harness.state().session.doc().work_features().iter().next().unwrap().0;
    harness.get_by_label("Axis1 보이기").click();
    harness.run();
    assert!(!harness.state().session.doc().work_features().get(id).unwrap().visible);
    harness.get_by_label("Axis1 삭제").click();
    harness.run();
    assert!(harness.state().session.doc().work_features().iter().next().is_none());
}

/// A subassembly folder is demoted into another subassembly from its own menu: folders nest without
/// dragging.
#[test]
fn a_subassembly_folder_is_nested_from_its_menu() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let second = app.session.add_body(solid, gc_doc::Placement::default()).unwrap();
    let key = gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(second.erase()));
    app.session
        .edit_subassemblies(gc_doc::SubassemblyOp::Demote { members: vec![key], name: "Inner".to_owned() })
        .unwrap();
    harness.run();

    let at = harness.get_by_label("Inner").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    harness.get_by_label("서브어셈블리로 묶기 (강등)").click();
    harness.run_steps(3);
    let name = egui::accesskit::Role::TextInput;
    harness.get_by_role_and_label(name, "서브어셈블리로 묶기 (강등)").click();
    harness.run();
    harness.get_by_role_and_label(name, "서브어셈블리로 묶기 (강등)").type_text("Outer");
    harness.run();
    harness.get_by_label("확인").click();
    harness.run_steps(3);
    let groups = harness.state().session.doc().subassemblies();
    assert_eq!(groups.len(), 2);
    let inner = groups.iter().find(|g| g.name == "Inner").unwrap();
    let outer = groups.iter().find(|g| g.name == "Outer").unwrap();
    assert_eq!(inner.parent, Some(outer.id), "Inner now sits in Outer");
}

/// Work planes and points have their dialogs; a double click in the browser edits a feature; a click in the
/// view selects it and Delete removes it.
#[test]
fn work_features_are_edited_from_the_browser_and_selected_in_the_view() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let add = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, name: &str, geometry| {
        let op = gc_doc::CellOp::AddWorkFeature(gc_doc::WorkFeature {
            name: name.to_owned(),
            geometry,
            visible: true,
        });
        harness.state_mut().session.execute(&gc_doc::Command::Cell(op)).unwrap();
    };
    add(
        &mut harness,
        "Plane1",
        gc_doc::WorkGeometry::PlaneOffset {
            base: gc_doc::PlaneRef::Origin(gc_doc::OriginPlane::Xy),
            distance: 0.05,
        },
    );
    add(
        &mut harness,
        "Axis1",
        gc_doc::WorkGeometry::AxisThroughPoints {
            points: [gc_doc::PointRef::Origin, gc_doc::PointRef::Fixed([0.0, 0.0, 0.2])],
        },
    );
    add(&mut harness, "Point1", gc_doc::WorkGeometry::Point { at: gc_doc::PointRef::Fixed([0.1, 0.0, 0.0]) });
    harness.run();

    // The ribbon has the three commands.
    harness.get_by_label("작업 평면");
    harness.get_by_label("작업 점");

    // A double click on a row in the browser opens the dialog on that feature.
    let at = harness.get_by_label("Plane1").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
    }
    harness.run_steps(3);
    assert!(harness.query_all_by_label("작업 평면").count() >= 2, "the dialog is up on the plane");
    assert!(harness.query_all_by_label("거리 (mm)").count() >= 1);

    // In the view: a click on the axis selects it, Delete removes it.
    harness.state_mut().fit_view();
    harness.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let axis =
        harness.state().session.doc().work_features().iter().find(|(_, f)| f.name == "Axis1").unwrap().0;
    let on_axis = harness.state().viewport().screen_of(gc_math::DVec3::new(0.0, 0.0, 0.1)).unwrap();
    harness.event(egui::Event::PointerMoved(on_axis));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: on_axis,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    harness.run_steps(3);
    assert_eq!(harness.state().viewport().work_selected, Some(axis));
    harness.key_press(egui::Key::Delete);
    harness.run_steps(3);
    assert!(harness.state().session.doc().work_features().iter().all(|(_, f)| f.name != "Axis1"));
}

/// At 1280 px every ribbon tab keeps all its buttons inside the window (and the Assemble and 3D Model tabs
/// have room to spare for the commands still to come).
#[test]
fn every_ribbon_tab_fits_1280_px() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    app.session.add_body(solid, gc_doc::Placement { xyz: [0.2, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    harness.run();
    let rightmost = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        harness
            .query_all_by_role(egui::accesskit::Role::Button)
            .map(|node| node.rect())
            .filter(|rect| rect.top() > 40.0 && rect.bottom() < 140.0)
            .map(|rect| rect.right())
            .fold(0.0_f32, f32::max)
    };
    let mut report = Vec::new();
    for tab in ["3D 모델", "조립", "로봇", "검사", "관리", "뷰"] {
        harness.get_by_label(tab).click();
        harness.run_steps(3);
        let right = rightmost(&harness);
        report.push((tab, right));
        if gpu && tab == "조립" {
            let image = harness.render().unwrap();
            let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
            std::fs::create_dir_all(&out).unwrap();
            image.save(out.join("review-53-assemble-tab.png")).unwrap();
        }
        assert!(right <= SIZE.x, "{tab}: the ribbon reaches {right} of {}", SIZE.x);
    }
    // Room to spare on the two tabs that gain commands: the last button ends well inside the window.
    for (tab, right) in &report {
        if matches!(*tab, "3D 모델" | "조립") {
            assert!(*right <= SIZE.x - 150.0, "{tab} has only {} px to spare", SIZE.x - right);
        }
    }
}

/// The sketch tab has Trim, Extend and Fillet buttons that start their tools (Inventor 2026 help, Trim,
/// GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D); outside a sketch C opens the Constrain command and a
/// sketch's C stays the circle (Predefined Command Alias and Shortcut Reference,
/// GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[test]
fn trim_extend_and_fillet_have_ribbon_buttons_and_c_means_constrain_outside_sketches() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    // C outside a sketch opens the Constrain command (its window title is "구속").
    harness.key_press(egui::Key::C);
    harness.run_steps(3);
    assert!(harness.query_all_by_label("구속").count() >= 1);

    harness.get_by_label("XY 스케치").click();
    harness.run_steps(3);
    for (label, kind) in
        [("자르기", ToolKind::Trim), ("연장", ToolKind::Extend), ("모깎기", ToolKind::Fillet)]
    {
        harness.get_by_label(label).click();
        harness.run_steps(2);
        assert_eq!(harness.state().session.tool(), kind, "{label}");
    }
    // In a sketch C is the circle.
    harness.key_press(egui::Key::C);
    harness.run_steps(2);
    assert_eq!(harness.state().session.tool(), ToolKind::Circle);
    // The sketch ribbon still fits.
    let finish = harness.get_by_label("스케치 마무리").rect();
    // With 40 px to spare for the commands still to come (ellipse, spline).
    assert!(finish.right() <= SIZE.x - 40.0, "{finish:?}");
}

/// The dimension box has a Driven check box: a driving dimension becomes a reference (driven) one and, when
/// that would not over-constrain, goes back (Inventor 2026 help, To Add Dimensions,
/// GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185).
#[test]
fn the_dimension_box_toggles_driven_and_driving() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Select);
    let (_, sketch) = app.session.active_sketch().unwrap();
    let width = *sketch.dimensions.iter().find(|(_, d)| d.source == "60 mm").unwrap().0;
    app.edit_dimension(width);
    harness.run();

    let driving = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        harness.state().session.active_sketch().unwrap().1.dimensions[&width].driving
    };
    assert!(driving(&harness));
    harness.get_by_label("종속 치수").click();
    harness.run_steps(3);
    assert!(!driving(&harness), "now a reference dimension");
    // The box is closed; open it again on the driven dimension: only the check box is there.
    harness.state_mut().edit_dimension(width);
    harness.run_steps(3);
    assert!(harness.query_all_by_label("dimension expression").next().is_none());
    harness.get_by_label("종속 치수").click();
    harness.run_steps(3);
    assert!(driving(&harness), "driving again");
}

/// `HGHT = 5 mm` typed in the dimension box makes the parameter and the dimension refers to it (Inventor 2026
/// help, To Add Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185); a bad name is refused.
#[test]
fn a_parameter_is_made_from_the_dimension_box() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("XY 스케치").click();
    harness.run();
    draw_sample(harness.state_mut());
    let app = harness.state_mut();
    app.session.set_tool(ToolKind::Select);
    let (_, sketch) = app.session.active_sketch().unwrap();
    let width = *sketch.dimensions.iter().find(|(_, d)| d.source == "60 mm").unwrap().0;
    app.edit_dimension(width);
    harness.run();
    let retype = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, text: &str| {
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness.get_by_label("dimension expression").type_text(text);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
    };
    let names = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>| -> Vec<(String, String)> {
        let params = harness.state().session.doc().params();
        params.iter().map(|(_, p)| (p.name.clone(), p.source.clone())).collect()
    };
    // A name that starts with a digit is refused: no parameter, the box stays.
    retype(&mut harness, "1a = 5 mm");
    assert!(names(&harness).is_empty());
    harness.get_by_label("dimension expression");
    // A good name makes the parameter and the dimension refers to it.
    retype(&mut harness, "HGHT = 5 mm");
    assert_eq!(names(&harness), vec![("HGHT".to_owned(), "5 mm".to_owned())]);
    let session = &harness.state().session;
    assert_eq!(session.active_sketch().unwrap().1.dimensions[&width].source, "HGHT");
    // One undo step takes back both the parameter and the dimension's use of it.
    harness.state_mut().session.undo();
    assert!(names(&harness).is_empty(), "the parameter went with one undo");
    assert_ne!(harness.state().session.active_sketch().unwrap().1.dimensions[&width].source, "HGHT");
    harness.state_mut().session.redo();
    // Reusing the name changes the parameter.
    harness.state_mut().edit_dimension(width);
    harness.run();
    retype(&mut harness, "HGHT = 8 mm");
    assert_eq!(names(&harness), vec![("HGHT".to_owned(), "8 mm".to_owned())]);
}

/// Ctrl+A over the view lists the components under the cursor (Select Other); a click on one selects it
/// (Inventor 2026 help, Predefined Command Alias and Shortcut Reference,
/// GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[test]
fn select_other_lists_what_lies_under_the_cursor() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.1, 0.1, 0.1] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    // The same place: a ray through one passes the other as well.
    let second = app.session.add_body(solid, gc_doc::Placement::default()).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let at = harness.state().viewport().screen_of(gc_math::DVec3::ZERO).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run_steps(3);
    let button = egui::accesskit::Role::Button;
    harness.get_by_role_and_label(button, "Body1");
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-54-select-other.png")).unwrap();
    }
    harness.get_by_role_and_label(button, "Body2").click();
    harness.run_steps(3);
    assert_eq!(
        harness.state().session.selected_item(),
        Some(gc_session::CellItem::Body(second)),
        "the second one is selected"
    );
    assert!(
        harness.query_all_by_role_and_label(button, "Body1").next().is_none(),
        "the list closed after the choice"
    );
}

/// Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C):
/// pick an edge, click a start point, then an end point; the component moves along the edge by the part
/// of the way that lies along it; Done sets it down as one undo step.
#[test]
fn grip_snap_moves_a_component_along_a_picked_edge() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let app = harness.state_mut();
    // The first component is grounded; the one that moves is the second.
    app.session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [-0.5, 0.0, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    let id =
        app.session.add_body(gc_doc::Solid::Box { size: [0.2; 3] }, gc_doc::Placement::default()).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    harness.get_by_label("그립 스냅").click();
    harness.run_steps(3);
    harness.get_by_label("기준이 될 구성요소의 모서리·꼭짓점·평면을 선택하세요");
    let screen = |h: &egui_kittest::Harness<'_, GraphCtrlApp>, p: [f64; 3]| {
        h.state().viewport().screen_of(gc_math::DVec3::from(p)).unwrap()
    };
    let click = |h: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2| {
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(3);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(3);
        }
    };
    // The top face's edge along Y at x = 0.1 (a little inside, where the top face is hit).
    let edge = screen(&harness, [0.098, 0.0, 0.1]);
    click(&mut harness, edge);
    harness.get_by_label("시작점을 클릭하세요");
    let from = screen(&harness, [0.0, 0.0, 0.1]);
    click(&mut harness, from);
    harness.get_by_label("끝점을 클릭하면 위치가 확정됩니다");
    let to = screen(&harness, [0.0, 0.06, 0.1]);
    harness.event(egui::Event::PointerMoved(to));
    harness.run_steps(3);
    if gpu {
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-55-grip-snap.png")).unwrap();
    }
    click(&mut harness, to);
    harness.get_by_label("완료 또는 접지 후 완료를 누르세요");
    assert_eq!(harness.state().session.doc().get(id).unwrap().placement.xyz, [0.0; 3], "still a preview");
    harness.get_by_label("완료").click();
    harness.run_steps(3);
    let xyz = harness.state().session.doc().get(id).unwrap().placement.xyz;
    assert!((xyz[1] - 0.06).abs() < 0.01, "followed the cursor along the edge: {xyz:?}");
    assert!(xyz[0].abs() < 1e-9 && xyz[2].abs() < 1e-9, "only along it: {xyz:?}");
    assert!(!harness.state().viewport().is_grip_snapping(), "Done ends the command");
    harness.state_mut().session.undo();
    let back = harness.state().session.doc().get(id).unwrap().placement.xyz;
    assert!(back[1].abs() < 1e-9, "one undo step puts it back: {back:?}");
}

/// Shift + right click over the view opens the Select command's menu (Inventor 2026 help, shortcut table
/// GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E: "Activates the Select command menu"): the selection priority
/// and Select Other. With the work feature priority a click on a body selects nothing.
#[test]
fn shift_right_click_opens_the_select_menu() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.1, 0.1, 0.1] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    app.session.add_body(solid, gc_doc::Placement::default()).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let at = harness.state().viewport().screen_of(gc_math::DVec3::ZERO).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    harness.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::SHIFT,
    });
    harness.run_steps(3);
    harness.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: egui::Modifiers::SHIFT,
    });
    harness.run_steps(3);
    harness.get_by_label("구성요소 우선");
    harness.get_by_label("작업 피처 우선");
    // Select Other from the menu lists the two bodies.
    harness.get_by_label("다른 항목 선택").click();
    harness.run_steps(3);
    let button = egui::accesskit::Role::Button;
    harness.get_by_role_and_label(button, "Body1");
    harness.get_by_role_and_label(button, "Body2");
}

/// With the work feature priority a click on a body selects nothing.
#[test]
fn the_work_feature_priority_leaves_bodies_unselected() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let app = harness.state_mut();
    app.session.add_body(gc_doc::Solid::Box { size: [0.1; 3] }, gc_doc::Placement::default()).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    app.viewport_mut().select_priority = gc_viewport::SelectPriority::Feature;
    harness.run();
    let at = harness.state().viewport().screen_of(gc_math::DVec3::ZERO).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    assert_eq!(harness.state().session.selected_item(), None);
    harness.state_mut().viewport_mut().select_priority = gc_viewport::SelectPriority::Component;
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    assert!(harness.state().session.selected_item().is_some());
}

/// A work feature clicked in the view is geometry for the Assemble command (Inventor 2026 help, To Place
/// Mate or Flush Constraints, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E: "Select the first point, curve,
/// face, or work feature").
#[test]
fn a_work_feature_clicked_in_the_view_fills_the_assemble_selection() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let op = gc_doc::CellOp::AddWorkFeature(gc_doc::WorkFeature {
        name: "Point1".to_owned(),
        geometry: gc_doc::WorkGeometry::Point { at: gc_doc::PointRef::Fixed([-0.4, 0.0, 0.0]) },
        visible: true,
    });
    for x in [-0.3, -0.5] {
        harness
            .state_mut()
            .session
            .add_body(
                gc_doc::Solid::Box { size: [0.05; 3] },
                gc_doc::Placement { xyz: [x, 0.0, 0.0], rpy: [0.0; 3] },
            )
            .unwrap();
    }
    harness.state_mut().session.execute(&gc_doc::Command::Cell(op)).unwrap();
    harness.state_mut().fit_view();
    harness.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    harness.get_by_label("어셈블").click();
    harness.run_steps(3);
    let at = harness.state().viewport().screen_of(gc_math::DVec3::new(-0.4, 0.0, 0.0)).unwrap();
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    harness.get_by_label_contains("첫 번째 선택");
    harness.get_by_label("첫 번째 선택: Point1 · 점");
}

/// The Limits tab of a joint (Inventor 2026 help, To Define and Manage Joint Relationships,
/// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6): Start and End in degrees for a rotational joint; the
/// browser then marks the joint +/-.
#[test]
fn a_rotational_joint_gets_angular_limits_from_its_dialog() {
    let gpu = gc_test_support::gpu().is_some();
    let builder = egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut harness = builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let app = harness.state_mut();
    let frame = |z: f64, nz: f64| gc_doc::Geometry::Frame {
        origin: [0.0, 0.0, z],
        z: [0.0, 0.0, nz],
        x: [1.0, 0.0, 0.0],
    };
    let sel = |id: gc_core::AnyId, geometry| gc_doc::Selection {
        occurrence: gc_doc::Occurrence::Item(id),
        geometry,
    };
    let table = app
        .session
        .add_body(
            gc_doc::Solid::Box { size: [2.0, 1.0, 1.0] },
            gc_doc::Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] },
        )
        .unwrap();
    let block = app
        .session
        .add_body(
            gc_doc::Solid::Box { size: [0.2; 3] },
            gc_doc::Placement { xyz: [0.3, 0.2, 1.6], rpy: [0.0; 3] },
        )
        .unwrap();
    let id = app
        .session
        .add_relationship(gc_doc::Relationship::new(
            "Hinge",
            gc_doc::RelationKind::Rotational { flip: false },
            sel(block.erase(), frame(-0.1, -1.0)),
            sel(table.erase(), frame(0.5, 1.0)),
        ))
        .unwrap();
    harness.run();
    let at = harness.get_by_label("Hinge").rect().center();
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    harness.get_by_label("한계…").click();
    harness.run_steps(3);
    harness.get_by_label("각도 한계").click();
    harness.run_steps(3);
    let input = egui::accesskit::Role::TextInput;
    for (label, text) in [("시작", "-30"), ("끝", "45")] {
        let field = harness.get_by_role_and_label(input, label);
        field.click();
        harness.run_steps(2);
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run_steps(2);
        harness.get_by_role_and_label(input, label).type_text(text);
        harness.run_steps(2);
    }
    if gc_test_support::gpu().is_some() {
        harness.run_steps(2);
        let image = harness.render().unwrap();
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
        std::fs::create_dir_all(&out).unwrap();
        image.save(out.join("review-56-joint-limits.png")).unwrap();
    }
    harness.get_by_label("확인").click();
    harness.run_steps(3);
    let (angular, linear) = harness.state().session.joint_limits(id);
    let range = angular.expect("angular limits set");
    assert!(linear.is_none());
    assert!((range.start + 30f64.to_radians()).abs() < 1e-9, "{range:?}");
    assert!((range.end - 45f64.to_radians()).abs() < 1e-9, "{range:?}");
    harness.get_by_label("Hinge +/-");
}

/// Grip Snap with a flat face as the reference (Inventor 2026 help, Grip Snap Move Options Bar Reference,
/// GUID-C030C32C-7E1E-4F37-BBDF-120412898D00: Plane, Move along normal using reference geometry): the
/// component moves along the face's normal by the part of the way that lies along it.
#[test]
fn grip_snap_moves_a_component_along_the_normal_of_a_picked_face() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let app = harness.state_mut();
    app.session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [-0.5, 0.0, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    let id =
        app.session.add_body(gc_doc::Solid::Box { size: [0.2; 3] }, gc_doc::Placement::default()).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    harness.get_by_label("그립 스냅").click();
    harness.run_steps(3);
    let screen = |h: &egui_kittest::Harness<'_, GraphCtrlApp>, p: [f64; 3]| {
        h.state().viewport().screen_of(gc_math::DVec3::from(p)).unwrap()
    };
    let click = |h: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2| {
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(3);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(3);
        }
    };
    // The middle of the top face.
    let face = screen(&harness, [0.0, 0.0, 0.1]);
    click(&mut harness, face);
    harness.get_by_label("면 안에서 이동");
    harness.get_by_label("법선 방향 이동").click();
    harness.run_steps(3);
    click(&mut harness, face);
    let to = screen(&harness, [0.0, 0.0, 0.16]);
    harness.event(egui::Event::PointerMoved(to));
    harness.run_steps(3);
    click(&mut harness, to);
    harness.get_by_label("완료").click();
    harness.run_steps(3);
    let xyz = harness.state().session.doc().get(id).unwrap().placement.xyz;
    assert!((xyz[2] - 0.06).abs() < 0.01, "followed the cursor along the normal: {xyz:?}");
    assert!(xyz[0].abs() < 1e-9 && xyz[1].abs() < 1e-9, "only along it: {xyz:?}");
}

/// A subassembly is one unit in its parent (Inventor 2026 help, Create Subassemblies in Place,
/// GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353): Free Move on one of its components moves all of it, as one
/// undo step; one holding a grounded component does not move.
#[test]
fn free_move_drags_a_whole_subassembly() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.1; 3] };
    let app = harness.state_mut();
    // The first component is grounded.
    let ground = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let at = |x: f64, y: f64| gc_doc::Placement { xyz: [x, y, 0.0], rpy: [0.0; 3] };
    let a = app.session.add_body(solid.clone(), at(0.3, 0.0)).unwrap();
    let b = app.session.add_body(solid, at(0.3, 0.3)).unwrap();
    let key =
        |id: gc_core::Id<gc_doc::Body>| gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(id.erase()));
    app.session
        .edit_subassemblies(gc_doc::SubassemblyOp::Demote {
            members: vec![key(a), key(b)],
            name: "Unit".into(),
        })
        .unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    app.viewport_mut().free_tool = Some(gc_viewport::FreeTool::Move);
    harness.run();
    let drag = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, from: egui::Pos2, by: egui::Vec2| {
        harness.event(egui::Event::PointerMoved(from));
        harness.run_steps(2);
        harness.event(egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
        for step in 1..=4 {
            harness.event(egui::Event::PointerMoved(from + by * (step as f32 / 4.0)));
            harness.run_steps(2);
        }
        harness.event(egui::Event::PointerButton {
            pos: from + by,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
    };
    let xyz = |h: &egui_kittest::Harness<'_, GraphCtrlApp>, id: gc_core::Id<gc_doc::Body>| {
        h.state().session.doc().get(id).unwrap().placement.xyz
    };
    let (a0, b0) = (xyz(&harness, a), xyz(&harness, b));
    let depth = harness.state().session.history().undo_depth();
    let on_a = harness.state().viewport().screen_of(gc_math::DVec3::new(0.3, 0.0, 0.05)).unwrap();
    // Half way through the drag the whole unit is shown moved; nothing is in the document yet.
    harness.event(egui::Event::PointerMoved(on_a));
    harness.run_steps(2);
    harness.event(egui::Event::PointerButton {
        pos: on_a,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(2);
    for step in 1..=2 {
        harness.event(egui::Event::PointerMoved(on_a + egui::vec2(30.0 * step as f32, 0.0)));
        harness.run_steps(2);
    }
    let shown = harness.state().session.shown_body(b).unwrap().placement.xyz;
    assert!(shown[0] > b0[0] + 0.005, "the other component is previewed too: {shown:?}");
    assert_eq!(xyz(&harness, b), b0, "the document is untouched until the drop");
    harness.event(egui::Event::PointerMoved(on_a + egui::vec2(60.0, 0.0)));
    harness.run_steps(2);
    harness.event(egui::Event::PointerButton {
        pos: on_a + egui::vec2(60.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(2);
    let (a1, b1) = (xyz(&harness, a), xyz(&harness, b));
    let da = [a1[0] - a0[0], a1[1] - a0[1], a1[2] - a0[2]];
    let db = [b1[0] - b0[0], b1[1] - b0[1], b1[2] - b0[2]];
    assert!(da.iter().map(|v| v * v).sum::<f64>().sqrt() > 0.01, "the dragged component moved: {da:?}");
    for i in 0..3 {
        assert!((da[i] - db[i]).abs() < 1e-9, "the other one went along: {da:?} {db:?}");
    }
    assert_eq!(harness.state().session.history().undo_depth(), depth + 1, "one undo step");

    // With the grounded body in the unit nothing moves.
    let g = harness.state().session.doc().get(ground).unwrap().placement.xyz;
    let key0 = gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(ground.erase()));
    let unit = harness.state().session.doc().subassemblies().first().map(|s| s.id).unwrap();
    harness
        .state_mut()
        .session
        .edit_subassemblies(gc_doc::SubassemblyOp::Move { members: vec![key0], into: Some(unit) })
        .unwrap();
    harness.run();
    let on_a = harness
        .state()
        .viewport()
        .screen_of(gc_math::DVec3::from_array(a1) + gc_math::DVec3::new(0.0, 0.0, 0.05))
        .unwrap();
    drag(&mut harness, on_a, egui::vec2(60.0, 0.0));
    assert_eq!(xyz(&harness, a), a1, "a unit with a grounded component stays");
    assert_eq!(xyz(&harness, ground), g);
}

/// Grip Snap on a component of a subassembly takes the rest of the unit along (GUID-FCEE3F0E-AAE2-4619-8645-2588FEECD353).
#[test]
fn grip_snap_moves_the_whole_subassembly_of_the_picked_component() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let app = harness.state_mut();
    // The first component is grounded; the one that moves is the second.
    app.session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [-0.5, 0.0, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    let id =
        app.session.add_body(gc_doc::Solid::Box { size: [0.2; 3] }, gc_doc::Placement::default()).unwrap();
    let mate = app
        .session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [0.0, 0.5, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    let key = |b: gc_core::Id<gc_doc::Body>| gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(b.erase()));
    app.session
        .edit_subassemblies(gc_doc::SubassemblyOp::Demote {
            members: vec![key(id), key(mate)],
            name: "Unit".into(),
        })
        .unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    harness.get_by_label("그립 스냅").click();
    harness.run_steps(3);
    harness.get_by_label("기준이 될 구성요소의 모서리·꼭짓점·평면을 선택하세요");
    let screen = |h: &egui_kittest::Harness<'_, GraphCtrlApp>, p: [f64; 3]| {
        h.state().viewport().screen_of(gc_math::DVec3::from(p)).unwrap()
    };
    let click = |h: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2| {
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(3);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(3);
        }
    };
    // The top face's edge along Y at x = 0.1 (a little inside, where the top face is hit).
    let edge = screen(&harness, [0.098, 0.0, 0.1]);
    click(&mut harness, edge);
    harness.get_by_label("시작점을 클릭하세요");
    let from = screen(&harness, [0.0, 0.0, 0.1]);
    click(&mut harness, from);
    harness.get_by_label("끝점을 클릭하면 위치가 확정됩니다");
    let to = screen(&harness, [0.0, 0.06, 0.1]);
    harness.event(egui::Event::PointerMoved(to));
    harness.run_steps(3);
    click(&mut harness, to);
    harness.get_by_label("완료 또는 접지 후 완료를 누르세요");
    assert_eq!(harness.state().session.doc().get(id).unwrap().placement.xyz, [0.0; 3], "still a preview");
    harness.get_by_label("완료").click();
    harness.run_steps(3);
    let xyz = harness.state().session.doc().get(id).unwrap().placement.xyz;
    assert!((xyz[1] - 0.06).abs() < 0.01, "followed the cursor along the edge: {xyz:?}");
    assert!(xyz[0].abs() < 1e-9 && xyz[2].abs() < 1e-9, "only along it: {xyz:?}");
    let other = harness.state().session.doc().get(mate).unwrap().placement.xyz;
    assert!((other[1] - 0.5 - xyz[1]).abs() < 1e-9, "the rest of the unit went along: {other:?}");
    assert!(!harness.state().viewport().is_grip_snapping(), "Done ends the command");
}

/// F5 returns to the previous view, Shift + F5 goes to the next; S makes a sketch (Inventor 2026 help,
/// Predefined Command Alias and Shortcut Reference, GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[test]
fn f5_walks_the_views_and_s_makes_a_sketch() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.1)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.state_mut().viewport_mut().look(gc_draw::StandardView::Front);
    harness.run_steps(8);
    let front = harness.state().viewport().camera;
    harness.state_mut().viewport_mut().look(gc_draw::StandardView::Right);
    harness.run_steps(8);
    let right = harness.state().viewport().camera;
    assert_ne!(front, right);
    harness.key_press(egui::Key::F5);
    harness.run_steps(2);
    assert_eq!(harness.state().viewport().camera, front, "back to the previous view");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F5);
    harness.run_steps(2);
    assert_eq!(harness.state().viewport().camera, right, "and forward again");

    assert!(harness.state().session.active_sketch().is_none());
    harness.key_press(egui::Key::S);
    harness.run_steps(3);
    assert!(harness.state().session.active_sketch().is_some(), "S made a sketch on XY");
}

/// Tab demotes the selected component into a new subassembly, Shift + Tab promotes it again (Inventor 2026
/// help, shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[test]
fn tab_demotes_and_shift_tab_promotes_the_selected_component() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05; 3] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let body = app.session.add_body(solid, gc_doc::Placement::default()).unwrap();
    app.session.select_item(Some(gc_session::CellItem::Body(body)));
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run_steps(3);
    harness.get_by_label("확인").click();
    harness.run_steps(3);
    let occurrence = gc_doc::Occurrence::Item(body.erase());
    assert!(harness.state().session.doc().subassembly_of(occurrence).is_some(), "demoted");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    harness.run_steps(3);
    assert!(harness.state().session.doc().subassembly_of(occurrence).is_none(), "promoted to the top");
}

/// Alt + drag from a face of a component onto a face of another places a Mate (Inventor 2026 help, To
/// Place Mate or Flush Constraints in Assemblies, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E: "Hold down
/// ALT, and then click the component to drag ... release the mouse to place the mate constraint").
#[test]
fn alt_drag_places_a_mate_between_two_faces() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.1; 3] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let free = gc_doc::Placement { xyz: [0.4, 0.0, 0.0], rpy: [0.0; 3] };
    app.session.add_body(solid, free).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let from = harness.state().viewport().screen_of(gc_math::DVec3::new(0.4, 0.0, 0.05)).unwrap();
    let to = harness.state().viewport().screen_of(gc_math::DVec3::new(0.0, 0.0, 0.05)).unwrap();
    let alt = egui::Modifiers::ALT;
    harness.event(egui::Event::PointerMoved(from));
    harness.run_steps(2);
    harness.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: alt,
    });
    harness.run_steps(2);
    for step in 1..=4 {
        harness.event(egui::Event::PointerMoved(from + (to - from) * (step as f32 / 4.0)));
        harness.run_steps(2);
    }
    assert_eq!(harness.state().session.relationships().len(), 0, "nothing until the release");
    harness.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: alt,
    });
    harness.run_steps(3);
    assert_eq!(harness.state().session.relationships().len(), 1, "the Mate was placed");
}

/// Ctrl+C then Ctrl+V pastes a copy of the selected body that hangs on the cursor; a click sets it down
/// (shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[test]
fn ctrl_c_and_ctrl_v_paste_a_copy_of_the_selected_body() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let app = harness.state_mut();
    let body =
        app.session.add_body(gc_doc::Solid::Box { size: [0.1; 3] }, gc_doc::Placement::default()).unwrap();
    app.session.select_item(Some(gc_session::CellItem::Body(body)));
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::C);
    harness.run_steps(2);
    assert_eq!(harness.state().session.clipboard().len(), 1, "Ctrl+C remembered the component");
    let bodies =
        |h: &egui_kittest::Harness<'_, GraphCtrlApp>| h.state().session.doc().bodies().iter().count();
    assert_eq!(bodies(&harness), 1);
    // Ctrl+V makes the copy; Esc takes the paste back.
    let over_view = egui::pos2(500.0, 400.0);
    harness.event(egui::Event::PointerMoved(over_view));
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::V);
    harness.run_steps(3);
    assert_eq!(bodies(&harness), 1, "nothing is made before the click");
    assert!(harness.state().viewport().is_placing(), "the paste waits for its place");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    assert_eq!(bodies(&harness), 1, "Esc pasted nothing");
    assert!(!harness.state().viewport().is_placing());
    // Pasted again, a click pastes where the cursor is: one undo step.
    let depth = harness.state().session.history().undo_depth();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::V);
    harness.run_steps(3);
    let target = egui::pos2(700.0, 500.0);
    harness.event(egui::Event::PointerMoved(target));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
    assert_eq!(bodies(&harness), 2);
    assert!(!harness.state().viewport().is_placing(), "set down");
    assert_eq!(harness.state().session.history().undo_depth(), depth + 1, "one undo step");
    let moved =
        harness.state().session.doc().bodies().iter().filter(|(_, b)| b.placement.xyz != [0.0; 3]).count();
    assert_eq!(moved, 1, "the copy stands where it was set down");
}

/// F10 shows or hides the sketch (shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E).
#[test]
fn f10_toggles_the_visibility_of_the_latest_sketch() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let sketch = harness.state_mut().session.create_sketch("XY").unwrap();
    harness.state_mut().session.finish_sketch();
    harness.run();
    assert!(harness.state().session.is_visible(sketch.erase()));
    harness.key_press(egui::Key::F10);
    harness.run_steps(2);
    assert!(!harness.state().session.is_visible(sketch.erase()), "hidden");
    harness.key_press(egui::Key::F10);
    harness.run_steps(2);
    assert!(harness.state().session.is_visible(sketch.erase()), "shown again");
}

/// The Constrain dialog's More section: a name, Maximum / Minimum, Use Offset As Resting Position (Inventor
/// 2026 help, GUID-0F500089-4F69-41C5-AFE8-B58AC9E8E15E).
#[test]
fn the_constrain_dialogs_more_section_sets_the_name_and_limits() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    let fixed = app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let placement = gc_doc::Placement { xyz: [0.12, 0.0, 0.0], rpy: [0.0; 3] };
    let moving = app.session.add_body(solid, placement).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();

    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("구속").click();
    harness.run_steps(5);
    let click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, world: gc_math::DVec3| {
        let at = harness.state().viewport().screen_of(world).unwrap();
        harness.event(egui::Event::PointerMoved(at));
        harness.run_steps(3);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run_steps(3);
        }
    };
    // First the Z axis that moves, then the one that stays.
    click(&mut harness, gc_math::DVec3::new(0.12, 0.0, 0.06));
    harness.get_by_label_contains("첫 번째 선택: Body2:1 · 축");
    click(&mut harness, gc_math::DVec3::new(0.0, 0.0, 0.06));
    harness.get_by_label_contains("두 번째 선택: Body1:1 · 축");
    harness.get_by_label("더 보기").click();
    harness.run_steps(3);
    let input = egui::accesskit::Role::TextInput;
    harness.get_by_role_and_label(input, "이름").click();
    harness.run_steps(2);
    harness.get_by_role_and_label(input, "이름").type_text("Stop");
    harness.get_by_role_and_label(egui::accesskit::Role::CheckBox, "최대").click();
    harness.run_steps(2);
    harness.get_by_role_and_label(input, "최대").click();
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_role_and_label(input, "최대").type_text("10");
    harness.run_steps(2);
    harness.get_by_label("오프셋을 휴지 위치로").click();
    harness.run_steps(2);
    harness.get_by_label("확인").click();
    harness.run();
    let session = &harness.state().session;
    let (_, relationship) = session.relationships().into_iter().next().expect("a constraint");
    assert_eq!(relationship.name, "Stop");
    let limits = relationship.limits.expect("limits");
    assert!(limits.min.is_none() && limits.resting, "{limits:?}");
    assert!((limits.max.unwrap() - 0.01).abs() < 1e-9, "{limits:?}");
    let _ = (fixed, moving);
}

/// The Limits tab of the Joint dialog sets the limits of a joint as it is created (Inventor 2026 help,
/// GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6).
#[test]
fn the_joint_dialogs_limits_tab_goes_with_a_new_rotational_joint() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let placement = gc_doc::Placement { xyz: [0.12, 0.0, 0.0], rpy: [0.0; 3] };
    let moving = app.session.add_body(solid, placement).unwrap();
    app.fit_view();
    app.viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();

    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("조인트").click();
    harness.run_steps(5);
    let click = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, world: gc_math::DVec3| {
        let at = harness.state().viewport().screen_of(world).unwrap();
        harness.event(egui::Event::PointerMoved(at));
        harness.run_steps(3);
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run_steps(3);
        }
    };
    click(&mut harness, gc_math::DVec3::new(0.12, 0.0, 0.0));
    harness.get_by_label_contains("첫 번째 원점: Body2:1");
    click(&mut harness, gc_math::DVec3::new(0.0, 0.0, 0.0));
    harness.get_by_label_contains("두 번째 원점: Body1:1");
    harness.get_by_label("회전").click();
    harness.run_steps(2);
    harness.get_by_label("한계").click();
    harness.run_steps(3);
    harness.get_by_label("각도 한계").click();
    harness.run_steps(2);
    let input = egui::accesskit::Role::TextInput;
    harness.get_by_role_and_label(input, "끝").click();
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_role_and_label(input, "끝").type_text("90");
    harness.run_steps(2);
    harness.get_by_label("확인").click();
    harness.run();
    let session = &harness.state().session;
    assert_eq!(session.relationships().len(), 1, "{:?}", session.last_error());
    let (id, _) = session.relationships().into_iter().next().unwrap();
    let (angular, linear) = session.joint_limits(id);
    let range = angular.expect("angular limits came with the joint");
    assert!(linear.is_none());
    assert!((range.end - 90f64.to_radians()).abs() < 1e-9, "{range:?}");
    let _ = moving;
}

/// A file with an unusable joint range opens; the status bar says what was dropped (Session::load_warnings).
#[test]
fn opening_a_file_with_a_damaged_joint_range_reports_the_warning() {
    let mut s = gc_session::Session::new();
    let frame = |z: f64, nz: f64| gc_doc::Geometry::Frame {
        origin: [0.0, 0.0, z],
        z: [0.0, 0.0, nz],
        x: [1.0, 0.0, 0.0],
    };
    let sel = |id: gc_core::AnyId, geometry| gc_doc::Selection {
        occurrence: gc_doc::Occurrence::Item(id),
        geometry,
    };
    let table = s
        .add_body(
            gc_doc::Solid::Box { size: [2.0, 1.0, 1.0] },
            gc_doc::Placement { xyz: [0.0, 0.0, 0.5], rpy: [0.0; 3] },
        )
        .unwrap();
    let block = s
        .add_body(
            gc_doc::Solid::Box { size: [0.2; 3] },
            gc_doc::Placement { xyz: [0.3, 0.2, 1.6], rpy: [0.0; 3] },
        )
        .unwrap();
    let hinge = s
        .add_relationship(gc_doc::Relationship::new(
            "Hinge",
            gc_doc::RelationKind::Rotational { flip: false },
            sel(block.erase(), frame(-0.1, -1.0)),
            sel(table.erase(), frame(0.5, 1.0)),
        ))
        .unwrap();
    s.set_joint_limits(hinge, Some(gc_doc::JointRange { start: -0.123, end: 0.456, current: None }), None)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cell.gcp");
    s.save_as(&path).unwrap();
    // Start past End: the range cannot be used.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("-0.123"), "the range is in the file");
    std::fs::write(&path, text.replacen("-0.123", "9.0", 1)).unwrap();

    let mut harness =
        egui_kittest::Harness::builder().with_size(SIZE).with_pixels_per_point(1.0).build_eframe(|cc| {
            GraphCtrlApp::new(cc, RunOptions::default()).with_dialogs(Box::new(Scripted(path.clone())))
        });
    harness.run();
    harness.get_by_label("파일").click();
    harness.run();
    harness.get_by_label("열기...").click();
    harness.run_steps(5);
    assert_eq!(harness.state().session.load_warnings().len(), 1);
    harness.get_by_label_contains("열었지만 쓸 수 없는 데이터 1건을 버렸습니다");
}
