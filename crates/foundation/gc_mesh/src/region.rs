//! The flat face of a mesh around a triangle: what a CAD program selects as a "face" when the model is a
//! triangle mesh. Neighbouring triangles join while they lie in the same plane; the outline is the
//! region's edges that no other region triangle shares.

use std::collections::{BTreeMap, BTreeSet};

use gc_math::DVec3;

use crate::MeshData;

/// A flat region of a mesh, in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanarRegion {
    /// The triangles of the region.
    pub triangles: Vec<u32>,
    /// The unit normal (by the winding of the first triangle).
    pub normal: DVec3,
    /// The centroid of the region (a point of its plane).
    pub point: DVec3,
    /// The area.
    pub area: f64,
    /// The outline: edges with only one region triangle, as pairs of corner positions.
    pub outline: Vec<[DVec3; 2]>,
}

type Key = [i64; 3];
/// An edge's two corners and how many region triangles use it.
type Use = ([f32; 3], [f32; 3], u32);

fn key(p: [f32; 3]) -> Key {
    #[allow(clippy::cast_possible_truncation, reason = "micrometre cells of a robot cell fit i64")]
    p.map(|c| (f64::from(c) * 1e6).round() as i64)
}

fn corner(p: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
}

/// Which triangles share an edge with each triangle (corners closer than a micrometre count as shared).
/// Building it is the costly part of finding a region; keep one per mesh and reuse it.
#[derive(Clone, Debug, Default)]
pub struct Adjacency {
    neighbours: Vec<Vec<u32>>,
}

impl Adjacency {
    /// The adjacency of a mesh's triangles.
    pub fn of(mesh: &MeshData) -> Self {
        let positions = mesh.positions();
        let indices = mesh.indices();
        let count = indices.len() / 3;
        let mut by_edge: BTreeMap<(Key, Key), Vec<u32>> = BTreeMap::new();
        for t in 0..count {
            let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
            let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else { continue };
            for (p, q) in [(a, b), (b, c), (c, a)] {
                let (kp, kq) = (key(p), key(q));
                #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
                by_edge.entry((kp.min(kq), kp.max(kq))).or_default().push(t as u32);
            }
        }
        let mut neighbours = vec![Vec::new(); count];
        for sharing in by_edge.values() {
            for &t in sharing {
                if let Some(list) = neighbours.get_mut(t as usize) {
                    list.extend(sharing.iter().copied().filter(|other| *other != t));
                }
            }
        }
        Self { neighbours }
    }

    fn of_triangle(&self, t: usize) -> &[u32] {
        self.neighbours.get(t).map_or(&[], Vec::as_slice)
    }
}

/// The flat region around `triangle`: triangles reached across shared edges whose normals are within
/// `angle` (radians) of the first one and whose corners lie within `distance` of its plane. Corners
/// closer than a micrometre count as shared, so meshes that repeat corners per triangle work too.
/// `None` when the triangle does not exist or is degenerate.
pub fn planar_region(mesh: &MeshData, triangle: u32, angle: f64, distance: f64) -> Option<PlanarRegion> {
    planar_region_in(mesh, &Adjacency::of(mesh), triangle, angle, distance)
}

