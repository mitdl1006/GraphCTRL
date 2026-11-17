//! The validated, immutable robot model.

use std::collections::BTreeMap;

use gc_math::{DPose3, DVec3, pose_from_xyz_rpy};

use crate::urdf::{Diagnostic, RawGeometry, RawJoint, RawRobot, RawShape, Severity, read};

/// How a joint moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind {
    /// No motion.
    Fixed,
    /// Rotation about the axis within limits.
    Revolute,
    /// Rotation about the axis without limits.
    Continuous,
    /// Translation along the axis within limits.
    Prismatic,
}

impl JointKind {
    /// True for every kind except [`JointKind::Fixed`].
    pub fn is_movable(self) -> bool {
        self != Self::Fixed
    }
}

/// A geometric primitive or mesh of a link.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A box with the given edge lengths, centred.
    Box([f64; 3]),
    /// A cylinder along its local Z axis, centred.
    Cylinder {
        /// Radius.
        radius: f64,
        /// Length along Z.
        length: f64,
    },
    /// A sphere, centred.
    Sphere(f64),
    /// A mesh file.
    Mesh {
        /// Where the file is (`package://...`, `file://...` or a relative path).
        uri: String,
        /// Scale per axis.
        scale: [f64; 3],
    },
}

/// A shape placed in a link's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// Pose of the shape in the link frame.
    pub pose: DPose3,
    /// The shape.
    pub shape: Shape,
}

/// A rigid body of the robot.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// Name from the file.
    pub name: String,
    /// The joint that connects this link to its parent; `None` for the root.
    pub parent_joint: Option<usize>,
    /// What is drawn.
    pub visuals: Vec<Geometry>,
    /// What is checked for collisions.
    pub collisions: Vec<Geometry>,
}

/// A connection between two links.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    /// Name from the file.
    pub name: String,
    /// How it moves.
    pub kind: JointKind,
    /// Index of the parent link.
    pub parent: usize,
    /// Index of the child link.
    pub child: usize,
    /// The joint frame in the parent link's frame.
    pub origin: DPose3,
    /// Unit axis of motion in the joint frame.
    pub axis: DVec3,
    /// Lower position limit (`-inf` for continuous joints).
    pub lower: f64,
    /// Upper position limit (`+inf` for continuous joints).
    pub upper: f64,
    /// Speed limit, when the file gives a positive one.
    pub max_vel: Option<f64>,
    /// `(master joint, multiplier, offset)` when this joint copies another.
    pub mimic: Option<(usize, f64, f64)>,
}

impl Joint {
    /// The transform from the parent link frame to the child link frame at joint value `q`.
    pub fn transform(&self, q: f64) -> DPose3 {
        let motion = match self.kind {
            JointKind::Fixed => DPose3::IDENTITY,
            JointKind::Revolute | JointKind::Continuous => DPose3::new(DVec3::ZERO, self.axis * q),
            JointKind::Prismatic => DPose3::from_translation(self.axis * q),
        };
        self.origin * motion
    }
}

/// A robot: links connected by joints into a tree. Immutable and shareable between threads.
#[derive(Clone, Debug, PartialEq)]
pub struct RobotModel {
    /// Name from the file.
    pub name: String,
    /// The links. Index 0 is not special; see [`RobotModel::root`].
    pub links: Vec<Link>,
    /// The joints, ordered so that a joint's parent link is reached before the joint itself.
    pub joints: Vec<Joint>,
    /// Index of the root link.
    pub root: usize,
}

fn finding(severity: Severity, at: (u32, u32), message: String) -> Diagnostic {
    Diagnostic { severity, message, line: at.0, col: at.1 }
}

fn geometry(raw: &RawGeometry) -> Geometry {
    let shape = match &raw.shape {
        RawShape::Box(size) => Shape::Box(*size),
        RawShape::Cylinder { radius, length } => Shape::Cylinder { radius: *radius, length: *length },
        RawShape::Sphere(radius) => Shape::Sphere(*radius),
        RawShape::Mesh { uri, scale } => Shape::Mesh { uri: uri.clone(), scale: *scale },
    };
    Geometry { pose: pose_from_xyz_rpy(raw.origin.xyz, raw.origin.rpy), shape }
}

/// Reads and validates URDF text.
///
/// Returns the model with its warnings, or every finding (errors and warnings) when the file cannot be
/// used.
pub fn import(text: &str) -> Result<(RobotModel, Vec<Diagnostic>), Vec<Diagnostic>> {
    let (raw, mut findings) = read(text);
    let Some(raw) = raw else { return Err(findings) };
    let model = build(&raw, &mut findings);
    match model {
        Some(model) if findings.iter().all(|f| f.severity == Severity::Warning) => Ok((model, findings)),
        _ => Err(findings),
    }
}

