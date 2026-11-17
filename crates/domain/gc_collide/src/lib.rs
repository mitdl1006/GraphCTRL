//! Collision checking between rigid bodies made of simple shapes, convex hulls and triangle meshes.
//!
//! The world knows nothing about robots: a body is a pose plus shapes. Bodies are either *movable* (robot
//! links, tools) or *static* (equipment, obstacles). A movable body keeps a padding distance from static
//! bodies; coming closer than the padding counts as a hit even without contact. Pairs of movable bodies
//! are checked for actual overlap, unless the pair is explicitly allowed (neighbouring links).
//!
//! The collision library is used only inside this crate.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

use std::collections::BTreeSet;
use std::sync::Arc;

use gc_math::{DPose3, DQuat, DVec3};
use gc_mesh::MeshData;
use parry3d_f64::query::{self, ClosestPoints};
use parry3d_f64::shape::SharedShape;

/// A shape to build a body from. Sizes are metres; every shape is centred on its own origin.
#[derive(Clone, Debug)]
pub enum ShapeDesc {
    /// A sphere of the given radius.
    Ball(f64),
    /// A box with the given edge lengths.
    Box([f64; 3]),
    /// A cylinder along its local Z axis.
    Cylinder {
        /// Radius.
        radius: f64,
        /// Length along Z.
        length: f64,
    },
    /// The convex hull of the points.
    Hull(Vec<[f64; 3]>),
    /// A triangle mesh, used as it is (may be concave). Meant for static bodies.
    Mesh(Arc<MeshData>),
}

/// Why a body could not be built or a query could not be answered.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CollideError {
    /// A size is not a positive finite number.
    #[error("a shape has a size that is not a positive number")]
    BadSize,
    /// The points do not span a volume, so they have no convex hull.
    #[error("the points do not span a volume")]
    DegenerateHull,
    /// The mesh has no triangles or could not be prepared for collision checking.
    #[error("the mesh cannot be used for collision checking: {0}")]
    BadMesh(String),
    /// The collision library cannot compare these two kinds of shape. Never treated as "no collision".
    #[error("these two shapes cannot be compared")]
    Unsupported,
}

/// Identifies a body of a [`World`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(usize);

/// How two bodies relate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Proximity {
    /// They overlap or touch.
    Colliding,
    /// They do not touch but are closer than the padding; the value is the gap.
    WithinPadding(f64),
    /// They are further apart than the padding.
    Clear,
}

/// The first problem found by [`World::first_hit`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The movable body involved.
    pub a: BodyId,
    /// The other body.
    pub b: BodyId,
    /// Overlap, or merely inside the padding.
    pub proximity: Proximity,
}

#[derive(Clone)]
struct Part {
    local: DPose3,
    shape: SharedShape,
    /// Centre and half extents of the shape's bounds in its own frame.
    centre: DVec3,
    half: DVec3,
}

impl Part {
    fn new(local: DPose3, shape: SharedShape) -> Self {
        let aabb = shape.compute_local_aabb();
        Self { local, shape, centre: (aabb.mins + aabb.maxs) / 2.0, half: (aabb.maxs - aabb.mins) / 2.0 }
    }
}

#[derive(Clone)]
struct Body {
    name: String,
    movable: bool,
    padding: f64,
    pose: DPose3,
    parts: Vec<Part>,
    /// World-space bounds of all parts, for skipping far-apart pairs cheaply.
    low: DVec3,
    high: DVec3,
}

impl Body {
    fn refresh_bounds(&mut self) {
        let mut low = DVec3::INFINITY;
        let mut high = DVec3::NEG_INFINITY;
        // The local bounds turned with the part: a little looser than the shape's own bounds, but it
        // costs the same for every shape, however many vertices a hull has.
        for part in &self.parts {
            let pose = self.pose * part.local;
            let centre = pose * part.centre;
            let turn = gc_math::DMat3::from_quat(pose.rotation);
            let half = turn.x_axis.abs() * part.half.x
                + turn.y_axis.abs() * part.half.y
                + turn.z_axis.abs() * part.half.z;
            low = low.min(centre - half);
            high = high.max(centre + half);
        }
        (self.low, self.high) = (low, high);
    }
}