/// [`planar_region`] with the mesh's adjacency already built.
pub fn planar_region_in(
    mesh: &MeshData,
    adjacency: &Adjacency,
    triangle: u32,
    angle: f64,
    distance: f64,
) -> Option<PlanarRegion> {
    let positions = mesh.positions();
    let indices = mesh.indices();
    let count = indices.len() / 3;
    let corners = |t: usize| -> Option<[[f32; 3]; 3]> {
        let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
        Some([at(0)?, at(1)?, at(2)?])
    };
    let normal_of = |c: &[[f32; 3]; 3]| {
        let (a, b, c) = (corner(c[0]), corner(c[1]), corner(c[2]));
        (b - a).cross(c - a)
    };
    let first = corners(triangle as usize)?;
    let normal = normal_of(&first).try_normalize()?;
    let point = corner(first[0]);
    let cos_limit = gc_math::fm::cos(angle);

    let joins = |t: usize| -> bool {
        corners(t).is_some_and(|c| {
            normal_of(&c).try_normalize().is_some_and(|n| n.dot(normal) >= cos_limit)
                && c.iter().all(|p| (corner(*p) - point).dot(normal).abs() <= distance)
        })
    };
    if triangle as usize >= count {
        return None;
    }
    let mut region = BTreeSet::from([triangle as usize]);
    let mut todo = vec![triangle as usize];
    while let Some(t) = todo.pop() {
        for &next in adjacency.of_triangle(t) {
            let next = next as usize;
            if !region.contains(&next) && joins(next) {
                region.insert(next);
                todo.push(next);
            }
        }
    }
    let mut area = 0.0;
    let mut weighted = DVec3::ZERO;
    let mut uses: BTreeMap<(Key, Key), Use> = BTreeMap::new();
    for &t in &region {
        let Some(c) = corners(t) else { continue };
        let piece = normal_of(&c).length() / 2.0;
        area += piece;
        weighted += (corner(c[0]) + corner(c[1]) + corner(c[2])) * (piece / 3.0);
        for (a, b) in [(c[0], c[1]), (c[1], c[2]), (c[2], c[0])] {
            let (ka, kb) = (key(a), key(b));
            uses.entry((ka.min(kb), ka.max(kb))).or_insert((a, b, 0)).2 += 1;
        }
    }
    let outline =
        uses.values().filter(|(_, _, n)| *n == 1).map(|(a, b, _)| [corner(*a), corner(*b)]).collect();
    #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
    let triangles = region.into_iter().map(|t| t as u32).collect();
    let centroid = if area > 0.0 { weighted / area } else { point };
    Some(PlanarRegion { triangles, normal, point: centroid, area, outline })
}

/// A cylindrical region of a mesh, in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct CylinderRegion {
    /// The triangles of the region.
    pub triangles: Vec<u32>,
    /// A point of the axis: the middle of the region along it.
    pub centre: DVec3,
    /// The unit axis direction.
    pub axis: DVec3,
    /// The radius.
    pub radius: f64,
    /// The length of the region along the axis.
    pub length: f64,
}

/// The cylindrical surface around `triangle`, when there is one: triangles reached across edges where
/// the surface bends by less than `bend` (radians) are fitted with an axis (the direction all their
/// normals are square to) and a circle; the fit must hold within `tolerance` (relative to the radius).
/// `None` for flat or irregular surfaces.
pub fn cylindrical_region(
    mesh: &MeshData,
    triangle: u32,
    bend: f64,
    tolerance: f64,
) -> Option<CylinderRegion> {
    cylindrical_region_in(mesh, &Adjacency::of(mesh), triangle, bend, tolerance)
}

