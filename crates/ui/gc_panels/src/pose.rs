//! Poses in numbers: what a waypoint is measured from, the same pose seen from other frames, transform
//! matrices, and a calculator for composing, inverting and re-expressing poses.

use egui::{RichText, Ui};
use gc_core::Id;
use gc_doc::{Placement, Robot, Waypoint, WaypointTarget};
use gc_i18n::{Lang, tr};
use gc_math::{DPose3, DQuat, DVec3};
use gc_session::{Reference, Session};

/// Values that round to zero are shown as 0, not -0.
fn clean(v: f64) -> f64 {
    if v.abs() < 5e-7 { 0.0 } else { v }
}

/// The name of a reference frame as the user sees it.
pub(crate) fn reference_name(session: &Session, lang: Lang, reference: Reference) -> String {
    match reference {
        Reference::World => tr!(lang, RefWorld).to_owned(),
        Reference::Base => tr!(lang, RefBase).to_owned(),
        Reference::Frame(frame) => session.doc().get(frame).map(|f| f.name.clone()).unwrap_or_default(),
    }
}

/// The reference frames there are: the world, the robot's base when there is a robot, the work frames.
pub(crate) fn references(session: &Session, robot: Option<Id<Robot>>) -> Vec<Reference> {
    let mut all = vec![Reference::World];
    if robot.is_some() {
        all.push(Reference::Base);
    }
    all.extend(session.doc().work_frames().iter().map(|(id, _)| Reference::Frame(id)));
    all
}

/// A choice among the reference frames. Returns the new choice when the user picked another one.
fn reference_combo(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    label: &str,
    robot: Option<Id<Robot>>,
    current: Option<Reference>,
    none_text: &str,
) -> Option<Reference> {
    let shown = current.map_or_else(|| none_text.to_owned(), |r| reference_name(session, lang, r));
    let mut picked = None;
    egui::ComboBox::from_label(label).selected_text(shown).show_ui(ui, |ui| {
        for reference in references(session, robot) {
            let name = reference_name(session, lang, reference);
            if ui.selectable_label(current == Some(reference), name).clicked() && current != Some(reference) {
                picked = Some(reference);
            }
        }
    });
    picked
}

/// A pose as the rows of a readout: position in millimetres, roll-pitch-yaw in degrees, the rotation
/// vector UR controllers use, and the quaternion.
pub(crate) fn pose_lines(pose: &DPose3) -> [String; 4] {
    let t = (pose.translation * 1000.0).to_array().map(clean);
    let [r, p, y] = gc_math::rpy_from_quat(pose.rotation).map(|a| clean(a.to_degrees()));
    let v = gc_math::rotvec_from_quat(pose.rotation).to_array().map(clean);
    let q = pose.rotation.to_array().map(clean);
    [
        format!("X {:.3}  Y {:.3}  Z {:.3} mm", t[0], t[1], t[2]),
        format!("R {r:.3}°  P {p:.3}°  Y {y:.3}°"),
        format!("Rx {:.5}  Ry {:.5}  Rz {:.5} rad", v[0], v[1], v[2]),
        format!("qx {:.6}  qy {:.6}  qz {:.6}  qw {:.6}", q[0], q[1], q[2], q[3]),
    ]
}

/// The 4×4 homogeneous transform of a pose, translation in millimetres.
pub(crate) fn matrix_lines(pose: &DPose3) -> [String; 4] {
    let m = pose.to_mat4();
    let row = |i: usize| {
        let c = |j: usize| clean(m.col(j)[i]);
        if i < 3 {
            format!("{:>10.6} {:>10.6} {:>10.6} {:>11.3}", c(0), c(1), c(2), c(3) * 1000.0)
        } else {
            format!("{:>10.0} {:>10.0} {:>10.0} {:>11.0}", c(0), c(1), c(2), c(3))
        }
    };
    [row(0), row(1), row(2), row(3)]
}

