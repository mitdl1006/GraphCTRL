//! The ribbon: tabs of command groups. Every button is a view of a session command.

use eframe::egui;
use gc_draw::StandardView;
use gc_i18n::{Lang, Msg, tr};
use gc_session::Session;
use gc_tools::{ConstraintKind, ToolKind};
use gc_ui::{Icon, ribbon_button, ribbon_group, ribbon_tab};
use gc_viewport::Viewport;

/// The ribbon tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    Model,
    Sketch,
    View,
}

/// The constraint buttons: kind, symbol and name.
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

pub(crate) fn show(
    ui: &mut egui::Ui,
    tab: &mut Tab,
    session: &mut Session,
    viewport: &mut Viewport,
    lang: Lang,
) {
    let sketching = session.active_sketch().is_some();
    ui.horizontal(|ui| {
        for (this, label) in
            [(Tab::Model, Msg::TabModel), (Tab::Sketch, Msg::TabSketch), (Tab::View, Msg::TabView)]
        {
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
                });
            }
            Tab::Sketch => {
                let current = session.tool();
                ribbon_group(ui, tr!(lang, GroupCreate), |ui| {
                    for (kind, icon, label) in [
                        (ToolKind::Select, Icon::Select, Msg::ToolSelect),
                        (ToolKind::Line, Icon::Line, Msg::ToolLine),
                        (ToolKind::Circle, Icon::Circle, Msg::ToolCircle),
                        (ToolKind::Rectangle, Icon::Rectangle, Msg::ToolRectangle),
                    ] {
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
                    for (kind, symbol, label) in CONSTRAINTS {
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
}
