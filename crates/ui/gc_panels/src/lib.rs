//! The panels around the viewport: model browser, properties and status bar.

mod cell;
mod pose;

use egui::{RichText, Ui};
use gc_core::Id;
use gc_doc::{FrameKind, Sketch};
use gc_i18n::{Lang, Msg, tr};
use gc_session::Session;
use gc_sketch::{CurveGeom, Item, SketchError};
use gc_tools::Prompt;
use gc_ui::Icon;
use gc_ui::colors;
use gc_ui::panel::TreeRow;

pub use cell::{CellUi, cell, status_text};
pub use pose::{PoseCalc, RotationInput, pose_calculator};

/// What the user asked for in the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserAction {
    /// Edit this sketch.
    EditSketch(Id<Sketch>),
    /// Apply this edit (delete a cell item, add or change a parameter).
    Edit(Box<gc_doc::Command>),
    /// Show or hide a body or a robot.
    Show(gc_session::Shown, bool),
    /// Select an item of the cell, or nothing.
    Select(Option<gc_session::CellItem>),
    /// Plan a path.
    Plan(gc_core::Id<gc_doc::MotionPath>),
}

/// What is typed into the parameter editor of the browser.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParamsUi {
    /// The name being typed.
    pub name: String,
    /// The expression being typed.
    pub source: String,
}

/// The named parameters: a list, and one editor that adds a parameter or, when the name exists, changes
/// it. Clicking a parameter loads it into the editor.
fn params(ui: &mut Ui, session: &Session, lang: Lang, state: &mut ParamsUi) -> Option<gc_doc::Command> {
    let mut command = None;
    egui::CollapsingHeader::new(tr!(lang, BrowserParams)).default_open(true).show(ui, |ui| {
        let doc = session.doc();
        ui.spacing_mut().item_spacing.y = 1.0;
        for (id, param) in doc.params().iter() {
            let text = format!("{} = {}", param.name, param.source);
            let delete = tr!(lang, CmdDeleteItem, param.name);
            let row = TreeRow {
                icon: Icon::Text("="),
                text: &text,
                selected: state.name == param.name,
                visible: None,
                delete: Some(&delete),
            };
            let result = gc_ui::panel::tree_row(ui, row);
            if result.row.clicked() {
                state.name.clone_from(&param.name);
                state.source.clone_from(&param.source);
            }
            if result.deleted {
                command = Some(gc_doc::Command::DeleteParam { id });
            }
        }
        let existing = doc.params().iter().find(|(_, p)| p.name == state.name).map(|(id, _)| id);
        ui.horizontal(|ui| {
            let name = egui::TextEdit::singleline(&mut state.name)
                .hint_text(tr!(lang, ParamName))
                .desired_width(52.0);
            ui.add(name)
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "parameter name"));
            let source = egui::TextEdit::singleline(&mut state.source)
                .hint_text(tr!(lang, ParamValue))
                .desired_width(78.0);
            let field = ui.add(source);
            field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "parameter value"));
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let label = if existing.is_some() { tr!(lang, ParamApply) } else { tr!(lang, ParamAdd) };
            let ready = !state.name.is_empty() && !state.source.is_empty();
            if (ui.add_enabled(ready, egui::Button::new(label)).clicked() || entered) && ready {
                command = Some(match existing {
                    Some(id) => gc_doc::Command::SetParam { id, source: state.source.clone() },
                    None => {
                        // A bare number is millimetres unless the expression speaks of angles.
                        let angle = ["deg", "rad", "°"].iter().any(|unit| state.source.contains(unit));
                        gc_doc::Command::AddParam {
                            name: state.name.clone(),
                            source: state.source.clone(),
                            angle,
                        }
                    }
                });
            }
        });
    });
    command
}

