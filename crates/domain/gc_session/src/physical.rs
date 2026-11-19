//! iProperties of the document and components, and the Physical tab's values worked out from the shapes
//! (Inventor 2026 help, iProperties - Physical Tab - Reference, GUID-877564AC-78CE-4216-9913-003A2DE8D698:
//! "Calculates mass, surface area, and volume … Center of Gravity: the x, y, z coordinates of the center of
//! gravity of the selected component, relative to the assembly origin").

use gc_doc::{CellOp, Command, EditError, IProperties, Occurrence, Solid};
use gc_math::{DMat3, DPose3, DVec3};
use gc_robot::Shape;

use crate::Session;

/// The Physical tab's values for a component or the whole document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalProperties {
    /// Mass, kg (the override when one is set).
    pub mass: f64,
    /// Volume, m³.
    pub volume: f64,
    /// Surface area, m².
    pub area: f64,
    /// Centre of gravity in the world (the assembly origin's frame), m.
    pub centre: DVec3,
    /// The density used, kg/m³ (for the document, the mass over the volume).
    pub density: f64,
    /// True when the mass is an override (or, for the document, includes one).
    pub overridden: bool,
    /// True when a mesh in it is not a closed solid (an edge without exactly two faces, as in an STL with
    /// faces missing): its volume, and with it the mass and the centre of gravity, are then not to be
    /// trusted; the area is.
    pub open: bool,
}

/// Volume, area and the volume's centroid of a shape in its own frame.
#[derive(Clone, Copy, Debug)]
struct Measure {
    volume: f64,
    area: f64,
    /// Volume times centroid (so measures add).
    moment: DVec3,
    /// A mesh in it is not closed.
    open: bool,
    /// The second moment of the volume about the frame's origin, ∫ x xᵀ dV (so measures add; the
    /// inertia tensor of a uniform solid is density × (trace · 1 − this)).
    second: DMat3,
}

impl Default for Measure {
    /// Nothing: no volume, no area, and a second moment of zero (a matrix's own default is the identity).
    fn default() -> Self {
        Self { volume: 0.0, area: 0.0, moment: DVec3::ZERO, open: false, second: DMat3::ZERO }
    }
}

/// The outer product a bᵀ.
fn outer(a: DVec3, b: DVec3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

impl Measure {
    fn moved(self, pose: &DPose3) -> Measure {
        let centroid = if self.volume > 0.0 { self.moment / self.volume } else { DVec3::ZERO };
        // x′ = R x + t: ∫ x′ x′ᵀ = R C Rᵀ + (R m) tᵀ + t (R m)ᵀ + V t tᵀ.
        let r = DMat3::from_quat(pose.rotation);
        let (m, t) = (r * self.moment, pose.translation);
        let second = r * self.second * r.transpose() + outer(m, t) + outer(t, m) + outer(t, t) * self.volume;
        Measure { moment: pose.transform_point(centroid) * self.volume, second, ..self }
    }

    fn plus(self, other: Measure) -> Measure {
        Measure {
            volume: self.volume + other.volume,
            open: self.open || other.open,
            area: self.area + other.area,
            moment: self.moment + other.moment,
            second: self.second + other.second,
        }
    }
}

/// Moments of inertia (Ixx, Iyy, Izz) and products of inertia (Ixy, Ixz, Iyz), kg·m². The products are
/// the positive integrals (∫ x y dm); Inventor shows them negated when its Negative integral option is on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MassMoments {
    /// About the X axis.
    pub ixx: f64,
    /// About the Y axis.
    pub iyy: f64,
    /// About the Z axis.
    pub izz: f64,
    /// ∫ x y dm.
    pub ixy: f64,
    /// ∫ x z dm.
    pub ixz: f64,
    /// ∫ y z dm.
    pub iyz: f64,
}

/// The Physical tab's Inertial Properties (Inventor 2026 help, iProperties - Physical Tab - Reference,
/// GUID-877564AC-78CE-4216-9913-003A2DE8D698: Principal, Global, COG).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InertialProperties {
    /// Global: about the assembly origin, along its axes.
    pub global: MassMoments,
    /// COG: about the centre of gravity, along the assembly's axes.
    pub cog: MassMoments,
    /// Principal Moments: about the principal axes of inertia through the centre of gravity (I1, I2, I3).
    pub principal: [f64; 3],
    /// Rotation to principal: roll, pitch, yaw from the assembly's axes to the principal axes, radians.
    pub rotation_to_principal: [f64; 3],
}

