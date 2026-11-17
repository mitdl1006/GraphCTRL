//! The application shell through egui_kittest: it must work without a GPU, and with one it is rendered to
//! PNG files that serve as evidence of what the user sees.

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
    let mut shot = |harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, name: &str| {
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
