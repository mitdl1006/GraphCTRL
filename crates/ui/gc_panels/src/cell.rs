//! Robot-cell panels: jogging, body placement and path planning.

use egui::{RichText, Ui};
use gc_doc::{CellOp, Command, Solid};
use gc_i18n::{Lang, tr};
use gc_session::{PLANNERS, PlanStatus, Session};
use gc_ui::colors;

/// Display names of the planners, in the order of [`PLANNERS`].
const PLANNER_NAMES: [&str; 5] = ["RRT-Connect", "RRT", "RRT*", "Informed RRT*", "PRM"];

/// State of the cell panels that lives between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellUi {
    /// True while the planned motion is being played.
    pub playing: bool,
    /// The path the panel and the ribbon work on; the newest one when unset or deleted.
    pub path: Option<gc_core::Id<gc_doc::MotionPath>>,
    /// The work frame new waypoints are taught in; the newest one when unset or deleted.
    pub frame: Option<gc_core::Id<gc_doc::WorkFrame>>,
}

impl CellUi {
    /// The work frame new waypoints are taught in.
    pub fn active_frame(&self, session: &Session) -> Option<gc_core::Id<gc_doc::WorkFrame>> {
        let frames = session.doc().work_frames();
        self.frame.filter(|id| frames.get(*id).is_some()).or_else(|| frames.iter().last().map(|(id, _)| id))
    }

    /// The path the panel and the ribbon work on.
    pub fn active_path(&self, session: &Session) -> Option<gc_core::Id<gc_doc::MotionPath>> {
        let paths = session.doc().paths();
        self.path.filter(|id| paths.get(*id).is_some()).or_else(|| paths.iter().last().map(|(id, _)| id))
    }
}

/// The text of a plan status.
pub fn status_text(lang: Lang, status: PlanStatus) -> &'static str {
    match status {
        PlanStatus::NotPlanned => tr!(lang, PlanNotPlanned),
        PlanStatus::Planning => tr!(lang, PlanPlanning),
        PlanStatus::Ready => tr!(lang, PlanReady),
        PlanStatus::Stale => tr!(lang, PlanStale),
        PlanStatus::Failed => tr!(lang, PlanFailed),
    }
}

/// Jog sliders, body placement and path planning. Shows nothing while the cell is empty.
pub fn cell(ui: &mut Ui, session: &mut Session, lang: Lang, state: &mut CellUi) {
    let pointer_down = ui.input(|i| i.pointer.any_down());
    jog(ui, session, lang);
    base(ui, session, lang);
    bodies(ui, session, lang);
    frames(ui, session, lang, state);
    if !pointer_down && !ui.memory(|m| m.focused().is_some()) {
        // The previews become one undo step each when the mouse is released.
        session.end_jog();
        session.end_base_preview();
        session.end_tcp_preview();
        session.end_body_preview();
        session.end_target_preview();
        session.end_frame_preview();
    }
    paths(ui, session, lang, state);
}

fn jog(ui: &mut Ui, session: &mut Session, lang: Lang) {
    let Some((robot, _)) = session.doc().robots().iter().next() else { return };
    let (Some(loaded), Some(mut joints)) = (session.robot_model(robot).cloned(), session.robot_joints(robot))
    else {
        return;
    };
    ui.add_space(8.0);
    ui.label(RichText::new(tr!(lang, PanelJog)).strong());
    let mut changed = false;
    let chain = &loaded.chain;
    for (index, name) in chain.joint_names().iter().enumerate() {
        let (lower, upper) = (chain.lower()[index].to_degrees(), chain.upper()[index].to_degrees());
        let shown = joints[index].to_degrees();
        let mut degrees = shown;
        // Only the text is rounded. A slider told to keep one decimal would round the value itself and
        // report that as a change, which would move the robot by itself.
        let slider = egui::Slider::new(&mut degrees, lower..=upper)
            .text(name)
            .custom_formatter(|value, _| format!("{value:.1}"))
            .suffix("°");
        ui.add(slider);
        if (degrees - shown).abs() > 1e-9 {
            joints[index] = degrees.to_radians();
            changed = true;
        }
    }
    if changed {
        session.jog(robot, &joints);
    }
    // What is wrong with the pose as it is shown, if anything.
    if let Some(problem) = session.robot_problem(robot) {
        ui.colored_label(colors::ERROR, problem_text(lang, &problem));
    }
}

