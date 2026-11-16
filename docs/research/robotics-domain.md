# GraphCTRL research: robotics domain facts and Rust foundations (2026-11-16)

Legend: **[V]** verified today from the cited primary source; **[S]** seen only as a search snippet of the named page; **[R]** recalled, unverified.

`www.universal-robots.com` and `docs.universal-robots.com` do not resolve in this session; both must be added to the session's allowed network domains. The official DH article, script manual and PolyScope manuals could not be opened, so UR facts come from UR's GitHub repositories [V], snippets [S] or memory [R].

## 1. Maths foundation

**What the ecosystem uses today** [V: crates.io dependency API, crate tarballs]

| Crate | Maths dependency |
|---|---|
| parry3d / parry3d-f64 0.31.1 | `glamx ^0.3` (glam 0.33); nalgebra is gone (`parry3d-0.31.1/Cargo.toml`) |
| rapier3d 0.36.1 | glamx ^0.3.1, plus nalgebra ^0.35 internally (SIMD, multibody Jacobians) |
| glamx 0.3.1 (Dimforge, MIT/Apache) | glam ^0.33.7; adds `Pose3`/`DPose3`, `Rot2`, 3x3 SVD/eigen; optional nalgebra ^0.35 conversions |
| bevy_math 0.20.1 | glam ^0.33.2 |
| rs-opw-kinematics 3.0.0 | glam ^0.32.1, f64 `DVec3`/`DQuat` (nalgebra removed in 2.0) |
| transform-gizmo 0.11.0 | glam ^0.32 + mint 0.5 |
| batchplan 0.11.0 | glam ^0.33, parry3d ^0.31 |
| k 0.32.0 (2024-09) | nalgebra ^0.30 |
| ik-geo 0.1.2 / caliper-ik 0.2.1 / copp 0.2.3 | nalgebra 0.32 / 0.35 / 0.35 |
| urdf-rs 0.10.0 | none |
| nalgebra 0.35.0 | converts to glam 0.30-0.33 only, not 0.34 |

- **Dimforge migration:** parry 0.26.0 and rapier 0.32.0 (9 Jan 2026) moved "from nalgebra to glam (via the glamx crate) for future compatibility with rust-gpu"; `Isometry` became `Pose` [V: dimforge/parry and dimforge/rapier `CHANGELOG.md`].
- **glam cadence:** 0.32 (2026-02-11), 0.33 (2026-05-21), 0.34.0 (2026-10-04) [V].
- **glam 0.34 breakage:** it removed `look_at_*`/`perspective_*`/`orthographic_*` from matrix types (now only in `glam::camera`/`dcamera`) and the unversioned `mint`/`approx`/`rkyv` feature names [V: glam-rs `CHANGELOG.md`].
- **glamx `DPose3`:** `#[repr(C)] { rotation: DQuat, translation: DVec3 }` with serde. `DPose3::new(translation, axisangle)` takes UR's rotation-vector form directly [V: `glamx-0.3.1/src/pose3.rs`].

**Decision**
- **Geometry:** use glam f64 types through `glamx 0.3` (`DVec2`, `DVec3`, `DQuat`, `DMat4`, `DPose3`), i.e. glam 0.33, not 0.34. parry3d-f64 then takes our types with no conversion.
- **Isolation:** only one workspace crate (`gc-math`) names glamx; everything else uses its re-exports and newtypes. glam's three breaking minors in 2026 make this necessary.
- **N-dimensional numerics:** sketch-solver Jacobians and 6xN manipulator Jacobians use nalgebra 0.35 or faer 0.24.4, private to the solver and kinematics crates. The glamx `nalgebra` feature bridges them.
- **Precision:**
  - f64 everywhere in document, solver and kinematics.
  - f32 only in GPU buffers. f32 spacing at 64-128 m is 7.6 µm, enough to jitter when zoomed in on a factory-scale scene.
  - Render camera-relative: compute `view * model` in f64 on the CPU and upload f32; mesh vertices stay f32 in part-local coordinates.
- **Interop:** bytemuck for GPU structs. mint 0.5 is the optional bridge between glam and egui's `emath` [V], but hand conversion at the UI boundary is simpler.
- **Units:**
  - Store SI (m, rad, s, kg), the same as URDF and URScript [R]; millimetres, inches and degrees are display-only.
  - Use private-field newtypes (`Length`, `Angle`, ...) rather than `uom` 0.38.0 [V: 2026-02-14]. About ten quantity kinds are needed.
  - Dimension expressions such as `2*d1 + 5 mm` need runtime dimension checking anyway, so the parameter engine carries a small `Quantity { si, dim }`.
