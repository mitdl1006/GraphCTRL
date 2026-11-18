//! The dimension edit box's `name = value` as one edit (Inventor 2026 help, To Work with Sketch Dimensions,
//! GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185: "enter or edit an expression that defines a parameter. For
//! example, enter HGHT = 5mm").

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "tests")]

use gc_doc::{Command, Created, SketchOp};
use gc_session::Session;
use gc_sketch::DimKind;

#[test]
fn naming_a_value_from_the_dimension_box_is_one_undo_step() {
    let mut s = Session::new();
    let sketch = s.create_sketch("XY").unwrap();
    let origin = *s.active_sketch().unwrap().1.points.keys().next().unwrap();
    let point = match s
        .execute(&Command::Sketch { sketch, op: SketchOp::AddPoint { pos: [0.03, 0.0, 0.0] } })
        .unwrap()[..]
    {
        [Created::Point(p)] => p,
        ref other => panic!("{other:?}"),
    };
    let op = SketchOp::AddDimension {
        kind: DimKind::HDistance { p: origin, q: point, sign: 1.0 },
        source: "30 mm".into(),
        text_pos: [0.0; 2],
    };
    let id = match s.execute(&Command::Sketch { sketch, op }).unwrap()[..] {
        [Created::Dimension(d), ..] => d,
        ref other => panic!("{other:?}"),
    };
    let before = s.doc().clone();
    let steps = s.history().undo_depth();
    s.edit_dimension_with_param(id, "HGHT", "5 mm * 4").unwrap();
    assert_eq!(s.history().undo_depth(), steps + 1);
    let sk = s.active_sketch().unwrap().1;
    assert_eq!(sk.dimensions[&id].source, "HGHT");
    assert!((sk.dimensions[&id].value - 0.02).abs() < 1e-12);
    assert!(s.doc().params().iter().any(|(_, p)| p.name == "HGHT"));
    // One undo takes both back.
    assert!(s.undo());
    assert!(s.doc().same_content(&before));
    // A bad name or a bad value adds nothing.
    for (name, value) in [("1bad", "5 mm"), ("OK", "5 +"), ("", "5 mm")] {
        assert!(s.edit_dimension_with_param(id, name, value).is_err(), "{name:?} = {value:?}");
        assert!(s.doc().same_content(&before), "{name:?} = {value:?} left a trace");
    }
    // A name already used (the parameter exists) is refused whole.
    s.edit_dimension_with_param(id, "HGHT", "10 mm").unwrap();
    let again = s.doc().clone();
    assert!(s.edit_dimension_with_param(id, "HGHT", "12 mm").is_err());
    assert!(s.doc().same_content(&again));
}