fn build(desc: &ShapeDesc) -> Result<(DPose3, SharedShape), CollideError> {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    Ok(match desc {
        ShapeDesc::Ball(radius) if positive(*radius) => (DPose3::IDENTITY, SharedShape::ball(*radius)),
        ShapeDesc::Box(size) if size.iter().all(|s| positive(*s)) => {
            (DPose3::IDENTITY, SharedShape::cuboid(size[0] / 2.0, size[1] / 2.0, size[2] / 2.0))
        }
        ShapeDesc::Cylinder { radius, length } if positive(*radius) && positive(*length) => {
            // The library's cylinder lies along Y; a quarter turn about X puts it along Z.
            let upright = DPose3::from_rotation(DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2));
            (upright, SharedShape::cylinder(length / 2.0, *radius))
        }
        ShapeDesc::Ball(_) | ShapeDesc::Box(_) | ShapeDesc::Cylinder { .. } => {
            return Err(CollideError::BadSize);
        }
        ShapeDesc::Hull(points) => {
            let points: Vec<DVec3> = points.iter().map(|p| DVec3::from_array(*p)).collect();
            if points.iter().any(|p| !p.is_finite()) {
                return Err(CollideError::BadSize);
            }
            if !spans_a_volume(&points) {
                return Err(CollideError::DegenerateHull);
            }
            (DPose3::IDENTITY, SharedShape::convex_hull(&points).ok_or(CollideError::DegenerateHull)?)
        }
        ShapeDesc::Mesh(mesh) => {
            if mesh.triangle_count() == 0 {
                return Err(CollideError::BadMesh("it has no triangles".to_owned()));
            }
            let vertices = mesh
                .positions()
                .iter()
                .map(|p| DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])))
                .collect();
            let indices = mesh.indices().chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
            let shape =
                SharedShape::trimesh(vertices, indices).map_err(|e| CollideError::BadMesh(e.to_string()))?;
            (DPose3::IDENTITY, shape)
        }
    })
}

/// True when the points do not all lie in one plane (or on one line, or in one place).
fn spans_a_volume(points: &[DVec3]) -> bool {
    let Some(first) = points.first().copied() else { return false };
    let farthest = |score: &dyn Fn(DVec3) -> f64| {
        points
            .iter()
            .copied()
            .fold((first, 0.0), |best, p| if score(p) > best.1 { (p, score(p)) } else { best })
    };
    let (second, length) = farthest(&|p| (p - first).length());
    if length <= 0.0 {
        return false;
    }
    let along = (second - first) / length;
    let (third, width) = farthest(&|p| (p - first).cross(along).length());
    let normal = (third - first).cross(along);
    let (_, height) = farthest(&|p| (p - first).dot(normal).abs() / normal.length().max(f64::MIN_POSITIVE));
    width > 1e-9 * length && height > 1e-9 * length
}

/// The convex hulls of the connected parts of a mesh: a much tighter stand-in for a robot link than one
/// hull around everything. Parts too flat to have a hull are skipped.
pub fn hulls_of(mesh: &MeshData) -> Vec<ShapeDesc> {
    mesh.welded(0.0)
        .connected_components()
        .iter()
        .map(|part| part.positions().iter().map(|p| p.map(f64::from)).collect::<Vec<_>>())
        .filter(|points: &Vec<[f64; 3]>| build(&ShapeDesc::Hull(points.clone())).is_ok())
        .map(ShapeDesc::Hull)
        .collect()
}

/// A set of bodies that can be checked against each other.
#[derive(Clone, Default)]
pub struct World {
    bodies: Vec<Body>,
    allowed: BTreeSet<(usize, usize)>,
}

fn proximity(a: &Body, b: &Body, padding: f64) -> Result<Proximity, CollideError> {
    // Bounds further apart than the padding cannot contain a closer pair of shapes.
    let gap = (a.low - b.high).max(b.low - a.high).max_element();
    if gap > padding {
        return Ok(Proximity::Clear);
    }
    let mut closest = Proximity::Clear;
    for part_a in &a.parts {
        for part_b in &b.parts {
            let (pose_a, pose_b) = (a.pose * part_a.local, b.pose * part_b.local);
            let unsupported = |_e| CollideError::Unsupported;
            if padding > 0.0 {
                // One query answers both questions: overlapping, or how close within the padding.
                match query::closest_points(&pose_a, &*part_a.shape, &pose_b, &*part_b.shape, padding)
                    .map_err(unsupported)?
                {
                    ClosestPoints::Intersecting => return Ok(Proximity::Colliding),
                    ClosestPoints::WithinMargin(on_a, on_b) => {
                        let gap = on_a.distance(on_b);
                        let closer = !matches!(closest, Proximity::WithinPadding(d) if d <= gap);
                        if gap <= padding && closer {
                            closest = Proximity::WithinPadding(gap);
                        }
                    }
                    ClosestPoints::Disjoint => {}
                }
            } else if query::intersection_test(&pose_a, &*part_a.shape, &pose_b, &*part_b.shape)
                .map_err(unsupported)?
                .intersecting
            {
                return Ok(Proximity::Colliding);
            }
        }
    }
    Ok(closest)
}

