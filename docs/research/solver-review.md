# Adversarial review of the constraint-solver report

Tags: **[V]** read today in the cited source; **[E]** reproduced today with a throw-away numpy model of the report's §5 pipeline (stdout only, no Rust compiled); **[R]** recalled; **[I]** inferred.

## 1. Fact check

**Confirmed [V].** Nearly every ezpz, SolveSpace, planegcs and crates.io statement holds, including the two surprising ones:
- SolveSpace master takes the minimum-norm step by QR of Aᵀ and backtracks in `NewtonSolve` (`src/system.cpp` L300-335, L407-420).
- planegcs defaults to DogLeg with dense full-pivot QR below 1000 parameters (`GCS.cpp` L485-495, L4847-4852; `Sketch.cpp` L142).
- ezpz: 25 variants, LM on normal equations, λ₀ 1e-9, 35 / 1e-8 / 1e-12, `find_dof.rs` exposing only `underconstrained`, panicking paths (`solver.rs` L416, `newton.rs` L103, `lib.rs` L366), swapped H/V doc comments, 53 tests + 15 proptests, kcl-ezpz history.

**Wrong, unsupported or outdated.**

| # | Claim | Finding |
|---|---|---|
| 1 | ezpz lacks arc–line / arc–arc tangency, collinear, diameter, driven dimensions | Wrong as a capability claim [V]. kcl-lib 0.2.196 builds arc tangency by composition: a hidden radius variable, `DistanceVar(start, center, r)`, `DistanceVar(end, center, r)`, a `DatumCircle{center, radius}` (public fields), then `CircleTangentToCircle` / `LineTangentToCircle` (fn `push_circular_radius_constraints`; tests `test_tangent_line_arc_…`, `test_tangent_arc_arc_…`). Collinear is 2 × `PointLineDistance(p, l, 0.0)`, which kcl-lib uses and which is the report's own formulation. Diameter is `CircleRadius(D/2)`. Driven dimensions need no solver support (the report's own §3 item 6). |
| 2 | "ezpz lacks four things milestone 1 needs" | Two are real: a DOF count and redundancy/conflict naming. Drag is host-side in Zoo's shipping product, and driven dimensions are host-side everywhere. |
| 3 | "The ezpz crate name is only eight months old" | crates.io `created_at` is 2024-11-15 (v0.0.1). The solver moved in at 0.2.18 on 2026-03-07 [V]. |
| 4 | No release since 2026-08-05, read as a risk | kcl-lib 0.2.196 (2026-11-09, weekly releases) requires `ezpz ^0.2.29`; 94.8k recent downloads [V]. Quiet, not abandoned. |
| 5 | "Rewrite 0/90/180 as parallel/perpendicular (ezpz lints this)" | ezpz only warns (`ShouldBeParallel`). Its angle residual is the cross form `u × R⁻¹v / mean(|u|,|v|)` (`constraints.rs` L614-637), which cannot tell θ from θ+180°. The advice does not transfer to an atan2 residual (A10). |
| 6 | "Frozen scale as planegcs `rescale`" | planegcs scales parallel/perpendicular per constraint by 1/(|d₁||d₂|) (`Constraints.cpp` L1155-1162, L1255-1262). The report's code divides by one global `len` (A8). |
| 7 | Tolerances in §5.8 | They are sketch-solver's constants verbatim (`src/solve.rs` L51-62: 1e-10·size, rank 1e-8, conflict 1e-7, min step 1/1024). FreeCAD's rank pivot threshold is 1e-13 (`GCS.cpp` L495, L5119): five orders apart, undiscussed. |
| 8 | "Dense up to ~300 unknowns, sparse Cholesky beyond" | Contradicts §5.3 ("no normal equations") and the SolveSpace comment the report quotes. faer's sparse QR uses only `qr::no_pivoting` kernels and exposes no rank (`faer-0.24.4/src/sparse/linalg/qr.rs`) [V], so faer offers no sparse rank diagnosis. |
| 9 | DOF table: "four-bar linkage 1" beside "three-length triangle 3" | Inconsistent convention. Ungrounded four-bar is 4; grounded is 1. |
| 10 | sketch-solver "code not audited"; `SolveOutcome` as specified | I read it [V]. `Solution.flipped: Vec<Orientation>` has `ArcSweep, AngleBranch, RadiusSign, TangentSide, Corner` (`src/report.rs` L46-65). It has an explicit zero-gradient-row case (`solve.rs` L885), no drag or weight API, and 21 `#[test]`s in the package. The report's `SolveOutcome` has no flip field although §5.7 demands orientation checks. |
| 11 | "faer built without rayon ⇒ deterministic" | Holds only until any crate in the graph enables faer's defaults (`std, rayon, sparse-linalg, rand, npy`) [V]; Cargo unifies features. |
| 12 | Still unverified | `LENGTH_EPS` (not in the headers I grepped), planegcs `smallF`, the "about 200" comment, and the Zoo blog (zoo.dev is outside this session's network allow-list). |

**Missed [V].**
- planegcs has a third class, *partially redundant* (`GCS.cpp` L5682-5703).
- ezpz treats degeneracy as "residual 0 + `Warning::Degenerate`" and takes `|radius|` in tangency (`constraints.rs` L520-533, L629-632): an outcome, not an error.
- parry3d 0.31.1 depends on glamx, not nalgebra, so no dense linear algebra arrives for free.
- faer pulls pulp, gemm, nano-gemm, private-gemm-x86, dyn-stack, equator, generativity, reborrow, bytemuck and num-complex.

## 2. Attacks on the formulations and algorithm

**Central flaw.** §5 steps 2–4 decide linear dependence at the start point, solve only the "independent" rows, and classify the dropped ones afterwards. No reference does this [V]:
- SolveSpace solves all equations rank-truncated and tests rank after the solve (`system.cpp` L586-591).
- sketch-solver runs LM on all rows and diagnoses at the result with the consistency test `r_k − Σ c_j r_j` (`solve.rs` L843-925).

| # | Case | What the specified design does | Required instead |
|---|---|---|---|
| A1 | Singular start: A fixed (0,0), B=(3,0), C=(7,0), lengths 3/4/5 | [E] Row 3 has remainder 0 at x₀, is dropped, then |AC|−5 = 2 is reported "conflicting" for a valid triangle. LM on all rows also stalls on the line (r = ∓⅔, ‖Jᵀr‖ ≈ 1e-12). | Never decide dependence at x₀. On a stationary point with r ≠ 0, apply one deterministic nudge along null(J) (seeded by parameter ids, 1e-3·S) and re-solve before reporting a conflict. A 1e-3 nudge solved it in 9 iterations [E]. |
| A2 | Triangle inequality: lengths 10/3/4 from a state satisfying the first two | [E] Rows are independent at x₀ (remainders 1, 0.82, 0.84). LM ends at r = (−1, 1, 1) with rank 2 and Jᵀr = 0. Result: `NotConverged` with `conflicting` empty. | `Inconsistent` means ‖Jᵀr‖ small with ‖r‖ > tol; `NotConverged` means budget exhausted. Diagnose at the limit point; the dependent row with a non-zero `combo` names the conflict set. |
| A3 | Manifold-only dependence: closed quad, four perpendiculars, drawn 5° skewed | [E] Fourth-row remainder is 1.25e-4 at the drawn point ("independent", rank 4, DOF 4) and 2e-10 at the solution (rank 3, DOF 5). Hits file load, paste, undo and dimension edits. | Compute rank, DOF and redundancy at the solution only. Never cache a row selection across frames. |
| A4 | H + H + Parallel; Fix + Fix + Distance | After substitution the row is identically zero, so "normalise rows" is 0/0; a NaN remainder can pass as independent. Blame cannot name Fix/H/Coincident because they own no rows. | Zero-gradient rule: redundant if |r| ≤ tol, else conflicting. Add provenance: each merge and each fixed parameter records its constraint, and blame is the merge path. Detect inconsistent merges in preprocessing. Use weighted union-find (a = b + c) to eliminate H/V distances as well. |
| A5 | New geometric constraint versus an older dimension | Class ordering processes the new constraint first, so the old dimension is blamed. | `try_add` orders the candidate last regardless of class. |
| A6 | Point at a circle centre; zero-length line under `signed_dist`, `EqualLength`, `Angle`; `Distance` between merged points | Dual `sqrt(0)` and `atan2(0,0)` give NaN, so `Err(NonFinite)` fails the whole solve. | Guard on f64 before differentiating, use a deterministic fallback gradient (+x), and list the constraint in `degenerate`. Reserve `SolveError` for programmer errors. |
| A7 | Point coincident to both ends of a line | The line collapses structurally; every direction-based residual is 0/0. | Detect in preprocessing (both endpoints in one class) and report `degenerate=[line]`. A length on it is Conflicting with the two coincidences; H/V on it is Redundant. |
| A8 | Scaling of `(d₁×d₂)/len` | With tol = 1e-10·S, lines of length ℓ pass as parallel up to φ = 1e-10·S²/ℓ². S=1000, ℓ=1 gives 1e-4 rad; ℓ=0.1 gives 1e-2 rad (0.57°). | Per-constraint frozen scale 1/max(|d₁|₀,|d₂|₀), so each residual is a point displacement. Solve in a recentred, 1/S-scaled frame: at 1e7 from the origin one ulp (1.9e-9) exceeds the 1e-9 floor. |
| A9 | Branches outside the zero set | Signed line–circle tangency accepts r < 0. `|c₁−c₂| − (r₁+r₂)` with r₂ < 0 is internal tangency. `d·(p−c)=0` holds for a cusp. Centre+start+end arcs jump 10° → 350° when the end passes the start. | Capture an orientation signature before the solve (radius sign, tangent side, corner turn, sweep half) and compare after. On mismatch run continuation, then report `flipped`. Roll back drag frames that mismatch. |
| A10 | Angles near 0°/180° | atan2 keeps full rank there, so rewriting 180° as "parallel" throws away direction and merges two branches. | Do not rewrite. Handle the ±180° error cut deterministically, use the per-constraint scale (the gradient grows as L₀/|d|), and use `libm::atan2`, not std, for cross-platform bits. |
| A11 | Singular configuration: point on a fixed circle plus a line tangent there | Gradients are parallel, so the new constraint is "Redundant" and DOF reads 1 for a rigid point. | Document DOF as infinitesimal. Before rejecting in `try_add`, re-test rank at a witness point (null-space step, re-projected); a rank increase means `SingularHere`, accepted with a warning. |
| A12 | Drag | See below. | See below. |
| A13 | Two-row constraints | A per-constraint `Dependency` cannot express one dependent row. | Put a row index in the report type and add a "partially redundant" class. |
| A14 | Performance | An (m+n)×n QR per LM iteration at n = 300 is about 1e8 flops; 2 ms is plausible only for n ≲ 100 [I]. | Benchmark at 50/100/300 unknowns before promising. |

**A12, drag in detail.**
- **Path dependence [E].** A free triangle (side 10) with one vertex dragged round a closed loop of radius 3 ends rotated 4.19° with 4 frames, 7.03° with 64 and 7.05° with 512. The result depends on mouse sampling.
- **Lag [E].** With D = 20 the grabbed point trails a reachable cursor by about 0.5 % of each move.
- **Flip of fully constrained geometry [E].** A, B fixed, C = (5,5) fully dimensioned; cursor at (5,−0.5) returns C = (5,−5).
- **Lifetime.** `DragSession<'s>` holding `&'s mut System` cannot be stored in an eframe `App` beside the `System` it borrows.
- **Required.** Use an owned typestate (below). Refuse to grab zero-mobility parameters. Solve "minimise ‖grabbed − cursor‖² subject to the constraints" in milestone 1. Sub-step cursor moves larger than ¼ of the smallest involved length. Replay recorded samples in tests, never wall-clock time.

## 3. Build versus buy

**I would not flip to ezpz; I would nearly flip to sketch-solver.**

The strongest argument against the verdict is that §5 is sketch-solver's pipeline with its exact constants.
- That crate is MIT OR Apache-2.0, libm-only, about 125 KB of source, and already returns `dof`, `free`, `redundant`, `conflicts` and `flipped` [V].
- Its README claims bit-identical results on aarch64/x86_64/wasm32 and validation against FreeCAD- and SolveSpace-derived corpora.
- The report's objections (too young, stateless, O(n³), 2D only) apply equally to a solver that does not exist yet and will also start dense, 2D and younger.
- Drag can live in the host, as Zoo ships on a stateless solver [V].
- Milestone 1 also contains the shell, viewport and sketch tools, and the constraint kernel is its highest numerical risk without being GraphCTRL's differentiator.

I still keep "build", narrowly, for four reasons:
- Evidence of sketch-solver's maturity is thin: one release, 590 downloads, 21 packaged tests, and a web search found no independent trace.
- It has no weights or pins, so drag would be hard `Fix` plus fallback.
- 3D waypoint and path constraints are on the roadmap and no crate offers them, so a dimension-agnostic core must exist eventually.
- Provenance and typestate need API control.

The verdict should rest on those reasons, not on the "missing constraints" list.

## 4. Prioritised corrections

**P0 (correctness)**
1. Solve all rows with rank-tolerant damped least squares; diagnose only at the result with `combo = r_k − Σ c_j r_j` (A1–A3).
2. Separate `Inconsistent` from `NotConverged`; nudge once before declaring a conflict.
3. Add provenance for Fix and merges, the zero-gradient rule, and weighted union-find (A4, A7).
4. Treat degeneracy as an outcome; no residual may emit NaN (A6).
5. Use per-constraint frozen scales and a normalised local frame (A8).
6. Add the orientation signature and a `flipped` field; enforce r > 0 and sweep continuity (A9).
7. Make `try_add` order the candidate last (A5).
8. Use an owned drag typestate with a zero-mobility guard, exact pin and sub-stepping (A12).
9. Fix the DOF table; make the arc radius one row.

**P1**
- Select dependent rows by class-blocked pivoted QR of Jᵀ (pivot inside a class, age as tie-break), with a 1e-10…1e-6 "ill-conditioned" band instead of one threshold.
- Add row-level and partially redundant reporting.
- Add the witness test for singular configurations.
- If faer is used, add a CI check that its `rayon` feature is off.

**P2**
- Sparse algebra and graph decomposition.

## 5. Test fixtures

Drawn coordinates are given; DOF = unknowns − rank at the solution.

| ID | Sketch | DOF | Expected |
|---|---|---|---|
| F1 | A=(0,0) fixed, B=(10,0.3), C=(0,5), D=(10,5.2); H(AB), H(CD), Parallel(AB,CD) | 4 | Solved; Parallel Redundant with {H, H}. |
| F2 | F1 + second H(AB) | 4 | Redundant with {H(AB)}. |
| F3 | A=(0,0) fixed, B=(10,0), C=(7.6,1.8); AB=10, BC=3, then AC=4 | 2 | `try_add` returns Conflicting{AC; AB, BC} and leaves state unchanged. Bulk solve returns `Inconsistent` with the same set. |
| F4 | A=(0,0) fixed, B=(3,0), C=(7,0); AB=3, BC=4, AC=5 | 1 | Solved, area 6, deterministic side, no conflict. |
| F5 | Line P1P2, point Q; Coincident(Q,P1), Coincident(Q,P2) | 2 | Solved, `degenerate=[line]`. Adding length 5 is Conflicting with both; H is Redundant; Angle is reported degenerate with no `Err`. |
| F6 | A=(0,0), B=(7,0) fixed; Distance 10, then 7 | 0 | Conflicting with {Fix A, Fix B}; then Redundant. |
| F7 | Free line AB; H(AB); VerticalDistance(A,B)=5 | 3 | Conflicting with {H}. |
| F8 | Quad (0,0), (10,0.9), (9.2,6.1), (−0.7,5.2); four Perpendicular, bulk solve | 5 | Solved; exactly one Redundant (the newest) with the other three. |
| F9 | Line (0,0)→(10,0) fixed; CCW arc c=(10,5), s≡(10,0), e=(15,5); tangent at s; drag c to (10,−5) | 2 | Every accepted frame keeps cross(d, c−s) > 0 and r > 0; other frames are rolled back or flagged `TangentSide`. |
| F10 | Arc c=(0,0), s=(10,0) fixed, e at 10°; drag e to −10° | 1 | Never a silent 350° arc. |
| F11 | l1 fixed; l2 from fixed origin to Q, length 10; Angle θ ∈ {0, 1e-6°, 90°, 179.999999°, 180°, 270°}; start at θ+170° and θ+180° | 0 | Solved, rank 2 in every case, Q = 10(cos θ, sin θ). |
| F12 | Fixed circle r=5 at origin; P=(0,0); PointOnCircle | 1 | Solved, |P| = 5, no NaN. |
| F13 | Line y=0 fixed; circle c=(3,2), r free; TangentLineCircle(Left); drag c to (3,−2) | 2 | r > 0 and side kept, or frame rejected / `RadiusSign`. |
| F14 | Four-bar | 4 / 1 | Ungrounded 4; two adjacent points fixed 1. |
| F15 | Fixed unit circle; P=(0,1); PointOnCircle, then P.y = 1 | 1 (infinitesimal) | Added with `SingularHere`; not a plain Redundant. |
| F16 | Triangle (0,0), (10,0), (0,10), three lengths; drag A round centre (0,3), radius 3, in 4/64/512 samples | 3 | All frames Solved; area sign constant; lag ≤ tol; identical samples give identical bits. With the exact-pin objective, net rotation is 0° for every count (spec as written: 4.19° / 7.03° / 7.05°). |
| F17 | A=(0,0), B=(10,0) fixed, C=(5,5), AC=BC=√50; drag C to (5,−4) and (30,−1) | 0 | Geometry unchanged bit-for-bit. |
| F18 | F1–F15 scaled ×1e-3 and ×1e3, and translated by (1e6,1e6) | same | Same status, DOF and blame sets. |
| F19 | Two 0.1-long lines 5° apart inside a sketch with S=1000; Parallel | n/a | Final misalignment ≤ 1e-6 rad. |
| F20 | Arc with a Radius dimension | 4 | One row; nothing redundant. |

## Corrected shapes (uncompiled)

```rust
// Owned typestate: storable in an eframe App between frames, no self-borrow.
pub struct Idle(System);
pub struct Dragging { sys: System, start: Vec<f64>, last_good: Vec<f64>,
                      grabbed: Vec<ParamId>, signature: Signature }
impl Idle {
    pub fn try_add(&mut self, c: Constraint) -> AddOutcome { todo!() } // candidate ordered last
    pub fn begin_drag(self, grabbed: &[ParamId]) -> Result<Dragging, (Idle, DragRefused)> { todo!() }
}
impl Dragging {                                   // no add/remove methods in this state
    pub fn update(&mut self, cursor: [f64; 2]) -> FrameOutcome { todo!() }
    pub fn commit(self) -> Idle { todo!() }
    pub fn cancel(self) -> Idle { todo!() }       // restores `start`
}
pub enum Blame { Row(ConstraintId, u8), Fixed(ParamId), Merged(ConstraintId) }
pub enum DepKind { Redundant, Conflicting, SingularHere }
pub struct Dependency { pub row: (ConstraintId, u8), pub with: Vec<Blame>, pub kind: DepKind }
pub enum Status { Solved, Inconsistent, NotConverged, Flipped }
// SolveOutcome gains: flipped: Vec<Orientation>, ill_conditioned: Vec<ConstraintId>
```

```text
solve(x0):
  hard = fixed + weighted union-find (a = b, a = b + c), with provenance
         inconsistent merge -> Conflicting(blame = merge path)
  per component, in a recentred 1/S-scaled frame:
    sig0 = signature(x0)
    x = LM on ALL rows, QR of [J; sqrt(lam) D]; guards keep residuals finite
    |r| <= tol        -> ok
    |J^T r| small     -> nudge once along null(J), retry; else Inconsistent
    otherwise         -> NotConverged
    signature(x) != sig0 -> continuation (halve the edit or cursor step); else Flipped
    diagnose at x: class-blocked pivoted QR of J^T -> rank, dependent rows, coefficients c
       combo = r_k - sum c_j r_j ; |combo| > 1e-7 -> Conflicting, else Redundant
       zero-gradient row: |r_k| > tol -> Conflicting, else Redundant; blame from provenance
    dof = n - rank ; mobility from the null space
```

## Sources

- ezpz 0.2.29, kcl-lib 0.2.196, sketch-solver 0.1.0, faer 0.24.4: `https://static.crates.io/crates/NAME/NAME-VERSION.crate` and `https://crates.io/api/v1/crates/NAME`.
- `https://raw.githubusercontent.com/KittyCAD/ezpz/main/ezpz/`.
- SolveSpace: `https://raw.githubusercontent.com/solvespace/solvespace/master/src/` (`system.cpp`, `constrainteq.cpp`, `entity.cpp`, `expr.cpp`, `util.h`).
- FreeCAD: `https://raw.githubusercontent.com/FreeCAD/FreeCAD/main/src/Mod/Sketcher/App/` (`planegcs/GCS.cpp`, `planegcs/Constraints.cpp`, `Sketch.cpp`).
- Web search hits (no independent mention of sketch-solver found): [ezpz docs](https://docs.rs/ezpz), [arael-sketch-solver source](https://docs.rs/crate/arael-sketch-solver/0.6.2/source/src/lib.rs), [Zoo blog, not reachable from this session](https://zoo.dev/blog/announcing-solver).

## Recommendation

1. **Keep the in-house core, but replace §5 steps 2–4 with the P0 pipeline.** Solve everything, diagnose at the result, and add provenance, guards, signatures and the owned drag typestate. As specified, the pipeline gives a false conflict on F4, an unnamed conflict on F3 and the wrong DOF on F8.
2. **Port, don't re-derive.** sketch-solver is permissive, so adapt its `diagnose`, orientation checks and dense QR with attribution. Keep the GPL/LGPL excerpts in the report out of the repository.
3. **Put the solver behind a backend trait in `gctrl-sketch`** and keep ezpz and sketch-solver as dev-dependency oracles. Time-box the in-house backend against the §5 fixtures; if it misses, ship milestone 1 on a sketch-solver adapter and continue behind the trait.
4. **Start with an in-crate pivoted Householder QR plus `libm`** behind the `linalg` facade, and make faer an optional feature for large components. That is about 200 lines of safe Rust, has no FMA, and gives the same bits on Windows, macOS and Linux. Milestone 1 never needs more than dense algebra on a few hundred unknowns. This reverses the report's default, which lists the in-crate QR only as a fallback.
5. **Adopt F1–F20 as the acceptance gate**, alongside the report's Jacobian, invariance and fuzz tests.

## Risks and unknowns

- Nothing was compiled. The [E] numbers come from a numpy model with finite-difference Jacobians, not from Rust.
- sketch-solver was read, not run. Its behaviour on F4 (flat start) is unknown; I found no nudge in its source, so it may also report a false conflict there [I].
- That faer's runtime SIMD dispatch breaks cross-CPU bit identity is [R]; only its dependency list and features are [V].
- I did not re-derive ezpz's analytic Jacobians.
- The witness test (A11) and the exact-pin drag objective add solver work to milestone 1, and their cost is unmeasured.
- Holonomic drag removes sampling dependence but can jump when the nearest branch changes; it needs F9, F10 and F16 before commitment.
- A rank threshold between 1e-8 and 1e-13 is a judgement call; the ill-conditioned band hides it rather than resolving it.

## Surprises

- The report's "specification" is sketch-solver's design with its exact constants, yet it changes the one step that matters (dropping rows before solving), and that change is what fails in the experiments.
- Zoo implements arc tangency on ezpz by composition with `DistanceVar`; the report's headline "missing" item is not missing.
- SolveSpace master really has changed: QR of Aᵀ, backtracking, and a `RELATION` constraint with parsed expressions.
- parry3d 0.31 has left nalgebra for glamx, so an "already in the tree" argument for any dense linear-algebra crate is gone.
- FreeCAD decides rank at 1e-13; the report and sketch-solver use 1e-8.
- Writing the cursor into a fully constrained point flips it to the mirror solution as soon as the cursor crosses the base line.
- The `ezpz` name was registered in November 2024, sixteen months before the solver was published under it.
