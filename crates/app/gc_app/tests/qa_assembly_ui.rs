//! Screen-side acceptance of the component origin folder (assembly parity P1-2/3), with the lead's decisions
//! of 2026-11-17: under `원점` the children are, in this order, `YZ 평면`, `XZ 평면`, `XY 평면`, `X축`, `Y축`,
//! `Z축`, `중심점`; hidden elements cannot be picked in the viewport (but can in the browser); once shown they
//! can. Inventor 2026 help, To Display Reference Planes (A53 in docs/ux/inventor-assembly-parity.md).

#![allow(clippy::unwrap_used, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn harness() -> egui_kittest::Harness<'static, GraphCtrlApp> {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness
}

fn click_at(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
}

#[test]
fn the_origin_folder_lists_its_seven_elements_in_the_decided_order() {
    let mut harness = harness();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    harness.query_all_by_label("원점").last().unwrap().click();
    harness.run();
    let names = ["YZ 평면", "XZ 평면", "XY 평면", "X축", "Y축", "Z축", "중심점"];
    let mut last = f32::NEG_INFINITY;
    for name in names {
        // The check box of each element reads "<name> 보이기"; its row has the name.
        let toggle = format!("{name} 보이기");
        let rows: Vec<f32> = harness.query_all_by_label(&toggle).map(|n| n.rect().center().y).collect();
        assert!(!rows.is_empty(), "{toggle} is missing from the origin folder");
        let y = *rows.last().unwrap();
        assert!(y > last, "{name} is out of order (y = {y}, previous {last})");
        last = y;
    }
}

/// A hidden plane is not drawn and not picked: a click where it would be selects nothing in the viewport
/// (the selection stays empty); the same click after showing it picks the plane.
#[test]
fn a_hidden_origin_plane_is_not_picked_in_the_viewport_a_shown_one_is() {
    let mut harness = harness();
    let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
    let body = harness.state_mut().session.add_body(solid, gc_doc::Placement::default()).unwrap();
    harness.run();
    let occurrence = gc_doc::Occurrence::Item(body.erase());
    let plane = gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy);
    harness.state_mut().viewport_mut().camera.pan([0.0, 0.0], [-250.0, 0.0], [1280.0, 700.0]);
    harness.run();
    let at = harness.state().viewport().screen_of(gc_math::DVec3::new(0.09, 0.09, 0.0)).unwrap();
    click_at(&mut harness, at);
    assert!(harness.state().viewport().origin_selected.is_none(), "a hidden plane must not be pickable");
    harness.state_mut().session.set_origin_visible(occurrence, &[plane], true).unwrap();
    harness.run();
    click_at(&mut harness, at);
    assert_eq!(harness.state().viewport().origin_selected, Some((occurrence, plane)));
    // Hiding it again lets go of it or at least stops it being pickable.
    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.state_mut().session.set_origin_visible(occurrence, &[plane], false).unwrap();
    harness.run();
    click_at(&mut harness, at);
    assert!(harness.state().viewport().origin_selected.is_none(), "hidden again, not pickable again");
}