impl World {
    /// An empty world.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a body made of shapes placed in the body's frame. `padding` is the distance a movable body
    /// keeps from static bodies (ignored for static bodies).
    pub fn add_body(
        &mut self,
        name: &str,
        movable: bool,
        padding: f64,
        shapes: &[(DPose3, ShapeDesc)],
    ) -> Result<BodyId, CollideError> {
        if !(padding.is_finite() && padding >= 0.0) {
            return Err(CollideError::BadSize);
        }
        let mut parts = Vec::with_capacity(shapes.len());
        for (pose, desc) in shapes {
            let (correction, shape) = build(desc)?;
            parts.push(Part::new(*pose * correction, shape));
        }
        let mut body = Body {
            name: name.to_owned(),
            movable,
            padding,
            pose: DPose3::IDENTITY,
            parts,
            low: DVec3::ZERO,
            high: DVec3::ZERO,
        };
        body.refresh_bounds();
        self.bodies.push(body);
        Ok(BodyId(self.bodies.len() - 1))
    }

    /// Number of bodies.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// True when the world has no bodies.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// The name a body was added with.
    pub fn name(&self, body: BodyId) -> &str {
        self.bodies.get(body.0).map_or("", |b| b.name.as_str())
    }

    /// Moves a body.
    pub fn set_pose(&mut self, body: BodyId, pose: DPose3) {
        if let Some(b) = self.bodies.get_mut(body.0) {
            b.pose = pose;
            b.refresh_bounds();
        }
    }

    /// Changes the padding of a movable body.
    pub fn set_padding(&mut self, body: BodyId, padding: f64) {
        if let Some(b) = self.bodies.get_mut(body.0) {
            b.padding = padding.max(0.0);
        }
    }

    /// Never report this pair (neighbouring links, a tool and the link it is mounted on, ...).
    pub fn allow(&mut self, a: BodyId, b: BodyId) {
        self.allowed.insert((a.0.min(b.0), a.0.max(b.0)));
    }

    /// Check this pair again.
    pub fn disallow(&mut self, a: BodyId, b: BodyId) {
        self.allowed.remove(&(a.0.min(b.0), a.0.max(b.0)));
    }

    /// True when the pair is excluded from checking.
    pub fn is_allowed(&self, a: BodyId, b: BodyId) -> bool {
        self.allowed.contains(&(a.0.min(b.0), a.0.max(b.0)))
    }

    /// How two bodies relate, with the padding that applies to the pair: the movable body's padding
    /// against a static body, none between two movable bodies.
    pub fn proximity(&self, a: BodyId, b: BodyId) -> Result<Proximity, CollideError> {
        let (Some(first), Some(second)) = (self.bodies.get(a.0), self.bodies.get(b.0)) else {
            return Ok(Proximity::Clear);
        };
        let padding = match (first.movable, second.movable) {
            (true, false) => first.padding,
            (false, true) => second.padding,
            _ => 0.0,
        };
        proximity(first, second, padding)
    }

    /// The pairs that are checked: every movable body against every other body, except allowed pairs and
    /// pairs of static bodies. Movable against static comes first (the common failure), in body order.
    fn pairs(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let movable = move |i: &usize| self.bodies[*i].movable;
        let against_static = (0..self.bodies.len()).filter(movable).flat_map(move |a| {
            (0..self.bodies.len()).filter(move |b| !self.bodies[*b].movable).map(move |b| (a, b))
        });
        let among_movable = (0..self.bodies.len())
            .filter(movable)
            .flat_map(move |a| (a + 1..self.bodies.len()).filter(movable).map(move |b| (a, b)));
        against_static.chain(among_movable).filter(|(a, b)| !self.allowed.contains(&(*a.min(b), *a.max(b))))
    }

    /// The first pair that collides or violates the padding, or `None` when the state is valid.
    pub fn first_hit(&self) -> Result<Option<Hit>, CollideError> {
        for (a, b) in self.pairs() {
            let found = self.proximity(BodyId(a), BodyId(b))?;
            if found != Proximity::Clear {
                return Ok(Some(Hit { a: BodyId(a), b: BodyId(b), proximity: found }));
            }
        }
        Ok(None)
    }

