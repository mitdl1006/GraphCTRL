//! The ribbon: tabs of command groups. Every button is a view of a session command.

use eframe::egui;
use gc_doc::{ArcSlotKind, ContactSolver, Placement, SlotKind, Solid, WaypointTarget};
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
    /// Inventor's Assemble tab: place, pattern, relationships, position.
    Assemble,
    Robot,
    Inspect,
    /// Inventor's Manage tab: parameters.
    Manage,
    View,
}

/// The constraint buttons: kind, symbol and name.
/// The constraint buttons of a 3D sketch.
// The 3D Sketch tab's Constrain panel (Inventor 2026 help, To Apply Geometric Constraints,
// GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930): Coincident, Collinear, Parallel, Perpendicular, Equal, Fix, the
// axis and plane alignments; Midpoint as in 2D.
const CONSTRAINTS_3D: [(ConstraintKind, &str, Msg); 13] = [
    (ConstraintKind::Coincident, "::", Msg::ConCoincident),
    (ConstraintKind::Collinear, "--", Msg::ConCollinear),
    (ConstraintKind::Parallel, "//", Msg::ConParallel),
    (ConstraintKind::Perpendicular, "_|_", Msg::ConPerpendicular),
    (ConstraintKind::Equal, "=", Msg::ConEqual),
    (ConstraintKind::Midpoint, "M", Msg::ConMidpoint),
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
    /// Place Component: ask for a file; a body then hangs on the cursor until it is set down.
    Place,
    /// Ask for a robot description file and add the robot.
    ImportRobot,
    /// Open the measure panel.
    StartMeasure,
    /// Open the interference analysis.
    StartInterference,
    /// Open the drive dialog.
    StartDrive,
    /// Open the Assemble command.
    StartAssemble,
    /// Open the Work Plane, Work Axis or Work Point command.
    StartWorkFeature(crate::work_feature::Category),
    /// Open the Constrain dialog.
    StartConstrain,
    /// Open the Joint dialog.
    StartJoint,
    /// Open the Pattern dialog.
    StartPattern,
    /// Open the mirror, copy or replace dialog in a mode.
    StartReplicate(crate::replicate::Mode),
    /// Open the parameters dialog.
    StartParameters,
    /// Project the edges the sketch plane cuts into the active sketch.
    ProjectCutEdges,
    /// Ground or free the selected component.
    ToggleGround,
    /// Open the sketch Mirror dialog.
    StartSketchMirror,
    /// Open the sketch Rectangular Pattern dialog.
    StartSketchRectPattern,
    /// Open the sketch Circular Pattern dialog.
    StartSketchCircPattern,
    /// Open the Auto Dimension dialog.
    StartAutoDimension,
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    tab: &mut Tab,
    session: &mut Session,
    viewport: &mut Viewport,
    cell: &mut CellUi,
    library: &[crate::app::LibraryRobot],
    translucent: &mut bool,
    dark: &mut bool,
    lang: Lang,
) -> Option<RibbonAction> {
    let mut action = None;
    let sketching = session.active_sketch().is_some();
    let spatial = session.sketch_is_spatial();
    ui.horizontal(|ui| {
        for (this, label) in [
            (Tab::Model, Msg::TabModel),
            (Tab::Sketch, Msg::TabSketch),
            (Tab::Assemble, Msg::TabAssemble),
            (Tab::Robot, Msg::TabRobot),
            (Tab::Inspect, Msg::TabInspect),
            (Tab::Manage, Msg::TabManage),
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
                    // One split button: the body starts the sketch made last, the arrow lists the planes
                    // and the 3D sketch.
                    let planes = ["XY", "XZ", "YZ"];
                    let labels: Vec<String> =
                        planes.iter().map(|plane| tr!(lang, CmdSketchOn, plane)).collect();
                    let mut items: Vec<gc_ui::SplitItem<'_>> = labels
                        .iter()
                        .map(|label| gc_ui::SplitItem { icon: Icon::Sketch, label, active: false })
                        .collect();
                    items.push(gc_ui::SplitItem {
                        icon: Icon::Line,
                        label: tr!(lang, CmdSketch3d),
                        active: false,
                    });
                    let more = tr!(lang, SplitMore, tr!(lang, GroupSketch));
                    if let Some(index) =
                        gc_ui::ribbon_split_button(ui, "new_sketch", &more, &items, !sketching)
                    {
                        // A failure is shown in the status bar. A new sketch starts with the line tool, as
                        // drawing is what comes first.
                        let made = match planes.get(index) {
                            Some(plane) => session.create_sketch(plane).is_ok(),
                            None => session.create_sketch_3d().is_ok(),
                        };
                        if made {
                            session.set_tool(ToolKind::Line);
                        }
                        *tab = Tab::Sketch;
                    }
                });
                // Work features: the axis types of an assembly (Inventor 2026 help, To Create Work Features,
                // GUID-38A9748E-FA2D-44D3-928C-DFE1326A9385).
                ribbon_group(ui, tr!(lang, GroupWorkFeatures), |ui| {
                    use crate::work_feature::Category;
                    // The plane is the large button; the axis and the point are stacked beside it.
                    if ribbon_button(ui, Icon::WorkPlane, tr!(lang, CmdWorkPlane), false, !sketching)
                        .clicked()
                    {
                        action = Some(RibbonAction::StartWorkFeature(Category::Plane));
                    }
                    let rows = [
                        gc_ui::StackRow {
                            icon: Icon::WorkAxis,
                            label: tr!(lang, CmdWorkAxis),
                            selected: false,
                            enabled: !sketching,
                        },
                        gc_ui::StackRow {
                            icon: Icon::WorkPoint,
                            label: tr!(lang, CmdWorkPoint),
                            selected: false,
                            enabled: !sketching,
                        },
                    ];
                    match gc_ui::ribbon_stack(ui, &rows) {
                        Some(0) => action = Some(RibbonAction::StartWorkFeature(Category::Axis)),
                        Some(1) => action = Some(RibbonAction::StartWorkFeature(Category::Point)),
                        _ => {}
                    }
                });
            }
            Tab::Sketch => {
                let current = session.tool();
                ribbon_group(ui, tr!(lang, GroupCreate), |ui| {
                    // Select is a plain button; the drawing tools are split buttons whose body runs the
                    // variant used last (Inventor's Create panel; Inventor 2026 help, To Create Lines, Arcs,
                    // and Bends, GUID-4F861674-F74C-474E-869A-4C190ED6EC4B; To Create 2D Shape Geometry,
                    // GUID-D489CE6D-7299-4211-A43A-F3580A4BA357).
                    if ribbon_button(
                        ui,
                        Icon::Select,
                        Msg::ToolSelect.text(lang),
                        current == ToolKind::Select,
                        sketching,
                    )
                    .clicked()
                    {
                        session.set_tool(ToolKind::Select);
                    }
                    let mut split = |ui: &mut egui::Ui,
                                     salt: &str,
                                     group: Msg,
                                     variants: &[(ToolKind, Icon, Msg)]| {
                        let items: Vec<gc_ui::SplitItem<'_>> = variants
                            .iter()
                            .map(|(kind, icon, label)| gc_ui::SplitItem {
                                icon: *icon,
                                label: label.text(lang),
                                active: current == *kind,
                            })
                            .collect();
                        let more = tr!(lang, SplitMore, group.text(lang));
                        if let Some(index) = gc_ui::ribbon_split_button(ui, salt, &more, &items, sketching) {
                            session.set_tool(variants[index].0);
                        }
                    };
                    if spatial {
                        // A 3D sketch has lines and arcs (Center Point Arc and Three Point Arc).
                        split(ui, "line", Msg::ToolLine, &[(ToolKind::Line, Icon::Line, Msg::ToolLine)]);
                        split(
                            ui,
                            "arc",
                            Msg::SplitArc,
                            &[
                                (ToolKind::Arc3, Icon::Arc, Msg::ToolArc3),
                                (ToolKind::Arc, Icon::Arc, Msg::ToolArc),
                            ],
                        );
                        // Bend: two lines that meet, then a radius (GUID-4F861674-F74C-474E-869A-4C190ED6EC4B).
                        ui.vertical(|ui| {
                            let button = gc_ui::ribbon_small_button(
                                ui,
                                Icon::Arc,
                                tr!(lang, ToolBend),
                                viewport.is_bending(),
                                sketching,
                            );
                            if button.clicked() {
                                session.set_tool(ToolKind::Select);
                                viewport.start_bend();
                            }
                        });
                    } else {
                        split(
                            ui,
                            "line",
                            Msg::ToolLine,
                            &[
                                (ToolKind::Line, Icon::Line, Msg::ToolLine),
                                (ToolKind::MidpointLine, Icon::MidLine, Msg::ToolMidpointLine),
                                (ToolKind::Spline, Icon::Spline, Msg::ToolSpline),
                            ],
                        );
                        split(
                            ui,
                            "circle",
                            Msg::ToolCircle,
                            &[
                                (ToolKind::Circle, Icon::Circle, Msg::ToolCircle),
                                (ToolKind::TangentCircle, Icon::TangentCircle, Msg::ToolTangentCircle),
                                (ToolKind::Ellipse, Icon::Ellipse, Msg::ToolEllipse),
                            ],
                        );
                        split(
                            ui,
                            "arc",
                            Msg::SplitArc,
                            &[
                                (ToolKind::Arc3, Icon::Arc, Msg::ToolArc3),
                                (ToolKind::ArcTangent, Icon::Arc, Msg::ToolArcTangent),
                                (ToolKind::Arc, Icon::Arc, Msg::ToolArc),
                            ],
                        );
                        split(
                            ui,
                            "rectangle",
                            Msg::SplitRectangle,
                            &[
                                (ToolKind::Rectangle, Icon::Rectangle, Msg::ToolRectangle),
                                (ToolKind::Rectangle3, Icon::Rectangle, Msg::ToolRectangle3),
                                (ToolKind::CenterRectangle, Icon::CenterRect, Msg::ToolCenterRectangle),
                                (ToolKind::CenterRectangle3, Icon::CenterRect3, Msg::ToolCenterRectangle3),
                            ],
                        );
                        split(
                            ui,
                            "slot",
                            Msg::ToolSlot,
                            &[
                                (ToolKind::Slot(SlotKind::CenterToCenter), Icon::Slot, Msg::ToolSlot),
                                (ToolKind::Slot(SlotKind::Overall), Icon::SlotOverall, Msg::ToolSlotOverall),
                                (
                                    ToolKind::Slot(SlotKind::CenterPoint),
                                    Icon::SlotCentre,
                                    Msg::ToolSlotCenterPoint,
                                ),
                                (
                                    ToolKind::ArcSlot(ArcSlotKind::ThreePoint),
                                    Icon::SlotArc3,
                                    Msg::ToolSlotThreePointArc,
                                ),
                                (
                                    ToolKind::ArcSlot(ArcSlotKind::CenterPoint),
                                    Icon::SlotArcCentre,
                                    Msg::ToolSlotCenterPointArc,
                                ),
                            ],
                        );
                    }
                    // Point and Polygon (Inventor 2026 help, To Work with Points,
                    // GUID-D4A4288F-32F1-4E23-A382-930EBA152E87), small, with the Format panel's Center
                    // Point toggle: 2D sketches.
                    if !spatial {
                        ui.vertical(|ui| {
                            for (kind, icon, label) in [
                                (ToolKind::Point, Icon::Point, Msg::ToolPoint),
                                (ToolKind::Polygon, Icon::Polygon, Msg::ToolPolygon),
                            ] {
                                let button = gc_ui::ribbon_small_button(
                                    ui,
                                    icon,
                                    label.text(lang),
                                    current == kind,
                                    sketching,
                                );
                                if button.clicked() {
                                    session.set_tool(kind);
                                }
                            }
                        });
                        ui.vertical(|ui| {
                            let on = session.center_point();
                            let toggle = gc_ui::ribbon_small_button(
                                ui,
                                Icon::Text("+"),
                                tr!(lang, ToolCenterPoint),
                                on,
                                sketching,
                            );
                            if toggle.clicked() {
                                // A refusal (a selected end point) shows in the status bar.
                                drop(session.set_center_point(!on));
                            }
                            // Project Cut Edges, small to keep the tab within 1280 px: a 2D sketch only.
                            let cut = gc_ui::ribbon_small_button(
                                ui,
                                Icon::CutEdges,
                                tr!(lang, ToolCutEdges),
                                false,
                                sketching,
                            );
                            if cut.clicked() {
                                action = Some(RibbonAction::ProjectCutEdges);
                            }
                        });
                    }
                });
                if !spatial {
                    // Inventor's Pattern panel: Rectangular, Circular, Mirror.
                    ribbon_group(ui, tr!(lang, GroupPattern), |ui| {
                        let patterns = [
                            (Icon::RectPattern, Msg::ToolRectPattern, RibbonAction::StartSketchRectPattern),
                            (Icon::CircPattern, Msg::ToolCircPattern, RibbonAction::StartSketchCircPattern),
                            (Icon::Mirror, Msg::ToolMirror, RibbonAction::StartSketchMirror),
                        ];
                        ui.vertical(|ui| {
                            for row in patterns.chunks(2) {
                                ui.horizontal(|ui| {
                                    for (icon, label, start) in row.iter().copied() {
                                        let button = gc_ui::ribbon_small_button(
                                            ui,
                                            icon,
                                            label.text(lang),
                                            false,
                                            sketching,
                                        );
                                        if button.clicked() {
                                            action = Some(start);
                                        }
                                    }
                                });
                            }
                        });
                    });
                    // Inventor's Modify panel: Offset, Split, Move, Copy, Rotate (To Copy, Move, or Rotate 2D
                    // Sketch Geometry, GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9); Chamfer next to them (Create
                    // panel in Inventor; small icons in two rows to keep the tab within 1280 px).
                    ribbon_group(ui, tr!(lang, GroupModify), |ui| {
                        ui.vertical(|ui| {
                            let rows = [
                                vec![
                                    (ToolKind::Offset, Icon::Offset, Msg::ToolOffset),
                                    (ToolKind::Split, Icon::Split, Msg::ToolSplit),
                                    (ToolKind::Move, Icon::Move, Msg::ToolMove),
                                    (ToolKind::Scale, Icon::Scale, Msg::ToolScale),
                                    (ToolKind::Trim, Icon::Trim, Msg::ToolTrim),
                                    (ToolKind::Fillet, Icon::Fillet, Msg::ToolFillet),
                                ],
                                vec![
                                    (ToolKind::Chamfer, Icon::Chamfer, Msg::ToolChamfer),
                                    (ToolKind::Copy, Icon::Copy, Msg::ToolCopy),
                                    (ToolKind::Rotate, Icon::Rotate, Msg::ToolRotate),
                                    (ToolKind::Stretch, Icon::Stretch, Msg::ToolStretch),
                                    (ToolKind::Extend, Icon::Extend, Msg::ToolExtend),
                                ],
                            ];
                            for row in rows {
                                ui.horizontal(|ui| {
                                    for (kind, icon, label) in row {
                                        let button = gc_ui::ribbon_small_button(
                                            ui,
                                            icon,
                                            label.text(lang),
                                            current == kind,
                                            sketching,
                                        );
                                        if button.clicked() {
                                            session.set_tool(kind);
                                        }
                                    }
                                });
                            }
                        });
                    });
                }
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
                    // The geometric constraints as small icons in two rows, as Inventor's Constrain panel.
                    let constraints: &[(ConstraintKind, &str, Msg)] =
                        if spatial { &CONSTRAINTS_3D } else { &CONSTRAINTS };
                    let per_row = constraints.len().div_ceil(2);
                    ui.vertical(|ui| {
                        for row in constraints.chunks(per_row.max(1)) {
                            ui.horizontal(|ui| {
                                for (kind, symbol, label) in row.iter().copied() {
                                    let tool = ToolKind::Constraint(kind);
                                    let button = gc_ui::ribbon_small_button(
                                        ui,
                                        Icon::Text(symbol),
                                        label.text(lang),
                                        current == tool,
                                        sketching,
                                    );
                                    if button.clicked() {
                                        session.set_tool(tool);
                                    }
                                }
                            });
                        }
                    });
                    // Automatic Dimensions and Constraints (To Apply 2D Sketch Dimensions Automatically,
                    // GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882): a 2D sketch.
                    if !spatial {
                        ui.vertical(|ui| {
                            let auto = gc_ui::ribbon_small_button(
                                ui,
                                Icon::Text("A|"),
                                tr!(lang, ToolAutoDimension),
                                false,
                                sketching,
                            );
                            if auto.clicked() {
                                action = Some(RibbonAction::StartAutoDimension);
                            }
                            // Flip Direction for the selected dimensions (Inventor's right-click entry;
                            // To Work with Sketch Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185).
                            let flip = gc_ui::ribbon_small_button(
                                ui,
                                Icon::Text("<>"),
                                tr!(lang, ToolFlipDimension),
                                false,
                                sketching && session.can_flip_selected(),
                            );
                            if flip.clicked() {
                                // A refusal shows in the status bar.
                                drop(session.flip_selected_dimensions());
                            }
                        });
                    }
                });
                ribbon_group(ui, tr!(lang, GroupExit), |ui| {
                    if ribbon_button(ui, Icon::Finish, tr!(lang, CmdFinishSketch), false, sketching).clicked()
                    {
                        // The tab that was open before the sketch comes back on its own.
                        session.finish_sketch();
                    }
                    // Cancel Sketch: discards what was done since the sketch was entered.
                    if gc_ui::ribbon_small_button(
                        ui,
                        Icon::Text("x"),
                        tr!(lang, CmdCancelSketch),
                        false,
                        sketching,
                    )
                    .clicked()
                    {
                        // False only past the 200-step history: then as much as it holds is taken back.
                        let _complete = session.cancel_sketch();
                    }
                });
            }
            Tab::Assemble => action = assemble_tab(ui, session, viewport, lang),
            Tab::Robot => action = robot_tab(ui, session, viewport, cell, library, lang),
            Tab::Inspect => {
                ribbon_group(ui, tr!(lang, GroupMeasure), |ui| {
                    if ribbon_button(ui, Icon::Dimension, tr!(lang, MenuMeasure), false, true).clicked() {
                        action = Some(RibbonAction::StartMeasure);
                    }
                });
                ribbon_group(ui, tr!(lang, GroupAnalyze), |ui| {
                    if ribbon_button(ui, Icon::Cuboid, tr!(lang, MenuInterference), false, true).clicked() {
                        action = Some(RibbonAction::StartInterference);
                    }
                    // Inventor: Inspect tab > Interference panel > Activate Contact Solver; the modes are the
                    // Interactive Contact choices of the document settings.
                    let mode = session.doc().contact_solver();
                    let rows = [gc_ui::StackRow {
                        icon: Icon::Cuboid,
                        label: tr!(lang, CmdContactSolver),
                        selected: mode != ContactSolver::Off,
                        enabled: true,
                    }];
                    if let Some(button) = gc_ui::ribbon_stack_responses(ui, &rows).first() {
                        egui::Popup::menu(button).show(|ui| {
                            for (this, label) in [
                                (ContactSolver::Off, tr!(lang, ContactOff)),
                                (ContactSolver::ContactSet, tr!(lang, ContactSetOnly)),
                                (ContactSolver::All, tr!(lang, ContactAll)),
                            ] {
                                if ui.selectable_label(mode == this, label).clicked() {
                                    drop(session.set_contact_solver(this));
                                    ui.close();
                                }
                            }
                        });
                    }
                });
            }
            Tab::Manage => {
                ribbon_group(ui, tr!(lang, GroupParameters), |ui| {
                    if ribbon_button(ui, Icon::Text("fx"), tr!(lang, MenuParameters), false, true).clicked() {
                        action = Some(RibbonAction::StartParameters);
                    }
                });
            }
            Tab::View => {
                ribbon_group(ui, tr!(lang, GroupViews), |ui| {
                    // Home and Fit All are the large buttons; the three standard views are stacked.
                    if ribbon_button(ui, Icon::Home, tr!(lang, ViewHome), false, true).clicked() {
                        viewport.look(StandardView::Iso);
                    }
                    let views = [
                        (StandardView::Front, Msg::ViewFront),
                        (StandardView::Top, Msg::ViewTop),
                        (StandardView::Right, Msg::ViewRight),
                    ];
                    let rows: Vec<gc_ui::StackRow<'_>> = views
                        .iter()
                        .map(|(_, label)| gc_ui::StackRow {
                            icon: Icon::View,
                            label: label.text(lang),
                            selected: false,
                            enabled: true,
                        })
                        .collect();
                    if let Some(index) = gc_ui::ribbon_stack(ui, &rows)
                        && let Some((view, _)) = views.get(index)
                    {
                        viewport.look(*view);
                    }
                    if ribbon_button(ui, Icon::Rectangle, tr!(lang, ViewFit), false, true).clicked() {
                        viewport.fit(session, [1000.0, 700.0]);
                    }
                    // Look At turns the view to the selected plane, face or line (the navigation bar's
                    // Look At, Page Up).
                    let can_look = viewport.can_look_at(session);
                    if ribbon_button(ui, Icon::LookAt, tr!(lang, NavLookAt), false, can_look).clicked() {
                        viewport.look_at(session);
                    }
                });
                ribbon_group(ui, tr!(lang, GroupProjection), |ui| {
                    let ortho = viewport.camera.ortho;
                    let rows = [
                        gc_ui::StackRow {
                            icon: Icon::View,
                            label: tr!(lang, ViewOrtho),
                            selected: ortho,
                            enabled: true,
                        },
                        gc_ui::StackRow {
                            icon: Icon::Perspective,
                            label: tr!(lang, ViewPerspective),
                            selected: !ortho,
                            enabled: true,
                        },
                    ];
                    match gc_ui::ribbon_stack(ui, &rows) {
                        Some(0) => viewport.camera.ortho = true,
                        Some(1) => viewport.camera.ortho = false,
                        _ => {}
                    }
                });
                ribbon_group(ui, tr!(lang, GroupAppearance), |ui| {
                    let kinds = [
                        (gc_viewport::SectionKind::Half, Msg::SectionHalf),
                        (gc_viewport::SectionKind::Quarter, Msg::SectionQuarter),
                        (gc_viewport::SectionKind::ThreeQuarter, Msg::SectionThreeQuarter),
                    ];
                    let rows: Vec<gc_ui::StackRow<'_>> = kinds
                        .iter()
                        .map(|(kind, label)| gc_ui::StackRow {
                            icon: Icon::LookAt,
                            label: label.text(lang),
                            selected: viewport.section.as_ref().is_some_and(|s| s.kind == *kind),
                            enabled: true,
                        })
                        .collect();
                    if let Some(index) = gc_ui::ribbon_stack(ui, &rows)
                        && let Some((kind, _)) = kinds.get(index)
                    {
                        viewport.section = Some(gc_viewport::Section::new(*kind));
                    }
                    if ribbon_button(
                        ui,
                        Icon::Finish,
                        tr!(lang, SectionDelete),
                        false,
                        viewport.section.is_some(),
                    )
                    .clicked()
                    {
                        viewport.section = None;
                    }
                });
                ribbon_group(ui, tr!(lang, GroupVisibility), |ui| {
                    // Degrees of freedom: a symbol at every component that can still move.
                    let items: Vec<gc_core::AnyId> = session
                        .components()
                        .iter()
                        .filter_map(|node| match node.occurrence {
                            gc_doc::Occurrence::Item(id) => Some(id),
                            _ => None,
                        })
                        .collect();
                    let on = !session.dof_symbols().is_empty();
                    if ribbon_button(ui, Icon::Orbit, tr!(lang, CmdDof), on, !items.is_empty()).clicked() {
                        drop(session.set_dof_shown(&items, !on));
                    }
                });
                ribbon_group(ui, tr!(lang, GroupPanels), |ui| {
                    let rows = [
                        gc_ui::StackRow {
                            icon: Icon::View,
                            label: tr!(lang, ViewPanelsSeeThrough),
                            selected: *translucent,
                            enabled: true,
                        },
                        gc_ui::StackRow {
                            icon: Icon::View,
                            label: tr!(lang, ViewDarkTheme),
                            selected: *dark,
                            enabled: true,
                        },
                    ];
                    match gc_ui::ribbon_stack(ui, &rows) {
                        Some(0) => *translucent = !*translucent,
                        Some(1) => *dark = !*dark,
                        _ => {}
                    }
                });
            }
        }
    });
    action
}

