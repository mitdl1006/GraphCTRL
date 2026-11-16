# Geometric constraint solver for GraphCTRL: build-versus-buy and specification\n\n**Verdict'
2 22068 '# Adversarial review of the constraint-solver report\n\nTags: **[V]** read today in the cite'
=====SOLVER REPORT=====
# Geometric constraint solver for GraphCTRL: build-versus-buy and specification

**Verdict: build an in-house solver crate on faer 0.24; do not depend on ezpz.** Nothing was compiled or run in this phase, so the Rust sketch in §6 is uncompiled and no performance figure here is measured.

Legend: **[V]** = verified today from the named primary source; **[R]** = recalled from memory, not re-checked.

## 1. ezpz 0.2.29 (read from `static.crates.io/crates/ezpz/ezpz-0.2.29.crate`)

| Aspect | Finding |
|---|---|
| Scope | README: "This is a 2D constraint solver". Variables are bare `Id = u32` and must be dense column indices (`Layout::index_of(var) = var as usize`). Entities: `DatumPoint{x_id,y_id}`, `DatumLineSegment`, `DatumCircle{center, radius}`, `DatumCircularArc{center,start,end}` (CCW), `DatumDistance`. [V `src/datatypes/inputs.rs`, `src/solver.rs`] |
| Constraints | 25 variants, `#[non_exhaustive]`: LineTangentToCircle, CircleTangentToCircle, Distance, DistanceVar, Vertical/HorizontalDistance, Vertical, Horizontal, LinesAtAngle(Parallel/Perpendicular/Other), Fixed, ScalarEqual, PointsCoincident, CircleRadius, LinesEqualLength, ArcRadius, Arc, Midpoint, PointLineDistance (+ vertical/horizontal), Symmetric, PointArcCoincident, ArcLength, ArcAngle, PointsAtAngle. [V `src/constraints.rs:37-93`] |
| Missing for us | Arc–line and arc–arc tangency (tangent takes `DatumCircle` only; arcs have no radius variable), collinear, diameter, driven dimensions, anything 3D. |
| Numerics | Levenberg–Marquardt on normal equations `(JᵀJ+λI)d = −Jᵀr` with faer sparse Cholesky (symbolic factor precomputed). λ₀ = 1e-9, ×0.1 on accept, ×10 on reject. Defaults: 35 iterations, residual ∞-norm 1e-8, step 1e-12; "satisfied" means residual < 1e-4. No component partitioning, no equality substitution. [V `src/solver/newton.rs`] |
| Jacobian | Hand-written analytic partials (`jacobian_rows`, about 1,300 lines), checked against central differences in proptests. [V] |
| Under-constrained | `solve_analysis` densifies J, runs column-pivoted QR (rank tolerance 1e-8·max diagonal of R), builds a null space and returns only `underconstrained: Vec<Id>`. No DOF count is exposed. [V `src/solver/find_dof.rs`] |
| Over-constrained | No redundancy or conflict analysis. Only `unsatisfied: Vec<usize>` after a least-squares solve, which spreads a conflict over every participant. [V `src/lib.rs`] |
| Priorities | Integer tiers; each tier re-solves all constraints of priority ≤ tier from the original guesses. Per-request `weight`. [V `src/lib.rs:196-234`] |
| Branches | Initial guesses are mandatory; tangent side is inferred from them (`set_from_initial_values`); point–line distance is signed. [V] |
| Dragging | No API; `solve()` is stateless and rebuilds the model each call. Zoo's own host does drag outside the solver: kcl-lib 0.2.196 `src/frontend.rs` says "Drag anchors add hidden fixed cursor points" and keeps a `point_freedom_cache`. [V] |
| Errors and panics | `Result<SolveOutcome, FailureOutcome>`; `NonLinearSystemError` exposes faer error types publicly. Panicking paths exist (`unwrap()` in `refresh_jacobian`, `assert_eq!` in the LM loop, `unreachable!`). No `unsafe` in the solver files. [V] |
| Determinism | faer built without rayon, `libm` for trig, priorities sorted, so run-to-run deterministic. Bit-identity across CPUs is unverified. |
| Dependencies | faer ^0.24.0 (std, sparse-linalg), indexmap, libm, mutants, thiserror ^2, winnow ^1.0 (text-format parser, not optional). [V crates.io API] |
| Tests | 53 scenario tests, about 15 proptests, mutation-testing annotations, benches up to 600 variables. [V] |
| Stability | Published as `kcl-ezpz` (22 versions, 2025-10 to 2026-03), renamed `ezpz` (12 releases 0.2.18–0.2.29, 2026-03-07 to 2026-08-05, none since). Upstream `main` is still 0.2.29 with the same 25 constraints. One reverse dependency: kcl-lib. [V] |

