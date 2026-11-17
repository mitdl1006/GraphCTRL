//! The panels around the viewport: model browser, properties and status bar.

mod cell;

use egui::{RichText, Ui};
use gc_core::Id;
use gc_doc::{FrameKind, Sketch};
use gc_i18n::{Lang, Msg, tr};
use gc_session::Session;
use gc_sketch::{CurveGeom, Item, SketchError};
use gc_tools::Prompt;
use gc_ui::colors;

pub use cell::{CellUi, cell, status_text};

/// What the user asked for in the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserAction {
    /// Edit this sketch.
    EditSketch(Id<Sketch>),
    /// Delete this item of the robot cell.
    Delete(gc_doc::CellOp),
}

/// The model browser: origin geometry and sketches. Double-click a sketch to edit it.
pub fn browser(ui: &mut Ui, session: &Session, lang: Lang) -> Option<BrowserAction> {
    let mut action = None;
    ui.heading(tr!(lang, PanelBrowser));
    ui.separator();
    egui::CollapsingHeader::new(tr!(lang, BrowserOrigin)).default_open(true).show(ui, |ui| {
        for (_, frame) in session.doc().frames().iter().filter(|(_, f)| f.kind == FrameKind::OriginPlane) {
            ui.label(&frame.name);
        }
    });
    egui::CollapsingHeader::new(tr!(lang, BrowserSketches)).default_open(true).show(ui, |ui| {
        let active = session.active_sketch().map(|(id, _)| id);
        for (id, sketch) in session.doc().sketches().iter() {
            let label = ui.selectable_label(active == Some(id), &sketch.name);
            if label.double_clicked() {
                action = Some(BrowserAction::EditSketch(id));
            }
        }
    });
    // Cell items: name and a small button that deletes the item.
    let doc = session.doc();
    let mut section = |ui: &mut Ui, title: &str, items: Vec<(String, String, gc_doc::CellOp)>| {
        if items.is_empty() {
            return;
        }
        egui::CollapsingHeader::new(title).default_open(true).show(ui, |ui| {
            for (name, shown, delete) in items {
                ui.horizontal(|ui| {
                    ui.label(shown);
                    let button = ui.small_button("x").on_hover_text(tr!(lang, CmdDeleteItem, name));
                    button.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, tr!(lang, CmdDeleteItem, name))
                    });
                    if button.clicked() {
                        action = Some(BrowserAction::Delete(delete));
                    }
                });
            }
        });
    };
    use gc_doc::CellOp;
    let plain = |name: &str| (name.to_owned(), name.to_owned());
    section(
        ui,
        tr!(lang, BrowserBodies),
        doc.bodies()
            .iter()
            .map(|(id, b)| (plain(&b.name).0, plain(&b.name).1, CellOp::DeleteBody(id)))
            .collect(),
    );
    section(
        ui,
        tr!(lang, BrowserRobots),
        doc.robots()
            .iter()
            .map(|(id, r)| (plain(&r.name).0, plain(&r.name).1, CellOp::DeleteRobot(id)))
            .collect(),
    );
    section(
        ui,
        tr!(lang, BrowserWaypoints),
        doc.waypoints()
            .iter()
            .map(|(id, w)| (plain(&w.name).0, plain(&w.name).1, CellOp::DeleteWaypoint(id)))
            .collect(),
    );
    let paths = doc
        .paths()
        .iter()
        .map(|(id, p)| {
            let shown = format!("{} ({})", p.name, status_text(lang, session.plan_status(id)));
            (p.name.clone(), shown, CellOp::DeletePath(id))
        })
        .collect();
    section(ui, tr!(lang, BrowserPaths), paths);
    action
}

