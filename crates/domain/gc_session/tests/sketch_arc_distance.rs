//! Distances to arcs themselves (Inventor 2026 help, To Work with Sketch Dimensions,
//! GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185, Flip Direction's dimension types: "Distance from a line to an arc
//! (the arc itself, not to the arc center point)", "Distance between 2 arcs").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, DimKind, GeoConstraint};

type K = Id<gc_doc::Sketch>;

fn run(s: &mut Session, k: K, op: SketchOp) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch: k, op })
}

fn curve(s: &mut Session, k: K, op: SketchOp) -> Id<Curve> {
    run(s, k, op)
        .unwrap()
        .iter()
        .find_map(|c| if let Created::Curve(id) = c { Some(*id) } else { None })
        .unwrap()
}

fn circle(s: &mut Session, k: K, c: [f64; 2], r: f64) -> Id<Curve> {
    curve(
        s,
        k,
        SketchOp::AddCircle {
            centre: [c[0], c[1], 0.0],
            radius: r,
            centre_snap: EndSnap::default(),
            rim_snap: EndSnap::default(),
        },
    )
}

fn measure(s: &Session, kind: &DimKind) -> f64 {
    s.active_sketch().unwrap().1.measure(kind).unwrap()
}

fn dimension(s: &mut Session, k: K, kind: DimKind, value: &str) {
    run(s, k, SketchOp::AddDimension { kind, source: value.into(), text_pos: [0.0, 0.0] }).unwrap();
}

#[test]
fn a_line_keeps_a_distance_from_a_circle_itself() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let line = curve(
        &mut s,
        k,
        SketchOp::AddLine {
            a: [-0.1, 0.0, 0.0],
            b: [0.1, 0.0, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: Some(true),
        },
    );
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(line))).unwrap();
    let c = circle(&mut s, k, [0.0, 0.05], 0.02);
    let kind = DimKind::LineArc { line, arc: c, side: 1.0 };
    assert!((measure(&s, &kind) - 0.03).abs() < 1e-9);
    dimension(&mut s, k, kind, "10");
    assert!((measure(&s, &kind) - 0.01).abs() < 1e-9);
    // The radius changes: the gap stays.
    let sk = s.active_sketch().unwrap().1;
    let (centre, r) = sk.round(c).unwrap();
    assert!((sk.pos(centre).unwrap()[1] - (r + 0.01)).abs() < 1e-9);
    // Flip Direction puts the circle on the other side.
    let flipped = gc_doc::flipped(&kind).unwrap();
    assert_eq!(flipped, DimKind::LineArc { line, arc: c, side: -1.0 });
}

#[test]
fn two_circles_keep_a_distance_apart_or_inside() {
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let a = circle(&mut s, k, [0.0, 0.0], 0.02);
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(a))).unwrap();
    let b = circle(&mut s, k, [0.08, 0.0], 0.01);
    let apart = DimKind::ArcArc { a, b, external: true };
    assert!((measure(&s, &apart) - 0.05).abs() < 1e-9);
    dimension(&mut s, k, apart, "15");
    assert!((measure(&s, &apart) - 0.015).abs() < 1e-9);
    // One inside the other.
    let big = circle(&mut s, k, [0.5, 0.0], 0.1);
    run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(big))).unwrap();
    let small = circle(&mut s, k, [0.52, 0.0], 0.03);
    let inside = DimKind::ArcArc { a: big, b: small, external: false };
    assert!((measure(&s, &inside) - 0.05).abs() < 1e-9);
    dimension(&mut s, k, inside, "20");
    assert!((measure(&s, &inside) - 0.02).abs() < 1e-9);
}

#[test]
fn the_dimension_tool_measures_from_a_line_to_a_circle_and_between_circles() {
    use gc_tools::{ToolEvent, ToolKind};
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let _line = curve(
        &mut s,
        k,
        SketchOp::AddLine {
            a: [-0.1, 0.0, 0.0],
            b: [0.1, 0.0, 0.0],
            snap_a: EndSnap::default(),
            snap_b: EndSnap::default(),
            horizontal: Some(true),
        },
    );
    let _c = circle(&mut s, k, [0.0, 0.05], 0.02);
    let _d = circle(&mut s, k, [0.1, 0.05], 0.01);
    s.set_tool(ToolKind::Dimension);
    let px = 1e-4;
    let click = |s: &mut Session, at: [f64; 2]| {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: true });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    };
    // Line, circle, then a place for the text and Enter for the value shown.
    click(&mut s, [0.05, 0.0]);
    click(&mut s, [0.0, 0.07]);
    click(&mut s, [0.03, 0.02]);
    let request = *s.value_request().unwrap();
    assert!((request.measured - 0.03).abs() < 1e-9, "{}", request.measured);
    s.submit_value("25 mm");
    let sk = s.active_sketch().unwrap().1;
    assert!(
        sk.dimensions.values().any(|d| matches!(d.kind, DimKind::LineArc { .. })),
        "{:?}",
        sk.dimensions.values().map(|d| d.kind).collect::<Vec<_>>()
    );
}

/// The dimension tool on two circles (lead's report): clicking the first circle's rim and then the second's
/// gives the distance between them, not a diameter; inside one another too.
#[test]
fn the_dimension_tool_on_two_circle_rims_gives_their_distance() {
    use gc_tools::{ToolEvent, ToolKind};
    for (b_centre, b_radius, external, expected) in
        [([0.1, 0.0], 0.01, true, 0.1 - 0.02 - 0.01), ([0.005, 0.0], 0.005, false, 0.02 - 0.005 - 0.005)]
    {
        let mut s = Session::new();
        let k = s.create_sketch("XY").unwrap();
        let a = circle(&mut s, k, [0.0, 0.0], 0.02);
        let b = circle(&mut s, k, b_centre, b_radius);
        s.set_tool(ToolKind::Dimension);
        let px = 1e-4;
        let click = |s: &mut Session, at: [f64; 2]| {
            s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: false });
            s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
        };
        // The rims, on their tops (away from each other's).
        click(&mut s, [0.0, 0.02]);
        click(&mut s, [b_centre[0], b_radius]);
        click(&mut s, [0.05, 0.06]);
        let request = *s.value_request().unwrap();
        assert_eq!(request.kind, DimKind::ArcArc { a, b, external }, "{:?}", request.kind);
        assert!((request.measured - expected).abs() < 1e-9, "{}", request.measured);
    }
}

/// Two crossing circles have no gap: the tool dimensions the distance between their centres instead of
/// falling back to one circle's diameter.
#[test]
fn the_dimension_tool_on_two_crossing_circles_gives_the_centre_distance() {
    use gc_tools::{ToolEvent, ToolKind};
    let mut s = Session::new();
    let k = s.create_sketch("XY").unwrap();
    let a = circle(&mut s, k, [0.0, 0.0], 0.02);
    let b = circle(&mut s, k, [0.03, 0.0], 0.02);
    s.set_tool(ToolKind::Dimension);
    let px = 1e-4;
    for at in [[0.0, 0.02], [0.03, 0.02], [0.015, 0.05]] {
        s.tool_event(ToolEvent::Move { pos: at, px, suppress_snap: false });
        s.tool_event(ToolEvent::Click { pos: at, px, modifier: false });
    }
    let request = *s.value_request().unwrap();
    let sk = s.active_sketch().unwrap().1;
    let (ca, cb) = (sk.round(a).unwrap().0, sk.round(b).unwrap().0);
    assert_eq!(request.kind, DimKind::Distance { p: ca, q: cb });
    assert!((request.measured - 0.03).abs() < 1e-9);
}