/// [`cylindrical_region`] with the mesh's adjacency already built.
pub fn cylindrical_region_in(
    mesh: &MeshData,
    adjacency: &Adjacency,
    triangle: u32,
    bend: f64,
    tolerance: f64,
) -> Option<CylinderRegion> {
    let positions = mesh.positions();
    let indices = mesh.indices();
    let count = indices.len() / 3;
    let corners = |t: usize| -> Option<[[f32; 3]; 3]> {
        let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
        Some([at(0)?, at(1)?, at(2)?])
    };
    let normal_of = |t: usize| {
        corners(t).and_then(|c| {
            let (a, b, c) = (corner(c[0]), corner(c[1]), corner(c[2]));
            (b - a).cross(c - a).try_normalize()
        })
    };
    let first = normal_of(triangle as usize)?;
    if triangle as usize >= count {
        return None;
    }
    let cos_bend = gc_math::fm::cos(bend);
    let mut region = BTreeSet::from([triangle as usize]);
    let mut todo = vec![triangle as usize];
    while let Some(t) = todo.pop() {
        let Some(n) = normal_of(t) else { continue };
        for &next in adjacency.of_triangle(t) {
            let next = next as usize;
            if !region.contains(&next) && normal_of(next).is_some_and(|m| m.dot(n) >= cos_bend) {
                region.insert(next);
                todo.push(next);
            }
        }
    }
    let normals: Vec<DVec3> = region.iter().filter_map(|t| normal_of(*t)).collect();
    // The axis is square to every normal: sum the cross products with the first normal, each turned to
    // agree with the sum so far.
    let mut sum = DVec3::ZERO;
    for n in &normals {
        let c = first.cross(*n);
        sum += if c.dot(sum) < 0.0 { -c } else { c };
    }
    let axis = sum.try_normalize()?;
    if normals.iter().any(|n| n.dot(axis).abs() > tolerance.max(0.02)) {
        return None;
    }
    // Fit a circle to the corners projected on the plane square to the axis (Kasa's algebraic fit).
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    let points: Vec<DVec3> = region.iter().filter_map(|t| corners(*t)).flat_map(|c| c.map(corner)).collect();
    let flat: Vec<(f64, f64)> = points.iter().map(|p| (p.dot(u), p.dot(v))).collect();
    let (mut sxx, mut sxy, mut syy, mut sx, mut sy, mut sz, mut sxz, mut syz) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    #[allow(clippy::cast_precision_loss, reason = "corner counts are small")]
    let n = flat.len() as f64;
    for (x, y) in &flat {
        let z = x * x + y * y;
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sx += x;
        sy += y;
        sz += z;
        sxz += x * z;
        syz += y * z;
    }
    // Solve [sxx sxy sx; sxy syy sy; sx sy n] [a b c]ᵀ = [sxz syz sz]ᵀ; centre (a/2, b/2).
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let m = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let d = det3(m);
    if d.abs() < 1e-30 {
        return None;
    }
    let rhs = [sxz, syz, sz];
    let with = |col: usize| {
        let mut k = m;
        for row in 0..3 {
            k[row][col] = rhs[row];
        }
        det3(k) / d
    };
    let (a, b, c) = (with(0), with(1), with(2));
    let (cx, cy) = (a / 2.0, b / 2.0);
    let radius = (c + cx * cx + cy * cy).sqrt();
    if !(radius.is_finite() && radius > 0.0) {
        return None;
    }
    if flat
        .iter()
        .any(|(x, y)| (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius).abs() > tolerance * radius)
    {
        return None;
    }
    let along: Vec<f64> = points.iter().map(|p| p.dot(axis)).collect();
    let low = along.iter().copied().fold(f64::INFINITY, f64::min);
    let high = along.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let centre = u * cx + v * cy + axis * ((low + high) / 2.0);
    #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
    let triangles = region.into_iter().map(|t| t as u32).collect();
    Some(CylinderRegion { triangles, centre, axis, radius, length: high - low })
}

/// A conical region of a mesh, in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ConeRegion {
    /// The triangles of the region.
    pub triangles: Vec<u32>,
    /// The point the cone's surface lines meet in (it may lie outside the region: a truncated cone).
    pub apex: DVec3,
    /// The unit axis direction, from the apex towards the region.
    pub axis: DVec3,
    /// The angle between the axis and the surface, radians (between 0 and a quarter turn).
    pub half_angle: f64,
    /// Where the region begins and ends, measured from the apex along the axis.
    pub span: (f64, f64),
}

/// The conical surface around `triangle`, when there is one: triangles reached across edges where the
/// surface bends by less than `bend` (radians) are fitted with an axis (the direction all their normals
/// make the same angle with) and an apex (the point all their planes pass through); the fit must hold
/// within `tolerance`. `None` for flat, cylindrical or irregular surfaces.
pub fn conical_region(mesh: &MeshData, triangle: u32, bend: f64, tolerance: f64) -> Option<ConeRegion> {
    conical_region_in(mesh, &Adjacency::of(mesh), triangle, bend, tolerance)
}

