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
    assert!(harness.query_all_by_label("joint_1").count() >= 1, "the jog sliders are shown");

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
    // Somewhere in the viewport the dark and the orange links of the robot are visible.
    let viewport = (220..1000).flat_map(|x| (130..760).map(move |y| (x, y)));
    let (mut dark, mut orange) = (0, 0);
    for (x, y) in viewport {
        let px = image.get_pixel(x, y);
        dark += u32::from(px[0] < 110 && px[1] < 110 && px[2] < 120 && px[0].abs_diff(px[2]) < 40);
        orange += u32::from(px[0] > 150 && px[1] > 60 && px[1] < 150 && px[2] < 80);
    }
    assert!(dark > 500 && orange > 500, "dark {dark}, orange {orange}");

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
    // for all the lines of the trace.
    let stats = harness.state().viewport().background().stats();
    assert_eq!(stats.draw_calls, 22);
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

    // Open the frame in the panel: X axis at 45 degrees, origin at (400, 100) in the world and (400, 200)
    // from the robot's base.
    let header = harness.query_all_by_label("Frame1").last().unwrap();
    header.click();
    harness.run();
    harness.get_by_label("X 400.0  Y 100.0  Z 0.0 mm");
    harness.get_by_label("X 400.0  Y 200.0  Z 0.0 mm");
    assert_eq!(
        harness.query_all_by_label("R 0.00°  P 0.00°  Y 45.00°").count(),
        2,
        "turned 45 degrees about Z"
    );
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
            harness.run();
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
    harness.get_by_label("UR5e_1").click();
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
    harness.get_by_label("숨기기").click();
    harness.run();
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
    harness.get_by_label("3D 스케치").click();
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
    assert_eq!(harness.query_all_by_label("fixture").count(), 2);
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