- **Conventions:** right-handed, Z-up world.
  - UR `base`, URDF and REP-103 are Z-up [V]; SolidWorks defaults to Y-up, with a Z-up option since 2020 [S: help.solidworks.com]; Inventor is Y-up [S: forum posts].
  - Offer both through a per-document `UpAxis` that changes only the ViewCube and orbit axis, standard-plane naming, and import/export adaptors. Model data never changes.
- **Orientation:** the quaternion is the truth. Keep the user's entered representation (RPY or rotation vector) as the parametric input.

```rust
// crate gc-math: the only crate that names glamx
pub use glamx::{DVec2, DVec3, DQuat, DMat4, DPose3};      // glam 0.33 via glamx 0.3

macro_rules! quantity { ($n:ident, $unit:literal) => {
    #[doc = $unit]
    #[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Default, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)] #[repr(transparent)]
    pub struct $n(f64);                                   // SI inside, field private
}}
quantity!(Length, "metres");   quantity!(Angle, "radians");   quantity!(Time, "seconds");
quantity!(LinSpeed, "m/s");    quantity!(AngSpeed, "rad/s");  quantity!(Mass, "kg");
impl Length { pub fn mm(v: f64) -> Self { Self(v * 1e-3) }  pub fn as_mm(self) -> f64 { self.0 * 1e3 } }
impl Angle  { pub fn deg(v: f64) -> Self { Self(v.to_radians()) } }

// Points and free vectors are distinct types; S is a zero-sized space tag.
pub struct Point3<S>(pub DVec3, PhantomData<S>);
pub struct Vector3<S>(pub DVec3, PhantomData<S>);
pub struct Transform<From, To>(pub DPose3, PhantomData<(From, To)>);
impl<A, B> Transform<A, B> {
    pub fn apply(&self, p: Point3<A>) -> Point3<B> { Point3(self.0.transform_point(p.0), PhantomData) }
    pub fn then<C>(&self, next: &Transform<B, C>) -> Transform<A, C> { /* next * self */ todo!() }
    pub fn inverse(&self) -> Transform<B, A> { Transform(self.0.inverse(), PhantomData) }
}
pub enum World {}  pub enum Local {}  pub enum Sketch {}  pub enum View {}   // static pipeline spaces
// user-created frames are runtime FrameIds, not type tags

/// UR pose p[x,y,z,rx,ry,rz] = metres + rotation vector (axis * angle)
pub fn pose_from_ur(p: [f64; 6]) -> DPose3 {
    DPose3::new(DVec3::new(p[0], p[1], p[2]), DVec3::new(p[3], p[4], p[5]))
}
/// URDF rpy is fixed-axis XYZ: R = Rz(yaw) * Ry(pitch) * Rx(roll)
pub fn quat_from_urdf_rpy(r: f64, p: f64, y: f64) -> DQuat {
    DQuat::from_rotation_z(y) * DQuat::from_rotation_y(p) * DQuat::from_rotation_x(r)
}
```

## 2. Universal Robots facts

**Product line.** [V] `Universal_Robots_ROS2_Description` (branch `rolling`) has `config/<model>/` for CB3 ur3/ur5/ur10, e-Series ur3e/ur5e/ur7e/ur10e/ur12e/ur16e, UR Series ur8long/ur15/ur18/ur20/ur30, and Gen 7 g-Series ur10g-1750/ur17g-1300/ur18g-950. Gen 7 was announced 2026-09-14 at IMTS [S].

**Nominal kinematics.** UR uses standard DH with a = [0, a2, a3, 0, 0, 0], d = [d1, 0, 0, d4, d5, d6], alpha = [π/2, 0, 0, π/2, −π/2, 0]. The alpha pattern is [R] but consistent with the YAML rolls. The lengths below are [V] from `config/<model>/default_kinematics.yaml`, mapped as d1 = shoulder.z, a2 = forearm.x, a3 = wrist_1.x, d4 = wrist_1.z, d5 = −wrist_2.y, d6 = wrist_3.y. Speeds are [V] from `joint_limits.yaml`.

