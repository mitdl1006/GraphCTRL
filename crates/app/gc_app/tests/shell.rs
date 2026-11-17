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

    // The comparison runs every planner and lists them.
    harness.get_by_label("알고리즘 비교").click();
    harness.run();
    let (rows, current) = harness.state().session.comparison(path).unwrap();
    assert!(current && rows.len() == 5 && rows[0].successes == rows[0].runs);
    harness.get_by_label("Informed RRT*");

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
    // The arm as it really looks: twenty coloured parts of the seven links, and the pillar.
    let stats = harness.state().viewport().background().stats();
    assert_eq!(stats.draw_calls, 21);
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
