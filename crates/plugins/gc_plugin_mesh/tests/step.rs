//! Reading a STEP AP242 file with tessellated shapes from `fixtures/step` (written by hand for these
//! tests).

#![allow(clippy::unwrap_used, clippy::float_cmp, reason = "test code")]

use gc_plugin_mesh::{ImportError, read_step};

fn fixture() -> Vec<u8> {
    let path = format!("{}/../../../fixtures/step/two_blocks_ap242.stp", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap()
}

#[test]
fn an_ap242_tessellated_assembly_keeps_its_tree_placements_units_and_colour() {
    let assembly = read_step(&fixture()).unwrap();
    // One mesh (the block), shown by two uses under the cell.
    assert_eq!(assembly.meshes.len(), 1);
    let mesh = &assembly.meshes[0];
    assert_eq!(mesh.name, "Block body");
    assert_eq!(mesh.indices.len(), 36, "twelve triangles");
    assert_eq!(mesh.color, [0.8, 0.1, 0.1, 1.0]);
    // Millimetres to metres: the cube is 20 mm.
    let max_z = mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
    assert!((max_z - 0.02).abs() < 1e-6, "{max_z}");
    let names: Vec<&str> = assembly.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["Cell", "Block:1", "Block:2"]);
    assert_eq!(assembly.nodes[1].parent, Some(0));
    assert_eq!(assembly.nodes[2].parent, Some(0));
    assert!(assembly.nodes[0].meshes.is_empty());
    assert_eq!(assembly.nodes[1].meshes, vec![0]);
    assert_eq!(assembly.nodes[2].meshes, vec![0]);
    assert_eq!(assembly.instanced_triangles(), 24);
    // The second use: 100 mm along X, turned a quarter about Z (its X along the cell's Y).
    let m = assembly.world_matrix(2);
    assert!((m[3][0] - 0.1).abs() < 1e-12 && m[3][1].abs() < 1e-12);
    assert!(m[0][0].abs() < 1e-12 && (m[0][1] - 1.0).abs() < 1e-12, "{m:?}");
    assert_eq!(assembly.world_matrix(1)[3], [0.0, 0.0, 0.0, 1.0]);
    assert!(assembly.warnings.is_empty(), "{:?}", assembly.warnings);
}

#[test]
fn files_that_are_not_step_or_have_no_tessellation_are_refused() {
    assert!(matches!(read_step(b"solid ascii stl"), Err(ImportError::Unreadable(_))));
    let text = String::from_utf8(fixture()).unwrap();
    // Without the tessellated representation there are no triangles.
    let without: String = text.lines().filter(|l| !l.starts_with("#73=")).collect::<Vec<_>>().join("\n");
    assert_eq!(read_step(without.as_bytes()), Err(ImportError::Empty));
    // A damaged instance is reported, not a panic.
    let broken = text.replace("#71=TRIANGULATED_FACE('',#70,0,", "#71=TRIANGULATED_FACE('',#70,0");
    assert!(matches!(read_step(broken.as_bytes()), Err(ImportError::Unreadable(_))));
}

/// A file of one tessellated surface set and no product structure, in metres.
fn surface_file(points: &[[f64; 3]], body: &str) -> String {
    let coords: Vec<String> = points.iter().map(|p| format!("({:?},{:?},{:?})", p[0], p[1], p[2])).collect();
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF'));\nENDSEC;\nDATA;\n\
         #1=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.));\n\
         #2=COORDINATES_LIST('',{},({}));\n#3={body};\n\
         #4=TESSELLATED_SHAPE_REPRESENTATION('Sheet',(#3),$);\nENDSEC;\nEND-ISO-10303-21;\n",
        points.len(),
        coords.join(",")
    )
}

#[test]
fn strips_fans_and_point_indices_without_a_product_tree() {
    let square = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.5, 0.5, 1.0]];
    // pnindex maps 1..=5 onto the points backwards; a strip of the square and a fan round the apex.
    let body = "COMPLEX_TRIANGULATED_SURFACE_SET('',#2,5,(),(5,4,3,2,1),((5,4,2,3)),((1,5,4,3,2,5)))";
    let assembly = read_step(surface_file(&square, body).as_bytes()).unwrap();
    assert_eq!(assembly.nodes.len(), 1);
    assert_eq!(assembly.nodes[0].name, "Sheet");
    assert_eq!(assembly.meshes[0].indices.len() / 3, 2 + 4);
    assert_eq!(assembly.meshes[0].positions[4], [0.5, 0.5, 1.0]);
}

#[test]
fn a_large_tessellated_file_reads_whole() {
    // A grid of 200 × 200 squares: 80,000 triangles, about 4 MB of text.
    let n = 200usize;
    #[allow(clippy::cast_precision_loss, reason = "small grid")]
    let points: Vec<[f64; 3]> =
        (0..=n).flat_map(|i| (0..=n).map(move |j| [i as f64 * 0.01, j as f64 * 0.01, 0.0])).collect();
    let at = |i: usize, j: usize| i * (n + 1) + j + 1;
    let triangles: Vec<String> = (0..n)
        .flat_map(|i| {
            (0..n).flat_map(move |j| {
                [
                    format!("({},{},{})", at(i, j), at(i + 1, j), at(i + 1, j + 1)),
                    format!("({},{},{})", at(i, j), at(i + 1, j + 1), at(i, j + 1)),
                ]
            })
        })
        .collect();
    let body = format!("TRIANGULATED_SURFACE_SET('',#2,0,(),(),({}))", triangles.join(","));
    let text = surface_file(&points, &body);
    let start = std::time::Instant::now();
    let assembly = read_step(text.as_bytes()).unwrap();
    eprintln!("{} bytes, {} triangles in {:?}", text.len(), assembly.instanced_triangles(), start.elapsed());
    assert_eq!(assembly.instanced_triangles(), 2 * n * n);
}