/// Labelled rows of poses, each with a copy button.
pub(crate) fn pose_rows(
    ui: &mut Ui,
    lang: Lang,
    salt: impl std::hash::Hash + std::fmt::Debug,
    rows: &[(String, DPose3)],
) {
    egui::Grid::new(salt).num_columns(3).show(ui, |ui| {
        for (title, pose) in rows {
            let lines = pose_lines(pose);
            ui.label(title);
            ui.vertical(|ui| {
                for line in &lines[..3] {
                    ui.label(RichText::new(line).monospace());
                }
            });
            if ui.small_button(tr!(lang, CmdCopy)).clicked() {
                ui.ctx().copy_text(format!("{title}\n{}", lines.join("\n")));
            }
            ui.end_row();
        }
    });
}

/// A transform matrix with a copy button.
pub(crate) fn matrix(ui: &mut Ui, lang: Lang, pose: &DPose3) {
    let lines = matrix_lines(pose);
    for line in &lines {
        ui.label(RichText::new(line).monospace());
    }
    if ui.small_button(tr!(lang, CmdCopy)).clicked() {
        ui.ctx().copy_text(lines.join("\n"));
    }
}

/// A waypoint's place in numbers: what it is measured from (and a choice to measure it from another
/// frame, which keeps it where it is), its position and rotation there, the same pose seen from every
/// other frame, and its transform matrix. `open` forces the section open (the waypoint is selected).
pub(crate) fn waypoint_place(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<Waypoint>, open: bool) {
    let Some(waypoint) = session.doc().get(id).cloned() else { return };
    let Some(world) = session.waypoint_pose(id) else { return };
    let current = session.waypoint_reference(id);
    let title = match current {
        Some(reference) => tr!(lang, PropInFrame, waypoint.name, reference_name(session, lang, reference)),
        None => waypoint.name.clone(),
    };
    egui::CollapsingHeader::new(title)
        .id_salt(("gc_waypoint_place", id.raw()))
        .open(open.then_some(true))
        .show(ui, |ui| {
            let none = match waypoint.target {
                WaypointTarget::SketchPoint { .. } => tr!(lang, RefSketchPoint),
                _ => tr!(lang, RefJoints),
            };
            let label = tr!(lang, PropReference);
            if let Some(reference) =
                reference_combo(ui, session, lang, label, Some(waypoint.robot), current, none)
            {
                // A refusal (a frame that cannot be placed) is shown in the status bar.
                drop(session.express_waypoint(id, reference));
                return;
            }
            // The numbers in the chosen frame, editable.
            if let Some(reference) = current
                && let Some(origin) = session.reference_pose(waypoint.robot, reference)
            {
                let local = origin.inverse() * world;
                let mut offset = Placement {
                    xyz: local.translation.to_array(),
                    rpy: gc_math::rpy_from_quat(local.rotation),
                };
                if crate::cell::placement(ui, lang, &mut offset) {
                    let moved = origin * gc_math::pose_from_xyz_rpy(offset.xyz, offset.rpy);
                    if let Some(target) = session.target_at(id, reference, moved) {
                        session.preview_target(id, target);
                    }
                }
            }
            egui::CollapsingHeader::new(tr!(lang, PropSeenFrom)).id_salt(("gc_seen_from", id.raw())).show(
                ui,
                |ui| {
                    let rows: Vec<(String, DPose3)> = references(session, Some(waypoint.robot))
                        .into_iter()
                        .filter_map(|reference| {
                            let origin = session.reference_pose(waypoint.robot, reference)?;
                            Some((reference_name(session, lang, reference), origin.inverse() * world))
                        })
                        .collect();
                    pose_rows(ui, lang, ("gc_seen_rows", id.raw()), &rows);
                },
            );
            egui::CollapsingHeader::new(tr!(lang, PropMatrix)).id_salt(("gc_matrix", id.raw())).show(
                ui,
                |ui| {
                    let origin = current
                        .and_then(|reference| session.reference_pose(waypoint.robot, reference))
                        .unwrap_or(DPose3::IDENTITY);
                    matrix(ui, lang, &(origin.inverse() * world));
                },
            );
        });
}