/// The properties of the selection. A selected dimension can be edited here.
pub fn properties(ui: &mut Ui, session: &mut Session, lang: Lang, expression: &mut String) {
    ui.heading(tr!(lang, PanelProperties));
    ui.separator();
    let Some((_, sketch)) = session.active_sketch() else {
        ui.weak(tr!(lang, PropNothing));
        return;
    };
    let selection: Vec<Item> = session.selection().iter().copied().collect();
    let length = |si: f64| format!("{} mm", trim(si * 1000.0));
    match selection.as_slice() {
        [] => {
            ui.weak(tr!(lang, PropNothing));
        }
        [Item::Point(p)] => {
            if let Some(pos) = session.point_pos(*p) {
                ui.label(tr!(lang, PropPoint, length(pos[0]), length(pos[1])));
            }
        }
        [Item::Curve(c)] => match sketch.curves.get(c).map(|c| c.geom) {
            Some(CurveGeom::Line { a, b }) => {
                if let Some(value) = sketch.measure(&gc_sketch::DimKind::Distance { p: a, q: b }) {
                    ui.label(tr!(lang, PropLine, length(value)));
                }
            }
            Some(CurveGeom::Circle { .. }) => {
                ui.label(tr!(lang, PropCircle, length(session.radius(*c).unwrap_or(0.0))));
            }
            Some(CurveGeom::Arc { .. }) => {
                ui.label(tr!(lang, PropArc, length(session.radius(*c).unwrap_or(0.0))));
            }
            None => {}
        },
        [Item::Constraint(_)] => {
            ui.label(tr!(lang, PropConstraint));
        }
        [Item::Dimension(d)] => {
            let Some(dimension) = sketch.dimensions.get(d) else { return };
            ui.label(tr!(lang, PropDimension, dimension.source));
            if dimension.driving {
                let id = *d;
                let memory = egui::Id::unique(("gc_expression_of", d.raw()));
                let loaded: Option<u64> = ui.data(|data| data.get_temp(memory));
                if loaded != Some(d.raw().get()) {
                    expression.clone_from(&dimension.source);
                    ui.data_mut(|data| data.insert_temp(memory, d.raw().get()));
                }
                let label = ui.label(tr!(lang, PropExpression));
                let field = ui.text_edit_singleline(expression).labelled_by(label.id);
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // A refused edit is shown in the status bar; the field keeps what was typed.
                    drop(session.edit_dimension(id, expression));
                }
            }
        }
        many => {
            ui.label(tr!(lang, PropSelected, many.len()));
        }
    }
}

fn trim(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn prompt_text(lang: Lang, prompt: Prompt) -> &'static str {
    let message = match prompt {
        Prompt::Select => Msg::PromptSelect,
        Prompt::LineStart => Msg::PromptLineStart,
        Prompt::LineEnd => Msg::PromptLineEnd,
        Prompt::CircleCentre => Msg::PromptCircleCentre,
        Prompt::CircleRim => Msg::PromptCircleRim,
        Prompt::RectangleFirst => Msg::PromptRectFirst,
        Prompt::RectangleSecond => Msg::PromptRectSecond,
        Prompt::DimensionPick => Msg::PromptDimPick,
        Prompt::DimensionPlace => Msg::PromptDimPlace,
        Prompt::DimensionValue => Msg::PromptDimValue,
        Prompt::ConstraintPick => Msg::PromptConstraint,
    };
    message.text(lang)
}

/// The status bar: what the tool expects next, why the last edit was refused, a notice from the
/// application (file saved, file error) and how constrained the sketch is.
pub fn status_bar(ui: &mut Ui, session: &Session, lang: Lang, gpu: bool, notice: Option<&str>) {
    ui.horizontal(|ui| {
        if session.active_sketch().is_some() {
            ui.label(prompt_text(lang, session.prompt()));
        } else {
            ui.label(tr!(lang, StatusReady));
        }
        if let Some(error) = session.last_error() {
            let message = match error {
                gc_doc::EditError::Redundant(_) => Msg::ErrRedundant,
                gc_doc::EditError::Conflicting(_) => Msg::ErrConflicting,
                gc_doc::EditError::Expr { .. } | gc_doc::EditError::Sketch(SketchError::InvalidValue) => {
                    Msg::ErrExpression
                }
                gc_doc::EditError::Sketch(SketchError::Unsolved(_)) => Msg::ErrUnsolved,
                gc_doc::EditError::InUse => Msg::ErrInUse,
                gc_doc::EditError::BadValue => Msg::ErrBadValue,
                _ => Msg::ErrOther,
            };
            ui.separator();
            ui.label(RichText::new(message.text(lang)).color(colors::ERROR));
        }
        if let Some(notice) = notice {
            ui.separator();
            ui.label(notice);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.weak(if gpu { tr!(lang, GpuOn) } else { tr!(lang, GpuOff) });
            if let Some(report) = session.report() {
                ui.separator();
                if report.dof == 0 {
                    ui.label(RichText::new(tr!(lang, StatusFullyConstrained)).strong());
                } else {
                    ui.label(tr!(lang, StatusDimensionsNeeded, report.dof));
                }
            }
        });
    });
}
