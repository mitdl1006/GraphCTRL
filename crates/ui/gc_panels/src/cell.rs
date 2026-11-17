//! Robot-cell panels: jogging, body placement and path planning.

use egui::{RichText, Ui};
use gc_doc::{CellOp, Command, Solid};
use gc_i18n::{Lang, tr};
use gc_session::{PLANNERS, PlanStatus, Session};
use gc_ui::colors;

/// Display names of the planners, in the order of [`PLANNERS`].
const PLANNER_NAMES: [&str; 5] = ["RRT-Connect", "RRT", "RRT*", "Informed RRT*", "PRM"];

/// The width of a number field, so that rows of X, Y, Z line up.
const FIELD_WIDTH: f32 = 66.0;

/// State of the cell panels that lives between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellUi {
    /// True while the planned motion is being played.
    pub playing: bool,
    /// The path the panel and the ribbon work on; the newest one when unset or deleted.
    pub path: Option<gc_core::Id<gc_doc::MotionPath>>,
    /// The work frame new waypoints are taught in; the newest one when unset or deleted.
    pub frame: Option<gc_core::Id<gc_doc::WorkFrame>>,
    /// Set when the user asked to load a robot's tool from a mesh file: the application shows the file
    /// dialog and takes this back.
    pub tool_mesh: Option<gc_core::Id<gc_doc::Robot>>,
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
            let kind = match item {
                CellItem::Body(_) => tr!(lang, BrowserBodies),
                CellItem::Robot(_) => tr!(lang, BrowserRobots),
                CellItem::Waypoint(_) => tr!(lang, BrowserWaypoints),
                CellItem::Frame(_) => tr!(lang, BrowserFrames),
                CellItem::Path(_) => tr!(lang, BrowserPaths),
            };
            gc_ui::panel::title(ui, kind, &name);
        }
    } else if !session.doc().robots().is_empty() || !session.doc().bodies().is_empty() {
        ui.add_space(4.0);
        gc_ui::panel::note(ui, tr!(lang, PropCellOverview));
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
        acm(ui, session, lang, robot);
        tool(ui, session, lang, robot, state);
    }
    match focus {
        None => bodies(ui, session, lang, None),
        Some(CellItem::Body(id)) => bodies(ui, session, lang, Some(id)),
        Some(_) => {}
    }
    if matches!(focus, None | Some(CellItem::Frame(_))) {
        frames(ui, session, lang, state, focus);
    }
    if let Some(CellItem::Waypoint(id)) = focus {
        ui.add_space(8.0);
        crate::pose::waypoint_place(ui, session, lang, id, true);
        taught(ui, session, lang, id);
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
    gc_ui::panel::section(ui, tr!(lang, PanelJog));
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
            .custom_formatter(|value, _| format!("{value:.1}"))
            .suffix("°");
        // One row per joint: its name in the label column, the slider filling the rest.
        ui.horizontal(|ui| {
            let label = gc_ui::panel::fixed_label(ui, 112.0, name);
            // Room for the value box beside the slider, so that the row never widens the panel.
            ui.spacing_mut().slider_width = (ui.available_width() - 84.0).clamp(60.0, 260.0);
            ui.add(slider).labelled_by(label.id);
        });
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

/// A waypoint taught as joint values: the values in degrees, editable, and a button that stores the
/// robot's current pose instead. A waypoint on a sketch point says so; it moves with the sketch.
fn taught(ui: &mut Ui, session: &mut Session, lang: Lang, waypoint: gc_core::Id<gc_doc::Waypoint>) {
    let Some(target) = session.shown_target(waypoint) else { return };
    let Some(robot) = session.doc().get(waypoint).map(|w| w.robot) else { return };
    match target {
        gc_doc::WaypointTarget::Joints(mut q) => {
            gc_ui::panel::section(ui, tr!(lang, PropTaughtJoints));
            let names: Vec<String> = session
                .robot_model(robot)
                .map(|loaded| loaded.chain.joint_names().to_vec())
                .unwrap_or_default();
            let mut changed = false;
            for (index, value) in q.iter_mut().enumerate() {
                let name = names.get(index).cloned().unwrap_or_else(|| format!("J{}", index + 1));
                ui.horizontal(|ui| {
                    let label = gc_ui::panel::fixed_label(ui, 112.0, &name);
                    let mut shown = value.to_degrees() + 0.0;
                    let drag = egui::DragValue::new(&mut shown).speed(1.0).suffix("°").max_decimals(3);
                    if ui
                        .add_sized([90.0, ui.spacing().interact_size.y], drag)
                        .labelled_by(label.id)
                        .changed()
                    {
                        *value = shown.to_radians();
                        changed = true;
                    }
                });
            }
            if ui.button(tr!(lang, CmdTeachAgain)).clicked()
                && let Some(now) = session.robot_joints(robot)
            {
                q = now;
                changed = true;
            }
            if changed {
                session.preview_target(waypoint, gc_doc::WaypointTarget::Joints(q));
            }
        }
        gc_doc::WaypointTarget::SketchPoint { sketch, .. } => {
            let name = session.doc().get(sketch).map(|s| s.name.clone()).unwrap_or_default();
            ui.add_space(8.0);
            ui.weak(tr!(lang, PropOnSketchPoint, name));
        }
        gc_doc::WaypointTarget::InFrame { .. }
        | gc_doc::WaypointTarget::InWorld { .. }
        | gc_doc::WaypointTarget::InBase { .. } => {}
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
    // How the configurations were found: the closed form of the arm's family, or numerically.
    let how = match &ghost.analytic {
        Some(family) => tr!(lang, IkAnalytic, family, ghost.solutions),
        None => tr!(lang, IkNumeric, ghost.solutions),
    };
    gc_ui::panel::note(ui, &how);
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
    // Adding zero turns -0 into 0, so that a value is never shown as "-0".
    let mut shown = value.to_degrees() + 0.0;
    let drag = egui::DragValue::new(&mut shown).speed(1.0).prefix(format!("{name} "));
    let changed = ui.add_sized([FIELD_WIDTH, ui.spacing().interact_size.y], drag).changed();
    if changed {
        *value = shown.to_radians();
    }
    changed
}

/// Position and rotation of a placement. Returns true when the user changed something.
pub(crate) fn placement(ui: &mut Ui, lang: Lang, placement: &mut gc_doc::Placement) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        gc_ui::panel::label(ui, tr!(lang, PropPosition));
        for (value, name) in placement.xyz.iter_mut().zip(["X", "Y", "Z"]) {
            changed |= millimetres(ui, value, name, -100_000.0);
        }
    });
    ui.horizontal(|ui| {
        gc_ui::panel::label(ui, tr!(lang, PropRotation));
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
    let mut mm = *value * 1000.0 + 0.0;
    let drag = egui::DragValue::new(&mut mm).speed(1.0).range(least..=100_000.0).prefix(format!("{name} "));
    let changed = ui.add_sized([FIELD_WIDTH, ui.spacing().interact_size.y], drag).changed();
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
        let selected = label(*chosen);
        gc_ui::panel::combo_row(ui, ("gc_frame_axis", id.raw(), salt), title, &selected, |ui| {
            ui.selectable_value(chosen, None, tr!(lang, PropSketchAxes));
            for (point, text) in &points {
                ui.selectable_value(chosen, Some(*point), text);
            }
        });
    }
    if (x_point, xy_point) != (frame.x_point, frame.xy_point) {
        // A refusal is explained in the status bar.
        drop(session.set_frame_axes(id, x_point, xy_point));
    }
}