| Model | d1 | a2 | a3 | d4 | d5 | d6 | Max joint speed, °/s (J1..J6) | Payload / reach |
|---|---|---|---|---|---|---|---|---|
| UR3e | 0.15185 | −0.24355 | −0.2132 | 0.13105 | 0.08535 | 0.0921 | 180, 180, 180, 360, 360, 360 | 3 kg / 500 mm [R] |
| UR5e, UR7e | 0.1625 | −0.425 | −0.3922 | 0.1333 | 0.0997 | 0.0996 | 180 on all six | 5 and 7.5 kg / 850 mm [R] |
| UR10e, UR12e | 0.1807 | −0.6127 | −0.57155 | 0.17415 | 0.11985 | 0.11655 | 120, 120, 180, 180, 180, 180 | 12.5 kg / 1300 mm [R] |
| UR16e | 0.1807 | −0.4784 | −0.36 | 0.17415 | 0.11985 | 0.11655 | as UR10e | 16 kg / 900 mm [R] |
| UR8 Long, UR10g-1750 | 0.2186 | −0.8989 | −0.7149 | 0.1824 | 0.1361 | 0.1434 | 180, 180, 240, 300, 300, 300 | 8 (10) kg / 1750 mm [S] |
| UR15, UR17g-1300 | 0.2186 | −0.6475 | −0.5164 | 0.1824 | 0.1361 | 0.1434 | as UR8 Long | 15 (17.5) kg / 1300 mm [S] |
| UR18, UR18g-950 | 0.2186 | −0.475 | −0.3389 | 0.1824 | 0.1361 | 0.1434 | as UR8 Long | 18 kg / 950 mm [S] |
| UR20 | 0.2363 | −0.862 | −0.7287 | 0.201 | 0.1593 | 0.1543 | 120, 120, 150, 210, 210, 210 | 20 kg / 1750 mm [S] |
| UR30 | 0.2363 | −0.637 | −0.5037 | 0.201 | 0.1593 | 0.1543 | as UR20 | 30 (35) kg / 1300 mm [S] |
| CB3 UR3 | 0.1519 | −0.24365 | −0.21325 | 0.11235 | 0.08535 | 0.0819 | as UR3e | |
| CB3 UR5 | 0.089159 | −0.425 | −0.39225 | 0.10915 | 0.09465 | 0.0823 | 180 on all six | |
| CB3 UR10 | 0.1273 | −0.612 | −0.5723 | 0.163941 | 0.1157 | 0.0922 | as UR10e | |

- **Shared kinematics** [V]: UR7e has the same kinematics hash and limits as UR5e, and UR12e the same as UR10e. The g-Series nominal values equal UR8 Long / UR15 / UR18.
- **Joint limits** [V]:
  - All joints are ±360°. UR3/UR3e wrist 3 has no position limit.
  - The elbow's ±180° in the ROS files is an artificial MoveIt workaround (stated in the YAML comment).
  - "Acceleration limits are not publicly available."
  - UR5e torques are 150/150/150/28/28/28 N·m.
- **Calibration:**
  - Each robot is factory-calibrated; with nominal values "end effector positions might be off in the magnitude of centimeters" [V: `ur_calibration/README.md`].
  - The primary interface's `KinematicsInfo` message carries `checksum`, `dh_theta`, `dh_a`, `dh_d`, `dh_alpha` and `calibration_status` [V: client library `kinematics_info.h`].
  - `default_kinematics.yaml` is the per-robot replaceable file, tagged `hash: calib_...` [V].
  - On the controller the deltas live in `calibration.conf` [R].
- **Frames** [V: `urdf/ur_macro.xacro`, `doc/robot_frames.rst`]:
  - `base_link` follows REP-103 (X forward, Y left, Z up).
  - `base` is the controller's Base, called a "Base feature" in PolyScope 5 and a "base frame" in PolyScope X. It is `base_link` rotated 180° about Z.
  - `flange` is the attachment frame, with X pointing out of the flange.
  - `tool0` is the all-zero TCP ("X+ left, Y+ up, Z+ front"). Composing the xacro rotations shows it has the same orientation as `wrist_3_link`.
  - `tool0` relative to `base` equals the teach-pendant pose.
- **Parameterisation** [V: `doc/index.rst`]: four YAML files per model.
  - `default_kinematics.yaml`: per-joint xyz + rpy, plus the calibration hash.
  - `joint_limits.yaml`: uses a custom `!degrees` tag.
  - `physical_parameters.yaml`: masses, centres of gravity, inertia tensors.
  - `visual_parameters.yaml`: `.dae` visual and `.stl` collision meshes, each with a `mesh_offset`.
  - The top level is xacro, not plain URDF.
