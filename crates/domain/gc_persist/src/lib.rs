//! Saving and loading documents as `.gctrl` files.
//!
//! The file is pretty-printed JSON with a format name and a version number. Everything in it is ordered,
//! so saving the same document twice gives the same bytes. Numbers round-trip exactly.
//!
//! Saving is atomic: the data is written to a temporary file next to the target, flushed to disk and
//! renamed over it, and the previous file is kept as `.bak`. A crash can leave the old file or the new
//! one, never a half-written one.
//!
//! Loading never solves anything. Sketches whose stored coordinates do not satisfy their constraints are
//! reported, not silently changed, so loading and saving again reproduces the file.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

pub mod places;

use std::collections::BTreeMap;
use std::io::Write as _;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use gc_core::Id;
use gc_doc::{
    Body, Contents, DocError, Document, Frame, FrameKind, MotionPath, Param, Robot, Sketch, Waypoint,
    WorkFrame,
};
use gc_expr::Unit;
use serde::{Deserialize, Serialize};

/// The name every `.gctrl` file carries in its header.
const FORMAT: &str = "gctrl";
/// The newest file version this build reads and the one it writes.
pub const VERSION: u32 = 1;

/// Why a document could not be saved or loaded.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    /// Reading or writing the file failed.
    #[error("{path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// The file is not valid JSON of the expected shape.
    #[error("the file is damaged or not a GraphCTRL document: {0}")]
    Format(#[from] serde_json::Error),
    /// The file is JSON but not a GraphCTRL document.
    #[error("this is not a GraphCTRL document")]
    NotGctrl,
    /// The file was written by a newer version of the application.
    #[error("the file has version {found}, but this application reads up to version {supported}")]
    TooNew {
        /// Version in the file.
        found: u32,
        /// Newest version this build reads.
        supported: u32,
    },
    /// The file's content contradicts itself.
    #[error("the document is inconsistent: {0}")]
    Invalid(#[from] DocError),
    /// The file names a unit this build does not know.
    #[error("unknown unit `{0}`")]
    UnknownUnit(String),
}

#[derive(Serialize, Deserialize)]
struct Header {
    format: String,
    version: u32,
}

#[derive(Serialize, Deserialize)]
struct ParamV1 {
    name: String,
    source: String,
    unit: String,
    value: f64,
}

#[derive(Serialize, Deserialize)]
enum FrameKindV1 {
    World,
    OriginPlane,
    User,
}

#[derive(Serialize, Deserialize)]
struct FrameV1 {
    name: String,
    kind: FrameKindV1,
    parent: Option<Id<Frame>>,
    xyz: [f64; 3],
    quat_xyzw: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<gc_doc::PlaneSource>,
}

/// Version 1 of the file.
#[derive(Serialize, Deserialize)]
struct FileV1 {
    format: String,
    version: u32,
    next_id: NonZeroU64,
    params: BTreeMap<Id<Param>, ParamV1>,
    frames: BTreeMap<Id<Frame>, FrameV1>,
    sketches: BTreeMap<Id<Sketch>, Sketch>,
    #[serde(default)]
    bodies: BTreeMap<Id<Body>, Body>,
    #[serde(default)]
    robots: BTreeMap<Id<Robot>, Robot>,
    #[serde(default)]
    waypoints: BTreeMap<Id<Waypoint>, Waypoint>,
    #[serde(default)]
    paths: BTreeMap<Id<MotionPath>, MotionPath>,
    #[serde(default)]
    work_frames: BTreeMap<Id<WorkFrame>, WorkFrame>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    work_features: BTreeMap<Id<gc_doc::WorkFeature>, gc_doc::WorkFeature>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    features: BTreeMap<Id<gc_doc::Feature>, gc_doc::Feature>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    assemblies: BTreeMap<Id<gc_doc::Assembly>, gc_doc::Assembly>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    timelines: BTreeMap<Id<gc_doc::Timeline>, gc_doc::Timeline>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    relationships: BTreeMap<Id<gc_doc::Relationship>, gc_doc::Relationship>,
}

/// A loaded document.
#[derive(Debug)]
pub struct Loaded {
    /// The document, exactly as stored.
    pub doc: Document,
    /// Sketches whose stored coordinates violate their own constraints. They are shown as they are; the
    /// user decides whether to re-solve them.
    pub needs_repair: Vec<Id<Sketch>>,
}

/// The document as the text of a `.gctrl` file.
pub fn to_text(doc: &Document) -> Result<String, PersistError> {
    let file = FileV1 {
        format: FORMAT.to_owned(),
        version: VERSION,
        next_id: doc.next_id(),
        params: doc
            .params()
            .iter()
            .map(|(id, p)| {
                let param = ParamV1 {
                    name: p.name.clone(),
                    source: p.source.clone(),
                    unit: p.unit.word().to_owned(),
                    value: p.value,
                };
                (id, param)
            })
            .collect(),
        frames: doc
            .frames()
            .iter()
            .map(|(id, f)| {
                let kind = match f.kind {
                    FrameKind::World => FrameKindV1::World,
                    FrameKind::OriginPlane => FrameKindV1::OriginPlane,
                    FrameKind::User => FrameKindV1::User,
                };
                (
                    id,
                    FrameV1 {
                        name: f.name.clone(),
                        kind,
                        parent: f.parent,
                        xyz: f.xyz,
                        quat_xyzw: f.quat_xyzw,
                        source: f.source,
                    },
                )
            })
            .collect(),
        sketches: doc.sketches().iter().map(|(id, s)| (id, s.clone())).collect(),
        bodies: doc.bodies().iter().map(|(id, b)| (id, b.clone())).collect(),
        robots: doc.robots().iter().map(|(id, r)| (id, r.clone())).collect(),
        waypoints: doc.waypoints().iter().map(|(id, w)| (id, w.clone())).collect(),
        paths: doc.paths().iter().map(|(id, p)| (id, p.clone())).collect(),
        work_frames: doc.work_frames().iter().map(|(id, f)| (id, f.clone())).collect(),
        work_features: doc.work_features().iter().map(|(id, f)| (id, f.clone())).collect(),
        features: doc.features().iter().map(|(id, f)| (id, f.clone())).collect(),
        assemblies: doc.assemblies().iter().map(|(id, a)| (id, a.clone())).collect(),
        timelines: doc.timeline().map(|(id, t)| (id, t.clone())).into_iter().collect(),
        relationships: doc.relationships().iter().map(|(id, r)| (id, r.clone())).collect(),
    };
    let mut text = serde_json::to_string_pretty(&file)?;
    text.push('\n');
    Ok(text)
}

/// Reads a document from the text of a `.gctrl` file.
pub fn from_text(text: &str) -> Result<Loaded, PersistError> {
    // The header is read first so that a newer or foreign file gets a clear message, not a parse error.
    let header: Header = serde_json::from_str(text).map_err(|_not_a_header| PersistError::NotGctrl)?;
    if header.format != FORMAT {
        return Err(PersistError::NotGctrl);
    }
    if header.version > VERSION {
        return Err(PersistError::TooNew { found: header.version, supported: VERSION });
    }
    let file: FileV1 = serde_json::from_str(text)?;
    let mut params = BTreeMap::new();
    for (id, p) in file.params {
        let unit = if p.unit.is_empty() {
            Unit::Scalar
        } else {
            Unit::from_word(&p.unit).ok_or_else(|| PersistError::UnknownUnit(p.unit.clone()))?
        };
        params.insert(id, Param { name: p.name, source: p.source, unit, value: p.value });
    }
    let frames = file
        .frames
        .into_iter()
        .map(|(id, f)| {
            let kind = match f.kind {
                FrameKindV1::World => FrameKind::World,
                FrameKindV1::OriginPlane => FrameKind::OriginPlane,
                FrameKindV1::User => FrameKind::User,
            };
            (
                id,
                Frame {
                    name: f.name,
                    kind,
                    parent: f.parent,
                    xyz: f.xyz,
                    quat_xyzw: f.quat_xyzw,
                    source: f.source,
                },
            )
        })
        .collect();
    let doc = Document::restore(Contents {
        next_id: file.next_id,
        params,
        frames,
        sketches: file.sketches,
        bodies: file.bodies,
        robots: file.robots,
        waypoints: file.waypoints,
        paths: file.paths,
        work_frames: file.work_frames,
        work_features: file.work_features,
        features: file.features,
        assemblies: file.assemblies,
        timelines: file.timelines,
        relationships: file.relationships,
    })?;
    let needs_repair = doc
        .sketches()
        .iter()
        .filter(|(_, sketch)| !sketch.violation().is_ok_and(|v| v <= 1.0))
        .map(|(id, _)| id)
        .collect();
    Ok(Loaded { doc, needs_repair })
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> PersistError + '_ {
    move |source| PersistError::Io { path: path.to_path_buf(), source }
}

/// Saves a document atomically, keeping the previous file as `<name>.bak`.
pub fn save(doc: &Document, path: &Path) -> Result<(), PersistError> {
    let text = to_text(doc)?;
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(io_error(path))?;
    temporary.write_all(text.as_bytes()).map_err(io_error(path))?;
    temporary.as_file().sync_all().map_err(io_error(path))?;
    if path.exists() {
        let mut backup = path.as_os_str().to_owned();
        backup.push(".bak");
        std::fs::copy(path, PathBuf::from(backup)).map_err(io_error(path))?;
    }
    temporary.persist(path).map_err(|e| PersistError::Io { path: path.to_path_buf(), source: e.error })?;
    Ok(())
}

/// Loads a document.
pub fn load(path: &Path) -> Result<Loaded, PersistError> {
    from_text(&std::fs::read_to_string(path).map_err(io_error(path))?)
}

#[cfg(test)]
mod tests {
    use gc_doc::{Command, Created, SketchOp, apply};
    use gc_sketch::DimKind;

    use super::*;

    fn sample() -> Document {
        let mut doc = Document::new();
        apply(&mut doc, &Command::AddParam { name: "폭".into(), source: "20 mm".into(), angle: false })
            .unwrap();
        apply(&mut doc, &Command::AddParam { name: "a".into(), source: "30".into(), angle: true }).unwrap();
        let frame = doc.origin_plane("XZ").unwrap();
        let created =
            apply(&mut doc, &Command::CreateSketch { name: "Sketch1".into(), frame, spatial: false })
                .unwrap();
        let Created::Sketch(sketch) = created.created[0] else { panic!("no sketch") };
        let rectangle =
            SketchOp::AddRectangle { a: [0.0, 0.0], b: [0.0123, 0.0071], snap_a: gc_doc::EndSnap::default() };
        let created = apply(&mut doc, &Command::Sketch { sketch, op: rectangle }).unwrap();
        let Created::Curve(bottom) = created.created[0] else { panic!("no line") };
        let (a, b) = doc.get(sketch).unwrap().line(bottom).unwrap();
        let width = SketchOp::AddDimension {
            kind: DimKind::Distance { p: a, q: b },
            source: "폭 * 2".into(),
            text_pos: [0.01, -0.004],
        };
        apply(&mut doc, &Command::Sketch { sketch, op: width }).unwrap();
        apply(
            &mut doc,
            &Command::Sketch {
                sketch,
                op: SketchOp::AddCircle {
                    centre: [0.05, 0.02, 0.0],
                    radius: 1.0 / 3.0 * 0.01,
                    centre_snap: gc_doc::EndSnap::default(),
                    rim_snap: gc_doc::EndSnap::default(),
                },
            },
        )
        .unwrap();
        // A robot cell: a body, a robot, a waypoint on a sketch point and a path.
        use gc_doc::{Body, CellOp, MotionPath, Placement, Robot, Solid, Waypoint, WaypointTarget};
        let cell = |doc: &mut Document, op: CellOp| apply(doc, &Command::Cell(op)).unwrap().created;
        let placement = Placement { xyz: [0.4, 0.0, 0.1], rpy: [0.0, 0.0, 0.5] };
        cell(
            &mut doc,
            CellOp::AddBody(Body {
                name: "기둥".into(),
                placement,
                solid: Solid::Cylinder { radius: 0.05, length: 1.0 },
                look: Default::default(),
            }),
        );
        let robot = Robot {
            name: "Robot1".into(),
            urdf: "<robot name=\"r\"/>".into(),
            resource_dir: String::new(),
            base_link: "base_link".into(),
            tip_link: "tool0".into(),
            placement: Placement::default(),
            tcp: Placement::default(),
            joints: vec![0.1, -0.2, 0.3],
            max_acc: 5.0,
            look: Default::default(),
            tool: None,
            acm_overrides: Vec::new(),
        };
        let Created::Robot(robot) = cell(&mut doc, CellOp::AddRobot(robot))[0] else { panic!("no robot") };
        let target = WaypointTarget::SketchPoint { sketch, point: a };
        let waypoint = Waypoint { name: "P1".into(), robot, target };
        let Created::Waypoint(waypoint) = cell(&mut doc, CellOp::AddWaypoint(waypoint))[0] else {
            panic!("no waypoint")
        };
        let path = MotionPath {
            name: "Path1".into(),
            robot,
            waypoints: vec![waypoint],
            linear: Vec::new(),
            planner: "rrt_connect".into(),
            seed: 7,
            tcp_speed: None,
            contacts: Vec::new(),
        };
        cell(&mut doc, CellOp::AddPath(path));
        // The design side: a work plane, an extrusion of the sketch, an imported assembly, and a stored
        // tree order with a suppressed item.
        let plane = gc_doc::WorkFeature {
            name: "Plane1".into(),
            geometry: gc_doc::WorkGeometry::PlaneOffset {
                base: gc_doc::PlaneRef::Origin(gc_doc::OriginPlane::Xy),
                distance: 0.25,
            },
            visible: true,
        };
        let Created::WorkFeature(plane) = cell(&mut doc, CellOp::AddWorkFeature(plane))[0] else { panic!() };
        let sketch = doc.sketches().iter().next().unwrap().0;
        let extrude = gc_doc::Extrude {
            sketch,
            profile: Vec::new(),
            extent: gc_doc::Extent::Distance(0.1),
            reversed: false,
        };
        cell(
            &mut doc,
            CellOp::AddFeature(gc_doc::Feature {
                name: "Field1".into(),
                kind: gc_doc::FeatureKind::Extrude(extrude),
            }),
        );
        let assembly = gc_doc::Assembly {
            name: "Machine".into(),
            file: "machine.glb".into(),
            placement: Placement { xyz: [1.0, 0.0, 0.0], rpy: [0.0; 3] },
            parts: vec![gc_doc::AssemblyPart {
                name: "Frame".into(),
                parent: None,
                matrix: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
                meshes: vec![0],
            }],
            meshes: vec![gc_doc::AssemblyMesh {
                name: "Plate".into(),
                color: [0.2, 0.4, 0.6, 1.0],
                triangles: 2,
            }],
            look: Default::default(),
        };
        cell(&mut doc, CellOp::AddAssembly(assembly));
        cell(&mut doc, CellOp::SetSuppressed { item: plane.erase(), suppressed: true });
        doc
    }

    #[test]
    fn load_after_save_is_the_identity_and_saving_is_deterministic() {
        let doc = sample();
        let text = to_text(&doc).unwrap();
        let loaded = from_text(&text).unwrap();
        assert!(loaded.doc.same_content(&doc));
        assert!(loaded.needs_repair.is_empty());
        assert_eq!(loaded.doc.next_id(), doc.next_id(), "ids are never reused after a reload");
        assert_eq!(to_text(&loaded.doc).unwrap(), text, "saving a loaded document reproduces the file");
        assert!(text.starts_with("{\n  \"format\": \"gctrl\",\n  \"version\": 1,"));
    }

    #[test]
    fn files_are_written_atomically_with_a_backup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cell.gctrl");
        let first = Document::new();
        save(&first, &path).unwrap();
        assert!(!directory.path().join("cell.gctrl.bak").exists());
        let second = sample();
        save(&second, &path).unwrap();
        assert!(load(&path).unwrap().doc.same_content(&second));
        assert!(load(&directory.path().join("cell.gctrl.bak")).unwrap().doc.same_content(&first));
        let leftovers: Vec<_> =
            std::fs::read_dir(directory.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers.len(), 2, "no temporary files remain: {leftovers:?}");
    }

    #[test]
    fn foreign_newer_and_damaged_files_are_refused_with_a_reason() {
        assert!(matches!(from_text("hello"), Err(PersistError::NotGctrl)));
        assert!(matches!(from_text(r#"{"format":"other","version":1}"#), Err(PersistError::NotGctrl)));
        assert!(matches!(
            from_text(r#"{"format":"gctrl","version":99}"#),
            Err(PersistError::TooNew { found: 99, supported: VERSION })
        ));
        assert!(matches!(from_text(r#"{"format":"gctrl","version":1}"#), Err(PersistError::Format(_))));
        let text = to_text(&sample()).unwrap();
        // Remove the frame the sketch sits on: the reference dangles.
        let broken = text.replace("\"XZ\"", "\"gone\"").replacen("\"3\": {", "\"99\": {", 1);
        assert!(matches!(from_text(&broken), Err(PersistError::Invalid(_))), "{broken}");
        assert!(matches!(load(Path::new("/nonexistent/x.gctrl")), Err(PersistError::Io { .. })));
    }

    #[test]
    fn a_sketch_that_violates_its_constraints_is_reported_not_changed() {
        let doc = sample();
        let text = to_text(&doc).unwrap();
        // Move one coordinate by hand: 0.04 (the solved width) becomes 0.05.
        assert!(text.contains("0.04,"));
        let tampered = text.replacen("0.04,", "0.05,", 1);
        let loaded = from_text(&tampered).unwrap();
        assert_eq!(loaded.needs_repair.len(), 1);
        assert_eq!(to_text(&loaded.doc).unwrap(), tampered, "loading did not touch the coordinates");
    }
}
