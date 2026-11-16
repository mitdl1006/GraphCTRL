# URDF-driven robot model, FK and IK for GraphCTRL: verified specification

Legend: **[V]** verified today from the cited source; **[N]** verified numerically today in a throwaway stdout-only script (NumPy port of the fetched sources); **[R]** recalled from memory, not re-verified.

Sources used below:
- `urdf-rs`: `https://static.crates.io/crates/urdf-rs/urdf-rs-0.10.0.crate`
- UR description (`URD`): `https://raw.githubusercontent.com/UniversalRobots/Universal_Robots_ROS2_Description/rolling/`
- Hawkins IK (`URK`): `https://raw.githubusercontent.com/ros-industrial/universal_robot/noetic-devel/ur_kinematics/`

---

## 1. URDF parsing: urdf-rs 0.10.0 and alternatives

### urdf-rs 0.10.0 [V: Cargo.toml, src/{lib,deserialize,funcs,utils,errors}.rs]

- **Licence / MSRV:** Apache-2.0; no `rust-version` declared. It uses `std::sync::LazyLock` and let-else, so effectively Rust ≥ 1.80.
- **Dependencies:** `RustyXML 0.3`, `serde-xml-rs 0.6`, `quick-xml 0.39` (serialisation only), `regex`, `serde`, `thiserror 2`. That is three XML stacks.
- **Entry points:** `read_file(path)`, `read_from_string(&str)`, `write_to_string(&Robot)`. In `utils`: `read_urdf_or_xacro[_with_args]`, `convert_xacro_to_urdf[_with_args]`, `rospack_find`, `expand_package_path(filename, base_dir)`.
- **Parse pipeline:** RustyXML DOM → version-dependent rewrite → re-sort of children → `serde_xml_rs::from_str`. All non-link/joint/material children of `<robot>` are discarded (`<transmission>`, `<gazebo>`, `<ros2_control>`).
- **URDF versions:** 1.1 `quat_xyzw` on `<origin>` is converted to rpy. 1.2 adds `acceleration`/`deceleration`/`jerk` on `<limit>`, with omitted limits meaning ±inf and validation of negative values and `upper < lower`. For files below 1.2 those attributes are stripped.
- **xacro:** shells out. `Command::new("rosrun") xacro xacro --inorder`, falling back to `xacro`.
- **package://:** `expand_package_path` shells out to `rospack find` / `ros2 pkg prefix` and calls `.unwrap()` on the result, so it panics without ROS. `file://` is stripped; relative paths are joined to `base_dir`; http(s) is passed through.
- **Error type:** `UrdfError`, an opaque newtype over a private `ErrorKind` (File, Xml, RustyXml, QuickXmlSe/De, Command, Other(String)). No public kind, no line or column.

Types (all `pub` fields):

| Type | Fields |
|---|---|
| `Robot` | `name: String`, `version: Option<String>`, `links: Vec<Link>`, `joints: Vec<Joint>`, `materials: Vec<Material>` |
| `Link` | `name`, `inertial: Inertial` (serde default, not `Option`), `visual: Vec<Visual>`, `collision: Vec<Collision>` |
| `Inertial` | `origin: Pose`, `mass: Mass{value: f64}`, `inertia: Inertia{ixx, ixy, ixz, iyy, iyz, izz}` |
| `Visual` | `name: Option<String>`, `origin: Pose`, `geometry: Geometry`, `material: Option<Material>` |
| `Collision` | `name: Option<String>`, `origin: Pose`, `geometry: Geometry` |
| `Geometry` | `Box{size: Vec3}`, `Cylinder{radius, length}`, `Capsule{radius, length}`, `Sphere{radius}`, `Mesh{filename: String, scale: Option<Vec3>}` |
| `Material` | `name`, `color: Option<Color{rgba: Vec4}>`, `texture: Option<Texture{filename}>` |
| `Pose` | `xyz: Vec3`, `rpy: Vec3` (both default 0) |
| `Vec3` / `Vec4` | newtypes over `[f64; 3]` / `[f64; 4]` with `Deref` |
| `Joint` | `name`, `joint_type: JointType`, `origin: Pose`, `parent: LinkName{link}`, `child: LinkName{link}`, `axis: Axis{xyz}` (default `1 0 0`), `limit: JointLimit` (default, not `Option`), `calibration`, `dynamics`, `mimic`, `safety_controller` (all `Option<_>`) |
| `JointType` | `Revolute, Continuous, Prismatic, Fixed (default), Floating, Planar, Spherical` |
| `JointLimit` | `lower`, `upper`, `effort` (default 0), `velocity` (required when `<limit>` is present), `acceleration`, `deceleration`, `jerk` (default +inf) |
| `Mimic` | `joint: String`, `multiplier: Option<f64>`, `offset: Option<f64>` |
| `SafetyController` | `soft_lower_limit`, `soft_upper_limit`, `k_position`, `k_velocity` |
| `Dynamics` / `Calibration` | `{damping, friction}` / `{rising, falling: Option<f64>}` |

Weaknesses that matter for a CAD-grade importer:
- A missing `<limit>` or `<inertial>` is indistinguishable from all-zero values.
- No structural validation (tree shape, duplicate names, dangling links).
- Unparsable numeric tokens in vectors are silently dropped by `filter_map`, then reported only as a count mismatch.
- Errors carry no source position.
- The `utils` module spawns processes and can panic.

### Alternatives [V: crates.io API and crate tarballs]

| Crate | Version / date | Finding |
|---|---|---|
| roxmltree | 0.21.2, 2026-10-24, MIT OR Apache-2.0, MSRV 1.60 | Read-only DOM. Node text positions (line/column) are [R]. |
| quick-xml | 0.42.0, 2026-08-22, MIT, MSRV 1.86 | Streaming; more code for the same job. |
| xurdf | 0.6.6, MIT | Depends on nalgebra 0.33, `pyisheval`, roxmltree 0.14, xmltree, anyhow. Its xacro subset uses a Python-ish evaluator. Reject. |
| xacro-pure | 0.1.1, 2026-07-13, Apache-2.0, 69 downloads | See below. |

xacro-pure, from its README, claims a full port:
- properties with scopes, lazy/eager evaluation and cycle detection;
- macros with params, defaults, `^` forwarding, block and `**` params;
- includes (`ns=`, `optional=`, globs) through an injected reader;
- `xacro:if` / `xacro:unless`;
- `$(find/arg/eval/env/optenv/dirname/cwd)`;
- `load_yaml` including the `!degrees` / `!radians` constructors.

It achieves Python parity by embedding `rustpython-vm =0.4.0` (exact pin), plus `malachite-bigint =0.2.3`, `xmltree`, `yaml-rust2 0.10`.

On paper it covers every construct UR's files use (all seen in the fetched xacro [V]): `xacro.load_yaml`, `!degrees`, `scope="parent"`, `name:=^`, `*origin` with `insert_block`, `${type in dict[name]}`, Python conditional expressions, `$(find pkg)`. Its tests cover SO-ARM and OpenArm, not UR. I did not verify that it actually expands UR's files.

