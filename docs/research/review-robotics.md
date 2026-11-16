# Plan review: robotics and motion planning

## 1. [BLOCKER] 4.1 crate table, 4.3 plugin structure, 5.6 (IK family selection, Refined), 5.10 (configuration hint)

**Problem.** The plugin split cannot be built as written, and the Stage 2/3 agents work on these crates in parallel.

- **Type visibility:** 4.1 says `gc_plugin_ur` depends only on `gc_robotics_api`, and 4.3 allows only serde POD and `&[f64]` at the boundary. 5.6 then specifies `Refined<Ur6rAnalytic>`. `Refined<S>` and `IkSolver` live in `gc_robot`; `Ur6rAnalytic` lives in the plugin. Neither crate can name the other's type.
- **Two traits, no adapter:** `KinematicsSolver` (API) and `IkSolver` (gc_robot) both exist. `gc_robot`'s dependency list does not include `gc_robotics_api`.
- **Family detection needs FK:** the plugin must do the zero-pose axis tests and the 64-random-q FK comparison, but FK is in `gc_robot` and the contents of `ChainSpec` are undefined.
- **UR vocabulary in the document:** 5.10 stores hints as shoulder/elbow/wrist in the generic waypoint model.
- **Planner boundary:** `PlanningScene` is listed as a POD but must expose a parry world. `gc_plan` planners are generic over `StateSpace`. The plan does not say whether `gc_motion` calls `gc_plan` directly or goes through the registry.

**Fix.** Write these interfaces into 4.3 and 5.6 and freeze them in Stage 1.

1. **API types (plain f64):** `Pose { xyz: [f64;3], quat_xyzw: [f64;4] }`; `JointSpec { name, kind: Revolute|Prismatic|Continuous, axis: [f64;3], lower, upper, max_vel: Option<f64>, to_next: Pose }`; `ChainSpec { base_to_first: Pose, joints: Vec<JointSpec>, tip: Pose }`. Fixed joints are collapsed. A chain containing mimic joints gets no analytic family.
2. **One reference FK:** `ChainSpec::fk(&self, q: &[f64]) -> Pose` lives in `gc_robotics_api`. Conformance test: it equals `gc_robot::Chain::fk_into` within 1e-12.
3. **Factory:** `KinematicsFactory { id(); probe(&ChainSpec, Option<&FamilyMetadata>) -> Option<Fit>; build(&ChainSpec) -> Result<Box<dyn KinematicsSolver>> }` with `Fit::{Exact, NearNominal { pos_err, rot_err }}`.
4. **Solver:** `KinematicsSolver: Send + Sync { descriptor() -> &SolverDescriptor; solve_all(target_base_to_tip: &Pose, seed: Option<&[f64]>, out_q: &mut Vec<f64>, out_branch: &mut Vec<u32>) -> Result<usize, IkFailure>; classify(q) -> Option<u32> }`. The descriptor carries completeness and `branch_axes` (key plus labels). Solutions are returned in the canonical turn only.
5. **Host-only code in `gc_robot`:** turn enumeration, limit filtering, mount/TCP adaptation, `Refined`, and verification. The adapter `PluginIk(Box<dyn KinematicsSolver>)` implements `IkSolver`; `Refined<PluginIk>` is used when `Fit != Exact`. Every plugin solution is checked (finite, in limits, FK residual) and dropped with a diagnostic on failure. Add `gc_robotics_api` to `gc_robot`'s dependencies.
6. **Document model:** `ConfigHint::{Auto, Branch { family: String, branch: u32, mask: u32 }, Seed(Vec<f64>), ExactJoints(Vec<f64>)}` plus optional `turns: Vec<i8>`. The UI builds the hint editor from `branch_axes`.
7. **Planner boundary:** `MotionPlanner::plan(&self, req: &PlanRequestPod, validity: &dyn ValidityOracle, cx)`. `ValidityOracle` is a trait object with batch methods `check_states(qs, dof, out)`, `check_motion(a, b)` and `clearance(q)`. `gc_motion` receives planners as `&dyn MotionPlanner` from a `PlannerSet` (default id `core.rrt_connect`) and never names concrete planner types. `gc_plugin_planners` is the only adapter from `gc_plan` generics.
8. **Gate:** a test-only mock plugin with planar 2R analytic IK, written against the API crate alone.

## 2. [MAJOR] 5.6 PlanningProfile (joint weights), 5.8 (state space, goal selection, RRT range)

**Problem.** The default joint weights make the wrist nearly free, which hurts both quality and speed.

