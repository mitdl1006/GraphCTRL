//! Per-frame cost of building what the viewport draws (`Scene::of`) and of a pick ray, for a cell of N
//! components (a robot and boxes). Target, with the session queries of `gc_session`'s `bench_scene`: the
//! whole frame's CPU preparation within 2 ms at 50 components (PLAN 5.2).
//!
//! `cargo run --release -p gc_scene --example bench_draw [-- --components N] [--frames N]`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, reason = "a bench tool")]

use std::time::Instant;

use gc_doc::{Placement, Solid};
use gc_session::Session;

fn argument(args: &[String], name: &str, default: usize) -> usize {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let components = argument(&args, "--components", 50);
    let frames = argument(&args, "--frames", 200);
    let urdf =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/urdf/six_axis.urdf"))
            .expect("the six-axis fixture");
    let mut session = Session::new();
    session.add_robot("Robot1", &urdf).unwrap();
    #[allow(clippy::cast_precision_loss, reason = "small counts")]
    for i in 0..components.saturating_sub(1) {
        let (row, column) = ((i / 8) as f64, (i % 8) as f64);
        let placement = Placement { xyz: [1.0 + 0.4 * column, -1.5 + 0.4 * row, 0.3], rpy: [0.0; 3] };
        session.add_body(Solid::Box { size: [0.2, 0.2, 0.2] }, placement).unwrap();
    }
    let mut scene_ms = Vec::with_capacity(frames);
    let mut pick_ms = Vec::with_capacity(frames);
    let mut picker = gc_scene::Picker::default();
    let ray = gc_math::Ray {
        origin: gc_math::DVec3::new(2.0, -1.5, 3.0),
        dir: gc_math::DVec3::new(0.0, 0.0, -1.0),
    };
    for _ in 0..frames {
        let t = Instant::now();
        let scene = gc_scene::Scene::of(&session);
        scene_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        std::hint::black_box(picker.pick(&scene, &ray));
        pick_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let p95 = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "index"
        )]
        let i = ((v.len() as f64) * 0.95) as usize;
        v.get(i.min(v.len().saturating_sub(1))).copied().unwrap_or(0.0)
    };
    println!(
        "| {components} components | Scene::of p95 {:.3} ms | pick p95 {:.3} ms |",
        p95(&mut scene_ms),
        p95(&mut pick_ms)
    );
}
