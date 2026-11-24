//! The user's core flow end to end, with a picture at each step (PNG files in `docs/evidence/img/` when a GPU is
//! there): a sketch's points and lines get roles (right-click menu, browser nodes, role panel), "Create Path" joins
//! two sketch points into a path with a preview, and a role becomes a task that the schedule plays.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};
use gc_tools::{ToolEvent, ToolKind};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
const PX: f64 = 1e-4;

type H = egui_kittest::Harness<'static, GraphCtrlApp>;

struct Flow {
    h: H,
    gpu: bool,
}

fn flow() -> Flow {
    let gpu = gc_test_support::gpu_or_skip("evidence_flow").is_some();
    let builder = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .with_accessibility_check(false);
    let mut h = if gpu {
        builder.wgpu().build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()))
    } else {
        builder.build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()))
    };
    h.run();
    Flow { h, gpu }
}

impl Flow {
    fn shot(&mut self, name: &str) {
        self.h.run_steps(2);
        if !self.gpu {
            return;
        }
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/evidence/img");
        std::fs::create_dir_all(&out).unwrap();
        // The software renderer sometimes times out on a busy frame: try again.
        let image = (0..4).find_map(|_| self.h.render().ok()).unwrap();
        image.save(out.join(name)).unwrap();
    }

    fn click_at(&mut self, at: egui::Pos2, button: egui::PointerButton) {
        self.h.event(egui::Event::PointerMoved(at));
        self.h.run_steps(2);
        for pressed in [true, false] {
            self.h.event(egui::Event::PointerButton {
                pos: at,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            self.h.run_steps(2);
        }
        self.h.run_steps(3);
    }

    fn sketch_screen(&self, p: [f64; 2]) -> egui::Pos2 {
        self.h.state().viewport().screen_of(gc_math::DVec3::new(p[0], p[1], 0.0)).unwrap()
    }

    fn world_screen(&self, p: [f64; 3]) -> egui::Pos2 {
        self.h.state().viewport().screen_of(gc_math::DVec3::from_array(p)).unwrap()
    }
}

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn tool_click(f: &mut Flow, at: [f64; 2]) {
    let app = f.h.state_mut();
    app.session.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
    app.session.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

#[test]
fn roles_then_create_path_then_a_task_from_a_role_that_plays() {
    let mut f = flow();
    // A robot first, so the same cell serves the whole flow.
    f.h.get_by_label("로봇").click();
    f.h.run();
    f.h.get_by_label("로봇 추가").click();
    f.h.run();
    f.h.get_by_label("UR5e 로봇").click();
    f.h.run();
    // A sketch with a polyline, a circle and a lone point (all within the arm's reach).
    f.h.state_mut().session.create_sketch("XY").unwrap();
    f.h.run();
    f.h.state_mut().session.set_tool(ToolKind::Line);
    for p in [mm(300.0, -150.0), mm(400.0, 0.0), mm(300.0, 150.0)] {
        tool_click(&mut f, p);
    }
    f.h.state_mut().session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
    f.h.state_mut().session.set_tool(ToolKind::Circle);
    tool_click(&mut f, mm(500.0, 0.0));
    tool_click(&mut f, mm(530.0, 0.0));
    f.h.state_mut().session.set_tool(ToolKind::Point);
    tool_click(&mut f, mm(250.0, 0.0));
    f.h.state_mut().session.set_tool(ToolKind::Select);
    f.h.run_steps(4);
    f.shot("flow-1-sketch.png");

    // ① Roles from the sketch's right-click menu: a point is a waypoint, the polyline (both lines in one go) a path.
    let sketch = f.h.state().session.active_sketch().unwrap().0;
    let on_point = f.sketch_screen(mm(250.0, 0.0));
    f.click_at(on_point, egui::PointerButton::Secondary);
    f.h.get_by_label("웨이포인트로 지정");
    f.shot("flow-2-point-menu.png");
    f.h.get_by_label("웨이포인트로 지정").click();
    f.h.run_steps(3);
    let on_line = f.sketch_screen(mm(350.0, -75.0));
    f.click_at(on_line, egui::PointerButton::Secondary);
    f.h.get_by_label("경로로 지정");
    f.shot("flow-3-line-menu.png");
    f.h.get_by_label("경로로 지정").click();
    f.h.run_steps(3);
    let on_circle = f.sketch_screen(mm(500.0, 30.0));
    f.click_at(on_circle, egui::PointerButton::Secondary);
    f.h.get_by_label("경로로 지정").click();
    f.h.run_steps(3);
    assert_eq!(
        f.h.state().session.sketch_roles(sketch).len(),
        3,
        "a waypoint, a polyline path and a closed path"
    );
    f.shot("flow-4-roles-in-browser.png");

    // ② Selecting a path role shows its panel (follow, motion, direction, tool attitude).
    let path_role = f.h.state().session.sketch_roles(sketch)[1];
    let path_name = f.h.state().session.doc().get(path_role).unwrap().name.clone();
    f.h.get_by_label(&path_name).click();
    f.h.run_steps(3);
    assert_eq!(f.h.state().viewport().role_selected, Some(path_role));
    f.shot("flow-5-role-panel.png");
    f.h.state_mut().session.finish_sketch();
    f.h.run_steps(3);

    // ③ A task from the waypoint role (browser right-click), then the schedule: work it out and play.
    let waypoint = f.h.state().session.sketch_roles(sketch)[0];
    let name = f.h.state().session.doc().get(waypoint).unwrap().name.clone();
    let row = f.h.get_all_by_label_contains(&name).find(|n| n.rect().min.x < 300.0).unwrap().rect().center();
    f.click_at(row, egui::PointerButton::Secondary);
    f.h.get_by_label("이 역할로 작업 만들기");
    f.shot("flow-6-role-menu-make-task.png");
    f.h.get_by_label("이 역할로 작업 만들기").click();
    f.h.run_steps(3);
    assert_eq!(f.h.state().session.doc().tasks().len(), 1);
    f.h.get_by_label("도구").click();
    f.h.run();
    f.h.get_by_label("스케줄").click();
    f.h.run_steps(3);
    f.h.get_by_label("스케줄 계산").click();
    f.h.run_steps(30);
    let (schedule, current) = f.h.state().session.schedule_result().map(|(s, c)| (s.makespan(), c)).unwrap();
    assert!(current && schedule > 0.0, "the schedule of the role's task is empty ({schedule})");
    f.shot("flow-7-schedule-worked-out.png");
    f.h.get_by_label("스케줄 재생").click();
    f.h.run_steps(20);
    assert!(f.h.state().session.schedule_playback().is_some());
    f.shot("flow-8-schedule-playing.png");
}

#[test]
fn create_path_between_two_sketch_points_with_a_preview() {
    let mut f = flow();
    f.h.get_by_label("로봇").click();
    f.h.run();
    f.h.get_by_label("로봇 추가").click();
    f.h.run();
    f.h.get_by_label("UR5e 로봇").click();
    f.h.run();
    let (a, b) = ([0.45, -0.15, 0.25], [0.45, 0.15, 0.25]);
    {
        let app = f.h.state_mut();
        let robot = app.session.doc().robots().iter().next().unwrap().0;
        app.session.jog(robot, &[0.0, -1.57, 1.57, -1.57, -1.57, 0.0]);
        app.session.end_jog();
        app.session.create_sketch_3d().unwrap();
        app.session.set_tool(ToolKind::Line);
        app.session.spatial_click(a, None, None);
        app.session.spatial_click(b, None, Some(1));
        app.session.tool_event(ToolEvent::Key(gc_tools::Key::Escape));
        app.session.finish_sketch();
    }
    f.h.run_steps(3);
    f.shot("route-1-sketch-points.png");
    let steps = f.h.state().session.history().undo_depth();
    f.h.get_by_label("경로 생성").click();
    f.h.run_steps(3);
    assert!(f.h.state().viewport().is_routing());
    f.shot("route-2-mode-on.png");
    // Hover the first point: it is ringed; click it; a line follows the cursor.
    let pa = f.world_screen(a);
    f.click_at(pa, egui::PointerButton::Primary);
    let mid = f.world_screen([0.45, 0.0, 0.25]);
    f.h.event(egui::Event::PointerMoved(mid));
    f.h.run_steps(3);
    f.shot("route-3-first-point-and-preview.png");
    // Esc cancels; nothing changed.
    f.h.key_press(egui::Key::Escape);
    f.h.run_steps(3);
    assert!(!f.h.state().viewport().is_routing());
    assert_eq!(f.h.state().session.history().undo_depth(), steps);
    // Again, to the second point: the path is made and planned, one undo step.
    f.h.get_by_label("경로 생성").click();
    f.h.run_steps(3);
    f.click_at(pa, egui::PointerButton::Primary);
    let pb = f.world_screen(b);
    f.click_at(pb, egui::PointerButton::Primary);
    f.h.run_steps(40);
    assert_eq!(f.h.state().session.doc().paths().len(), 1);
    assert_eq!(f.h.state().session.history().undo_depth(), steps + 1);
    f.shot("route-4-path-made.png");
    assert!(f.h.state_mut().session.undo());
    assert!(f.h.state().session.doc().paths().is_empty());
    f.shot("route-5-after-undo.png");
}