- **Licence** [V: repo `README.md`]: BSD-3-Clause, except the meshes for UR8 Long, UR15, UR18, UR20, UR30, the three g-Series arms and the Smart Panel. Those are "Graphical Documentation" under UR terms that "do not fully comply with OSI's definition of Open Source".
- **IK:**
  - Hawkins' `ur_kin.h` (BSD) declares `int inverse(const double* T, double* q_sols, double q6_des=0.0)`, "maximum of 8" solutions in [0, 2π) [V: ros-industrial/universal_robot].
  - With ±360° joints, each branch also has 2π turn variants.
  - Rust options: ik-geo 0.1.2 (BSD-3) has a hardcoded `ur5()` but depends on nalgebra 0.32 and `nlopt` (C) [V]; caliper-ik 0.2.1 claims "analytic 6R" but has a single release [V]. Nothing is mature.
  - The analytic solution holds only for nominal geometry; calibrated models need a numeric refinement step.

Derived package format (data-only; UR5e values [V]):

```toml
package = { id = "com.universal-robots.ur5e", name = "UR5e", format = 1, variants = ["ur7e"] }
licence = { description = "BSD-3-Clause", meshes = "BSD-3-Clause" }  # newer arms: UR Graphical Documentation terms

[kinematics]                     # standard DH, theta_i = q_i
a     = [0.0, -0.425, -0.3922, 0.0, 0.0, 0.0]
d     = [0.1625, 0.0, 0.0, 0.1333, 0.0997, 0.0996]
alpha = [1.5707963267948966, 0.0, 0.0, 1.5707963267948966, -1.5707963267948966, 0.0]
ik    = { solver = "ur-analytic", refine = "newton" }   # refine is mandatory with calibration deltas

[frames]                         # as in ur_macro.xacro
base   = { parent = "base_link",    rpy_deg = [0, 0, 180] }
flange = { parent = "wrist_3_link", rpy_deg = [0, -90, -90] }
tool0  = { parent = "flange",       rpy_deg = [90, 0, 90] }

[[joint]]
name = "shoulder_pan_joint"
limit_deg = [-360, 360]
max_speed_deg_s = 180
max_torque_nm = 150              # five more; UR3e wrist_3 is continuous

[[link]]
name = "upper_arm_link"
mass_kg = 8.058
cog_m = [-0.2125, 0.0, 0.11336]
visual    = { mesh = "meshes/visual/upperarm.dae",    xyz = [0.0, 0.0, 0.138], rpy_deg = [90, 0, -90] }
collision = { mesh = "meshes/collision/upperarm.stl", xyz = [0.0, 0.0, 0.138], rpy_deg = [90, 0, -90] }

[controller]
dialect = "urscript"
native_moves = ["movej", "movel", "movep", "movec"]
# per robot INSTANCE, never in the package:
# calibration = { delta_a = [...], delta_d = [...], delta_alpha = [...], delta_theta = [...], hash = "calib_..." }
```

## 3. UR programming model to mirror

- **Moves:**
  - MoveJ is joint-interpolated; MoveL moves the TCP linearly.
  - MoveP "will move the tool in linear motion at constant speed with circular blends", and a CircleMove is two waypoints (via, end) mapping to `movec` [S: UR article "Circular path using MoveP/MoveC"].
  - Defaults [R]: movej a = 1.4 rad/s², v = 1.05 rad/s; movel/movep a = 1.2 m/s², v = 0.25 m/s.
- **Waypoints:** Fixed, Relative and Variable position ([S] manual table of contents; details [R]). A waypoint stores a pose plus joint angles, and a Move selects a TCP and a Feature [R].
- **Blend:** radius `r` in metres around a waypoint.
  - Overlapping blends skip waypoints in motion version 1.
  - Motion version 2 "dynamically reduce[s] blend radii"; from PolyScope X 10.9 it is the only version.
  - No blending happens when time is given instead of speed and acceleration [S: script manual SW 5.22 / 10.15].
- **Features:** Base, Tool, Point, Line, Plane [S: URCap API `domain.feature`]. PolyScope X calls them frames [V: `robot_frames.rst`]. These map 1:1 to CAD work features.
- **Installation:**
  - TCP offsets.
  - Payload: `set_payload(mass, cog)` and `set_target_payload(mass, cog, inertia, transition_time)` [V: `external_control.urscript`].
  - Mounting [R].
  - Safety planes and limits [R].
