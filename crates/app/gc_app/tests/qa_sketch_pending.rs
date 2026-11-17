//! Acceptance tests for sketch parity items whose screen side is not built yet
//! (docs/ux/inventor-sketch-parity.md, P1). Each is `#[ignore]`d with the reason; when the feature lands
//! the lead (or the owner) removes the `#[ignore]` and fills in the harness calls, then the failure goes to
//! `docs/handoff/qa.md` as a bug if it does not pass. The steps and expected values come from the Inventor
//! 2026 help (base <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>); the headless half
//! of each item is already covered in `gc_session/tests/{inventor_sketch,qa_sketch_p1}.rs`.

#![allow(clippy::panic, reason = "test code: a pending acceptance test fails loudly when it is run")]

macro_rules! pending {
    ($name:ident, $reason:literal, $steps:literal) => {
        #[test]
        #[ignore = $reason]
        fn $name() {
            panic!("not implemented yet. Steps: {}", $steps);
        }
    };
}

pending!(
    p1_6_hud_boxes_appear_next_to_the_cursor_and_tab_moves_between_them,
    "FEATURE P1-6: heads-up input boxes are not drawn (S4 GUID-33A8B11D-A04C-4809-BC82-C0419215915F)",
    "Line tool, click start; boxes Length+Angle appear near the cursor; type 50, Tab (padlock shows on Length), \
     type 90, Enter -> line of 50 at 90 deg with a driving dimension; Esc cancels the typed values; invalid \
     text turns the box red and Enter does nothing; same for circle (Diameter) and rectangles; with a \
     Korean IME on, Tab/Enter still work and L/C/D do not leak into the box"
);

pending!(
    p1_8_trim_extend_fillet_have_ribbon_buttons_and_the_shortcuts,
    "FEATURE P1-8/P1-14: no ribbon buttons or keys for Trim, Extend, Fillet (S19 GUID-DD00037F-6BBA-4B95-AA90-4F99DCEB667D, \
     S28 GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E)",
    "Ribbon sketch tab shows Trim/Extend/Fillet; TR starts Trim; Shift held switches Trim<->Extend with the \
     tool label following; hover shows the piece to go (preview); a Fillet radius box keeps its value; Esc \
     and right-click Done end the tool"
);

pending!(
    p1_11_double_click_on_a_dimension_edits_it_and_name_equals_value_makes_a_parameter,
    "FEATURE P1-11: `HGHT = 5mm` does not create a parameter in the edit box (S14 GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185)",
    "Double-click a dimension -> box with the current value; type `HGHT = 5mm` + Enter -> parameter HGHT \
     exists and the dimension refers to it; reusing the name updates the parameter; `1a`, a reserved word or \
     a space in the name is refused with the reason; Esc leaves everything bit-identical; Measure fills the \
     current value"
);

pending!(
    p1_12_driven_dimension_toggle_and_over_constraint_accept_cancel,
    "FEATURE P1-12: no Driven toggle in the UI; the over-constraint dialog is only in the session (S14)",
    "On a fully constrained sketch add a dimension -> dialog; Accept makes a bracketed (driven) dimension, \
     Cancel leaves the document unchanged; on a not fully constrained sketch the Driven toggle turns a \
     dimension driven and back, and refuses going back to driving when that would over-constrain; driven \
     dimensions do not change the DOF count; one Undo step each"
);

pending!(
    p1_14_the_command_aliases_start_the_tools_even_with_a_korean_keyboard_layout,
    "FEATURE P1-14: only L, C, A, R, D exist; T, TR, `-` etc. missing (S28 GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E)",
    "L, C, D, T, TR start Line, Circle, Dimension, ... as in the alias table; A and R stay as aliases; \
     keys are matched by key code so the Korean layout works; typing into a text box never starts a tool; \
     a 2D sketch Tab does not switch the 3D plane"
);
