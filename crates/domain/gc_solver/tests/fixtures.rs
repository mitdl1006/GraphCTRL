//! Acceptance fixtures of the solver kernel (F1-F20 in docs/PLAN.md, appendix C), except the drag
//! fixtures, which live in `drag.rs`.

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: exact values are compared and failures should panic"
)]

use gc_solver::{
    AddOutcome, Blame, Constraint, ConstraintId, DepKind, Line2, P2, Round, SolveReport, Status, System,
};

/// A sketch under construction, optionally scaled and shifted to test invariance.
struct Sketch {
    sys: System,
    next: u64,
    scale: f64,
    shift: f64,
}

impl Sketch {
    fn new() -> Self {
        Self::transformed(1.0, 0.0)
    }

    fn transformed(scale: f64, shift: f64) -> Self {
        Self { sys: System::new(), next: 0, scale, shift }
    }

    fn pt(&mut self, x: f64, y: f64) -> P2 {
        self.sys.add_point2(x * self.scale + self.shift, y * self.scale + self.shift)
    }

    /// A length in the sketch's units.
    fn len(&self, value: f64) -> f64 {
        value * self.scale
    }

    fn id(&mut self) -> ConstraintId {
        self.next += 1;
        ConstraintId(self.next)
    }

    fn add(&mut self, constraint: Constraint) -> ConstraintId {
        let id = self.id();
        self.sys.add(id, constraint);
        id
    }

    fn fix(&mut self, point: P2) -> ConstraintId {
        self.add(Constraint::Fix(point.to_vec()))
    }

    fn try_add(&mut self, constraint: Constraint) -> (ConstraintId, AddOutcome) {
        let id = self.id();
        (id, self.sys.try_add(id, constraint))
    }

    fn solved(&mut self) -> SolveReport {
        let report = self.sys.solve();
        assert_eq!(report.status, Status::Solved, "{report:?}");
        assert!(self.sys.violation() <= 1.0, "violation {}", self.sys.violation());
        report
    }

    /// Position in untransformed units.
    fn at(&self, point: P2) -> [f64; 2] {
        self.sys.point2(point).map(|v| (v - self.shift) / self.scale)
    }
}

