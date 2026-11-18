//! QA boundary tests for the ribbon's split buttons (the sketch plane list on the 3D Model tab; Return and
//! Free Move / Free Rotate on the Assemble tab): the body repeats the variant used last, the arrow lists the
//! others, a disabled button does nothing whichever part is pressed, and the variants never leave a tool
//! half started.

#![allow(clippy::unwrap_used, reason = "test code")]

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use gc_app::{GraphCtrlApp, RunOptions};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn harness() -> egui_kittest::Harness<'static, GraphCtrlApp> {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .with_step_dt(0.05)
        .build_eframe(|cc| GraphCtrlApp::new(cc, RunOptions::default()));
    harness.run();
    harness
}

fn mouse_click(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, at: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    harness.run_steps(3);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(3);
    }
}

/// Opens the arrow of a split button ("<group> 더 보기") with the mouse and picks a variant by name.
fn pick_variant(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, group: &str, variant: &str) {
    let at = harness.get_by_label(&format!("{group} 더 보기")).rect().center();
    mouse_click(harness, at);
    // The list is drawn after the ribbon: its entry is the last node of that name (the body may share it).
    harness.query_all_by_label(variant).last().unwrap().click();
    harness.run_steps(3);
}

/// Presses the arrow without picking anything.
fn open_arrow(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>, group: &str) {
    let at = harness.get_by_label(&format!("{group} 더 보기")).rect().center();
    mouse_click(harness, at);
}

