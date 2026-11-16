//! Shell smoke tests through egui_kittest.

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// The app must work without any GPU: tools, panels and commands do not depend on the renderer.
#[test]
fn shell_runs_without_gpu() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness.get_by_label("파일");
    harness.get_by_label("한글 입력 시험");
}

/// With a GPU the offscreen viewport is rendered and shown; a PNG is written as stage evidence.
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
    harness.get_by_label("GPU: 사용 중");
    let image = harness.render().unwrap();
    assert_eq!(image.dimensions(), (1280, 800));
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/evidence");
    std::fs::create_dir_all(&out).unwrap();
    image.save(out.join("stage0-shell.png")).unwrap();
    // The viewport area (right of the test panel) must show the cleared target colour, not black.
    let px = image.get_pixel(900, 400);
    assert!(px[0] > 200 && px[1] > 200 && px[2] > 200, "viewport pixel was {px:?}");
}
