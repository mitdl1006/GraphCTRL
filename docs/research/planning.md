# Collision checking and geometric motion planning for URDF-defined arms in pure Rust

Legend: **[V]** verified today from the crate tarball or crates.io API; **[R]** recalled from memory, not re-verified. All performance figures are estimates until benchmarked.

## 0. What "high quality" means: acceptance criteria

| # | Criterion | Target for milestone 1 (UR5e, 100k-triangle scene, 4 cores) |
|---|---|---|
| Q1 | Validity | Every returned path passes an independent re-validation (bounds, self-collision, environment) at the edge-check resolution. 0 violations over the benchmark suite; enforced by property test. |
| Q2 | Stated guarantee | "Collision-free with padding `p` at joint resolution `δ`"; optional certified mode (section 2.4) upgrades this to "provably free for the padded convex model". |
| Q3 | Success rate | At least 99% over 100 seeds on free-space transfer scenes; at least 90% on the narrow-passage benchmark within 5 s. |
| Q4 | Path length | After post-processing, weighted joint length at most 1.15× the best cost from RRT* run for 30 s on the same problem (median over seeds). |
| Q5 | Consistency | Coefficient of variation of path length over 100 seeds at most 10% after shortcutting. No extra joint turns: each joint's travel at most its straight-line travel plus 2× detour bound. |
| Q6 | Smoothness | No interior corner above a configurable angle after smoothing; the path can be time-parameterised (rsruckig or TOPP) without stop-and-go at interior vertices. |
| Q7 | Clearance | Minimum clearance reported; at least the padding by construction; optional clearance-seeking refinement. |
| Q8 | Latency | Median planning plus simplification under 300 ms for typical cell transfers; hard budget set by the user. |
| Q9 | Determinism | Same request, seed, iteration budget and build gives a bit-identical path on one platform. |
| Q10 | Explainability | Failures name the cause: link pair or obstacle id, joint and limit, IK branch, or budget exhausted with tree statistics. |

## 1. parry3d-f64 0.31.1: exact API [V]

Source: `https://static.crates.io/crates/parry3d-f64/parry3d-f64-0.31.1.crate`.

**Manifest**
- Edition 2021, Apache-2.0, and no `rust-version` field, so no declared MSRV.
- Default features: `required-features` (`dim3`, `f64`), `std`, `spade`.
- Optional features: `parallel` (rayon), `simd8`, `serde-serialize`, `rkyv`, `bytemuck-serialize`, `enhanced-determinism` (`simba/libm_force`, `glamx/libm`, `glamx/scalar-math`, indexmap), `wavefront`.
- There are no `simd-stable` or `simd-nightly` features any more.
- Dependencies: glamx 0.3, simba 0.10.1 (`wide`), rstar 0.13, spade 2.15, hashbrown 0.17, ena, slab, smallvec, arrayvec, bitflags, downcast-rs 2, thiserror 2, ordered-float 5, foldhash, num-traits/num-derive, log, approx. No nalgebra.
- Compile weight: `Cargo.lock` lists 149 packages including dev and optional ones; I did not measure build time.

**Maths types (`parry3d_f64::math`)**
- `Real = f64`, `Vector = glamx::DVec3`, `Pose = Pose3 = glamx::DPose3`, `Rotation = Rot3 = glamx::DRot3`, `Matrix = DMat3`.
- Our maths crate's `DPose3` is therefore parry's pose type, so no conversion is needed.
- Verified operators: `pose * vector` and `pose.rotation * vector`. Pose composition and inverse method names were not checked.

**Shapes (`parry3d_f64::shape`)**
- `Ball::new(radius)`, `Cuboid::new(half_extents: Vector)`, `Cylinder::new(half_height, radius)` (axis is local Y, as in previous versions [R]).
- `Capsule::new(a, b, radius)` plus `new_x`, `new_y`, `new_z(half_height, radius)`.
- `ConvexPolyhedron::from_convex_hull(&[Vector]) -> Option<_>` and `from_convex_mesh(Vec<Vector>, &[[u32; 3]]) -> Option<_>`.
- `TriMesh::new(Vec<Vector>, Vec<[u32; 3]>) -> Result<TriMesh, TriMeshBuilderError>` uses empty flags; `TriMesh::with_flags(.., TriMeshFlags)` takes flags.
- `TriMeshFlags`: `HALF_EDGE_TOPOLOGY`, `CONNECTED_COMPONENTS`, `DELETE_BAD_TOPOLOGY_TRIANGLES`, `ORIENTED`, `MERGE_DUPLICATE_VERTICES`, `DELETE_DEGENERATE_TRIANGLES`, `DELETE_DUPLICATE_TRIANGLES`, `FIX_INTERNAL_EDGES`, `DEFORMABLE`, `FIX_INTERNAL_EDGES_TWO_SIDED`.
- `TriMesh` accessors: `bvh()`, `vertices()`, `indices()`, `triangle(i)`, `local_aabb()`, `aabb(&Pose)`, `scaled(Vector)`, `transform_vertices(&Pose)`, `append`.
- `Compound::new(Vec<(Pose, SharedShape)>)`, with `shapes()`, `aabbs()`, `bvh()`, and `Compound::decompose_trimesh(&TriMesh) -> Option<Compound>`.
- `SharedShape(pub Arc<dyn Shape>)` with constructors `ball`, `cuboid(hx, hy, hz)`, `cylinder`, `capsule*`, `round_cylinder`, `round_cuboid`, `trimesh`, `trimesh_with_flags`, `convex_hull(&[Vector]) -> Option`, `round_convex_hull(points, border_radius) -> Option`, `convex_mesh`, `convex_decomposition(vertices, indices)`, `convex_decomposition_with_params`, `voxelized_mesh`, `compound`.
- The `Shape` trait offers `compute_aabb(&Pose)`, `compute_local_aabb()`, `compute_swept_aabb(start, end)`, `compute_bounding_sphere`, `as_support_map()`, `as_composite_shape()`, `is_convex()`.