### Verdict

- **Write our own URDF reader on roxmltree** (about 500 lines). URDF has roughly 15 element types. We need `Option`-accurate fields, positioned diagnostics, warnings as data, and a pluggable resource resolver.
- **Use urdf-rs as a dev-dependency only**, as a differential-test oracle. It is the acceptable fallback if schedule is tight, provided it is wrapped and `urdf_rs::utils` is never called.
- **No xacro at runtime.** A whole Python VM with exact-pinned dependencies and 69 downloads is the wrong reliability trade. The UR plugin should generate its model natively (section 2).

---

## 2. Official UR description, expanded for UR5e

Source: `URD/urdf/ur_macro.xacro`, `urdf/inc/ur_common.xacro`, `urdf/ur.urdf.xacro`, `config/ur5e/*.yaml` [V].

`ur.urdf.xacro` adds `<link name="world"/>` and instantiates `ur_robot` with `parent="world"` and origin 0. The topology is fixed; only numbers come from the four YAML files. With the default `tf_prefix=""`, `safety_limits=false` and no smart panel, the expansion is as follows.

### Joints (π = 3.14159265358979)

| # | Joint | Type | Parent → child | xyz (m) | rpy (rad) | Axis | lower / upper | vel (rad/s) | effort (N·m) |
|---|---|---|---|---|---|---|---|---|---|
| 0 | `base_joint` | fixed | world → base_link | 0 0 0 | 0 0 0 | – | – | – | – |
| 1 | `base_link-base_link_inertia` | fixed | base_link → base_link_inertia | 0 0 0 | 0 0 π | – | – | – | – |
| 2 | `shoulder_pan_joint` | revolute | base_link_inertia → shoulder_link | 0 0 0.1625 | 0 0 0 | 0 0 1 | ∓6.283185 | 3.141593 | 150 |
| 3 | `shoulder_lift_joint` | revolute | shoulder_link → upper_arm_link | 0 0 0 | 1.570796327 0 0 | 0 0 1 | ∓6.283185 | 3.141593 | 150 |
| 4 | `elbow_joint` | revolute | upper_arm_link → forearm_link | −0.425 0 0 | 0 0 0 | 0 0 1 | **∓3.141593** | 3.141593 | 150 |
| 5 | `wrist_1_joint` | revolute | forearm_link → wrist_1_link | −0.3922 0 0.1333 | 0 0 0 | 0 0 1 | ∓6.283185 | 3.141593 | 28 |
| 6 | `wrist_2_joint` | revolute | wrist_1_link → wrist_2_link | 0 −0.0997 −2.0449e-11 | 1.570796327 0 0 | 0 0 1 | ∓6.283185 | 3.141593 | 28 |
| 7 | `wrist_3_joint` | revolute | wrist_2_link → wrist_3_link | 0 0.0996 −2.0428e-11 | 1.570796326589793 π π | 0 0 1 | ∓6.283185 | 3.141593 | 28 |
| 8 | `wrist_3_link-ft_frame` | fixed | wrist_3_link → ft_frame | 0 0 0 | π 0 0 | – | – | – | – |
| 9 | `base_link-base_fixed_joint` | fixed | base_link → base | 0 0 0 | 0 0 π | – | – | – | – |
| 10 | `wrist_3-flange` | fixed | wrist_3_link → flange | 0 0 0 | 0 −π/2 −π/2 | – | – | – | – |
| 11 | `flange-tool0` | fixed | flange → tool0 | 0 0 0 | π/2 0 π/2 | – | – | – | – |

- Every revolute joint has `<dynamics damping="0" friction="0"/>`.
- No acceleration limits anywhere (`has_acceleration_limits: false`).
- `wrist_3_joint` becomes `continuous` when `has_position_limits` is false. The UR3e filter output showed no `max_position` line for wrist_3, which suggests that case, but I did not read the flag itself.
- The rpy `(π/2, π, π)` on wrist_3 equals Rx(−π/2) [N].
- flange · tool0 is the identity rotation, so **tool0 coincides with wrist_3_link** (Z out of the flange) [N]. `flange` has X out of the flange.

### Links

Mesh filenames are `package://ur_description/meshes/ur5e/{visual/NAME.dae, collision/NAME.stl}`. Visual and collision share the same origin.

| Link | Mesh name | Mesh origin xyz | Mesh origin rpy | Mass (kg) | Inertial origin xyz | Inertial rpy | ixx, iyy, izz / ixy, ixz, iyz |
|---|---|---|---|---|---|---|---|
| world, base_link, base, flange, tool0, ft_frame | – | – | – | – | – | – | – |
| base_link_inertia | base | 0 0 0 | 0 0 π | 4.0 ("might be incorrect") | 0 0 0 | 0 | 0.00443333, 0.00443333, 0.0072 / 0, 0, 0 |
| shoulder_link | shoulder | 0 0 0 | 0 0 π | 3.761 | 0 −0.00193 −0.02561 | π/2 0 0 | 0.00700210, 0.00648091, 0.00657286 / 7.3e-7, −1.053e-5, 4.9994e-4 |
| upper_arm_link | upperarm | 0 0 0.138 | π/2 0 −π/2 | 8.058 | −0.2125 0 0.11336 | 0 | 0.01505885, 0.33388086, 0.33247207 / −5.4e-5, 5.63e-6, −1.81e-6 |
| forearm_link | forearm | 0 0 0.007 | π/2 0 −π/2 | 2.846 | −0.2422 0 0.0265 | 0 | 0.00399632, 0.07879254, 0.07848510 / −1.365e-5, 1.37272e-3, −6.6e-6 |
| wrist_1_link | wrist1 | 0 0 −0.127 | π/2 0 0 | 1.37 | 0 −0.01634 −0.0018 | π/2 0 0 | 0.00165491, 0.00135962, 0.00126279 / −2.82e-6, −4.38e-6, 1.0157e-4 |
| wrist_2_link | wrist2 | 0 0 −0.0997 | 0 0 0 | 1.3 | 0 0.01634 −0.0018 | −π/2 0 0 | 0.00135617, 0.00127827, 0.00096614 / −2.74e-6, 4.44e-6, −5.048e-5 |
| wrist_3_link | wrist3 | 0 −0.0005 −0.0989 | π/2 0 0 | 0.365 | 0 0 −0.001159 | 0 | 0.00018694, 0.00018908, 0.00025756 / 6e-8, −1.7e-7, −9.2e-7 |

The base inertia is a cylinder (r = 0.06, l = 0.05), computed as `0.0833333·m·(3r²+l²)` and `0.5·m·r²`.

### Calibration