/// How rotations are typed into the calculator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RotationInput {
    /// Roll, pitch, yaw in degrees (fixed axes X, then Y, then Z).
    #[default]
    Rpy,
    /// A rotation vector in radians, as UR controllers use.
    RotVec,
    /// A unit quaternion (x, y, z, w).
    Quaternion,
}

/// The pose calculator's state.
#[derive(Clone, Debug)]
pub struct PoseCalc {
    /// The first pose.
    pub a: DPose3,
    /// The second pose.
    pub b: DPose3,
    /// How rotations are typed.
    pub input: RotationInput,
    /// The frame `a` is measured from, for re-expressing it.
    pub from: Reference,
    /// The frame to re-express `a` in.
    pub to: Reference,
}

impl Default for PoseCalc {
    fn default() -> Self {
        Self {
            a: DPose3::IDENTITY,
            b: DPose3::IDENTITY,
            input: RotationInput::Rpy,
            from: Reference::World,
            to: Reference::World,
        }
    }
}

/// Edits a pose. Returns true when it changed.
fn pose_editor(ui: &mut Ui, lang: Lang, pose: &mut DPose3, input: RotationInput) -> bool {
    let mut changed = false;
    let mut t = (pose.translation * 1000.0).to_array().map(|v| v + 0.0);
    ui.horizontal(|ui| {
        ui.label(tr!(lang, PropPosition));
        for (value, name) in t.iter_mut().zip(["X", "Y", "Z"]) {
            changed |= ui.add(egui::DragValue::new(value).speed(1.0).prefix(format!("{name} "))).changed();
        }
    });
    let mut rotation = pose.rotation;
    ui.horizontal(|ui| {
        ui.label(tr!(lang, PropRotation));
        match input {
            RotationInput::Rpy => {
                let mut rpy = gc_math::rpy_from_quat(rotation).map(|a| a.to_degrees() + 0.0);
                let mut edited = false;
                for (value, name) in rpy.iter_mut().zip(["R", "P", "Y"]) {
                    edited |= ui
                        .add(egui::DragValue::new(value).speed(1.0).suffix("°").prefix(format!("{name} ")))
                        .changed();
                }
                if edited {
                    rotation =
                        gc_math::quat_from_rpy(rpy[0].to_radians(), rpy[1].to_radians(), rpy[2].to_radians());
                    changed = true;
                }
            }
            RotationInput::RotVec => {
                let mut v = gc_math::rotvec_from_quat(rotation).to_array();
                let mut edited = false;
                for (value, name) in v.iter_mut().zip(["Rx", "Ry", "Rz"]) {
                    edited |=
                        ui.add(egui::DragValue::new(value).speed(0.01).prefix(format!("{name} "))).changed();
                }
                if edited {
                    rotation = gc_math::quat_from_rotvec(DVec3::from_array(v));
                    changed = true;
                }
            }
            RotationInput::Quaternion => {
                let mut q = rotation.to_array();
                let mut edited = false;
                for (value, name) in q.iter_mut().zip(["qx", "qy", "qz", "qw"]) {
                    edited |=
                        ui.add(egui::DragValue::new(value).speed(0.01).prefix(format!("{name} "))).changed();
                }
                let quat = DQuat::from_array(q);
                if edited && quat.length() > 1e-9 {
                    rotation = quat.normalize();
                    changed = true;
                }
            }
        }
    });
    if changed {
        *pose = DPose3::from_parts(DVec3::from_array(t) / 1000.0, rotation);
    }
    changed
}