**Queries (`parry3d_f64::query`)**: every one returns `Result<_, Unsupported>`, where `Unsupported` is a unit struct.

```rust
fn intersection_test(pos1:&Pose,g1:&dyn Shape,pos2:&Pose,g2:&dyn Shape) -> Result<ShapeIntersection,Unsupported>;
fn distance(pos1,g1,pos2,g2) -> Result<ShapeDistance,Unsupported>;   // { distance, subshape1, subshape2 }
fn closest_points(pos1,g1,pos2,g2,max_dist:Real) -> Result<ClosestPoints,Unsupported>;
        // enum { Intersecting, WithinMargin(Vector,Vector), Disjoint }
fn contact(pos1,g1,pos2,g2,prediction:Real) -> Result<Option<Contact>,Unsupported>;
        // Contact { point1, point2, normal1, normal2, dist (negative = penetration), subshape1, subshape2 }
fn cast_shapes(pos1,vel1:Vector,g1,pos2,vel2:Vector,g2,ShapeCastOptions) -> Result<Option<ShapeCastHit>,Unsupported>;
        // options { max_time_of_impact, target_distance, stop_at_penetration, compute_impact_geometry_on_penetration }
fn cast_shapes_nonlinear(&NonlinearRigidMotion,g1,&NonlinearRigidMotion,g2,t0,t1,stop_at_penetration) -> Result<Option<ShapeCastHit>,Unsupported>;
        // NonlinearRigidMotion { start: Pose, local_center, linvel, angvel }
```

- `intersection_test` and `distance` no longer return a bare `bool` or `f64`; they return structs that carry the sub-shape ids (for example the triangle index). `ShapeIntersection::new(intersecting: bool)` is verified; the field name `intersecting` is inferred.
- `DefaultQueryDispatcher` covers ball-ball, cuboid-cuboid (SAT), cuboid-triangle, half-space versus support map, support map versus support map (GJK/EPA), and composite versus any shape through BVH traversal. TriMesh and Compound versus convex work for all queries.
- `QueryDispatcher` is a trait with a `pos12` relative-pose form and `.chain()` for custom pairs.

**BVH (`parry3d_f64::partitioning`)**: one structure, `Bvh`, is shared by TriMesh, Compound and user scenes.
- Build: `Bvh::from_leaves(BvhBuildStrategy::{Binned (default), Ploc}, &[Aabb])`, `from_iter`.
- Update: `insert(aabb, leaf_index: u32)`, `remove(leaf)`, `refit(&mut BvhWorkspace)`, `refit_parallel` and other `*_parallel` variants.
- Query: `intersect_aabb(&Aabb) -> impl Iterator<Item = u32>`, `traverse(|&BvhNode| -> TraversalAction)`, `find_best(max_cost, aabb_cost, leaf_cost)`, `leaf_pairs(&other, check)`, `cast_ray`, `project_point`, `root_aabb()`.
- This is enough for our own scene broad phase. The old `Qbvh` type is absent from the file list.

**Convex tools (`parry3d_f64::transformation`)**
- `convex_hull(&[Vector3]) -> (Vec<Vector3>, Vec<[u32; 3]>)` and `try_convex_hull -> Result<_, ConvexHullError>`.
- `vhacd::{VHACD, VHACDParameters}` is present in the f64 crate. `VHACD::decompose(&params, points, indices, keep_voxel_map)`, then `compute_convex_hulls(downsampling)` or `compute_exact_convex_hulls`.
- Parameters: `concavity`, `alpha`, `beta`, `resolution`, `plane_downsampling`, `convex_hull_downsampling`, `fill_mode`, `convex_hull_approximation`, `max_convex_hulls`. Defaults in 3D: `resolution: 64`, `concavity: 0.01`.
- Cost: voxel-based, roughly 0.1 to several seconds per link mesh depending on resolution [R, estimate]. Run it at import time and cache the result.

**`Aabb`**: `new(mins, maxs)`, `from_points`, `transform_by(&Pose)`, `merged`, `loosened(margin)`, `intersects`, `intersects_moving_aabb`.

### Snippet A: URDF geometry to shapes, state check, clearance