- **URScript calls used by UR's own driver script** [V: `Universal_Robots_Client_Library/resources/external_control.urscript`]: `movej`, `movel`, `movep`, `movec`, `optimovej`/`optimovel(target, a, v, r)`, `servoj(q, t=...)`, `speedj`, `speedl(twist, a, t)`, `stopj`, `stopl`, `get_inverse_kin(pose, qnear)`, `force_mode(...)`, `set_tcp`. Full signatures, and `p[x,y,z,rx,ry,rz]` as metres plus rotation vector, are [R].
- **Interfaces:**
  - Ports: 29999 Dashboard, 30001 Primary, 30002 Secondary, 30003 Real-time, 30004 RTDE, 30020 Interpreter, 30011-30013 read-only [S: UR "Overview of client interfaces"].
  - 30003 is "Deprecated. Not available in Polyscope X" [S: page title].
  - PolyScope X has no Dashboard server; a REST "Robot API" arrived with 10.11.0 and covers a subset [S: ur_client_library docs].
  - External control uses reverse and trajectory sockets 50001-50003 [V: ur-driver `README.md`].
  - Rates are [R]: RTDE 500 Hz on e-Series, 125 Hz on CB3.
- **URSim** [S: Docker Hub]: `universalrobots/ursim_e-series` (VNC), `ursim_cb3`, and `ursim_polyscopex` (web UI on port 80, amd64 and arm64).
- **URCap / URCapX:** URCap is Java/OSGi [R]. URCapX is a containerised backend plus web frontend, with REST through ingress on port 80 [S: PolyScope X SDK docs].
- **Rust crates** [V]:
  - `ur-driver` 0.1.0 (2026-08-11, Apache-2.0 AND BSD-3-Clause, 21 downloads) is a pure-Rust port of the client library v2.14.1, covering RTDE, primary, dashboard, reverse interface and script sender.
  - `ur-script` 0.2.0 (2022) and `rur` 0.1.0 (2020) are dead.

## 4. Rust ecosystem short-list

All versions, dates and licences below are [V] from the crates.io API unless marked.

| Area | Use | Watch | Avoid |
|---|---|---|---|
| Geometry, collision | glamx 0.3.1; parry3d-f64 0.31.1 (Apache-2.0) | rapier3d 0.36.1 with rapier3d-urdf and mjcf-rs 0.36.1 | avian3d 0.8.0 (Bevy-bound); mujoco-rs 7.1.0 (C library) |
| Kinematics, IK | own FK/IK in the UR plugin | batchplan 0.11.0 (first release 2026-10-21; URDF/MJCF/USD, RRT-Connect, f32 API); caliper-* 0.2.1 | k 0.32.0 (2024); rs-opw-kinematics 3.0.0 for UR (spherical wrist only); ik-geo as a dependency |
| Robot formats | urdf-rs 0.10.0 | xurdf 0.6.6; xacro-pure 0.1.1; sdformat 0.1.0; openusd 0.8.0 | |
| Meshes | mesh-loader 0.1.13 (STL/COLLADA/OBJ, last release 2024-10); gltf 1.4.1; stl_io 0.11.0 | dae-parser 0.11.0; tobj 4.0.5 | |
| STEP | none yet | stepmesh 0.1.0 (2026-11-03, pure Rust, outputs mm); haecceity 0.5.4 (first release 2026-10-27); monstertruck 0.4.2 | truck-stepio 0.3.0 / ruststep 0.4.0 (2024-09); opencascade 0.3.0 / occt-sys 7.8.1 (LGPL-2.1, C++ build) |
| Planning | none yet | oxmpl 0.8.1 (BSD-3; RRT, RRT-Connect, RRT*, PRM; SE3/SO3 spaces; no maths dependency) | rrt 0.7.0; openrr-planner 0.1.0 (2023) |
| Trajectory | rsruckig 3.0.1 (MIT, pure Rust); kurbo 0.13.1 for 2D curves | copp 0.2.3 (TOPP and convex objectives; nalgebra 0.35 + clarabel) | ruckig 0.19.4 (cxx + C++) |
| ROS 2 | none yet | ros2-client 0.12.1 (RustDDS); zenoh 1.10.2 | r2r 0.9.7 and rclrs 0.8.0 as hard dependencies (need a ROS install [R]) |
| BT, FSM, PDDL | petgraph 0.8.3 plus own runtime graph model | bonsai-bt 0.14.0; ferroplan 0.30.0 (MIT/Apache) | behaviortree-rs 0.2.4 (2024); pddl 0.4.0 and miniplan (EUPL-1.2); statig/rust-fsm/smlang for user-authored machines (compile-time DSLs) |
| Node UI, layout | rust-sugiyama 0.4.0 (petgraph ^0.8.1) | egui-snarl 0.12.0 and egui_graphs 0.32.0 (both egui ^0.36); layout-rs 0.1.3 | egui_node_graph2 0.7.0 (2024); fdg-sim 0.9.1 (2022); dagre_rust 0.0.5 |

