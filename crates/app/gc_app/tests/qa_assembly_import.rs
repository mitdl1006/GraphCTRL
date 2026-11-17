//! QA edge cases for importing assembly files in the application (GLB/glTF and STEP AP242): what is an
//! assembly by its name, and what the importer and the project-open reader do with files that are missing,
//! empty, damaged or not what their name says. The module is compiled into this test because it is private to
//! the application crate.

#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code; the module is included whole"
)]

#[path = "../src/assembly_import.rs"]
mod assembly_import;

use std::path::Path;

use assembly_import::{importer, is_assembly, reader};

fn fixture() -> String {
    format!("{}/../../../fixtures/step/two_blocks_ap242.stp", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn only_assembly_file_types_are_assemblies_whatever_their_case() {
    for yes in [
        "a.glb",
        "a.GLB",
        "a.gltf",
        "a.GlTf",
        "a.stp",
        "a.STP",
        "a.step",
        "a.STEP",
        "dir.with.dots/a.b.step",
        "한글 파일.stp",
    ] {
        assert!(is_assembly(Path::new(yes)), "{yes}");
    }
    for no in [
        "",
        "a",
        "a.",
        ".stp",
        "a.stl",
        "a.dae",
        "a.obj",
        "a.step.txt",
        "a.stpx",
        "step",
        "a.stp/",
        "a.gctrl",
    ] {
        // ".stp" is a hidden file called ".stp" with no extension in Rust's path rules; "a.stp/" has the extension stp.
        let expected = no == "a.stp/";
        assert_eq!(is_assembly(Path::new(no)), expected, "{no:?}");
    }
}

/// Missing, a directory, empty, random bytes, and a file whose content does not match its extension: each is
/// an error that names the file, never a panic.
#[test]
fn unreadable_files_give_errors_that_name_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let read = importer();
    let reopen = reader();
    let make = |name: &str, bytes: &[u8]| {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_string_lossy().into_owned()
    };
    let step_text = std::fs::read(fixture()).unwrap();
    let cases: Vec<(String, &str)> = vec![
        (dir.path().join("missing.stp").to_string_lossy().into_owned(), "missing"),
        (dir.path().to_string_lossy().into_owned(), "a folder"),
        (make("empty.stp", b""), "empty step"),
        (make("empty.glb", b""), "empty glb"),
        (make("random.glb", &[0x13; 1000]), "random glb"),
        (make("random.stp", &[0x13; 1000]), "random stp"),
        (make("glb_named_step.stp", b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00"), "glb bytes in a .stp"),
        (make("step_named_glb.glb", &step_text), "step text in a .glb"),
        (make("noext", &step_text), "no extension (read as glTF)"),
        (make("truncated.stp", &step_text[..step_text.len() / 2]), "half a step file"),
        (make("한글.stp", b"not a step file"), "a Korean name"),
    ];
    for (file, what) in &cases {
        let result = read(file);
        let error = result.err().unwrap_or_else(|| panic!("{what}: accepted"));
        assert!(!error.is_empty(), "{what}");
        let again = reopen(file);
        assert!(again.is_err(), "{what}: the project reader accepted it");
    }
    // The error for a file that exists names it.
    let error = read(&cases[4].0).err().unwrap();
    assert!(error.contains("random.glb"), "{error}");
}

/// The fixture imports as a valid assembly; reopening a project reads the same meshes again.
#[test]
fn the_fixture_imports_and_reads_back_the_same_meshes() {
    let (assembly, meshes) = importer()(&fixture()).unwrap();
    assert!(assembly.is_valid());
    assert_eq!(assembly.meshes.len(), meshes.len());
    assert_eq!(assembly.name, "two_blocks_ap242");
    assert!(assembly.parts.iter().all(|p| p.meshes.iter().all(|m| (*m as usize) < meshes.len())));
    let again = reader()(&fixture()).unwrap();
    assert_eq!(again.len(), meshes.len());
    for (a, b) in again.iter().zip(&meshes) {
        assert_eq!(a.positions(), b.positions());
        assert_eq!(a.indices(), b.indices());
    }
    // A copy under another name and folder reads the same (the file is found by its path, not its name).
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("kopie 한글.STEP");
    std::fs::copy(fixture(), &copy).unwrap();
    let (other, _) = importer()(&copy.to_string_lossy()).unwrap();
    assert_eq!(other.name, "kopie 한글");
    assert_eq!(other.parts.len(), assembly.parts.len());
}
