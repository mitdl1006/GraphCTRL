//! Acceptance tests for the assembly parity items (docs/ux/inventor-assembly-parity.md, section 5, P1 1-13).
//! The assembly model does not exist yet, so every item is `#[ignore]`d with its reason and the steps
//! to check, taken from the Inventor 2026 help (base
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>; source ids A.. as in the document).
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
     first component is grounded automatically (the help pages disagree, A11 vs A18: pin the behaviour the \
     lead decides); Undo restores in one step; a cycle (component inside itself) is refused; deleting a \
     parent asks about its children"
);

pending!(
    a2_every_component_has_an_origin_folder_with_planes_axes_and_a_centre_point,
    "FEATURE assembly P1-2: no per-component origin folder (A52, A53, A55)",
    "Two components, each with 3 planes + 3 axes + centre point, hidden by default; showing one origin \
     draws it at the component's pose and it follows when the component moves; `Hide Work Features` hides all; \
     the folder names match the lead's decision (the 2026 help does not name the axes: A52/A55)"
);

pending!(
    a3_origin_planes_axes_and_points_can_be_picked_in_the_viewport,
    "FEATURE assembly P1-3: no picking of origin features (A52, A53)",
    "Click a shown origin plane -> highlighted, browser synced, properties show the owner and coordinates; \
     Esc clears; a hidden plane is not pickable in the graphics window (decide and pin); right-click offers \
     create sketch"
);

pending!(
    a4_a_sketch_on_a_component_plane_follows_the_component,
    "FEATURE assembly P1-4: sketches on component planes (A11, A41, A42; session half exists: gc_session/tests/sketch_planes.rs)",
    "Sketch on a fixture's face/origin plane, move the fixture -> the sketch stays on the plane in world \
     space; save+reopen keeps the reference; replacing the part so the face vanishes marks the sketch with an \
     error and does not delete it; offset and midplane work planes behave the same"
);

pending!(
    a5_project_geometry_links_component_edges_and_work_features_into_a_sketch,
    "FEATURE assembly P1-5: project geometry (A38, A39, S20 GUID-B8D8B07B-86F2-4CB7-AB21-3131D94AD1CC)",
    "Project an edge, a vertex, a work feature and a mesh loop; moving the source updates the projection; \
     deleting the source turns it into fixed curves; Shift+click makes it construction; projecting nothing \
     (empty click) changes nothing"
);

pending!(
    a6_project_cut_edges_makes_the_intersection_of_a_plane_and_a_mesh,
    "FEATURE assembly P1-6: project cut edges (A38)",
    "Box mesh + plane through it -> 4 edges of the intersection, moving the plane updates them within \
     1e-9 m; a plane that misses the mesh creates nothing and the status bar says why; a plane touching only \
     a face/edge/vertex produces the documented result (pin it)"
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
    a9_mate_flush_insert_constraints_with_predict_offset,
    "FEATURE assembly P1-9: no assembly constraints (A19, A22, A28)",
    "Mate the top face of a box to the base bottom of a robot -> the robot moves there to 1e-9 m; Flush flips \
     the facing; the Relationships folder lists it with a filled state dot; Predict Offset proposes the \
     current distance; Show Preview does not change the document; conflicting or redundant constraints are \
     refused with the reason (Design-Doctor style), document untouched"
);

pending!(
    a10_assemble_shows_only_valid_types_and_dof_symbols_show_what_is_left,
    "FEATURE assembly P1-10: Assemble / Show / DOF symbols (A16, A24, A27, A33)",
    "Pick two planes -> only Mate-Mate, Mate-Flush and Angle-Directed are offered; after constraining, the \
     DOF symbol shows the remaining freedoms; Show / Hide All glyphs toggle; state dots: black fully constrained, \
     hollow under-constrained"
);

pending!(
    a11_rigid_and_rotational_joints_and_mounting_a_robot_on_equipment,
    "FEATURE assembly P1-11: joints (A30, A69)",
    "Rigid joint from a robot's base origin to a point on a work plane of the equipment -> moving the \
     equipment moves the robot; a tool origin rigidly joined to the flange frame makes the tool coincide with the \
     TCP; Rotational shows +/- limits; Flip component / Align / Invert change the result as documented; \
     jointing a component to itself or closing a loop is refused"
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