Not on crates.io under these names: `optik`, `eaik`, `toppra`, `ompl`, `fdg` [V: API 404].

## 5. Motion-planning taxonomy and common interface

| Family | Members | Notes |
|---|---|---|
| Joint interpolation | trapezoid (UR `movej`), S-curve / jerk-limited (Ruckig), quintic | synchronised axes |
| Cartesian | LIN (slerp orientation), CIRC (3-point), spline, constant-speed process move with blends | needs IK along the path with branch continuity and singularity handling |
| Sampling-based | RRT, RRT-Connect, RRT*, PRM | plus shortcut and smoothing passes |
| Optimisation-based | CHOMP, STOMP, TrajOpt | usually seeded by the two rows above |
| Constrained | orientation cones, Cartesian-path graph search over IK branches and turns | graph search suits UR's 8 x turns solutions |
| Time parameterisation | per-segment trapezoid, TOTG, TOPP-RA / COPP, Ruckig | applies to any geometric path |
| Collision handling | validate-only or avoid | orthogonal to all of the above |
| Controller-native | emit `movej`/`movel`/`movep` and emulate the controller | differs from app-planned trajectories streamed by `servoj` or trajectory forwarding |

All families fit a two-stage pipeline (geometric path, then timing) with one request type; see the traits in section 6.

## 6. Data-model implications for milestone 1

- **Identity:** every user-visible entity gets a persistent, typed, never-reused ID. slotmap keys are runtime-only.
- **Frame graph from day one:** World, then equipment body, mount frame, robot base, flange, TCP. Work planes, axes and points are frames with parametric definitions. A sketch lives on a `FrameId`.
- **Imported CAD:** reference it through pose-anchored frames, not face or edge indices; this sidesteps topological naming.
- **Two layers:** the motion layer references the geometry layer and never copies coordinates.
  - A sketch point gives position only.
  - Orientation, arm configuration, blend and speed are waypoint attributes.
- **Derived data:** solver and planner results are caches, not document state.
- **Update flow:**
  1. Parameter edit; sketch re-solve as a transaction that keeps the last good solution on failure.
  2. Changed-entity set from revision counters.
  3. Dependent frames and waypoints recomputed.
  4. Validation: IK with the configuration hint, limits, singularity margin, collision.
  5. Only segments whose input hash changed are re-planned.
  6. Diagnostics are attached to `WaypointId` / `SegmentId`.

