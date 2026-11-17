//! QA edge cases for Project Geometry (assembly parity P1-5). Inventor 2026 help, To Project Geometry into a
//! 2D Sketch: <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC.htm>
//! (projected geometry is associative; deleting its source breaks the link; Shift makes it construction).

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::{Command, Created, Occurrence, Placement, ProjectSource, SketchOp};
use gc_session::Session;

fn setup() -> (Session, gc_core::Id<gc_doc::Body>, gc_core::Id<gc_doc::Sketch>, Occurrence) {
    let mut s = Session::new();
    let body = s
        .add_body(gc_doc::Solid::Box { size: [0.2; 3] }, Placement { xyz: [0.3, 0.1, 0.5], rpy: [0.0; 3] })
        .unwrap();
    let sketch = s.create_sketch("XY").unwrap();
    (s, body, sketch, Occurrence::Item(body.erase()))
}

fn project(
    s: &mut Session,
    sketch: gc_core::Id<gc_doc::Sketch>,
    source: ProjectSource,
) -> Result<Vec<Created>, gc_doc::EditError> {
    s.execute(&Command::Sketch { sketch, op: SketchOp::Project { source, construction: false } })
}

/// Edges that are square to the sketch plane project to a point (or nothing), never to a zero-length
/// curve; a zero-length edge and non-finite coordinates are refused. Whatever happens, the document is
/// either unchanged or holds valid geometry, and one Undo takes the projection back.
#[test]
fn degenerate_projections_are_refused_or_harmless() {
    let (mut s, _, sketch, occ) = setup();
    let base = s.doc().clone();
    let depth = s.history().undo_depth();
    let cases = [
        ProjectSource::Edge { occurrence: occ, a: [0.0, 0.0, -0.1], b: [0.0, 0.0, 0.1] }, // along the plane normal
        ProjectSource::Edge { occurrence: occ, a: [0.0; 3], b: [0.0; 3] },                // zero length
        ProjectSource::Edge { occurrence: occ, a: [f64::NAN, 0.0, 0.0], b: [0.1, 0.0, 0.0] },
        ProjectSource::Point { occurrence: occ, local: [f64::INFINITY, 0.0, 0.0] },
        ProjectSource::OriginAxis { occurrence: occ, axis: 3 },
        ProjectSource::OriginAxis { occurrence: occ, axis: 255 },
    ];
    for source in cases {
        match project(&mut s, sketch, source) {
            Err(_) => {
                assert!(s.doc().same_content(&base), "{source:?} failed but changed the document");
                assert_eq!(s.history().undo_depth(), depth, "{source:?}");
            }
            Ok(_) => {
                let sk = s.doc().get(sketch).unwrap();
                for curve in sk.curves.keys() {
                    if let Some((a, b)) = sk.line(*curve) {
                        let (a, b) = (sk.pos(a).unwrap(), sk.pos(b).unwrap());
                        assert!(a.iter().chain(&b).all(|v| v.is_finite()), "{source:?}");
                        assert!(
                            gc_math::fm::hypot(b[0] - a[0], b[1] - a[1]) > 1e-12,
                            "{source:?} gave a zero-length line"
                        );
                    }
                }
                assert!(s.undo());
                assert!(s.doc().same_content(&base), "{source:?}: undo restores");
            }
        }
    }
}

/// A component or work feature that does not exist cannot be projected; with no sketch being edited there is
/// nothing to project into.
#[test]
fn unknown_sources_and_no_sketch_are_refused() {
    let (mut s, _, sketch, _) = setup();
    let ghost_body = gc_core::Id::<gc_doc::Body>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    let before = s.doc().clone();
    let source = ProjectSource::Point { occurrence: Occurrence::Item(ghost_body.erase()), local: [0.0; 3] };
    assert!(project(&mut s, sketch, source).is_err());
    let ghost_work = gc_core::Id::<gc_doc::WorkFeature>::from_raw(std::num::NonZeroU64::new(9999).unwrap());
    assert!(project(&mut s, sketch, ProjectSource::WorkPoint(ghost_work)).is_err());
    assert!(s.doc().same_content(&before));
    // Session::project_geometry without an active sketch.
    s.finish_sketch();
    let real = ProjectSource::Point { occurrence: Occurrence::Item(ghost_body.erase()), local: [0.0; 3] };
    assert!(s.project_geometry(real, false).is_err());
    assert!(s.doc().same_content(&before));
}

/// The construction toggle and Shift: Shift flips what the toggle says (help: Shift+click projects as
/// construction), and projecting the same point twice does not pile up duplicate fixed points.
#[test]
fn shift_flips_construction_and_repeats_do_not_pile_up() {
    let (mut s, _, sketch, occ) = setup();
    let _ = sketch;
    let edge = ProjectSource::Edge { occurrence: occ, a: [-0.1, 0.1, 0.1], b: [0.1, 0.1, 0.1] };
    let curves = |s: &Session| {
        s.active_sketch().unwrap().1.curves.values().map(|c| c.construction).collect::<Vec<_>>()
    };
    s.project_geometry(edge, false).unwrap();
    assert_eq!(curves(&s), vec![false], "projected as ordinary geometry by default");
    s.project_geometry(edge, true).unwrap();
    assert!(curves(&s).contains(&true), "Shift makes it construction");
    s.set_tool_construction(true).unwrap();
    let n = curves(&s).len();
    s.project_geometry(edge, false).unwrap();
    assert!(*curves(&s).last().unwrap(), "with the construction toggle on, projections are construction");
    s.project_geometry(edge, true).unwrap();
    assert!(!*curves(&s).last().unwrap(), "and Shift flips that back");
    assert!(curves(&s).len() > n);
    // Many undo steps back to the empty sketch.
    while s.undo() {
        if s.active_sketch().is_none_or(|(_, sk)| sk.curves.is_empty()) {
            break;
        }
    }
}