ezpz is a well-tested batch 2D solver, but it lacks four things milestone 1 needs: a DOF count, naming of conflicting or redundant constraints, driven dimensions, and persistent drag state. It also has no route to 3D, and its closed enum cannot be extended by us.

## 2. Other Rust options today

| Option | Facts | Verdict |
|---|---|---|
| `slvs` 0.6.0 | 2023-06-20, GPL-3.0; wraps SolveSpace (GPL v3, `COPYING.txt`). [V] | Reject: licence, C++ build, stale. |
| planegcs port | crates.io search for "planegcs" returns nothing; the original is LGPL-2.1-or-later C++. [V] | None exists. |
| `isotope` 0.0.7 | Description: "Isotope scans AWS services and makes suggestions…". [V] | Not a CAD solver; the brief's premise is wrong. |
| `sketch-solver` 0.1.0 | 2026-10-07, MIT OR Apache-2.0, only dependency libm, one release, 590 downloads. 2D; dense Householder QR; LM plus continuation; reports DOF, free coordinates with "mobility", redundant, conflicts and flips. [V README, `src/report.rs`] | Best permissive design reference. Too young, stateless, O(n³), 2D only. Code not audited. |
| `brepkit-sketch` 4.11.26 | AGPL-3.0-only; 302 versions since 2026-08-07. [V] | Reject: licence. |
| `arael-sketch-solver` 0.9.3, `vcad-kernel-constraints` 0.10.0, `acs-solver` 0.1.19, `laman` 0.3.0, `cadcraft-constraints` 1.4.7, `runmat-constraint` 0.7.1 | All first published in 2026. They pull their own stacks (nalgebra 0.33, wgpu ^30, kernel crates); runmat's licence is "non-standard" with 9 downloads. [V metadata only] | Not dependable. |
| NLLS crates | `levenberg-marquardt` 0.15 needs nalgebra ^0.34; `gomez` 0.5.1 needs ^0.32; `tiny-solver` 0.18.10 and `factrs` 0.3.0 need faer ^0.23 + nalgebra ^0.34 + num-dual 0.12/0.13; `argmin` 0.11 needs argmin-math; `ceres-solver` 0.5.1 is a C++ binding. [V] | They duplicate linear-algebra stacks, and none offers rank or conflict diagnosis or drag semantics. |
| Autodiff | `num-dual` 0.15.1: only mandatory dependency is num-traits, nalgebra ^0.35 optional. `autodiff` 0.7.0 is from 2023. `ad_trait` 0.3.1 pulls ndarray, serde, once_cell. [V] | Write a fixed-size dual of about 120 lines. |
| Linear algebra | faer 0.24.4 has dense column-pivoted QR, full-pivot LU, SVD, and sparse Cholesky/LU/QR with AMD/COLAMD; default features include rayon. nalgebra 0.35 has `col_piv_qr`, `full_piv_lu`, `svd`; nalgebra-sparse 0.12 has only Cholesky. [V file lists] | faer with `default-features = false`. |

## 3. How the reference solvers work

**SolveSpace** [V `system.cpp`, `constrainteq.cpp`, `expr.cpp`, `entity.cpp`, `generate.cpp` at master]:

1. Constraints and entities emit symbolic `Expr` equations. `Expr::PartialWrt` is a recursive chain rule over PLUS, MINUS, TIMES, DIV, SQRT, SQUARE, SIN, COS, ASIN, ACOS, followed by constant folding.
2. `SolveBySubstitution` removes every equation of the literal form `a − b = 0` by merging parameters, keeping a dragged parameter as representative.
3. Equations with a single unknown are solved alone first.
4. The rank test is `rank(J)` by sparse QR; `dof = n − rank`; redundant if `rank < m`.
5. Newton runs up to 50 iterations to |Fᵢ| ≤ `LENGTH_EPS/100` (LENGTH_EPS = 1e-6 is [R]). A guard rejects NaN or |x| > 1e11.
6. Reference dimensions emit no equations; their value is recomputed afterwards (`ModifyToSatisfy`).
7. Dragging: the UI writes the cursor into the dragged parameters, and `MarkDraggedParams` flags them so the minimum-norm step prefers moving other parameters.

