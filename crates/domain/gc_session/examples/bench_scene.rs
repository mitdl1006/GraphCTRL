//! Per-frame cost of the session queries the viewport and the browser make, for a cell of 50 components
//! (a robot and bodies, some tied by assembly constraints). Target: the whole set within 2 ms per frame.
//!
//! Then the large-mesh case (lead's request: equipment files of 100,000 triangles × 20): 20 mesh bodies of
//! that size, tied by mates; a frame's queries, a drag step (constraint solve), a contact-solver drive step,
//! Measure between two meshes and Analyze Interference over all of them.
//!
//! `cargo run --release -p gc_session --example bench_scene [-- --components N] [--frames N] [--meshes N]
//! [--triangles N] [--append FILE]` (`cargo xtask bench-quality` runs it and appends the tables to the
//! quality report).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, reason = "a bench tool")]

use std::time::Instant;

use gc_doc::{AxisSense, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid};
use gc_session::{CellItem, Session, Shown};

const BUDGET_MS: f64 = 2.0;
/// What one interactive step (a drag or drive frame, a measurement) may take before the viewport stutters.
const INTERACTIVE_MS: f64 = 16.0;

fn argument(args: &[String], name: &str, default: usize) -> usize {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn cell(components: usize) -> Session {
    let urdf =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/urdf/six_axis.urdf"))
            .expect("the six-axis fixture");
    let mut session = Session::new();
    let floor = session
        .add_body(Solid::Box { size: [6.0, 6.0, 0.1] }, Placement { xyz: [0.0, 0.0, -0.05], rpy: [0.0; 3] })
        .unwrap();
    session.add_robot("Robot1", &urdf).unwrap();
    // Bodies on a grid around the robot; every fourth one sits on the floor by a mate.
    #[allow(clippy::cast_precision_loss, reason = "small counts")]
    for i in 0..components.saturating_sub(2) {
        let (row, column) = ((i / 8) as f64, (i % 8) as f64);
        let placement = Placement { xyz: [1.0 + 0.4 * column, -1.5 + 0.4 * row, 0.3], rpy: [0.0; 3] };
        let body = session.add_body(Solid::Box { size: [0.2, 0.2, 0.2] }, placement).unwrap();
        if i % 4 == 0 {
            let plane = |z: f64, n: f64| Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] };
            let on = |id: gc_core::Id<gc_doc::Body>, geometry| Selection {
                occurrence: Occurrence::Item(id.erase()),
                geometry,
            };
            session
                .add_relationship(Relationship::new(
                    format!("Mate:{i}"),
                    RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
                    on(body, plane(-0.1, -1.0)),
                    on(floor, plane(0.05, 1.0)),
                ))
                .unwrap();
        }
    }
    session
}

/// One frame's queries: what the scene and the browser ask.
fn frame(session: &Session) -> usize {
    let doc = session.doc();
    let mut touched = 0;
    for (id, _) in doc.bodies().iter() {
        if session.shown_body(id).is_some() && session.shown_look(Shown::Body(id)).is_some() {
            touched += 1;
        }
    }
    for (id, _) in doc.robots().iter() {
        touched += session.link_poses(id).map_or(0, |p| p.len());
        touched += usize::from(session.robot_problem(id).is_some());
        touched += usize::from(session.tcp_pose(id).is_some());
        touched += usize::from(session.selected_item() == Some(CellItem::Robot(id)));
    }
    let statuses = session.component_statuses();
    touched += session
        .components()
        .iter()
        .filter(|n| matches!(n.occurrence, Occurrence::Item(i) if statuses.contains_key(&i)))
        .count();
    touched
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "index"
    )]
    let at = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[at.min(sorted.len() - 1)]
}

/// A closed sphere-like mesh of about `triangles` triangles and radius `r`, as binary STL.
fn sphere_stl(triangles: usize, r: f32) -> Vec<u8> {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a few hundred rings"
    )]
    let n = ((triangles as f64 / 2.0).sqrt().ceil() as usize).max(4);
    #[allow(clippy::cast_precision_loss, reason = "a few hundred rings")]
    let vertex = |i: usize, j: usize| {
        let (theta, phi) =
            (std::f32::consts::PI * i as f32 / n as f32, std::f32::consts::TAU * j as f32 / n as f32);
        [r * theta.sin() * phi.cos(), r * theta.sin() * phi.sin(), r * theta.cos()]
    };
    let mut tris = Vec::with_capacity(2 * n * n);
    for i in 0..n {
        for j in 0..n {
            let (a, b, c, d) = (vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1), vertex(i, j + 1));
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    let mut out = vec![0u8; 80];
    out.extend_from_slice(&u32::try_from(tris.len()).unwrap().to_le_bytes());
    for t in tris {
        out.extend_from_slice(&[0u8; 12]);
        for v in t {
            for c in v {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0u8; 2]);
    }
    out
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed().as_secs_f64() * 1000.0)
}