    /// The smallest distance between any checked pair (0 when something collides; infinite when nothing
    /// is checked).
    pub fn clearance(&self) -> Result<f64, CollideError> {
        let mut smallest = f64::INFINITY;
        for (a, b) in self.pairs() {
            let (first, second) = (&self.bodies[a], &self.bodies[b]);
            for part_a in &first.parts {
                for part_b in &second.parts {
                    let (pose_a, pose_b) = (first.pose * part_a.local, second.pose * part_b.local);
                    let found = query::distance(&pose_a, &*part_a.shape, &pose_b, &*part_b.shape)
                        .map_err(|_unsupported| CollideError::Unsupported)?;
                    smallest = smallest.min(found.distance);
                }
            }
        }
        Ok(smallest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, y: f64, z: f64) -> DPose3 {
        DPose3::from_translation(DVec3::new(x, y, z))
    }

    fn one(desc: ShapeDesc) -> Vec<(DPose3, ShapeDesc)> {
        vec![(DPose3::IDENTITY, desc)]
    }

    /// A unit cube mesh centred on the origin (as an obstacle given by triangles).
    fn cube_mesh() -> Arc<MeshData> {
        let c = |x: f32, y: f32, z: f32| [x - 0.5, y - 0.5, z - 0.5];
        let positions = vec![
            c(0., 0., 0.),
            c(1., 0., 0.),
            c(1., 1., 0.),
            c(0., 1., 0.),
            c(0., 0., 1.),
            c(1., 0., 1.),
            c(1., 1., 1.),
            c(0., 1., 1.),
        ];
        #[rustfmt::skip]
        let indices = vec![0,2,1, 0,3,2, 4,5,6, 4,6,7, 0,1,5, 0,5,4, 2,3,7, 2,7,6, 1,2,6, 1,6,5, 3,0,4, 3,4,7];
        Arc::new(MeshData::new(positions, indices).unwrap())
    }

    #[test]
    fn padding_turns_a_near_miss_into_a_hit() {
        let mut world = World::new();
        let ball = world.add_body("ball", true, 0.005, &one(ShapeDesc::Ball(0.1))).unwrap();
        let wall = world.add_body("wall", false, 0.0, &one(ShapeDesc::Box([1.0, 1.0, 0.2]))).unwrap();
        // The wall's top is at z = 0.1. The ball's lowest point is `gap` above it.
        let mut place = |gap: f64| {
            world.set_pose(ball, at(0.0, 0.0, 0.1 + 0.1 + gap));
            world.first_hit().unwrap().map(|hit| hit.proximity)
        };
        assert_eq!(place(0.02), None);
        assert!(matches!(place(0.003), Some(Proximity::WithinPadding(d)) if (d - 0.003).abs() < 1e-9));
        assert_eq!(place(-0.01), Some(Proximity::Colliding));
        world.set_pose(ball, at(0.0, 0.0, 0.23));
        assert!((world.clearance().unwrap() - 0.03).abs() < 1e-9);
        assert_eq!(world.first_hit().unwrap(), None);
        world.set_padding(ball, 0.05);
        let hit = world.first_hit().unwrap().unwrap();
        assert_eq!((hit.a, hit.b), (ball, wall));
        assert_eq!(world.name(hit.b), "wall");
    }

    #[test]
    fn cylinders_stand_along_z() {
        let mut world = World::new();
        let probe = world.add_body("probe", true, 0.0, &one(ShapeDesc::Ball(0.01))).unwrap();
        world.add_body("post", false, 0.0, &one(ShapeDesc::Cylinder { radius: 0.1, length: 2.0 })).unwrap();
        let mut hit_at = |x: f64, y: f64, z: f64| {
            world.set_pose(probe, at(x, y, z));
            world.first_hit().unwrap().is_some()
        };
        assert!(hit_at(0.0, 0.0, 0.9), "inside the post near its top");
        assert!(!hit_at(0.0, 0.0, 1.2), "above the post");
        assert!(!hit_at(0.0, 0.9, 0.0), "beside the post: it does not lie along Y");
        assert!(!hit_at(0.9, 0.0, 0.0));
        assert!(hit_at(0.105, 0.0, 0.5), "touching its side");
    }

    #[test]
    fn movable_bodies_collide_with_each_other_unless_allowed() {
        let mut world = World::new();
        let upper = world.add_body("upper", true, 0.01, &one(ShapeDesc::Box([0.4, 0.1, 0.1]))).unwrap();
        let lower = world.add_body("lower", true, 0.01, &one(ShapeDesc::Box([0.4, 0.1, 0.1]))).unwrap();
        world.set_pose(lower, at(0.35, 0.0, 0.0));
        let hit = world.first_hit().unwrap().unwrap();
        assert_eq!((hit.a, hit.b, hit.proximity), (upper, lower, Proximity::Colliding));
        // Padding does not apply between two movable bodies: a 5 mm gap is fine.
        world.set_pose(lower, at(0.405, 0.0, 0.0));
        assert_eq!(world.first_hit().unwrap(), None);
        // Neighbouring links overlap at their joint by design.
        world.set_pose(lower, at(0.35, 0.0, 0.0));
        world.allow(lower, upper);
        assert!(world.is_allowed(upper, lower));
        assert_eq!(world.first_hit().unwrap(), None);
        world.disallow(upper, lower);
        assert!(world.first_hit().unwrap().is_some());
    }

    #[test]
    fn meshes_and_hulls_take_part() {
        let mut world = World::new();
        let ball = world.add_body("ball", true, 0.0, &one(ShapeDesc::Ball(0.1))).unwrap();
        world.add_body("crate", false, 0.0, &one(ShapeDesc::Mesh(cube_mesh()))).unwrap();
        world.set_pose(ball, at(0.0, 0.0, 0.65));
        assert_eq!(world.first_hit().unwrap(), None);
        assert!((world.clearance().unwrap() - 0.05).abs() < 1e-6);
        world.set_pose(ball, at(0.0, 0.0, 0.58));
        assert!(world.first_hit().unwrap().is_some());

        // The same cube as the hull of its mesh, used as a movable body.
        let hulls = hulls_of(&cube_mesh());
        assert_eq!(hulls.len(), 1);
        let mut second = World::new();
        let block = second.add_body("block", true, 0.0, &[(DPose3::IDENTITY, hulls[0].clone())]).unwrap();
        second
            .add_body("floor", false, 0.0, &[(at(0.0, 0.0, -0.1), ShapeDesc::Box([4.0, 4.0, 0.2]))])
            .unwrap();
        second.set_pose(block, at(0.0, 0.0, 0.52));
        assert_eq!(second.first_hit().unwrap(), None);
        second.set_pose(block, at(0.0, 0.0, 0.49));
        assert!(second.first_hit().unwrap().is_some());
    }

    #[test]
    fn bad_shapes_are_refused_at_construction() {
        let mut world = World::new();
        let add = |world: &mut World, desc| world.add_body("x", true, 0.0, &one(desc)).map(|_| ());
        assert_eq!(add(&mut world, ShapeDesc::Ball(0.0)), Err(CollideError::BadSize));
        assert_eq!(add(&mut world, ShapeDesc::Box([1.0, -1.0, 1.0])), Err(CollideError::BadSize));
        assert_eq!(
            add(&mut world, ShapeDesc::Cylinder { radius: f64::NAN, length: 1.0 }),
            Err(CollideError::BadSize)
        );
        let flat = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(add(&mut world, ShapeDesc::Hull(flat)), Err(CollideError::DegenerateHull));
        let empty = Arc::new(MeshData::new(Vec::new(), Vec::new()).unwrap());
        assert!(matches!(add(&mut world, ShapeDesc::Mesh(empty)), Err(CollideError::BadMesh(_))));
        assert_eq!(
            world.add_body("x", true, -1.0, &one(ShapeDesc::Ball(1.0))).map(|_| ()),
            Err(CollideError::BadSize)
        );
        assert!(world.is_empty());
    }

    #[test]
    fn sphere_distances_agree_with_the_formula() {
        let mut state = 5_u64;
        let mut unit = || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1_u64 << 53) as f64
        };
        for _ in 0..200 {
            let (r1, r2) = (0.05 + unit() * 0.3, 0.05 + unit() * 0.3);
            let centre = DVec3::new(unit() - 0.5, unit() - 0.5, unit() - 0.5) * 2.0;
            let mut world = World::new();
            let moving = world.add_body("a", true, 0.02, &one(ShapeDesc::Ball(r1))).unwrap();
            world.add_body("b", false, 0.0, &one(ShapeDesc::Ball(r2))).unwrap();
            world.set_pose(moving, DPose3::from_translation(centre));
            let gap = centre.length() - r1 - r2;
            let hit = world.first_hit().unwrap().map(|h| h.proximity);
            if gap <= 0.0 {
                assert_eq!(hit, Some(Proximity::Colliding));
            } else if gap < 0.02 {
                assert!(
                    matches!(hit, Some(Proximity::WithinPadding(d)) if (d - gap).abs() < 1e-9),
                    "{hit:?} vs {gap}"
                );
            } else {
                assert_eq!(hit, None, "gap {gap}");
            }
            assert!((world.clearance().unwrap() - gap.max(0.0)).abs() < 1e-9);
        }
    }
}