- `default_kinematics.yaml` holds, per joint, the six numbers of the joint `<origin>` (x, y, z, roll, pitch, yaw) plus `hash: calib_12788084448423163542`.
- In the description as fetched, the hash is only loaded into the property `kinematics_hash` [V]. Its consumption by the driver's ros2_control tag, compared against the controller's calibration checksum, is [R].
- A robot-specific file is produced by the UR driver's calibration tool from the controller's delta-DH parameters. It replaces **all six joint origins** with non-nominal values: small roll/pitch/yaw everywhere, non-zero y/z offsets [R].
- The default file already contains artefacts of that conversion (`z: −2.04e-11`, roll `1.570796327`).
- Consequences: a calibrated UR is *not* exactly a Hawkins chain, and DH-versus-URDF self-tests need a tolerance of about 1e-9, not 1e-12.

### Other e-Series models [V: config/*]

| Model | d1 | a2 | a3 | d4 | d5 | d6 | Velocity limits (deg/s) | Efforts (N·m) |
|---|---|---|---|---|---|---|---|---|
| UR3e | 0.15185 | −0.24355 | −0.2132 | 0.13105 | 0.08535 | 0.0921 | 180 ×3, 360 ×3 | 54, 54, 28, 9, 9, 9 |
| UR10e | 0.1807 | −0.6127 | −0.57155 | 0.17415 | 0.11985 | 0.11655 | 120, 120, 180 ×4 | 330, 330, 150, 54, 54, 54 |
| UR16e | 0.1807 | −0.4784 | −0.36 | 0.17415 | 0.11985 | 0.11655 | 120, 120, 180 ×4 | 330, 330, 150, 54, 54, 54 |

The elbow limit is ±180° on all of them.

### Mesh inventory (bytes, HTTP `content-length`) [V]

| Part | ur3e .dae / .stl | ur5e .dae / .stl | ur10e .dae / .stl | ur16e .dae / .stl |
|---|---|---|---|---|
| base | 239,766 / 21,784 | 358,056 / 21,084 | 370,618 / 22,984 | uses ur10e |
| shoulder | 2,079,529 / 90,684 | 1,797,083 / 70,084 | 2,344,671 / 85,284 | uses ur10e |
| upperarm | 3,649,862 / 101,084 | 3,082,486 / 99,684 | 3,168,984 / 93,784 | 3,091,149 / 136,984 |
| forearm | 1,282,393 / 51,384 | 1,140,937 / 53,284 | 1,375,880 / 67,284 | 1,343,332 / 99,584 |
| wrist1 | 1,418,257 / 63,584 | 1,334,663 / 59,584 | 1,241,549 / 66,984 | uses ur10e |
| wrist2 | 1,467,167 / 72,784 | 1,554,839 / 67,584 | 1,428,002 / 89,184 | uses ur10e |
| wrist3 | 83,663 / 8,084 | 66,077 / 7,184 | 71,927 / 6,984 | uses ur10e |
| **Total** | 10.22 MB / 409 kB | 9.33 MB / 378 kB | 10.00 MB / 432 kB | 4.43 MB / 237 kB own |

- `config/ur16e/visual_parameters.yaml` points base, shoulder and all wrists at `meshes/ur10e/...` [V]; the corresponding `meshes/ur16e/` paths return 404.
- All STL sizes are ≡ 84 (mod 50), consistent with binary STL. The UR5e collision set would then be 420 + 1400 + 1992 + 1064 + 1190 + 1350 + 142 = **7,558 triangles**, which is cheap for parry trimeshes or convex hulls.
- Visuals are COLLADA, about 9–10 MB per robot. That is the argument for converting to a compact binary at plugin build time.

### Licence [V: URD/README.md lines 21–26]

> "The UR8LONG meshes, UR15 meshes, UR18 meshes, UR20 meshes, UR30 meshes, UR10g-1750 meshes, UR17g-1300 meshes, UR18g-950 meshes and common Smart Panel meshes constitutes "Graphical Documentation" the use of which is subject to and governed by our "Terms and Conditions for use of Graphical Documentation"."

> "…do not fully comply with OSI's definition of Open Source, but they do allow you to use, modify and share "Graphical Documentation" … subject to certain restrictions."

> "All other content is licensed under the BSD-3-Clause license"

So the ur3e, ur5e, ur10e and ur16e meshes are BSD-3-Clause. Note that the **Smart Panel mesh is restricted**; keep it disabled.

### Ready-made expanded URDFs (HEAD checks on raw.githubusercontent.com) [V]

| Location | Result |
|---|---|
| `Gepetto/example-robot-data/master/robots/ur_description/urdf/ur5_robot.urdf` | Exists, 12.6 kB, repo BSD-3-Clause. It is the **CB3 UR5, not the e-Series**; `ur5e_robot.urdf` is 404. |
| `google-deepmind/mujoco_menagerie/main/universal_robots_ur5e/ur5e.xml` | MJCF, 6.6 kB. LICENSE is BSD-3 text ("Copyright 2018 ROS Industrial Consortium"). Not URDF. |
| `ros-industrial/universal_robot/noetic-devel/ur_description/urdf/ur5e.xacro` | xacro only; no `.urdf`. |
| `Daniella1/urdf_files_dataset/.../ur_description/urdf/ur5e.urdf` | Xacro-generated, 14 kB. Licence not checked; ROS 1 lineage. |

Conclusion: generate our own expansion from the four YAML files. It is a pure function over a fixed topology (tables above) and gives calibration support for free.

---

## 3. From URDF to a planning-ready model

### URDF semantics (the spec to implement) [R: URDF specification / urdfdom]

- **Origin:** `<origin xyz rpy>` is the child frame expressed in the parent frame. Rotation is fixed-axis roll(X), pitch(Y), yaw(Z): **R = Rz(yaw)·Ry(pitch)·Rx(roll)**. This is the convention the DH agreement test in section 4 was run with [N].
- **Joint frame:** it equals the child link frame at q = 0 and sits at `parent_link_frame · origin`. Motion is applied *after* the origin: `T_parent_child(q) = T_origin · T_motion(q)`.
- **Axis:** expressed in the joint (child) frame; default `(1,0,0)`; must be normalised by the importer; a zero vector is an error.
- **Joint types:**
  - revolute: rotation about the axis, limits required.
  - continuous: no position limits; velocity and effort still apply.
  - prismatic: translation along the axis, limits in metres.
  - fixed: no motion.
  - floating: 6 DoF.
  - planar: motion in the plane normal to the axis.
  - spherical: URDF 1.2 / urdf-rs only.