fn verdict(ms: f64, budget: f64) -> &'static str {
    if ms <= budget { "pass" } else { "FAIL" }
}

/// The large-mesh case: `count` mesh bodies of about `triangles` triangles each.
fn large_meshes(count: usize, triangles: usize, frames: usize) -> String {
    let directory = tempfile::tempdir().expect("a temporary folder");
    let file = directory.path().join("equipment.stl");
    std::fs::write(&file, sphere_stl(triangles, 0.25)).expect("the mesh file");
    let file = file.to_string_lossy().into_owned();
    let mut session = Session::new();
    // As the application runs it: mesh files are read on job threads.
    session.set_executor(gc_jobs::Executor::thread());
    let floor = session
        .add_body(Solid::Box { size: [20.0, 20.0, 0.1] }, Placement { xyz: [0.0, 0.0, -0.05], rpy: [0.0; 3] })
        .unwrap();
    let plane = |z: f64, n: f64| Geometry::Plane { point: [0.0, 0.0, z], normal: [0.0, 0.0, n] };
    let on = |id: gc_core::Id<gc_doc::Body>, geometry| Selection {
        occurrence: Occurrence::Item(id.erase()),
        geometry,
    };
    let (cold_load, load_ms) = timed(|| {
        let mut bodies = Vec::new();
        #[allow(clippy::cast_precision_loss, reason = "small counts")]
        for i in 0..count {
            // A slightly different scale each: separate meshes in memory, as different files would be.
            let solid = Solid::Mesh { file: file.clone(), scale: 1.0 + i as f64 * 1e-3 };
            // The last one overlaps its neighbour, so that interference has something to find.
            let slot = if i + 1 == count && i > 0 { i - 1 } else { i };
            let shift = if slot == i { 0.0 } else { 0.3 };
            let placement = Placement {
                xyz: [(slot % 5) as f64 * 0.6 + shift, (slot / 5) as f64 * 0.6, 0.25],
                rpy: [0.0; 3],
            };
            let body = session.add_body(solid, placement).unwrap();
            // Every body sits on the floor by a mate (its lowest point on the floor's top).
            let low = -0.25 * (1.0 + i as f64 * 1e-3);
            session
                .add_relationship(Relationship::new(
                    format!("Mate:{i}"),
                    RelationKind::Mate { offset: 0.0, sense: AxisSense::Opposed },
                    on(body, plane(low, -1.0)),
                    on(floor, plane(0.05, 1.0)),
                ))
                .unwrap();
            bodies.push(body);
        }
        bodies
    });
    let bodies = cold_load;
    // The window stays live meanwhile: placeholders until the meshes arrive.
    let (_, arrive_ms) = timed(|| {
        while session.meshes_loading() > 0 {
            session.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    let mut times: Vec<f64> =
        (0..frames.max(1)).map(|_| timed(|| std::hint::black_box(frame(&session))).1).collect();
    times.sort_by(f64::total_cmp);
    let frame_p95 = percentile(&times, 0.95);

    // A drag step: one body moved, the mates re-solved, committed.
    let (_, drag_ms) = timed(|| {
        let mut body = session.doc().get(bodies[0]).unwrap().clone();
        body.placement.xyz[0] += 0.05;
        session.preview_body(bodies[0], body);
        session.end_body_preview();
    });
    let (_, solve_ms) = timed(|| std::hint::black_box(session.doc().solve_relationships().is_ok()));

    // A drive step with the contact solver on for all components: the first body slides into the second.
    let slide = session
        .add_relationship(Relationship::new(
            "Flush:slide",
            RelationKind::Flush { offset: 0.0 },
            on(bodies[0], Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Yz))),
            on(floor, Geometry::origin(gc_doc::OriginElement::Plane(gc_doc::OriginPlane::Yz))),
        ))
        .unwrap();
    session.set_contact_solver(gc_doc::ContactSolver::All).unwrap();
    let (_, drive_ms) = timed(|| session.drive_relationship(slide, 0.002).unwrap());
    session.end_drive_relationship();
    // Into the neighbour 0.1 m away in one call (50 steps of 2 mm): it is held by its mate, so the drive
    // stops at contact.
    let (_, into_ms) = timed(|| session.drive_relationship(slide, 0.15).unwrap());
    let stop = session.contact_stop().expect("contact stops the drive");
    assert!(stop.value.is_some_and(|v| (0.09..0.11).contains(&v)), "{stop:?}");
    session.end_drive_relationship();
    session.set_contact_solver(gc_doc::ContactSolver::Off).unwrap();

    let (a, b) = (CellItem::Body(bodies[0]), CellItem::Body(bodies[1]));
    let (measured, measure_ms) = timed(|| session.measure_pair(a, b));
    assert!(measured.and_then(|m| m.minimum).is_some_and(|c| c.distance > 0.05), "a real mesh distance");
    let all: Vec<CellItem> = bodies.iter().map(|b| CellItem::Body(*b)).collect();
    let (rows, interference_ms) = timed(|| session.interference(&all, &[]));
    #[allow(clippy::cast_precision_loss, reason = "small counts")]
    let pairs = (count * count.saturating_sub(1) / 2) as f64;
    format!(
        "\n## Large meshes ({count} mesh bodies × {triangles} triangles, mates to the floor)\n\n\
         | step | time | budget | verdict |\n|---|---|---|---|\n\
         | adding the bodies: blocking (document edits only) | {load_ms:.1} ms | — | — |\n\
         | … then until every mesh is in (background: scale, BVH) | {arrive_ms:.1} ms | — | — |\n\
         | frame queries, p95 of {frames} | {frame_p95:.3} ms | {BUDGET_MS} ms | {} |\n\
         | drag step (move + solve + commit) | {drag_ms:.2} ms | {INTERACTIVE_MS} ms | {} |\n\
         | constraint solve alone | {solve_ms:.2} ms | {INTERACTIVE_MS} ms | {} |\n\
         | drive step with the contact solver (all) | {drive_ms:.2} ms | {INTERACTIVE_MS} ms | {} |\n\
         | drive 0.15 m in one call, stopped by contact | {into_ms:.1} ms | — (a frame plays a few steps) | — |\n\
         | Measure, mesh to mesh | {measure_ms:.2} ms | {INTERACTIVE_MS} ms | {} |\n\
         | Analyze Interference, all {pairs} pairs ({} found) | {interference_ms:.1} ms | command, not per frame | — |\n\n\
         Constraint solving works on poses only, so mesh size does not enter it; the meshes cost in what\n\
         touches their triangles (contact, measure, interference), each on demand.\n",
        verdict(frame_p95, BUDGET_MS),
        verdict(drag_ms, INTERACTIVE_MS),
        verdict(solve_ms, INTERACTIVE_MS),
        verdict(drive_ms, INTERACTIVE_MS),
        verdict(measure_ms, INTERACTIVE_MS),
        rows.len(),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let components = argument(&args, "--components", 50);
    let frames = argument(&args, "--frames", 200);
    let session = cell(components);
    let cold = Instant::now();
    frame(&session);
    let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let mut times: Vec<f64> = (0..frames)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(frame(&session));
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    let (median, p95, max) = (percentile(&times, 0.5), percentile(&times, 0.95), percentile(&times, 1.0));
    let verdict = if p95 <= BUDGET_MS { "pass" } else { "FAIL" };
    let table = format!(
        "\n## Scene queries per frame ({components} components, {frames} frames)\n\n\
         | first frame | median | p95 | max | budget | verdict |\n|---|---|---|---|---|---|\n\
         | {cold_ms:.3} ms | {median:.3} ms | {p95:.3} ms | {max:.3} ms | {BUDGET_MS} ms (p95) | {verdict} |\n\n\
         The session side of a frame: bodies shown, robot link poses, collision state (cached), TCP, and the\n\
         browser's constraint status for every component. The renderer's own draw list is measured separately.\n"
    );
    println!("{table}");
    let meshes =
        large_meshes(argument(&args, "--meshes", 20), argument(&args, "--triangles", 100_000), frames);
    println!("{meshes}");
    if let Some(file) = args.iter().position(|a| a == "--append").and_then(|i| args.get(i + 1)) {
        let mut text = std::fs::read_to_string(file).unwrap_or_default();
        text.push_str(&table);
        text.push_str(&meshes);
        std::fs::write(file, text).expect("the report file");
    }
}