/// Moments and products of inertia from a mass-weighted second moment (∫ x xᵀ dm).
fn mass_moments(second: &DMat3) -> MassMoments {
    let (x, y, z) = (second.x_axis, second.y_axis, second.z_axis);
    MassMoments { ixx: y.y + z.z, iyy: x.x + z.z, izz: x.x + y.y, ixy: x.y, ixz: x.z, iyz: y.z }
}

/// The eigenvalues and eigenvectors (as columns, a right-handed set) of a symmetric matrix, by Jacobi
/// rotations.
fn principal_axes(matrix: &DMat3) -> ([f64; 3], DMat3) {
    let mut a = matrix.to_cols_array_2d();
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..64 {
        // The largest off-diagonal entry is turned to zero.
        let (p, q) = [(0, 1), (0, 2), (1, 2)]
            .into_iter()
            .max_by(|x, y| a[x.0][x.1].abs().total_cmp(&a[y.0][y.1].abs()))
            .unwrap_or((0, 1));
        let scale = a[0][0].abs().max(a[1][1].abs()).max(a[2][2].abs()).max(f64::MIN_POSITIVE);
        if a[p][q].abs() <= 1e-15 * scale {
            break;
        }
        let theta = 0.5 * gc_math::fm::atan2(2.0 * a[p][q], a[q][q] - a[p][p]);
        let (sin, cos) = gc_math::fm::sin_cos(theta);
        for row in &mut a {
            let (x, y) = (row[p], row[q]);
            row[p] = cos * x - sin * y;
            row[q] = sin * x + cos * y;
        }
        let (row_p, row_q) = (a[p], a[q]);
        for k in 0..3 {
            a[p][k] = cos * row_p[k] - sin * row_q[k];
            a[q][k] = sin * row_p[k] + cos * row_q[k];
        }
        for row in &mut v {
            let (x, y) = (row[p], row[q]);
            row[p] = cos * x - sin * y;
            row[q] = sin * x + cos * y;
        }
    }
    let column = |k: usize| DVec3::new(v[0][k], v[1][k], v[2][k]);
    let (x, y) = (column(0), column(1));
    // The third axis completes a right-handed set (an eigenvector's sign is free).
    ([a[0][0], a[1][1], a[2][2]], DMat3::from_cols(x, y, x.cross(y)))
}

fn primitive(shape: &Shape) -> Option<Measure> {
    use std::f64::consts::PI;
    Some(match *shape {
        Shape::Box([x, y, z]) => {
            let volume = x * y * z;
            Measure {
                volume,
                area: 2.0 * (x * y + y * z + z * x),
                moment: DVec3::ZERO,
                open: false,
                second: DMat3::from_diagonal(DVec3::new(x * x, y * y, z * z) * (volume / 12.0)),
            }
        }
        Shape::Cylinder { radius: r, length: l } => {
            let volume = PI * r * r * l;
            Measure {
                volume,
                area: 2.0 * PI * r * (r + l),
                moment: DVec3::ZERO,
                open: false,
                second: DMat3::from_diagonal(DVec3::new(r * r / 4.0, r * r / 4.0, l * l / 12.0) * volume),
            }
        }
        Shape::Sphere(r) => {
            let volume = 4.0 / 3.0 * PI * r * r * r;
            Measure {
                volume,
                area: 4.0 * PI * r * r,
                moment: DVec3::ZERO,
                open: false,
                second: DMat3::from_diagonal(DVec3::splat(r * r / 5.0 * volume)),
            }
        }
        Shape::Mesh { .. } => return None,
    })
}

/// True when every edge of a mesh has exactly two faces (corners matched by position, since STL files
/// repeat them per face): the mesh then encloses a volume.
fn closed(data: &gc_mesh::MeshData) -> bool {
    let p = data.positions();
    let key = |i: u32| p.get(i as usize).map(|v| v.map(f32::to_bits));
    let mut edges: std::collections::HashMap<([u32; 3], [u32; 3]), u32> = std::collections::HashMap::new();
    for tri in data.indices().chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (key(tri[0]), key(tri[1]), key(tri[2])) else { return false };
        for (from, to) in [(a, b), (b, c), (c, a)] {
            // A face of no size (two corners at one place) has no edge there.
            if from != to {
                *edges.entry(if from < to { (from, to) } else { (to, from) }).or_insert(0) += 1;
            }
        }
    }
    !edges.is_empty() && edges.values().all(|faces| *faces == 2)
}

