//! Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C):
//! the session side of the calculation. The viewport picks the edge and the two points, shows the result
//! (with the HUD's freedom report) and commits it with Done or Commit and Ground.

use gc_core::AnyId;
use gc_doc::{CellOp, Command, EditError, GripLine, GripMotion, Placement};
use gc_math::DPose3;

use crate::Session;

impl Session {
    /// A work plane, axis or point of the assembly as a relationship's selection (its geometry is read live
    /// from the work feature): pass it to [`Session::add_relationship`], `possible_relations` and the like.
    pub fn work_feature_selection(&self, id: gc_core::Id<gc_doc::WorkFeature>) -> Option<gc_doc::Selection> {
        self.doc().work_feature_selection(id)
    }

    /// Where Grip Snap puts a component (body, robot or imported assembly): moved along the reference line
    /// by the part of `to − from` along it, or turned about it by the angle from `from` to `to`. `None` for
    /// something that is not a component, a line of no length, or (turning) a point on the line.
    pub fn grip_snap_pose(
        &self,
        item: AnyId,
        motion: GripMotion,
        line: GripLine,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Option<DPose3> {
        self.doc().grip_snap_pose(item, motion, line, from, to)
    }

    /// The HUD's report: true when the component's remaining freedom allows moving along (or turning
    /// about) the line. Grounded: never; untouched by relationships: always.
    pub fn grip_allows(&self, item: AnyId, motion: GripMotion, line: GripLine) -> bool {
        self.doc().grip_allows(item, motion, line)
    }

    /// Ends a Grip Snap: puts the components where they were snapped (what is tied to them follows), and
    /// with `ground` (Commit and Ground) grounds them too. One undo step.
    pub fn grip_snap_commit(&mut self, moves: &[(AnyId, DPose3)], ground: bool) -> Result<(), EditError> {
        let moves: Vec<(AnyId, Placement)> = moves
            .iter()
            .map(|(item, pose)| {
                (
                    *item,
                    Placement {
                        xyz: pose.translation.to_array(),
                        rpy: gc_math::rpy_from_quat(pose.rotation),
                    },
                )
            })
            .collect();
        let op = if ground { CellOp::MoveAndGround(moves) } else { CellOp::MoveComponents(moves) };
        self.execute(&Command::Cell(op)).map(|_| ())
    }
}