```rust
use parry3d_f64::{math::{Pose, Vector}, query, shape::{SharedShape, TriMeshFlags}, bounding_volume::{Aabb, BoundingVolume}};

pub enum LinkGeom<'a> { Box{size:[f64;3]}, Cylinder{radius:f64,length:f64}, Sphere{radius:f64},
                        Mesh{verts:&'a [Vector], tris:&'a [[u32;3]], scale:Vector} }
pub enum MeshMode { ConvexHull, ConvexDecomposition, TriMesh }

/// Returns the shape plus an extra local pose (URDF cylinders are Z-aligned, parry's are Y-aligned).
pub fn link_shape(g:&LinkGeom, mode:MeshMode, y_to_z:Pose) -> Result<(SharedShape,Option<Pose>),ShapeError> {
    Ok(match g {
        LinkGeom::Box{size}             => (SharedShape::cuboid(size[0]/2.,size[1]/2.,size[2]/2.), None),
        LinkGeom::Sphere{radius}        => (SharedShape::ball(*radius), None),
        LinkGeom::Cylinder{radius,length}=> (SharedShape::cylinder(length/2., *radius), Some(y_to_z)),
        LinkGeom::Mesh{verts,tris,scale} => {
            let v: Vec<Vector> = verts.iter().map(|p| *p * *scale).collect();
            match mode {
                MeshMode::ConvexHull => (SharedShape::convex_hull(&v).ok_or(ShapeError::DegenerateHull)?, None),
                MeshMode::ConvexDecomposition => (SharedShape::convex_decomposition(&v, tris), None), // VHACD: cache!
                MeshMode::TriMesh => (SharedShape::trimesh_with_flags(v, tris.to_vec(),
                        TriMeshFlags::MERGE_DUPLICATE_VERTICES | TriMeshFlags::DELETE_DEGENERATE_TRIANGLES)
                        .map_err(ShapeError::TriMesh)?, None),
            }
        }
    })
}

pub struct Body { pub shape: SharedShape, pub pose: Pose, pub aabb: Aabb }   // pose/aabb refreshed after FK
pub enum Hit { SelfPair(usize,usize), Env{link:usize, obstacle:u32} }

/// `pairs` = self-collision pairs NOT disabled by the ACM, ordered distal-first.
/// `env_bvh` has one leaf per obstacle (leaf index = obstacle index). `pad` = safety margin in metres.
pub fn first_collision(links:&[Body], pairs:&[(usize,usize)], env:&[Body],
                       env_bvh:&parry3d_f64::partitioning::Bvh, pad:f64) -> Result<Option<Hit>,query::Unsupported> {
    for (li,l) in links.iter().enumerate().rev() {                 // distal links first: they hit most often
        let q = l.aabb.loosened(pad);
        for oi in env_bvh.intersect_aabb(&q) {
            let o = &env[oi as usize];
            let hit = if pad == 0.0 { query::intersection_test(&l.pose,&*l.shape,&o.pose,&*o.shape)?.intersecting }
                      else { !matches!(query::closest_points(&l.pose,&*l.shape,&o.pose,&*o.shape,pad)?,
                                       query::ClosestPoints::Disjoint) };
            if hit { return Ok(Some(Hit::Env{link:li, obstacle:oi})); }
        }
    }
    for &(a,b) in pairs {
        if !links[a].aabb.intersects(&links[b].aabb) { continue; }
        if query::intersection_test(&links[a].pose,&*links[a].shape,&links[b].pose,&*links[b].shape)?.intersecting {
            return Ok(Some(Hit::SelfPair(a,b)));
        }
    }
    Ok(None)
}

/// Minimum clearance (metres); 0.0 if penetrating.
pub fn clearance(links:&[Body], env:&[Body]) -> Result<f64,query::Unsupported> {
    let mut d = f64::INFINITY;
    for l in links { for o in env {
        if l.aabb.loosened(d.min(1e3)).intersects(&o.aabb) {      // prune with the current best
            d = d.min(query::distance(&l.pose,&*l.shape,&o.pose,&*o.shape)?.distance);
        }
    }}
    Ok(d)
}
```

Treat `Unsupported` as a programming error surfaced through a typed `CollisionError`. Never map it to "free".

## 2. Collision modelling practice

### 2.1 Robot representation
What other stacks do [R]:
- **MoveIt + FCL**: URDF collision meshes as BVH meshes (often simplified), mesh-versus-mesh, per-link padding and scale.
- **Tesseract**: convex hulls per link with Bullet, which enables swept-hull continuous checks.
- **VAMP / cuRobo**: sphere sets per link, 20 to 80 spheres per arm, with SIMD or GPU batching; this trades model inflation for speed.

Recommendation for this project:
- **Default: one convex hull per link.** UR links are near-convex tubes, and GJK against the environment mesh BVH is fast and supports `distance`.
- **Opt-in: VHACD compound** for concave tools and grippers.
- **Never use TriMesh for robot links** in planning: mesh-mesh is slow and gives no interior semantics.
- Capsules are a later fast path that speeds up self-collision checks.

### 2.2 Padding and attached geometry
- Use two margins: a robot-wide `padding` (default 5 to 10 mm) implemented as a `closest_points(max_dist = pad)` test, plus optional per-link overrides.
- Avoid rebuilding shapes as `round_convex_hull` when the padding changes.
- Tools and payloads are extra bodies rigidly attached to `tool0` and get their own ACM rows. The tool is allowed against the flange link and checked against everything else.

### 2.3 Allowed-collision matrix, following the MoveIt Setup Assistant [R]
1. Disable **adjacent** links (parent and child across any joint; for fixed joints merge the bodies).
2. Check the **default pose**: pairs colliding there are disabled, with reason "default".
3. Sample `N = 10 000` uniform random configurations within limits (MoveIt default; its UI goes up to about 100k).
4. A pair colliding in at least 95% of samples is **always** in collision: disable it.
5. A pair that never collides in all N samples is **never** in collision: disable it.
6. Remaining pairs stay enabled.
7. Persist the result with its reasons in the robot package. The UR5e plugin ships a precomputed ACM, and any imported URDF can recompute it with a progress bar.

Caveat: "never" is statistical. Show the pair list in the UI and let the user re-enable pairs. Recompute when a tool is attached.

### 2.4 Environment and state-check cost
- Static obstacles are `TriMesh` (each with its own BVH) or primitives. A scene-level `Bvh` holds one leaf per obstacle; call `refit` when obstacles move.
- Per state: FK (6 pose multiplications), 6 to 8 link AABBs, a scene BVH query, then narrow phase only for overlapping pairs. Order distal links first, environment before self, and the pair that last collided first (a cache).
- Expected cost [estimate, to be benchmarked]: 2 to 20 µs when the arm's AABBs miss everything; 20 to 150 µs near a 100k-triangle mesh. That is roughly 10k to 100k state checks per second per core, comparable to MoveIt/FCL and 2 to 3 orders of magnitude slower than VAMP.