fn line(a: P2, b: P2) -> Line2 {
    Line2 { a, b }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn constraints_in(with: &[Blame]) -> Vec<ConstraintId> {
    let mut ids: Vec<ConstraintId> = with
        .iter()
        .map(|b| match b {
            Blame::Row(c, _) | Blame::Merged(c) => *c,
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The transforms of F18: as drawn, scaled down and up, and far from the origin.
const TRANSFORMS: [(f64, f64); 4] = [(1.0, 0.0), (1e-3, 0.0), (1e3, 0.0), (1.0, 1e6)];

#[test]
fn f1_f2_parallel_after_two_horizontals_is_redundant() {
    for (scale, shift) in TRANSFORMS {
        let mut s = Sketch::transformed(scale, shift);
        let (a, b, c, d) = (s.pt(0.0, 0.0), s.pt(10.0, 0.3), s.pt(0.0, 5.0), s.pt(10.0, 5.2));
        s.fix(a);
        let h1 = s.add(Constraint::Horizontal(a, b));
        let h2 = s.add(Constraint::Horizontal(c, d));
        let parallel = s.add(Constraint::Parallel(line(a, b), line(c, d)));
        let report = s.solved();
        assert_eq!(report.dof, 4, "scale {scale} shift {shift}");
        let deps: Vec<_> = report.dependencies_of(parallel).collect();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].kind, DepKind::Redundant);
        let with = constraints_in(&deps[0].with);
        assert!(with.contains(&h1) && with.contains(&h2), "{with:?}");
        assert_eq!(s.at(b)[1], 0.0);
        assert_eq!(s.sys.value(d[1]), s.sys.value(c[1]), "merged coordinates are bit-identical");

        // F2: a second Horizontal on the same line.
        let (_, outcome) = s.try_add(Constraint::Horizontal(a, b));
        let AddOutcome::Redundant { with } = outcome else { panic!("{outcome:?}") };
        assert_eq!(constraints_in(&with), vec![h1]);
        assert_eq!(s.solved().dof, 4);
    }
}

#[test]
fn f3_triangle_inequality_violation_names_the_conflicting_lengths() {
    for (scale, shift) in TRANSFORMS {
        let mut s = Sketch::transformed(scale, shift);
        let (a, b, c) = (s.pt(0.0, 0.0), s.pt(10.0, 0.0), s.pt(7.6, 1.8));
        s.fix(a);
        let ab = s.add(Constraint::Distance { p: a, q: b, value: s.len(10.0) });
        let bc = s.add(Constraint::Distance { p: b, q: c, value: s.len(3.0) });
        assert_eq!(s.solved().dof, 2);
        let before = s.sys.clone();
        let candidate = Constraint::Distance { p: a, q: c, value: s.len(4.0) };
        let (id, outcome) = s.try_add(candidate.clone());
        let AddOutcome::Conflicting { with } = outcome else { panic!("{outcome:?}") };
        assert_eq!(constraints_in(&with), vec![ab, bc], "scale {scale} shift {shift}");
        for p in [a, b, c] {
            assert_eq!(s.sys.point2(p), before.point2(p), "a refused constraint changes nothing");
        }
        assert!(s.sys.constraint(id).is_none());

        // Bulk solve with the conflict present.
        s.sys.add(id, candidate);
        let report = s.sys.solve();
        assert_eq!(report.status, Status::Inconsistent);
        let conflict: Vec<_> =
            report.dependencies.iter().filter(|d| d.kind == DepKind::Conflicting).collect();
        assert_eq!(conflict.len(), 1);
        assert_eq!(conflict[0].row.0, id, "the newest constraint is the one blamed");
        assert_eq!(constraints_in(&conflict[0].with), vec![ab, bc]);
        assert_eq!(s.sys.point2(c), before.point2(c), "a failed solve changes nothing");
    }
}

#[test]
fn f4_flat_start_is_solved_after_a_nudge() {
    for fixed in [true, false] {
        for (scale, shift) in TRANSFORMS {
            let mut s = Sketch::transformed(scale, shift);
            let (a, b, c) = (s.pt(0.0, 0.0), s.pt(3.0, 0.0), s.pt(7.0, 0.0));
            if fixed {
                s.fix(a);
            }
            s.add(Constraint::Distance { p: a, q: b, value: s.len(3.0) });
            s.add(Constraint::Distance { p: b, q: c, value: s.len(4.0) });
            s.add(Constraint::Distance { p: a, q: c, value: s.len(5.0) });
            let report = s.solved();
            assert_eq!(report.dof, if fixed { 1 } else { 3 });
            assert!(report.dependencies.is_empty(), "{:?}", report.dependencies);
            let (pa, pb, pc) = (s.at(a), s.at(b), s.at(c));
            let area = ((pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0])).abs() / 2.0;
            assert!((area - 6.0).abs() < 1e-6, "area {area}");
        }
    }
}

#[test]
fn f4_the_side_chosen_by_the_nudge_is_deterministic() {
    let build = || {
        let mut s = Sketch::new();
        let (a, b, c) = (s.pt(0.0, 0.0), s.pt(3.0, 0.0), s.pt(7.0, 0.0));
        s.fix(a);
        s.add(Constraint::Distance { p: a, q: b, value: 3.0 });
        s.add(Constraint::Distance { p: b, q: c, value: 4.0 });
        s.add(Constraint::Distance { p: a, q: c, value: 5.0 });
        s.solved();
        (s.at(b), s.at(c))
    };
    assert_eq!(build(), build());
}

#[test]
fn f5_a_collapsed_line_is_degenerate_not_an_error() {
    let mut s = Sketch::new();
    let (p1, p2, q) = (s.pt(0.0, 0.0), s.pt(4.0, 1.0), s.pt(2.0, 2.0));
    let c1 = s.add(Constraint::Coincident(q, p1));
    let c2 = s.add(Constraint::Coincident(q, p2));
    assert_eq!(s.solved().dof, 2);
    assert_eq!(s.sys.point2(p1), s.sys.point2(p2));

    let (_, outcome) = s.try_add(Constraint::Distance { p: p1, q: p2, value: 5.0 });
    let AddOutcome::Conflicting { with } = outcome else { panic!("{outcome:?}") };
    assert_eq!(constraints_in(&with), vec![c1, c2]);

    let (_, outcome) = s.try_add(Constraint::Horizontal(p1, p2));
    assert!(matches!(outcome, AddOutcome::Redundant { .. }), "{outcome:?}");

    // An angle against a collapsed line has no meaning; it is reported, not turned into a failure.
    let (r1, r2) = (s.pt(10.0, 0.0), s.pt(12.0, 3.0));
    let angle =
        s.add(Constraint::Angle { l1: line(p1, p2), rev1: false, l2: line(r1, r2), rev2: false, value: 1.0 });
    let report = s.solved();
    assert!(report.degenerate.contains(&angle));
}

#[test]
fn f6_distance_between_fixed_points() {
    let mut s = Sketch::new();
    let (a, b) = (s.pt(0.0, 0.0), s.pt(7.0, 0.0));
    let fix_a = s.fix(a);
    let fix_b = s.fix(b);
    assert_eq!(s.solved().dof, 0);
    let (_, outcome) = s.try_add(Constraint::Distance { p: a, q: b, value: 10.0 });
    let AddOutcome::Conflicting { with } = outcome else { panic!("{outcome:?}") };
    assert_eq!(constraints_in(&with), vec![fix_a, fix_b]);
    let (_, outcome) = s.try_add(Constraint::Distance { p: a, q: b, value: 7.0 });
    let AddOutcome::Redundant { with } = outcome else { panic!("{outcome:?}") };
    assert_eq!(constraints_in(&with), vec![fix_a, fix_b]);
}

#[test]
fn f7_vertical_distance_on_a_horizontal_line_conflicts() {
    let mut s = Sketch::new();
    let (a, b) = (s.pt(0.0, 0.0), s.pt(10.0, 0.4));
    let h = s.add(Constraint::Horizontal(a, b));
    assert_eq!(s.solved().dof, 3);
    let (_, outcome) = s.try_add(Constraint::VDistance { p: a, q: b, sign: 1.0, value: 5.0 });
    let AddOutcome::Conflicting { with } = outcome else { panic!("{outcome:?}") };
    assert_eq!(constraints_in(&with), vec![h]);

    // The review's other example: a horizontal distance against an aligned distance on a horizontal line.
    let d = s.add(Constraint::Distance { p: a, q: b, value: 10.0 });
    s.solved();
    let (_, outcome) = s.try_add(Constraint::HDistance { p: a, q: b, sign: 1.0, value: 7.0 });
    let AddOutcome::Conflicting { with } = outcome else { panic!("{outcome:?}") };
    assert!(constraints_in(&with).contains(&d), "{with:?}");
}

#[test]
fn f8_the_fourth_right_angle_of_a_quad_is_redundant() {
    for (scale, shift) in TRANSFORMS {
        let mut s = Sketch::transformed(scale, shift);
        let p = [s.pt(0.0, 0.0), s.pt(10.0, 0.9), s.pt(9.2, 6.1), s.pt(-0.7, 5.2)];
        let side = |i: usize| line(p[i], p[(i + 1) % 4]);
        let ids: Vec<ConstraintId> =
            (0..4).map(|i| s.add(Constraint::Perpendicular(side(i), side((i + 1) % 4)))).collect();
        let report = s.solved();
        assert_eq!(report.dof, 5, "scale {scale} shift {shift}");
        assert_eq!(report.dependencies.len(), 1, "{:?}", report.dependencies);
        let dependency = &report.dependencies[0];
        assert_eq!((dependency.row.0, dependency.kind), (ids[3], DepKind::Redundant));
        assert_eq!(constraints_in(&dependency.with), ids[..3].to_vec());
    }
}

#[test]
fn f11_angles_solve_on_every_branch_including_the_cut() {
    for degrees in [0.0, 1e-6, 90.0, 179.999_999, 180.0, 270.0] {
        for start_offset in [170.0_f64, 180.0] {
            let theta = f64::to_radians(degrees);
            let start = theta + start_offset.to_radians();
            let mut s = Sketch::new();
            let (o1, x1) = (s.pt(0.0, 0.0), s.pt(10.0, 0.0));
            let (o2, q) = (s.pt(0.0, 0.0), s.pt(10.0 * libm::cos(start), 10.0 * libm::sin(start)));
            s.fix(o1);
            s.fix(x1);
            s.fix(o2);
            s.add(Constraint::Distance { p: o2, q, value: 10.0 });
            s.add(Constraint::Angle {
                l1: line(o1, x1),
                rev1: false,
                l2: line(o2, q),
                rev2: false,
                value: theta,
            });
            let report = s.solved();
            assert_eq!(report.dof, 0, "theta {degrees} start +{start_offset}");
            assert!(report.dependencies.is_empty());
            let got = s.at(q);
            let want = [10.0 * libm::cos(theta), 10.0 * libm::sin(theta)];
            assert!(dist(got, want) < 1e-7, "theta {degrees} start +{start_offset}: {got:?} vs {want:?}");
        }
    }
}

#[test]
fn f12_a_point_at_the_centre_of_its_circle_moves_onto_it() {
    let mut s = Sketch::new();
    let centre = s.pt(0.0, 0.0);
    let radius = s.sys.add_radius(5.0);
    s.fix(centre);
    s.add(Constraint::Radius { round: Round { c: centre, r: radius }, value: 5.0 });
    let p = s.pt(0.0, 0.0);
    s.add(Constraint::PointOnCircle(p, Round { c: centre, r: radius }));
    let report = s.solved();
    assert_eq!(report.dof, 1);
    let at = s.at(p);
    assert!(at.iter().all(|v| v.is_finite()));
    assert!((dist(at, [0.0, 0.0]) - 5.0).abs() < 1e-9);
}

#[test]
fn f14_four_bar_linkage() {
    let build = |grounded: bool| {
        let mut s = Sketch::new();
        let p = [s.pt(0.0, 0.0), s.pt(4.0, 0.0), s.pt(5.0, 3.0), s.pt(1.0, 2.5)];
        if grounded {
            s.fix(p[0]);
            s.fix(p[1]);
        }
        for i in 0..4 {
            let (a, b) = (p[i], p[(i + 1) % 4]);
            let value = dist(s.at(a), s.at(b));
            s.add(Constraint::Distance { p: a, q: b, value });
        }
        s.solved()
    };
    assert_eq!(build(false).dof, 4);
    let grounded = build(true);
    assert_eq!(grounded.dof, 1);
    // The bar between the two fixed points is implied by them.
    assert_eq!(grounded.dependencies.len(), 1);
    assert_eq!(grounded.dependencies[0].kind, DepKind::Redundant);
}

#[test]
fn f15_a_constraint_that_only_bites_at_this_position_is_added_as_singular() {
    let mut s = Sketch::new();
    let centre = s.pt(0.0, 0.0);
    let radius = s.sys.add_radius(1.0);
    s.fix(centre);
    s.add(Constraint::Radius { round: Round { c: centre, r: radius }, value: 1.0 });
    let p = s.pt(0.0, 1.0);
    s.add(Constraint::PointOnCircle(p, Round { c: centre, r: radius }));
    assert_eq!(s.solved().dof, 1);
    // P is at the top of the circle; pinning its height there leaves no real freedom.
    let (id, outcome) = s.try_add(Constraint::CoordValue { param: p[1], value: 1.0 });
    assert!(matches!(outcome, AddOutcome::AddedSingular(_)), "{outcome:?}");
    assert!(s.sys.constraint(id).is_some());

    // Contrast: pinning the same height twice is plainly redundant.
    let (_, outcome) = s.try_add(Constraint::CoordValue { param: p[1], value: 1.0 });
    assert!(matches!(outcome, AddOutcome::Redundant { .. }), "{outcome:?}");
}

#[test]
fn f19_short_lines_become_parallel_to_a_microradian() {
    let mut s = Sketch::new();
    // Large geometry elsewhere in the sketch.
    let (far_a, far_b) = (s.pt(0.0, 0.0), s.pt(1000.0, 0.0));
    s.add(Constraint::Distance { p: far_a, q: far_b, value: 1000.0 });
    let five_degrees = f64::to_radians(5.0);
    let (a, b) = (s.pt(500.0, 500.0), s.pt(500.1, 500.0));
    let (c, d) = (
        s.pt(500.0, 500.2),
        s.pt(500.0 + 0.1 * libm::cos(five_degrees), 500.2 + 0.1 * libm::sin(five_degrees)),
    );
    s.add(Constraint::Parallel(line(a, b), line(c, d)));
    s.solved();
    let direction = |p: P2, q: P2, s: &Sketch| {
        let (p, q) = (s.at(p), s.at(q));
        libm::atan2(q[1] - p[1], q[0] - p[0])
    };
    let misalignment = (direction(a, b, &s) - direction(c, d, &s)).abs();
    assert!(misalignment <= 1e-6, "misalignment {misalignment}");
}

#[test]
fn f20_an_arc_with_a_radius_dimension_has_four_degrees_of_freedom() {
    let mut s = Sketch::new();
    let (c, start, end) = (s.pt(0.0, 0.0), s.pt(5.0, 0.0), s.pt(0.0, 5.2));
    let radius = s.sys.add_radius(5.0);
    let round = Round { c, r: radius };
    s.add(Constraint::Arc { round, start, end });
    assert_eq!(s.solved().dof, 5);
    let (_, outcome) = s.try_add(Constraint::Radius { round, value: 6.0 });
    let AddOutcome::Added(report) = outcome else { panic!("{outcome:?}") };
    assert_eq!(report.dof, 4);
    assert!(report.dependencies.is_empty());
    assert!((dist(s.at(c), s.at(start)) - 6.0).abs() < 1e-9);
    assert!((dist(s.at(c), s.at(end)) - 6.0).abs() < 1e-9);
}

#[test]
fn g7_solving_a_solved_system_changes_no_bit() {
    let mut s = Sketch::new();
    let p = [s.pt(0.0, 0.0), s.pt(10.0, 0.3), s.pt(9.7, 5.2), s.pt(0.4, 5.0)];
    s.fix(p[0]);
    s.add(Constraint::Horizontal(p[0], p[1]));
    s.add(Constraint::Vertical(p[1], p[2]));
    s.add(Constraint::Horizontal(p[2], p[3]));
    s.add(Constraint::Vertical(p[3], p[0]));
    s.add(Constraint::Distance { p: p[0], q: p[2], value: 13.0 });
    let first = s.solved();
    assert!(first.changed);
    let snapshot: Vec<[u64; 2]> = p.iter().map(|q| s.sys.point2(*q).map(f64::to_bits)).collect();
    let second = s.solved();
    assert!(!second.changed);
    let again: Vec<[u64; 2]> = p.iter().map(|q| s.sys.point2(*q).map(f64::to_bits)).collect();
    assert_eq!(snapshot, again);
    assert_eq!(first.dof, second.dof);
}

#[test]
fn mobility_tells_fixed_from_free_parameters() {
    let mut s = Sketch::new();
    let (a, b, c) = (s.pt(0.0, 0.0), s.pt(10.0, 0.5), s.pt(3.0, 4.0));
    s.fix(a);
    s.add(Constraint::Horizontal(a, b));
    let report = s.solved();
    assert!(!report.is_mobile(a[0]) && !report.is_mobile(a[1]));
    assert!(report.is_mobile(b[0]) && !report.is_mobile(b[1]));
    assert!(report.is_mobile(c[0]) && report.is_mobile(c[1]));
    s.add(Constraint::Distance { p: a, q: b, value: 10.0 });
    let report = s.solved();
    assert!(!report.is_mobile(b[0]));
    assert_eq!(report.dof, 2);
}

#[test]
fn dimension_edits_follow_continuously_and_restore_on_failure() {
    let mut s = Sketch::new();
    let (a, b, c) = (s.pt(0.0, 0.0), s.pt(10.0, 0.0), s.pt(4.0, 3.0));
    s.fix(a);
    s.fix(b);
    let ac = s.add(Constraint::Distance { p: a, q: c, value: 5.0 });
    s.add(Constraint::Distance { p: b, q: c, value: 6.708_203_932_499_369 });
    s.solved();
    assert!(s.at(c)[1] > 0.0);
    // Shrinking a side keeps the triangle on the same side of AB.
    let report = s.sys.set_dimension(ac, 4.0);
    assert_eq!(report.status, Status::Solved);
    assert!(s.at(c)[1] > 0.0);
    assert!((dist(s.at(a), s.at(c)) - 4.0).abs() < 1e-9);
    // An impossible value is refused and everything, including the dimension, is restored.
    let before = s.at(c);
    let report = s.sys.set_dimension(ac, 1.0);
    assert_eq!(report.status, Status::Inconsistent);
    assert_eq!(s.at(c), before);
    assert_eq!(s.sys.constraint(ac).and_then(Constraint::value), Some(4.0));
}

#[test]
fn three_d_points_use_the_same_machinery() {
    let mut sys = System::new();
    let origin = sys.add_point3(0.0, 0.0, 0.0);
    let p = sys.add_point3(1.0, 2.0, 2.5);
    sys.add(ConstraintId(1), Constraint::Fix(origin.to_vec()));
    assert_eq!(sys.solve().dof, 3);
    assert!(matches!(
        sys.try_add(ConstraintId(2), Constraint::Distance3 { p: origin, q: p, value: 2.0 }),
        AddOutcome::Added(ref r) if r.dof == 2
    ));
    // On the XY plane: one more freedom gone.
    assert!(matches!(
        sys.try_add(ConstraintId(3), Constraint::CoordValue { param: p[2], value: 0.0 }),
        AddOutcome::Added(ref r) if r.dof == 1
    ));
    let at = sys.point3(p);
    assert!(at[2].abs() < 1e-12 && ((at[0] * at[0] + at[1] * at[1]).sqrt() - 2.0).abs() < 1e-9);

    // Alternatively along X from the origin: fully determined.
    let mut sys2 = System::new();
    let origin = sys2.add_point3(0.0, 0.0, 0.0);
    let p = sys2.add_point3(1.0, 0.3, -0.2);
    sys2.add(ConstraintId(1), Constraint::Fix(origin.to_vec()));
    sys2.add(ConstraintId(2), Constraint::Distance3 { p: origin, q: p, value: 2.0 });
    sys2.add(ConstraintId(3), Constraint::AlongAxis { p: origin, q: p, axis: 0 });
    let report = sys2.solve();
    assert_eq!((report.status, report.dof), (Status::Solved, 0));
    let at = sys2.point3(p);
    assert!((at[0] - 2.0).abs() < 1e-9);
    assert_eq!((at[1], at[2]), (0.0, 0.0), "substituted coordinates are exact");
}