/// [`conical_region`] with the mesh's adjacency already built.
pub fn conical_region_in(
    mesh: &MeshData,
    adjacency: &Adjacency,
    triangle: u32,
    bend: f64,
    tolerance: f64,
) -> Option<ConeRegion> {
    let positions = mesh.positions();
    let indices = mesh.indices();
    let count = indices.len() / 3;
    if triangle as usize >= count {
        return None;
    }
    let corners = |t: usize| -> Option<[DVec3; 3]> {
        let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
        Some([corner(at(0)?), corner(at(1)?), corner(at(2)?)])
    };
    let normal_of = |t: usize| corners(t).and_then(|[a, b, c]| (b - a).cross(c - a).try_normalize());
    let first = normal_of(triangle as usize)?;
    let cos_bend = gc_math::fm::cos(bend);
    let mut region = BTreeSet::from([triangle as usize]);
    let mut todo = vec![triangle as usize];
    while let Some(t) = todo.pop() {
        let Some(n) = normal_of(t) else { continue };
        for &next in adjacency.of_triangle(t) {
            let next = next as usize;
            if !region.contains(&next) && normal_of(next).is_some_and(|m| m.dot(n) >= cos_bend) {
                region.insert(next);
                todo.push(next);
            }
        }
    }
    let faces: Vec<(DVec3, DVec3)> =
        region.iter().filter_map(|t| Some((normal_of(*t)?, corners(*t)?[0]))).collect();
    if faces.len() < 3 {
        return None;
    }
    // The normals of a cone all make the same angle with its axis: their tips lie in one plane square
    // to it. The axis is square to every difference of normals.
    let differences: Vec<DVec3> =
        faces.iter().map(|(n, _)| *n - first).filter(|d| d.length() > 1e-9).collect();
    let mut sum = DVec3::ZERO;
    for pair in differences.windows(2) {
        let c = pair[0].cross(pair[1]);
        sum += if c.dot(sum) < 0.0 { -c } else { c };
    }
    let mut axis = sum.try_normalize()?;
    #[allow(clippy::cast_precision_loss, reason = "triangle counts are small")]
    let mut lean = faces.iter().map(|(n, _)| n.dot(axis)).sum::<f64>() / faces.len() as f64;
    // Outward normals lean back towards the apex.
    if lean > 0.0 {
        axis = -axis;
        lean = -lean;
    }
    let slack = tolerance.max(0.02);
    // Nearly square to the axis is a cylinder, nearly along it a flat face.
    if lean.abs() < slack
        || lean.abs() > 1.0 - slack
        || faces.iter().any(|(n, _)| (n.dot(axis) - lean).abs() > slack)
    {
        return None;
    }
    // The apex lies in every triangle's plane: least squares over n·(apex − p) = 0.
    let mut m = [[0.0_f64; 3]; 3];
    let mut rhs = [0.0_f64; 3];
    for (n, p) in &faces {
        let n = n.to_array();
        let reach = DVec3::from_array(n).dot(*p);
        for row in 0..3 {
            for col in 0..3 {
                m[row][col] += n[row] * n[col];
            }
            rhs[row] += n[row] * reach;
        }
    }
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det3(m);
    if d.abs() < 1e-18 {
        return None;
    }
    let with = |col: usize| {
        let mut k = m;
        for row in 0..3 {
            k[row][col] = rhs[row];
        }
        det3(k) / d
    };
    let apex = DVec3::new(with(0), with(1), with(2));
    let points: Vec<DVec3> = region.iter().filter_map(|t| corners(*t)).flatten().collect();
    let along: Vec<f64> = points.iter().map(|p| (*p - apex).dot(axis)).collect();
    let low = along.iter().copied().fold(f64::INFINITY, f64::min);
    let high = along.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let size = high.abs().max(low.abs());
    if !apex.is_finite() || low < -tolerance * size || size <= 0.0 {
        return None;
    }
    // The angle of the surface itself, from the corners (flat facets lean a little less than the cone
    // they are cut from): every corner's distance from the axis grows with its height in one ratio.
    let ratios: Vec<f64> = points
        .iter()
        .zip(&along)
        .filter(|(_, h)| **h > 1e-9 * size)
        .map(|(p, h)| ((*p - apex) - axis * *h).length() / *h)
        .collect();
    if ratios.is_empty() {
        return None;
    }
    #[allow(clippy::cast_precision_loss, reason = "corner counts are small")]
    let slope = ratios.iter().sum::<f64>() / ratios.len() as f64;
    if !(slope.is_finite() && slope > 0.0) || ratios.iter().any(|r| (r - slope).abs() > tolerance.max(1e-6)) {
        return None;
    }
    let half_angle = gc_math::fm::atan2(slope, 1.0);
    #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
    let triangles = region.into_iter().map(|t| t as u32).collect();
    Some(ConeRegion { triangles, apex, axis, half_angle, span: (low, high) })
}

/// A spherical region of a mesh, in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SphereRegion {
    /// The triangles of the region.
    pub triangles: Vec<u32>,
    /// The centre.
    pub centre: DVec3,
    /// The radius.
    pub radius: f64,
}

