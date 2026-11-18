//! The Ellipse tool (Inventor 2026 help, To Create 2D Shape Geometry, Ellipse): click the centre, a point
//! that sets the first axis, then a point on the ellipse.

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

fn ellipses(s: &Session) -> Vec<(f64, f64, f64)> {
    let sk = s.active_sketch().unwrap().1;
    sk.curves
        .values()
        .filter_map(|c| match c.geom {
            CurveGeom::Ellipse { c, major, minor, .. } => {
                let (c, m) = (sk.pos(c).unwrap(), sk.pos(major).unwrap());
                Some((c[0], ((m[0] - c[0]).powi(2) + (m[1] - c[1]).powi(2)).sqrt(), minor))
            }
            CurveGeom::Line { .. }
            | CurveGeom::Circle { .. }
            | CurveGeom::Arc { .. }
            | CurveGeom::Spline { .. } => None,
        })
        .collect()
}

#[test]
fn three_clicks_draw_an_ellipse_with_previews_and_prompts() {
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    s.set_tool(ToolKind::Ellipse);
    assert_eq!(s.prompt(), Prompt::EllipseCentre);
    click(&mut s, mm(10.0, 10.0));
    assert_eq!(s.prompt(), Prompt::EllipseAxis);
    // Between the clicks a line from the centre is previewed.
    s.tool_event(ToolEvent::Move { pos: mm(40.0, 10.0), px: PX, suppress_snap: true });
    assert!(!s.preview().lines.is_empty());
    click(&mut s, mm(40.0, 10.0));
    assert_eq!(s.prompt(), Prompt::EllipseMinor);
    // The ellipse itself is previewed while the third point is chosen.
    s.tool_event(ToolEvent::Move { pos: mm(25.0, 22.0), px: PX, suppress_snap: true });
    assert!(s.preview().lines.len() > 8, "the ellipse's outline");
    click(&mut s, mm(25.0, 22.0));
    let made = ellipses(&s);
    assert_eq!(made.len(), 1);
    let (cx, a, b) = made[0];
    assert!((cx - 0.01).abs() < 1e-9 && (a - 0.03).abs() < 1e-9);
    assert!((b - 0.012).abs() < 1e-9, "the third point's distance from the axis: {b}");
    // One undo removes it; the tool starts over for the next one.
    assert_eq!(s.prompt(), Prompt::EllipseCentre);
    s.undo();
    assert!(ellipses(&s).is_empty());
    // A third point on the axis draws nothing and starts over.
    click(&mut s, mm(0.0, 0.0));
    click(&mut s, mm(20.0, 0.0));
    click(&mut s, mm(10.0, 0.0));
    assert!(ellipses(&s).is_empty());
    assert_eq!(s.prompt(), Prompt::EllipseCentre);
    // Escape drops a half-drawn ellipse (and, with nothing pending, ends the command).
    click(&mut s, mm(0.0, 0.0));
    s.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(s.prompt(), Prompt::EllipseCentre);
    s.tool_event(ToolEvent::Key(Key::Escape));
    assert_eq!(s.prompt(), Prompt::Select);
}
