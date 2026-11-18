//! The properties of a sketch role (plan 9.1, "sketch → robot paths and waypoints"): how a path is
//! travelled (along the curves, in straight lines between their ends, by joint moves), the tool-tip speed,
//! the direction, and how the tool is held (measured from the sketch plane or a work frame, with roll, pitch
//! and yaw). Every change is one undo step.

use egui::{RichText, Ui};
use gc_core::Id;
use gc_doc::{AimReference, PathMotion, RoleKind, SketchRole};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// The editor of a role: nothing when the role no longer exists.
pub fn role_properties(ui: &mut Ui, session: &mut Session, lang: Lang, id: Id<SketchRole>) {
    let Some(role) = session.doc().get(id).cloned() else { return };
    gc_ui::panel::header(ui, &role.name);
    match &role.kind {
        RoleKind::Waypoint { .. } => {
            ui.label(tr!(lang, RoleWaypoint));
        }
        RoleKind::Path { curves, reversed, motion, tcp_speed } => {
            ui.label(tr!(lang, RolePath, curves.len()));
            let mut chosen = *motion;
            ui.label(RichText::new(tr!(lang, RoleMotion)).strong());
            for (value, label) in [
                (PathMotion::Follow, tr!(lang, RoleFollow)),
                (PathMotion::Linear, tr!(lang, RoleLinear)),
                (PathMotion::Joint, tr!(lang, RoleJoint)),
            ] {
                ui.radio_value(&mut chosen, value, label);
            }
            if chosen != *motion {
                // A refusal is explained in the status bar.
                drop(session.set_role_motion(id, chosen));
            }
            // The tool-tip speed, mm/s; empty for as fast as the joints allow.
            let key = egui::Id::unique("gc_role_speed").with(id);
            let mut text =
                ui.data(|d| d.get_temp::<(Option<f64>, String)>(key)).filter(|(from, _)| from == tcp_speed);
            let (_, typed) = text.get_or_insert_with(|| {
                (*tcp_speed, tcp_speed.map_or_else(String::new, |v| format!("{}", v * 1000.0)))
            });
            ui.horizontal(|ui| {
                let label = ui.label(tr!(lang, RoleSpeed));
                let field =
                    ui.add(egui::TextEdit::singleline(typed).desired_width(80.0)).labelled_by(label.id);
                if field.lost_focus() {
                    let wanted = if typed.trim().is_empty() {
                        Some(None)
                    } else {
                        gc_doc::evaluate_value(session.doc(), typed, gc_doc::Unit::Mm)
                            .filter(|v| *v > 0.0)
                            .map(Some)
                    };
                    if let Some(speed) = wanted
                        && speed != *tcp_speed
                    {
                        drop(session.set_role_speed(id, speed));
                    }
                }
            });
            if let Some(typed) = text {
                ui.data_mut(|d| d.insert_temp(key, typed));
            }
            let flip = if *reversed { tr!(lang, RoleReversed) } else { tr!(lang, RoleReverse) };
            if ui.button(flip).clicked() {
                drop(session.reverse_role(id));
            }
        }
    }
    ui.separator();
    // How the tool is held.
    ui.label(RichText::new(tr!(lang, RoleAim)).strong());
    let mut aim = role.aim;
    let name_of = |reference: AimReference| match reference {
        AimReference::SketchPlane => tr!(lang, RoleAimSketch).to_owned(),
        AimReference::Frame(frame) => session
            .doc()
            .get(frame)
            .map_or_else(String::new, |f: &gc_doc::WorkFrame| tr!(lang, RoleAimFrame, f.name)),
    };
    let references = session.role_aim_references();
    egui::ComboBox::from_id_salt(("gc_role_aim", id)).selected_text(name_of(aim.reference)).show_ui(
        ui,
        |ui| {
            for reference in &references {
                ui.selectable_value(&mut aim.reference, *reference, name_of(*reference));
            }
        },
    );
    let mut committed = aim.reference != role.aim.reference;
    egui::Grid::new(("gc_role_rpy", id)).num_columns(2).show(ui, |ui| {
        for (index, label) in
            [tr!(lang, RoleRoll), tr!(lang, RolePitch), tr!(lang, RoleYaw)].into_iter().enumerate()
        {
            ui.label(label);
            let mut degrees = aim.rpy[index].to_degrees();
            let response = ui.add(egui::DragValue::new(&mut degrees).speed(0.5).suffix("°"));
            aim.rpy[index] = degrees.to_radians();
            // One undo step per drag, not one per frame.
            if response.drag_stopped() || (response.lost_focus() && response.changed()) {
                committed = true;
            }
            ui.end_row();
        }
    });
    if committed && aim != role.aim {
        drop(session.set_role_aim(id, aim));
    }
}
