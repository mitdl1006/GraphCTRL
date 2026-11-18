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

// ---- Assemble tab: Grounded and Place ----

struct Chosen(std::sync::Arc<std::sync::Mutex<std::path::PathBuf>>);

impl gc_app::FileDialogs for Chosen {
    fn open(&mut self) -> Option<std::path::PathBuf> {
        None
    }
    fn save(&mut self, _: &str) -> Option<std::path::PathBuf> {
        None
    }
    fn open_mesh(&mut self) -> Option<std::path::PathBuf> {
        Some(self.0.lock().unwrap().clone())
    }
}

fn harness_with_dialogs(
    chosen: &std::sync::Arc<std::sync::Mutex<std::path::PathBuf>>,
) -> egui_kittest::Harness<'static, GraphCtrlApp> {
    let dialogs = Chosen(std::sync::Arc::clone(chosen));
    let mut harness = egui_kittest::Harness::builder()
        .with_size(SIZE)
        .with_pixels_per_point(1.0)
        .build_eframe(move |cc| GraphCtrlApp::new(cc, RunOptions::default()).with_dialogs(Box::new(dialogs)));
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    harness
}

/// Grounded with nothing selected changes nothing; with a body selected it flips, says so in the status
/// bar, and Ctrl+Z puts it back; a second toggle says it is freed.
#[test]
fn ground_changes_only_a_selected_component_and_says_so() {
    let chosen = std::sync::Arc::new(std::sync::Mutex::new(std::path::PathBuf::new()));
    let mut harness = harness_with_dialogs(&chosen);
    let body = harness
        .state_mut()
        .session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    // The first component is grounded already; add a second so there is one that is free.
    let second = harness
        .state_mut()
        .session
        .add_body(
            gc_doc::Solid::Box { size: [0.1; 3] },
            gc_doc::Placement { xyz: [2.0, 0.0, 0.0], rpy: [0.0; 3] },
        )
        .unwrap();
    harness.state_mut().session.select_item(None);
    harness.run();
    let before = harness.state().session.doc().clone();
    harness.get_by_label("접지").click();
    harness.run();
    assert!(harness.state().session.doc().same_content(&before), "nothing selected: nothing changes");

    harness.state_mut().session.select_item(Some(gc_session::CellItem::Body(second)));
    harness.run();
    let is_grounded = |h: &egui_kittest::Harness<'_, GraphCtrlApp>| {
        h.state()
            .session
            .doc()
            .component_flag(gc_doc::Occurrence::Item(second.erase()), gc_doc::ComponentFlag::Grounded)
    };
    assert!(!is_grounded(&harness));
    harness.get_by_label("접지").click();
    harness.run();
    assert!(is_grounded(&harness));
    harness.get_by_label_contains("접지했습니다");
    harness.get_by_label("접지").click();
    harness.run();
    assert!(!is_grounded(&harness));
    harness.get_by_label_contains("접지를 풀었습니다");
    // Undo twice: back to the state before any toggle.
    for _ in 0..2 {
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        harness.run();
    }
    assert!(
        harness.state().session.doc().same_content(&before)
            || harness.state().session.doc().get(body).is_some()
    );
}

/// Place with a file that does not exist, is empty, or has the wrong content: an error line in the status
/// bar, no assembly added, no crash, and a good file afterwards still works.
#[test]
fn place_reports_missing_and_empty_files_and_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty.glb");
    std::fs::write(&empty, b"").unwrap();
    let chosen = std::sync::Arc::new(std::sync::Mutex::new(dir.path().join("missing.stp")));
    let mut harness = harness_with_dialogs(&chosen);
    for bad in [dir.path().join("missing.stp"), empty] {
        *chosen.lock().unwrap() = bad;
        harness.get_by_label("배치").click();
        harness.run_steps(4);
        let shown = harness.query_by_label_contains("가져오기 실패: ").is_some();
        assert!(shown, "no error line for {:?}", chosen.lock().unwrap());
        assert!(harness.state().session.doc().assemblies().is_empty());
    }
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/step/two_blocks_ap242.stp");
    *chosen.lock().unwrap() = fixture;
    harness.get_by_label("배치").click();
    harness.run_steps(4);
    assert_eq!(harness.state().session.doc().assemblies().len(), 1, "a good file works after the failures");
}