```cpp
// system.cpp — drag weighting and minimum-norm Newton step
if(IsDragged(mat.param[c])) scale[c] = 1 / 20.0;     // column scale, undone on X afterwards
// m <= n: x = A'(AA')^-1 B, via QR of A' ("squaring the condition number ... makes large models fail")
solver.compute(At);  rank = solver.rank();
w = R.triangularView<Upper>().transpose().solve(rhs.topRows(rank));
v.topRows(rank) = R.triangularView<Upper>().solve(w);
mat.X = mat.A.num.transpose() * (solver.colsPermutation() * v);
// NewtonSolve: relax = 1, 1/2, ... (8 tries) until |F|^2 decreases, else the full step
// FindWhichToRemoveToFixJacobian: for each constraint c, rebuild the equations without c:
if(rank == mat.m) bad->Add(&(c->h));                 // removing c restores full row rank
// MarkParamsFree: drop one parameter column at a time; if rank is still m, it is free
```

**planegcs** [V `GCS.cpp`, `Constraints.cpp`, `SubSystem.cpp`, `Sketch.cpp` at main]:

- **Model.** Parameters are `double*`. Each constraint class has `error()`, an analytic `grad(param)`, a `scale` (`rescale`), a step limiter `maxStep` (angles capped at π/18), a tag (0 = never blamed, −1 = temporary) and a `driving` flag.
- **`initSolution`.** Diagnose, drop redundant and driven constraints, split into connected components (boost), eliminate `Equal` constraints by merging parameters, and build two subsystems per component (tag ≥ 0 and tag < 0).
- **Algorithms.** Default is DogLeg (`defaultSolver(GCS::DogLeg)`, Gauss step by full-pivot LU). Tolerances: `convergence 1e-10`, `maxIter 100`, `smallF 1e-20`. `Sketch::solve` falls back DogLeg → LM → BFGS → SQP with "stay at the initial value" temporary constraints.
- **Two-level solve.** When temporary constraints exist, it minimises their error subject to the real constraints (SQP with BFGS Hessian and L1-merit line search).
- **Dragging.** `initMove` refuses if conflicts exist, adds temporary coincident constraints to copies of the grabbed points (weak ones rescaled by 0.01), and re-solves per mouse move.
- **Diagnosis.** Dense full-pivot QR below `autoQRThreshold` (constructor default 1000; the code comment says about 200), else sparse QR, because "SparseQR is known to fail rank detection on specific geometric structures".

```cpp
// GCS.cpp diagnose(): QR of J^T (columns = constraints), R reduced to echelon form
for (int j = rank; j < constrNum; j++) {             // every dependent constraint
    for (int row = 0; row < rank; row++)
        if (fabs(R(row, j)) > 1e-10) conflictGroups[j - rank].push_back(/* pivot constraint */);
    conflictGroups[j - rank].push_back(/* constraint j */);
}
// choose constraints to skip (in most groups, then fewest equations per tag, then newest),
// solve without them, then classify each skipped one:
return (err * err < this->convergenceRedundant);     // still satisfied => redundant, else conflicting
// The same reduction on QR of J yields groups of parameters that still move (dependent params).
```

- **Arcs.** `Arc{center, rad, startAngle, endAngle, start, end}`; `addConstraintArcRules` ties the endpoints with two `CurveValue` constraints (9 parameters − 4 equations = 5 DOF).

**D-Cubed 2D DCM** ([R], plus the review arXiv 2202.13795): graph-constructive decomposition. Rigid clusters are found by triconnected or triangle-pattern rules and solved as a sequence of small closed-form problems, choosing the root nearest the current shape, with a numeric fallback for non-decomposable parts. Treat this as a later optimisation; components plus substitution give most of the benefit.

Licence hygiene: SolveSpace is GPL-3 and planegcs is LGPL, so re-implement from the mathematics and do not translate code. ezpz and sketch-solver are permissive.