```rust
macro_rules! id { ($n:ident) => {
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug, Serialize, Deserialize)]
    pub struct $n(NonZeroU64); } }
id!(FrameId); id!(SketchId); id!(SkPointId); id!(SkCurveId); id!(ConstraintId); id!(ParamId);
id!(BodyId); id!(RobotId); id!(TcpId); id!(WaypointId); id!(PathId); id!(SegmentId);

pub struct Frame { pub id: FrameId, pub parent: Option<FrameId>, pub name: String, pub def: FrameDef }
pub enum FrameDef {
    Fixed(DPose3),                                               // pose in parent
    WorkPlane(PlaneDef), WorkAxis(AxisDef), WorkPoint(PointDef), // parametric (ParamId / other frames)
    BodyAnchor { body: BodyId, local: DPose3 },                  // imported equipment
    RobotLink { robot: RobotId, link: LinkName },                // base, flange, tool0 (from FK)
    Tcp { robot: RobotId, tcp: TcpId },
}

pub enum PositionSource {
    SketchPoint { sketch: SketchId, point: SkPointId },
    OnCurve { sketch: SketchId, curve: SkCurveId, t: f64 },      // arc-length normalised
    Explicit { frame: FrameId, xyz: [Length; 3] },
    Offset { base: WaypointId, in_frame: FrameId, delta: [Length; 3] },   // UR relative waypoint
}
pub enum OrientationSource {
    Explicit { frame: FrameId, rot: DQuat },
    PlaneNormal { twist: Angle },                                // tool Z = -sketch-plane normal
    Tangent { twist: Angle, lead: Angle, tilt: Angle },          // follow the curve
    SameAs(WaypointId),
}
pub struct ArmConfig { pub shoulder: Side, pub elbow: UpDown, pub wrist: Flip, pub turns: [i8; 6] }
pub enum ConfigHint { Any, Branch(ArmConfig), Seed(JointVector), ExactJoints(JointVector) }
pub struct Waypoint { pub id: WaypointId, pub name: String,
    pub position: PositionSource, pub orientation: OrientationSource,
    pub config: ConfigHint, pub blend: Option<Length>, pub speed: Option<SpeedSpec> }

pub enum MotionKind { Joint, Linear, Circular { via: WaypointId }, Spline, Process, Free }
pub struct Segment { pub id: SegmentId, pub to: WaypointId, pub motion: MotionKind,
    pub follow: Option<(SketchId, SkCurveId)>,                   // exact geometry to trace, if any
    pub strategy: StrategyRef, pub limits: MotionLimits }
pub struct StrategyRef { pub planner: PlannerKey,                // "core.lin", "ur.native.movep", ...
                         pub params: ParamBlob }                 // validated against the planner schema
pub struct Path { pub id: PathId, pub robot: RobotId, pub tcp: TcpId, pub reference: FrameId,
                  pub start: WaypointId, pub segments: Vec<Segment> }

pub trait PathPlanner: Send + Sync {
    fn descriptor(&self) -> &PlannerDescriptor;                  // key, MotionKinds, param schema
    fn plan(&self, req: &PlanRequest<'_>, cx: &PlanCx<'_>) -> Result<JointPath, PlanError>;
}
pub trait TimeParameterizer: Send + Sync {
    fn retime(&self, p: &JointPath, l: &MotionLimits, cx: &PlanCx<'_>) -> Result<JointTrajectory, PlanError>;
}
pub struct PlanRequest<'a> {
    pub robot: &'a RobotModel,               // nominal kinematics, limits, optional calibration
    pub tcp: &'a DPose3, pub payload: &'a Payload,
    pub scene: &'a dyn CollisionScene,       // immutable snapshot; parry3d-f64 behind the trait
    pub start: JointState,
    pub goals: &'a [Goal<'a>],               // Joint(q) | Pose { in_base, config } | Follow(&dyn Curve3)
    pub constraints: &'a [PathConstraint],
    pub tolerances: Tolerances,
}
pub struct PlanCx<'a> { pub cancel: &'a CancelToken, pub seed: u64, pub progress: &'a dyn Progress }
pub struct JointTrajectory { pub samples: Vec<TrajSample> /* t, q, qd, qdd */, pub duration: Time }
pub struct PlanReport { pub diagnostics: Vec<Diagnostic>, pub stats: PlanStats }
```

## Sources