/// A state problem in the user's language.
pub fn problem_text(lang: Lang, problem: &gc_session::StateProblem) -> String {
    use gc_session::StateProblem as P;
    match problem {
        P::Collision { a, b } => tr!(lang, StateCollision, a, b),
        P::WithinPadding { a, b, gap } => tr!(lang, StateTooClose, a, b, format!("{:.1}", gap * 1000.0)),
        P::JointLimit { joint } => tr!(lang, StateJointLimit, joint),
        P::Unknown => tr!(lang, StateUnknown).to_owned(),
    }
}

/// An angle shown and edited in degrees. Returns true when the user changed it.
fn degrees(ui: &mut Ui, value: &mut f64, name: &str) -> bool {
    let mut shown = value.to_degrees();
    let changed = ui.add(egui::DragValue::new(&mut shown).speed(1.0).prefix(format!("{name} "))).changed();
    if changed {
        *value = shown.to_radians();
    }
    changed
}

/// Position and rotation of a placement. Returns true when the user changed something.
fn placement(ui: &mut Ui, lang: Lang, placement: &mut gc_doc::Placement) -> bool {
    let mut changed = false;
    ui.label(tr!(lang, PropPosition));
    ui.horizontal(|ui| {
        for (value, name) in placement.xyz.iter_mut().zip(["X", "Y", "Z"]) {
            changed |= millimetres(ui, value, name, -100_000.0);
        }
    });
    ui.label(tr!(lang, PropRotation));
    ui.horizontal(|ui| {
        for (value, name) in placement.rpy.iter_mut().zip(["R", "P", "Y"]) {
            changed |= degrees(ui, value, name);
        }
    });
    changed
}

/// Where the robot's base stands: on the floor, on a table, on a piece of equipment.
fn base(ui: &mut Ui, session: &mut Session, lang: Lang) {
    let Some((robot, _)) = session.doc().robots().iter().next() else { return };
    let Some(mut shown) = session.shown_base(robot) else { return };
    let mut changed = false;
    egui::CollapsingHeader::new(tr!(lang, PanelBase)).id_salt("gc_robot_base").show(ui, |ui| {
        changed = placement(ui, lang, &mut shown);
    });
    if changed {
        session.preview_base(robot, shown);
    }
    // The tool centre point, measured from the flange: what waypoints and paths are about.
    let Some(mut tcp) = session.shown_tcp(robot) else { return };
    let mut changed = false;
    egui::CollapsingHeader::new(tr!(lang, PanelTcp)).id_salt("gc_robot_tcp").show(ui, |ui| {
        changed = placement(ui, lang, &mut tcp);
    });
    if changed {
        session.preview_tcp(robot, tcp);
    }
}

/// A length shown and edited in millimetres. Returns true when the user changed it.
fn millimetres(ui: &mut Ui, value: &mut f64, name: &str, least: f64) -> bool {
    let mut mm = *value * 1000.0;
    let drag = egui::DragValue::new(&mut mm).speed(1.0).range(least..=100_000.0).prefix(format!("{name} "));
    let changed = ui.add(drag).changed();
    if changed {
        *value = mm / 1000.0;
    }
    changed
}

/// The work frames (which one new waypoints are taught in, and how each is turned) and the waypoints
/// defined in them, whose positions are edited here.
fn frames(ui: &mut Ui, session: &mut Session, lang: Lang, state: &mut CellUi) {
    let frames: Vec<_> = session.doc().work_frames().iter().map(|(id, f)| (id, f.name.clone())).collect();
    if frames.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.label(RichText::new(tr!(lang, PanelFrames)).strong());
    if let Some(active) = state.active_frame(session) {
        let mut chosen = active;
        let name = frames.iter().find(|(id, _)| *id == active).map(|(_, n)| n.clone()).unwrap_or_default();
        egui::ComboBox::from_label(tr!(lang, BrowserFrames))
            .selected_text(tr!(lang, PropTeachFrame, name))
            .show_ui(ui, |ui| {
                for (id, name) in &frames {
                    ui.selectable_value(&mut chosen, *id, name);
                }
            });
        state.frame = Some(chosen);
    }
    for (id, name) in &frames {
        let Some(mut frame) = session.shown_frame(*id) else { continue };
        let mut changed = false;
        egui::CollapsingHeader::new(name).id_salt(("gc_frame", id.raw())).show(ui, |ui| {
            ui.label(tr!(lang, PropRotation));
            ui.horizontal(|ui| {
                for (value, axis) in frame.rpy.iter_mut().zip(["R", "P", "Y"]) {
                    changed |= degrees(ui, value, axis);
                }
            });
        });
        if changed {
            session.preview_frame(*id, frame);
        }
    }
    let waypoints: Vec<_> = session.doc().waypoints().iter().map(|(id, w)| (id, w.name.clone())).collect();
    for (id, name) in waypoints {
        let Some(gc_doc::WaypointTarget::InFrame { frame, mut offset }) = session.shown_target(id) else {
            continue;
        };
        let frame_name = session.doc().get(frame).map(|f| f.name.clone()).unwrap_or_default();
        let mut changed = false;
        egui::CollapsingHeader::new(tr!(lang, PropInFrame, name, frame_name))
            .id_salt(("gc_in_frame", id.raw()))
            .show(ui, |ui| changed = placement(ui, lang, &mut offset));
        if changed {
            session.preview_target(id, gc_doc::WaypointTarget::InFrame { frame, offset });
        }
    }
}

