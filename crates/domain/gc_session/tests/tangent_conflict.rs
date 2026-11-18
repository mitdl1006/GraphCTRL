//! A tangent between a fixed line and a fixed circle or ellipse it crosses cannot hold: refused as a
//! conflict, with no trace (QA observation).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_core::Id;
use gc_doc::{Command, Created, EndSnap, SketchOp};
use gc_session::Session;
use gc_sketch::{Curve, GeoConstraint};

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

#[test]
fn a_fixed_line_across_a_fixed_curve_cannot_be_made_tangent() {
    for ellipse in [false, true] {
        let mut s = Session::new();
        let k = s.create_sketch("XY").unwrap();
        let round = if ellipse {
            curve(
                &mut s,
                k,
                SketchOp::AddEllipse {
                    centre: [0.0; 3],
                    major: [0.04, 0.0, 0.0],
                    minor: 0.02,
                    centre_snap: EndSnap::default(),
                },
            )
        } else {
            curve(
                &mut s,
                k,
                SketchOp::AddCircle {
                    centre: [0.0; 3],
                    radius: 0.02,
                    centre_snap: EndSnap::default(),
                    rim_snap: EndSnap::default(),
                },
            )
        };
        run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(round))).unwrap();
        let cross = curve(
            &mut s,
            k,
            SketchOp::AddLine {
                a: [-0.06, 0.005, 0.0],
                b: [0.06, 0.005, 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        );
        run(&mut s, k, SketchOp::AddConstraint(GeoConstraint::FixCurve(cross))).unwrap();
        let before = s.doc().clone();
        let result = run(
            &mut s,
            k,
            SketchOp::AddConstraint(GeoConstraint::Tangent { a: cross, b: round, side: 1.0, external: true }),
        );
        let report = s.active_sketch().unwrap().1.clone().solve();
        assert!(result.is_err(), "ellipse {ellipse}: accepted; {report:?}");
        assert!(s.doc().same_content(&before));
    }
}