- crates.io API and `static.crates.io` tarballs: parry3d 0.31.1, glamx 0.3.1, rs-opw-kinematics 3.0.0, ur-driver 0.1.0, ik-geo 0.1.2, ruckig 0.19.4, rsruckig 3.0.1, oxmpl 0.8.1, stepmesh 0.1.0, haecceity 0.5.4.
- `raw.githubusercontent.com/dimforge/parry/master/CHANGELOG.md`, `.../dimforge/rapier/master/CHANGELOG.md`, `.../bitshifter/glam-rs/main/CHANGELOG.md`.
- `raw.githubusercontent.com/UniversalRobots/Universal_Robots_ROS2_Description/rolling/`: `README.md`, `config/*/*.yaml`, `urdf/ur_macro.xacro`, `doc/index.rst`, `doc/robot_frames.rst`.
- `raw.githubusercontent.com/UniversalRobots/Universal_Robots_ROS2_Driver/main/ur_calibration/README.md`.
- `raw.githubusercontent.com/UniversalRobots/Universal_Robots_Client_Library/master/`: `resources/external_control.urscript`, `include/ur_client_library/primary/robot_state/kinematics_info.h`.
- `raw.githubusercontent.com/ros-industrial/universal_robot/noetic-devel/ur_kinematics/include/ur_kinematics/ur_kin.h`.
- Snippets only:
  - [UR client interfaces overview](https://www.universal-robots.com/articles/ur/interface-communication/overview-of-client-interfaces/)
  - [UR real-time guide](https://docs.universal-robots.com/tutorials/communication-protocol-tutorials/realtime-guide.html)
  - [ur_client_library dashboard client](https://docs.ros.org/en/rolling/p/ur_client_library/doc/architecture/dashboard_client.html)
  - [UR motion version](https://www.universal-robots.com/manuals/EN/HTML/SW10_15/Content/prod-scriptmanual/all_scripts/motion_version_desc.htm)
  - [MoveP/MoveC article](https://www.universal-robots.com/articles/ur/programming/circular-path-using-movepmovec/)
  - [URCap feature API](https://www.universal-robots.com/apidoc/115998/com/ur/urcap/api/domain/feature/package-summary.html)
  - [ursim_polyscopex](https://hub.docker.com/r/universalrobots/ursim_polyscopex)
  - [UR Gen 7 news](https://www.universal-robots.com/news-and-media/news-center/universal-robots-unveils-gen-7-new-platform-industrial-automation-physical-ai)
  - [UR FAQ](https://www.universal-robots.com/products/faq)
  - [SolidWorks up axis](https://help.solidworks.com/2020/English/WhatsNew/t_view_up_axis.htm)

## Recommendation

1. **Maths:** build on glamx 0.3 (glam 0.33) in f64, wrapped in a `gc-math` crate that owns `Length`/`Angle` newtypes, `Point3<S>`/`Vector3<S>`/`Transform<A, B>`, and UR/URDF rotation conversions with round-trip property tests. Ban direct `glam` and `nalgebra` imports elsewhere (clippy `disallowed_types`, cargo-deny duplicate-version check). Keep nalgebra or faer private to the solver.
2. **Units and axes:** SI storage, millimetres and degrees as display defaults, Z-up right-handed world, and a per-document `UpAxis` view preference so Inventor and SolidWorks users can have Y-up.
3. **Milestone 1 model:** ship the frame graph, work plane/axis/point features, a named-parameter table, persistent typed IDs, and externally addressable sketch points and curves, even though only sketching is visible. Waypoints and paths then bind to them later with no migration.
4. **Motion model:** mirror UR's concepts and names (Feature/Frame, TCP, fixed/relative/variable waypoint, blend radius, MoveJ/L/P/C). Store arm configuration as branch plus turns.
5. **Robots:** ship them as data-only packages modelled on UR's four YAML files, with per-asset licence fields and calibration per robot instance. Write the UR plugin's analytic IK in-house (Hawkins method plus Newton refinement) and test it against URSim.
6. **Planning:** use a two-stage registry (`PathPlanner`, then `TimeParameterizer`) keyed by string, deterministic by seed and cancellable. First strategies: `core.joint`, `core.lin`, `core.circ`, `ur.native.*`. rsruckig is the first external dependency.

## Risks and unknowns

- **glam version skew:** the ecosystem is split across 0.32, 0.33 and 0.34. The plan depends on glamx and nalgebra following glam promptly.
- **Unverified UR facts:** the DH alpha pattern, URScript signatures and defaults, PolyScope waypoint semantics and all payload/reach figures are recalled or secondary. Re-verify them once the UR hosts are allowed.
- **Mesh licence:** shipping the newer UR meshes with the application needs a legal check; they may have to be downloaded by the user.
- **Simulation fidelity:** controller-native moves (blends, motion version 1 vs 2, optimove) can only be approximated without URSim. Calibrated robots deviate from nominal by up to centimetres.
- **STEP import:** the pure-Rust importers are weeks old, truck has been stale since 2024-09, and OpenCascade means LGPL plus a C++ build. Equipment import may start mesh-only (STL, OBJ, glTF, COLLADA).
- **Young crates:** ur-driver, batchplan, caliper and oxmpl are too new to depend on. `pddl` is EUPL-1.2.
- **egui lag:** the node-graph and gizmo crates target egui 0.36, not 0.37.
- **PolyScope X interfaces:** it removes Dashboard and port 30003, so the comms layer must be capability-based per controller generation.

## Surprises

- Dimforge left nalgebra: parry3d and rapier have been glam-based since January 2026, and rs-opw-kinematics followed. "nalgebra is the robotics standard" no longer holds for geometry types.
- The newest glam (0.34.x) is the wrong pick today. 0.33 is the version parry, rapier, bevy_math and nalgebra's conversions agree on.
- OPW solvers cannot handle UR arms, which have no spherical wrist. No mature Rust crate offers UR closed-form IK.
- UR7e and UR12e are kinematically identical to UR5e and UR10e in UR's own description. The g-Series reuses the UR8 Long / UR15 / UR18 nominal kinematics.
- The widely copied ±180° elbow limit is a MoveIt workaround. Real joints are ±360°, so IK has more than eight candidates once turns are counted.
- UR's `base` is rotated 180° about Z from ROS `base_link`. `flange` has X pointing out while `tool0` has Z pointing out.
- The meshes of UR's newer arms are not open source.
- The `ruckig` crate is C++ behind cxx; `rsruckig` is the pure-Rust port.