fn bodies(ui: &mut Ui, session: &mut Session, lang: Lang) {
    let ids: Vec<_> = session.doc().bodies().iter().map(|(id, _)| id).collect();
    if ids.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.label(RichText::new(tr!(lang, PanelBodies)).strong());
    for id in ids {
        let Some(mut body) = session.shown_body(id).cloned() else { continue };
        let mut changed = false;
        egui::CollapsingHeader::new(&body.name).id_salt(("gc_body", id.raw())).show(ui, |ui| {
            changed |= placement(ui, lang, &mut body.placement);
            match &mut body.solid {
                Solid::Mesh { file, scale } => {
                    ui.horizontal(|ui| {
                        let label = ui.label(tr!(lang, PropScale));
                        let drag = egui::DragValue::new(scale).speed(0.001).range(1e-6..=1000.0);
                        changed |= ui.add(drag).labelled_by(label.id).changed();
                    });
                    if let Some(error) = session.mesh_error(file) {
                        ui.colored_label(colors::ERROR, error);
                    }
                }
                Solid::Box { size } => {
                    ui.label(tr!(lang, PropSize));
                    ui.horizontal(|ui| {
                        for (value, name) in size.iter_mut().zip(["X", "Y", "Z"]) {
                            changed |= millimetres(ui, value, name, 1.0);
                        }
                    });
                }
                Solid::Cylinder { radius, length } => {
                    ui.label(tr!(lang, PropSize));
                    ui.horizontal(|ui| {
                        changed |= millimetres(ui, radius, "R", 1.0);
                        changed |= millimetres(ui, length, "L", 1.0);
                    });
                }
                Solid::Sphere { radius } => {
                    ui.label(tr!(lang, PropSize));
                    changed |= ui.horizontal(|ui| millimetres(ui, radius, "R", 1.0)).inner;
                }
            }
        });
        if changed {
            session.preview_body(id, body);
        }
    }
}

