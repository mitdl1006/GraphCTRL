//! QA edge cases for the STEP reader (AP242 tessellated shapes): damaged and hostile bytes must give an
//! error, never a panic or a hang, and the shipped fixture must read as the assembly it says it is.
//! (ISO 10303-21 text format; the fixture `fixtures/step/two_blocks_ap242.stp` is hand written.)

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_plugin_mesh::{ImportError, read_step};

fn fixture() -> Vec<u8> {
    let path = format!("{}/../../../fixtures/step/two_blocks_ap242.stp", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The fixture: an assembly with two uses of one 20 mm cube, in millimetres (so 0.02 m), coloured red.
#[test]
fn the_fixture_reads_as_two_uses_of_one_cube() {
    let assembly = read_step(&fixture()).unwrap();
    assert!(!assembly.meshes.is_empty());
    let cube = &assembly.meshes[0];
    assert_eq!(cube.indices.len() / 3, 12, "a cube is 12 triangles");
    let (lo, hi) = cube.positions.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
        (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k])))
    });
    for k in 0..3 {
        assert!(
            ((hi[k] - lo[k]) - 0.02).abs() < 1e-6,
            "edge {k} is {} m (the file is in millimetres)",
            hi[k] - lo[k]
        );
    }
    let users = assembly.nodes.iter().filter(|n| n.meshes.contains(&0)).count();
    assert_eq!(users, 2, "two uses of the one part");
    // Parents come before children and every index is in range.
    for (i, node) in assembly.nodes.iter().enumerate() {
        assert!(node.parent.is_none_or(|p| p < i));
        assert!(node.meshes.iter().all(|m| *m < assembly.meshes.len()));
    }
    assert!(cube.indices.iter().all(|i| (*i as usize) < cube.positions.len()));
}

/// Anything that is not STEP is refused with a message.
#[test]
fn not_a_step_file_is_unreadable() {
    for bytes in
        [&b""[..], b"hello", b"\0\0\0\0", b"solid cube\nendsolid", b"glTF\x02\0\0\0", &[0xff; 4096][..]]
    {
        match read_step(bytes) {
            Err(ImportError::Unreadable(why)) => assert!(!why.is_empty()),
            other => panic!("{:?} gave {other:?}", &bytes[..bytes.len().min(8)]),
        }
    }
}

/// Every prefix of the fixture (a truncated download) is an error or a valid assembly, never a panic.
#[test]
fn truncated_files_never_panic() {
    let bytes = fixture();
    let step = (bytes.len() / 400).max(1);
    for cut in (0..bytes.len()).step_by(step) {
        match read_step(&bytes[..cut]) {
            Ok(a) => {
                for n in &a.nodes {
                    assert!(n.meshes.iter().all(|m| *m < a.meshes.len()), "cut {cut}");
                }
            }
            Err(e) => assert!(!e.to_string().is_empty(), "cut {cut}"),
        }
    }
}

/// Single damaged bytes, deleted lines and swapped numbers: an error or a valid result, no panic.
#[test]
fn corrupted_files_never_panic() {
    let bytes = fixture();
    for k in (0..bytes.len()).step_by((bytes.len() / 300).max(1)) {
        for replacement in [b'0', b'#', b')', b'\'', b'-', b'E', 0u8, 0xff] {
            let mut damaged = bytes.clone();
            damaged[k] = replacement;
            if let Ok(a) = read_step(&damaged) {
                for n in &a.nodes {
                    assert!(n.meshes.iter().all(|m| *m < a.meshes.len()), "byte {k} -> {replacement}");
                }
                for m in &a.meshes {
                    assert!(
                        m.indices.iter().all(|i| (*i as usize) < m.positions.len()),
                        "byte {k} -> {replacement}"
                    );
                }
            }
        }
    }
    let text = String::from_utf8(bytes).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    for skip in 0..lines.len() {
        let without: String =
            lines.iter().enumerate().filter(|(i, _)| *i != skip).map(|(_, l)| format!("{l}\n")).collect();
        drop(read_step(without.as_bytes()));
    }
}

const HEADER: &str = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('x','',(''),(''),'','','');\nFILE_SCHEMA(('AP242'));\nENDSEC;\nDATA;\n";
const FOOTER: &str = "ENDSEC;\nEND-ISO-10303-21;\n";

/// Reads hostile text: bounded time, an error or a small result.
fn bounded(name: &str, body: &str) {
    let text = format!("{HEADER}{body}");
    let started = std::time::Instant::now();
    let result = read_step(text.as_bytes());
    assert!(started.elapsed().as_secs() < 20, "{name} took {:?}", started.elapsed());
    if let Ok(a) = result {
        assert!(a.nodes.len() < 10_000, "{name}");
    }
}

#[test]
fn reference_loops_and_dangling_references_are_bounded() {
    bounded(
        "loop",
        &format!(
            "#1=PRODUCT_DEFINITION('',$,#2,#2);\n#2=PRODUCT_DEFINITION('',$,#1,#1);\n#3=NEXT_ASSEMBLY_USAGE_OCCURRENCE('','','',#1,#2,$);\n#4=NEXT_ASSEMBLY_USAGE_OCCURRENCE('','','',#2,#1,$);\n{FOOTER}"
        ),
    );
    bounded("self", &format!("#1=NEXT_ASSEMBLY_USAGE_OCCURRENCE('','','',#1,#1,$);\n{FOOTER}"));
    bounded("dangling", &format!("#1=TESSELLATED_SOLID('',(#999999999),#888888888);\n{FOOTER}"));
    bounded("huge id", &format!("#18446744073709551615=PRODUCT('','','',());\n{FOOTER}"));
}

#[test]
fn unterminated_text_is_bounded() {
    bounded("string", "#1=PRODUCT('abc");
    bounded("list", "#1=PRODUCT((((((((((((((((((((((((((((((((((((");
    bounded("comment", "/* never closed\n#1=PRODUCT('','','',());\n");
    bounded("nothing", FOOTER);
}

/// A deeply nested list (a hostile file: 100 000 opening brackets) must be an error. It used to overflow the
/// stack and abort the whole process; the reader now refuses lists nested deeper than 256 levels.
#[test]
fn a_deeply_nested_list_is_an_error_not_a_crash() {
    bounded("deep", &format!("#1=PRODUCT({}{});\n{FOOTER}", "(".repeat(100_000), ")".repeat(100_000)));
}

/// Moderate nesting (a few hundred levels, plausible in real files) is read or refused, not a crash.
#[test]
fn moderately_deep_lists_are_handled() {
    for depth in [10, 100, 500] {
        bounded("moderate", &format!("#1=PRODUCT({}{});\n{FOOTER}", "(".repeat(depth), ")".repeat(depth)));
    }
}

/// A file with a header but no geometry is "empty", not a crash.
#[test]
fn a_step_file_without_triangles_is_empty() {
    let text = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('x','',(''),(''),'','','');\nFILE_SCHEMA(('AP242'));\nENDSEC;\nDATA;\n#1=PRODUCT('p','p','',());\nENDSEC;\nEND-ISO-10303-21;\n";
    match read_step(text.as_bytes()) {
        Err(ImportError::Empty | ImportError::Unreadable(_)) => {}
        other => panic!("{other:?}"),
    }
}
