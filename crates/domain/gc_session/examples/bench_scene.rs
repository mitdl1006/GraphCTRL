//! Per-frame cost of the session queries the viewport and the browser make, for a cell of 50 components
//! (a robot and bodies, some tied by assembly constraints). Target: the whole set within 2 ms per frame.
//!
//! `cargo run --release -p gc_session --example bench_scene [-- --components N] [--frames N] [--append FILE]`
//! (`cargo xtask bench-quality` runs it and appends the table to the quality report).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, reason = "a bench tool")]

use std::time::Instant;

use gc_doc::{AxisSense, Geometry, Occurrence, Placement, RelationKind, Relationship, Selection, Solid};
use gc_session::{CellItem, Session, Shown};

const BUDGET_MS: f64 = 2.0;

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
    if let Some(file) = args.iter().position(|a| a == "--append").and_then(|i| args.get(i + 1)) {
        let mut text = std::fs::read_to_string(file).unwrap_or_default();
        text.push_str(&table);
        std::fs::write(file, text).expect("the report file");
    }
}
