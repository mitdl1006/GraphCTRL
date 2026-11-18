//! The Spline (Interpolation) tool: click fit points; Enter or a second click on the last one finishes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_session::Session;
use gc_sketch::CurveGeom;
use gc_tools::{Key, Prompt, ToolEvent, ToolKind};

const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn click(s: &mut Session, at: [f64; 2]) {
    s.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: true });
    s.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: true });
}

fn splines(s: &Session) -> Vec<usize> {
    s.active_sketch()
        .unwrap()
        .1
        .curves
        .values()
        .filter_map(|c| if let CurveGeom::Spline { points } = &c.geom { Some(points.len()) } else { None })
        .collect()
}

#[test]
fn clicks_make_fit_points_and_enter_or_a_repeat_click_finishes() {
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    s.set_tool(ToolKind::Spline);
    assert_eq!(s.prompt(), Prompt::SplineFirst);
    click(&mut s, mm(0.0, 0.0));
    assert_eq!(s.prompt(), Prompt::SplineNext);
    click(&mut s, mm(10.0, 8.0));
    click(&mut s, mm(20.0, -4.0));
    s.tool_event(ToolEvent::Move { pos: mm(30.0, 2.0), px: PX, suppress_snap: true });
    assert!(s.preview().lines.len() > 20, "the curve through the points and the cursor");
    s.tool_event(ToolEvent::Key(Key::Enter));
    assert_eq!(splines(&s), vec![3]);
    assert_eq!(s.prompt(), Prompt::SplineFirst);
    // A second click on the last point finishes too; one undo removes the spline.
    click(&mut s, mm(0.0, 20.0));
    click(&mut s, mm(15.0, 25.0));
    click(&mut s, mm(15.0, 25.0));
    assert_eq!(splines(&s).len(), 2);
    s.undo();
    assert_eq!(splines(&s), vec![3]);
    // Enter with one point does nothing; Escape drops it.
    click(&mut s, mm(40.0, 40.0));
    s.tool_event(ToolEvent::Key(Key::Enter));
    assert_eq!(splines(&s), vec![3]);
    s.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(s.prompt(), Prompt::SplineFirst);
}