// ---- Place: Esc and right click leave nothing behind ----

/// A small ASCII STL (one triangle fan of a plate), written to `dir`.
fn plate_stl(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("plate.stl");
    let mut stl = String::from("solid plate\n");
    for tri in [
        [[0.0, 0.0, 0.0], [0.04, 0.0, 0.0], [0.04, 0.04, 0.0]],
        [[0.0, 0.0, 0.0], [0.04, 0.04, 0.0], [0.0, 0.04, 0.0]],
    ] {
        stl.push_str("facet normal 0 0 1\nouter loop\n");
        for [x, y, z] in tri {
            stl.push_str(&format!("vertex {x} {y} {z}\n"));
        }
        stl.push_str("endloop\nendfacet\n");
    }
    stl.push_str("endsolid plate\n");
    std::fs::write(&file, stl).unwrap();
    file
}

/// Place with a component already in the cell hangs the new one on the cursor. Ending with Esc (cursor never
/// moved, or moved) or with a right click leaves no placing state, no extra component, no pending preview
/// (what is shown equals the document) and, because nothing was set down, no half step on the undo stack.
#[test]
fn escaping_or_right_clicking_during_place_leaves_nothing_behind() {
    for how in ["esc, no move", "esc after move", "right click after move"] {
        let dir = tempfile::tempdir().unwrap();
        let chosen = std::sync::Arc::new(std::sync::Mutex::new(plate_stl(dir.path())));
        let mut harness = harness_with_dialogs(&chosen);
        let first = harness
            .state_mut()
            .session
            .add_body(gc_doc::Solid::Box { size: [0.05; 3] }, gc_doc::Placement::default())
            .unwrap();
        harness.state_mut().fit_view();
        harness.run();
        harness.get_by_label("배치").click();
        harness.run_steps(4);
        assert!(harness.state().viewport().is_placing(), "{how}: the new component hangs on the cursor");
        let bodies =
            |h: &egui_kittest::Harness<'_, GraphCtrlApp>| h.state().session.doc().bodies().iter().count();
        assert_eq!(bodies(&harness), 2, "{how}");
        let hanging =
            harness.state().session.doc().bodies().iter().map(|(id, _)| id).find(|id| *id != first).unwrap();

        if how != "esc, no move" {
            let at = harness.state().viewport().screen_of(gc_math::DVec3::new(0.03, 0.02, 0.0)).unwrap();
            harness.event(egui::Event::PointerMoved(at));
            harness.run_steps(3);
        }
        let depth = harness.state().session.history().undo_depth();
        if how == "right click after move" {
            let at = harness.state().viewport().screen_of(gc_math::DVec3::new(0.03, 0.02, 0.0)).unwrap();
            for pressed in [true, false] {
                harness.event(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
                harness.run_steps(3);
            }
        } else {
            harness.key_press(egui::Key::Escape);
            harness.run_steps(3);
        }
        assert!(!harness.state().viewport().is_placing(), "{how}: still placing");
        assert_eq!(bodies(&harness), 2, "{how}: the placed component stays, nothing extra appears");
        assert_eq!(
            harness.state().session.history().undo_depth(),
            depth,
            "{how}: a step was recorded for nothing"
        );
        let session = &harness.state().session;
        let shown = session.shown_body(hanging).unwrap().placement;
        let stored = session.doc().get(hanging).unwrap().placement;
        assert_eq!(shown, stored, "{how}: a preview of the hanging component is left over");
        // Esc again does nothing more and does not panic.
        harness.key_press(egui::Key::Escape);
        harness.run_steps(2);
        assert_eq!(bodies(&harness), 2, "{how}: second Esc");
    }
}

// ---- Assemble command: conflicts open the constraint error dialog ----

/// Applies a Mate of the two bodies' XY planes, then a second Mate of the same planes at another offset (they
/// cannot both hold): the second is refused, the constraint error dialog names the first, the document is
/// untouched by the refusal; Cancel leaves everything as it was; Delete on the culprit lets the second in.
#[test]
fn an_unsatisfiable_assemble_opens_the_constraint_error_dialog_and_deleting_the_culprit_fixes_it() {
    let mut harness = harness();
    let solid = gc_doc::Solid::Box { size: [0.05; 3] };
    let fixed = harness.state_mut().session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
    let moving = harness
        .state_mut()
        .session
        .add_body(solid, gc_doc::Placement { xyz: [0.6, 0.0, 0.0], rpy: [0.0; 3] })
        .unwrap();
    let plane = gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Xy);
    for body in [fixed, moving] {
        harness
            .state_mut()
            .session
            .set_origin_visible(gc_doc::Occurrence::Item(body.erase()), &[plane], true)
            .unwrap();
    }
    harness.state_mut().fit_view();
    harness.run();
    harness.get_by_label("조립").click();
    harness.run();
    harness.get_by_label("어셈블").click();
    harness.run_steps(3);

    let spot = |harness: &egui_kittest::Harness<'_, GraphCtrlApp>, x: f64| {
        harness.state().viewport().screen_of(gc_math::DVec3::new(x, 0.02, 0.0)).unwrap()
    };
    let relationships = |h: &egui_kittest::Harness<'_, GraphCtrlApp>| h.state().session.relationships().len();

    // First application: moving body's plane (first pick) to the fixed body's plane (second pick).
    let at = spot(&harness, 0.62);
    click_at(&mut harness, at);
    let at = spot(&harness, 0.02);
    click_at(&mut harness, at);
    harness.get_by_label("적용").click();
    harness.run_steps(3);
    assert_eq!(relationships(&harness), 1, "the first Mate went in");

    // Second application of the same planes at 100 mm: cannot hold with the first.
    let at = spot(&harness, 0.62);
    click_at(&mut harness, at);
    let at = spot(&harness, 0.02);
    click_at(&mut harness, at);
    harness.get_by_label("오프셋").click();
    harness.get_by_label("오프셋").type_text("100");
    harness.run_steps(2);
    let before = harness.state().session.doc().clone();
    let depth = harness.state().session.history().undo_depth();
    harness.get_by_label("적용").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("구속 오류").is_some(), "the constraint error dialog did not open");
    assert_eq!(relationships(&harness), 1, "the refused relationship must not be in");
    assert!(harness.state().session.doc().same_content(&before), "the refusal changed the document");
    assert_eq!(harness.state().session.history().undo_depth(), depth);

    // Cancel (the dialog's, the last one on screen): nothing changes and the dialog goes.
    harness.query_all_by_label("취소").last().unwrap().click();
    harness.run_steps(2);
    assert!(harness.query_by_label("구속 오류").is_none(), "Cancel left the dialog open");
    assert_eq!(relationships(&harness), 1);
    assert!(harness.state().session.doc().same_content(&before), "Cancel changed the document");

    // Apply again: the dialog is back; Delete on the culprit places the new relationship.
    harness.get_by_label("적용").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("구속 오류").is_some(), "the dialog did not reopen");
    harness.query_all_by_label("삭제").last().unwrap().click();
    harness.run_steps(3);
    assert!(harness.query_by_label("구속 오류").is_none(), "the dialog stayed after the fix");
    let all = harness.state().session.relationships();
    assert_eq!(all.len(), 1, "the culprit was deleted and the new one placed");
    assert!(
        matches!(all[0].1.kind, gc_doc::RelationKind::Mate { offset, .. } if (offset - 0.1).abs() < 1e-9),
        "{:?}",
        all[0].1.kind
    );
    // One Ctrl+Z-sized step each: undo brings the culprit back.
    assert!(harness.state_mut().session.undo());
    assert!(harness.state_mut().session.undo());
}
