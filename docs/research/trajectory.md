# GraphCTRL: from geometric joint path to timed, displayable trajectory

Legend: **[V]** verified today from a primary source (path or URL given); **[S]** secondary source only (search snippet); **[R]** recalled from memory.

`universal-robots.com` is blocked in this session, so the URScript manual could not be opened; URScript defaults are [S]/[R]. I wrote no files, including the plan file the harness suggested, because the task is read-only.

## 1. Time-parameterisation algorithms

### 1.1 Synchronised point-to-point (movej-like)
Treat the move as a straight joint-space line `q(σ) = q0 + σ·Δq`, σ∈[0,1], and time the scalar σ.

- **Lead-axis limits**: `σ̇max = min_i v_i/|Δq_i|`, `σ̈max = min_i a_i/|Δq_i|`, `σ⃛max = min_i j_i/|Δq_i|`. All joints then start, cruise and stop together.
- **Trapezoid**: if `1 ≥ σ̇max²/σ̈max`, `T = 1/σ̇max + σ̇max/σ̈max`; otherwise triangular, `T = 2·sqrt(1/σ̈max)`.
- **S-curve (7 segments)**: when both a and v plateaus are reached, `T = d/v + v/a + a/j`. The degenerate cases are many, so delegate to rsruckig.
- **Analytic oracle for tests**: UR5e, one joint, 90°, URScript defaults 1.05 rad/s and 1.4 rad/s² gives `T = 1.5708/1.05 + 1.05/1.4 = 2.246 s`.

### 1.2 TOTG (Kunz and Stilman 2012), as implemented in MoveIt
All items below are [V] from `moveit2/main/moveit_core/trajectory_processing/src/time_optimal_trajectory_generation.cpp` unless marked.