/// Solves a small linear system by elimination with pivoting; `None` when it has no single solution.
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|x, y| a[*x][col].abs().total_cmp(&a[*y][col].abs()))?;
        if a[pivot][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..N {
            let factor = a[row][col] / a[col][col];
            for k in col..N {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let rest: f64 = (row + 1..N).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - rest) / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// The spherical surface around `triangle`, when there is one: triangles reached across edges where the
/// surface bends by less than `bend` (radians) are fitted with a sphere through their corners; the fit
/// must hold within `tolerance` (relative to the radius) and the triangles must face straight away from
/// (or towards) the centre. `None` for flat, cylindrical, conical or irregular surfaces.
pub fn spherical_region(mesh: &MeshData, triangle: u32, bend: f64, tolerance: f64) -> Option<SphereRegion> {
    spherical_region_in(mesh, &Adjacency::of(mesh), triangle, bend, tolerance)
}

/// [`spherical_region`] with the mesh's adjacency already built.
pub fn spherical_region_in(
    mesh: &MeshData,
    adjacency: &Adjacency,
    triangle: u32,
    bend: f64,
    tolerance: f64,
) -> Option<SphereRegion> {
    let positions = mesh.positions();
    let indices = mesh.indices();
    if triangle as usize >= indices.len() / 3 {
        return None;
    }
    let corners = |t: usize| -> Option<[DVec3; 3]> {
        let at = |k: usize| indices.get(3 * t + k).and_then(|i| positions.get(*i as usize)).copied();
        Some([corner(at(0)?), corner(at(1)?), corner(at(2)?)])
    };
    let normal_of = |t: usize| corners(t).and_then(|[a, b, c]| (b - a).cross(c - a).try_normalize());
    normal_of(triangle as usize)?;
    let cos_bend = gc_math::fm::cos(bend);
    let mut region = BTreeSet::from([triangle as usize]);
    let mut todo = vec![triangle as usize];
    while let Some(t) = todo.pop() {
        let Some(n) = normal_of(t) else { continue };
        for &next in adjacency.of_triangle(t) {
            let next = next as usize;
            if !region.contains(&next) && normal_of(next).is_some_and(|m| m.dot(n) >= cos_bend) {
                region.insert(next);
                todo.push(next);
            }
        }
    }
    if region.len() < 4 {
        return None;
    }
    let points: Vec<DVec3> = region.iter().filter_map(|t| corners(*t)).flatten().collect();
    // |p|² = 2 c·p + (r² − |c|²): least squares for the centre c and that last constant, about the
    // points' mean so that the numbers stay small.
    #[allow(clippy::cast_precision_loss, reason = "corner counts are small")]
    let mean = points.iter().copied().sum::<DVec3>() / points.len() as f64;
    let mut m = [[0.0_f64; 4]; 4];
    let mut rhs = [0.0_f64; 4];
    for p in &points {
        let q = *p - mean;
        let row = [2.0 * q.x, 2.0 * q.y, 2.0 * q.z, 1.0];
        let value = q.length_squared();
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] += row[i] * row[j];
            }
            rhs[i] += row[i] * value;
        }
    }
    let [cx, cy, cz, constant] = solve(m, rhs)?;
    let offset = DVec3::new(cx, cy, cz);
    let radius = (constant + offset.length_squared()).sqrt();
    let centre = mean + offset;
    let extent = points.iter().map(|p| p.distance(mean)).fold(0.0, f64::max);
    // A nearly flat patch fits a huge sphere: not a spherical face.
    if !(radius.is_finite() && radius > 0.0) || radius > 1e3 * extent {
        return None;
    }
    if points.iter().any(|p| (p.distance(centre) - radius).abs() > tolerance * radius) {
        return None;
    }
    // Every triangle looks along its radius (a cone or cylinder that happens to pass the fit does not).
    let radial = region.iter().all(|t| {
        let (Some(n), Some([a, b, c])) = (normal_of(*t), corners(*t)) else { return false };
        let out = ((a + b + c) / 3.0 - centre).try_normalize().unwrap_or(DVec3::ZERO);
        n.dot(out).abs() > 1.0 - tolerance.max(0.02)
    });
    if !radial {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, reason = "triangle indices fit u32")]
    let triangles = region.into_iter().map(|t| t as u32).collect();
    Some(SphereRegion { triangles, centre, radius })
}

