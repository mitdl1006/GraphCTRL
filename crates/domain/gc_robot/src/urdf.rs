//! Reading URDF text into a faithful intermediate form.
//!
//! "Faithful" means that what the file does not say stays unsaid (`Option`), so that validation can tell a
//! missing limit from a limit of zero.

use roxmltree::{Document, Node};

/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The model cannot be used.
    Error,
    /// The model is usable, but something looks wrong.
    Warning,
}

/// Something the reader or the validation found, with its place in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// What was found.
    pub message: String,
    /// Line in the file, starting at 1 (0 when not tied to a place).
    pub line: u32,
    /// Column in the line, starting at 1.
    pub col: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Origin {
    pub xyz: [f64; 3],
    pub rpy: [f64; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RawShape {
    Box([f64; 3]),
    Cylinder { radius: f64, length: f64 },
    Sphere(f64),
    Mesh { uri: String, scale: [f64; 3] },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RawGeometry {
    pub origin: Origin,
    pub shape: RawShape,
}

#[derive(Clone, Debug)]
pub(crate) struct RawLink {
    pub name: String,
    pub visuals: Vec<RawGeometry>,
    pub collisions: Vec<RawGeometry>,
    pub at: (u32, u32),
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RawLimit {
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub velocity: Option<f64>,
}

#[derive(Clone, Debug)]
pub(crate) struct RawJoint {
    pub name: String,
    pub kind: String,
    pub parent: String,
    pub child: String,
    pub origin: Origin,
    pub axis: Option<[f64; 3]>,
    pub limit: Option<RawLimit>,
    /// `(master joint, multiplier, offset)`.
    pub mimic: Option<(String, f64, f64)>,
    pub at: (u32, u32),
}

#[derive(Clone, Debug)]
pub(crate) struct RawRobot {
    pub name: String,
    pub links: Vec<RawLink>,
    pub joints: Vec<RawJoint>,
}

struct Reader<'a, 'input> {
    doc: &'a Document<'input>,
    findings: Vec<Diagnostic>,
}

impl Reader<'_, '_> {
    fn at(&self, node: Node<'_, '_>) -> (u32, u32) {
        let pos = self.doc.text_pos_at(node.range().start);
        (pos.row, pos.col)
    }

    fn error(&mut self, node: Node<'_, '_>, message: String) {
        let (line, col) = self.at(node);
        self.findings.push(Diagnostic { severity: Severity::Error, message, line, col });
    }

    /// Parses one number of an attribute; reports and returns `None` when it is not a finite number.
    fn number(&mut self, node: Node<'_, '_>, attribute: &str, text: &str) -> Option<f64> {
        match text.trim().parse::<f64>() {
            Ok(value) if value.is_finite() => Some(value),
            _ => {
                let hint = if text.contains(',') { " (use a point as decimal separator)" } else { "" };
                self.error(node, format!("`{attribute}=\"{text}\"` is not a number{hint}"));
                None
            }
        }
    }

    fn scalar(&mut self, node: Node<'_, '_>, attribute: &str) -> Option<f64> {
        let text = node.attribute(attribute)?;
        self.number(node, attribute, text)
    }

    /// A whitespace-separated triple; `None` when the attribute is absent.
    fn triple(&mut self, node: Node<'_, '_>, attribute: &str) -> Option<[f64; 3]> {
        let text = node.attribute(attribute)?;
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() != 3 {
            self.error(node, format!("`{attribute}=\"{text}\"` must have three numbers"));
            return None;
        }
        let values: Vec<f64> = parts.iter().filter_map(|part| self.number(node, attribute, part)).collect();
        <[f64; 3]>::try_from(values).ok()
    }

    fn origin(&mut self, parent: Node<'_, '_>) -> Origin {
        let mut origin = Origin { xyz: [0.0; 3], rpy: [0.0; 3] };
        if let Some(node) = child(parent, "origin") {
            origin.xyz = self.triple(node, "xyz").unwrap_or([0.0; 3]);
            origin.rpy = self.triple(node, "rpy").unwrap_or([0.0; 3]);
        }
        origin
    }

    fn geometry(&mut self, element: Node<'_, '_>) -> Option<RawGeometry> {
        let origin = self.origin(element);
        let Some(geometry) = child(element, "geometry") else {
            self.error(element, format!("<{}> has no <geometry>", element.tag_name().name()));
            return None;
        };
        let shape_node = geometry.children().find(Node::is_element)?;
        let shape = match shape_node.tag_name().name() {
            "box" => RawShape::Box(self.triple(shape_node, "size")?),
            "cylinder" => RawShape::Cylinder {
                radius: self.scalar(shape_node, "radius")?,
                length: self.scalar(shape_node, "length")?,
            },
            "sphere" => RawShape::Sphere(self.scalar(shape_node, "radius")?),
            "mesh" => {
                let Some(uri) = shape_node.attribute("filename") else {
                    self.error(shape_node, "<mesh> has no filename".to_owned());
                    return None;
                };
                let scale = self.triple(shape_node, "scale").unwrap_or([1.0; 3]);
                RawShape::Mesh { uri: uri.to_owned(), scale }
            }
            other => {
                self.error(shape_node, format!("unknown geometry <{other}>"));
                return None;
            }
        };
        Some(RawGeometry { origin, shape })
    }

    fn name(&mut self, node: Node<'_, '_>) -> Option<String> {
        match node.attribute("name") {
            Some(name) if !name.trim().is_empty() => Some(name.to_owned()),
            _ => {
                self.error(node, format!("<{}> has no name", node.tag_name().name()));
                None
            }
        }
    }

    fn link(&mut self, node: Node<'_, '_>) -> Option<RawLink> {
        let name = self.name(node)?;
        let list = |reader: &mut Self, tag: &str| {
            node.children().filter(|c| c.has_tag_name(tag)).filter_map(|c| reader.geometry(c)).collect()
        };
        let visuals = list(self, "visual");
        let collisions = list(self, "collision");
        Some(RawLink { name, visuals, collisions, at: self.at(node) })
    }

    fn joint(&mut self, node: Node<'_, '_>) -> Option<RawJoint> {
        let name = self.name(node)?;
        let Some(kind) = node.attribute("type") else {
            self.error(node, format!("joint `{name}` has no type"));
            return None;
        };
        let end = |reader: &mut Self, tag: &str| match child(node, tag).and_then(|c| c.attribute("link")) {
            Some(link) => Some(link.to_owned()),
            None => {
                reader.error(node, format!("joint `{name}` has no <{tag} link=...>"));
                None
            }
        };
        let (parent, child_link) = (end(self, "parent"), end(self, "child"));
        let origin = self.origin(node);
        let axis = child(node, "axis").and_then(|a| self.triple(a, "xyz"));
        let limit = child(node, "limit").map(|l| RawLimit {
            lower: self.scalar(l, "lower"),
            upper: self.scalar(l, "upper"),
            velocity: self.scalar(l, "velocity"),
        });
        let mimic = child(node, "mimic").and_then(|m| {
            let Some(master) = m.attribute("joint") else {
                self.error(m, format!("<mimic> of joint `{name}` names no joint"));
                return None;
            };
            let multiplier = self.scalar(m, "multiplier").unwrap_or(1.0);
            let offset = self.scalar(m, "offset").unwrap_or(0.0);
            Some((master.to_owned(), multiplier, offset))
        });
        Some(RawJoint {
            name,
            kind: kind.to_owned(),
            parent: parent?,
            child: child_link?,
            origin,
            axis,
            limit,
            mimic,
            at: self.at(node),
        })
    }
}

fn child<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|c| c.has_tag_name(tag))
}

/// Parses URDF text. Returns what could be read together with every finding; the robot is `None` when
/// the text is not a URDF document at all.
pub(crate) fn read(text: &str) -> (Option<RawRobot>, Vec<Diagnostic>) {
    let whole_file = |message: String| Diagnostic { severity: Severity::Error, message, line: 0, col: 0 };
    if text.contains("${") || text.contains("$(") {
        let message = "the file contains unexpanded xacro expressions; convert it to plain URDF first \
                       (for example with `xacro model.urdf.xacro > model.urdf`)";
        return (None, vec![whole_file(message.to_owned())]);
    }
    let doc = match Document::parse(text) {
        Ok(doc) => doc,
        Err(error) => {
            let pos = error.pos();
            let finding = Diagnostic {
                severity: Severity::Error,
                message: format!("not valid XML: {error}"),
                line: pos.row,
                col: pos.col,
            };
            return (None, vec![finding]);
        }
    };
    let root = doc.root_element();
    if !root.has_tag_name("robot") {
        return (
            None,
            vec![whole_file(format!("the root element is <{}>, not <robot>", root.tag_name().name()))],
        );
    }
    let mut reader = Reader { doc: &doc, findings: Vec::new() };
    let name = root.attribute("name").unwrap_or("robot").to_owned();
    let links = root.children().filter(|c| c.has_tag_name("link")).filter_map(|c| reader.link(c)).collect();
    let joints =
        root.children().filter(|c| c.has_tag_name("joint")).filter_map(|c| reader.joint(c)).collect();
    (Some(RawRobot { name, links, joints }), reader.findings)
}
