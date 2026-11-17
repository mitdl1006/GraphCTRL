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
    use gc_session::CellItem;
    let pointer_down = ui.input(|i| i.pointer.any_down());
    // As in CAD programs, the panel shows the selected item; with nothing selected, the whole cell.
    let focus = session.selected_item();
    if let Some(item) = focus {
        if let Some(name) = session.item_name(item) {
            ui.add_space(6.0);
            ui.label(RichText::new(name).heading().color(colors::SELECTED));
        }
    } else if !session.doc().robots().is_empty() || !session.doc().bodies().is_empty() {
        ui.add_space(6.0);
        ui.weak(tr!(lang, PropCellOverview));
    }
    let first_robot = session.doc().robots().iter().next().map(|(id, _)| id);
    let robot = match focus {
        Some(CellItem::Robot(id)) => Some(id),
        None => first_robot,
        Some(_) => None,
    };
    if let Some(robot) = robot {
        jog(ui, session, lang, robot);
        base(ui, session, lang, robot);
        tool(ui, session, lang, robot);
    }
    match focus {
        None => bodies(ui, session, lang, None),
        Some(CellItem::Body(id)) => bodies(ui, session, lang, Some(id)),
        Some(_) => {}
    }
    if matches!(focus, None | Some(CellItem::Frame(_) | CellItem::Waypoint(_))) {
        frames(ui, session, lang, state, focus);
    }
    if let Some(CellItem::Waypoint(id)) = focus {
        reach(ui, session, lang, id);
    }
    if !pointer_down && !ui.memory(|m| m.focused().is_some()) {
        // The previews become one undo step each when the mouse is released.
        session.end_jog();
        session.end_base_preview();
        session.end_tcp_preview();
        session.end_tool_preview();
        session.end_body_preview();
        session.end_target_preview();
        session.end_frame_preview();
        session.end_look_preview();
    }
    match focus {
        None => paths(ui, session, lang, state),
        Some(CellItem::Path(id)) => {
            state.path = Some(id);
            paths(ui, session, lang, state);
        }
        Some(_) => {}
    }
}