fn paths(ui: &mut Ui, session: &mut Session, lang: Lang, state: &mut CellUi) {
    let Some(path_id) = state.active_path(session) else {
        state.playing = false;
        return;
    };
    let Some(path) = session.doc().get(path_id).cloned() else { return };
    ui.add_space(8.0);
    ui.label(RichText::new(tr!(lang, PanelPath)).strong());
    // With several paths, this chooses the one the panel and the ribbon work on.
    let names: Vec<_> = session.doc().paths().iter().map(|(id, p)| (id, p.name.clone())).collect();
    if names.len() > 1 {
        let mut chosen = path_id;
        egui::ComboBox::from_label(tr!(lang, BrowserPaths)).selected_text(&path.name).show_ui(ui, |ui| {
            for (id, name) in names {
                ui.selectable_value(&mut chosen, id, name);
            }
        });
        if chosen != path_id {
            state.path = Some(chosen);
            state.playing = false;
            session.set_playback(None);
            return;
        }
    } else {
        ui.label(&path.name);
    }
    let mut planner = path.planner.clone();
    let mut seed = path.seed;
    let shown =
        PLANNERS.iter().position(|(id, _)| *id == planner).map_or(planner.as_str(), |i| PLANNER_NAMES[i]);
    egui::ComboBox::from_label(tr!(lang, PropPlanner)).selected_text(shown).show_ui(ui, |ui| {
        for ((id, _), name) in PLANNERS.iter().zip(PLANNER_NAMES) {
            ui.selectable_value(&mut planner, (*id).to_owned(), name);
        }
    });
    ui.horizontal(|ui| {
        let label = ui.label(tr!(lang, PropSeed));
        ui.add(egui::DragValue::new(&mut seed).speed(0.1)).labelled_by(label.id);
    });
    if planner != path.planner || seed != path.seed {
        // A refusal is shown in the status bar.
        drop(session.execute(&Command::Cell(CellOp::SetPlanner { id: path_id, planner, seed })));
    }
    // How each waypoint is approached: freely planned, or in a straight line of the tool tip.
    for (index, waypoint) in path.waypoints.iter().enumerate() {
        let Some(name) = session.doc().get(*waypoint).map(|w| w.name.clone()) else { continue };
        let mut linear = path.linear.get(index).copied().unwrap_or(false);
        if ui.checkbox(&mut linear, tr!(lang, PropLinear, name)).changed() {
            drop(session.execute(&Command::Cell(CellOp::SetLinear { id: path_id, index, linear })));
        }
    }
    let status = session.plan_status(path_id);
    let color = match status {
        PlanStatus::Ready => colors::TRACE,
        PlanStatus::Failed => colors::ERROR,
        PlanStatus::Stale | PlanStatus::NotPlanned | PlanStatus::Planning => colors::INACTIVE,
    };
    ui.colored_label(color, status_text(lang, status));
    if let Some(error) = session.plan_error(path_id) {
        ui.colored_label(colors::ERROR, error);
    }
    comparison(ui, session, lang, path_id);
    let Some(planned) = session.planned(path_id) else {
        state.playing = false;
        return;
    };
    let duration = planned.duration();
    ui.label(tr!(lang, PlanDuration, format!("{duration:.2}")));
    let clearance = planned.min_clearance();
    if clearance.is_finite() {
        ui.label(tr!(lang, PlanClearance, format!("{:.1}", clearance * 1000.0)));
    }
    if planned.segments.iter().all(|s| s.straight) {
        ui.label(tr!(lang, PlanStraight));
    } else {
        ui.label(tr!(lang, PlanChecks, planned.state_checks()));
    }
    let mut time = session.playback().filter(|(id, _)| *id == path_id).map_or(0.0, |(_, t)| t);
    let before = time;
    let slider = egui::Slider::new(&mut time, 0.0..=duration)
        .text(tr!(lang, PropTime))
        .custom_formatter(|value, _| format!("{value:.2}"));
    ui.add(slider);
    if state.playing && before >= duration {
        time = 0.0;
    }
    if state.playing {
        time += f64::from(ui.input(|i| i.stable_dt)).min(0.1);
        if time >= duration {
            time = duration;
            state.playing = false;
        }
        ui.ctx().request_repaint();
    }
    if state.playing || (time - before).abs() > 1e-9 || session.playback().is_some() {
        session.set_playback(Some((path_id, time)));
    }
}

/// The planner comparison of a path: a button to run it, its progress, and the table.
fn comparison(ui: &mut Ui, session: &mut Session, lang: Lang, path: gc_core::Id<gc_doc::MotionPath>) {
    match session.comparison_progress() {
        Some(progress) => {
            ui.horizontal(|ui| {
                ui.add(egui::ProgressBar::new(progress).desired_width(120.0));
                if ui.button(tr!(lang, Cancel)).clicked() {
                    session.cancel_comparison();
                }
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        None => {
            if ui.button(tr!(lang, CmdCompare)).clicked() {
                session.compare(path, 5);
            }
        }
    }
    let Some((rows, current)) = session.comparison(path) else { return };
    if !current {
        ui.weak(tr!(lang, CompareStale));
    }
    egui::Grid::new(("gc_comparison", path.raw())).striped(true).show(ui, |ui| {
        for title in
            [tr!(lang, PropPlanner), tr!(lang, ColSuccess), tr!(lang, ColDuration), tr!(lang, ColChecks)]
        {
            ui.label(RichText::new(title).strong());
        }
        ui.end_row();
        for row in rows {
            let name = PLANNERS
                .iter()
                .position(|(id, _)| *id == row.planner)
                .map_or(row.planner, |i| PLANNER_NAMES[i]);
            ui.label(name);
            ui.label(format!("{}/{}", row.successes, row.runs));
            ui.label(row.duration.map_or_else(|| "-".to_owned(), |d| format!("{d:.2}")));
            ui.label(row.state_checks.to_string());
            ui.end_row();
        }
    });
}
