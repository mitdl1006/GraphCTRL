//! Grip Snap (Inventor 2026 help, Grip Snap move or rotate, GUID-E1CA109A-D5A1-4131-AC3A-AE217716DD8C):
//! the session side of the calculation. The viewport picks the edge and the two points, shows the result
//! (with the HUD's freedom report) and commits it with Done or Commit and Ground.

use gc_core::AnyId;
use gc_doc::{CellOp, Command, EditError, GripLine, GripMotion, Placement};
use gc_math::DPose3;

use crate::Session;

impl Session {
    /// Sets a joint's Limits (angular for Rotational and Cylindrical, linear for Slider and Cylindrical;
    /// `None` clears one); a range with `current` holds the joint there. One undo step.
    pub fn set_joint_limits(
        &mut self,
        id: gc_core::Id<gc_doc::Relationship>,
        angular: Option<gc_doc::JointRange>,
        linear: Option<gc_doc::JointRange>,
    ) -> Result<(), EditError> {
        self.execute(&Command::Cell(CellOp::SetJointLimits { id, angular, linear })).map(|_| ())
    }

    /// A joint's Limits: (angular, linear). The browser marks a joint that has one with +/-.
    pub fn joint_limits(
        &self,
        id: gc_core::Id<gc_doc::Relationship>,
    ) -> (Option<gc_doc::JointRange>, Option<gc_doc::JointRange>) {
        self.doc().joint_condition(id).map_or((None, None), |c| (c.angular, c.linear))
    }

    /// Where a joint stands now: (turn about its Z axis in radians, slide along it in metres), the values
    /// its Limits use (the dialog's Current).
    pub fn joint_position(&self, id: gc_core::Id<gc_doc::Relationship>) -> Option<(f64, f64)> {
        self.doc().joint_position(id)
    }

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

    /// Where Grip Snap puts a component for a motion about a point, line or plane reference (Grip Snap Move
    /// Options Bar Reference, GUID-C030C32C-7E1E-4F37-BBDF-120412898D00). `None` when the motion does not
    /// go with the reference or the points give none.
    pub fn grip_snap_pose_on(
        &self,
        item: AnyId,
        motion: GripMotion,
        reference: gc_doc::GripReference,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Option<DPose3> {
        self.doc().grip_snap_pose_on(item, motion, reference, from, to)
    }

    /// True when the component's remaining freedom allows that snap (any reference).
    pub fn grip_snap_allowed(
        &self,
        item: AnyId,
        motion: GripMotion,
        reference: gc_doc::GripReference,
        from: [f64; 3],
        to: [f64; 3],
    ) -> bool {
        self.doc().grip_snap_allowed(item, motion, reference, from, to)
    }

    /// Shows several components (bodies, robots, imported assemblies) at other poses at once, without
    /// touching the document: a subassembly dragged as one unit, a Grip Snap session. They are shown where
    /// Drive shows components (`driven_placement`, `shown_body`, `shown_base`). Refused (nothing shown) when
    /// one of them is not a component or a pose is not finite. Replaces any earlier such preview.
    pub fn preview_components(&mut self, moves: &[(AnyId, DPose3)]) -> Result<(), EditError> {
        let valid = moves.iter().all(|(item, pose)| {
            matches!(item.kind, gc_core::Kind::Body | gc_core::Kind::RobotInstance | gc_core::Kind::Assembly)
                && self.doc().component_origin(gc_doc::Occurrence::Item(*item)).is_some()
                && pose.translation.is_finite()
                && pose.rotation.is_finite()
        });
        if !valid {
            return Err(EditError::BadValue);
        }
        self.show_relation_drive(moves.to_vec());
        Ok(())
    }

    /// Ends [`Session::preview_components`]: the components are shown where the document has them again
    /// (commit with [`Session::grip_snap_commit`] or [`Session::move_subassembly`] first to keep the moves).
    pub fn end_component_preview(&mut self) {
        self.end_drive_relationship();
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
