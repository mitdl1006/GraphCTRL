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
    bodies(ui, session, lang);
    if !pointer_down && !ui.memory(|m| m.focused().is_some()) {
        // The previews become one undo step each when the mouse is released.
        session.end_jog();
        session.end_body_preview();
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
            let mut millimetres = |ui: &mut Ui, value: &mut f64, name: &str, least: f64| {
                let mut mm = *value * 1000.0;
                let drag = egui::DragValue::new(&mut mm)
                    .speed(1.0)
                    .range(least..=100_000.0)
                    .prefix(format!("{name} "));
                if ui.add(drag).changed() {
                    *value = mm / 1000.0;
                    changed = true;
                }
            };
            ui.label(tr!(lang, PropPosition));
            ui.horizontal(|ui| {
                for (value, name) in body.placement.xyz.iter_mut().zip(["X", "Y", "Z"]) {
                    millimetres(ui, value, name, -100_000.0);
                }
            });
            ui.label(tr!(lang, PropSize));
            ui.horizontal(|ui| match &mut body.solid {
                Solid::Box { size } => {
                    for (value, name) in size.iter_mut().zip(["X", "Y", "Z"]) {
                        millimetres(ui, value, name, 1.0);
                    }
                }
                Solid::Cylinder { radius, length } => {
                    millimetres(ui, radius, "R", 1.0);
                    millimetres(ui, length, "L", 1.0);
                }
                Solid::Sphere { radius } => millimetres(ui, radius, "R", 1.0),
            });
        });
        if changed {
            session.preview_body(id, body);
        }
    }
}

fn paths(ui: &mut Ui, session: &mut Session, lang: Lang, state: &mut CellUi) {
    let Some((path_id, path)) = session.doc().paths().iter().last().map(|(id, p)| (id, p.clone())) else {
        state.playing = false;
        return;
    };
    ui.add_space(8.0);
    ui.label(RichText::new(format!("{}: {}", tr!(lang, PanelPath), path.name)).strong());
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