### 2.5 Edge validation
- **Implement first:** discretised checking at a joint-space resolution `δ`. Default: the maximum joint step is at most 1 to 2 degrees, and additionally the TCP or elbow sweep is at most 10 mm, using the per-link reach bound below. The number of steps is the larger of the two requirements.
- OMPL's `longest_valid_segment_fraction` defaults to 0.01 of the space extent and MoveIt configurations usually set 0.005 [R]. A fraction of the extent is a poor unit for ±360° UR joints (0.005 × 720° = 3.6°), so use absolute units.
- Check order is van der Corput bisection (midpoint first), which finds collisions earliest.
- **Certified mode, second step** (Schwarzer, Saha and Latombe adaptive checking [R]):
  - Precompute for each link `i` and joint `j ≤ i` the radius `r_ij`, the maximum distance from joint axis `j` to any point of link `i` over all configurations.
  - On a straight joint segment no point of link `i` travels more than `λ_i = Σ_j r_ij |Δq_j|`.
  - If `λ_i < d_i(q_a) + d_i(q_b)`, where `d_i` is the link's clearance from `query::distance`, the segment is free for link `i`. Otherwise bisect, down to a floor.
  - This yields a proof for the convex-padded model. Run it once on the final path, where distance queries cost 3 to 10× a boolean check.
- `cast_shapes_nonlinear` assumes a constant screw motion per body. Link motion under joint interpolation is not a constant screw, so it does not give a proof. Tesseract-style swept hulls between consecutive states are likewise only an approximation.

**Guarantee stated to the user:** "Path verified collision-free with X mm padding, sampled every at most Y degrees or Z mm" or, in certified mode, "proven collision-free for the padded convex model along the interpolated joint path". Neither statement covers controller tracking error or blend deviations.

## 3. Planners

**State space**
- R^n box with a weighted Euclidean metric `d = sqrt(Σ w_j Δq_j²)`. Default weights are proportional to each joint's maximum swept reach, so proximal joints cost more.
- UR joints are revolute with ±2π limits, not continuous. No wrapping topology applies: `q` and `q + 2π` are distinct states.
- For true `continuous` URDF joints provide an SO(2) component with shortest-arc difference and interpolation.

**RRT**
- Sample `q_rand` (with goal bias 0.05), find the nearest node, steer by at most `range`, add the node if the edge is valid, and stop when the goal is satisfied.
- `range` defaults to 20% of the maximum extent in OMPL; for arms 0.5 to 1.0 rad weighted is better.

**RRT-Connect (Kuffner and LaValle)**
- Two trees. `EXTEND(Ta, q_rand)` gives `q_new`; then `CONNECT(Tb, q_new)` repeats EXTEND until Trapped or Reached; then swap the trees.
- The goal tree is seeded with all goal configurations (multi-goal).
- Edge-check each step only once; the newly added edges are already validated.

**RRT\***
- After steering, collect neighbours within `r_n = min(η, γ (ln n / n)^{1/d})`, with `γ > 2 (1 + 1/d)^{1/d} (μ(X_free)/ζ_d)^{1/d}`. The k-nearest variant uses `k = ⌈k_rrg ln n⌉` with `k_rrg = e + e/d`; prefer it, since it needs no volume estimate.
- Choose the parent with minimum `cost(parent) + c(parent, new)` among collision-free edges, testing in ascending cost order and stopping at the first valid edge.
- Rewire each neighbour whose cost improves through the new node, and propagate the cost difference to its descendants (store child lists).
- Keep the best goal node, and prune with the admissible heuristic `g + h ≥ c_best`.

**Informed RRT\***
- After the first solution, sample from the prolate hyperspheroid with foci at start and goal.
- With `c_min = d(start, goal)`: draw `x_ball` uniformly in the unit n-ball, and form `x = C · diag(c_best/2, √(c_best² − c_min²)/2, …) · x_ball + centre`.
- `C` rotates `e1` onto the focal axis (a Householder reflection is enough).
- Work in metric-scaled coordinates (`√w_j q_j`) and reject samples outside the bounds.
- For multiple goals, pick a goal's ellipsoid with probability proportional to its measure and reject by the multiplicity of containing ellipsoids (the OMPL approach [R]).

**PRM, PRM\*, LazyPRM**
- Roadmap with k-nearest connection (k = 10) or the PRM* rule `k = ⌈e(1 + 1/d) ln n⌉`, then A* (in-house; `pathfinding` is not needed).
- LazyPRM defers edge checks to query time and removes invalid edges; it suits repeated queries in a static cell. Keep the roadmap cached per scene revision.
- Offer LazyPRM* as the multi-query option. It is optional for milestone 1.

**BIT\***
- Not for the first release: the batch queues, edge-queue invariants and pruning are intricate. Informed RRT* plus shortcutting covers the "optimising" choice.
- Revisit it, or AIT*/EIT*, later.

**Nearest neighbours**
- kiddo 6.3.0 [V] is designed for 2 to 4 dimensions, and its mutable tree has no rebalancing (README). Its dimension is a const generic [R], which fits runtime n-DoF badly.
- kd-tree 0.6.2 [V] is static only. rstar 0.14.0 [V] is an R-tree that supports insertion, also with const dimension.
- **Decision:** an in-house `NearestNeighbors` trait. Use a linear scan (SoA, about 1 to 2 ns per dimension per node) up to roughly 2 000 to 5 000 nodes, then an in-house GNAT or incremental kd-tree with runtime dimension. A linear scan of 5k nodes × 6D costs about 30 µs, which is comparable to one edge check; RRT-Connect solutions for arms typically need fewer than 2k nodes.