- **Limits:** `lower`/`upper` in rad or m; `velocity` and `effort` are symmetric magnitudes. For URDF below 1.2, a missing lower/upper means 0. `safety_controller` soft limits lie inside the hard limits. URDF 1.2 adds acceleration, deceleration and jerk.
- **Mimic:** `q = multiplier · q_master + offset`, defaults 1 and 0. The joint is not an independent DoF. Chains of mimics are possible; cycles are an error.
- **Inertial:** the origin is the COM frame in the link frame; the tensor is expressed in that frame.
- **Geometry:** visual and collision are independent lists, each with its own origin. Cylinder and capsule are along local Z and centred. Mesh `scale` is per-axis (negative values mirror and flip winding).
- **Mesh units:** STL is unitless; assume metres. COLLADA carries `<unit>` and `<up_axis>`, which must be honoured.
- **Resource URIs:** `package://PKG/path`, `file://`, and relative paths (relative to the URDF file). Resolve through a `ResourceResolver` trait that maps PKG to a plugin asset root or user-configured directories. Never use ROS tooling.

### How others structure it [R]

- **MoveIt 2:** `RobotModel` (URDF tree plus SRDF) contains `JointModelGroup`s (chain base→tip or joint lists), end effectors, virtual joints, passive joints, group states (named poses) and `disable_collisions` pairs (the allowed-collision matrix). Its setup assistant samples random states to find always/never/adjacent/default colliding pairs. Acceleration limits live in a separate `joint_limits.yaml`.
- **Pinocchio:** immutable `Model` (joints with placements, parent array in topological order, flat `q` indexing via `idx_q`/`nq`; frames as a separate list attached to joints) and mutable `Data` scratch. Fixed joints are collapsed into frames.
- **Tesseract:** scene graph plus kinematic groups; SRDF-like file with ACM, TCPs and group definitions; separate forward/inverse kinematics plugins per group.
- **k 0.32** [V: source]: `Rc`-based node tree with interior mutability, joint positions stored in the nodes, nalgebra. `SerialChain::from_end` gives chain extraction. Not thread-friendly.

**Take Pinocchio's split**: an immutable, `Sync` `RobotModel` and a per-thread `FkScratch`. Add an SRDF-equivalent `PlanningProfile` data file to the robot package.

### Information beyond URDF, with automatic defaults

| Need | Default derivation |
|---|---|
| Planning chain | Tip is `tool0` if present, else the unique leaf whose path contains the most movable joints. Base is the root, or the first ancestor after which a movable joint appears. If several leaves qualify, there are several groups and the user chooses. |
| TCP | Tip frame plus a user tool offset (pose). |
| Active joints | Movable non-mimic joints on the path. Mimic joints are resolved inside FK. |
| Acceleration / jerk limits | From URDF 1.2 if present, else the package profile, else a heuristic (accel = 5·v_max per second, jerk = 10·a_max per second), flagged "assumed" in the UI. |
| Joint weights (distance metric) | Proportional to the maximum Cartesian displacement of downstream geometry per radian (reach of the sub-chain), normalised. |
| Continuous joints | Represented as unbounded with wrap-aware distance and interpolation. A revolute joint with range > 2π (UR: ±2π) is **not** continuous: plan without wrapping, with turn variants in IK. |
| Allowed-collision matrix | Adjacent links (connected through fixed or movable joints) are always allowed. Then sample N ≈ 10,000 random configurations: pairs colliding in more than 95 % of samples ("always") or in none ("never") are disabled, the rest are enabled. Cache keyed by model hash. |
| Collision padding | Per-link, default 0, with a global UI default (for example 2–5 mm). |
| Named poses | From the package (home, zero, upright). |

### Validation checks

Errors:
- Not exactly one root; cycles; a link with two parents; duplicate names; dangling parent or child.
- Non-finite numbers; zero axis on a movable joint.
- `lower > upper`; a revolute or prismatic joint without `<limit>`.
- Mimic targeting an unknown joint, a fixed joint, or forming a cycle.
- Unresolvable mesh; non-rigid rotation after scaling.