/// The model browser: origin geometry and sketches. Double-click a sketch to edit it.
pub fn browser(ui: &mut Ui, session: &Session, lang: Lang, state: &mut ParamsUi) -> Option<BrowserAction> {
    let mut action = None;
    gc_ui::panel::header(ui, tr!(lang, PanelBrowser));
    egui::CollapsingHeader::new(tr!(lang, BrowserOrigin)).default_open(true).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (_, frame) in session.doc().frames().iter().filter(|(_, f)| f.kind == FrameKind::OriginPlane) {
            let row = TreeRow {
                icon: Icon::Frame,
                text: &frame.name,
                selected: false,
                visible: None,
                delete: None,
            };
            gc_ui::panel::tree_row(ui, row);
        }
    });
    egui::CollapsingHeader::new(tr!(lang, BrowserSketches)).default_open(true).show(ui, |ui| {
        let active = session.active_sketch().map(|(id, _)| id);
        ui.spacing_mut().item_spacing.y = 1.0;
        for (id, sketch) in session.doc().sketches().iter() {
            let selected = active == Some(id);
            let delete = tr!(lang, CmdDeleteItem, sketch.name);
            let row = TreeRow {
                icon: Icon::Sketch,
                text: &sketch.name,
                selected,
                visible: None,
                delete: Some(&delete),
            };
            let result = gc_ui::panel::tree_row(ui, row);
            if result.row.double_clicked() {
                action = Some(BrowserAction::EditSketch(id));
            }
            result.row.context_menu(|ui| {
                if ui.button(tr!(lang, CmdEditSketch)).clicked() {
                    action = Some(BrowserAction::EditSketch(id));
                    ui.close();
                }
                if ui.button(delete.as_str()).clicked() {
                    action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::DeleteSketch { id })));
                    ui.close();
                }
            });
            if result.deleted {
                action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::DeleteSketch { id })));
            }
        }
    });
    if let Some(command) = params(ui, session, lang, state) {
        action = Some(BrowserAction::Edit(Box::new(command)));
    }
    // Cell items: a selectable name with a context menu, and a small button that deletes the item.
    let doc = session.doc();
    let selected = session.selected_item();
    use gc_session::CellItem;
    type Row = (String, String, CellItem);
    let mut section = |ui: &mut Ui, title: &str, items: Vec<Row>| {
        if items.is_empty() {
            return;
        }
        egui::CollapsingHeader::new(title).default_open(true).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for (name, shown, item) in items {
                // Bodies and robots can be hidden; they stay obstacles for planning.
                let look = item.shown().and_then(|shown| Some((shown, session.shown_look(shown)?)));
                let mut visible = look.is_some_and(|(_, look)| !look.hidden);
                let show_label = tr!(lang, ShowItem, name);
                let delete_label = tr!(lang, CmdDeleteItem, name);
                let icon = match item {
                    CellItem::Body(_) => Icon::Cuboid,
                    CellItem::Robot(_) => Icon::Robot,
                    CellItem::Frame(_) => Icon::Frame,
                    CellItem::Waypoint(_) => Icon::Waypoint,
                    CellItem::Path(_) => Icon::Path,
                };
                let row = TreeRow {
                    icon,
                    text: &shown,
                    selected: selected == Some(item),
                    visible: look.map(|_| (&mut visible, show_label.as_str())),
                    delete: Some(&delete_label),
                };
                let result = gc_ui::panel::tree_row(ui, row);
                if let (true, Some((shown, _))) = (result.toggled, look) {
                    action = Some(BrowserAction::Show(shown, visible));
                }
                if result.row.clicked() || result.row.secondary_clicked() {
                    action = Some(BrowserAction::Select(Some(item)));
                }
                result.row.context_menu(|ui| {
                    ui.label(RichText::new(&name).strong());
                    ui.separator();
                    if let Some((shown, look)) = look {
                        let text = if look.hidden { tr!(lang, CmdShow) } else { tr!(lang, CmdHide) };
                        if ui.button(text).clicked() {
                            action = Some(BrowserAction::Show(shown, look.hidden));
                            ui.close();
                        }
                    }
                    if let CellItem::Path(path) = item
                        && ui.button(tr!(lang, CmdPlan)).clicked()
                    {
                        action = Some(BrowserAction::Plan(path));
                        ui.close();
                    }
                    if ui.button(delete_label.as_str()).clicked() {
                        action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(item.delete_op()))));
                        ui.close();
                    }
                });
                if result.deleted {
                    action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(item.delete_op()))));
                }
            }
        });
    };
    let row = |name: &str, item| (name.to_owned(), name.to_owned(), item);
    section(
        ui,
        tr!(lang, BrowserBodies),
        doc.bodies().iter().map(|(id, b)| row(&b.name, CellItem::Body(id))).collect(),
    );
    section(
        ui,
        tr!(lang, BrowserRobots),
        doc.robots().iter().map(|(id, r)| row(&r.name, CellItem::Robot(id))).collect(),
    );
    section(
        ui,
        tr!(lang, BrowserFrames),
        doc.work_frames().iter().map(|(id, f)| row(&f.name, CellItem::Frame(id))).collect(),
    );
    section(
        ui,
        tr!(lang, BrowserWaypoints),
        doc.waypoints().iter().map(|(id, w)| row(&w.name, CellItem::Waypoint(id))).collect(),
    );
    let paths = doc
        .paths()
        .iter()
        .map(|(id, p)| {
            let shown = format!("{} ({})", p.name, status_text(lang, session.plan_status(id)));
            (p.name.clone(), shown, CellItem::Path(id))
        })
        .collect();
    section(ui, tr!(lang, BrowserPaths), paths);
    action
}

/// The properties of the selection. A selected dimension can be edited here.
pub fn properties(ui: &mut Ui, session: &mut Session, lang: Lang, expression: &mut String) {
    gc_ui::panel::header(ui, tr!(lang, PanelProperties));
    // Outside sketches the robot cell's panel below says what is selected.
    let Some((_, sketch)) = session.active_sketch() else { return };
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
        Prompt::ArcCentre => Msg::PromptArcCentre,
        Prompt::ArcStart => Msg::PromptArcStart,
        Prompt::ArcEnd => Msg::PromptArcEnd,
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
        if session.sketch_is_spatial() {
            let plane = ["YZ", "ZX", "XY"][session.spatial_plane() % 3];
            ui.separator();
            ui.label(tr!(lang, StatusPlane, plane));
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
                gc_doc::EditError::Underconstrained => Msg::ErrUnderconstrained,
                gc_doc::EditError::DegenerateFrame => Msg::ErrDegenerateFrame,
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