fn jog(ui: &mut Ui, session: &mut Session, lang: Lang, robot: gc_core::Id<gc_doc::Robot>) {
    let (Some(loaded), Some(mut joints)) = (session.robot_model(robot).cloned(), session.robot_joints(robot))
    else {
        return;
    };
    ui.add_space(8.0);
    ui.label(RichText::new(tr!(lang, PanelJog)).strong());
    transparency(ui, session, lang, gc_session::Shown::Robot(robot));
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

/// Whether the robot reaches a waypoint, and how it would stand there (shown as a ghost).
fn reach(ui: &mut Ui, session: &Session, lang: Lang, waypoint: gc_core::Id<gc_doc::Waypoint>) {
    let Some(ghost) = session.waypoint_ghost(waypoint) else { return };
    ui.add_space(8.0);
    match (&ghost.joints, &ghost.problem) {
        (None, _) => {
            ui.colored_label(colors::ERROR, tr!(lang, ReachOut));
        }
        (Some(_), Some(problem)) => {
            ui.colored_label(colors::ERROR, tr!(lang, ReachProblem, problem_text(lang, problem)));
        }
        (Some(joints), None) => {
            ui.label(tr!(lang, ReachOk));
            let degrees: Vec<String> = joints.iter().map(|q| format!("{:.1}°", q.to_degrees())).collect();
            ui.weak(degrees.join("  "));
        }
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
fn base(ui: &mut Ui, session: &mut Session, lang: Lang, robot: gc_core::Id<gc_doc::Robot>) {
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

/// How see-through a body or robot is, previewed while the slider moves.
fn transparency(ui: &mut Ui, session: &mut Session, lang: Lang, item: gc_session::Shown) {
    let Some(mut look) = session.shown_look(item) else { return };
    let mut percent = look.transparency * 100.0;
    let slider = egui::Slider::new(&mut percent, 0.0..=95.0)
        .text(tr!(lang, PropTransparency))
        .suffix(" %")
        .custom_formatter(|value, _| format!("{value:.0}"));
    if ui.add(slider).changed() {
        look.transparency = (percent / 100.0).clamp(0.0, 0.95);
        session.preview_look(item, look);
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

/// The points that set a frame's axes, chosen among the points of its sketch.
fn frame_axes(
    ui: &mut Ui,
    session: &mut Session,
    lang: Lang,
    id: gc_core::Id<gc_doc::WorkFrame>,
    frame: &gc_doc::WorkFrame,
) {
    let Some(sketch) = session.doc().get(frame.sketch) else { return };
    let mm = |v: f64| format!("{:.1}", v * 1000.0);
    let points: Vec<(gc_core::Id<gc_sketch::SkPoint>, String)> = sketch
        .points
        .iter()
        .filter(|(point, _)| **point != frame.point)
        .map(|(point, p)| (*point, format!("({}, {}, {})", mm(p.pos[0]), mm(p.pos[1]), mm(p.pos[2]))))
        .collect();
    let label = |chosen: Option<gc_core::Id<gc_sketch::SkPoint>>| {
        chosen
            .and_then(|c| points.iter().find(|(p, _)| *p == c))
            .map_or_else(|| tr!(lang, PropSketchAxes).to_owned(), |(_, text)| text.clone())
    };
    let (mut x_point, mut xy_point) = (frame.x_point, frame.xy_point);
    for (title, chosen, salt) in
        [(tr!(lang, PropXPoint), &mut x_point, "x"), (tr!(lang, PropXyPoint), &mut xy_point, "xy")]
    {
        egui::ComboBox::new(("gc_frame_axis", id.raw(), salt), title).selected_text(label(*chosen)).show_ui(
            ui,
            |ui| {
                ui.selectable_value(chosen, None, tr!(lang, PropSketchAxes));
                for (point, text) in &points {
                    ui.selectable_value(chosen, Some(*point), text);
                }
            },
        );
    }
    if (x_point, xy_point) != (frame.x_point, frame.xy_point) {
        // A refusal is explained in the status bar.
        drop(session.set_frame_axes(id, x_point, xy_point));
    }
}

/// The six numbers of a frame's pose: in the world and in the first robot's base frame.
fn frame_pose(ui: &mut Ui, session: &Session, lang: Lang, id: gc_core::Id<gc_doc::WorkFrame>) {
    let Some(world) = session.work_frame_pose(id) else { return };
    let mut rows = vec![(tr!(lang, PropPoseWorld), world)];
    if let Some((robot, _)) = session.doc().robots().iter().next()
        && let Some(base) = session.shown_base(robot)
    {
        let base = gc_math::pose_from_xyz_rpy(base.xyz, base.rpy);
        rows.push((tr!(lang, PropPoseBase), base.inverse() * world));
    }
    egui::Grid::new(("gc_frame_pose", id.raw())).show(ui, |ui| {
        for (title, pose) in rows {
            // Values that round to zero are shown as 0, not -0.
            let clean = |v: f64| if v.abs() < 5e-7 { 0.0 } else { v };
            let t = (pose.translation * 1000.0).to_array().map(clean);
            let t = gc_math::DVec3::from_array(t);
            let [r, p, y] = gc_math::rpy_from_quat(pose.rotation).map(|a| clean(a.to_degrees()));
            let v =
                gc_math::DVec3::from_array(gc_math::rotvec_from_quat(pose.rotation).to_array().map(clean));
            ui.label(title);
            ui.label(format!("X {:.1}  Y {:.1}  Z {:.1} mm", t.x, t.y, t.z));
            ui.end_row();
            ui.label("");
            ui.label(format!("R {r:.2}°  P {p:.2}°  Y {y:.2}°"));
            ui.end_row();
            ui.label(tr!(lang, PropRotvec));
            ui.label(format!("Rx {:.4}  Ry {:.4}  Rz {:.4} rad", v.x, v.y, v.z));
            ui.end_row();
        }
    });
}

/// The work frames (which one new waypoints are taught in, and how each is turned) and the waypoints
/// defined in them, whose positions are edited here.
fn frames(
    ui: &mut Ui,
    session: &mut Session,
    lang: Lang,
    state: &mut CellUi,
    focus: Option<gc_session::CellItem>,
) {
    use gc_session::CellItem;
    let shows = |item: CellItem| focus.is_none_or(|f| f == item);
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
    for (id, name) in frames.iter().filter(|(id, _)| shows(CellItem::Frame(*id))) {
        let Some(mut frame) = session.shown_frame(*id) else { continue };
        let mut changed = false;
        egui::CollapsingHeader::new(name)
            .id_salt(("gc_frame", id.raw()))
            .open(focus.is_some().then_some(true))
            .show(ui, |ui| {
                frame_axes(ui, session, lang, *id, &frame);
                frame_pose(ui, session, lang, *id);
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
    for (id, name) in waypoints.into_iter().filter(|(id, _)| shows(CellItem::Waypoint(*id))) {
        let Some(gc_doc::WaypointTarget::InFrame { frame, mut offset }) = session.shown_target(id) else {
            continue;
        };
        let frame_name = session.doc().get(frame).map(|f| f.name.clone()).unwrap_or_default();
        let mut changed = false;
        egui::CollapsingHeader::new(tr!(lang, PropInFrame, name, frame_name))
            .id_salt(("gc_in_frame", id.raw()))
            .open(focus.is_some().then_some(true))
            .show(ui, |ui| changed = placement(ui, lang, &mut offset));
        if changed {
            session.preview_target(id, gc_doc::WaypointTarget::InFrame { frame, offset });
        }
    }
}

/// The size of a box, cylinder or sphere, or the scale of a mesh. Returns true when the user changed it.
fn solid_size(ui: &mut Ui, session: &Session, lang: Lang, solid: &mut Solid) -> bool {
    let mut changed = false;
    match solid {
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
    changed
}

/// The shapes a tool can be made of here, with a size that suits a small gripper or torch, centred so
/// that it starts at the flange.
fn tool_of(kind: usize) -> Option<gc_doc::Tool> {
    let along_z = |solid, half: f64| gc_doc::Tool {
        solid,
        placement: gc_doc::Placement { xyz: [0.0, 0.0, half], rpy: [0.0; 3] },
    };
    match kind {
        1 => Some(along_z(Solid::Cylinder { radius: 0.03, length: 0.15 }, 0.075)),
        2 => Some(along_z(Solid::Box { size: [0.08, 0.08, 0.12] }, 0.06)),
        3 => Some(along_z(Solid::Sphere { radius: 0.04 }, 0.04)),
        _ => None,
    }
}

/// Where the far end of a tool is, along its own Z axis, in the flange's frame.
fn tool_end(tool: &gc_doc::Tool) -> gc_doc::Placement {
    let half = match tool.solid {
        Solid::Cylinder { length, .. } => length / 2.0,
        Solid::Box { size } => size[2] / 2.0,
        Solid::Sphere { radius } => radius,
        Solid::Mesh { .. } => 0.0,
    };
    let pose = gc_math::pose_from_xyz_rpy(tool.placement.xyz, tool.placement.rpy);
    let end = pose.transform_point(gc_math::DVec3::new(0.0, 0.0, half));
    gc_doc::Placement { xyz: end.to_array(), rpy: tool.placement.rpy }
}

/// What the robot carries on its flange: its shape, size and place. It is drawn with the robot and
/// checked for collisions like a link.
fn tool(ui: &mut Ui, session: &mut Session, lang: Lang, robot: gc_core::Id<gc_doc::Robot>) {
    let Some(mut tool) = session.shown_tool(robot) else { return };
    let mut changed = false;
    let mut tcp_to_end = false;
    egui::CollapsingHeader::new(tr!(lang, PanelTool)).id_salt("gc_robot_tool").show(ui, |ui| {
        let kinds = [tr!(lang, ToolNone), tr!(lang, ToolCylinder), tr!(lang, ToolBox), tr!(lang, ToolSphere)];
        let current = match tool.as_ref().map(|t| &t.solid) {
            None => 0,
            Some(Solid::Cylinder { .. }) => 1,
            Some(Solid::Box { .. }) => 2,
            Some(Solid::Sphere { .. }) => 3,
            Some(Solid::Mesh { .. }) => 4,
        };
        let mut chosen = current;
        egui::ComboBox::from_label(tr!(lang, PropToolShape))
            .selected_text(kinds.get(current).copied().unwrap_or(tr!(lang, ToolMesh)))
            .show_ui(ui, |ui| {
                for (index, name) in kinds.iter().enumerate() {
                    ui.selectable_value(&mut chosen, index, *name);
                }
            });
        if chosen != current {
            tool = tool_of(chosen);
            changed = true;
        }
        if let Some(tool) = &mut tool {
            changed |= solid_size(ui, session, lang, &mut tool.solid);
            changed |= placement(ui, lang, &mut tool.placement);
            tcp_to_end = ui.button(tr!(lang, ToolTcpAtEnd)).clicked();
        }
    });
    if tcp_to_end && let Some(end) = tool.as_ref().map(tool_end) {
        session.preview_tcp(robot, end);
    }
    if changed {
        session.preview_tool(robot, tool);
    }
}

fn bodies(ui: &mut Ui, session: &mut Session, lang: Lang, only: Option<gc_core::Id<gc_doc::Body>>) {
    let ids: Vec<_> =
        session.doc().bodies().iter().map(|(id, _)| id).filter(|id| only.is_none_or(|o| o == *id)).collect();
    if ids.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.label(RichText::new(tr!(lang, PanelBodies)).strong());
    for id in ids {
        let Some(mut body) = session.shown_body(id).cloned() else { continue };
        let mut changed = false;
        egui::CollapsingHeader::new(&body.name)
            .id_salt(("gc_body", id.raw()))
            .open(only.is_some().then_some(true))
            .show(ui, |ui| {
                transparency(ui, session, lang, gc_session::Shown::Body(id));
                changed |= placement(ui, lang, &mut body.placement);
                changed |= solid_size(ui, session, lang, &mut body.solid);
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
    steps(ui, session, lang, path_id, &path);
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
/// The waypoints of a path in order, each with how it is approached (freely planned, or in a straight
/// line of the tool tip), buttons to move it up or down or take it out, and a list to add one.
fn steps(
    ui: &mut Ui,
    session: &mut Session,
    lang: Lang,
    path_id: gc_core::Id<gc_doc::MotionPath>,
    path: &gc_doc::MotionPath,
) {
    let mut waypoints = path.waypoints.clone();
    let mut linear: Vec<bool> =
        (0..waypoints.len()).map(|i| path.linear.get(i).copied().unwrap_or(false)).collect();
    let count = waypoints.len();
    let mut changed = false;
    let mut action = None;
    for index in 0..count {
        let Some(name) = waypoints.get(index).and_then(|w| session.doc().get(*w)).map(|w| w.name.clone())
        else {
            continue;
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(index > 0, egui::Button::new(tr!(lang, StepUp)).small()).clicked() {
                action = Some((index, Step::Up));
            }
            if ui.add_enabled(index + 1 < count, egui::Button::new(tr!(lang, StepDown)).small()).clicked() {
                action = Some((index, Step::Down));
            }
            if ui.add_enabled(count > 1, egui::Button::new(tr!(lang, StepRemove)).small()).clicked() {
                action = Some((index, Step::Remove));
            }
            if let Some(value) = linear.get_mut(index) {
                changed |= ui.checkbox(value, tr!(lang, PropLinear, name)).changed();
            }
        });
    }
    match action {
        Some((index, Step::Up)) if index > 0 => {
            waypoints.swap(index, index - 1);
            linear.swap(index, index - 1);
            changed = true;
        }
        Some((index, Step::Down)) if index + 1 < count => {
            waypoints.swap(index, index + 1);
            linear.swap(index, index + 1);
            changed = true;
        }
        Some((index, Step::Remove)) if count > 1 && index < count => {
            waypoints.remove(index);
            linear.remove(index);
            changed = true;
        }
        _ => {}
    }
    // Any waypoint of the path's robot can be added at the end, also one already in the path.
    let mut added = None;
    let candidates: Vec<_> = session
        .doc()
        .waypoints()
        .iter()
        .filter(|(_, w)| w.robot == path.robot)
        .map(|(id, w)| (id, w.name.clone()))
        .collect();
    egui::ComboBox::from_label(tr!(lang, StepAdd)).selected_text("").show_ui(ui, |ui| {
        for (id, name) in candidates {
            if ui.selectable_label(false, name).clicked() {
                added = Some(id);
            }
        }
    });
    if let Some(id) = added {
        waypoints.push(id);
        linear.push(false);
        changed = true;
    }
    if changed {
        // A refusal is shown in the status bar.
        drop(session.execute(&Command::Cell(CellOp::SetPathSteps { id: path_id, waypoints, linear })));
    }
}

/// What a button next to a waypoint of a path does.
#[derive(Clone, Copy)]
enum Step {
    Up,
    Down,
    Remove,
}

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