/// A result of the calculator: its numbers, its matrix, and buttons to copy it or use it as pose A.
fn result(ui: &mut Ui, lang: Lang, title: &str, pose: &DPose3, use_as_a: &mut Option<DPose3>) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong());
        if ui.small_button(tr!(lang, CmdUseAsA)).clicked() {
            *use_as_a = Some(*pose);
        }
        if ui.small_button(tr!(lang, CmdCopy)).clicked() {
            let mut text = vec![title.to_owned()];
            text.extend(pose_lines(pose));
            text.extend(matrix_lines(pose));
            ui.ctx().copy_text(text.join("\n"));
        }
    });
    for line in pose_lines(pose) {
        ui.label(RichText::new(line).monospace());
    }
    egui::CollapsingHeader::new(tr!(lang, PropMatrix)).id_salt(("gc_calc_matrix", title)).show(ui, |ui| {
        for line in matrix_lines(pose) {
            ui.label(RichText::new(line).monospace());
        }
    });
}

/// The pose calculator: two poses typed as roll-pitch-yaw, rotation vector or quaternion; pose A seen
/// from another frame; and A·B, A^-1 and A^-1·B, each with its matrix.
pub fn pose_calculator(ui: &mut Ui, session: &Session, lang: Lang, calc: &mut PoseCalc) {
    let robot = session.doc().robots().iter().next().map(|(id, _)| id);
    ui.horizontal(|ui| {
        ui.label(tr!(lang, PropRotationInput));
        ui.selectable_value(&mut calc.input, RotationInput::Rpy, tr!(lang, InputRpy));
        ui.selectable_value(&mut calc.input, RotationInput::RotVec, tr!(lang, InputRotvec));
        ui.selectable_value(&mut calc.input, RotationInput::Quaternion, tr!(lang, InputQuaternion));
    });
    ui.add_space(6.0);
    ui.label(RichText::new(tr!(lang, CalcPoseA)).strong());
    pose_editor(ui, lang, &mut calc.a, calc.input);
    // Pose A from the selected waypoint, measured from the "from" frame.
    if let Some(gc_session::CellItem::Waypoint(id)) = session.selected_item()
        && let Some(world) = session.waypoint_pose(id)
        && let Some(owner) = session.doc().get(id).map(|w| w.robot)
        && let Some(origin) = session.reference_pose(owner, calc.from)
        && ui.button(tr!(lang, CmdFromWaypoint)).clicked()
    {
        calc.a = origin.inverse() * world;
    }
    ui.label(RichText::new(tr!(lang, CalcPoseB)).strong());
    pose_editor(ui, lang, &mut calc.b, calc.input);
    ui.separator();

    // A measured from one frame, seen from another.
    ui.horizontal(|ui| {
        if let Some(r) = reference_combo(ui, session, lang, tr!(lang, CalcFrom), robot, Some(calc.from), "") {
            calc.from = r;
        }
        if let Some(r) = reference_combo(ui, session, lang, tr!(lang, CalcTo), robot, Some(calc.to), "") {
            calc.to = r;
        }
    });
    let mut use_as_a = None;
    let frame = |reference| robot.map_or(Some(DPose3::IDENTITY), |r| session.reference_pose(r, reference));
    if let (Some(from), Some(to)) = (frame(calc.from), frame(calc.to)) {
        let title = tr!(
            lang,
            CalcExpressed,
            reference_name(session, lang, calc.from),
            reference_name(session, lang, calc.to)
        );
        result(ui, lang, &title, &(to.inverse() * from * calc.a), &mut use_as_a);
    }
    ui.separator();
    result(ui, lang, tr!(lang, CalcCompose), &(calc.a * calc.b), &mut use_as_a);
    result(ui, lang, tr!(lang, CalcInverse), &calc.a.inverse(), &mut use_as_a);
    result(ui, lang, tr!(lang, CalcRelative), &(calc.a.inverse() * calc.b), &mut use_as_a);
    if let Some(pose) = use_as_a {
        calc.a = pose;
    }
}