**Sampling**
- Uniform; goal bias; Gaussian (keep an invalid/valid pair's valid sample); bridge test (two invalid samples with a valid midpoint) for narrow passages. These are pluggable `Sampler`s.

**Termination**
- Any of: time, iterations, collision-check count, cost threshold, cancel flag.
- Deterministic mode uses iteration or check budgets only, because wall-clock stops are not reproducible.

**Random numbers** [V versions]
- rand 0.10.4 (2026-11-15), rand_chacha 0.10.0, rand_pcg 0.10.2, fastrand 2.5.0.
- Use `rand_chacha::ChaCha8Rng` (or `rand_pcg::Pcg64Mcg`) by explicit type; these have documented value-stable streams [R]. `StdRng` is explicitly not stable across rand versions [R], and oxmpl uses it [V].
- Derive floats ourselves, `(next_u64() >> 11) as f64 * 2^-53`, so that a rand upgrade cannot change paths.
- Cross-platform bit-identity additionally requires libm-independent trigonometry in FK (the `libm` crate; parry's `enhanced-determinism` feature exists for the same reason). Promise same-platform determinism and treat cross-platform as best effort.

**Parallel portfolio**
- Run K (planner, seed) jobs on threads, each with an iteration budget and its own RNG derived from `(seed, job index)`.
- Deterministic selection: wait for all jobs, or for a deterministic subset, and take the best cost with ties broken by lowest job index. Do not take the "first to finish" as MoveIt's parallel pipelines do.
- A fast mode that accepts the first finisher is offered and flagged as non-deterministic.

**Visualisation**
- Planners emit `PlannerEvent::{NodeAdded { id, parent, tree }, EdgeRewired, SolutionImproved { cost }}` into a bounded sink, and expose `fn graph(&self) -> PlannerGraph` (nodes as joint vectors, edges, tree tag).
- The UI maps nodes to TCP positions through FK for a 3D overlay.

## 4. Path quality

**OMPL `PathSimplifier`** [R: parameters from memory]
- `reduceVertices(maxSteps = 0 → path length, maxEmptySteps = 0, rangeRatio = 0.33)`: try to connect two non-adjacent vertices within a window and delete those between.
- `shortcutPath(maxSteps, maxEmptySteps, rangeRatio = 0.33, snapToVertex = 0.005)`: pick two random points on the path, not only vertices, and connect them. This is the most valuable operator.
- `collapseCloseVertices`: connect near-duplicate vertices.
- `smoothBSpline(maxSteps = 5, minChange)`: corner-cutting subdivision; each new midpoint is kept only if the edges are valid.
- `perturbPath(stepSize, …)`: random vertex perturbation accepted if the cost decreases.
- `simplify()` runs reduceVertices, collapseCloseVertices and shortcutPath in a loop while improving, then smoothBSpline, then re-validates.
- Add the per-joint **partial shortcut** (Geraerts and Overmars): interpolate one joint only between two points. It removes unnecessary joint motion, which matters for Q5.

**Hybridisation**
- From the portfolio's several solutions, build a graph over path vertices, add valid cross-connections between nearby states of different paths, and run a shortest-path search.
- It is cheap and noticeably lowers variance; schedule it at the end of milestone 1.

**Optimisation-based refinement** [R]
- CHOMP (covariant gradient, needs a signed-distance field), STOMP (sampling, no gradients), TrajOpt (sequential convex QP with signed-distance linearisation and swept hulls), GPMP2 (factor graph).
- A full TrajOpt is not realistic for the first release.
- A realistic light step is a **clearance and length elastic-band pass**: for each interior waypoint, take a gradient step on `w_len·smoothness + w_clr·hinge(d_safe − d)`, using `closest_points` witnesses mapped through the positional Jacobian, and accept it only if the edges remain valid. That is about 300 lines with no solver.
- clarabel 0.11.1 [V: Apache-2.0, 2025-06-11, pure-Rust default, optional faer ^0.21 and BLAS] is the QP candidate if TrajOpt is built later.

**Cost functions**
- Weighted joint length is the planning default.
- Reported only: TCP Cartesian length, minimum and mean clearance.
- Manipulability (`σ_min(J)` or the Yoshikawa measure) is a soft penalty and a hard floor for LIN moves.
- Joint reconfiguration counts wrist flips and elbow changes; treat it as a goal-selection cost, not a path cost.

**Metrics shown per run in the UI**
- Status; planning, simplification and total time.
- State checks, edge checks and nearest-neighbour queries.
- Tree or roadmap size.
- Joint length (weighted and raw, per joint) and TCP length.
- Maximum corner angle, and a smoothness figure (sum of squared second differences).
- Minimum clearance and where it occurs.
- Minimum `σ_min(J)`.
- Vertex count before and after simplification.
- Over a seed sweep: success rate and median/p95 of the above.

## 5. Task-level behaviours

- **PTP**: straight joint interpolation. Validate it; if it is blocked, either fail with the collision report or, with "auto-avoid", fall back to the sampling planner. Pilz PTP never avoids obstacles [R].
- **LIN / CIRC**:
  - Sample the Cartesian path (default 1 to 2 mm, about 0.5°; SLERP for orientation).
  - Solve analytic IK at each sample and stay on the start configuration's branch by picking the solution nearest to the previous one.
  - Fail on a jump: `max_j |Δq_j|` above the threshold, absolute per joint as in MoveIt 2's `CartesianPrecision`/revolute jump threshold [R], or a branch change.
  - Return the achieved fraction, as `computeCartesianPath` does [R], and name the cause: unreachable, joint limit `j`, singularity (`σ_min` below ε near wrist, elbow or shoulder alignment), or collision.
  - Optional ladder-graph mode in the style of Descartes [R]: all IK branches and ±2π turns per sample form the rungs, edges are bounded by joint speed, and a DAG shortest path finds a consistent branch when the start configuration is free to choose.
- **Free transfer**: the default sampling pipeline (section 9).
- **Sequences**: a strategy per segment (PTP, LIN, CIRC or Free). Each segment's end configuration seeds the next. Run a look-ahead over IK candidates of the next waypoints (small dynamic programme) to avoid dead-end branches.
- **Goal selection**:
  - Enumerate up to 8 IK branches times admissible ±2π turns per joint (the UR limits allow them).
  - Filter by limits and collision.
  - Rank by weighted joint travel from the start plus a penalty for deviating from the user's configuration hint (shoulder, elbow, wrist flags; turn numbers).
  - Pass the top k (for example 8) as the goal set.
- **Start-state fixing**:
  - If the start is out of bounds by less than ε (1e-3 rad), clamp it.
  - If it is in collision by less than the padding, sample within a small ball (0.02 rad, 100 tries) for a valid state and prepend a short segment. This follows MoveIt's FixStartState adapters [R].
  - Always report what was done; never do it silently.
- **Errors**: `StartInCollision { link_a, other: LinkOrObstacle, depth }`, `GoalInCollision { .. }`, `GoalUnreachable { reason: OutOfReach | JointLimit { joint } | NoIkBranchMatchesHint }`, `StartOutOfBounds { joint, value, limit }`, `NoPathWithinBudget { stats, nearest_goal_distance }`, `CartesianIncomplete { fraction, cause }`, `Cancelled`.

## 6. Build versus buy

| Crate | Facts [V] | Verdict |
|---|---|---|
| **oxmpl 0.8.1** | BSD-3, MSRV 1.85, depends on log and rand 0.10, 11 releases since 2025-06, 3k downloads. Clean OMPL-like traits (`StateSpace`, `StateValidityChecker`, `Goal*`, `Planner::solve(timeout) -> Result<Path, PlanningError>`). Integration tests per planner and space. Limitations are listed below the table. | Do not depend on it. Read it as a reference for space implementations. |
| **batchplan 0.11.0** | MIT/Apache, MSRV 1.90, first release 2026-10-21 and 16 releases in 26 days, 397 downloads. f32 glam 0.33, parry3d 0.31 (f32), urdf-rs 0.10, optional wgpu 30. URDF, MJCF and USD input, auto-fitted collision spheres, SIMD RRT-Connect, L-BFGS B-spline optimisation, jerk-limited timing; UR5e assets bundled. Ships `AGENTS.md`. | Too young, f32, sphere-only robot model, opinionated end-to-end API. Not a dependency. Useful as a benchmark comparator and a source of ideas (sphere fitting, SIMD checks). |
| **rrt 0.7.0** | 2023-02, rand 0.8, kdtree 0.7. | Stale; reject. |
| **openrr-planner 0.1.0** | 2023, ncollide3d 0.33, k 0.29, urdf-rs 0.6. | Dead stack; reject. |
| **pathfinding 4.16.0** | 2026-09, MSRV 1.88. | Not needed; A*/Dijkstra over our roadmap is about 60 lines. Fine if graph search is wanted elsewhere. |
| **kiddo 6.3.0** | 2026-09, MSRV 1.89; low-dimension focus, mutable tree without rebalancing. | Not for planner nearest-neighbour search. |
| **clarabel 0.11.1** | Apache-2.0, last release 2025-06. | Defer until a TrajOpt-class optimiser is in scope. |

oxmpl limitations found in `src/geometric/planners/rrt_connect.rs` [V]:
- Nearest neighbour is a linear scan with no abstraction.
- "RRT-Connect" performs a single `extend` on the other tree, not the greedy connect loop.
- The goal tree is seeded with one sampled goal.
- The edge check hard-codes `0.1 × longest_valid_segment`.
- It uses `StdRng`.
- There is no cancellation, statistics, tree export, simplifier or parameter schema.

**Layer verdicts:** state space and planners: build. Nearest-neighbour search: build. Simplification: build. Optimisation: build the light elastic-band pass; buy clarabel later. Collision: buy parry. Timing: buy rsruckig or evaluate copp.

## 7. Rust design

Crate `gc-planning` has no dependency on parry or on robots. `gc-robot-planning` adapts the robot model plus the collision world into `StateValidity`.

```rust
pub trait StateSpace: Send + Sync {
    type State: Clone + Send + Sync + 'static;
    fn dimension(&self) -> usize;
    fn distance(&self, a:&Self::State, b:&Self::State) -> f64;
    fn interpolate(&self, a:&Self::State, b:&Self::State, t:f64, out:&mut Self::State);
    fn sample_uniform(&self, rng:&mut dyn UniformSource, out:&mut Self::State);
    fn satisfies_bounds(&self, s:&Self::State) -> bool;
    fn enforce_bounds(&self, s:&mut Self::State);
    fn max_extent(&self) -> f64;
}
pub trait UniformSource { fn unit(&mut self) -> f64; }          // [0,1); backed by ChaCha8, value-stable

pub trait StateValidity<S: StateSpace>: Send + Sync {
    fn check(&self, s:&S::State) -> Result<(), Invalid>;        // Invalid carries link pair / obstacle id
    fn clearance(&self, _s:&S::State) -> Option<f64> { None }
}
pub trait MotionValidity<S: StateSpace>: Send + Sync {
    /// Ok(()) or the last valid parameter t in [0,1) plus the reason. `a` is assumed valid.
    fn check(&self, a:&S::State, b:&S::State) -> Result<(), (f64, Invalid)>;
}
pub struct DiscreteMotion<'a,S:StateSpace,V>{ pub space:&'a S, pub validity:&'a V, pub resolution:f64 } // bisection order

pub struct PlannerDescriptor { pub id:&'static str, pub name:&'static str, pub params:&'static [ParamSpec],
                               pub caps: Capabilities }          // optimal, anytime, multi_goal, multi_query, bidirectional
pub struct ParamSpec { pub key:&'static str, pub label:&'static str, pub kind:ParamKind /* F64{min,max,default,unit} | Int | Bool | Enum */ }

pub struct Budget { pub max_time:Option<Duration>, pub max_iterations:Option<u64>, pub max_checks:Option<u64>, pub cost_threshold:Option<f64> }
pub struct Context<'a> { pub seed:u64, pub budget:Budget, pub cancel:&'a AtomicBool, pub events:&'a dyn EventSink }
pub struct Request<'a,S:StateSpace> { pub space:&'a S, pub validity:&'a dyn StateValidity<S>, pub motion:&'a dyn MotionValidity<S>,
                                      pub start:S::State, pub goals:Vec<S::State>, pub params:ParamValues }
pub struct Solution<S:StateSpace> { pub path:Vec<S::State>, pub cost:f64, pub goal_index:usize, pub stats:Stats }
pub struct Stats { pub iterations:u64, pub state_checks:u64, pub edge_checks:u64, pub nn_queries:u64, pub nodes:usize, pub elapsed:Duration }
pub enum PlanError { InvalidStart(Invalid), NoValidGoal(Vec<Invalid>), BudgetExhausted(Stats), Cancelled(Stats), BadParam{key:&'static str} }

pub trait Planner<S: StateSpace>: Send {
    fn descriptor(&self) -> &'static PlannerDescriptor;
    fn solve(&mut self, req:&Request<S>, cx:&Context) -> Result<Solution<S>, PlanError>;
    fn graph(&self) -> PlannerGraph<S::State>;                  // for visualisation
}
pub trait PathProcessor<S: StateSpace> { fn id(&self)->&'static str;
    fn process(&self, path:&mut Vec<S::State>, req:&Request<S>, cx:&Context) -> ProcessReport; }
// Pipeline: [ReduceVertices, Shortcut, PartialShortcut, SmoothBSpline, Densify] then mandatory FinalValidation.
```

Generic RRT-Connect (complete logic; counters and events elided):

```rust
struct Node<T>{ q:T, parent:u32 }                               // u32::MAX = root
enum Ext { Trapped, Advanced(u32), Reached(u32) }

fn extend<S:StateSpace>(t:&mut Vec<Node<S::State>>, target:&S::State, range:f64, req:&Request<S>) -> Ext {
    let (mut best, mut bd) = (0usize, f64::INFINITY);           // NN: linear scan; swap for NearestNeighbors trait
    for (i,n) in t.iter().enumerate() { let d = req.space.distance(&n.q, target); if d < bd { bd = d; best = i; } }
    let reached = bd <= range;
    let mut q_new = target.clone();
    if !reached { req.space.interpolate(&t[best].q, target, range/bd, &mut q_new); }
    if req.motion.check(&t[best].q, &q_new).is_err() { return Ext::Trapped; }
    t.push(Node{ q:q_new, parent:best as u32 });
    let id = (t.len()-1) as u32;
    if reached { Ext::Reached(id) } else { Ext::Advanced(id) }
}
fn trace<T:Clone>(t:&[Node<T>], mut i:u32) -> (Vec<T>, u32) {   // returns states leaf->root and the root index
    let mut v = Vec::new();
    loop { v.push(t[i as usize].q.clone()); let p = t[i as usize].parent; if p == u32::MAX { return (v, i); } i = p; }
}
pub fn rrt_connect<S:StateSpace>(req:&Request<S>, cx:&Context, range:f64, rng:&mut dyn UniformSource)
    -> Result<Solution<S>, PlanError>
{
    req.validity.check(&req.start).map_err(PlanError::InvalidStart)?;
    let mut tb: Vec<Node<S::State>> = Vec::new(); let mut bad = Vec::new(); let mut goal_of_root = Vec::new();
    for (gi,g) in req.goals.iter().enumerate() {
        match req.validity.check(g) { Ok(()) => { tb.push(Node{q:g.clone(),parent:u32::MAX}); goal_of_root.push(gi); }
                                      Err(e) => bad.push(e) }
    }
    if tb.is_empty() { return Err(PlanError::NoValidGoal(bad)); }
    let mut ta = vec![Node{ q:req.start.clone(), parent:u32::MAX }];
    let (mut a_is_start, mut it, mut q_rand) = (true, 0u64, req.start.clone());
    let t0 = Instant::now();
    loop {
        if cx.cancel.load(Ordering::Relaxed) { return Err(PlanError::Cancelled(Stats::default())); }
        if cx.budget.max_iterations.is_some_and(|m| it >= m) || cx.budget.max_time.is_some_and(|m| t0.elapsed() >= m) {
            return Err(PlanError::BudgetExhausted(Stats::default()));
        }
        it += 1;
        req.space.sample_uniform(rng, &mut q_rand);
        if let Ext::Advanced(ia) | Ext::Reached(ia) = extend(&mut ta, &q_rand, range, req) {
            let q_new = ta[ia as usize].q.clone();
            let mut last = extend(&mut tb, &q_new, range, req);               // greedy CONNECT
            while let Ext::Advanced(_) = last { last = extend(&mut tb, &q_new, range, req); }
            if let Ext::Reached(ib) = last {
                let (ts, is, tg, ig) = if a_is_start { (&ta, ia, &tb, ib) } else { (&tb, ib, &ta, ia) };
                let (mut path, _) = trace(ts, is); path.reverse();            // start .. meeting point
                let (gpath, root) = trace(tg, ig);                            // meeting point .. goal root
                path.extend(gpath.into_iter().skip(1));
                let cost = path.windows(2).map(|w| req.space.distance(&w[0], &w[1])).sum();
                return Ok(Solution{ path, cost, goal_index: goal_of_root[root as usize], stats: Stats::default() });
            }
        }
        std::mem::swap(&mut ta, &mut tb); a_is_start = !a_is_start;
    }
}
```

The roots of the goal tree occupy indices `0..goals_valid`, so `goal_of_root[root]` identifies which IK candidate was reached.

**One planner, two robots**
- `RealVectorSpace { lo, hi, weights }` with `State = SmallVec<[f64; 8]>` serves both the 3D point robot (n = 3; validity is a `Ball` against the scene) and the arm (n = DoF; validity is FK plus the section 1 check).
- The point robot is the visual demonstration and test bed: the tree is drawn directly in workspace.

**Test strategy**
1. Analytic worlds:
   - Empty box: the path must equal the straight line after shortcutting.
   - Single wall with a gap of known width: the optimal length is known in closed form.
   - Two discs in 2D with a known tangent-path optimum: RRT* cost converges to within 5% in N iterations.
   - Disconnected space: must return `BudgetExhausted`, never a path.
2. Narrow-passage benchmarks (2D/3D slit, "bug trap") with success-rate thresholds per planner.
3. Property tests (proptest over seeds and random box worlds):
   - Every returned path passes an independent dense re-check at half the resolution and stays inside bounds.
   - Start and goal are exact.
   - Same seed and iteration budget give identical bytes.
   - For RRT* variants, cost is non-increasing with a larger iteration budget (same seed) and never below the straight-line bound.
   - Simplifiers never increase cost and never invalidate a path.
4. Collision layer: parry results compared with brute-force triangle tests on random convex pairs; ACM generation is stable for the UR5e (golden file).
5. UR5e golden scenes (table, fence, machine door) with a CI metrics report for Q3 to Q8.

## 8. Recommendation

1. **Collision**
   - parry3d-f64 0.31.1 with default features, plus `parallel` only if BVH builds prove slow.
   - Convex hull per robot link, computed at import and cached in the robot package; VHACD opt-in for tools.
   - Environment as `TriMesh` and primitives under one scene `Bvh`.
   - Padding through `closest_points(max_dist)`.
   - ACM generated by the Setup-Assistant algorithm (10k samples, 95% rule) and shipped precomputed for UR models.
2. **Edge checking**: bisection-ordered discrete checks in absolute units (default 1° or 10 mm swept) in milestone 1, with certified distance-bound validation of the final path as the next step.
3. **Default pipeline for "Free" moves**
   - IK goal set: top 8 by weighted travel and configuration hint.
   - Try the straight joint segment first.
   - Otherwise a portfolio of 4 RRT-Connect seeds.
   - Then hybridise, ReduceVertices, Shortcut plus PartialShortcut, B-spline smoothing, densify, final validation, and time parameterisation.
4. **Selectable planners in milestone 1**: RRT, RRT-Connect (default), RRT*, Informed RRT*, PRM/LazyPRM, all behind `PlannerDescriptor` with parameter schemas. BIT* and TrajOpt are deferred.
5. **Build all planning layers in-house** in a robot-agnostic `gc-planning` crate. Nearest-neighbour search starts as a linear scan behind a trait. RNG is `rand_chacha::ChaCha8Rng` with our own float derivation.
6. **LIN/CIRC**: analytic-IK branch tracking with absolute jump thresholds and fraction reporting. The ladder graph is an option, not the default.
7. **UI**: tree and roadmap overlay through the event sink, a per-run metrics panel, and a seed-sweep "compare planners" table using the Q1 to Q10 metrics.

## 9. Risks and unknowns

- **Performance is unmeasured.** The 10k to 100k checks per second estimate must be benchmarked early. If it is too slow, add capsule or sphere proxies for the robot and pre-simplified environment meshes.
- **GJK robustness** in f64 for thin or degenerate hulls and for `closest_points` near contact: fuzz it against brute force. `ConvexPolyhedron::from_convex_hull` returns `None` on degenerate input, so fall back to an AABB cuboid and warn.
- **parry API churn**: 0.31 changed the return types and the maths stack. Isolate parry behind our `gc-collision` crate and pin the minor version.
- **Convex hull over-approximation** can block legitimate tight motions, such as a gripper entering a fixture. This needs per-link VHACD and a user-editable ACM.
- **ACM "never" pairs** are statistical and can hide a real collision in rare poses.
- **Unverified parry details**: the `ShapeIntersection` field name, pose compose and inverse method names, the cylinder axis convention, and the absence of an MSRV declaration.
- **Recalled figures**: the OMPL and MoveIt parameter defaults (section 2.5, section 2.3, section 4) are from memory. Check them against upstream source before quoting them in the UI or documentation.
- **Determinism versus time budgets and threads**: deterministic mode must use iteration budgets. Cross-OS bit-identity needs libm-pinned trigonometry.
- **batchplan** is moving fast; it may become a credible "buy" within months. Re-evaluate before milestone 2.

## 10. Surprises

- parry's `intersection_test` and `distance` now return `ShapeIntersection` and `ShapeDistance` structs with sub-shape ids, not `bool` and `f64`. Most online examples are outdated. The `simd-stable` feature and `Qbvh` are gone; `Bvh` is the only tree.
- parry3d-f64 still pulls simba 0.10 (with `wide`) and rstar 0.13 even after dropping nalgebra.
- oxmpl's "RRT-Connect" does not implement the greedy connect step, and it seeds the goal tree with a single goal.
- UR joints are bounded ±360°, not continuous. The right model is a plain box in R^6 in which `q + 2π` variants are distinct goals. That multiplies IK candidates (up to 8 × turn variants) and makes goal selection a first-class quality factor.
- kiddo explicitly positions itself for 2 to 4 dimensions and its mutable tree never rebalances. For typical arm trees of under 5k nodes a linear scan is competitive.
- A one-month-old crate, batchplan, already bundles UR5e assets, URDF loading, SIMD RRT-Connect and jerk-limited timing on the same glam 0.33, parry 0.31 and wgpu 30 versions as our stack, but in f32 and sphere-only.
- `cast_shapes_nonlinear` exists, but it cannot certify joint-interpolated link motion. The distance-bound adaptive check gives a real proof using only `query::distance`.