## 4. Formulations

Conventions: d = b − a; × is the 2D cross product; L₀ is a length frozen at solve start; σ is a sign stored when the constraint is created. Every residual is in length units, so one length tolerance applies.

| Constraint | Residual(s) | Pitfalls and precedent |
|---|---|---|
| Coincident | p − q (2 rows) | Eliminate by substitution. |
| Point on line | (d × (p−a)) / \|d\| | Zero-length line is degenerate. SolveSpace instead adds an unknown t: a + t·d − p = 0. |
| Point on circle/arc | \|p−c\| − r | Gradient undefined at p = c. Arc range is not clamped (ezpz clamps, which is non-smooth). |
| Midpoint | m − (a+b)/2 (2 rows, linear) | |
| Horizontal / vertical | a.y − b.y / a.x − b.x | Substitution. |
| Parallel / perpendicular | (d₁ × d₂)/L₀ / (d₁ · d₂)/L₀ | Also satisfied by a collapsed line. Frozen scale as planegcs `rescale`. |
| Collinear | both endpoints of l₂ on l₁ (2 rows) | |
| Tangent line–circle | σ·(d × (c−a))/\|d\| − r | σ from the drawing (ezpz `LineSide`, planegcs `ccw`); never use an absolute value. |
| Tangent circle–circle | \|c₁−c₂\| − (r₁+r₂), or − σ(r₁−r₂) for internal | Mode fixed at creation. Concentric is singular (planegcs switches to r₁ − r₂). |
| Tangent at shared endpoint | coincident + (d · (p−c))/L₀; arc–arc: ((p−c₁) × (p−c₂))/L₀ | SolveSpace ARC_LINE_TANGENT / CURVE_CURVE_TANGENT. Check for a side flip afterwards. |
| Equal length / radius / concentric | \|d₁\| − \|d₂\| / r₁ − r₂ / c₁ − c₂ | The last two by substitution. |
| Symmetric about a line | ((q−p)·d)/L₀ and (d×(p−a) + d×(q−a))/L₀ | SolveSpace form; polynomial. |
| Fix | remove the parameters from the unknowns | No rows. |
| Distance | \|p−q\| − D | p = q with D > 0 gives 0/0, so nudge deterministically. D = 0 becomes coincident. |
| Horizontal / vertical distance | (q.x − p.x) − σD | Linear. |
| Point–line distance | (d × (p−a))/\|d\| − σD | Signed, which "stops the solver from randomly flipping" (ezpz comment). |
| Line–line distance | parallel + point–line distance | ezpz `parallel_lines_distance`. |
| Angle | L₀·atan2(d₁ × R(−θ)d₂, d₁ · R(−θ)d₂) | Directed lines plus a stored supplement flag. planegcs uses this atan2 form. SolveSpace uses cos∠ − cos θ with an `other` flag, which loses rank near 0°/180° and needs a gain hack. Rewrite 0/90/180 as parallel/perpendicular (ezpz lints this). |
| Radius / diameter | r − R / 2r − D | |

**Arc parameterisation.**
- SolveSpace, ezpz and sketch-solver use centre + start + end with one implicit equation |s−c| = |e−c| (SolveSpace omits it when the endpoints are constrained coincident).
- planegcs uses centre, radius and two angles plus dependent endpoints.
- Recommended: centre + start + end, CCW. Endpoints are real points, there is no angle wrapping, and radius and sweep are derived.

**3D analogues.**
- Distance: |p−q| − D.
- Point on plane: n·(p−o), minus h for a plane distance.
- Point on line: a + t·d − p = 0 with auxiliary unknown t (three rows, net two).
- Parallel: a − λb = 0 with auxiliary λ (SolveSpace `VectorsParallel3d`).
- Perpendicular: (a·b)/L₀².
- Along X/Y/Z: two linear rows, by substitution.
- Line versus plane: n·d = 0 (parallel) or d − λn = 0 (perpendicular).
- Coplanarity: each point on a plane entity.
- Angle: atan2(|a×b|, a·b) − θ.
- Arc–line tangency: only for arcs in a workplane (in-plane rows plus the 2D form).
- Orientation: SolveSpace stores a quaternion (4 parameters) plus the row |q| − 1 = 0 [V `entity.cpp`]. Start with that for 6-DoF waypoints, renormalising after each accepted step and keeping the sign continuous. A tangent-space update is the later upgrade.