/// Inventor's Assemble tab: Component (place), Pattern, Relationships (constrain, joint, drive) and
/// Position (free move, free rotate). Robot-only commands stay in the Robot tab.
fn assemble_tab(
    ui: &mut egui::Ui,
    session: &mut Session,
    viewport: &mut Viewport,
    lang: Lang,
) -> Option<RibbonAction> {
    let mut action = None;
    let robot = session.doc().robots().iter().next().map(|(id, _)| id);
    let has_components = !session.components().is_empty();
    ribbon_group(ui, tr!(lang, GroupComponent), |ui| {
        if ribbon_button(ui, Icon::Import, tr!(lang, CmdPlace), false, true).clicked() {
            action = Some(RibbonAction::Place);
        }
        let rows = [gc_ui::StackRow {
            icon: Icon::Select,
            label: tr!(lang, ReplicateReplace),
            selected: false,
            enabled: has_components,
        }];
        if gc_ui::ribbon_stack(ui, &rows).is_some() {
            action = Some(RibbonAction::StartReplicate(crate::replicate::Mode::Replace));
        }
    });
    ribbon_group(ui, tr!(lang, PatternTitle), |ui| {
        if ribbon_button(ui, Icon::RectPattern, tr!(lang, PatternTitle), false, has_components).clicked() {
            action = Some(RibbonAction::StartPattern);
        }
        let rows = [
            gc_ui::StackRow {
                icon: Icon::Mirror,
                label: tr!(lang, ReplicateMirror),
                selected: false,
                enabled: has_components,
            },
            gc_ui::StackRow {
                icon: Icon::Cuboid,
                label: tr!(lang, ReplicateCopy),
                selected: false,
                enabled: has_components,
            },
        ];
        match gc_ui::ribbon_stack(ui, &rows) {
            Some(0) => action = Some(RibbonAction::StartReplicate(crate::replicate::Mode::Mirror)),
            Some(1) => action = Some(RibbonAction::StartReplicate(crate::replicate::Mode::Copy)),
            _ => {}
        }
    });
    // Failures of these commands are shown in the status bar. Constrain and Joint are the large buttons;
    // Assemble and Drive are stacked beside them.
    ribbon_group(ui, tr!(lang, GroupRelationships), |ui| {
        if ribbon_button(ui, Icon::Frame, tr!(lang, ConstrainTitle), false, true).clicked() {
            action = Some(RibbonAction::StartConstrain);
        }
        if ribbon_button(ui, Icon::Orbit, tr!(lang, JointTitle), false, true).clicked() {
            action = Some(RibbonAction::StartJoint);
        }
        let rows = [
            gc_ui::StackRow {
                icon: Icon::Frame,
                label: tr!(lang, AssembleTitle),
                selected: false,
                enabled: has_components,
            },
            gc_ui::StackRow {
                icon: Icon::Play,
                label: tr!(lang, MenuDrive),
                selected: false,
                enabled: robot.is_some() || !session.relationships().is_empty(),
            },
        ];
        match gc_ui::ribbon_stack(ui, &rows) {
            Some(0) => action = Some(RibbonAction::StartAssemble),
            Some(1) => action = Some(RibbonAction::StartDrive),
            _ => {}
        }
    });
    // Edit in place: back out of the component or subassembly being edited (Inventor 2026 help, Edit
    // components in place, GUID-C79C36C9-A6DB-46C5-8864-41F66C750231).
    ribbon_group(ui, tr!(lang, GroupEdit), |ui| {
        let editing = session.edit_target().is_some();
        // One split button: the body is the way back used last.
        let items = [
            gc_ui::SplitItem { icon: Icon::Home, label: tr!(lang, CmdReturn), active: false },
            gc_ui::SplitItem { icon: Icon::Home, label: tr!(lang, CmdReturnParent), active: false },
            gc_ui::SplitItem { icon: Icon::Home, label: tr!(lang, CmdReturnTop), active: false },
        ];
        let more = tr!(lang, SplitMore, tr!(lang, GroupEdit));
        match gc_ui::ribbon_split_button(ui, "return", &more, &items, editing) {
            Some(0) => session.return_edit(),
            Some(1) => session.return_to_parent(),
            Some(2) => session.return_to_top(),
            _ => {}
        }
    });
    ribbon_group(ui, tr!(lang, GroupPosition), |ui| {
        let has = !session.components().is_empty();
        // Free Move and Free Rotate share one split button; the one running is on its body.
        let tools = [
            (gc_viewport::FreeTool::Move, tr!(lang, MarkMove)),
            (gc_viewport::FreeTool::Rotate, tr!(lang, MarkRotate)),
        ];
        let items: Vec<gc_ui::SplitItem<'_>> = tools
            .iter()
            .map(|(tool, label)| gc_ui::SplitItem {
                icon: Icon::Pan,
                label,
                active: viewport.free_tool == Some(*tool),
            })
            .collect();
        let more = tr!(lang, SplitMore, tr!(lang, GroupPosition));
        if let Some(index) = gc_ui::ribbon_split_button(ui, "free", &more, &items, has) {
            let tool = tools[index].0;
            viewport.free_tool = if viewport.free_tool == Some(tool) { None } else { Some(tool) };
        }
        // Grip Snap and Ground are stacked beside the large Free Move split button. Ground applies to the
        // selected body, robot or imported assembly and is shown pressed when it is grounded.
        let selected = session.selected_occurrence().filter(|o| session.can_ground(*o));
        let grounded =
            selected.is_some_and(|o| session.doc().component_flag(o, gc_doc::ComponentFlag::Grounded));
        let rows = [
            gc_ui::StackRow {
                icon: Icon::GripSnap,
                label: tr!(lang, GripTitle),
                selected: viewport.is_grip_snapping(),
                enabled: has,
            },
            gc_ui::StackRow {
                icon: Icon::Ground,
                label: tr!(lang, CmdGround),
                selected: grounded,
                enabled: selected.is_some(),
            },
        ];
        match gc_ui::ribbon_stack(ui, &rows) {
            Some(0) => viewport.start_grip_snap(),
            Some(1) => action = Some(RibbonAction::ToggleGround),
            _ => {}
        }
    });
    action
}

