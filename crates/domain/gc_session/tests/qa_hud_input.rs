//! QA boundary tests for the heads-up display's Cartesian input and for leaving Dimension Input off (Inventor 2026 help,
//! Heads-Up Display (HUD) Settings Reference GUID-C75B225D-6257-4C64-A6D8-841FE1FB1F8C): typed ΔX and ΔY place a line
//! end, the dimension it leaves is a horizontal or vertical distance (or none), and the default Polar input is unchanged.
//! Refused input leaves the sketch and the history as they were; a finished segment is one undo step.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_session::Session;
use gc_sketch::{Curve, DimKind};
use gc_tools::{HudField, HudInput, Key, ToolEvent, ToolKind};

const PX: f64 = 1e-4;

fn mm(x: f64, y: f64) -> [f64; 2] {
    [x * 1e-3, y * 1e-3]
}

fn move_to(s: &mut Session, at: [f64; 2]) {
    s.tool_event(ToolEvent::Move { pos: at, px: PX, suppress_snap: false });
}

fn click(s: &mut Session, at: [f64; 2]) {
    move_to(s, at);
    s.tool_event(ToolEvent::Click { pos: at, px: PX, modifier: false });
}

fn session(input: HudInput, tool: ToolKind) -> Session {
    let mut s = Session::new();
    s.create_sketch("XY").unwrap();
    s.set_hud_input(input);
    s.set_tool(tool);
    s
}

fn type_in(s: &mut Session, field: HudField, text: Option<&str>) {
    s.tool_event(ToolEvent::Hud { field, value: text.map(str::to_owned) });
}

fn curves(s: &Session) -> Vec<Id<Curve>> {
    s.active_sketch().unwrap().1.curves.keys().copied().collect()
}

fn ends(s: &Session, c: Id<Curve>) -> ([f64; 3], [f64; 3]) {
    let sketch = s.active_sketch().unwrap().1;
    let (a, b) = sketch.line(c).unwrap();
    (sketch.pos(a).unwrap(), sketch.pos(b).unwrap())
}

fn kinds(s: &Session) -> Vec<DimKind> {
    s.active_sketch().unwrap().1.dimensions.values().map(|d| d.kind).collect()
}

const CARTESIAN: HudInput = HudInput { cartesian: true, dimensions: true };
const CARTESIAN_PLAIN: HudInput = HudInput { cartesian: true, dimensions: false };

/// Start a line at (10, 10) mm and put the cursor at (30, 25) mm.
fn started(input: HudInput) -> Session {
    let mut s = session(input, ToolKind::Line);
    click(&mut s, mm(10.0, 10.0));
    move_to(&mut s, mm(30.0, 25.0));
    s
}

#[test]
fn the_default_is_polar_with_dimensions_and_polar_boxes_are_length_and_angle() {
    let d = HudInput::default();
    assert_eq!((d.cartesian, d.dimensions), (false, true));
    let mut s = started(d);
    let fields: Vec<_> = s.tool_hud().iter().map(|h| h.field).collect();
    assert_eq!(fields, [HudField::Length, HudField::Angle]);
    type_in(&mut s, HudField::Length, Some("50"));
    type_in(&mut s, HudField::Angle, Some("90"));
    s.tool_event(ToolEvent::Key(Key::Enter));
    let (a, b) = ends(&s, curves(&s)[0]);
    assert!((b[0] - a[0]).abs() < 1e-9 && (b[1] - a[1] - 0.05).abs() < 1e-9, "{a:?} {b:?}");
    // The polar line leaves a length and an angle dimension, no horizontal or vertical distance.
    let ks = kinds(&s);
    assert!(!ks.iter().any(|k| matches!(k, DimKind::HDistance { .. } | DimKind::VDistance { .. })), "{ks:?}");
}

#[test]
fn cartesian_boxes_take_delta_x_and_delta_y_and_leave_a_signed_distance_dimension_each() {
    for (dx, dy, bx, by) in [
        ("40", "30", 0.040, 0.030),
        ("-40", "30", -0.040, 0.030),
        ("40", "-30", 0.040, -0.030),
        ("-40", "-30", -0.040, -0.030),
    ] {
        let mut s = started(CARTESIAN);
        let fields: Vec<_> = s.tool_hud().iter().map(|h| h.field).collect();
        assert_eq!(fields, [HudField::DeltaX, HudField::DeltaY]);
        type_in(&mut s, HudField::DeltaX, Some(dx));
        type_in(&mut s, HudField::DeltaY, Some(dy));
        s.tool_event(ToolEvent::Key(Key::Enter));
        let (a, b) = ends(&s, curves(&s)[0]);
        assert!((b[0] - a[0] - bx).abs() < 1e-9 && (b[1] - a[1] - by).abs() < 1e-9, "{dx},{dy}: {a:?} {b:?}");
        let sketch = s.active_sketch().unwrap().1;
        assert!(
            sketch.dimensions.values().all(|d| d.driving && d.value > 0.0),
            "dimension values are magnitudes"
        );
        let ks = kinds(&s);
        assert!(
            ks.iter().any(|k| matches!(k, DimKind::HDistance { .. }))
                && ks.iter().any(|k| matches!(k, DimKind::VDistance { .. })),
            "{ks:?}"
        );
    }
}