/// A circular edge of a mesh (the rim of a hole, the end of a shaft), in the mesh's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct CircleEdge {
    /// Which of the given edges make it up (indices into the list passed in).
    pub edges: Vec<u32>,
    /// The centre.
    pub centre: DVec3,
    /// The unit normal of the circle's plane: the side the flat face next to the edge looks to when
    /// there is one (the side Insert mates against), otherwise either side.
    pub normal: DVec3,
    /// The radius.
    pub radius: f64,
    /// True for a full circle, false for an arc.
    pub closed: bool,
}

/// The circular edge that `picked` (an index into `edges`, the mesh's feature edges as vertex pairs,
/// see [`crate::edges::feature_edges`]) belongs to: the edges joined end to end through corners where
/// exactly two feature edges meet, when at least five of them lie on one circle within `tolerance`
/// (relative to the radius). `None` for straight or irregular edges.
pub fn circular_edge(
    mesh: &MeshData,
    edges: &[[u32; 2]],
    picked: usize,
    tolerance: f64,
) -> Option<CircleEdge> {
    let positions = mesh.positions();
    let at = |index: u32| positions.get(index as usize).copied().map(corner);
    // Corners that share a position (to a micrometre) are one.
    let key = |index: u32| -> Option<[i64; 3]> {
        let p = positions.get(index as usize)?;
        #[allow(clippy::cast_possible_truncation, reason = "micrometre cells of a robot cell fit i64")]
        Some(p.map(|c| (f64::from(c) * 1e6).round() as i64))
    };
    let mut by_corner: BTreeMap<[i64; 3], Vec<usize>> = BTreeMap::new();
    for (i, edge) in edges.iter().enumerate() {
        by_corner.entry(key(edge[0])?).or_default().push(i);
        by_corner.entry(key(edge[1])?).or_default().push(i);
    }
    let first = *edges.get(picked)?;
    // Walk from one end of the picked edge until the chain ends, branches or comes back.
    let walk = |start: u32| -> Option<(Vec<usize>, Vec<DVec3>, bool)> {
        let mut chain = Vec::new();
        let mut points = Vec::new();
        let (mut edge, mut corner_key) = (picked, key(start)?);
        loop {
            let here = by_corner.get(&corner_key)?;
            if here.len() != 2 {
                return Some((chain, points, false));
            }
            let next = *here.iter().find(|e| **e != edge)?;
            if next == picked {
                return Some((chain, points, true));
            }
            let [a, b] = *edges.get(next)?;
            let far = if key(a)? == corner_key { b } else { a };
            chain.push(next);
            points.push(at(far)?);
            edge = next;
            corner_key = key(far)?;
            if chain.len() > edges.len() {
                return None;
            }
        }
    };
    let (ahead, ahead_points, closed) = walk(first[1])?;
    let (behind, behind_points) =
        if closed { (Vec::new(), Vec::new()) } else { walk(first[0]).map(|w| (w.0, w.1))? };
    let mut chain: Vec<usize> = behind.iter().rev().copied().collect();
    chain.push(picked);
    chain.extend(ahead);
    let mut points: Vec<DVec3> = behind_points.iter().rev().copied().collect();
    points.push(at(first[0])?);
    points.push(at(first[1])?);
    points.extend(ahead_points);
    if closed {
        // The walk ends on the corner it started from.
        points.pop();
    }
    if points.len() < 5 {
        return None;
    }
    // The plane: the normal from the turns of the polyline about its mean.
    #[allow(clippy::cast_precision_loss, reason = "corner counts are small")]
    let mean = points.iter().copied().sum::<DVec3>() / points.len() as f64;
    let mut sum = DVec3::ZERO;
    for pair in points.windows(2) {
        let c = (pair[0] - mean).cross(pair[1] - mean);
        sum += if c.dot(sum) < 0.0 { -c } else { c };
    }
    let mut normal = sum.try_normalize()?;
    let u = normal.any_orthonormal_vector();
    let w = normal.cross(u);
    // A circle through the corners in that plane: x² + y² = 2a x + 2b y + c.
    let mut m = [[0.0_f64; 3]; 3];
    let mut rhs = [0.0_f64; 3];
    for p in &points {
        let q = *p - mean;
        let (x, y) = (q.dot(u), q.dot(w));
        let row = [2.0 * x, 2.0 * y, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += row[i] * row[j];
            }
            rhs[i] += row[i] * (x * x + y * y);
        }
    }
    let [a, b, c] = solve(m, rhs)?;
    let radius = (c + a * a + b * b).sqrt();
    let centre = mean + u * a + w * b;
    let extent = points.iter().map(|p| p.distance(mean)).fold(0.0, f64::max);
    if !(radius.is_finite() && radius > 0.0) || radius > 1e3 * extent {
        return None;
    }
    let off = points.iter().any(|p| {
        let q = *p - centre;
        q.dot(normal).abs() > tolerance * radius || (q.length() - radius).abs() > tolerance * radius
    });
    if off {
        return None;
    }
    // The side the flat face on the edge looks to: a triangle on the picked edge whose normal lies along
    // the circle's.
    let picked_keys = [key(first[0])?, key(first[1])?];
    for triangle in mesh.indices().chunks_exact(3) {
        let keys = [key(triangle[0]), key(triangle[1]), key(triangle[2])];
        if !picked_keys.iter().all(|k| keys.contains(&Some(*k))) {
            continue;
        }
        let (Some(p0), Some(p1), Some(p2)) = (at(triangle[0]), at(triangle[1]), at(triangle[2])) else {
            continue;
        };
        let Some(face) = (p1 - p0).cross(p2 - p0).try_normalize() else { continue };
        if face.dot(normal).abs() > 0.99 {
            if face.dot(normal) < 0.0 {
                normal = -normal;
            }
            break;
        }
    }
    #[allow(clippy::cast_possible_truncation, reason = "edge indices fit u32")]
    let edges = chain.into_iter().map(|e| e as u32).collect();
    Some(CircleEdge { edges, centre, normal, radius, closed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_face_is_two_triangles_with_four_outline_edges() {
        let cube = crate::test_util::cube([0.0; 3], 2.0);
        let region = planar_region(&cube, 0, 0.01, 1e-6).unwrap();
        assert_eq!(region.triangles.len(), 2);
        assert!((region.area - 4.0).abs() < 1e-9);
        assert_eq!(region.outline.len(), 4);
        // The centroid is the middle of the face.
        let middle = region.point;
        assert!(region.normal.dot(middle - DVec3::new(1.0, 1.0, 1.0)).abs() > 0.99, "{middle:?}");
        let soup = crate::test_util::soup(&cube);
        let again = planar_region(&soup, 0, 0.01, 1e-6).unwrap();
        assert_eq!(again.triangles.len(), 2);
        assert!(planar_region(&cube, 999, 0.01, 1e-6).is_none());
    }

    #[test]
    fn a_cylinder_side_is_recognised_with_its_axis_and_radius() {
        let cylinder = crate::primitives::cylinder(0.05, 0.3, 48).unwrap();
        // Find a side triangle: its normal is square to Z.
        let side = (0..cylinder.triangle_count())
            .find(|t| {
                let c: Vec<DVec3> = cylinder.triangles().nth(*t).unwrap().to_vec();
                (c[1] - c[0]).cross(c[2] - c[0]).normalize().z.abs() < 1e-6
            })
            .unwrap();
        #[allow(clippy::cast_possible_truncation, reason = "test mesh")]
        let found = cylindrical_region(&cylinder, side as u32, 20f64.to_radians(), 0.01).unwrap();
        assert!(found.axis.z.abs() > 1.0 - 1e-9, "{:?}", found.axis);
        assert!((found.radius - 0.05).abs() < 1e-4, "{}", found.radius);
        assert!((found.length - 0.3).abs() < 1e-6);
        assert!(found.centre.truncate().length() < 1e-6);
        // A box face is not a cylinder.
        let cube = crate::test_util::cube([0.0; 3], 1.0);
        assert!(cylindrical_region(&cube, 0, 20f64.to_radians(), 0.01).is_none());
    }

    /// A truncated cone: radius 1 at z = 0, radius 0.5 at z = 1, so the apex is at z = 2 and the surface
    /// makes atan(0.5) with the axis.
    fn frustum(segments: u32) -> MeshData {
        let mut positions = Vec::new();
        for k in 0..segments {
            let turn = std::f32::consts::TAU * k as f32 / segments as f32;
            let (s, c) = turn.sin_cos();
            positions.push([c, s, 0.0]);
            positions.push([0.5 * c, 0.5 * s, 1.0]);
        }
        let mut indices = Vec::new();
        for k in 0..segments {
            let (a, b) = (2 * k, 2 * k + 1);
            let (c, d) = (2 * ((k + 1) % segments), 2 * ((k + 1) % segments) + 1);
            indices.extend([a, c, b, b, c, d]);
        }
        MeshData::new(positions, indices).unwrap()
    }

    #[test]
    fn a_frustum_side_is_a_cone_and_a_cylinder_or_a_cube_is_not() {
        let mesh = frustum(48);
        let cone = conical_region(&mesh, 0, 0.3, 1e-3).unwrap();
        assert_eq!(cone.triangles.len(), 96);
        assert!((cone.apex - DVec3::new(0.0, 0.0, 2.0)).length() < 1e-3, "{:?}", cone.apex);
        assert!((cone.axis - DVec3::new(0.0, 0.0, -1.0)).length() < 1e-6, "{:?}", cone.axis);
        assert!((cone.half_angle - gc_math::fm::atan2(0.5, 1.0)).abs() < 1e-6, "{}", cone.half_angle);
        assert!((cone.span.0 - 1.0).abs() < 1e-3 && (cone.span.1 - 2.0).abs() < 1e-3);
        assert!(cylindrical_region(&mesh, 0, 0.3, 1e-3).is_none());
        let cube = crate::test_util::cube([0.0; 3], 2.0);
        assert!((0..12).all(|t| conical_region(&cube, t, 0.3, 1e-3).is_none()));
        assert!(conical_region(&mesh, u32::MAX, 0.3, 1e-3).is_none());
    }

    #[test]
    fn a_ball_is_a_sphere_and_a_cube_a_cylinder_and_a_cone_are_not() {
        let ball = crate::primitives::sphere(0.25, 24).unwrap();
        let sphere = spherical_region(&ball, 0, 0.6, 1e-3).unwrap();
        assert!(sphere.centre.length() < 1e-6, "{:?}", sphere.centre);
        assert!((sphere.radius - 0.25).abs() < 1e-6, "{}", sphere.radius);
        assert_eq!(sphere.triangles.len(), ball.triangle_count());
        let cube = crate::test_util::cube([0.0; 3], 2.0);
        assert!((0..12).all(|t| spherical_region(&cube, t, 0.6, 1e-3).is_none()));
        let rod = crate::primitives::cylinder(0.1, 0.5, 32).unwrap();
        #[allow(clippy::cast_possible_truncation, reason = "a small test mesh")]
        let none = (0..rod.triangle_count() as u32).all(|t| spherical_region(&rod, t, 0.6, 1e-3).is_none());
        assert!(none);
        assert!(spherical_region(&frustum(48), 0, 0.3, 1e-3).is_none());
        assert!(spherical_region(&ball, u32::MAX, 0.6, 1e-3).is_none());
    }

    #[test]
    fn the_rim_of_a_cylinder_is_a_circular_edge_and_a_cube_edge_is_not() {
        let rod = crate::primitives::cylinder(0.1, 0.5, 32).unwrap();
        let edges = crate::edges::feature_edges(&rod, 0.5);
        assert!(!edges.is_empty());
        let mut seen = 0;
        for picked in 0..edges.len() {
            let Some(circle) = circular_edge(&rod, &edges, picked, 1e-3) else { continue };
            seen += 1;
            assert!(circle.closed && circle.edges.len() == 32, "{circle:?}");
            assert!((circle.radius - 0.1).abs() < 1e-6);
            // A rim at either end of the axis, its normal looking out of the end face.
            assert!(circle.centre.x.abs() < 1e-6 && circle.centre.y.abs() < 1e-6);
            assert!((circle.centre.z.abs() - 0.25).abs() < 1e-6, "{:?}", circle.centre);
            assert!((circle.normal.z - circle.centre.z.signum()).abs() < 1e-6, "{circle:?}");
        }
        assert_eq!(seen, 64, "both rims, whichever of their edges is picked");
        let cube = crate::test_util::cube([0.0; 3], 2.0);
        let edges = crate::edges::feature_edges(&cube, 0.5);
        assert!((0..edges.len()).all(|picked| circular_edge(&cube, &edges, picked, 1e-3).is_none()));
        assert!(circular_edge(&cube, &edges, usize::MAX, 1e-3).is_none());
    }
}
