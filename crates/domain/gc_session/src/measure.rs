//! Measure in the cell, as Inventor's Measure command does in an assembly (Tools tab > Measure panel >
//! Measure; https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm):
//! with component priority, two components give their minimum distance; a point gives its position,
//! two points the distance and the delta; two directed items the angle between them.
//!
//! Distances use the collision shapes, so a measured gap and the collision check agree.

use gc_collide::{Closest, Geometry};
use gc_math::{DPose3, DVec3};
use gc_motion::RobotScene;

use crate::Session;
use crate::cell::{CellItem, PADDING, pose_of, shape_of, tool_shapes};

/// What one selected item measures on its own.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemMeasure {
    /// The item's name.
    pub name: String,
    /// Its origin in the world (metres): a robot's base, a body's placement, a waypoint's tool centre
    /// point, a frame's origin.
    pub position: [f64; 3],
}

/// What two selected items measure together.
#[derive(Clone, Debug, PartialEq)]
pub struct PairMeasure {
    /// The minimum distance between their shapes and where it is; `None` when one has no shapes (a
    /// path) or they could not be measured.
    pub minimum: Option<Closest>,
    /// The distance between their origins (Inventor's Center to Center).
    pub centre_to_centre: f64,
    /// The second origin minus the first, per world axis.
    pub delta: [f64; 3],
    /// The angle between their Z axes (radians), when both have a direction: frames, waypoints (the
    /// tool axis), robots and bodies (their placement).
    pub angle: Option<f64>,
}

impl Session {
    /// The pose of an item as it is shown; `None` for a path.
    fn item_pose(&self, item: CellItem) -> Option<DPose3> {
        match item {
            CellItem::Body(id) => self.shown_body(id).map(|b| pose_of(&b.placement)),
            CellItem::Robot(id) => self.shown_base(id).map(|p| pose_of(&p)),
            CellItem::Waypoint(id) => self.waypoint_pose(id),
            CellItem::Frame(id) => self.work_frame_pose(id),
            CellItem::Path(_) => None,
        }
    }

    /// The shapes of an item as it is shown, in the world: a body's solid, a robot's links and tool at
    /// its shown joint values, a waypoint or frame as its origin point. `None` for a path, or when a mesh
    /// is missing.
    pub fn item_geometry(&self, item: CellItem) -> Option<Geometry> {
        match item {
            CellItem::Body(id) => {
                let body = self.shown_body(id)?;
                let shape = shape_of(&body.solid, &self.cell.meshes)?;
                Geometry::from_shapes(&[(pose_of(&body.placement), shape)]).ok()
            }
            CellItem::Robot(id) => {
                let loaded = self.robot_model(id)?;
                let stored = self.doc().get(id)?;
                let base = self.shown_base(id)?;
                let tool = self.shown_tool(id)?;
                let (scene, _) = RobotScene::new(
                    std::sync::Arc::clone(&loaded.model),
                    loaded.chain.clone(),
                    &stored.tip_link,
                    pose_of(&base),
                    &[],
                    PADDING,
                    &|uri| loaded.file(uri),
                    &tool_shapes(tool.as_ref(), &self.cell.meshes).unwrap_or_default(),
                )
                .ok()?;
                Some(scene.geometry(&self.robot_joints(id)?))
            }
            CellItem::Waypoint(_) | CellItem::Frame(_) => {
                self.item_pose(item).map(|pose| Geometry::point(pose.translation.to_array()))
            }
            CellItem::Path(_) => None,
        }
    }

    /// What a selected item measures on its own; `None` for a path.
    pub fn measure_item(&self, item: CellItem) -> Option<ItemMeasure> {
        let pose = self.item_pose(item)?;
        Some(ItemMeasure {
            name: self.item_name(item).unwrap_or_default(),
            position: pose.translation.to_array(),
        })
    }

    /// What two selected items measure together; `None` when either is a path or gone.
    pub fn measure_pair(&self, first: CellItem, second: CellItem) -> Option<PairMeasure> {
        let (a, b) = (self.item_pose(first)?, self.item_pose(second)?);
        let minimum = match (self.item_geometry(first), self.item_geometry(second)) {
            (Some(ga), Some(gb)) => ga.closest(&gb).ok().flatten(),
            _ => None,
        };
        let delta = b.translation - a.translation;
        let (za, zb) = (a.rotation * DVec3::Z, b.rotation * DVec3::Z);
        let angle = gc_math::fm::atan2(za.cross(zb).length(), za.dot(zb));
        Some(PairMeasure {
            minimum,
            centre_to_centre: delta.length(),
            delta: delta.to_array(),
            angle: Some(angle),
        })
    }
}