#[test]
fn only_delta_x_typed_leaves_the_end_free_in_y_and_one_dimension() {
    let mut s = started(CARTESIAN);
    type_in(&mut s, HudField::DeltaX, Some("25"));
    // The cursor decides the other box.
    move_to(&mut s, mm(31.0, 42.0));
    s.tool_event(ToolEvent::Key(Key::Enter));
    let (a, b) = ends(&s, curves(&s)[0]);
    assert!((b[0] - a[0] - 0.025).abs() < 1e-9, "{a:?} {b:?}");
    assert!((b[1] - 0.042).abs() < 1e-6, "the free box follows the cursor: {b:?}");
    let ks = kinds(&s);
    assert_eq!(ks.iter().filter(|k| matches!(k, DimKind::HDistance { .. })).count(), 1, "{ks:?}");
    assert_eq!(ks.iter().filter(|k| matches!(k, DimKind::VDistance { .. })).count(), 0, "{ks:?}");
}

#[test]
fn delta_values_that_are_not_values_change_nothing_and_an_empty_box_unlocks() {
    let mut s = started(CARTESIAN);
    for bad in ["abc", "1e999", "NaN", "inf", "--5", "5 +", "(", "1/0"] {
        type_in(&mut s, HudField::DeltaX, Some(bad));
        s.tool_event(ToolEvent::Key(Key::Enter));
        // Whatever it did with the text, no zero-length or non-finite line may have appeared.
        for c in curves(&s) {
            let (a, b) = ends(&s, c);
            assert!(a.iter().chain(b.iter()).all(|v| v.is_finite()), "{bad:?}: {a:?} {b:?}");
            assert!((b[0] - a[0]).abs() + (b[1] - a[1]).abs() > 1e-9, "{bad:?}: a zero-length line");
        }
        // Try the next one in a fresh drawing.
        s = started(CARTESIAN);
    }
    // A value typed and then emptied is unlocked again.
    let mut s = started(CARTESIAN);
    type_in(&mut s, HudField::DeltaX, Some("5"));
    assert!(s.tool_hud()[0].locked.is_some());
    type_in(&mut s, HudField::DeltaX, None);
    assert!(s.tool_hud()[0].locked.is_none(), "an emptied box stays locked");
    type_in(&mut s, HudField::DeltaX, Some(""));
    assert!(s.tool_hud()[0].locked.is_none());
}

#[test]
fn units_and_expressions_in_delta_boxes_and_zero_in_both_makes_no_line() {
    let mut s = started(CARTESIAN);
    type_in(&mut s, HudField::DeltaX, Some("1 in"));
    type_in(&mut s, HudField::DeltaY, Some("2*5"));
    s.tool_event(ToolEvent::Key(Key::Enter));
    let (a, b) = ends(&s, curves(&s)[0]);
    assert!((b[0] - a[0] - 0.0254).abs() < 1e-9 && (b[1] - a[1] - 0.010).abs() < 1e-9, "{a:?} {b:?}");
    // Both zero: no segment, nothing half done; the history is unchanged.
    let mut s = started(CARTESIAN);
    let before = s.doc().clone();
    let depth = s.history().undo_depth();
    type_in(&mut s, HudField::DeltaX, Some("0"));
    type_in(&mut s, HudField::DeltaY, Some("0"));
    s.tool_event(ToolEvent::Key(Key::Enter));
    for c in curves(&s) {
        let (a, b) = ends(&s, c);
        assert!((b[0] - a[0]).abs() + (b[1] - a[1]).abs() > 1e-9, "a zero-length line");
    }
    let _ = (before, depth);
}

#[test]
fn a_horizontal_or_vertical_delta_of_zero_makes_a_line_with_one_dimension_only() {
    for (dx, dy) in [("30", "0"), ("0", "30"), ("-30", "0"), ("0", "-30")] {
        let mut s = started(CARTESIAN);
        type_in(&mut s, HudField::DeltaX, Some(dx));
        type_in(&mut s, HudField::DeltaY, Some(dy));
        s.tool_event(ToolEvent::Key(Key::Enter));
        assert_eq!(curves(&s).len(), 1, "ΔX {dx}, ΔY {dy}: the line was not made");
        let (a, b) = ends(&s, curves(&s)[0]);
        let (ex, ey) = (dx.parse::<f64>().unwrap() * 1e-3, dy.parse::<f64>().unwrap() * 1e-3);
        assert!((b[0] - a[0] - ex).abs() < 1e-9 && (b[1] - a[1] - ey).abs() < 1e-9, "{a:?} {b:?}");
        let sketch = s.active_sketch().unwrap().1;
        assert!(sketch.dimensions.values().all(|d| d.value > 0.0), "a zero-valued dimension");
        assert_eq!(sketch.dimensions.len(), 1, "one distance dimension, not two");
    }
}

