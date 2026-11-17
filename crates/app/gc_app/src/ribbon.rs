//! The ribbon: tabs of command groups. Every button is a view of a session command.

use eframe::egui;
use gc_doc::{Placement, Solid, WaypointTarget};
use gc_draw::StandardView;
use gc_i18n::{Lang, Msg, tr};
use gc_panels::CellUi;
use gc_session::Session;
use gc_sketch::Item;
use gc_tools::{ConstraintKind, ToolKind};
use gc_ui::{Icon, ribbon_button, ribbon_group, ribbon_tab};
use gc_viewport::Viewport;

/// The ribbon tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    Model,
    Sketch,
    Robot,
    View,
}

/// The constraint buttons: kind, symbol and name.
/// The constraint buttons of a 3D sketch.
const CONSTRAINTS_3D: [(ConstraintKind, &str, Msg); 8] = [
    (ConstraintKind::Coincident, "::", Msg::ConCoincident),
    (ConstraintKind::Fix, "#", Msg::ConFix),
    (ConstraintKind::AlongX, "X", Msg::ConAlongX),
    (ConstraintKind::AlongY, "Y", Msg::ConAlongY),
    (ConstraintKind::AlongZ, "Z", Msg::ConAlongZ),
    (ConstraintKind::OnXY, "XY", Msg::ConOnXY),
    (ConstraintKind::OnYZ, "YZ", Msg::ConOnYZ),
    (ConstraintKind::OnZX, "ZX", Msg::ConOnZX),
];

const CONSTRAINTS: [(ConstraintKind, &str, Msg); 12] = [
    (ConstraintKind::Coincident, "::", Msg::ConCoincident),
    (ConstraintKind::Collinear, "--", Msg::ConCollinear),
    (ConstraintKind::Concentric, "(o)", Msg::ConConcentric),
    (ConstraintKind::Fix, "#", Msg::ConFix),
    (ConstraintKind::Parallel, "//", Msg::ConParallel),
    (ConstraintKind::Perpendicular, "_|_", Msg::ConPerpendicular),
    (ConstraintKind::Horizontal, "H", Msg::ConHorizontal),
    (ConstraintKind::Vertical, "V", Msg::ConVertical),
    (ConstraintKind::Tangent, "T", Msg::ConTangent),
    (ConstraintKind::Symmetric, "S", Msg::ConSymmetric),
    (ConstraintKind::Equal, "=", Msg::ConEqual),
    (ConstraintKind::Midpoint, "M", Msg::ConMidpoint),
];