- **Weights:** "proportional to the maximum displacement of downstream geometry per radian" gives UR5e wrist 3 roughly 0.04-0.05 of the base weight (flange radius about 0.04 m against about 0.9 m reach). Every UR5e joint is limited to 180 deg/s, so a 2π wrist turn costs 2 s of cycle time but ranks like 0.3 rad of base motion.
- **Goal ranking:** turn variants are chosen almost arbitrarily under that metric.
- **Edge cost:** an RRT step measured in that metric can spin a wrist by several radians, which needs hundreds of 1° checks per edge.
- **Sampling volume:** sampling the full ±2π box is 32 times the volume of one turn. The official `joint_limits.yaml` limits the elbow to ±π because the wider range "leads to planning problems".
- **Goal set:** "top 8 by rank" has no diversity rule. It can hold turn variants of one branch and miss the only collision-free branch.

Q4, Q5 and Q8 are all measured in this metric.

**Fix.** 1. **Cost metric is time-proportional:** `w_j = (1/v_max_j) / max_k(1/v_max_k)`. This is uniform on a UR5e. Reach radii `r_ij` are used only for the edge-check sweep bound. Record the metric and its provenance in `PlanningProfile`.
2. **Steer cap:** `|Δq_j| ≤ 0.5 rad` per extension, independent of weights.
3. **Turn-local sampling:** sample in `bbox(start ∪ goals) ± π`, clamped to limits. On `NoPathWithinBudget`, retry once with the full limits and report which box was used.
4. **Goal-set rule:**
   - Take the turn variant nearest to the start for each IK branch (up to 8).
   - Filter by limits and collision.
   - Rank by `cost(start, g) + hint penalty + λ·Σ max(0, |q_j| − π)`, so sequences do not drift toward ±2π.
   - Fill the remaining slots (up to 8 in total) with the next-best turn variants.
   - Break ties by (branch index, turn tuple).
5. **Reporting:** state the reached goal's branch label, turns and rank.
6. **Tests:**
   - A generic pose yields exactly 256 candidates before filtering (8 branches × 2^5 turn variants, elbow ±π).
   - With all branches valid, the top 8 are 8 distinct branches.
   - Start equal to the goal pose selects the start configuration with cost 0.

## 3. [MAJOR] 5.1 (waypoint model), 5.8 (sequences), 5.10

**Problem.** - **Start state undefined:** waypoints are Cartesian only (`PositionSource`), there is no joint-space waypoint, and nothing says a path starts from the robot instance's joint state.
- **Greedy sequencing:** "the previous segment's end configuration seeds the next" lets a Free/PTP segment pick its goal by distance alone. The following LIN/CIRC must then stay on that branch and turn. It often cannot (wrist singularity, ±2π limit, collision) even though another candidate works. The research report proposed a look-ahead over IK candidates; the plan dropped it.
- **Hints:** they lack turn numbers and seed/exact-joint forms.