Warnings:
- Non-unit axis (normalise it).
- `velocity == 0` or missing (the planner cannot time the motion).
- Limits wider than ±2π on a revolute joint.
- Mass ≤ 0 or an inertia tensor that is not positive-definite or violates the triangle inequality.
- Mesh units suspicious (bounding box > 50 m or < 1 mm, i.e. millimetre STL).
- `tf_prefix` leftovers, unexpanded `${...}` or `$(find` (an un-xacro'ed file).
- Collision missing where visual exists.
- Massive root link.
- `continuous` with limits.
- Negative scale.

Other typical pitfalls: a comma decimal separator, rpy given in degrees (|value| > 2π), and Windows path separators.

---

## 4. Forward kinematics and Jacobians (f64, glam)

### FK

At import, precompute per joint `X_i = T_origin` (pose), the unit axis `a_i` in the joint frame, the kind, and the parent index. Fixed joints are pre-multiplied into their child's `X` (collapsed), and named frames are kept as (movable link, constant offset).

- Tree FK in topological order: `W_child = W_parent · X_i · M_i(q)`, where `M` is `(DQuat::from_axis_angle(a, q), 0)` or `(identity, a·q)`.
- Quaternion-plus-translation composition costs about 60 flops per joint. For 6 joints that is well under a microsecond [R, estimate].

**Transform chain versus product of exponentials:** both are equivalent. The transform chain follows URDF directly and needs no home-configuration bookkeeping. PoE (screw axes in the base frame at zero pose) is useful only for analytic-family detection (section 5c), which needs the zero-pose axis lines anyway. Implement the transform chain and derive the zero-pose axes from it.

**Hot path:** `fk_into(&self, q: &[f64], out: &mut [Pose])` with a caller-owned scratch buffer and no allocation. For batches, `fk_batch(qs: &[f64] /* n × dof */, out: &mut [Pose])`. Collision checking needs all link poses, so compute all of them once per configuration.

### Geometric Jacobian (6×N, world frame, reference point p_tip)

- Revolute column: `Jv_i = z_i × (p_tip − p_i)`, `Jω_i = z_i`.
- Prismatic column: `Jv_i = z_i`, `Jω_i = 0`.
- `z_i = R_world_joint_i · a_i`; `p_i` is the joint origin.
- Mimic: add `multiplier ·` column to the master's column.
- Store as `[[f64; 6]; N]` (column-major, `DVec3` pairs). No matrix crate is needed.

### Singularity and manipulability

Form `A = J·Jᵀ` (6×6, symmetric positive semi-definite) with characteristic-length scaling so units are commensurate: divide the linear rows by L, where L is the robot reach.

- Yoshikawa manipulability `w = sqrt(det A)`.
- σ_min = sqrt(λ_min(A)), from an in-crate cyclic Jacobi eigen-solver on the 6×6 matrix (about 50 lines; 5–8 sweeps).
- Squaring halves the precision (σ floor about 1e-8 relative). That is irrelevant for thresholds around 1e-2 to 1e-3.
- If exact small singular values are ever needed, a one-sided Jacobi SVD on Jᵀ is about 80 lines.
- **Do not add nalgebra 0.35 to production crates** for this; use it as a dev-dependency to cross-check. The limits of glamx's 3×3 SVD were not checked, but a 3×3 routine does not cover a 6×N matrix.
- The DLS solve uses the same `A`: `(A + λ²I) y = e` via in-crate 6×6 Cholesky, then `dq = Jᵀ y`.

### DH and frame relationships [N: 2000 random q, max error 4e-16]

Standard (distal) DH: `A_i = Rz(θ_i)·Tz(d_i)·Tx(a_i)·Rx(α_i)`, using the UR5e table from the brief.

- **`T(base → tool0)_URDF(q) = A_1(q1)···A_6(q6)`** with zero joint offsets and identical joint signs.
- `base_link = Rz(π) · base`, so `T(base_link → tool0) = Rz(π) · T_DH`.
- `base_link_inertia` coincides with `base`.
- `tool0` ≡ `wrist_3_link` ≡ DH frame 6. `flange` = frame 6 · R with R = [[0,1,0],[0,0,1],[1,0,0]] (X out).
- At q = 0, `base → tool0` has translation (−0.8172, −0.2329, 0.0628) and rotation rows [1,0,0], [0,0,−1], [0,1,0] [N].

Self-check tests:
1. URDF FK equals DH FK for 1000 seeded random q, tolerance 1e-9 (YAML rounding).
2. The zero-pose translation above.
3. The Jacobian matches central finite differences (1e-6).
4. `flange · tool0` rotation is the identity.

---

## 5. Inverse kinematics

### (a) UR closed form (Hawkins 2013) [V: URK/src/ur_kin.cpp, include/ur_kinematics/ur_kin.h]

**Frame convention.** `forward(q)` returns **Rz(π) · T_DH(q) · M** with M = [[0,−1,0],[0,0,−1],[1,0,0]] [N: 8e-16]. That is `base_link → ee_link`, where the legacy ee_link has X out of the flange and equals today's `flange` rotated 180° about X. It is *not* `base_link → flange`; the mismatch was confirmed numerically (element error 2.0).

Adapter: for a target `T_bt = T(base → tool0)`, call the port with `T = Rz(π) · T_bt · M`. Better, strip both offsets in the port and work directly on `T_DH` (the first lines of `inverse()` merely undo them by sign and column permutation).

**Algorithm** (T_ij are entries of T_DH after un-permuting; d1, a2, a3, d4, d5, d6 as in the table):

1. **q1, two solutions (shoulder left/right).**
   - `A = d6·T12 − T13`, `B = d6·T02 − T03`. These are minus the y and x of the frame-5 origin P5 = p6 − d6·z6. `R = A² + B²`.
   - If `d4² > R` there is no solution: P5 lies inside the cylinder of radius d4 around the base axis.
   - Otherwise `q1 = atan2(−B, A) ± acos(d4/√R)`, wrapped to [0, 2π).
   - Special branches exist for |A| < ε or |B| < ε.
2. **q5, two per q1 (wrist sign).** `numer = T03·s1 − T13·c1 − d4`; `q5 = ±acos(numer/d6)` (the code returns `acos` and `2π − acos`). The ratio is snapped to ±1 when within ε.
3. **q6.** If |sin q5| < ε (wrist singularity: axes 4 and 6 parallel, only q4 + q6 is determined) then `q6 = q6_des`, a caller-supplied free value. Otherwise `q6 = atan2(sgn(s5)·−(T01·s1 − T11·c1), sgn(s5)·(T00·s1 − T10·c1))`.
4. **q2, q3, q4, planar 3R with two solutions (elbow).**
   - Compute `x04x`, `x04y` (direction of frame 4 in the arm plane) and `p13x`, `p13y` (the frame-1 to frame-3 vector in the plane) exactly as in the source.
   - `c3 = (p13x² + p13y² − a2² − a3²) / (2·a2·a3)`. If |c3| > 1 there is no solution (out of reach). If |c3| = 1 the arm is stretched or folded (elbow singularity; the two branches merge).
   - `q3 = ±acos(c3)`.
   - `q2 = atan2((A'·p13y ∓ B'·p13x)/den, (A'·p13x ± B'·p13y)/den)` with `A' = a2 + a3·c3`, `B' = a3·s3`, `den = a2² + a3² + 2·a2·a3·c3`.
   - `q4 = atan2(c23·x04y − s23·x04x, x04x·c23 + x04y·s23)`.

This gives up to 2×2×2 = 8 solutions, all in [0, 2π).

**Numerical check** [N]: over 2000 random poses FK(IK) error is ≤ 1.4e-10 and the generating q is always among the solutions. Solution counts were 8 (77 %), 6, 4 and 2: branches legitimately disappear.

**Defects to fix in a port:**
- `ZERO_THRESH = 1e-8` is absolute and applied to mixed quantities (lengths and sines).
- The q5 `acos(numer/d6)` has **no range check**, so it yields NaN when |numer| > |d6|. The same holds for `asin(div)` in the |A| < ε branch. The port must test `|x| ≤ 1 + ε`, clamp, and otherwise drop the branch, and assert all outputs are finite.
- Use `atan2`-based forms where possible.
- At the wrist singularity, take `q6` from the seed.

**Branch naming** [N on one sample]. Loop indices (i, j, k) map to:
- i: shoulder (the two q1 values);
- j: wrist, sign(sin q5) (j = 0 gives q5 ∈ [0, π]);
- k: elbow, sign(sin q3) (k = 0 gives q3 ∈ [0, π]).

Define `UrConfig { shoulder: Left|Right, elbow: Up|Down, wrist: NoFlip|Flip }`. Classify *any* q, not just solver output, by geometric sign tests:
- shoulder = sign of P5's coordinate along the shoulder y-axis (equivalently which `acos` sign);
- elbow = sign(sin q3);
- wrist = sign(sin q5).

Which sign is called "Up" depends on the shoulder side. Decide the label by a geometric elbow-height test and pin it in a test; I did not verify it against PolyScope naming.

**±2π range expansion.**
- Canonicalise each solution to (−π, π].
- Each joint with range ±2π admits `q` and `q − 2π·sgn(q)`: 2 variants, or 1 for the elbow (±π), giving up to 2⁵ = 32 variants per branch.
- `solve_near(seed)`: per joint independently pick `q + 2πk` nearest the seed within limits. This is separable and O(dof).
- `solve_all`: return the 8 canonical branches and a lazy iterator over turn variants. The planner picks goal variants by weighted joint distance from the start.

**Calibrated kinematics.**
- Solve with nominal DH (from the package's nominal parameters, or by fitting, section 5c).
- Refine each branch by LM/Newton on the *actual* URDF chain. Calibration deltas are of millimetre and milliradian order [R], so 2–4 iterations to 1e-10 are expected (untested).
- Verify that the branch label is unchanged.
- If refinement fails near a singularity, fall back to the numeric solver seeded from that branch.

**Licence.** BSD-3-Clause, "Copyright (c) 2013, Georgia Institute of Technology", author Kelsey Hawkins [V: ur_kin.h]. A port is a derivative work, so:
- keep the notice, conditions and disclaimer in the source file;
- reproduce them in the binary distribution's third-party notices;
- do not use the Georgia Tech name for endorsement.

### (b) Generic numeric IK

**What existing solvers do** [R]:
- KDL `ChainIkSolverPos_NR(_JL)`: pseudo-inverse Newton with clamping; stalls on limits and local minima.
- TRAC-IK: runs KDL with random restarts when stuck *and* an SQP (NLopt SLSQP) formulation concurrently, taking the first or best result. Markedly higher solve rates than plain KDL.
- BioIK: memetic (evolutionary plus gradient) search with multiple goals; robust but slower and less precise.
- RelaxedIK / RangedIK: weighted non-linear optimisation trading accuracy for smoothness and feasibility; not exact, so unsuitable as the primary solver for CAD waypoints.

**Recommended algorithm:** Levenberg–Marquardt with multi-start, as follows.

- **Error:** `e = [ (p* − p)/L ; log(R*·Rᵀ) ]` in the world frame, with the rotation vector from the quaternion log (shortest arc: flip the quaternion sign if w < 0).
- **Tolerances:** separate `pos_tol` (default 1e-6 m; 1e-9 for refinement) and `rot_tol` (1e-6 rad). Never a single mixed norm.
- **Step:** `dq = W⁻¹Jᵀ(J W⁻¹ Jᵀ + λ²I)⁻¹ e` with W the joint weights. Sugihara-style damping `λ² = ½‖e‖²·w_e + λ_min²` (λ_min² ≈ 1e-6) removes hand tuning [R].
- **Step control:** accept or reject by error decrease (true LM gain ratio); cap ‖dq‖∞ (for example 0.5 rad).
- **Joint limits:** projected / active-set. Clamp, then remove saturated joints' columns for the next iteration, re-enabling them when the unconstrained step points inward. This is better than a pure clamp (which stalls) and than barrier terms (which bias the solution away from valid near-limit goals).
- **Continuous joints:** never clamp.
- **Restarts:** first the seed, then deterministic quasi-random seeds within limits (seeded PCG/SplitMix or a Halton sequence), for a fixed budget (`max_restarts`, `max_iters` about 50 each).
- **Stagnation:** restart on ‖dq‖ < 1e-10 without convergence.
- **Several solutions:** collect converged results and deduplicate by wrapped joint distance > 1e-3. No completeness guarantee; report it as `Completeness::Sampled`.
- **7-DoF:** the minimum-norm step handles redundancy. Add a null-space term `(I − J⁺J)·∇h` with h = distance to the seed or to the mid-range, or expose `solve_with_free(joint, value)` to parameterise the arm angle.
- **Determinism:** a fixed seed in options, no wall-clock termination by default (an iteration budget instead); an optional deadline only for interactive dragging.
- **Targets (unbenchmarked):** about 5–20 µs per successful seeded 6R solve, under 1 ms for 100 restarts.
- **Failure reporting:** `IkFailure { best_q, pos_err, rot_err, reason: OutOfReach | JointLimits | Singular | Budget, restarts, iters }`.

**Build versus buy** [V: sources read]:

| Crate | Finding | Verdict |
|---|---|---|
| k 0.32.0 `JacobianIkSolver` | Undamped: LU solve of the square Jacobian (errors at singularities), SVD pseudo-inverse with a fixed 1e-4 epsilon for redundant chains, fixed step multiplier, clamp-only limits, mutates joint state held in `Rc` nodes, nalgebra 0.30, no restarts inside the solver. | Reject |
| caliper-ik 0.2.1 | LM with manipulability-gated damping, step clamp, limits, multi-restart with seeded SplitMix64; analytic solver for spherical-wrist 6R only (not UR). Apache-2.0, MSRV 1.89, nalgebra 0.35, tied to `caliper-model`/`-spatial`/`-kinematics`, single release (2026-07-31), 342 downloads. | Good reference design; do not depend |
| ik-geo 0.1.2 | Subproblem-based analytic IK for many 6R families including UR ("three parallel and two intersecting"). nalgebra 0.32 and the **nlopt C library**. | Reject as a dependency; the IK-Geo paper (arXiv 2211.05737) is the roadmap for future analytic families |

**Verdict: build in-house.** It is roughly 400 lines on top of our FK and Jacobian, uses our types, is deterministic, and has no C or nalgebra dependency.

### (c) Solver selection

1. **Package metadata wins.** A robot package may declare `ik = { family = "ur6r", nominal = { d1, a2, a3, d4, d5, d6 } }` for a named chain.
2. **Auto-detection** from the collapsed chain (6 revolute joints). Compute zero-pose axis lines (point pᵢ, direction zᵢ) in the base frame, with tolerances ε_ang ≈ 1e-6 and ε_len ≈ 1e-7·reach. The UR family requires:
   - z2 ∥ z3 ∥ z4;
   - z1 ⟂ z2 and their lines intersect;
   - z5 ⟂ z4 and they intersect;
   - z6 ⟂ z5 and they intersect.
3. **Parameter extraction:**
   - d1 = offset of axis 2 along z1 from the base origin.
   - a2, a3 = signed distances between axes 2–3 and 3–4.
   - d4 = offset along z2 between axis 1 and axis 5.
   - d5 = distance between axes 4 and 6 along z5.
   - d6 = distance from axis 5 to the tip origin along z6.
   - Then fit per-joint sign sᵢ ∈ {±1} and offset oᵢ with `q_dh = sᵢ·q_urdf + oᵢ`, and constant `T_pre` (base → DH0) and `T_post` (DH6 → tip).
4. **Acceptance test.** FK_DH equals FK_URDF at 64 seeded random q.
   - Error below 1e-9: `Exact`.
   - Error below about 5 mm / 0.5°: `NearNominal`, meaning analytic plus refinement (the calibrated-UR case; here fit the nominal parameters by least squares).
   - Otherwise: numeric only.
5. **Numeric LM** is always constructed. It is the fallback, the refiner, and the test oracle.

---

## 6. Rust design proposal

### Crates

| Crate | Contents | Dependencies |
|---|---|---|
| `gc-robot-model` | URDF reader, `RobotModel`, validation, chains, FK, Jacobian, small dense linear algebra (6×6 Cholesky, Jacobi eigen) | `gc-math` (glamx), `roxmltree`, `thiserror` |
| `gc-ik` | `IkSolver` trait, `NumericLm`, `Ur6rAnalytic`, family detection, `Refined<S>` | `gc-robot-model` |
| `gc-robot-ur` (plugin) | YAML-derived constants, URDF generator, calibration import, mesh assets, IK-family metadata | plugin API |

### Decisions

- **Joint vectors.** Public APIs take `&[f64]` / `&mut [f64]`. The owned type is `JointVec`, an inline-capacity-8 small vector (spills to the heap for dual-arm or rail systems). Paths are stored flat as `Vec<f64>` (n × dof). Const generics are used only inside analytic solvers (`[f64; 6]`). Reason: the model is runtime data from a file, so a const-generic model type would infect every plugin boundary.
- **Typed ids.** `LinkId(u32)`, `JointId(u32)`, `DofIndex(u16)`. Angle versus length is carried by `JointKind`, not by the vector.
- **Limits.** `PositionLimit::{Bounded{lo, hi}, Unbounded}`. Velocity, acceleration and jerk are `Option<f64>` with provenance `LimitSource::{Urdf, Package, Assumed}`.

### Code sketch

Against verified facts: the glamx `DPose3 { rotation: DQuat, translation: DVec3 }` shape and the URDF rpy semantics. The glam method names used (`from_rotation_x/y/z`, `from_axis_angle`, `cross`, `Mul`) are [R].

```rust
// ---------- gc-robot-model ----------
use gc_math::{DQuat, DVec3};                 // re-exported glamx/glam f64 types

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose { pub rotation: DQuat, pub translation: DVec3 }   // mirrors glamx DPose3
impl Pose {
    pub const IDENTITY: Self = Self { rotation: DQuat::IDENTITY, translation: DVec3::ZERO };
    /// URDF <origin>: R = Rz(yaw) * Ry(pitch) * Rx(roll)
    pub fn from_urdf(xyz: [f64; 3], rpy: [f64; 3]) -> Self {
        let r = DQuat::from_rotation_z(rpy[2]) * DQuat::from_rotation_y(rpy[1])
              * DQuat::from_rotation_x(rpy[0]);
        Self { rotation: r.normalize(), translation: DVec3::from_array(xyz) }
    }
    #[inline] pub fn mul(&self, o: &Pose) -> Pose {
        Pose { rotation: self.rotation * o.rotation,
               translation: self.translation + self.rotation * o.translation }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct LinkId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct JointId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PositionLimit { Bounded { lo: f64, hi: f64 }, Unbounded }

#[derive(Clone, Debug, PartialEq)]
pub enum JointKind {
    Fixed,
    Revolute  { axis: DVec3, limit: PositionLimit },   // Unbounded == URDF "continuous"
    Prismatic { axis: DVec3, limit: PositionLimit },
    Mimic     { master: JointId, multiplier: f64, offset: f64, revolute: bool, axis: DVec3 },
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String, pub parent: LinkId, pub child: LinkId,
    pub origin: Pose, pub kind: JointKind,
    pub max_velocity: Option<f64>, pub max_effort: Option<f64>,
    pub max_acceleration: Option<f64>, pub max_jerk: Option<f64>,
}

pub struct RobotModel { /* links, joints (topologically sorted), name maps, root */ }

/// A serial chain base -> tip with fixed joints collapsed. Immutable, Send + Sync.
pub struct Chain {
    pre: Pose,                    // base frame -> first movable joint frame
    segs: Vec<Segment>,           // one per DoF
    lower: Vec<f64>, upper: Vec<f64>,   // +-INFINITY when unbounded
}
struct Segment { revolute: bool, axis: DVec3, post: Pose } // motion(q) then constant `post`

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("{file}:{line}:{col}: {msg}")] Parse { file: String, line: u32, col: u32, msg: String },
    #[error("link `{0}` has more than one parent")] MultipleParents(String),
    #[error("kinematic loop through joint `{0}`")] Loop(String),
    #[error("no chain from `{base}` to `{tip}`")] NoChain { base: String, tip: String },
    #[error("joint `{0}`: zero-length axis")] ZeroAxis(String),
    #[error("expected {expected} joint values, got {got}")] Dof { expected: usize, got: usize },
}

impl Chain {
    pub fn dof(&self) -> usize { self.segs.len() }

    /// Allocation-free FK. `frames[i]` = pose of joint frame i *before* its motion
    /// (origin p_i, axis R_i * a_i); returns the tip pose. All in the chain base frame.
    pub fn fk_into(&self, q: &[f64], frames: &mut [Pose]) -> Pose {
        debug_assert!(q.len() == self.dof() && frames.len() == self.dof());
        let mut t = self.pre;
        for (i, s) in self.segs.iter().enumerate() {
            frames[i] = t;
            let m = if s.revolute {
                Pose { rotation: DQuat::from_axis_angle(s.axis, q[i]), translation: DVec3::ZERO }
            } else {
                Pose { rotation: DQuat::IDENTITY, translation: s.axis * q[i] }
            };
            t = t.mul(&m).mul(&s.post);
        }
        t
    }

    /// Geometric Jacobian columns [v; w] in the base frame, reference point = tip origin.
    pub fn jacobian_into(&self, frames: &[Pose], tip: &Pose, jac: &mut [[f64; 6]]) {
        for (i, s) in self.segs.iter().enumerate() {
            let z = frames[i].rotation * s.axis;
            let (v, w) = if s.revolute {
                (z.cross(tip.translation - frames[i].translation), z)
            } else { (z, DVec3::ZERO) };
            jac[i] = [v.x, v.y, v.z, w.x, w.y, w.z];
        }
    }
}

// ---------- gc-ik ----------
pub type JointVec = smallvec::SmallVec<[f64; 8]>;   // or an in-house equivalent

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Shoulder { Left, Right }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Elbow { Up, Down }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Wrist { NoFlip, Flip }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Branch { Ur { shoulder: Shoulder, elbow: Elbow, wrist: Wrist }, Unclassified }

#[derive(Clone, Debug)]
pub struct IkSolution { pub q: JointVec, pub branch: Branch, pub pos_err: f64, pub rot_err: f64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness { AllBranches, Sampled }

#[derive(Clone, Debug)]
pub struct IkOptions {
    pub pos_tol: f64, pub rot_tol: f64,          // metres / radians, checked separately
    pub max_iters: u32, pub max_restarts: u32, pub rng_seed: u64,
    pub expand_turns: bool,                      // emit +-2*pi variants within limits
}

#[derive(Debug, thiserror::Error)]
pub enum IkError {
    #[error("target unreachable (best: {pos_err:.3e} m, {rot_err:.3e} rad)")]
    Unreachable { best_q: JointVec, pos_err: f64, rot_err: f64 },
    #[error("all solutions violate joint limits")] JointLimits,
    #[error("iteration budget exhausted after {restarts} restarts")] Budget { restarts: u32 },
    #[error("solver does not support this chain: {0}")] Unsupported(&'static str),
}

pub trait IkSolver: Send + Sync {
    fn completeness(&self) -> Completeness;
    /// All distinct solutions inside joint limits (canonical turn unless `expand_turns`).
    fn solve_all(&self, target: &Pose, opts: &IkOptions, out: &mut Vec<IkSolution>)
        -> Result<(), IkError>;
    /// The solution closest to `seed` (weighted, wrap-aware joint distance).
    fn solve_near(&self, target: &Pose, seed: &[f64], opts: &IkOptions)
        -> Result<IkSolution, IkError>;
}

pub struct UrParams { pub d1: f64, pub a2: f64, pub a3: f64, pub d4: f64, pub d5: f64, pub d6: f64 }
/// Port of ur_kin.cpp `inverse` (BSD-3, (c) 2013 Georgia Tech), operating on T_DH directly.
pub struct Ur6rAnalytic {
    pub params: UrParams,
    pre_inv: Pose, post_inv: Pose,     // chain base -> DH0 and DH6 -> tip, inverted
    sign: [f64; 6], offset: [f64; 6],  // q_dh = sign * q_urdf + offset
    lower: [f64; 6], upper: [f64; 6],
}
/// Analytic seeds + LM polish on the true (possibly calibrated) chain.
pub struct Refined<S: IkSolver> { pub seeds: S, pub polish: NumericLm }
pub struct NumericLm { /* Arc<Chain>, joint weights, characteristic length */ }

pub enum Family { Ur6r { params: UrParams, exact: bool }, Generic }
pub fn detect_family(chain: &Chain) -> Family { /* section 5(c) */ unimplemented!() }
```

### Tests

- **Property tests** (seeded generator; `proptest` version unverified, or in-house):
  - for every solver, q in limits ⇒ some element of `solve_all(FK(q))` matches q (wrap-aware 1e-6);
  - every returned solution satisfies FK within tolerance and limits;
  - no NaN ever, for random *unreachable* targets too.
- **Analytic versus numeric cross-check:** every `Ur6rAnalytic` branch is a fixed point of `NumericLm`.
- **Continuity:** along a 0.5 m Cartesian line sampled at 1 mm, `solve_near(prev)` yields ‖Δq‖∞ below a bound, except where σ_min falls below a threshold. Assert that a singularity is reported there rather than a jump.
- **Branch stability:** the classification of `solve_near` output equals that of the seed away from singularities.
- **Golden values:** UR5e zero-pose FK; DH versus URDF; the four e-Series parameter sets.
- **Calibration:** perturb joint origins by about 1 mm / 1 mrad and check that `Refined` converges in ≤ 5 iterations with the same branch.
- **Importer:** a differential test against `urdf-rs` (dev-dependency) on a URDF corpus, and one test per validation rule using malformed files.
- **Detection:** UR chains are detected; a spherical-wrist chain is not.

---

## Recommendation

1. **URDF is the single source of kinematic truth.** Use our own roxmltree 0.21.2 reader that produces positioned diagnostics and a validated, immutable `RobotModel`. Keep urdf-rs 0.10.0 as a dev-dependency oracle only. Support URDF 1.0–1.2 including the 1.2 acceleration and jerk limits; reject xacro input with a clear message.
2. **No xacro engine in the product.** The UR plugin implements `ur_macro.xacro` as about 150 lines of Rust over the four YAML-derived parameter sets (tables in section 2). It emits both a `RobotModel` and an exportable `.urdf`, and accepts a user calibration YAML to produce a calibrated model. Ship ur3e, ur5e, ur10e and ur16e (BSD-3 meshes; ur16e reuses five ur10e meshes); exclude the Smart Panel. Convert the 9–10 MB DAE visuals to a compact binary at build time and use the STL collision meshes for parry.
3. **A package-side `PlanningProfile`** (our SRDF equivalent) with auto-derived defaults: chain, TCP, acceleration and jerk limits with provenance, ACM by sampling, padding, named poses.
4. **FK and Jacobian in-house on glam f64**, with a transform chain and caller-owned scratch. Singularity metrics come from a 6×6 `JJᵀ` Jacobi eigen-solve. No nalgebra or faer in production.
5. **IK is a trait with two in-house implementations**: a hardened `Ur6rAnalytic` port (BSD-3 notice kept) and `NumericLm` (Sugihara damping, active-set limits, seeded restarts). Compose them as `Refined<Ur6rAnalytic>` so calibrated robots work. Select the family by package metadata, then by geometric detection, then numeric.
6. **Milestone order:** importer and validation → FK with the DH golden test → Jacobian and LM → UR analytic and branches → detection and refinement → planner integration.

## Risks and unknowns

- xacro-pure was not test-run on UR's files; the roxmltree position API, `smallvec` and `proptest` versions, and glam method names were not re-verified today.
- The Up/Down and Flip labels versus PolyScope naming are unverified. Decide by a geometric test and pin it in tests.
- Real calibration magnitudes and the exact calibrated-YAML shape are recalled, not fetched. Refinement convergence near singularities needs test data from a real robot.
- Auto-ACM by sampling can wrongly disable pairs that collide only rarely. Keep the result user-editable and conservative ("never" requires zero hits, and only if the links are non-adjacent).
- The acceleration and jerk heuristics are placeholders. UR publishes no joint acceleration limits in the description (`has_acceleration_limits: false`).
- Users will bring xacro-only robots (most ROS packages). Without xacro support they must pre-expand; consider an optional, feature-gated xacro-pure import later.
- COLLADA unit and up-axis handling and visual mesh fidelity (mesh-loader's last release was 2024-10) belong to the mesh topic, but they affect what "URDF import" appears to deliver.
- The UR Graphical Documentation terms for newer models (UR15, UR20, UR30 and others) could not be read because the host is unreachable.

## Surprises

- **The UR5e elbow limit is ±180°, not ±360°** (`joint_limits.yaml`, all four e-Series models). This contradicts the "all joints ±360°" assumption in the brief. Likewise, UR3e wrist speeds are 360°/s and UR10e/UR16e shoulder speeds are 120°/s; "180°/s on all six" holds only for the UR5e.
- **ur_kin.cpp's frame is neither `base → tool0` nor `base_link → flange`.** It is `Rz(π)·T_DH·M`, the legacy `ee_link`, which is the flange rotated 180° about X. A naive port that feeds it flange poses is silently wrong.
- **ur_kin.cpp can return NaN solutions**, because the `acos` for q5 is unguarded. It is not safe to port verbatim.
- **`tool0` is exactly `wrist_3_link`**: the flange and tool0 rotations cancel. The URDF `base → tool0` chain equals the standard-DH chain with zero offsets, to 4e-16.
- **The "default" kinematics file already contains calibration-conversion artefacts** (z = −2e-11, π/2 rounded to nine digits), so exact-equality tests against DH fail at about 1e-10.
- **urdf-rs pulls three XML libraries** and its helper functions spawn `rosrun`, `xacro` and `rospack`, with an `unwrap()` that panics without ROS.
- **A faithful pure-Rust xacro exists (xacro-pure) but embeds a Python VM** (`rustpython-vm`, exact-pinned).
- **Gepetto's example-robot-data has no UR5e URDF**, only the CB3 UR5. No canonical, permissively licensed, pre-expanded UR5e URDF was found in the repositories checked, so generating our own is the cleanest path.
- **The UR16e has only two meshes of its own**; the rest are UR10e files. Also, the "common Smart Panel" mesh sits under the restrictive Graphical Documentation terms even for otherwise BSD models.