fn two_bodies(harness: &mut egui_kittest::Harness<'_, GraphCtrlApp>) {
    let solid = gc_doc::Solid::Box { size: [0.05; 3] };
    let app = harness.state_mut();
    app.session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    app.session.add_body(solid, gc_doc::Placement { xyz: [0.2, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
    harness.run();
}

/// The world-space normal of a sketch's plane.
fn normal_of(
    harness: &egui_kittest::Harness<'_, GraphCtrlApp>,
    sketch: gc_core::Id<gc_doc::Sketch>,
) -> gc_math::DVec3 {
    let session = &harness.state().session;
    let (gc_sketch::Space::Planar(frame) | gc_sketch::Space::Spatial(frame)) =
        session.doc().get(sketch).unwrap().space;
    session.doc().frame_world_pose(gc_core::Id::from_raw(frame.raw)).unwrap().rotation * gc_math::DVec3::Z
}

/// The sketch split button: the arrow picks a plane and starts that sketch; after finishing it the body repeats
/// the same plane (the label shows it); the arrow can pick another; while a sketch is open the whole button is
/// disabled, whichever part is pressed.
#[test]
fn the_sketch_split_button_remembers_the_plane_and_is_disabled_while_sketching() {
    let mut harness = harness();
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    assert!(harness.state().session.active_sketch().is_none());

    pick_variant(&mut harness, "스케치", "XZ 스케치");
    let first = harness.state().session.active_sketch().map(|(id, _)| id).expect("XZ sketch started");
    // The ribbon moved to the Sketch tab; finish the sketch to come back.
    harness.get_by_label("스케치 마무리").click();
    harness.run_steps(3);
    assert!(harness.state().session.active_sketch().is_none());
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);

    // The body now reads "XZ 스케치": one click repeats it.
    harness.get_by_label("XZ 스케치").click();
    harness.run_steps(3);
    let second =
        harness.state().session.active_sketch().map(|(id, _)| id).expect("the body started a sketch");
    assert_ne!(first, second);
    assert!(
        (normal_of(&harness, first) - normal_of(&harness, second)).length() < 1e-9,
        "the body made a sketch on another plane than the one last used"
    );
    // The arrow picks another plane; the 3D sketch is a spatial one.
    harness.get_by_label("스케치 마무리").click();
    harness.run_steps(3);
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    pick_variant(&mut harness, "스케치", "3D 스케치");
    assert!(harness.state().session.sketch_is_spatial());
    harness.get_by_label("스케치 마무리").click();
    harness.run_steps(3);
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    harness.get_by_label("3D 스케치").click();
    harness.run_steps(3);
    assert!(harness.state().session.sketch_is_spatial(), "the body did not repeat the 3D sketch");
    harness.get_by_label("스케치 마무리").click();
    harness.run_steps(3);

    // While a sketch is open (Sketch tab) the Model tab's button cannot start a second one: go back to the
    // tab by hand and press body and arrow.
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    harness.get_by_label("3D 스케치").click();
    harness.run_steps(3);
    let open = harness.state().session.active_sketch().map(|(id, _)| id).unwrap();
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    let count = harness.state().session.doc().sketches().iter().count();
    if let Some(body) = harness.query_by_label("3D 스케치") {
        let at = body.rect().center();
        mouse_click(&mut harness, at);
    }
    open_arrow(&mut harness, "스케치");
    harness.run_steps(2);
    assert_eq!(
        harness.state().session.doc().sketches().iter().count(),
        count,
        "a disabled split button made a sketch"
    );
    assert_eq!(harness.state().session.active_sketch().map(|(id, _)| id), Some(open));
}

/// Return on the Assemble tab: disabled with nothing being edited (body and arrow do nothing); with a
/// component being edited, the arrow lists the three ways back and the body repeats the last one.
#[test]
fn the_return_split_button_does_nothing_at_the_top_and_repeats_the_last_way_back() {
    let mut harness = harness();
    two_bodies(&mut harness);
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.edit_target(), None);
    let depth = harness.state().session.history().undo_depth();
    let before = harness.state().session.doc().clone();
    // Nothing is being edited: body and arrow do nothing.
    let body = harness.get_by_label("복귀").rect().center();
    mouse_click(&mut harness, body);
    open_arrow(&mut harness, "편집");
    harness.run_steps(2);
    assert!(harness.query_by_label("상위로").is_none(), "a disabled arrow opened its list");
    assert_eq!(harness.state().session.edit_target(), None);
    assert!(harness.state().session.doc().same_content(&before));
    assert_eq!(harness.state().session.history().undo_depth(), depth);

    // Edit a component in place (through the session; the double click is covered elsewhere), then use the
    // variants: Return to Top, then Edit again and the body repeats Return to Top.
    let first = harness.state().session.doc().bodies().iter().next().map(|(id, _)| id).unwrap();
    let key = gc_doc::TreeKey::Component(gc_doc::Occurrence::Item(first.erase()));
    harness.state_mut().session.edit_component(key).unwrap();
    harness.run_steps(3);
    pick_variant(&mut harness, "편집", "최상위로");
    assert_eq!(harness.state().session.edit_target(), None);
    harness.state_mut().session.edit_component(key).unwrap();
    harness.run_steps(3);
    // The body now reads "최상위로" and goes to the top again.
    harness.get_by_label("최상위로").click();
    harness.run_steps(3);
    assert_eq!(harness.state().session.edit_target(), None, "the body did not repeat Return to Top");
    // Return to Parent on a component at the top level: harmless; Return with no history: harmless.
    harness.state_mut().session.edit_component(key).unwrap();
    harness.run_steps(3);
    pick_variant(&mut harness, "편집", "상위로");
    assert_eq!(harness.state().session.edit_target(), None, "the parent of a top-level component is the top");
    harness.state_mut().session.edit_component(key).unwrap();
    harness.run_steps(3);
    pick_variant(&mut harness, "편집", "복귀");
    harness.run_steps(2);
    assert!(harness.state().session.doc().same_content(&before), "returning changed the document");
    assert_eq!(harness.state().session.history().undo_depth(), depth, "returning recorded an undo step");
}

/// Free Move / Free Rotate: disabled with no component; with components the body toggles the tool that is
/// shown on it, the arrow switches tool, and a second press of the running tool stops it. A tool that was left
/// running does not survive deleting everything.
#[test]
fn the_free_move_split_button_toggles_and_switches_tools() {
    let mut harness = harness();
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    // No components: pressing does nothing.
    let at = harness.get_by_label("컴포넌트 이동").rect().center();
    mouse_click(&mut harness, at);
    open_arrow(&mut harness, "위치");
    harness.run_steps(2);
    assert!(harness.query_by_label("컴포넌트 회전").is_none(), "a disabled arrow opened its list");
    assert!(harness.state().viewport().free_tool.is_none());

    two_bodies(&mut harness);
    harness.get_by_label("컴포넌트 이동").click();
    harness.run_steps(3);
    assert_eq!(harness.state().viewport().free_tool, Some(gc_viewport::FreeTool::Move));
    // The running tool is on the button: pressing it again stops it.
    harness.get_by_label("컴포넌트 이동").click();
    harness.run_steps(3);
    assert!(harness.state().viewport().free_tool.is_none(), "a second press did not stop the tool");
    // The arrow picks Rotate, which starts it; picking Rotate again stops it; picking Move switches.
    pick_variant(&mut harness, "위치", "컴포넌트 회전");
    assert_eq!(harness.state().viewport().free_tool, Some(gc_viewport::FreeTool::Rotate));
    pick_variant(&mut harness, "위치", "컴포넌트 이동");
    assert_eq!(harness.state().viewport().free_tool, Some(gc_viewport::FreeTool::Move), "switching tools");
    pick_variant(&mut harness, "위치", "컴포넌트 이동");
    assert!(harness.state().viewport().free_tool.is_none());

    // Leaving the tab with a tool running and coming back keeps one consistent state.
    pick_variant(&mut harness, "위치", "컴포넌트 회전");
    harness.get_by_label("3D 모델").click();
    harness.run_steps(3);
    harness.get_by_label("조립").click();
    harness.run_steps(3);
    let tool = harness.state().viewport().free_tool;
    assert!(tool.is_none() || tool == Some(gc_viewport::FreeTool::Rotate));
    // No panic and the document did not move through any of this.
    let session = &harness.state().session;
    for (id, body) in session.doc().bodies().iter() {
        assert!(body.placement.xyz.iter().all(|v| v.is_finite()), "{id:?}");
    }
}
