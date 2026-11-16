//! Bounding-volume hierarchy over the triangles of a mesh, for ray casting (picking).

use gc_math::{Aabb, DVec3, Ray};

use crate::MeshData;

const LEAF_SIZE: usize = 4;

#[derive(Clone, Debug)]
enum Node {
    Leaf { aabb: Aabb, start: usize, len: usize },
    Inner { aabb: Aabb, left: usize, right: usize },
}

impl Node {
    fn aabb(&self) -> &Aabb {
        match self {
            Self::Leaf { aabb, .. } | Self::Inner { aabb, .. } => aabb,
        }
    }
}

/// Where a ray hits a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Distance along the ray.
    pub t: f64,
    /// Index of the triangle that was hit.
    pub triangle: u32,
    /// The hit point.
    pub point: DVec3,
    /// Unit normal of the triangle, by its winding (not flipped towards the ray).
    pub normal: DVec3,
}

/// A static hierarchy over the triangles of one mesh.
#[derive(Clone, Debug)]
pub struct MeshBvh {
    nodes: Vec<Node>,
    /// Triangle indices, grouped so that each leaf owns a contiguous range.
    order: Vec<u32>,
    triangles: Vec<[DVec3; 3]>,
}

fn bounds(triangles: &[[DVec3; 3]], order: &[u32]) -> Aabb {
    Aabb::from_points(order.iter().flat_map(|i| triangles[*i as usize]))
}

/// Möller-Trumbore ray-triangle intersection, both faces. Returns the ray parameter.
fn hit_triangle(ray: &Ray, [a, b, c]: [DVec3; 3]) -> Option<f64> {
    let (edge1, edge2) = (b - a, c - a);
    let p = ray.dir.cross(edge2);
    let det = edge1.dot(p);
    // Relative threshold: rejects degenerate triangles and rays in the triangle's plane.
    if det.abs() <= 1e-14 * edge1.length() * edge2.length() {
        return None;
    }
    let s = ray.origin - a;
    let u = s.dot(p) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(edge1);
    let v = ray.dir.dot(q) / det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = edge2.dot(q) / det;
    (t >= 0.0).then_some(t)
}

impl MeshBvh {
    /// Builds the hierarchy (median split along the widest axis of the centroids).
    pub fn build(mesh: &MeshData) -> Self {
        let triangles: Vec<[DVec3; 3]> = mesh.triangles().collect();
        let mut order: Vec<u32> = (0..u32::try_from(triangles.len()).unwrap_or(u32::MAX)).collect();
        let mut nodes = Vec::new();
        if !order.is_empty() {
            let count = order.len();
            Self::build_node(&triangles, &mut order, 0, count, &mut nodes);
        }
        Self { nodes, order, triangles }
    }

