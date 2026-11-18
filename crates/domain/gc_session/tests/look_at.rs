//! Look At (Inventor 2026 help, Navigation Bar, GUID-3B926737-92BA-46CF-B722-97F68651772B): "zooms and rotates
//! the model to display the selected element planar to the screen or a selected edge or line horizontal to
//! the screen".

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_doc::{
    CellOp, Command, Created, EndSnap, Geometry, Occurrence, OriginPlane, Placement, PlaneRef, PointRef,
    Selection, SketchOp, Solid, WorkFeature, WorkGeometry,
};
use gc_math::DVec3;
use gc_session::{LookItem, Session};

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

fn work(s: &mut Session, geometry: WorkGeometry) -> gc_core::Id<WorkFeature> {
    let feature = WorkFeature { name: "W".into(), geometry, visible: true };
    match s.execute(&Command::Cell(CellOp::AddWorkFeature(feature))).unwrap()[..] {
        [Created::WorkFeature(id)] => id,
        ref other => panic!("{other:?}"),
    }
}

#[test]
fn planes_face_the_screen_from_the_eyes_side() {
    let mut s = Session::new();
    let plane =
        work(&mut s, WorkGeometry::PlaneOffset { base: PlaneRef::Origin(OriginPlane::Xy), distance: 0.3 });
    // Looking down: seen from above. Looking up from below: seen from below.
    let down = s.look_target(LookItem::WorkFeature(plane), DVec3::new(0.2, 0.1, -1.0)).unwrap();
    assert!(near(down.toward_eye, DVec3::Z) && (down.centre.z - 0.3).abs() < 1e-12);
    assert!(down.up.dot(down.toward_eye).abs() < 1e-12);
    let up = s.look_target(LookItem::WorkFeature(plane), DVec3::new(0.0, 0.0, 1.0)).unwrap();
    assert!(near(up.toward_eye, -DVec3::Z));
    // A component's origin plane.
    let b =
        s.add_body(Solid::Box { size: [0.1; 3] }, Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    let side = s
        .look_target(LookItem::ComponentPlane(Occurrence::Item(b.erase()), OriginPlane::Yz), DVec3::NEG_X)
        .unwrap();
    assert!(near(side.toward_eye, DVec3::X) && near(side.centre, DVec3::new(1.0, 0.0, 0.0)));
    // A planar face of the body.
    let face = Selection {
        occurrence: Occurrence::Item(b.erase()),
        geometry: Geometry::Plane { point: [0.0, 0.05, 0.0], normal: [0.0, 1.0, 0.0] },
    };
    let t = s.look_target(LookItem::Face(face), DVec3::new(0.3, -1.0, -0.2)).unwrap();
    assert!(near(t.toward_eye, DVec3::Y) && near(t.up, DVec3::Z));
    // A point has nothing to look at.
    let point = work(&mut s, WorkGeometry::Point { at: PointRef::Fixed([0.0, 0.0, 1.0]) });
    assert!(s.look_target(LookItem::WorkFeature(point), DVec3::NEG_Z).is_none());
}

#[test]
fn lines_lie_level_on_the_screen() {
    let mut s = Session::new();
    // A work axis along X, looked at from a slant: seen from square to it, close to the slant.
    let axis = work(
        &mut s,
        WorkGeometry::AxisThroughPoints { points: [PointRef::Origin, PointRef::Fixed([1.0, 0.0, 0.0])] },
    );
    let t = s.look_target(LookItem::WorkFeature(axis), DVec3::new(-0.5, 0.0, -1.0)).unwrap();
    assert!(t.toward_eye.dot(DVec3::X).abs() < 1e-12 && near(t.toward_eye, DVec3::Z));
    // Level: the line's direction is square to up.
    assert!(t.up.dot(DVec3::X).abs() < 1e-12);
    // A line of a sketch on XY at 30°: seen from above the sketch, lying level.
    let k = s.create_sketch("XY").unwrap();
    let (c, sn) = (3f64.sqrt() / 2.0, 0.5);
    let created = s
        .execute(&Command::Sketch {
            sketch: k,
            op: SketchOp::AddLine {
                a: [0.0, 0.0, 0.0],
                b: [0.1 * c, 0.1 * sn, 0.0],
                snap_a: EndSnap::default(),
                snap_b: EndSnap::default(),
                horizontal: None,
            },
        })
        .unwrap();
    let curve =
        created.iter().find_map(|x| if let Created::Curve(id) = x { Some(*id) } else { None }).unwrap();
    let t = s.look_target(LookItem::SketchCurve { sketch: k, curve }, DVec3::new(0.4, 0.4, -1.0)).unwrap();
    assert!(near(t.toward_eye, DVec3::Z));
    assert!(t.up.dot(DVec3::new(c, sn, 0.0)).abs() < 1e-12, "{:?}", t.up);
    assert!((t.reach.unwrap() - 0.05).abs() < 1e-12);
    // The whole sketch: its plane, centred on what is drawn.
    let t = s.look_target(LookItem::Sketch(k), DVec3::NEG_Z).unwrap();
    assert!(near(t.toward_eye, DVec3::Z) && near(t.centre, DVec3::new(0.05 * c, 0.05 * sn, 0.0)));
}
