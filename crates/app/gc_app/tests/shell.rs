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

    // Playing advances the shown robot along the motion without touching the document.
    let revision = harness.state().session.doc().revision();
    harness.get_by_label("재생").click();
    harness.run_steps(8);
    let (_, time) = harness.state().session.playback().unwrap();
    assert!(time > 0.0);
    harness.get_by_label("정지").click();
    harness.run_steps(2);
    assert_eq!(harness.state().session.doc().revision(), revision);

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

    // Half-way through the motion, in perspective.
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
    // The seven link meshes and the pillar: eight draw calls, thousands of triangles.
    let stats = harness.state().viewport().background().stats();
    assert_eq!(stats.draw_calls, 8);
    assert!(stats.triangles > 7000, "{stats:?}");
    harness.render().unwrap().save(out.join("review-7-ur5e-path.png")).unwrap();
    let app = harness.state_mut();
    let duration = app.session.planned(path).unwrap().duration();
    app.session.set_playback(Some((path, duration * 0.5)));
    harness.run();
    harness.render().unwrap().save(out.join("review-8-ur5e-playback.png")).unwrap();
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