fn robot_tab(
    ui: &mut egui::Ui,
    session: &mut Session,
    viewport: &mut Viewport,
    cell: &mut CellUi,
    library: &[crate::app::LibraryRobot],
    lang: Lang,
) -> Option<RibbonAction> {
    let mut action = None;
    let robot = session.doc().robots().iter().next().map(|(id, _)| id);
    let path = cell.active_path(session);
    ribbon_group(ui, tr!(lang, GroupAdd), |ui| {
        // One robot per cell for now: the panels show the first one.
        // The robot library in one list: the packages of the library folder and what plugins offer.
        let add = ribbon_button(ui, Icon::Robot, tr!(lang, CmdRobotLibrary), false, robot.is_none());
        egui::Popup::menu(&add).show(|ui| {
            if library.is_empty() {
                ui.label(tr!(lang, LibraryEmpty));
            }
            for entry in library {
                if ui.button(tr!(lang, CmdAddRobot, entry.name())).clicked() {
                    if let Err(error) = entry.add(session) {
                        log::error!("the robot could not be added: {error}");
                    }
                    viewport.fit(session, [1000.0, 700.0]);
                }
            }
        });
        // The imports are stacked beside the large Add button, then the three shapes.
        let rows = [
            gc_ui::StackRow {
                icon: Icon::Import,
                label: tr!(lang, CmdImportRobot),
                selected: false,
                enabled: robot.is_none(),
            },
            gc_ui::StackRow {
                icon: Icon::Import,
                label: tr!(lang, CmdImportMesh),
                selected: false,
                enabled: true,
            },
        ];
        match gc_ui::ribbon_stack(ui, &rows) {
            Some(0) => action = Some(RibbonAction::ImportRobot),
            Some(1) => action = Some(RibbonAction::ImportMesh),
            _ => {}
        }
        let at = |x: f64, z: f64| Placement { xyz: [x, 0.0, z], rpy: [0.0; 3] };
        let shapes = [
            (Icon::Cuboid, Msg::CmdAddBox, Solid::Box { size: [0.1, 0.1, 1.4] }, at(0.55, 0.7)),
            (
                Icon::Cylinder,
                Msg::CmdAddCylinder,
                Solid::Cylinder { radius: 0.06, length: 0.8 },
                at(0.0, 0.4),
            ),
            (Icon::Sphere, Msg::CmdAddSphere, Solid::Sphere { radius: 0.1 }, at(0.0, 0.1)),
        ];
        let rows: Vec<gc_ui::StackRow<'_>> = shapes
            .iter()
            .map(|(icon, label, _, _)| gc_ui::StackRow {
                icon: *icon,
                label: label.text(lang),
                selected: false,
                enabled: true,
            })
            .collect();
        if let Some(index) = gc_ui::ribbon_stack(ui, &rows)
            && let Some((_, _, solid, placement)) = shapes.get(index)
        {
            // New bodies are spread out sideways so that they do not hide each other.
            let offset = 0.3 * session.doc().bodies().len() as f64;
            let placement = Placement {
                xyz: [placement.xyz[0], placement.xyz[1] - offset, placement.xyz[2]],
                ..*placement
            };
            drop(session.add_body(solid.clone(), placement));
            viewport.fit(session, [1000.0, 700.0]);
        }
    });
    ribbon_group(ui, tr!(lang, GroupPath), |ui| {
        let waypoints: Vec<_> = session
            .doc()
            .waypoints()
            .iter()
            .filter(|(_, w)| Some(w.robot) == robot)
            .map(|(id, _)| id)
            .collect();
        // The large buttons: make the path, plan it, play it.
        if ribbon_button(ui, Icon::Path, tr!(lang, CmdMakePath), false, !waypoints.is_empty()).clicked()
            && let Some(robot) = robot
        {
            // The new path becomes the one the panel works on.
            cell.path = session.add_path(robot, waypoints).ok();
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
        if ribbon_button(ui, Icon::Play, label, cell.playing, planned).clicked()
            && let Some(path) = path
        {
            cell.playing = !cell.playing;
            if cell.playing {
                session.set_playback(Some((path, 0.0)));
            }
        }
        // One selected point of the sketch being edited can become a waypoint that follows the sketch.
        let point = match (session.active_sketch(), session.selection().iter().next()) {
            (Some((sketch, _)), Some(Item::Point(point))) if session.selection().len() == 1 => {
                Some((sketch, *point))
            }
            _ => None,
        };
        let frame = cell.active_frame(session);
        // The ways to add a waypoint are stacked: here, at a sketch point, in a work frame.
        let rows = [
            gc_ui::StackRow {
                icon: Icon::Waypoint,
                label: tr!(lang, CmdAddWaypoint),
                selected: false,
                enabled: robot.is_some(),
            },
            gc_ui::StackRow {
                icon: Icon::Waypoint,
                label: tr!(lang, CmdWaypointAtPoint),
                selected: false,
                enabled: robot.is_some() && point.is_some(),
            },
            gc_ui::StackRow {
                icon: Icon::Waypoint,
                label: tr!(lang, CmdWaypointInFrame),
                selected: false,
                enabled: robot.is_some() && frame.is_some(),
            },
        ];
        match (gc_ui::ribbon_stack(ui, &rows), robot) {
            (Some(0), Some(robot)) => drop(session.add_waypoint_here(robot)),
            (Some(1), Some(robot)) => {
                if let Some((sketch, point)) = point {
                    drop(session.add_waypoint(robot, WaypointTarget::SketchPoint { sketch, point }));
                }
            }
            (Some(2), Some(robot)) => {
                if let Some(frame) = frame {
                    drop(session.add_waypoint_in_frame(robot, frame));
                }
            }
            _ => {}
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
        // The sketch being edited, else the newest one, becomes a path along its lines.
        let sketch = session
            .active_sketch()
            .map(|(id, _)| id)
            .or_else(|| session.doc().sketches().iter().last().map(|(id, _)| id))
            .filter(|id| {
                session.doc().get(*id).is_some_and(|s| s.curves.keys().any(|c| s.line(*c).is_some()))
            });
        let rows = [
            gc_ui::StackRow {
                icon: Icon::Frame,
                label: tr!(lang, CmdWorkFrame),
                selected: false,
                enabled: frame_sketch.is_some(),
            },
            gc_ui::StackRow {
                icon: Icon::Sketch,
                label: tr!(lang, CmdPathFromSketch),
                selected: false,
                enabled: robot.is_some() && sketch.is_some(),
            },
        ];
        match gc_ui::ribbon_stack(ui, &rows) {
            Some(0) => {
                if let Some(sketch) = frame_sketch {
                    // A refusal (a point can still move, or the points give no axes) is explained in the
                    // status bar.
                    cell.frame = session.add_work_frame_by_points(sketch, &frame_points).ok();
                }
            }
            Some(1) => {
                if let (Some(robot), Some(sketch)) = (robot, sketch) {
                    cell.path = session.path_from_sketch(robot, sketch).ok();
                }
            }
            _ => {}
        }
    });
    action
}