/// The six numbers of a frame's pose: in the world and in the first robot's base frame.
fn frame_pose(ui: &mut Ui, session: &Session, lang: Lang, id: gc_core::Id<gc_doc::WorkFrame>) {
    let Some(world) = session.work_frame_pose(id) else { return };
    let mut rows = vec![(tr!(lang, PropPoseWorld).to_owned(), world)];
    if let Some((robot, _)) = session.doc().robots().iter().next()
        && let Some(base) = session.robot_base_frame(robot)
    {
        rows.push((tr!(lang, PropPoseBase).to_owned(), base.inverse() * world));
    }
    crate::pose::pose_rows(ui, lang, ("gc_frame_pose", id.raw()), &rows);
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
    gc_ui::panel::section(ui, tr!(lang, PanelFrames));
    if let Some(active) = state.active_frame(session) {
        let mut chosen = active;
        let name = frames.iter().find(|(id, _)| *id == active).map(|(_, n)| n.clone()).unwrap_or_default();
        let selected = tr!(lang, PropTeachFrame, name);
        gc_ui::panel::combo_row(ui, "gc_teach_frame", tr!(lang, BrowserFrames), &selected, |ui| {
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
    // With nothing selected, the waypoints given as poses, each under what it is measured from.
    if focus.is_none() {
        let waypoints: Vec<_> = session
            .doc()
            .waypoints()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| session.waypoint_reference(*id).is_some())
            .collect();
        for id in waypoints {
            crate::pose::waypoint_place(ui, session, lang, id, false);
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
fn tool_end(session: &Session, tool: &gc_doc::Tool) -> gc_doc::Placement {
    let half = match &tool.solid {
        Solid::Cylinder { length, .. } => length / 2.0,
        Solid::Box { size } => size[2] / 2.0,
        Solid::Sphere { radius } => *radius,
        // A mesh: the top of its box, which is where most tool models point.
        Solid::Mesh { .. } => session.body_mesh(&tool.solid).map_or(0.0, |mesh| mesh.bounds.max.z),
    };
    let pose = gc_math::pose_from_xyz_rpy(tool.placement.xyz, tool.placement.rpy);
    let end = pose.transform_point(gc_math::DVec3::new(0.0, 0.0, half));
    gc_doc::Placement { xyz: end.to_array(), rpy: tool.placement.rpy }
}

/// What the robot carries on its flange: its shape, size and place. It is drawn with the robot and
/// checked for collisions like a link.
fn tool(
    ui: &mut Ui,
    session: &mut Session,
    lang: Lang,
    robot: gc_core::Id<gc_doc::Robot>,
    state: &mut CellUi,
) {
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
        let selected = kinds.get(current).copied().unwrap_or(tr!(lang, ToolMesh));
        gc_ui::panel::combo_row(ui, "gc_tool_shape", tr!(lang, PropToolShape), selected, |ui| {
            for (index, name) in kinds.iter().enumerate() {
                ui.selectable_value(&mut chosen, index, *name);
            }
        });
        if ui.button(tr!(lang, ToolFromFile)).clicked() {
            state.tool_mesh = Some(robot);
        }
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
    if tcp_to_end && let Some(end) = tool.as_ref().map(|t| tool_end(session, t)) {
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
    gc_ui::panel::section(ui, tr!(lang, PanelBodies));
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
    gc_ui::panel::section(ui, tr!(lang, PanelPath));
    // With several paths, this chooses the one the panel and the ribbon work on.
    let names: Vec<_> = session.doc().paths().iter().map(|(id, p)| (id, p.name.clone())).collect();
    if names.len() > 1 {
        let mut chosen = path_id;
        gc_ui::panel::combo_row(ui, "gc_path_choice", tr!(lang, BrowserPaths), &path.name, |ui| {
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
    }
    let mut planner = path.planner.clone();
    let mut seed = path.seed;
    let shown = PLANNERS
        .iter()
        .position(|(id, _)| *id == planner)
        .map_or(planner.clone(), |i| PLANNER_NAMES[i].to_owned());
    gc_ui::panel::combo_row(ui, "gc_planner", tr!(lang, PropPlanner), &shown, |ui| {
        for ((id, _), name) in PLANNERS.iter().zip(PLANNER_NAMES) {
            ui.selectable_value(&mut planner, (*id).to_owned(), name);
        }
    });
    ui.horizontal(|ui| {
        let label = gc_ui::panel::label(ui, tr!(lang, PropSeed));
        ui.add(egui::DragValue::new(&mut seed).speed(0.1)).labelled_by(label.id);
    });
    if planner != path.planner || seed != path.seed {
        // A refusal is shown in the status bar.
        drop(session.execute(&Command::Cell(CellOp::SetPlanner { id: path_id, planner, seed })));
    }
    gc_ui::panel::section(ui, tr!(lang, PropSteps));
    steps(ui, session, lang, path_id, &path);
    tcp_speed(ui, session, lang, path_id, path.tcp_speed);
    let status = session.plan_status(path_id);
    let color = match status {
        PlanStatus::Ready => colors::TRACE,
        PlanStatus::Failed => colors::ERROR,
        PlanStatus::Stale | PlanStatus::NotPlanned | PlanStatus::Planning => colors::INACTIVE,
    };
    gc_ui::panel::section(ui, tr!(lang, PropResult));
    gc_ui::panel::chip(ui, status_text(lang, status), color);
    if let Some(error) = session.plan_error(path_id) {
        ui.colored_label(colors::ERROR, error);
    }
    comparison(ui, session, lang, path_id);
    let Some(planned) = session.planned(path_id) else {
        state.playing = false;
        return;
    };
    let duration = planned.duration();
    gc_ui::panel::card(ui, |ui| {
        ui.label(tr!(lang, PlanDuration, format!("{duration:.2}")));
        ui.label(tr!(lang, PlanPeakTcp, format!("{:.0}", planned.peak_tcp_speed() * 1000.0)));
        let clearance = planned.min_clearance();
        if clearance.is_finite() {
            ui.label(tr!(lang, PlanClearance, format!("{:.1}", clearance * 1000.0)));
        }
        if let Some(g) = planned.guarantee() {
            let numbers = [g.padding_nominal, g.padding_min, g.step_mm, g.step_deg, g.clearance];
            guarantee_note(ui, lang, g.holds_between_states(), numbers);
        }
        if planned.segments.iter().all(|s| s.straight) {
            gc_ui::panel::note(ui, tr!(lang, PlanStraight));
        } else {
            gc_ui::panel::note(ui, &tr!(lang, PlanChecks, planned.state_checks()));
        }
    });
    contacts(ui, session, lang, path_id, &path);
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
    gc_ui::panel::combo_row(ui, "gc_step_add", tr!(lang, StepAdd), "", |ui| {
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

/// The speed limit of the tool tip on a path's straight moves, like a UR program's linear speed. The
/// value is stored when the user lets go of it.
fn tcp_speed(
    ui: &mut Ui,
    session: &mut Session,
    lang: Lang,
    path: gc_core::Id<gc_doc::MotionPath>,
    stored: Option<f64>,
) {
    let mut limited = stored.is_some();
    let id = ui.scope_id().with(("gc_tcp_speed", path.raw()));
    let mut speed: f64 =
        ui.data(|d| d.get_temp(id)).unwrap_or_else(|| stored.unwrap_or(DEFAULT_TCP_SPEED) * 1000.0);
    let mut commit = false;
    ui.horizontal(|ui| {
        commit |= ui.checkbox(&mut limited, tr!(lang, PropTcpLimit)).changed();
        if limited {
            let response = ui.add(egui::DragValue::new(&mut speed).range(1.0..=5000.0).suffix(" mm/s"));
            if response.changed() {
                ui.data_mut(|d| d.insert_temp(id, speed));
            }
            commit |= response.drag_stopped()
                || response.lost_focus()
                || (response.changed() && !response.dragged());
        }
    });
    if commit {
        ui.data_mut(|d| d.remove::<f64>(id));
        let speed = limited.then_some(speed / 1000.0);
        if speed != stored {
            drop(session.execute(&Command::Cell(CellOp::SetTcpSpeed { id: path, speed })));
        }
    }
}

/// The limit offered when a path's tool speed is first limited, m/s.
const DEFAULT_TCP_SPEED: f64 = 0.25;

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

/// What the final checks of a path establish, in words.
///
/// `numbers`: nominal padding (m), smallest padding (m), step (mm), step (degrees), clearance (m).
fn guarantee_note(ui: &mut Ui, lang: Lang, holds: bool, numbers: [f64; 5]) {
    let [padding, padding_min, step_mm, step_deg, clearance] = numbers;
    if holds {
        gc_ui::panel::note(
            ui,
            &tr!(
                lang,
                PlanGuarantee,
                format!("{:.1}", padding * 1000.0),
                format!("{step_mm:.1}"),
                format!("{step_deg:.1}"),
                format!("{:.1}", clearance * 1000.0)
            ),
        );
    } else {
        gc_ui::panel::note(ui, tr!(lang, PlanGuaranteeSampled));
    }
    if padding_min < padding {
        gc_ui::panel::note(ui, tr!(lang, PlanGuaranteeContact));
    }
    gc_ui::panel::note(ui, tr!(lang, PlanGuaranteeExcluded));
}

/// The pairs of links of a robot and whether each is checked for collisions; switching one changes the
/// plan's guarantee, so a plan made before becomes out of date.
fn acm(ui: &mut Ui, session: &mut Session, lang: Lang, robot: gc_core::Id<gc_doc::Robot>) {
    let Some(pairs) = session.link_pairs(robot) else { return };
    let mut toggled = None;
    egui::CollapsingHeader::new(tr!(lang, PanelAcm)).id_salt("gc_robot_acm").show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for pair in &pairs {
            let name = tr!(lang, AcmPair, pair.a, pair.b);
            // The reason is matched by its name: gc_session does not re-export `PairReason` yet.
            let reason = match format!("{:?}", pair.reason).as_str() {
                "Adjacent" => tr!(lang, AcmAdjacent),
                "TouchingAtZero" => tr!(lang, AcmTouching),
                _ => tr!(lang, AcmChecked),
            };
            ui.horizontal(|ui| {
                let mut checked = pair.checked;
                let check = ui.checkbox(&mut checked, &name);
                if check.changed() {
                    toggled = Some((pair.a.clone(), pair.b.clone(), checked));
                }
                ui.label(RichText::new(reason).size(11.0).color(colors::MUTED));
            });
        }
    });
    if let Some((a, b, checked)) = toggled {
        // A refusal is shown in the status bar.
        drop(session.set_pair_checked(robot, &a, &b, checked));
    }
}

/// The contact allowances of a path: where a link may touch an obstacle that is otherwise kept apart.
fn contacts(
    ui: &mut Ui,
    session: &mut Session,
    lang: Lang,
    path_id: gc_core::Id<gc_doc::MotionPath>,
    path: &gc_doc::MotionPath,
) {
    let bodies: Vec<_> = session.doc().bodies().iter().map(|(id, b)| (id, b.name.clone())).collect();
    let links: Vec<String> = {
        let mut names: Vec<String> =
            session.link_pairs(path.robot).unwrap_or_default().into_iter().flat_map(|p| [p.a, p.b]).collect();
        names.sort();
        names.dedup();
        names
    };
    if bodies.is_empty() || links.is_empty() {
        return;
    }
    gc_ui::panel::section(ui, tr!(lang, PanelContacts));
    let mut edited = path.contacts.clone();
    let mut remove = None;
    for (i, contact) in edited.iter_mut().enumerate() {
        let body = bodies.iter().find(|(id, _)| *id == contact.obstacle).map_or("?", |(_, n)| n.as_str());
        let text = tr!(lang, ContactRow, contact.index + 1, contact.link, body);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&text).size(12.0));
            let delete = tr!(lang, CmdDeleteItem, text);
            let cross = ui.add(egui::Button::new("×").frame(false)).on_hover_text(&delete);
            cross.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &delete));
            if cross.clicked() {
                remove = Some(i);
            }
        });
        ui.checkbox(&mut contact.whole_motion, tr!(lang, ContactWhole));
        ui.checkbox(&mut contact.allow_touch, tr!(lang, ContactTouch));
    }
    if let Some(i) = remove {
        edited.remove(i);
    }
    if edited != path.contacts {
        drop(session.execute(&Command::Cell(CellOp::SetContacts { id: path_id, contacts: edited })));
        return;
    }
    // The form that adds one: which move, which link, which obstacle.
    let key = egui::Id::unique(("gc_contact_form", path_id.raw()));
    let (mut step, mut link, mut body): (usize, usize, usize) =
        ui.data(|d| d.get_temp(key)).unwrap_or_default();
    step = step.min(path.waypoints.len().saturating_sub(1));
    link = link.min(links.len() - 1);
    body = body.min(bodies.len() - 1);
    gc_ui::panel::combo_row(
        ui,
        ("gc_contact_move", path_id.raw()),
        tr!(lang, ContactMove),
        &(step + 1).to_string(),
        |ui| {
            for i in 0..path.waypoints.len() {
                ui.selectable_value(&mut step, i, (i + 1).to_string());
            }
        },
    );
    gc_ui::panel::combo_row(
        ui,
        ("gc_contact_link", path_id.raw()),
        tr!(lang, ContactLink),
        &links[link],
        |ui| {
            for (i, name) in links.iter().enumerate() {
                ui.selectable_value(&mut link, i, name);
            }
        },
    );
    gc_ui::panel::combo_row(
        ui,
        ("gc_contact_body", path_id.raw()),
        tr!(lang, ContactObstacle),
        &bodies[body].1,
        |ui| {
            for (i, (_, name)) in bodies.iter().enumerate() {
                ui.selectable_value(&mut body, i, name);
            }
        },
    );
    ui.data_mut(|d| d.insert_temp(key, (step, link, body)));
    if ui.button(tr!(lang, ContactAdd)).clicked() && !path.waypoints.is_empty() {
        // A refusal is shown in the status bar.
        drop(session.allow_contact(path_id, step, &links[link], bodies[body].0));
    }
}