/// Something the ribbon asks the application to do because it needs more than the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RibbonAction {
    /// Ask for a mesh file and add it as a body.
    ImportMesh,
    /// Ask for a robot description file and add the robot.
    ImportRobot,
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    tab: &mut Tab,
    session: &mut Session,
    viewport: &mut Viewport,
    cell: &mut CellUi,
    library: &[(String, String)],
    lang: Lang,
) -> Option<RibbonAction> {
    let mut action = None;
    let sketching = session.active_sketch().is_some();
    let spatial = session.sketch_is_spatial();
    ui.horizontal(|ui| {
        for (this, label) in [
            (Tab::Model, Msg::TabModel),
            (Tab::Sketch, Msg::TabSketch),
            (Tab::Robot, Msg::TabRobot),
            (Tab::View, Msg::TabView),
        ] {
            if ribbon_tab(ui, label.text(lang), *tab == this) {
                *tab = this;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.set_min_height(76.0);
        match tab {
            Tab::Model => {
                ribbon_group(ui, tr!(lang, GroupSketch), |ui| {
                    for plane in ["XY", "XZ", "YZ"] {
                        let label = tr!(lang, CmdSketchOn, plane);
                        if ribbon_button(ui, Icon::Sketch, &label, false, !sketching).clicked() {
                            // A failure is shown in the status bar.
                            drop(session.create_sketch(plane));
                            *tab = Tab::Sketch;
                        }
                    }
                    if ribbon_button(ui, Icon::View, tr!(lang, CmdSketch3d), false, !sketching).clicked() {
                        drop(session.create_sketch_3d());
                        *tab = Tab::Sketch;
                    }
                });
            }
            Tab::Sketch => {
                let current = session.tool();
                ribbon_group(ui, tr!(lang, GroupCreate), |ui| {
                    let tools = [
                        (ToolKind::Select, Icon::Select, Msg::ToolSelect),
                        (ToolKind::Line, Icon::Line, Msg::ToolLine),
                        (ToolKind::Circle, Icon::Circle, Msg::ToolCircle),
                        (ToolKind::Arc, Icon::Arc, Msg::ToolArc),
                        (ToolKind::Rectangle, Icon::Rectangle, Msg::ToolRectangle),
                    ];
                    // A 3D sketch has points and lines.
                    let count = if spatial { 2 } else { tools.len() };
                    for (kind, icon, label) in tools.into_iter().take(count) {
                        if ribbon_button(ui, icon, label.text(lang), current == kind, sketching).clicked() {
                            session.set_tool(kind);
                        }
                    }
                });
                ribbon_group(ui, tr!(lang, GroupConstrain), |ui| {
                    let dimension = ToolKind::Dimension;
                    if ribbon_button(
                        ui,
                        Icon::Dimension,
                        tr!(lang, ToolDimension),
                        current == dimension,
                        sketching,
                    )
                    .clicked()
                    {
                        session.set_tool(dimension);
                    }
                    let constraints: &[(ConstraintKind, &str, Msg)] =
                        if spatial { &CONSTRAINTS_3D } else { &CONSTRAINTS };
                    for (kind, symbol, label) in constraints.iter().copied() {
                        let tool = ToolKind::Constraint(kind);
                        if ribbon_button(ui, Icon::Text(symbol), label.text(lang), current == tool, sketching)
                            .clicked()
                        {
                            session.set_tool(tool);
                        }
                    }
                });
                ribbon_group(ui, tr!(lang, GroupExit), |ui| {
                    if ribbon_button(ui, Icon::Finish, tr!(lang, CmdFinishSketch), false, sketching).clicked()
                    {
                        session.finish_sketch();
                        *tab = Tab::Model;
                    }
                });
            }
            Tab::Robot => action = robot_tab(ui, session, viewport, cell, library, lang),
            Tab::View => {
                ribbon_group(ui, tr!(lang, GroupViews), |ui| {
                    for (view, label) in [
                        (StandardView::Iso, Msg::ViewHome),
                        (StandardView::Front, Msg::ViewFront),
                        (StandardView::Top, Msg::ViewTop),
                        (StandardView::Right, Msg::ViewRight),
                    ] {
                        if ribbon_button(ui, Icon::View, label.text(lang), false, true).clicked() {
                            viewport.look(view);
                        }
                    }
                    if ribbon_button(ui, Icon::Rectangle, tr!(lang, ViewFit), false, true).clicked() {
                        viewport.fit(session, [1000.0, 700.0]);
                    }
                });
                ribbon_group(ui, tr!(lang, GroupProjection), |ui| {
                    let ortho = viewport.camera.ortho;
                    if ribbon_button(ui, Icon::View, tr!(lang, ViewOrtho), ortho, true).clicked() {
                        viewport.camera.ortho = true;
                    }
                    if ribbon_button(ui, Icon::View, tr!(lang, ViewPerspective), !ortho, true).clicked() {
                        viewport.camera.ortho = false;
                    }
                });
            }
        }
    });
    action
}

fn robot_tab(
    ui: &mut egui::Ui,
    session: &mut Session,
    viewport: &mut Viewport,
    cell: &mut CellUi,
    library: &[(String, String)],
    lang: Lang,
) -> Option<RibbonAction> {
    let mut action = None;
    let robot = session.doc().robots().iter().next().map(|(id, _)| id);
    let path = cell.active_path(session);
    // Failures of these commands are shown in the status bar.
    ribbon_group(ui, tr!(lang, GroupAdd), |ui| {
        // One robot per cell for now: the panels show the first one.
        for (name, urdf) in library {
            let label = tr!(lang, CmdAddRobot, name);
            if ribbon_button(ui, Icon::View, &label, false, robot.is_none()).clicked() {
                let name = format!("{name}_{}", session.doc().robots().len() + 1);
                if let Err(error) = session.add_robot(&name, urdf) {
                    log::error!("the robot could not be added: {error}");
                }
                viewport.fit(session, [1000.0, 700.0]);
            }
        }
        if ribbon_button(ui, Icon::View, tr!(lang, CmdImportRobot), false, robot.is_none()).clicked() {
            action = Some(RibbonAction::ImportRobot);
        }
        if ribbon_button(ui, Icon::View, tr!(lang, CmdImportMesh), false, true).clicked() {
            action = Some(RibbonAction::ImportMesh);
        }
        let at = |x: f64, z: f64| Placement { xyz: [x, 0.0, z], rpy: [0.0; 3] };
        for (icon, label, solid, placement) in [
            (Icon::Rectangle, Msg::CmdAddBox, Solid::Box { size: [0.1, 0.1, 1.4] }, at(0.55, 0.7)),
            (Icon::Circle, Msg::CmdAddCylinder, Solid::Cylinder { radius: 0.06, length: 0.8 }, at(0.0, 0.4)),
            (Icon::Circle, Msg::CmdAddSphere, Solid::Sphere { radius: 0.1 }, at(0.0, 0.1)),
        ] {
            if ribbon_button(ui, icon, label.text(lang), false, true).clicked() {
                // New bodies are spread out sideways so that they do not hide each other.
                let offset = 0.3 * session.doc().bodies().len() as f64;
                let placement = Placement {
                    xyz: [placement.xyz[0], placement.xyz[1] - offset, placement.xyz[2]],
                    ..placement
                };
                drop(session.add_body(solid, placement));
                viewport.fit(session, [1000.0, 700.0]);
            }
        }
    });
    ribbon_group(ui, tr!(lang, GroupPath), |ui| {
        if ribbon_button(ui, Icon::Text("+P"), tr!(lang, CmdAddWaypoint), false, robot.is_some()).clicked()
            && let Some(robot) = robot
        {
            drop(session.add_waypoint_here(robot));
        }
        // One selected point of the sketch being edited can become a waypoint that follows the sketch.
        let point = match (session.active_sketch(), session.selection().iter().next()) {
            (Some((sketch, _)), Some(Item::Point(point))) if session.selection().len() == 1 => {
                Some((sketch, *point))
            }
            _ => None,
        };
        let enabled = robot.is_some() && point.is_some();
        if ribbon_button(ui, Icon::Text("+S"), tr!(lang, CmdWaypointAtPoint), false, enabled).clicked()
            && let (Some(robot), Some((sketch, point))) = (robot, point)
        {
            drop(session.add_waypoint(robot, WaypointTarget::SketchPoint { sketch, point }));
        }
        // One to three selected points, fully constrained, become a work frame. In the order they were
        // picked: the origin, the point the X axis points to, a point on the positive-Y side of XY.
        let order = session.selection_order();
        let frame_points: Vec<_> = order
            .iter()
            .filter_map(|item| match item {
                Item::Point(point) => Some(*point),
                _ => None,
            })
            .collect();
        let frame_sketch = session
            .active_sketch()
            .map(|(id, _)| id)
            .filter(|_| (1..=3).contains(&frame_points.len()) && frame_points.len() == order.len());
        if ribbon_button(ui, Icon::Text("+F"), tr!(lang, CmdWorkFrame), false, frame_sketch.is_some())
            .clicked()
            && let Some(sketch) = frame_sketch
        {
            // A refusal (a point can still move, or the points give no axes) is explained in the status bar.
            cell.frame = session.add_work_frame_by_points(sketch, &frame_points).ok();
        }
        let frame = cell.active_frame(session);
        let enabled = robot.is_some() && frame.is_some();
        if ribbon_button(ui, Icon::Text("+W"), tr!(lang, CmdWaypointInFrame), false, enabled).clicked()
            && let (Some(robot), Some(frame)) = (robot, frame)
        {
            drop(session.add_waypoint_in_frame(robot, frame));
        }
        let waypoints: Vec<_> = session
            .doc()
            .waypoints()
            .iter()
            .filter(|(_, w)| Some(w.robot) == robot)
            .map(|(id, _)| id)
            .collect();
        if ribbon_button(ui, Icon::Line, tr!(lang, CmdMakePath), false, !waypoints.is_empty()).clicked()
            && let Some(robot) = robot
        {
            // The new path becomes the one the panel works on.
            cell.path = session.add_path(robot, waypoints).ok();
        }
        // The sketch being edited, else the newest one, becomes a path along its lines.
        let sketch = session
            .active_sketch()
            .map(|(id, _)| id)
            .or_else(|| session.doc().sketches().iter().last().map(|(id, _)| id))
            .filter(|id| {
                session.doc().get(*id).is_some_and(|s| s.curves.keys().any(|c| s.line(*c).is_some()))
            });
        let enabled = robot.is_some() && sketch.is_some();
        if ribbon_button(ui, Icon::Sketch, tr!(lang, CmdPathFromSketch), false, enabled).clicked()
            && let (Some(robot), Some(sketch)) = (robot, sketch)
        {
            cell.path = session.path_from_sketch(robot, sketch).ok();
        }
        // While a plan is being computed the same button cancels it.
        let planning = session.is_planning();
        let label = if planning { tr!(lang, Cancel) } else { tr!(lang, CmdPlan) };
        if ribbon_button(ui, Icon::Finish, label, planning, path.is_some()).clicked()
            && let Some(path) = path
        {
            cell.playing = false;
            if planning {
                session.cancel_planning();
            } else {
                session.plan(path);
            }
        }
        let planned = path.is_some_and(|p| session.planned(p).is_some());
        let label = if cell.playing { tr!(lang, CmdStop) } else { tr!(lang, CmdPlay) };
        if ribbon_button(ui, Icon::Text(">"), label, cell.playing, planned).clicked()
            && let Some(path) = path
        {
            cell.playing = !cell.playing;
            if cell.playing {
                session.set_playback(Some((path, 0.0)));
            }
        }
    });
    action
}