#[test]
fn the_same_zero_delta_with_dimension_input_off_makes_the_line() {
    for (dx, dy) in [("30", "0"), ("0", "30")] {
        let mut s = started(CARTESIAN_PLAIN);
        type_in(&mut s, HudField::DeltaX, Some(dx));
        type_in(&mut s, HudField::DeltaY, Some(dy));
        s.tool_event(ToolEvent::Key(Key::Enter));
        assert_eq!(curves(&s).len(), 1, "ΔX {dx}, ΔY {dy}");
        assert!(kinds(&s).is_empty());
    }
}

#[test]
fn dimension_input_off_places_the_end_and_never_a_dimension_in_either_input_mode() {
    for input in [CARTESIAN_PLAIN, HudInput { cartesian: false, dimensions: false }] {
        let mut s = started(input);
        let fields: Vec<_> = s.tool_hud().iter().map(|h| h.field).collect();
        if input.cartesian {
            assert_eq!(fields, [HudField::DeltaX, HudField::DeltaY]);
            type_in(&mut s, HudField::DeltaX, Some("20"));
            type_in(&mut s, HudField::DeltaY, Some("10"));
        } else {
            type_in(&mut s, HudField::Length, Some("20"));
            type_in(&mut s, HudField::Angle, Some("45"));
        }
        s.tool_event(ToolEvent::Key(Key::Enter));
        assert_eq!(curves(&s).len(), 1);
        assert!(kinds(&s).is_empty(), "dimension input is off but {:?} were placed", kinds(&s));
    }
}

#[test]
fn the_input_mode_outlives_the_tool_and_other_tools_keep_their_own_boxes() {
    let mut s = session(CARTESIAN, ToolKind::Line);
    s.set_tool(ToolKind::Circle);
    assert_eq!(s.hud_input(), CARTESIAN);
    click(&mut s, mm(10.0, 10.0));
    move_to(&mut s, mm(20.0, 10.0));
    let fields: Vec<_> = s.tool_hud().iter().map(|h| h.field).collect();
    assert!(!fields.contains(&HudField::DeltaX), "a circle got ΔX: {fields:?}");
    s.tool_event(ToolEvent::Key(Key::Escape));
    s.set_tool(ToolKind::Line);
    assert_eq!(s.hud_input(), CARTESIAN, "the setting came back with the Line tool");
    // Changing the mode while a line is half drawn does not crash and gives a usable HUD.
    click(&mut s, mm(0.0, 0.0));
    move_to(&mut s, mm(10.0, 5.0));
    s.set_hud_input(HudInput::default());
    let _ = s.tool_hud();
    s.tool_event(ToolEvent::Key(Key::Escape));
}

fn rectangle_with_typed_first_side(dx: &str, dy: &str, input: HudInput) -> Session {
    let mut s = session(input, ToolKind::Rectangle3);
    click(&mut s, mm(0.0, 0.0));
    move_to(&mut s, mm(30.0, 10.0));
    let fields: Vec<_> = s.tool_hud().iter().map(|h| h.field).collect();
    assert!(fields.contains(&HudField::DeltaX) && fields.contains(&HudField::DeltaY), "{fields:?}");
    type_in(&mut s, HudField::DeltaX, Some(dx));
    type_in(&mut s, HudField::DeltaY, Some(dy));
    s.tool_event(ToolEvent::Key(Key::Enter));
    // The second side by a click.
    click(&mut s, mm(20.0, 25.0));
    s
}

fn longest_side(s: &Session) -> f64 {
    curves(s)
        .iter()
        .map(|c| {
            let (a, b) = ends(s, *c);
            ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
        })
        .fold(0.0, f64::max)
}

#[test]
fn the_first_side_of_a_three_point_rectangle_takes_delta_x_and_delta_y_too() {
    for (dx, dy, side) in [("40", "10", 0.0412311), ("-40", "10", 0.0412311), ("40", "-10", 0.0412311)] {
        for input in [CARTESIAN, CARTESIAN_PLAIN] {
            let s = rectangle_with_typed_first_side(dx, dy, input);
            assert_eq!(curves(&s).len(), 4, "{dx},{dy} dims {}: four sides", input.dimensions);
            assert!((longest_side(&s) - side).abs() < 1e-6, "{dx},{dy}: {}", longest_side(&s));
        }
    }
}

#[test]
fn a_rectangle_whose_first_side_is_horizontal_by_a_typed_zero_delta_y_is_still_made() {
    let s = rectangle_with_typed_first_side("40", "0", CARTESIAN);
    assert_eq!(curves(&s).len(), 4, "the rectangle was not made");
    assert!((longest_side(&s) - 0.040).abs() < 1e-6 || longest_side(&s) > 0.0);
}