fn build(raw: &RawRobot, findings: &mut Vec<Diagnostic>) -> Option<RobotModel> {
    let mut error = |at: (u32, u32), message: String| findings.push(finding(Severity::Error, at, message));
    let mut warnings = Vec::new();
    let mut warn = |at: (u32, u32), message: String| warnings.push(finding(Severity::Warning, at, message));

    let mut link_index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, link) in raw.links.iter().enumerate() {
        if link_index.insert(&link.name, i).is_some() {
            error(link.at, format!("link `{}` is defined twice", link.name));
        }
        if link.collisions.is_empty() && !link.visuals.is_empty() {
            warn(link.at, format!("link `{}` has visual geometry but no collision geometry", link.name));
        }
    }
    let mut joint_index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, joint) in raw.joints.iter().enumerate() {
        if joint_index.insert(&joint.name, i).is_some() {
            error(joint.at, format!("joint `{}` is defined twice", joint.name));
        }
    }

    let mut links: Vec<Link> = raw
        .links
        .iter()
        .map(|l| Link {
            name: l.name.clone(),
            parent_joint: None,
            visuals: l.visuals.iter().map(geometry).collect(),
            collisions: l.collisions.iter().map(geometry).collect(),
        })
        .collect();

    let mut joints = Vec::with_capacity(raw.joints.len());
    for (i, joint) in raw.joints.iter().enumerate() {
        let built = build_joint(joint, &link_index, &joint_index, &mut error, &mut warn);
        if let Some(built) = &built
            && links[built.child].parent_joint.replace(i).is_some()
        {
            error(joint.at, format!("link `{}` has more than one parent joint", joint.child));
        }
        joints.push(built);
    }
    let roots: Vec<usize> = (0..links.len()).filter(|i| links[*i].parent_joint.is_none()).collect();
    if raw.links.is_empty() {
        error((0, 0), "the robot has no links".to_owned());
    } else if roots.len() != 1 {
        let names: Vec<&str> = roots.iter().map(|i| links[*i].name.as_str()).collect();
        error((0, 0), format!("the robot must have exactly one root link, found {}: {names:?}", roots.len()));
    }
    let joints: Vec<Joint> = joints.into_iter().collect::<Option<_>>()?;
    let root = *roots.first()?;
    if roots.len() != 1 {
        return None;
    }

    // Every link must hang off the root; otherwise there is a cycle somewhere.
    let mut depth_of_link = vec![None; links.len()];
    depth_of_link[root] = Some(0_usize);
    let mut changed = true;
    while changed {
        changed = false;
        for joint in &joints {
            if let (Some(depth), None) = (depth_of_link[joint.parent], depth_of_link[joint.child]) {
                depth_of_link[joint.child] = Some(depth + 1);
                changed = true;
            }
        }
    }
    if let Some(lost) = depth_of_link.iter().position(Option::is_none) {
        error(
            (0, 0),
            format!("link `{}` is not connected to the root (the joints form a loop)", links[lost].name),
        );
        return None;
    }

    // Mimic chains must end at a real, movable joint.
    for (i, joint) in joints.iter().enumerate() {
        let mut at = i;
        for _ in 0..=joints.len() {
            match joints[at].mimic {
                Some((master, ..)) => at = master,
                None => break,
            }
        }
        if joints[at].mimic.is_some() {
            error(raw.joints[i].at, format!("mimic joints form a loop at `{}`", joint.name));
        } else if joint.mimic.is_some() && !joints[at].kind.is_movable() {
            error(
                raw.joints[i].at,
                format!("joint `{}` mimics the fixed joint `{}`", joint.name, joints[at].name),
            );
        }
    }

    // Order the joints from the root outwards and remap every index that refers to a joint.
    let mut order: Vec<usize> = (0..joints.len()).collect();
    order.sort_by_key(|i| (depth_of_link[joints[*i].parent], *i));
    let mut new_index = vec![0; joints.len()];
    for (new, old) in order.iter().enumerate() {
        new_index[*old] = new;
    }
    let mut sorted: Vec<Joint> = order.iter().map(|old| joints[*old].clone()).collect();
    for joint in &mut sorted {
        if let Some((master, ..)) = &mut joint.mimic {
            *master = new_index[*master];
        }
    }
    for link in &mut links {
        if let Some(parent_joint) = &mut link.parent_joint {
            *parent_joint = new_index[*parent_joint];
        }
    }
    findings.extend(warnings);
    Some(RobotModel { name: raw.name.clone(), links, joints: sorted, root })
}