/// A closed mesh's volume (by the divergence theorem: signed tetrahedra from the origin), area and centroid.
fn mesh(data: &gc_mesh::MeshData) -> Measure {
    let p = data.positions();
    let mut m = Measure::default();
    for tri in data.indices().chunks_exact(3) {
        let at =
            |i: u32| p.get(i as usize).map(|v| DVec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2])));
        let (Some(a), Some(b), Some(c)) = (at(tri[0]), at(tri[1]), at(tri[2])) else { continue };
        let v = a.dot(b.cross(c)) / 6.0;
        m.volume += v;
        m.moment += (a + b + c) / 4.0 * v;
        // A tetrahedron from the origin: ∫ x xᵀ dV = V/20 · (a aᵀ + b bᵀ + c cᵀ + (a+b+c)(a+b+c)ᵀ).
        let sum = a + b + c;
        m.second += (outer(a, a) + outer(b, b) + outer(c, c) + outer(sum, sum)) * (v / 20.0);
        m.area += (b - a).cross(c - a).length() / 2.0;
    }
    m.open = !closed(data);
    // A mesh wound inside out gives a negative volume; the shape is the same.
    if m.volume < 0.0 {
        m.volume = -m.volume;
        m.moment = -m.moment;
        m.second = -m.second;
    }
    m
}

impl Session {
    /// The iProperties of a component, or of the document (`None`).
    pub fn properties(&self, target: Option<Occurrence>) -> IProperties {
        self.doc().properties(target)
    }