## 5. Interactive behaviour (specification)

1. **Preprocess** (cached until the structure changes).
   - Drop fixed parameters.
   - Merge parameters for exact `a − b = 0` rows with union-find; the representative is the grabbed parameter, else the lowest id.
   - Split into connected components.
   - Order rows by class (internal, geometric, dimensional), then by age.
2. **Select independent rows.** Normalise rows, then orthogonalise in that fixed order. A row whose remainder is ≤ 1e-8 is dependent, and its coefficients name the earlier constraints it depends on.
3. **Solve independent rows only.** LM step from QR of the stacked [J; √λ·D], with no normal equations. Accept on decrease; λ ×0.1 / ×10. D = 1, or 20 for grabbed parameters. A small λ keeps the step near minimum-norm, so free geometry stays put. Up to 50 iterations.
4. **Classify dependent rows at the result.** |r| ≤ 1e-7·S means redundant, otherwise conflicting, where S is the sketch size. Since ordering puts the newest last, the just-added dimension is the one blamed, and the UI can offer to make it driven.
5. **Diagnose** on structural change or drag end, not every frame. Column-pivoted QR of J gives rank and DOF = n − rank. The null-space basis gives a per-parameter mobility; an entity is fully constrained when all its parameters have mobility near zero.
6. **Drag.** Write the cursor into the grabbed parameters and warm-start from the previous frame. On failure restore the last good frame. Later, replace the weighting with an exact null-space projection of the cursor goal (the planegcs two-level idea).
7. **Branch keeping.** Signed formulations with stored flags, a post-solve orientation check, and continuation for large dimension edits (sketch-solver steps the value down to 1/1024).
8. **Tolerances.** Residual max(1e-9, 1e-10·S); rank 1e-8 relative; degenerate length 1e-9·S.
9. **Performance target.** A warm-started component solve in ≤ 2 ms; only the grabbed component is re-solved; dense algebra up to about 300 unknowns per component, sparse Cholesky beyond.
10. **Jacobians.** Exact forward-mode duals; finite differences only as a test oracle.

**Tests.**
- AD Jacobian against central differences, per constraint.
- Random consistent sketches (generate geometry, derive constraints, perturb) reach residual ≤ tolerance without flips.
- Rigid-transform and scale invariance of solution, rank and DOF.
- DOF table: point 2, segment 4, three-length triangle 3, circle 3, arc 5, four-bar linkage 1.
- Redundancy and conflict fixtures.
- Drag continuity.
- Bit-identical reruns.
- Differential tests against ezpz and sketch-solver as dev-dependencies.
- Fuzzing that must never panic.

## 6. Rust design

Layout:
- `gctrl-solver`: pure maths. Modules `scalar`, `param`, `constraint`, `system`, `solve`, `diagnose`, `linalg` (a facade over faer). Dependencies: faer and thiserror.
- `gctrl-sketch`: entities with stable ids, dimensions with driving/driven mode, capture of sign flags, serialisation.
- UI crates see only `gctrl-sketch`.

Built-in constraints are one enum with exhaustive matches. A boxed trait object for plugin constraints can be added later.