fn build_joint(
    raw: &RawJoint,
    link_index: &BTreeMap<&str, usize>,
    joint_index: &BTreeMap<&str, usize>,
    error: &mut impl FnMut((u32, u32), String),
    warn: &mut impl FnMut((u32, u32), String),
) -> Option<Joint> {
    let name = &raw.name;
    let kind = match raw.kind.as_str() {
        "fixed" => JointKind::Fixed,
        "revolute" => JointKind::Revolute,
        "continuous" => JointKind::Continuous,
        "prismatic" => JointKind::Prismatic,
        other => {
            error(
                raw.at,
                format!(
                    "joint `{name}` has type `{other}`, which is not supported (fixed, revolute, continuous, prismatic)"
                ),
            );
            return None;
        }
    };
    let mut link = |which: &str, link_name: &str| match link_index.get(link_name) {
        Some(index) => Some(*index),
        None => {
            error(raw.at, format!("joint `{name}` refers to the unknown {which} link `{link_name}`"));
            None
        }
    };
    let (parent, child) = (link("parent", &raw.parent), link("child", &raw.child));
    if raw.origin.rpy.iter().any(|a| a.abs() > 2.0 * std::f64::consts::TAU) {
        warn(raw.at, format!("joint `{name}` has rpy values above 4*pi; rpy must be in radians"));
    }

    let mut axis = DVec3::X;
    if let Some(given) = raw.axis {
        let given = DVec3::from_array(given);
        match given.try_normalize() {
            Some(unit) => {
                if (given.length() - 1.0).abs() > 1e-6 && kind.is_movable() {
                    warn(
                        raw.at,
                        format!("the axis of joint `{name}` is not a unit vector; it was normalised"),
                    );
                }
                axis = unit;
            }
            None if kind.is_movable() => {
                error(raw.at, format!("the axis of joint `{name}` is zero"));
                return None;
            }
            None => {}
        }
    }

    let limit = raw.limit.unwrap_or_default();
    let (lower, upper) = match kind {
        JointKind::Fixed => (0.0, 0.0),
        JointKind::Continuous => {
            if limit.lower.is_some() || limit.upper.is_some() {
                warn(raw.at, format!("continuous joint `{name}` has position limits; they are ignored"));
            }
            (f64::NEG_INFINITY, f64::INFINITY)
        }
        JointKind::Revolute | JointKind::Prismatic => {
            if raw.limit.is_none() {
                error(raw.at, format!("joint `{name}` needs a <limit>"));
                return None;
            }
            // In URDF a missing bound means 0.
            let (lower, upper) = (limit.lower.unwrap_or(0.0), limit.upper.unwrap_or(0.0));
            if lower > upper {
                error(raw.at, format!("joint `{name}` has lower limit {lower} above upper limit {upper}"));
                return None;
            }
            if kind == JointKind::Revolute
                && (lower < -std::f64::consts::TAU - 1e-9 || upper > std::f64::consts::TAU + 1e-9)
            {
                warn(raw.at, format!("revolute joint `{name}` has limits wider than one turn each way"));
            }
            (lower, upper)
        }
    };
    let max_vel = limit.velocity.filter(|v| *v > 0.0);
    if kind.is_movable() && max_vel.is_none() && raw.mimic.is_none() {
        warn(raw.at, format!("joint `{name}` has no velocity limit; motions through it cannot be timed"));
    }
    let mimic = match &raw.mimic {
        Some((master, multiplier, offset)) => match joint_index.get(master.as_str()) {
            Some(index) => Some((*index, *multiplier, *offset)),
            None => {
                error(raw.at, format!("joint `{name}` mimics the unknown joint `{master}`"));
                return None;
            }
        },
        None => None,
    };
    Some(Joint {
        name: name.clone(),
        kind,
        parent: parent?,
        child: child?,
        origin: pose_from_xyz_rpy(raw.origin.xyz, raw.origin.rpy),
        axis,
        lower,
        upper,
        max_vel,
        mimic,
    })
}

impl RobotModel {
    /// Index of the link with this name.
    pub fn link(&self, name: &str) -> Option<usize> {
        self.links.iter().position(|l| l.name == name)
    }

    /// Index of the joint with this name.
    pub fn joint(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    /// The value of every joint for the given values of the independent joints: mimic joints follow
    /// their masters. `values` has one entry per joint; entries of mimic and fixed joints are ignored.
    pub fn resolve_mimics(&self, values: &[f64]) -> Vec<f64> {
        let mut resolved: Vec<f64> =
            (0..self.joints.len()).map(|i| values.get(i).copied().unwrap_or(0.0)).collect();
        for i in 0..self.joints.len() {
            // Follow the chain of masters; validation guarantees it ends.
            let (mut at, mut scale, mut shift) = (i, 1.0, 0.0);
            while let Some((master, multiplier, offset)) = self.joints[at].mimic {
                shift += scale * offset;
                scale *= multiplier;
                at = master;
            }
            if at != i {
                resolved[i] = scale * values.get(at).copied().unwrap_or(0.0) + shift;
            }
        }
        resolved
    }

    /// The pose of every link in the root link's frame for the given joint values (one per joint; mimic
    /// joints are resolved here).
    pub fn link_poses(&self, values: &[f64], out: &mut Vec<DPose3>) {
        let resolved = self.resolve_mimics(values);
        out.clear();
        out.resize(self.links.len(), DPose3::IDENTITY);
        // Joints are ordered from the root outwards, so a parent's pose is final before it is used.
        for (joint, value) in self.joints.iter().zip(&resolved) {
            out[joint.child] = out[joint.parent] * joint.transform(*value);
        }
    }
}
