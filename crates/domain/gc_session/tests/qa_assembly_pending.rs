//! Acceptance tests for the assembly parity items (docs/ux/inventor-assembly-parity.md, section 5, P1 1-13).
//! The assembly model does not exist yet, so every item is `#[ignore]`d with its reason and the steps
//! to check, taken from the Inventor 2026 help (base
//! <https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/>; source ids A.. as in the document).
//! Done and removed from this list (see `qa_relations.rs`, `qa_joints.rs`, `qa_origin_folder.rs`, `qa_assembly_edges.rs`,
//! `gc_mesh/tests/qa_slice.rs`): P1-9 Mate/Flush/Insert, P1-10 status and DOF (data side), P1-11 joints and tool mount,
//! P1-6 cut edges (plane_cut). P1-5 project geometry still needs its own acceptance test (see handoff).
//! Also done: P1-1 component states (`qa_assembly_edges.rs`), P1-2/3 origin folder (`qa_origin_folder.rs`,
//! `gc_app/tests/qa_assembly_ui.rs`), P1-4 sketches on component planes (`sketch_planes.rs`), P1-8 section views,
//! P1-12 measure and P1-13 free move (frontend kittests in `gc_app/tests/shell.rs`, edge cases listed in the handoff).
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
    a7_slice_graphics_f7_cuts_away_what_hides_the_sketch_plane,
    "FEATURE assembly P1-7: F7 exists for sketch bodies (shell.rs slice_graphics_...) but not for imported assemblies (A40)",
    "Imported equipment in front of the plane: F7 clips it, the plane is visible; Finish Sketch and starting \
     a feature command restore it; F7 twice toggles; slicing with nothing in front changes nothing"
);