    /// Replaces the iProperties of a component, or of the document (`None`). One undo step; saved.
    pub fn set_properties(
        &mut self,
        target: Option<Occurrence>,
        properties: IProperties,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(CellOp::SetProperties { target, properties })).map(|_| ())
    }

    /// The Part Number shown for a component: its own, else its name ("If you do not enter a part number,
    /// the file name is automatically assigned").
    pub fn part_number(&self, target: Occurrence) -> String {
        let own = self.properties(Some(target)).part_number;
        if !own.is_empty() {
            return own;
        }
        let key = gc_doc::TreeKey::Component(target);
        self.component_tree().into_iter().find(|row| row.key == key).map(|row| row.name).unwrap_or_default()
    }

    /// Volume, area and centroid of a body (a primitive or a mesh that has been read), in the world.
    fn body_measure(&self, body: gc_core::Id<gc_doc::Body>) -> Option<Measure> {
        let shown = self.shown_body(body)?;
        let local = match &shown.solid {
            Solid::Box { size } => primitive(&Shape::Box(*size))?,
            Solid::Cylinder { radius, length } => {
                primitive(&Shape::Cylinder { radius: *radius, length: *length })?
            }
            Solid::Sphere { radius } => primitive(&Shape::Sphere(*radius))?,
            Solid::Mesh { .. } => mesh(&self.body_mesh(&shown.solid)?.data),
        };
        Some(local.moved(&gc_math::pose_from_xyz_rpy(shown.placement.xyz, shown.placement.rpy)))
    }

    /// Volume, area and centroid of a robot's links (their visual shapes as shown), in the world.
    fn robot_measure(&self, robot: gc_core::Id<gc_doc::Robot>) -> Option<Measure> {
        let loaded = self.robot_model(robot)?;
        let poses = self.link_poses(robot)?;
        let mut total = Measure::default();
        for (link, pose) in loaded.model.links.iter().zip(&poses) {
            for geometry in &link.visuals {
                let local = match &geometry.shape {
                    Shape::Mesh { uri, scale } => loaded
                        .mesh(uri, *scale)
                        .iter()
                        .fold(Measure::default(), |sum, (part, _)| sum.plus(mesh(&part.data))),
                    other => primitive(other)?,
                };
                total = total.plus(local.moved(&(*pose * geometry.pose)));
            }
        }
        Some(total)
    }

    /// The Physical tab of a component placed in the cell (a body or robot), or of the whole document
    /// (`None`: every body and robot, each with its own density or override). Mass is density × volume
    /// unless overridden. `None` for a component whose shape is not known (a mesh still being read, an
    /// imported assembly).
    pub fn physical_properties(&self, target: Option<Occurrence>) -> Option<PhysicalProperties> {
        self.physical_properties_with(target, false)
    }

    /// [`Session::physical_properties`] with the Physical tab's Include QTY Overrides (Inventor 2026
    /// help, iProperties - Physical Tab - Reference, GUID-877564AC-78CE-4216-9913-003A2DE8D698: "Mass,
    /// Volume, and Area includes contributions from … BOM Quantity overrides when the … Include QTY
    /// Overrides check [box is] selected"; "If static quantities are created, the center of gravity …
    /// may not be accurate"). For the whole document, the components of a BOM row with a Static
    /// Quantity count as that many instead of as many as there are.
    pub fn physical_properties_with(
        &self,
        target: Option<Occurrence>,
        include_quantity_overrides: bool,
    ) -> Option<PhysicalProperties> {
        self.physical(target, include_quantity_overrides).map(|(properties, _)| properties)
    }

    /// The Physical tab's Inertial Properties of a component or of the whole document (`None`), with
    /// respect to the assembly's coordinate system: Global (about its origin), COG (about the centre of
    /// gravity) and Principal (the principal moments and the rotation to the principal axes). Each
    /// component is taken as uniform, with its mass (density × volume, or its override) spread over its
    /// volume. BOM Quantity overrides are not included, as in Inventor. `None` when the shape is not
    /// known or has no mass.
    pub fn inertial_properties(&self, target: Option<Occurrence>) -> Option<InertialProperties> {
        let (properties, second) = self.physical(target, false)?;
        if properties.mass.is_nan() || properties.mass <= 0.0 {
            return None;
        }
        let c = properties.centre;
        let about_cog = second - outer(c, c) * properties.mass;
        let cog = mass_moments(&about_cog);
        // The inertia tensor about the centre of gravity: trace · 1 − ∫ x xᵀ dm.
        let trace = about_cog.x_axis.x + about_cog.y_axis.y + about_cog.z_axis.z;
        let tensor = DMat3::from_diagonal(DVec3::splat(trace)) - about_cog;
        let (principal, axes) = principal_axes(&tensor);
        let rotation_to_principal = gc_math::rpy_from_quat(gc_math::DQuat::from_mat3(&axes).normalize());
        Some(InertialProperties { global: mass_moments(&second), cog, principal, rotation_to_principal })
    }

    /// The physical properties with the mass-weighted second moment about the assembly origin
    /// (∫ x xᵀ dm), from which the inertial properties come.
    fn physical(
        &self,
        target: Option<Occurrence>,
        include_quantity_overrides: bool,
    ) -> Option<(PhysicalProperties, DMat3)> {
        // How much each component counts: the row's static quantity shared over its components.
        let mut share: std::collections::BTreeMap<gc_core::AnyId, f64> = std::collections::BTreeMap::new();
        if include_quantity_overrides && target.is_none() {
            for row in self.bill_of_materials(crate::BomView::PartsOnly).iter().filter(|r| r.static_quantity)
            {
                let items: Vec<gc_core::AnyId> = row
                    .keys
                    .iter()
                    .filter_map(|key| match key {
                        gc_doc::TreeKey::Component(Occurrence::Item(item)) => Some(*item),
                        _ => None,
                    })
                    .collect();
                #[allow(clippy::cast_precision_loss, reason = "component counts are small")]
                let each = f64::from(row.quantity) / items.len().max(1) as f64;
                share.extend(items.into_iter().map(|item| (item, each)));
            }
        }
        let of = |item: gc_core::AnyId| -> Option<(Measure, IProperties)> {
            let measure = match item.kind {
                gc_core::Kind::Body => self.body_measure(gc_core::Id::from_raw(item.raw))?,
                gc_core::Kind::RobotInstance => self.robot_measure(gc_core::Id::from_raw(item.raw))?,
                _ => return None,
            };
            let mut properties = self.properties(Some(Occurrence::Item(item)));
            let measure = match share.get(&item) {
                Some(factor) => {
                    properties.mass_override = properties.mass_override.map(|m| m * factor);
                    Measure {
                        volume: measure.volume * factor,
                        area: measure.area * factor,
                        moment: measure.moment * *factor,
                        second: measure.second * *factor,
                        ..measure
                    }
                }
                None => measure,
            };
            Some((measure, properties))
        };
        let parts: Vec<(Measure, IProperties)> = match target {
            Some(Occurrence::Item(item)) => vec![of(item)?],
            Some(Occurrence::Part { .. } | Occurrence::Link { .. } | Occurrence::Tool { .. }) => return None,
            None => {
                let doc = self.doc();
                doc.bodies()
                    .iter()
                    .map(|(id, _)| id.erase())
                    .chain(doc.robots().iter().map(|(id, _)| id.erase()))
                    // Reference components are "excluded from mass and volume calculations" (About the BOM
                    // Structure, GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF).
                    .filter(|item| !self.bom_reference(*item))
                    .filter_map(of)
                    .collect()
            }
        };
        let (mut mass, mut weighted, mut total, mut overridden) =
            (0.0, DVec3::ZERO, Measure::default(), false);
        let mut second = DMat3::ZERO;
        for (measure, props) in &parts {
            let m = match props.mass_override {
                Some(m) => {
                    overridden = true;
                    m
                }
                None => props.density_or_default() * measure.volume,
            };
            let centroid = if measure.volume > 0.0 { measure.moment / measure.volume } else { DVec3::ZERO };
            mass += m;
            weighted += centroid * m;
            // The component's mass spread evenly over its volume.
            if measure.volume > 0.0 {
                second += measure.second * (m / measure.volume);
            }
            total = total.plus(*measure);
        }
        let centre = if mass > 0.0 {
            weighted / mass
        } else if total.volume > 0.0 {
            total.moment / total.volume
        } else {
            DVec3::ZERO
        };
        let density = if total.volume > 0.0 { mass / total.volume } else { gc_doc::DEFAULT_DENSITY };
        let properties = PhysicalProperties {
            mass,
            volume: total.volume,
            area: total.area,
            centre,
            density,
            overridden,
            open: total.open,
        };
        Some((properties, second))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box, a cylinder and a sphere as meshes have the second moments of the solids they stand for.
    #[test]
    fn mesh_second_moments_match_the_solids() {
        let close = |a: &DMat3, b: &DMat3, tolerance: f64| {
            a.to_cols_array().iter().zip(b.to_cols_array()).all(|(x, y)| (x - y).abs() <= tolerance)
        };
        let cuboid = gc_mesh::primitives::cuboid([0.1, 0.2, 0.3]).unwrap();
        let solid = primitive(&Shape::Box([0.1, 0.2, 0.3])).unwrap();
        let meshed = mesh(&cuboid);
        assert!((meshed.volume - solid.volume).abs() < 1e-9);
        assert!(close(&meshed.second, &solid.second, 1e-9), "{:?} vs {:?}", meshed.second, solid.second);
        // Round shapes are faceted: close, not equal.
        let ball = mesh(&gc_mesh::primitives::sphere(0.1, 64).unwrap());
        let sphere = primitive(&Shape::Sphere(0.1)).unwrap();
        assert!(close(&ball.second, &sphere.second, 0.02 * sphere.second.x_axis.x));
        // Moved: the parallel axis theorem.
        let pose = DPose3::from_parts(DVec3::new(1.0, 0.0, 0.0), gc_math::DQuat::from_rotation_z(0.7));
        let moved = solid.moved(&pose);
        let trace = |m: &DMat3| m.x_axis.x + m.y_axis.y + m.z_axis.z;
        assert!((trace(&moved.second) - trace(&solid.second) - solid.volume).abs() < 1e-12);
        // Principal axes of a tensor with known axes.
        let turn = DMat3::from_quat(gc_math::DQuat::from_rotation_z(0.4));
        let tensor = turn * DMat3::from_diagonal(DVec3::new(3.0, 2.0, 1.0)) * turn.transpose();
        let (values, axes) = principal_axes(&tensor);
        let mut sorted = values;
        sorted.sort_by(f64::total_cmp);
        assert!(sorted.iter().zip([1.0, 2.0, 3.0]).all(|(a, b)| (a - b).abs() < 1e-12), "{values:?}");
        let rebuilt = axes * DMat3::from_diagonal(DVec3::from_array(values)) * axes.transpose();
        assert!(close(&rebuilt, &tensor, 1e-12));
        assert!((axes.determinant() - 1.0).abs() < 1e-12);
    }
}