**Fix.** 1. **Model:** add `WaypointTarget::{Cartesian { position, orientation }, Joints(Vec<f64>)}` and `Path.start: StartSpec::{CurrentRobotState, NamedPose(id), Waypoint(id)}`. Default is `CurrentRobotState`, snapshotted into the request and the input hash.
2. **Sequence resolution pass in `gc_motion` (P1, before any sampling planner):**
   - Build a candidate set `C_i` of up to 16 per Cartesian waypoint: branches × nearest turns, filtered by limits, collision and hint.
   - For each LIN/CIRC segment and each `c ∈ C_i`, run the Cartesian tracker to get feasible (c, c') pairs with cost.
   - Run a dynamic programme over the sequence. Free/PTP edges cost the straight-line metric; constrained edges must be feasible pairs.
   - Fix one configuration per waypoint. Plan Free segments multi-goal only when the next segment is unconstrained.
3. **Error:** `SequenceInfeasible { segment, tried: Vec<(branch, turns, cause, fraction)> }`.
4. **Test (UR5e):** W2→W3 is a pure tool-Z rotation LIN of +1.2 rad, which changes only q6 by +1.2. The start state makes the nearest W2 candidate have q6 = 5.5 rad.
   - Greedy must give `CartesianIncomplete { JointLimit { wrist_3 } }` at fraction (2π − 5.5)/1.2 = 0.653.
   - The DP must choose q6 = 5.5 − 2π = −0.783 at W2 and complete the sequence.

## 4. [MAJOR] 5.6 (FK, Jacobian, IK, PlanningProfile), 5.8 (state space, LIN/CIRC)

**Problem.** The owner asked for planning from any robot description file, but several rules cover only the UR 6R case.

- **LIN/CIRC:** defined as "follow the branch with analytic IK". No behaviour is specified for chains without an analytic family, which is every non-UR URDF.
- **Jacobian:** given for revolute joints only. Prismatic and mimic columns are missing, mimic limits are not folded into the master's limits, and the 1° edge step has no prismatic unit.
- **Continuous joints:** SO(2) components sit inside a type called `RealVectorSpace { lo, hi }`. Nothing says how wrapped planner output becomes a continuous `q(s)` for `gc_traj`, how turn variants work for unbounded joints, or that Informed RRT* ellipsoid sampling is invalid with wrapped dimensions.
- **DoF below 6:** a 4-DoF SCARA or 5-DoF arm cannot reach an arbitrary 6D pose, so every waypoint fails as `OutOfReach` with no explanation.
- **DoF above 6:** no statement on redundancy.
- **Joints outside the chain:** gripper fingers, rails or a second arm are not said to be frozen and included in collision FK.
- **Planning groups:** the tie rule for several candidate leaves is undefined and there is no group schema.

**Fix.** 1. **Generic Cartesian tracking:** add `IkSolver::track(target, prev_q)`. The default is damped-least-squares continuation from `prev_q`, with a per-sample `‖Δq‖∞` cap and up to 3 bisections. Analytic families override it. The UI shows "IK: analytic, N branches" or "IK: numeric (sampled)".
2. **Jacobian and limits:** prismatic column `Jv = z, Jω = 0`; a mimic joint adds `m·column` to its master. Master limits are intersected with `(limit − o)/m` of each mimic. Edge step is ≤ 1° for revolute and ≤ 10 mm for prismatic, plus the sweep bound.
3. **State space:** rename to `JointSpace { kinds: Bounded { lo, hi } | Wrapping, weights }`.
   - Planners return unwrapped paths (`q_{i+1} = q_i + shortest_diff`).
   - IK for a Wrapping joint returns the representative nearest the seed.
   - Informed sampling is disabled, with a note, when any joint is Wrapping.
4. **Under-actuated targets:** `IkTarget::{Pose6, PositionAxis { free: ToolZ }, Position3}` in `NumericLm`, by projecting the error rows. Defaults by DoF: ≥ 6 uses `Pose6`, 5 uses `PositionAxis`, ≤ 4 uses `Position3` (or position plus yaw when all revolute axes are parallel). Show the mode on the waypoint. Add the error `GoalUnreachable::UnderActuated { dof, mode }`.
5. **Groups:** `PlanningProfile.groups: Vec<Group { name, base_link, tip_link }>` in schema v1. M1 allows one active group per path and shows a chooser when several leaves tie. Non-group joints are frozen at the instance state and included in collision FK and the input hash.
6. **Import capability report:** DoF and joint kinds, IK family and fit, limit provenance (missing velocity blocks timing with a message), groups, and collision shape count.
7. **Fixtures:** hand-written planar 3R, 7R, SCARA RRPR, gantry with a mimic gripper, and one continuous joint.
   - FK equals hand DH within 1e-12.
   - Jacobian equals central differences within 1e-6, including prismatic and mimic.
   - A 200 mm LIN on the 7R deviates ≤ 0.1 mm.
   - Continuous joint 3.0 → −3.0 travels 0.283 rad and unwraps to 3.283.
   - SCARA tool-down waypoint plans; a tilted waypoint returns `UnderActuated`.

## 5. [MAJOR] 5.7 (link shapes, tool body, ACM), 5.1 (robot instance), 5.10

**Problem.** - **Tool is one sentence:** "an extra body on `tool0`" plus a TCP offset. The geometry source, storage, multiple TCPs, the pose conversion and the ACM update are undefined, and TCP editing appears only as a P2 gizmo. Without a tool, the collision statement covers a bare flange.
- **Hull granularity:** "one convex hull per link" merges several `<collision>` elements and primitives into one hull. Two boxes forming an L become a filled wedge. A two-finger gripper hull fills the finger gap.
- **ACM blind spot:** the 95% rule gives no diagnostic for pairs colliding in a mid-range share of samples, which is the signature of hull inflation and blocks large parts of C-space.

**Fix.** 1. **One shape per `<collision>` element:** box → cuboid, sphere → ball, cylinder → cylinder with the Z→Y pose, mesh → hull. A link is a list of (pose, shape).
2. **Meshes with several parts (P1):** one hull per connected component after vertex welding, via union-find. VHACD stays P2.
3. **Tool in `gc_doc` (P1, numeric editing in the properties panel):** `Tool { name, attach_link, mount: Pose, geometry: Vec<(Pose, Mesh | Box | Cylinder | Sphere)>, tcps: Vec<{ name, pose }>, active_tcp }`.
4. **Target conversion, stated once:** `T_base_tip = T_world_base⁻¹ · T_world_target · T_tip_tcp⁻¹`.
5. **Tool ACM:** tool ↔ attach link is always allowed. Attaching or removing a tool starts an incremental ACM job for tool pairs only.
6. **ACM classes:** `Adjacent | Default | Always(≥ 95%) | Never(0/N, measured with padding) | Sometimes(p)`. Warn "hull over-approximation likely" for 20% ≤ p < 95% and highlight the pair.
7. **Tests:**
   - On 2,000 seeded samples, any pair that an exact triangle-triangle check on the STL meshes finds colliding in ≥ 1 and < 95% of samples must remain enabled in the generated ACM.
   - The UR5e ACM golden file is stable across runs.
   - With a 100 mm cylinder tool, tool–wrist_3 is disabled and tool–forearm is enabled.

## 6. [MAJOR] 5.7 (padding), 5.8 (start-state fixing, errors), 5.10, 3.1 step 7

**Problem.** - **Contact is impossible:** a global 5 mm padding with no allowed-contact mechanism turns every process waypoint within 5 mm of a fixture or workpiece into `GoalInCollision`. That is the normal case for waypoints taken from sketch points on a table or fixture plane.
- **Start fixing contradicts the guarantee:** the only relief is the start rule, which moves the start away from where the user put it and prepends a segment that lies inside the padding. That segment cannot honestly become `Trajectory<Validated>`.
- **Self-collision padding:** unspecified.

**Fix.** 1. **Padding is a pair policy:** `pad(robot body, obstacle)` is the robot default or a per-link override. Self-pairs default to 0; state this in 5.7.
2. **Contact allowance:** `ContactAllowance { body: ToolOrLink, obstacle, scope: Waypoint(id) | Segment(id), radius (default 50 mm), allow_touch: bool }`. Inside the radius around the waypoint TCP the pair is checked with padding 0. Penetration stays forbidden unless `allow_touch` is set.
3. **Errors:** split `GoalWithinPadding { link, other, distance }` from `GoalInCollision { link, other, depth }`. The UI offers the allowance in the first case.
4. **Start rule:**
   - Keep the clamp for limit violations ≤ 1e-3 rad.
   - Never move an in-padding start. Plan the first segment with reduced padding until clearance ≥ pad.
   - A penetrating start is an error.
5. **Guarantee record:** `Validated` carries `Guarantee { padding_nominal, padding_min, reduced_intervals, step_deg, step_mm, model, acm_hash }`. The UI shows reduced intervals on the timeline.
6. **Test:** UR5e, table top at z = 0, cylinder tool (r = 10 mm, length 100 mm), TCP at the tip touching the table with tool Z down.
   - Without an allowance: `GoalWithinPadding { distance ≈ 0 }`.
   - With an allowance: the plan succeeds and `padding_min = 0` only inside the 50 mm sphere.

## 7. [MAJOR] 5.7 (edge check, environment), 5.9 (validate), Appendix E Q2

**Problem.** The stated guarantee is not yet honest.

- **Sweep bound undefined:** "link sweep bound ≤ 10 mm" has no formula. It has to be a true upper bound.
- **Surface meshes:** an environment `TriMesh` has zero thickness. Sampling alone proves nothing, and a body that starts inside a closed mesh reads as free.
- **Two geometries:** the "checked every Y°/Z mm" statement comes from the planner's polyline. What is displayed and played is the blended `q(s)`, which `validate()` checks at a different interval (2-5 mm).
- **Blend collisions:** no failure policy.
- **Equipment meshes:** nothing covers millimetre units, non-manifold input or 10^5-10^6 triangles (flags, budgets, memory).

**Fix.** 1. **Sweep bound:** `sweep(a, b) = max_i Σ_{j ≤ i} r_ij·|Δq_j|`, where `r_ij` is an upper bound on the distance from joint axis j to any vertex of the bodies on link i, tool included. Precompute at import. Steps = `ceil(max(max_j |Δq_j| / 1°, sweep / δ))`.
2. **Invariant `δ ≤ 2·pad_min`:** the defaults (5 mm, 10 mm) satisfy it, and δ follows when padding is lowered. A point that crosses a surface is within δ/2 of it at the nearest sample, so the padded test must fire. With pad = 0 the result is labelled "sampled only".
3. **Guarantee text:** "The unpadded convex model does not touch any obstacle along the path (padding p, step δ ≤ 2p). Not covered: ACM-disabled pairs, bodies that start inside a closed mesh, controller tracking error."
4. **Single source:** the guarantee attached to a result is the one `validate()` computes on the final `q(s)`.
5. **Blend policy:** halve the blend radius up to 4 times, then use zero blend, and report `BlendReduced { vertex, from, to }`.
6. **Equipment import:**
   - Weld vertices.
   - Use only `MERGE_DUPLICATE_VERTICES | DELETE_DEGENERATE_TRIANGLES | DELETE_DUPLICATE_TRIANGLES`. Never use `ORIENTED` or `HALF_EDGE_TOPOLOGY`.
   - Per-body collision mode `Mesh | ConvexHull | Box | None`.
   - Warn above 500k triangles; ask above 2M.
   - Unit selector, auto-guessing mm when the bounding-box diagonal exceeds 20.
   - Build the BVH as a cancellable job.
   - Optional ray-parity "start inside closed mesh" warning.
7. **Tests:**
   - Property test on random box worlds: a brute-force check of the unpadded model at δ/20 finds 0 hits on every returned path.
   - Synthetic 1M-triangle mesh: build ≤ 10 s, RSS ≤ 1.5 GB, median state check ≤ 200 µs.

## 8. [MAJOR] 5.9 (timing, validation typestate, limit profiles), Appendix E Q6

**Problem.** - **Wrong trapezoid oracle:** `T = d/v + v/a` holds only when `d ≥ v²/a`. With the plan's own 5 rad/s² profile on a UR5e (v = π rad/s) that threshold is 1.974 rad (113°), so most moves are triangular. The formula also assumes one axis limits both velocity and acceleration.
- **TOPP-RA rows:** the spec is a 2-variable LP, but a TCP acceleration norm is not linear on curved paths.
- **Degenerate parameter:** parameterising LIN by TCP arc length divides by zero for pure or near-pure reorientation.
- **No failure policy:** nothing happens when `validate()` finds a violation between grid points or TOPP-RA is infeasible.
- **Q6:** "without stopping" is not measurable.
- **CIRC:** orientation mode and arcs beyond 180° are undefined.

**Fix.** 1. **Oracle for a straight joint move Δq:** `V = min_j v_j/|Δq_j|`, `A = min_j a_j/|Δq_j|`. `T = 1/V + V/A` if `V² ≤ A`, else `2/√A`.
2. **Golden values:**
   - UR5e (v = π, a = 5): Δq1 = 1 rad → 0.894427 s; Δq1 = π → 1.628319 s.
   - URScript profile (v = 1.05, a = 1.4): Δ = π/2 → 2.245997 s; Δ = 0.5 → 1.195229 s.
   - UR10e (120 deg/s on J1-J2, 180 deg/s elsewhere, a = 5): Δq = (1, 0, 2, 0, 0, 0) → V = 1.570796, A = 2.5, T = 1.264938 s; Δq = (2, 0, 1, 0, 0, 0) → V = 1.047198, T = 1.373809 s.
   - TOPP-RA on the same paths matches within 0.5% at N = 200.
3. **TOPP-RA spec:**
   - Grid N in [100, 2000], including blend boundaries.
   - ṡ = 0 at both ends; `Δt = 2Δs / (ṡ_i + ṡ_{i+1})`.
   - TCP speed is a bound on ṡ.
   - TCP acceleration uses per-axis rows `|p'_k·u + p''_k·x| ≤ a_tcp/√3`.
   - On a validation failure: double N (up to 3 times), then scale limits by 0.95 (up to 3 times), then return `TimingFailed { joint, s, kind }`.
4. **Cartesian parameter:** `ds = max(dp / 1 m, dθ / π)` with separate linear and angular TCP limits (defaults 0.25 m/s, 1.2 m/s², π/2 rad/s). Test: a pure 90° reorientation LIN times against the angular limit.
5. **Q6 metric:** no interior grid point has ṡ < 1e-6; for corners below 90° the interior minimum ṡ is ≥ 0.2 of the local velocity bound; total time ≤ 1.3 × the sum of the per-leg straight-move times.
6. **CIRC:** defined by start/via/end (a sketch arc is converted). Collinear points return `CircDegenerate`. Arcs above 180° are allowed; a full circle needs two segments. Orientation mode is `Fixed` (slerp, default) or `FollowArc`.

## 9. [MAJOR] Appendix E (Q3, Q4, Q5, Q8), 5.8 (pipeline), 3.2 (P1/P2), 6 (profiles), 8 (risks)

**Problem.** Several quality targets are not measurable or not reachable as written.

- **Q4 reference:** "RRT* 30 s best cost" is wall-clock, which contradicts Q9. It is also far from converged. Edges cost about 50-100 state checks, k ≈ ⌈3.17·ln n⌉ ≈ 25 neighbours, and the plan's own estimate is 10^4-10^5 checks/s. That allows a few thousand iterations in 30 s in 6-D, so shortcut output will usually beat it and the 1.15 ratio is vacuous.
- **Q3 and Q8:** "within 5 s" and "300 ms" are wall-clock on an unspecified machine, and tests run at dev `opt-level = 1`.
- **Shortcut cost:** one attempt is a long edge of about 100-300 checks. There is no budget, and this stage will dominate latency.
- **Q5:** a 10% coefficient of variation needs the portfolio/hybridisation that 3.2 moved to P2. Multi-goal RRT-Connect lands in different homotopy classes and goals across seeds.
- **Undefined scenes:** the golden scenes and the narrow-passage benchmark are not specified.
- **No throughput gate:** the risk table flags unmeasured performance but no stage checks it.

**Fix.** Replace the Appendix E targets with the following. Use release builds, single-thread planners, seeds 0..99. Assert state-check counts; report times.

1. **Scenes:** S1 empty; S2 table plus fence; S3 machine door (400 × 400 mm opening, TCP enters 150 mm); S4 slot (tool hull width + 2·pad + 30 mm).
2. **Q3 success:** S1 100% and S2 ≥ 99% within 20k state checks; S3 ≥ 95% and S4 ≥ 80% within 200k.
3. **Q4 path length:**
   - Reference is a committed best-known cost per scene from `cargo xtask plan-reference` (Informed RRT*, 2M checks × 8 seeds, full post-processing, minimum).
   - Median ≤ 1.20× on S2 and S3.
   - S1 equals the straight line exactly.
   - The test fails if any seed beats the reference by more than 2%.
   - RRT* optimality is tested on closed-form worlds only.
4. **Q5 consistency:** promote a deterministic best-of-K = 4 seeds to P1 (every seed finishes its budget, take the minimum cost, tie to the lowest index). Hybridisation stays P2. Coefficient of variation ≤ 10% on S1/S2 and ≤ 20% on S3/S4. At least 90% of seeds reach the reference goal branch.
5. **Q8 latency:**
   - S1 ≤ 1.5k checks, median under 50 ms.
   - S2 plan plus simplify: median ≤ 30k checks, p95 ≤ 100k.
   - S3 median ≤ 100k checks.
   - Simplifier budget ≤ max(5k, 2 × planning checks), with exit after 20 consecutive failed shortcuts.
6. **Stage 3 throughput gate, before `gc_motion`:** release bench on UR5e with a 100k-triangle mesh. Require a median ≥ 30k state checks/s near obstacles and ≥ 100k/s in free space. If it fails, add a bounding-sphere prefilter and capsule proxies for the UR links as a P1 fallback.
7. **Profiles:** `opt-level = 3` for `gc_linalg`, `gc_robot`, `gc_collide`, `gc_plan`, `gc_traj` and `gc_motion` in dev. Default `cargo test` runs 10 seeds. The full 100-seed report runs with `cargo test --release -p gc_motion --test quality -- --ignored` at Stages 3 and 6, and its numbers go into the stage report.

## 10. [MAJOR] 3.1 step 7, 5.1 (update flow, input hash), 5.9 (jobs, cancellation), 4.2 and Q9 (determinism)

**Problem.** - **Replanning from scratch:** when a sketch dimension moves a waypoint by 1 mm, the result can jump to another IK branch, turn or homotopy class. In a parametric tool that reads as low quality.
- **Input hash contents unlisted:** resolved start configuration, ACM, padding policy, tool, limit profile and planner version are not named. Results can be shown as fresh when they are stale, and a changed end configuration must invalidate downstream segments.
- **Cancellation:** an `AtomicBool` with no latency bound and no polling points in IK restarts, ACM sampling, TOPP-RA or edge checks.
- **Determinism:** Q9 is only tested inside one process. FK through glam `DQuat` uses std trigonometry unless `glamx/libm` is on, so the plan's libm claim does not cover it. parry depends on hashbrown/foldhash, whose per-process seeding may affect iteration order unless `enhanced-determinism` is enabled. That feature is `simba/libm_force`, `indexmap`, `glamx/libm`, `glamx/scalar-math` (read from the 0.31.1 manifest).

**Fix.** 1. **Replan ladder:**
   - Revalidate the existing path against the new inputs. If it is valid and the endpoints are unchanged, keep it and label it "valid, not re-optimised".
   - Otherwise repair: rank the previous goal branch and turns first, reconnect the old interior vertices to the new endpoints, validate and post-process.
   - Otherwise run a full plan.
2. **New criterion Q11:** a 1 mm waypoint perturbation in S2/S3 keeps the goal branch and turns and changes cost by ≤ 5% (10 perturbations).
3. **Segment input hash:** robot model hash, group and frozen joints, mount pose, tool and TCP, limit profile, ACM hash, padding policy, obstacle set (mesh hash, pose, mode), resolved start q, resolved target, segment parameters, planner id and version, seed, budget. A changed end configuration at segment k marks k+1 onward stale.
4. **Cancellation:** poll at least every 256 state checks, every IK restart and every TOPP-RA pass. Test: `Cancelled` arrives within 100 ms on S4 with an unsatisfiable goal.
5. **Determinism:**
   - Enable `parry3d-f64/enhanced-determinism` in Stage 0 and measure the bench cost.
   - `cargo xtask determinism` runs the planning example in two separate processes, and with 1 and 4 workers, and compares output bytes.
   - Reword Q9 to "same binary on the same OS and architecture".

## 11. [MINOR] 5.6 (UR package, xtask gen-ur, Ur6rAnalytic), Appendix D, section 2 asset row

**Problem.** - **Unpinned source:** assets come from the moving `rolling` branch.
- **YAML tag:** `joint_limits.yaml` uses the custom tag `!degrees` and no YAML crate is planned.
- **Licence file:** the upstream `LICENSE` holds only the BSD-3 clauses, with no copyright line, and `ur_macro.xacro` has no header.
- **Root link:** `ur.urdf.xacro` adds a `world` link and a `base_joint`. The plan does not say what the generated URDF's root or the chain base is, although the analytic IK works in `base` = Rz(π)·`base_link`.
- **Elbow limit:** the ±π limit is described upstream as artificial ("we artificially limit this joint to half its actual joint position limit to avoid (MoveIt/OMPL) planning problems"). The plan presents it as a plain limit.
- **Detection tolerances:** unspecified. With the research values (1e-6 rad) a calibrated URDF never reaches `NearNominal`.
- **Turn count at zero:** the URDF limit is exactly ±2π, so q = 0 has three in-limit variants (−2π, 0, +2π).
- **Wrist singularity:** "q6 from the seed" is specified, but `solve_all` has no seed argument.

**Fix.** 1. **Provenance:** `assets/ur/SOURCES.toml` lists URL, fetch date, sha256 and licence per file. `cargo xtask fetch-ur` is manual, uses only raw.githubusercontent.com and verifies hashes. `gen-ur` runs offline.
2. **Notices:** commit the upstream LICENSE text verbatim with the attribution "Universal Robots A/S and contributors, Universal_Robots_ROS2_Description, BSD-3-Clause". Reproduce it and the Hawkins notice in `THIRD_PARTY_NOTICES.md` and in the About dialog.
3. **YAML:** read the four files with a small indentation-based reader in xtask (`!degrees` → × π/180). Unit-test it on the committed files.
4. **Generated URDF:** root is `base_link` with no `world`; it contains `base`, `flange`, `tool0` and `ft_frame`. The planning chain is `base_link → tool0`. A test asserts the fit gives pre = Rz(π), post = identity, all signs +1 and all offsets 0.
5. **Elbow option:** profile field `elbow_limit: Official(±π) | Hardware(±2π)`, default Official, with the upstream reason as tooltip.
6. **Detection:** structural test at 2° and 5 mm, then classify by FK error (`Exact` below 1e-9, `NearNominal` below 5 mm and 0.5°).
7. **Turn rule:** every integer k with `lo − 1e-9 ≤ q + 2πk ≤ hi + 1e-9`, clamped into the limits.
8. **Seed:** `solve_all(target, seed: Option<&[f64]>)`. When |sin q5| < 1e-6, take q6 from the seed (or 0) and flag `wrist_singular`.
9. **Label test:** pin the Up/Down and Flip labels with a geometric elbow-height test.

## 12. [MAJOR] 6 (Stage 2/3 gates), 7 (verification), new Appendix G

**Problem.** The robotics gates ("URDF FK == DH", "IK round trip and branches", "UR5e golden scene planning", "TOPP-RA acceptance") carry no expected values. The only pose given is q = 0, which is singular in both elbow (q3 = 0) and wrist (q5 = 0), so it is a poor IK fixture.

**Fix.** Add Appendix G. Values are for the UR5e parameters in Appendix D, frame `base → tool0`, tolerance 1e-9 unless noted. They were computed with a throwaway NumPy DH model.

**FK**
- FK-1: q = 0 → p = (−0.8172, −0.2329, 0.0628), R rows (1, 0, 0), (0, 0, −1), (0, 1, 0).
- FK-2: q = (0, −π/2, 0, −π/2, 0, 0) → p = (0, −0.2329, 1.0794), R rows (−1, 0, 0), (0, 0, −1), (0, −1, 0).
- FK-3: q = (0, −π/2, π/2, −π/2, −π/2, 0) → p = (−0.4919, −0.1333, 0.4879), R rows (0, 1, 0), (1, 0, 0), (0, 0, −1), i.e. tool Z down. In `base_link`, p = (0.4919, 0.1333, 0.4879).
- FK-4: q = (0.5, −1.2, 1.4, −1.0, 1.1, 0.3) → p = (−0.503947, −0.478682, 0.474912), R rows (0.859176, 0.393197, −0.327434), (−0.500799, 0.514913, −0.695748), (−0.104966, 0.761749, 0.639313), tolerance 1e-6.

**IK**
- IK-1: target FK-3 gives exactly 8 canonical solutions with 8 distinct `UrConfig` labels. They include:
  - (0, −1.570796, 1.570796, −1.570796, −1.570796, 0)
  - (0, −0.080231, −1.570796, 0.080231, −1.570796, 0)
  - (0, −1.073366, 0.941663, 1.702500, 1.570796, π)
  - (−2.612324, −1.570796, −1.570796, −1.570796, 1.570796, 0.529269)
- IK-2: target FK-4 gives 8 solutions, including (0.5, 0.132412, −1.4, 0.467588, 1.1, 0.3) and (−2.211242, −1.955160, −1.363878, −2.260569, −1.724602, −0.007680). Turn expansion gives exactly 256 candidates.
- IK-3: over 2,000 seeded q in (−π, π]^6, the generating q is always among the solutions (1e-6). Counts are always even and never NaN. The share with 8 solutions lies in [70%, 85%] (measured: 8 → 77%, 6 → 5%, 4 → 15%, 2 → 3%).
- IK-4:
  - The FK-1 and FK-2 targets (singular) return ≥ 1 finite solution within 1e-6, flagged singular.
  - Target p = (1.2, 0, 0.3) returns `OutOfReach`.
  - Target p = (0.05, 0.05, 0.6) with R = I puts the wrist centre 0.0707 m from the base axis, inside d4 = 0.1333, so it returns 0 solutions and no NaN.

**Cartesian**
- LIN-1: from the FK-3 pose, 0.2 m along base −x. Fraction 1.0, constant `UrConfig`, TCP deviation ≤ 0.1 mm at sample midpoints.
- LIN-2: same start, 0.6 m along −x. Returns `CartesianIncomplete` with cause elbow singularity or out of reach, at a fraction in [0.45, 0.51]. The exact reach limit is at 0.5097: the arm is straight when √(r² + 0.425²) = 0.8172, r = 0.6980.

**Planning**
- PLAN-1 (TCP-sphere mode): r = 0.05, padding 0.005, ball obstacle R = 0.25 at the origin (ρ = 0.305), start (−0.6, 0, 0), goal (0.6, 0, 0). Optimum = 2√(d² − ρ²) + ρ(π − 2·acos(ρ/d)) = 1.358674 m.
  - RRT-Connect plus shortcut ≤ 1.05×.
  - Informed RRT* with 200k checks ≤ 1.03×, non-increasing in budget.
  - Never below 1.3577.
- PLAN-2: empty scene gives 2 vertices and cost equal to the metric distance.
- PLAN-3: goal enclosed by a closed box gives `NoPathWithinBudget` with statistics, never a path.
- PLAN-4: applying one rigid transform to the mount, obstacles and waypoints gives the same joint path within 1e-9.

**Timing:** use the values listed under the 5.9 issue.

## Keep (must not change)

- URDF as the single source of kinematic truth: own roxmltree reader with file:line:col diagnostics, plus a `PlanningProfile` for everything URDF lacks, with provenance labels (URDF / package / assumed). No runtime xacro; the UR URDF is generated from the official YAML files by xtask.
- The frame facts in Appendix D and the IK approach: port Hawkins directly on `T_DH` with range-checked `acos`/`asin`, test FK == DH at 1e-9, and wrap analytic solutions with a numeric polish for calibrated chains. These match the research; I re-derived the zero-pose values and the 77% eight-solution share independently.
- Bounded ±2π revolute joints as a plain box in which `q` and `q + 2π` are distinct goals, with wrapping reserved for true `continuous` joints, and IK branch × turn enumeration feeding a multi-goal RRT-Connect.
- Separation of geometric path and time law: TOPP-RA instead of TOTG, the ban on splining through time-stamped knots, the `Trajectory<Unvalidated> → Trajectory<Validated>` typestate, velocity limits read per model from the URDF, acceleration from named profiles, and the "estimated cycle time" label.
- A robot-agnostic `gc_plan` with mandatory independent re-validation, edge resolution in absolute units, iteration and check budgets instead of wall clock, an own PRNG, parry `Unsupported` raised as a typed error, the specific failure taxonomy (`CartesianIncomplete { fraction, cause }`, never-silent start fixes), and an editable ACM that records its reasons.

## Verdict

The robotics core of the plan is sound and consistent with the research, but it is not ready to execute: one blocker and nine major issues need to be written in first. The blocker is the plugin boundary: `Refined<Ur6rAnalytic>`, the two IK traits, an undefined `ChainSpec` and UR-specific hints in the document model cannot be built as specified, and parallel agents would diverge on them. The quoted UR facts hold: I confirmed the elbow ±π limit, the `!degrees` tags, the continuous UR3e wrist 3 and the 5 rad/s² MoveIt value against the upstream files today. The larger gap is that the plan is a good UR5e free-space planner rather than high-quality planning from an arbitrary robot description. To close it, the plan needs a time-based cost metric with turn-local sampling and a diverse goal set, a defined start state with sequence-level IK resolution, generic handling of prismatic, mimic, continuous and non-6-DoF chains, a real tool/TCP model with per-element collision shapes, and a contact allowance for waypoints near surfaces. The collision guarantee becomes honest once the sweep bound is defined, the step is tied to twice the padding, and the statement comes from validating the final blended path. The quality table needs check-count budgets, defined scenes, a committed best-known reference instead of "RRT* for 30 s", a throughput gate and best-of-4 seeds in P1; the trapezoid oracle is wrong for most UR5e moves and must be replaced. None of this reopens a fixed decision, and all of it fits the existing crate layout. The LIN reach fractions, the wrist-weight ratio and the throughput figures in my fixes are derived or estimated, not run, and should be confirmed by the proposed tests and bench gate.
