//! iProperties of the document and components, and the Physical tab's values worked out from the shapes
//! (Inventor 2026 help, iProperties - Physical Tab - Reference, GUID-877564AC-78CE-4216-9913-003A2DE8D698:
//! "Calculates mass, surface area, and volume … Center of Gravity: the x, y, z coordinates of the center of
//! gravity of the selected component, relative to the assembly origin").

use gc_doc::{CellOp, Command, EditError, IProperties, Occurrence, Solid};
use gc_math::{DPose3, DVec3};
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
}

/// Volume, area and the volume's centroid of a shape in its own frame.
#[derive(Clone, Copy, Debug, Default)]
struct Measure {
    volume: f64,
    area: f64,
    /// Volume times centroid (so measures add).
    moment: DVec3,
}

impl Measure {
    fn moved(self, pose: &DPose3) -> Measure {
        let centroid = if self.volume > 0.0 { self.moment / self.volume } else { DVec3::ZERO };
        Measure { moment: pose.transform_point(centroid) * self.volume, ..self }
    }

    fn plus(self, other: Measure) -> Measure {
        Measure {
            volume: self.volume + other.volume,
            area: self.area + other.area,
            moment: self.moment + other.moment,
        }
    }
}

fn primitive(shape: &Shape) -> Option<Measure> {
    use std::f64::consts::PI;
    Some(match *shape {
        Shape::Box([x, y, z]) => {
            Measure { volume: x * y * z, area: 2.0 * (x * y + y * z + z * x), moment: DVec3::ZERO }
        }
        Shape::Cylinder { radius: r, length: l } => {
            Measure { volume: PI * r * r * l, area: 2.0 * PI * r * (r + l), moment: DVec3::ZERO }
        }
        Shape::Sphere(r) => {
            Measure { volume: 4.0 / 3.0 * PI * r * r * r, area: 4.0 * PI * r * r, moment: DVec3::ZERO }
        }
        Shape::Mesh { .. } => return None,
    })
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
        m.area += (b - a).cross(c - a).length() / 2.0;
    }
    // A mesh wound inside out gives a negative volume; the shape is the same.
    if m.volume < 0.0 {
        m.volume = -m.volume;
        m.moment = -m.moment;
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
        let of = |item: gc_core::AnyId| -> Option<(Measure, IProperties)> {
            let measure = match item.kind {
                gc_core::Kind::Body => self.body_measure(gc_core::Id::from_raw(item.raw))?,
                gc_core::Kind::RobotInstance => self.robot_measure(gc_core::Id::from_raw(item.raw))?,
                _ => return None,
            };
            Some((measure, self.properties(Some(Occurrence::Item(item)))))
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
                    .filter_map(of)
                    .collect()
            }
        };
        let (mut mass, mut weighted, mut total, mut overridden) =
            (0.0, DVec3::ZERO, Measure::default(), false);
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
        Some(PhysicalProperties { mass, volume: total.volume, area: total.area, centre, density, overridden })
    }
}