```rust
#![forbid(unsafe_code)]
use core::ops::{Add, Div, Mul, Neg, Sub};

/// Residual code is generic over this: f64 for values, Dual for exact Jacobian rows.
pub trait Scalar: Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self>
    + Div<Output = Self> + Neg<Output = Self> {
    fn cst(v: f64) -> Self;
    fn re(self) -> f64;
    fn sqrt(self) -> Self;
    fn atan2(self, x: Self) -> Self;
}

/// Forward-mode dual over the (at most N) local variables of one constraint. No heap, Copy.
#[derive(Clone, Copy, Debug)]
pub struct Dual<const N: usize> { pub re: f64, pub eps: [f64; N] }

impl<const N: usize> Mul for Dual<N> {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut eps = [0.0; N];
        for i in 0..N { eps[i] = self.re * o.eps[i] + self.eps[i] * o.re; }
        Dual { re: self.re * o.re, eps }
    }
}
// Add, Sub, Div, Neg and `impl Scalar` follow the same pattern.

macro_rules! id { ($($n:ident),*) => {$(
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)] pub struct $n(u32);
)*} }
id!(ParamId, ConstraintId);

#[derive(Clone, Copy, Debug)] pub struct Point2 { pub x: ParamId, pub y: ParamId }
#[derive(Clone, Copy, Debug)] pub struct Line2 { pub a: Point2, pub b: Point2 }
#[derive(Clone, Copy, Debug)] pub struct Circle2 { pub c: Point2, pub r: ParamId }
#[derive(Clone, Copy, Debug)] pub struct Arc2 { pub c: Point2, pub s: Point2, pub e: Point2 } // CCW
#[derive(Clone, Copy, Debug)] pub enum Side { Left, Right }

#[derive(Clone, Copy, Debug)]
pub enum Constraint {
    Coincident(Point2, Point2),
    PointOnLine(Point2, Line2),
    PointOnCircle(Point2, Circle2),
    Midpoint(Point2, Line2),
    Horizontal(Line2),
    Vertical(Line2),
    Parallel(Line2, Line2),
    Perpendicular(Line2, Line2),
    TangentLineCircle(Line2, Circle2, Side),
    EqualLength(Line2, Line2),
    Symmetric(Point2, Point2, Line2),
    Distance(Point2, Point2, f64),
    PointLineDistance(Point2, Line2, f64), // signed: positive = left of a -> b
    Angle(Line2, Line2, f64),              // directed, radians
    Radius(Circle2, f64),
    ArcRule(Arc2),                         // internal: |s - c| = |e - c|
    // 3D variants (Distance3, PointOnPlane, Parallel3 with an aux param, UnitQuat) go here.
}

/// Hands out variables. The f64 impl reads values; the dual impl gives each ParamId a local
/// slot on first use and seeds eps[slot] = 1, so residual code cannot mis-index a column.
pub trait Eval<S: Scalar> {
    fn var(&mut self, p: ParamId) -> S;
    fn pt(&mut self, p: Point2) -> [S; 2] { [self.var(p.x), self.var(p.y)] }
}

fn sub<S: Scalar>(a: [S; 2], b: [S; 2]) -> [S; 2] { [a[0] - b[0], a[1] - b[1]] }
fn dot<S: Scalar>(a: [S; 2], b: [S; 2]) -> S { a[0] * b[0] + a[1] * b[1] }
fn cross<S: Scalar>(a: [S; 2], b: [S; 2]) -> S { a[0] * b[1] - a[1] * b[0] }
fn norm<S: Scalar>(a: [S; 2]) -> S { dot(a, a).sqrt() }
fn dir<S: Scalar>(e: &mut impl Eval<S>, l: Line2) -> [S; 2] { let a = e.pt(l.a); sub(e.pt(l.b), a) }
fn signed_dist<S: Scalar>(e: &mut impl Eval<S>, p: Point2, l: Line2) -> S {
    let (a, d) = (e.pt(l.a), dir(e, l));
    cross(d, sub(e.pt(p), a)) / norm(d)
}

impl Constraint {
    /// Emits residuals in length units; `len` is the characteristic length frozen at solve start.
    pub fn residuals<S: Scalar>(&self, e: &mut impl Eval<S>, len: f64, mut out: impl FnMut(S)) {
        let k = S::cst(1.0 / len);
        match *self {
            Constraint::Coincident(p, q) => { let d = sub(e.pt(p), e.pt(q)); out(d[0]); out(d[1]); }
            Constraint::PointOnLine(p, l) => out(signed_dist(e, p, l)),
            Constraint::PointOnCircle(p, c) => out(norm(sub(e.pt(p), e.pt(c.c))) - e.var(c.r)),
            Constraint::Midpoint(m, l) => {
                let (m, a, b, h) = (e.pt(m), e.pt(l.a), e.pt(l.b), S::cst(0.5));
                out(m[0] - (a[0] + b[0]) * h); out(m[1] - (a[1] + b[1]) * h);
            }
            Constraint::Horizontal(l) => out(e.var(l.a.y) - e.var(l.b.y)),
            Constraint::Vertical(l) => out(e.var(l.a.x) - e.var(l.b.x)),
            Constraint::Parallel(l1, l2) => out(cross(dir(e, l1), dir(e, l2)) * k),
            Constraint::Perpendicular(l1, l2) => out(dot(dir(e, l1), dir(e, l2)) * k),
            Constraint::TangentLineCircle(l, c, side) => {
                let s = match side { Side::Left => 1.0, Side::Right => -1.0 };
                out(signed_dist(e, c.c, l) * S::cst(s) - e.var(c.r));
            }
            Constraint::EqualLength(l1, l2) => out(norm(dir(e, l1)) - norm(dir(e, l2))),
            Constraint::Symmetric(p, q, l) => {
                let (a, d, p, q) = (e.pt(l.a), dir(e, l), e.pt(p), e.pt(q));
                out(dot(sub(q, p), d) * k);
                out((cross(d, sub(p, a)) + cross(d, sub(q, a))) * k);
            }
            Constraint::Distance(p, q, d) => out(norm(sub(e.pt(q), e.pt(p))) - S::cst(d)),
            Constraint::PointLineDistance(p, l, h) => out(signed_dist(e, p, l) - S::cst(h)),
            Constraint::Angle(l1, l2, theta) => {
                let (d1, d2) = (dir(e, l1), dir(e, l2));
                let (s, c) = (S::cst(theta.sin()), S::cst(theta.cos()));
                let r = [c * d2[0] + s * d2[1], c * d2[1] - s * d2[0]]; // d2 rotated by -theta
                out(cross(d1, r).atan2(dot(d1, r)) * S::cst(len));
            }
            Constraint::Radius(c, r) => out(e.var(c.r) - S::cst(r)),
            Constraint::ArcRule(a) => {
                let c = e.pt(a.c);
                out(norm(sub(e.pt(a.s), c)) - norm(sub(e.pt(a.e), c)));
            }
        } // no wildcard arm: a new variant fails to compile until it is handled here
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status { Solved, Inconsistent, NotConverged }

/// A constraint linearly dependent on earlier ones (earlier = higher class, then older).
#[derive(Clone, Debug)]
pub struct Dependency { pub constraint: ConstraintId, pub with: Vec<ConstraintId> }

#[derive(Clone, Debug)]
pub struct SolveOutcome {
    pub status: Status,
    pub dof: u32,
    pub rank: u32,
    pub free: Vec<(ParamId, f64)>,           // mobility in (0, 1], ordered by ParamId
    pub redundant: Vec<Dependency>,          // dependent and satisfied
    pub conflicting: Vec<Dependency>,        // dependent and violated
    pub unsatisfied: Vec<(ConstraintId, f64)>,
    pub degenerate: Vec<ConstraintId>,       // e.g. a zero-length line
    pub iterations: u32,
    pub residual_norm: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum SolveError {
    #[error("constraint {0:?} refers to unknown parameter {1:?}")] UnknownParam(ConstraintId, ParamId),
    #[error("non-finite value while evaluating {0:?}")] NonFinite(ConstraintId),
    #[error("{0} unknowns exceed the limit of {1}")] TooLarge(usize, usize),
}

pub enum AddOutcome { Added(SolveOutcome), Redundant(Dependency), Conflicting(Dependency) }
pub struct DragSession<'s> { sys: &'s mut System, saved: Vec<f64>, grabbed: Vec<ParamId> }

impl System {
    pub fn solve(&mut self, opts: &SolveOptions) -> Result<SolveOutcome, SolveError> { /* §5 */ }
    /// On Redundant/Conflicting nothing changes, so the UI can offer a driven dimension instead.
    pub fn try_add(&mut self, c: Constraint) -> Result<AddOutcome, SolveError> { /* ... */ }
    /// The exclusive borrow forbids structural edits mid-drag; Drop without commit rolls back.
    pub fn drag(&mut self, grabbed: &[ParamId]) -> DragSession<'_> { /* ... */ }
}
```