- **Path**: linear segments joined by circular blends. Blend radius is limited by `max_deviation` and half the neighbouring segment lengths ("enforce max deviation", l.138); the closed form `ℓ = min(|a|/2, |b|/2, δ·sin(α/2)/(1−cos(α/2)))`, `r = ℓ/tan(α/2)` is [R].
- **Limit curves**: a velocity curve `ṡ ≤ min_i v_i/|f'_i|` and an acceleration curve from pairs of joint constraints `|f'_i·s̈ + f''_i·ṡ²| ≤ a_i`.
- **Integration**: forward at maximum `s̈` until a limit curve is hit, find the next switching point, integrate backward at minimum `s̈` until it meets the forward branch, then a final backward pass from the path end.
- **Step size**: fixed-step explicit integration, `DEFAULT_TIMESTEP = 1e-3`, `EPS = 1e-6`.
- **Pitfalls and MoveIt's fixes**:
  - 180° reversals are rejected ("not supported by the current implementation", l.231).
  - Repeated waypoints are filtered with `min_angle_change` (upstream issue tobiaskunz/trajectories#3).
  - Angles are unwound first ("does not actually work properly when angles wrap").
  - Mixed revolute and prismatic joints break `path_tolerance` (warning only).
  - Zero acceleration limits caused an infinite loop; this is now an explicit error.
  - Near-duplicate steps at switching points are skipped (moveit#1665).
  - Failure modes are "Negative path velocity" and "Did not hit start trajectory".
  - Defaults are `path_tolerance = 0.1` and `resample_dt = 0.1`.
- **Path deviation**: the output leaves the input path within the tolerance, so "additional collision checks might be required" ([V] moveit2_tutorials `time_parameterization_tutorial.rst`).
- **Limitations**: starts and ends at rest only [S]; acceleration is bang-bang, so jerk is unbounded [V tutorial: "none of the time parameterization algorithms support jerk limits"].

### 1.3 TOPP-RA (Pham and Pham 2018) [R]
- **Setup**: grid `s_0..s_N`, state `x = ṡ²`, control `u = s̈`, exact relation `x_{k+1} = x_k + 2Δ_k·u_k`. All joint velocity and acceleration constraints are linear in `(u, x)`.
- **Backward pass**: controllable intervals `K_k = [lo, hi]`, each from two LPs in two variables.
- **Forward pass**: greedily take the largest `u` with `x_{k+1} ∈ K_{k+1}`.
- **Time**: `t_{k+1} = t_k + 2Δ_k/(√x_k + √x_{k+1})`.
- **Why it is more robust than TOTG**: there is no switching-point search and no ODE integration against a limit curve. Infeasibility shows up as an empty `K_k`. It works on any C¹ path with known `q'` and `q''`. Extra constraints such as TCP speed are just more rows.
- **Caveat**: constraints hold at grid points only. Use a fine grid (Δ giving at most 0.5° of joint change), a 2–3% margin, and post-validation.

### 1.4 Ruckig
- **Scope**: time-optimal jerk-limited state-to-state motion.
- **Waypoints**: "Upstream ruckig calculates intermediate waypoints only in the Pro version or via its cloud API" ([V] as stated in `rsruckig-3.0.1/src/rsruckig/calculator_waypoints.rs`; upstream itself not opened).
- **How MoveIt uses it** ([V] `ruckig_traj_smoothing.cpp`): as a smoother after TOTG, waypoint to waypoint, stretching duration by 1.1× up to 50× on failure, with a default jerk of 1000 rad/s³ and overshoot checks every 0.01 s.
- **Consequence**: Ruckig does not follow a geometric path. It passes through the waypoints only, so collisions must be re-checked.

### 1.5 Iterative parabolic and iterative spline
- Both were deleted from MoveIt 2 in PR #1780 (Dec 2022); TOTG is the only option listed in the current tutorial [V/S].
- Reasons [R]: they stretch per-segment durations heuristically, are not time-optimal, depend on waypoint density, and iterative parabolic produced acceleration spikes or violations at segment joins.

### 1.6 Splines through waypoints
- **Cubic (C², natural or clamped)**: the right geometric path model to feed TOPP-RA.
- **Quintic**: gives C⁴ and exact boundary accelerations.
- **Do not time by "spline through time-stamped knots"**: overshoot between knots breaks limits and collision guarantees. Always separate path `q(s)` from time law `s(t)`.

### 1.7 What to implement first
1. **PTP synchronised trapezoid**, in-house (about 150 lines), with the exact analytic oracle.
2. **PTP jerk-limited** through rsruckig `calculate` (offline).
3. **Planner paths** in three steps:
   - shortcut and simplify the path;
   - fit a C² path, either blended-corner (deviation-bounded, like TOTG's) or a cubic spline;
   - run **in-house TOPP-RA** with joint velocity and acceleration rows.
4. **Optional jerk limiting that preserves the path**: box-filter `ṡ(t)` over a window `T_f` (FIR smoothing [R]). This adds about `T_f` to the duration and bounds jerk to roughly `a/T_f`. Re-validate, and rescale if a limit is exceeded.
5. **Post-validation** every time (section 5).

**Acceptance tests**
- At 2 ms sampling, `|q̇_i| ≤ v_i(1+1e-6)` and `|q̈_i| ≤ a_i(1+1e-3)`; position limits hold.
- `q` and `q̇` are continuous: finite-difference jumps stay within `a·dt` and `j·dt`.
- `q̇ = 0` at both ends; the end position is exact to 1e-9.
- Single-segment duration equals the trapezoid formula to 1e-6; multi-joint equals the lead-axis formula.
- A TOPP-RA straight line has duration no less than the trapezoid optimum and within 1% of it at N = 1000.
- Halving the grid changes T by less than 0.5%.
- Property tests on random paths with proptest: no panics, monotone time, deterministic bytes.
- copp `topp2_ra` as a dev-only oracle, T within 1%.
- Degenerate inputs give typed errors: duplicate waypoints, zero-length path, 180° reversal, a joint that does not move, a zero limit.

Core of the in-house TOPP-RA (sketch):

```rust
/// Per grid point: qp = dq/ds, qpp = d2q/ds2; limits v, a (per joint, symmetric).
/// Rows: -a_j <= qp_j*u + qpp_j*x <= a_j ; 0 <= x <= xmax = min_j (v_j/|qp_j|)^2
fn u_range(x: f64, qp: &[f64], qpp: &[f64], a: &[f64]) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    for j in 0..qp.len() {
        let c = qpp[j] * x;
        if qp[j].abs() < 1e-12 { if c.abs() > a[j] { return None; } continue; }
        let (l, h) = ((-a[j] - c) / qp[j], (a[j] - c) / qp[j]);
        lo = lo.max(l.min(h)); hi = hi.min(l.max(h));
    }
    (lo <= hi).then_some((lo, hi))
}
// Backward: K[N] = [0,0]; K[k] = { x in [0,xmax_k] : exists u in u_range(x),
//           x + 2*ds*u in K[k+1] }  -> interval; ends found by a 2-variable LP
//           (g(x) = min_hi - max_lo is concave in x: bisection or vertex enumeration).
// Forward:  x0 = 0; u_k = min(u_hi(x_k), (K[k+1].hi - x_k) / (2*ds)); require u_k >= u_lo
//           and x_k + 2*ds*u_k >= K[k+1].lo, otherwise TimingError::Infeasible { k }.
// Time:     t[k+1] = t[k] + 2*ds / (x[k].sqrt() + x[k+1].sqrt()).
```

## 2. Crates: verified facts and verdicts

| Crate | Facts | Verdict |
|---|---|---|
| **rsruckig 3.0.1** (2026-08-29, MIT) | Deps are only `arrayvec`, `num-traits`, `thiserror` [V Cargo.toml]. Const-generic `Ruckig::<DOF, E>`; `DOF = 0` with `new(Some(n), dt)` gives heap or dynamic DoF. Offline entry point: `calculate(&mut self, &InputParameter<DOF>, &mut Trajectory<DOF>) -> Result<RuckigResult, RuckigError>`. `Trajectory` offers `at_time`, `get_duration`, `get_intermediate_durations`, `get_position_extrema`, `get_first_time_at_position`. `InputParameter` has `intermediate_positions`, `per_section_*` limits, `min/max_position`, `minimum_duration`, `synchronization`, `duration_discretization`. Error policy is a type parameter (`ThrowErrorHandler`, `IgnoreErrorHandler`, custom). Result codes include `ErrorInvalidInput`, `ErrorPositionalLimits`, `ErrorZeroLimits`, `ErrorExecutionTimeCalculation`, `ErrorSynchronizationCalculation`. 0 `unsafe` in `src`. 18 `panic!/unwrap/expect/unreachable!` sites. The published tarball has **0 `#[test]`** (package root is `lib/`; the README describes a suite kept in the repo). | **Buy** for PTP S-curve and jogging. Wrap it behind our own trait and in `catch_unwind`. |
| rsruckig waypoints | An independent heuristic solver, not upstream code: heuristic pass-through states, chained state-to-state sections, 4 refinement sweeps. "Not guaranteed time-optimal"; needs finite jerk. README claims durations within 1.4% of Ruckig Pro on 42 random 3-DoF chains. The 3.x line is recent (3.0.0 on 2026-07-11; six releases that month). | **Do not** make it the default for planner paths. Offer it as an alternative algorithm, with a collision re-check. |
| **copp 0.2.3** (2026-09-16, MIT, MSRV 1.89) | Implements TOPP2-RA, COPP2-SOCP, TOPP3-LP/SOCP, COPP3-SOCP, plus spline and B-spline path construction. Non-optional deps: `nalgebra 0.35`, `clarabel 0.11`, `rayon`, `chrono`, `itertools`, `log`. Its own benchmark reports TOPP2-RA at 0.67 ms for 7 DoF and 1000 intervals. 110 unit tests in `src`. `unsafe` appears on 434 lines across `src` (location not inspected; the crate ships C/C++/Python FFI modules). First release 2026-04-14; 1066 downloads; a commercial "PRO" sibling exists. The 0.2.3 changelog fixes serious numerical bugs (`a` clamped at 1e6, misaligned constraint rows, unit dependence), and the waypoint constructors are "explicitly documented as unstable". | **Do not depend on it in the core.** Use it as a dev-dependency oracle, and optionally as a feature-gated plugin for jerk-limited TOPP3. |
| Other trajectory crates | No crate named `toppra` or `totg` exists [V 404]. `ferromotion-ruckig 0.73.0` (65 versions since 2026-07-12), `caliper-motion 0.2.1` (single release, 309 downloads, contains `topp.rs`, tied to the caliper nalgebra stack), `s_curve 0.1.7` (2023), `scurve_motion 0.1.0`, `trajectory 0.1.0` (openrr, 2022). | Avoid all: churn, staleness or stack lock-in. |
| Spline crates | `splines 5.0.0` (2025-04-26, BSD-3), `bspline 1.1.0` (2022-02-11), `enterpolation 0.3.0` (2025-05-08), `uniform-cubic-splines 0.5.2` (2026-04-14) [V metadata]. I did not read their APIs; my recollection is that they are graphics-oriented and lack the analytic `q'`, `q''` and arc-length tooling needed here [R]. | **Build** the natural or clamped cubic spline and blended-corner path in-house (about 200 lines, tridiagonal solve). |
| **egui_inbox 0.14.0** | See section 7. | Optional convenience. |

## 3. UR numbers and semantics

**Joint speed limits, deg/s** [V `Universal_Robots_ROS2_Description/rolling/config/<model>/joint_limits.yaml`; every file has `has_acceleration_limits: false`]

| Model | pan | lift | elbow | wrist 1–3 |
|---|---|---|---|---|
| ur3, ur3e | 180 | 180 | 180 | 360 |
| ur5, ur5e, ur7e | 180 | 180 | 180 | 180 |
| ur10, ur10e, ur12e, ur16e | 120 | 120 | 180 | 180 |
| ur20, ur30 | 120 | 120 | 150 | 210 |
| ur15, ur18, ur8long | 180 | 180 | 240 | 300 |

Position limits are ±360°, elbow ±180° [V; wrist values were not read for every model].

**Acceleration and Cartesian limits used by UR's own configuration** [V `Universal_Robots_ROS2_Driver/main/ur_moveit_config/config/`]
- `joint_limits.yaml`: `max_acceleration: 5.0` rad/s² on all six joints. The file comments that the robot "does not inherently have any limits on joint accelerations (only on torques)" and that the values were "chosen conservatively". The `humble` and `jazzy` branches are identical.
- `pilz_cartesian_limits.yaml`: `max_trans_vel 1.0`, `max_trans_acc 2.25`, `max_trans_dec -5.0`, `max_rot_vel 1.57`.
- UR's client-library script calls `speedj(qd, 40.0, steptime)` [V `Universal_Robots_Client_Library/master/resources/external_control.urscript` l.471].

**URScript defaults** [S, consistent with R; not opened at the manual]
- `movej(q, a=1.4, v=1.05, t=0, r=0)`, in rad/s² and rad/s.
- `movel(pose, a=1.2, v=0.25, t=0, r=0)`, in m/s² and m/s.
- `movep(pose, a=1.2, v=0.25, r=0)`.
- `servoj(q, a, v, t=0.002, lookahead_time=0.1, gain=300)`: `a` and `v` are unused, lookahead is 0.03–0.2 s, and `t` is 0.008 on CB3 (125 Hz) versus 0.002 on e-Series (500 Hz).
- `t > 0` overrides `a` and `v`.

**Blend radius** [R]
- `r` is a TCP-space sphere around the waypoint, in metres. The robot leaves the programmed segment on entering it and never reaches the waypoint.
- Blends of adjacent waypoints must not overlap.
- If the next motion command is not ready, the robot stops at the waypoint.
- `movep` holds constant tool speed with circular blends, and speed is capped by acceleration on small radii (about `v ≤ √(a·r)`).

**What can only be approximated without URSim or the controller**
- the exact profile shape and jerk;
- torque- and payload-dependent limits;
- the proprietary blend geometry;
- safety-plane and speed-slider scaling;
- `servoj` filtering lag;
- behaviour near singularities in `movel` (slow-down or protective stop);
- IK branch selection.

Present GraphCTRL timings as "estimated cycle time". Ship editable per-robot limit profiles: "UR-MoveIt conservative (5 rad/s²)", "URScript defaults (1.05 / 1.4)" and "custom".

## 4. Cartesian-space timing (LIN / CIRC)

**What Pilz does** [V `moveit_planners/pilz_industrial_motion_planner/src/trajectory_generator_lin.cpp`, `trajectory_functions.cpp`, `velocity_profile_atrap.cpp`]
- An asymmetric trapezoid on the Cartesian path. Rotation is coupled through an equivalent radius `max_trans_vel/max_rot_vel` (l.184).
- It samples at a fixed `sampling_time`, runs IK per sample seeded by the previous solution, and derives `q̇` and `q̈` by finite differences.
- `verifySampleJointLimits` then **fails the plan**: "Joint velocity limit … violated. Set the velocity scaling factor lower!". There is no automatic retiming.

**Recommended for GraphCTRL (better than Pilz)**
1. Build the Cartesian path `X(s)` with `s` as TCP arc length. Position is a line or arc. Orientation is slerp in a scalar tied to `s`; for pure rotations use angle as the path parameter.
2. Sample densely (chord ≤ 0.5 mm and ≤ 0.2°) and run branch-consistent IK. Reject on a branch flip or `‖Δq‖∞` above threshold, with `CartesianError::SingularityOrBranchChange { s }`.
3. Fit a joint path `q(s)`.
4. Run the **same TOPP-RA** with extra rows `ṡ ≤ v_tcp`, `|s̈| ≤ a_tcp`, plus the joint velocity and acceleration rows.

Away from singularities the result is the trapezoid with a constant TCP speed plateau. Near them TCP speed drops automatically instead of failing. Report "programmed TCP speed not reached on x% of the segment". Keep the pure Cartesian trapezoid as a "strict constant speed" mode that errors like Pilz.

## 5. Trajectory representation and validation

- **Canonical form**: `Trajectory { path: Arc<JointPath> /* q(s), q', q'' */, law: TimeLaw /* s(t), piecewise-constant s̈ or Ruckig profile */, segments: Vec<SegmentMeta> }` with `sample(t) -> State { q, qd, qdd }`. It is exact, compact and resolution-independent, and it preserves the collision-checked geometry.
- **Derived caches**: a uniform 2 ms table for validation and export (matches the e-Series 500 Hz cycle [S]); a TCP polyline for display.
- **Type-state**: `Trajectory<Unvalidated>` becomes `Trajectory<Validated>` through `validate()`. Only the validated type can be played as "OK" or exported.
- **Post-validation**:
  - limits at 2 ms;
  - collisions at adaptive joint steps (the largest link-point sweep ≤ 2–5 mm via per-link radius bounds), reporting the first colliding pair and its time;
  - TCP speed and acceleration;
  - joint-limit margin.
- **Statistics**:
  - duration;
  - per-joint peak `|q̇|/v_max` and `|q̈|/a_max` as percentages, with the limiting joint per segment;
  - TCP path length;
  - TCP speed min, mean and max, plus its profile;
  - minimum clearance (when distance queries are on);
  - minimum manipulability and distance to singularity;
  - planner name, seed and iteration count.
- **Exporter hooks**: `SegmentMeta { kind: MoveJ | MoveL | MoveP | MoveC | Free, target, a, v, blend_r, source_feature_id }`.
  - Sketch-authored moves export as native `movej`/`movel`/`movep`.
  - Planner "Free" segments export as `movej` chains with blends (re-timed by the controller) or as a 500 Hz `servoj` stream.
  - UR's own script also carries cubic and quintic joint-spline runners and `optimovej`/`optimovel` wrappers [V external_control.urscript].

## 6. Visualisation

**What the reference tools show** [all R]
- **RoboDK**: station tree with targets as frames; TCP trace on or off; collision map with colliding objects in red; joint sliders with limit bars; Alt-drag of TCP or flange; target reachability shown by warning icons and a configuration picker; cycle-time estimate; timeline scrubbing in newer versions.
- **RViz MotionPlanning**: start and goal ghost robots (green and orange); interactive 6-DoF marker; animated looping trail with a "Show Trail" step; colliding links in red; velocity and acceleration scaling spinboxes; trajectory slider; planner choice and planning time.
- **Tesseract**: tool-path axes at each waypoint; trajectory player with slider; contact-result markers (arrows between closest points); a joint-trajectory plot widget.
- **Visual Components**: statement list tied to 3D waypoints; path lines with blend zones; reach colour-coding; collision detector with highlights; jog panel for joints and Cartesian; cycle-time statistics.
- **PolyScope Move tab**: per-joint jog bars with limit ends; TCP translate and rotate arrows; selectable feature frame (Base, Tool or custom); TCP pose readout; Freedrive; speed slider.

**MVP specification**
1. **TCP trace polyline**, coloured by segment kind or by speed as a toggle. Waypoint markers use the sketch point's identity.
2. **Animated robot playback**: timeline scrubber, play, pause, loop, speed 0.1–4×, time and segment readout. Drive it with `ctx.request_repaint()` while playing only.
3. **Ghost poses** at start and goal, and at the hovered waypoint.
4. **Validity overlay**: colliding link pair or obstacle tinted red when scrubbing to the collision time; red tick marks on the timeline for collisions and limit violations.
5. **Waypoint reachability colouring**: reachable, reachable in another configuration, unreachable, in collision.
6. **Joint sliders** with a limit-proximity gradient and live collision tint.
7. **Statistics panel** and a small per-joint velocity-utilisation plot (egui_plot).

**Second step**: a TCP gizmo for Cartesian jogging through IK; the planner tree or roadmap projected to TCP space as a line list, capped at about 20k edges.

## 7. Background execution

**egui_inbox 0.14.0** [V `src/lib.rs`, `Cargo.toml`]
- Depends on egui `0.37.0` (optional default feature) and `parking_lot`; `#![forbid(unsafe_code)]`.
- Constructors: `UiInbox::new()`, `new_with_ctx(&ctx)`, `channel()`, `channel_with_ctx()`.
- Reading: `read(&ui) -> impl Iterator<Item = T>` drains the queue; `replace(&ui, &mut T)` keeps only the last item; `read_without_ctx()`; `set_ctx()`.
- Sending: `sender()` gives a cloneable `UiInboxSender`. `send(item) -> Result<(), SendError<T>>` pushes, calls `request_repaint()`, and returns **`Err` once the inbox is dropped**.
- It works without egui through `RequestRepaintContext::from_callback`.
- The queue is an unbounded `Vec` behind a mutex, so throttle progress messages.

Verdict: usable, but it is about 40 lines of functionality. Keep the job system in a non-UI crate behind a `Notifier` closure, and adapt it to `egui::Context::request_repaint` in the app.

**egui_kittest 0.37.0** [V `src/lib.rs`]
- `Harness::step()`, `run_steps(n)`, `try_run_realtime()`, `state()`, `state_mut()`.
- `run()` loops "until no more repaints are requested" and panics past the maximum step count. Do not call `run()` while a job is repainting.

**Design rules**
- **Snapshots**: each job gets an `Arc<SceneSnapshot>` (robot model, parry shapes, limits) and a `DocRevision`. Job code holds no locks.
- **Stale results**: results carry `(JobId, DocRevision)`. The UI drops them when the revision differs, or keeps them as "stale preview".
- **Cancellation**: an `AtomicBool` polled every N iterations and inside collision loops. Starting a new job of the same kind cancels the old one.
- **Determinism**:
  - an explicit seed in the request;
  - **iteration budgets** as the primary termination rule (a wall-clock timeout is secondary and flagged as non-reproducible);
  - when racing planners, collect all results in order and pick by `(cost, planner_index)`; "first to finish wins" is opt-in;
  - no HashMap iteration order and no unordered parallel float reductions in algorithms.
- **Threads**: one `std::thread` per job, and a dedicated `rayon::ThreadPool` (cores − 1) for portfolios, so neither the UI thread nor the global pool is starved.
- **Panics**: `catch_unwind` at the job boundary becomes `JobError::Panicked(msg)`. This requires `panic = "unwind"` in all profiles.

```rust
use std::{any::Any, panic::{self, AssertUnwindSafe},
          sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc}, thread};

#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct JobId(pub u64);
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct DocRevision(pub u64);

#[derive(Clone)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}

pub enum JobMsg<T> {
    Progress { fraction: f32, note: &'static str },
    Done(Result<T, JobError>),
}
#[derive(Debug)]
pub enum JobError { Cancelled, Panicked(String), Failed(String) }

pub struct JobCtx<T> {
    pub cancel: CancelToken,
    tx: mpsc::Sender<JobMsg<T>>,
    notify: Arc<dyn Fn() + Send + Sync>,   // app: move || egui_ctx.request_repaint()
}
impl<T> JobCtx<T> {
    pub fn progress(&self, fraction: f32, note: &'static str) {   // throttle at call site
        let _ = self.tx.send(JobMsg::Progress { fraction, note });
        (self.notify)();
    }
}

pub struct JobHandle<T> {
    pub id: JobId, pub revision: DocRevision,
    cancel: CancelToken, rx: mpsc::Receiver<JobMsg<T>>,
}
impl<T> JobHandle<T> {
    pub fn cancel(&self) { self.cancel.0.store(true, Ordering::Relaxed); }
    /// Call once per frame; never blocks.
    pub fn poll(&self) -> impl Iterator<Item = JobMsg<T>> + '_ { self.rx.try_iter() }
}
impl<T> Drop for JobHandle<T> { fn drop(&mut self) { self.cancel(); } }

pub fn spawn_job<T, F>(id: JobId, revision: DocRevision,
                       notify: Arc<dyn Fn() + Send + Sync>, work: F) -> JobHandle<T>
where T: Send + 'static,
      F: FnOnce(&JobCtx<T>) -> Result<T, JobError> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let cancel = CancelToken(Arc::new(AtomicBool::new(false)));
    let ctx = JobCtx { cancel: cancel.clone(), tx, notify };
    thread::Builder::new().name(format!("job-{}", id.0)).spawn(move || {
        let out = panic::catch_unwind(AssertUnwindSafe(|| work(&ctx)))
            .unwrap_or_else(|p| Err(JobError::Panicked(panic_text(p))));
        let out = if ctx.cancel.is_cancelled() { Err(JobError::Cancelled) } else { out };
        let _ = ctx.tx.send(JobMsg::Done(out));
        (ctx.notify)();
    }).expect("spawn job thread");
    JobHandle { id, revision, cancel, rx }
}

fn panic_text(p: Box<dyn Any + Send>) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic".into())
}

// UI side, each frame:
// for msg in handle.poll() { match msg {
//   JobMsg::Done(Ok(t)) if handle.revision == doc.revision() => doc.set_trajectory(t),
//   JobMsg::Done(Ok(_)) => { /* stale: drop or mark */ }  ... } }

// Deterministic portfolio (inside a job, on a dedicated pool):
// let results: Vec<_> = pool.install(|| planners.par_iter().enumerate()
//     .map(|(i, p)| p.plan(&snapshot, seed ^ i as u64, &ctx.cancel)).collect());
// let best = results.into_iter().enumerate().filter_map(|(i, r)| r.ok().map(|r| (i, r)))
//     .min_by(|a, b| a.1.cost.total_cmp(&b.1.cost).then(a.0.cmp(&b.0)));
```

**Headless tests**
- Unit-test jobs without egui: use a `notify` that counts calls, `handle.rx.recv_timeout()` in tests, and cancellation asserted through a job that loops until cancelled.
- UI tests: build the `Harness` with app state, click "Plan", then loop `harness.step(); if harness.state().job_done() { break } sleep(2 ms)` with a deadline. Assert on the trajectory and take a snapshot.
- For exact repeatability, inject an `Executor` trait with an `InlineExecutor` that runs the closure synchronously at spawn.
- Panic test: a job that panics yields `JobError::Panicked` and the UI stays responsive.

## Recommendation

1. **Pipeline (default)**: planner path → shortcut and simplify → C² blended or spline path with bounded deviation, collision re-checked → **in-house TOPP-RA** (joint velocity and acceleration, optional TCP rows) → optional FIR jerk smoothing → `Trajectory<Unvalidated>` → 2 ms limit check and adaptive collision validation → `Trajectory<Validated>` → statistics, display and playback.
2. **PTP moves**: in-house synchronised trapezoid as the "UR-like" default; **rsruckig 3.0.1** for the jerk-limited variant.
3. **Selectable alternatives** behind one `TimeParameterizer` trait: trapezoid PTP per segment (stop at every waypoint), rsruckig waypoints, copp TOPP3 (feature-gated). All of them go through the same validator.
4. **Cartesian LIN/CIRC**: use the same TOPP-RA with TCP rows, plus a "strict constant speed" mode that fails as Pilz does.
5. **Limits**: read velocity from the URDF; ship UR acceleration presets (5 rad/s² as MoveIt-conservative; 1.4 rad/s² and 1.05 rad/s as URScript defaults) and a jerk default. These are user-editable and stored per robot.
6. **Build in-house**: cubic-spline and blend path, TOPP-RA, trapezoid, validator, job system. **Buy**: rsruckig, and rayon for the dedicated pool. **Dev-only**: copp as oracle, egui_kittest. egui_inbox is optional.
7. **Threading**: the `spawn_job` design above, with revision-tagged results, iteration budgets and seeded RNG.

## Risks and unknowns

- URScript defaults and blend semantics were not checked against the manual (host blocked). Confirm before labelling anything "UR-accurate". Cycle times will differ from URSim, so add a URSim comparison milestone later.
- UR publishes no acceleration or jerk limits. 5 rad/s² is UR's conservative MoveIt value, not a controller truth.
- rsruckig: the crates.io tarball ships no tests, the waypoint solver is new and heuristic, and there are 18 panic-capable sites. Gate it with our own property tests and `catch_unwind`.
- copp's young, fast-moving API and its heavy dependencies (nalgebra 0.35, clarabel) conflict with the glam-only decision if it enters the core.
- TOPP-RA grid collocation can exceed limits between grid points on high-curvature blends, hence the margin and mandatory post-validation. FIR jerk smoothing can slightly break s-dependent limits, so re-validate and rescale.
- Path smoothing and blending change geometry after planning. A collision re-check of the final path is mandatory, and the planner needs a clearance margin of at least the blend deviation.
- Cartesian moves depend on branch-consistent IK from the other research track; behaviour near wrist singularities needs a clear, typed failure.
- Thread cancellation is cooperative: a long parry query or rsruckig call cannot be interrupted mid-call.
- `catch_unwind` needs `panic = "unwind"`.
- Bit-for-bit determinism across Windows and Linux is not guaranteed for `libm` functions (sin, cos, acos). Promise determinism per platform.
- Spline-crate capabilities were judged from memory, not from source.

## Surprises

- **Upstream open-source Ruckig has no local intermediate waypoints**, but rsruckig 3.x ships its own independent waypoint solver (heuristic, not time-optimal), as well as its own position limits and tracking interface. It is not a pure port.
- **Pilz does not rescale on joint-limit violation**: it aborts and tells the user to lower the scaling factor.
- **MoveIt 2 has exactly one time-parameterisation algorithm left** (TOTG), and it rejects 180° reversals and paths not starting or ending at rest.
- UR's own config states the robot "does not inherently have any limits on joint accelerations (only on torques)". Any acceleration-limited timing is a modelling choice.
- UR's own external-control script streams `speedj` with 40 rad/s², eight times the MoveIt config value, and now contains `optimovej`/`optimovel` wrappers and controller-side cubic and quintic spline interpolation. A future exporter can target these instead of raw `servoj`.
- UR joint speed limits are not uniform across the range: UR3(e) wrists run at 360°/s, UR10e/12e/16e/20/30 bases at 120°/s, and the UR15/18/8long family has 240°/s elbows and 300°/s wrists. The limits must come from per-model data, never from a hard-coded 180°/s.
- No TOPP-RA or TOTG port exists on crates.io under an obvious name. The only reachability-analysis TOPP is copp, five months old.

Sources (web search):
- [MoveIt 2 PR #1780: remove iterative spline and parabolic time parameterization](https://github.com/moveit/moveit2/pull/1780)
- [MoveIt time parameterization tutorial](https://moveit.picknik.ai/main/doc/examples/time_parameterization/time_parameterization_tutorial.html)
- [UR script directory, PolyScope 5](https://www.universal-robots.com/manuals/EN/PDF/SW5_26/scriptmanualG5/script_directory_Poly5.pdf) (search snippet only)
- [UR article: servoj command](https://www.universal-robots.com/articles/ur/programming/servoj-command/) (search snippet only)
- [UR forum: understanding servoj and lookahead time](https://forum.universal-robots.com/t/understanding-servoj-and-lookahead-time/41813)
- [Robotiq DoF discussion 950](https://dof.robotiq.com/discussion/950)
