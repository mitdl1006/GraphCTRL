//! The marking menu (Inventor 2026 help, "About Marking Menus":
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-4B081B74-45EA-451E-84BE-9FFE0E2ACAE3.htm):
//! a quick mark toward a command picks it; a right-click without a mark opens the radial menu, whose
//! commands and overflow menu are clicked by name; Esc closes it.

#![allow(clippy::unwrap_used, reason = "test code")]

use egui_kittest::kittest::Queryable;
use gc_ui::marking::{MarkingItem, MarkingMenu, MarkingPick};

struct State {
    fonts: bool,
    menu: MarkingMenu,
    picked: Vec<MarkingPick>,
}

fn harness() -> egui_kittest::Harness<'static, State> {
    egui_kittest::Harness::builder().with_size(egui::vec2(800.0, 600.0)).build_ui_state(
        |ui, state: &mut State| {
            let ctx = &ui.ctx().clone();
            // The bundled fonts, as the application installs them (they apply from the next frame; the
            // menu is closed on the first one).
            if !state.fonts {
                gc_ui::fonts::install(ctx);
                state.fonts = true;
            }
            if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
                && let Some(at) = ctx.input(|i| i.pointer.latest_pos())
            {
                let time = ctx.input(|i| i.time);
                state.menu.press(at, time);
            }
            let radial = [
                Some(MarkingItem::new("North")),
                None,
                Some(MarkingItem::new("East")),
                None,
                Some(MarkingItem::new("South")),
                None,
                Some(MarkingItem { label: "West".into(), enabled: false }),
                None,
            ];
            let overflow = [MarkingItem::new("More")];
            if let Some(pick) = state.menu.show(ctx, &radial, &overflow) {
                state.picked.push(pick);
            }
        },
        State { fonts: false, menu: MarkingMenu::default(), picked: Vec::new() },
    )
}

fn button(harness: &mut egui_kittest::Harness<'_, State>, at: egui::Pos2, pressed: bool) {
    harness.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
}

#[test]
fn a_quick_mark_picks_the_command_in_its_direction() {
    let mut harness = harness();
    let centre = egui::pos2(400.0, 300.0);
    harness.event(egui::Event::PointerMoved(centre));
    harness.step();
    button(&mut harness, centre, true);
    harness.event(egui::Event::PointerMoved(centre + egui::vec2(70.0, 4.0)));
    harness.step();
    button(&mut harness, centre + egui::vec2(70.0, 4.0), false);
    assert_eq!(harness.state().picked, vec![MarkingPick::Radial(2)]);
    assert!(!harness.state().menu.is_open());

    // A mark toward a disabled command picks nothing and leaves the menu open.
    button(&mut harness, centre, true);
    harness.event(egui::Event::PointerMoved(centre - egui::vec2(70.0, 0.0)));
    harness.step();
    button(&mut harness, centre - egui::vec2(70.0, 0.0), false);
    assert_eq!(harness.state().picked.len(), 1);
    assert!(harness.state().menu.is_open());
}

#[test]
fn a_right_click_opens_the_menu_and_its_commands_are_clicked() {
    let mut harness = harness();
    let centre = egui::pos2(400.0, 300.0);
    harness.event(egui::Event::PointerMoved(centre));
    harness.step();
    button(&mut harness, centre, true);
    button(&mut harness, centre, false);
    harness.run_steps(2);
    assert!(harness.state().menu.is_open());
    harness.get_by_label("South").click();
    harness.run_steps(2);
    assert_eq!(harness.state().picked, vec![MarkingPick::Radial(4)]);

    button(&mut harness, centre, true);
    button(&mut harness, centre, false);
    harness.run_steps(2);
    harness.get_by_label("More").click();
    harness.run_steps(2);
    assert_eq!(harness.state().picked.last(), Some(&MarkingPick::Overflow(0)));

    button(&mut harness, centre, true);
    button(&mut harness, centre, false);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().menu.is_open());
}