    /// Builds the node for `order[start..start + len]` and returns its index. The recursion depth is
    /// logarithmic because every split halves the range.
    fn build_node(
        triangles: &[[DVec3; 3]],
        order: &mut [u32],
        start: usize,
        len: usize,
        nodes: &mut Vec<Node>,
    ) -> usize {
        let range = &mut order[start..start + len];
        let aabb = bounds(triangles, range);
        let index = nodes.len();
        if len <= LEAF_SIZE {
            nodes.push(Node::Leaf { aabb, start, len });
            return index;
        }
        let centroid = |i: u32| {
            let [a, b, c] = triangles[i as usize];
            (a + b + c) / 3.0
        };
        let spread = Aabb::from_points(range.iter().map(|i| centroid(*i))).size();
        let axis = if spread.x >= spread.y && spread.x >= spread.z {
            0
        } else if spread.y >= spread.z {
            1
        } else {
            2
        };
        let half = len / 2;
        // Ties are broken by triangle index so the tree is the same in every run.
        range.select_nth_unstable_by(half, |a, b| {
            centroid(*a)[axis].total_cmp(&centroid(*b)[axis]).then(a.cmp(b))
        });
        nodes.push(Node::Leaf { aabb, start, len }); // placeholder, replaced below
        let left = Self::build_node(triangles, order, start, half, nodes);
        let right = Self::build_node(triangles, order, start + half, len - half, nodes);
        nodes[index] = Node::Inner { aabb, left, right };
        index
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The bounding box of the mesh.
    pub fn aabb(&self) -> Aabb {
        self.nodes.first().map_or(Aabb::EMPTY, |node| *node.aabb())
    }

    /// The nearest hit of `ray` within `max_t`. Among hits at the same distance the lowest triangle index
    /// wins, so the result does not depend on the tree layout.
    pub fn raycast(&self, ray: &Ray, max_t: f64) -> Option<RayHit> {
        let mut best: Option<(f64, u32)> = None;
        let mut stack = Vec::with_capacity(64);
        if !self.nodes.is_empty() {
            stack.push(0_usize);
        }
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            let limit = best.map_or(max_t, |(t, _)| t);
            match node.aabb().intersect_ray(ray) {
                Some((enter, _)) if enter <= limit => {}
                _ => continue,
            }
            match node {
                Node::Inner { left, right, .. } => {
                    stack.push(*right);
                    stack.push(*left);
                }
                Node::Leaf { start, len, .. } => {
                    for triangle in &self.order[*start..start + len] {
                        let Some(t) = hit_triangle(ray, self.triangles[*triangle as usize]) else { continue };
                        let closer = match best {
                            None => t <= max_t,
                            Some((best_t, best_triangle)) => {
                                t < best_t || (t.total_cmp(&best_t).is_eq() && *triangle < best_triangle)
                            }
                        };
                        if closer {
                            best = Some((t, *triangle));
                        }
                    }
                }
            }
        }
        best.map(|(t, triangle)| {
            let [a, b, c] = self.triangles[triangle as usize];
            RayHit { t, triangle, point: ray.at(t), normal: (b - a).cross(c - a).normalize_or_zero() }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{cube, merged};

    fn brute_force(mesh: &MeshData, ray: &Ray) -> Option<(f64, u32)> {
        let mut best: Option<(f64, u32)> = None;
        for (index, triangle) in mesh.triangles().enumerate() {
            if let Some(t) = hit_triangle(ray, triangle)
                && best.is_none_or(|(best_t, _)| t < best_t)
            {
                best = Some((t, u32::try_from(index).unwrap()));
            }
        }
        best
    }

    #[test]
    fn hits_the_near_face_of_a_cube() {
        let bvh = MeshBvh::build(&cube([0.0; 3], 1.0));
        let ray = Ray::new(DVec3::new(0.25, 0.5, 5.0), -DVec3::Z).unwrap();
        let hit = bvh.raycast(&ray, f64::INFINITY).unwrap();
        assert!((hit.t - 4.0).abs() < 1e-12);
        assert!((hit.point - DVec3::new(0.25, 0.5, 1.0)).length() < 1e-12);
        assert!((hit.normal - DVec3::Z).length() < 1e-12);
        assert_eq!(bvh.raycast(&ray, 3.9), None, "max_t cuts the ray short");
        let miss = Ray::new(DVec3::new(2.0, 0.5, 5.0), -DVec3::Z).unwrap();
        assert_eq!(bvh.raycast(&miss, f64::INFINITY), None);
        let away = Ray::new(DVec3::new(0.25, 0.5, 5.0), DVec3::Z).unwrap();
        assert_eq!(bvh.raycast(&away, f64::INFINITY), None);
    }

    #[test]
    fn a_ray_from_inside_hits_the_back_face() {
        let bvh = MeshBvh::build(&cube([0.0; 3], 1.0));
        let ray = Ray::new(DVec3::splat(0.5), DVec3::X).unwrap();
        let hit = bvh.raycast(&ray, f64::INFINITY).unwrap();
        assert!((hit.t - 0.5).abs() < 1e-12);
        assert!((hit.normal - DVec3::X).length() < 1e-12, "the normal follows the winding, not the ray");
    }

    #[test]
    fn empty_and_degenerate_meshes_are_harmless() {
        let empty = MeshBvh::build(&MeshData::new(Vec::new(), Vec::new()).unwrap());
        let ray = Ray::new(DVec3::ZERO, DVec3::X).unwrap();
        assert_eq!(empty.raycast(&ray, f64::INFINITY), None);
        assert!(empty.aabb().is_empty());
        let sliver = MeshData::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], vec![0, 1, 2]).unwrap();
        let down = Ray::new(DVec3::new(0.5, 0.0, 1.0), -DVec3::Z).unwrap();
        assert_eq!(MeshBvh::build(&sliver).raycast(&down, f64::INFINITY), None);
    }

    #[test]
    fn agrees_with_brute_force_on_many_rays() {
        // A scene of many cubes at pseudo-random places, large enough for a deep tree.
        let mut state = 99_u64;
        let mut unit = || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 40) as f64 / (1_u64 << 24) as f64
        };
        let mut scene = cube([0.0; 3], 1.0);
        for _ in 0..60 {
            #[allow(clippy::cast_possible_truncation, reason = "test coordinates are small")]
            let at = [unit() as f32 * 10.0, unit() as f32 * 10.0, unit() as f32 * 10.0];
            #[allow(clippy::cast_possible_truncation, reason = "test sizes are small")]
            let size = 0.2 + unit() as f32;
            scene = merged(&scene, &cube(at, size));
        }
        let bvh = MeshBvh::build(&scene);
        assert_eq!(bvh.triangle_count(), 61 * 12);
        let mut hits = 0;
        for _ in 0..2000 {
            let origin = DVec3::new(unit() * 14.0 - 2.0, unit() * 14.0 - 2.0, unit() * 14.0 - 2.0);
            let dir = DVec3::new(unit() - 0.5, unit() - 0.5, unit() - 0.5);
            let Some(ray) = Ray::new(origin, dir) else { continue };
            let expected = brute_force(&scene, &ray);
            let got = bvh.raycast(&ray, f64::INFINITY).map(|hit| (hit.t, hit.triangle));
            assert_eq!(got, expected);
            hits += usize::from(got.is_some());
        }
        assert!(hits > 100, "the comparison is only meaningful if rays hit, got {hits}");
    }
}