Determinism rules: index-based `Vec` storage, no `HashMap` iteration, a fixed row order, and faer without rayon.

## Sources

- ezpz: `https://static.crates.io/crates/ezpz/ezpz-0.2.29.crate`, `https://crates.io/api/v1/crates/ezpz`, `https://raw.githubusercontent.com/KittyCAD/ezpz/main/ezpz/Cargo.toml`, [Zoo blog](https://zoo.dev/blog/announcing-solver), kcl-lib 0.2.196 crate.
- SolveSpace: `https://raw.githubusercontent.com/solvespace/solvespace/master/src/` — `system.cpp`, `constrainteq.cpp`, `expr.cpp`, `entity.cpp`, `generate.cpp`, `util.h`; `COPYING.txt`.
- FreeCAD: `https://raw.githubusercontent.com/FreeCAD/FreeCAD/main/src/Mod/Sketcher/App/` — `planegcs/GCS.cpp`, `planegcs/Constraints.cpp`, `planegcs/SubSystem.cpp`, `planegcs/GCS.h`, `Sketch.cpp`.
- Other crates: the crates.io API and the `.crate` archives of sketch-solver 0.1.0, faer 0.24.4, nalgebra 0.35.0, nalgebra-sparse 0.12.0, num-dual 0.15.1, levenberg-marquardt 0.15.0.
- Literature: [A review on geometric constraint solving](https://arxiv.org/pdf/2202.13795), [Owen & Power](https://arxiv.org/pdf/math/0311037).

## Recommendation

1. **Build `gctrl-solver` in-house.** Use faer 0.24 (`default-features = false`, `std`, later `sparse-linalg`) behind a small `linalg` facade, and thiserror.
2. **Use the §4 formulations** (signed, length-scaled, frozen normalisation) with centre + start + end arcs, written once over `Scalar` and differentiated by a fixed-size dual.
3. **Use the §5 pipeline:** substitution and components, ordered independent-row selection, LM via stacked QR, redundant/conflict classification, pivoted-QR DOF and mobility, weighted warm-start drag with rollback.
4. **Milestone 1 scope:** 2D entities and the constraint list above, `SolveOutcome`, `try_add` and `DragSession`. Reserve 3D variants, auxiliary parameters and `UnitQuat` in the enum design without implementing them.
5. **Add ezpz and sketch-solver as dev-dependencies only**, for differential tests and scenario ideas.
6. **Defer** sparse factorisation, exact null-space drag goals, tangent-space orientations and graph-constructive decomposition until profiling or 3D waypoints demand them.

## Risks and unknowns

- Numerical robustness is now ours: rank tolerances at near-singular shapes can make DOF flicker, and unpivoted ordered orthogonalisation is less robust than pivoted QR. Cross-check the two ranks and flag disagreement.
- faer compile time and cross-CPU bit-identity are unverified. If bit-identical results across machines become a requirement, a small own Householder QR is the fallback.
- No performance number here is measured; the 2 ms target is an estimate.
- Weighted-column dragging is a heuristic. Unreachable cursor positions need the rollback path, and the projected method may be needed sooner than planned.
- The quaternion-plus-norm-row choice for waypoints is provisional.
- The 2026 crates were judged from metadata and README only; `sketch-solver`'s code was not audited.
- D-Cubed details and SolveSpace's `LENGTH_EPS` value are recalled; whether `levenberg-marquardt` accepts m < n was not checked.
- GPL/LGPL sources were read for algorithms, so implementation must be written from the mathematics. Test cases translated from them carry licence implications.

## Surprises

- crates.io `isotope` is an AWS advisory tool, not CADmium's sketch solver.
- ezpz exposes neither a DOF count nor any redundant/conflict detection. The `ezpz` crate name is only eight months old (renamed from `kcl-ezpz`) and has had no release since 2026-08-05. Zoo implements dragging in the host, not in the solver.
- ezpz's doc comments for `Vertical` and `Horizontal` are swapped relative to the code, and its public error enum exposes faer types.
- SolveSpace master no longer matches the classic description: Newton now backtracks, and the minimum-norm step is computed by QR of Jᵀ instead of AAᵀ, with a source comment blaming the squared condition number for false "unsolvable" reports.
- FreeCAD defaults to DogLeg, runs two QR factorisations (J and Jᵀ) concurrently for diagnosis, and prefers dense full-pivot QR because Eigen's sparse QR mis-detects rank on some sketches.
- At least eight new Rust sketch-solver crates appeared in 2026, one of them AGPL with 302 releases in three months; none is mature.
- nalgebra-sparse still offers only Cholesky, and `levenberg-marquardt` 0.15 pins nalgebra ^0.34, so it would duplicate nalgebra next to 0.35.
