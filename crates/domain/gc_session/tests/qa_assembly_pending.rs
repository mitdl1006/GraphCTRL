//! Acceptance tests for the assembly parity items (docs/ux/inventor-assembly-parity.md, section 5, P1 1-13).
//! The assembly model does not exist yet, so every item is `#[ignore]`d with its reason and the steps
//! to check, taken from the Inventor 2026 help (base
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>; source ids A.. as in the document).
//! Done and removed from this list (see `qa_relations.rs`, `qa_joints.rs`, `qa_origin_folder.rs`, `qa_assembly_edges.rs`,
//! `gc_mesh/tests/qa_slice.rs`): P1-9 Mate/Flush/Insert, P1-10 status and DOF (data side), P1-11 joints and tool mount,
//! P1-6 cut edges (plane_cut). P1-5 project geometry still needs its own acceptance test (see handoff).
//! When an item lands: remove the `#[ignore]`, write the body against the new session API, and report a
//! failure in `docs/handoff/qa.md`. Common invariants for all: undo∘redo = identity, save∘load = identity,
//! a refused edit leaves the document bit-identical, no panic on any input.

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
    a1_components_form_a_hierarchy_with_grounded_visible_enabled_suppressed_flags,
    "FEATURE assembly P1-1: no component hierarchy in the document (A1, A2, A5)",
    "Import two parts and a sub-assembly; set flags; save and reopen -> hierarchy and flags identical; the \
     first component is grounded automatically and later ones are not (LEAD DECISION 2026-11-17; covered \
     by gc_session/tests/qa_assembly_edges.rs::only_the_first_component_is_grounded); Undo restores in one step; a cycle (component inside itself) is refused; deleting a \
     parent asks about its children"
);

pending!(
    a2_every_component_has_an_origin_folder_with_planes_axes_and_a_centre_point,
    "FEATURE assembly P1-2 (screen half): data model and visibility are tested in qa_origin_folder.rs; the browser folder rows with the decided names and the drawing are on the frontend branch de9ef3e, not merged into the lead branch yet (A52, A53, A55)",
    "LEAD DECISION 2026-11-17: under a folder `원점` (en `Origin`) the children, in this order, are `YZ 평면`, \
     `XZ 평면`, `XY 평면`, `X축`, `Y축`, `Z축`, `중심점` (en YZ Plane, XZ Plane, XY Plane, X Axis, Y Axis, Z Axis, \
     Center Point), the same spelling as the document origin. Two components each have one; hidden by default; \
     showing one draws it at the component's pose and it follows when the component moves; `Hide Work Features` \
     hides all"
);

pending!(
    a3_origin_planes_axes_and_points_can_be_picked_in_the_viewport,
    "FEATURE assembly P1-3 (screen half): viewport picking of origin features is on the frontend branch de9ef3e, not merged into the lead branch yet (A52, A53)",
    "LEAD DECISION 2026-11-17: hidden origin planes, axes and points are NOT selectable in the viewport but ARE \
     selectable in the browser; once shown they are selectable in the viewport. Click a shown plane -> \
     highlighted, browser synced, properties show the owner and coordinates; Esc clears; right-click offers \
     create sketch; a click on a hidden plane's place selects whatever is behind it"
);

pending!(
    a4_a_sketch_on_a_component_plane_follows_the_component,
    "FEATURE assembly P1-4: sketches on component planes (A11, A41, A42; session half exists: gc_session/tests/sketch_planes.rs)",
    "Sketch on a fixture's face/origin plane, move the fixture -> the sketch stays on the plane in world \
     space; save+reopen keeps the reference; replacing the part so the face vanishes marks the sketch with an \
     error and does not delete it; offset and midplane work planes behave the same"
);

pending!(
    a7_slice_graphics_f7_cuts_away_what_hides_the_sketch_plane,
    "FEATURE assembly P1-7: F7 exists for sketch bodies (shell.rs slice_graphics_...) but not for imported assemblies (A40)",
    "Imported equipment in front of the plane: F7 clips it, the plane is visible; Finish Sketch and starting \
     a feature command restore it; F7 twice toggles; slicing with nothing in front changes nothing"
);

pending!(
    a8_section_views_half_quarter_three_quarter_with_caps,
    "FEATURE assembly P1-8: no section views (A44, A45)",
    "Half section of equipment + UR: the cut is visible and the cap fills closed faces (pixel check inside the \
     cut area differs from the background); Flip reverses; value box moves the plane; Delete Section View \
     restores; more than 500 parts hides the caps; a plane that misses everything shows the full model"
);

pending!(
    a12_measure_gives_distances_and_angles_that_agree_with_the_collision_library,
    "FEATURE assembly P1-12: measure tool (A46)",
    "Distance from the TCP to an equipment edge is shown in mm; two components give a minimum distance equal to \
     gc_collide's within 1e-6 m; min/centre/max switch; selecting nothing or one item shows nothing, not an \
     error; touching components measure 0, overlapping ones are reported as overlapping"
);

pending!(
    a13_free_move_and_free_rotate_then_update_returns_constrained_components,
    "FEATURE assembly P1-13: free move/rotate and update (A12, A13)",
    "Drag a constrained component, Update -> back at its constrained pose (1e-9 m); a grounded or \
     unconstrained component keeps the new pose; one Undo step per drag; Esc during the drag restores bit-for-bit"
);
