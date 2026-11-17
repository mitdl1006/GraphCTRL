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

pub use cell::{CellUi, cell, problem_text, status_text};
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
    /// Show or hide elements of a component's origin folder.
    OriginVisible(gc_doc::Occurrence, Vec<gc_doc::OriginElement>, bool),
    /// Select an origin element in the browser (the view follows), or nothing.
    OriginSelect(Option<(gc_doc::Occurrence, gc_doc::OriginElement)>),
    /// Start a sketch on an origin plane of a component.
    SketchOnComponent(gc_doc::Occurrence, gc_doc::OriginPlane),
    /// Drive an assembly relationship (the Drive dialog).
    DriveRelationship(gc_core::Id<gc_doc::Relationship>),
    /// Put a component into the contact set, or take it out.
    ContactSet(gc_core::AnyId, bool),
    /// Suppress a relationship (or bring it back) and solve again.
    SuppressRelationship(gc_core::Id<gc_doc::Relationship>, bool),
}

/// What is typed into the parameter editor of the browser.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParamsUi {
    /// The name being typed.
    pub name: String,
    /// The expression being typed.
    pub source: String,
    /// The browser item being renamed, and the name typed so far.
    pub rename: Option<(gc_core::AnyId, String)>,
    /// The rename has just begun: the field takes the keyboard focus.
    pub rename_fresh: bool,
    /// The origin element selected in the view or here.
    pub origin_selected: Option<(gc_doc::Occurrence, gc_doc::OriginElement)>,
    /// The component statuses of this frame: one query for the whole tree, not one per row.
    pub statuses: std::collections::BTreeMap<gc_core::AnyId, gc_doc::ComponentStatus>,
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
                edit: None,
                focus: false,
                dim: false,
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
    state.statuses = session.component_statuses();
    gc_ui::panel::header(ui, tr!(lang, PanelBrowser));
    // F2 renames the selected node when the mouse is over the browser (over the 3D view, F2 pans).
    let hovering = ui.rect_contains_pointer(ui.max_rect());
    if hovering && ui.input(|i| i.key_pressed(egui::Key::F2)) {
        let target = session
            .selected_item()
            .map(cell_any)
            .or_else(|| session.active_sketch().map(|(id, _)| id.erase()));
        if let Some(target) = target
            && let Some(name) = name_of(session, target)
        {
            state.rename = Some((target, name));
            state.rename_fresh = true;
        }
    }
    // The document's tree, as in an assembly browser: the document, its relationships and
    // representations, the origin, and the components (bodies and robots, "name:1" each) with an origin
    // folder of their own. What is specific to the robot cell follows in folders of its own.
    let doc = session.doc();
    let selected = session.selected_item();
    use gc_session::CellItem;
    type Row = (String, String, CellItem);

    let components: Vec<Row> = session
        .components()
        .iter()
        .filter(|node| node.parent.is_none())
        .filter_map(|node| {
            let gc_doc::Occurrence::Item(any) = node.occurrence else { return None };
            let item = match any.kind {
                gc_core::Kind::Body => CellItem::Body(any.downcast()?),
                gc_core::Kind::RobotInstance => CellItem::Robot(any.downcast()?),
                _ => return None,
            };
            Some((node.name.clone(), format!("{}:1", node.name), item))
        })
        .collect();
    let document = session.file_path().and_then(|p| p.file_stem().map(|n| n.to_string_lossy().into_owned()));
    let document = document.unwrap_or_else(|| tr!(lang, BrowserDocument).to_owned());
    egui::CollapsingHeader::new(RichText::new(document).strong())
        .id_salt("gc_browser_document")
        .default_open(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            egui::CollapsingHeader::new(tr!(lang, BrowserRelationships)).default_open(true).show(ui, |ui| {
                for (id, relationship) in session.relationships() {
                    let suppressed = session.doc().is_inactive(id.erase());
                    let delete = tr!(lang, CmdDeleteItem, relationship.name);
                    let row = TreeRow {
                        icon: Icon::Path,
                        text: &relationship.name,
                        selected: false,
                        visible: None,
                        delete: Some(&delete),
                        edit: None,
                        focus: false,
                        dim: suppressed,
                    };
                    let result = gc_ui::panel::tree_row(ui, row);
                    if let Some(status) = session.relationship_status(id).filter(|_| !suppressed) {
                        use gc_doc::RelationStatus;
                        status_dot(
                            ui,
                            result.row.rect,
                            Some(status == RelationStatus::Full),
                            status == RelationStatus::Sick,
                        );
                    }
                    // A rigid joint has nothing to drive.
                    let drivable = !matches!(relationship.kind, gc_doc::RelationKind::Rigid { .. });
                    result.row.context_menu(|ui| {
                        if ui.add_enabled(drivable, egui::Button::new(tr!(lang, MenuDrive))).clicked() {
                            action = Some(BrowserAction::DriveRelationship(id));
                            ui.close();
                        }
                        let text = if suppressed { tr!(lang, CmdUnsuppress) } else { tr!(lang, CmdSuppress) };
                        if ui.button(text).clicked() {
                            action = Some(BrowserAction::SuppressRelationship(id, !suppressed));
                            ui.close();
                        }
                        if ui.button(delete.as_str()).clicked() {
                            action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                                gc_doc::CellOp::DeleteRelationship(id),
                            ))));
                            ui.close();
                        }
                    });
                    if result.deleted {
                        action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                            gc_doc::CellOp::DeleteRelationship(id),
                        ))));
                    }
                }
            });
            let patterns = session.patterns();
            if !patterns.is_empty() {
                egui::CollapsingHeader::new(tr!(lang, BrowserPatterns)).default_open(true).show(ui, |ui| {
                    for (index, pattern) in patterns.iter().enumerate() {
                        let delete = tr!(lang, CmdDeleteItem, pattern.name);
                        let text = format!("{} ({})", pattern.name, pattern.layout.count());
                        let row = TreeRow {
                            icon: Icon::Frame,
                            text: &text,
                            selected: false,
                            visible: None,
                            delete: Some(&delete),
                            edit: None,
                            focus: false,
                            dim: false,
                        };
                        let result = gc_ui::panel::tree_row(ui, row);
                        let remove = gc_doc::CellOp::DeletePattern { index, keep_copies: false };
                        result.row.context_menu(|ui| {
                            if ui.button(delete.as_str()).clicked() {
                                action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(
                                    remove.clone(),
                                ))));
                                ui.close();
                            }
                        });
                        if result.deleted {
                            action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(remove))));
                        }
                    }
                });
            }
            let row = TreeRow {
                icon: Icon::Path,
                text: tr!(lang, BrowserRepresentations),
                selected: false,
                visible: None,
                delete: None,
                edit: None,
                focus: false,
                dim: false,
            };
            gc_ui::panel::tree_row(ui, row);
            egui::CollapsingHeader::new(tr!(lang, BrowserOrigin)).default_open(false).show(ui, |ui| {
                for (_, frame) in
                    session.doc().frames().iter().filter(|(_, f)| f.kind == FrameKind::OriginPlane)
                {
                    let row = TreeRow {
                        icon: Icon::Frame,
                        text: &frame.name,
                        selected: false,
                        visible: None,
                        delete: None,
                        edit: None,
                        focus: false,
                        dim: false,
                    };
                    gc_ui::panel::tree_row(ui, row);
                }
            });
            browser_section(ui, session, lang, state, selected, &mut action, None, components);
        });
    egui::CollapsingHeader::new(tr!(lang, BrowserSketches)).default_open(true).show(ui, |ui| {
        let active = session.active_sketch().map(|(id, _)| id);
        ui.spacing_mut().item_spacing.y = 1.0;
        for (id, sketch) in session.doc().sketches().iter() {
            let node = Node {
                icon: Icon::Sketch,
                name: &sketch.name,
                text: &sketch.name,
                any: id.erase(),
                selected: active == Some(id),
                delete: gc_doc::Command::DeleteSketch { id },
                suppressible: true,
            };
            let result = node_row(ui, session, lang, state, &node, &mut action, |ui, action| {
                if ui.button(tr!(lang, CmdEditSketch)).clicked() {
                    *action = Some(BrowserAction::EditSketch(id));
                    ui.close();
                }
            });
            if result.double_clicked() {
                action = Some(BrowserAction::EditSketch(id));
            }
        }
    });
    if let Some(command) = params(ui, session, lang, state) {
        action = Some(BrowserAction::Edit(Box::new(command)));
    }
    let row = |name: &str, item| (name.to_owned(), name.to_owned(), item);
    browser_section(
        ui,
        session,
        lang,
        state,
        selected,
        &mut action,
        Some(tr!(lang, BrowserFrames)),
        doc.work_frames().iter().map(|(id, f)| row(&f.name, CellItem::Frame(id))).collect(),
    );
    browser_section(
        ui,
        session,
        lang,
        state,
        selected,
        &mut action,
        Some(tr!(lang, BrowserWaypoints)),
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
    browser_section(ui, session, lang, state, selected, &mut action, Some(tr!(lang, BrowserPaths)), paths);
    action
}

/// A group of rows of the browser tree (components, frames, waypoints, paths): a folder with `title`, or
/// the rows alone.
#[allow(clippy::too_many_arguments, reason = "the browser's shared state")]
fn browser_section(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    selected: Option<gc_session::CellItem>,
    action: &mut Option<BrowserAction>,
    title: Option<&str>,
    items: Vec<(String, String, gc_session::CellItem)>,
) {
    use gc_session::CellItem;
    if items.is_empty() {
        return;
    }
    let rows = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (name, shown, item) in items {
            let icon = match item {
                CellItem::Body(_) => Icon::Cuboid,
                CellItem::Robot(_) => Icon::Robot,
                CellItem::Frame(_) => Icon::Frame,
                CellItem::Waypoint(_) => Icon::Waypoint,
                CellItem::Path(_) => Icon::Path,
            };
            let node = Node {
                icon,
                name: &name,
                text: &shown,
                any: cell_any(item),
                selected: selected == Some(item),
                delete: gc_doc::Command::Cell(item.delete_op()),
                suppressible: matches!(item, CellItem::Waypoint(_) | CellItem::Path(_)),
            };
            let result = node_row(ui, session, lang, state, &node, action, |ui, action| {
                if let CellItem::Body(_) | CellItem::Robot(_) = item {
                    // Inventor: right-click a component > Contact Set.
                    let any = cell_any(item);
                    let member = session.doc().in_contact_set(any);
                    if ui.checkbox(&mut { member }, tr!(lang, CmdContactSet)).clicked() {
                        *action = Some(BrowserAction::ContactSet(any, !member));
                        ui.close();
                    }
                }
                if let CellItem::Path(path) = item
                    && ui.button(tr!(lang, CmdPlan)).clicked()
                {
                    *action = Some(BrowserAction::Plan(path));
                    ui.close();
                }
            });
            if result.clicked() || result.secondary_clicked() {
                *action = Some(BrowserAction::Select(Some(item)));
            }
            // Components have an origin folder of their own.
            if let CellItem::Body(_) | CellItem::Robot(_) = item {
                origin_folder(ui, session, lang, state, gc_doc::Occurrence::Item(cell_any(item)), action);
                if let CellItem::Robot(robot) = item {
                    robot_links(ui, session, lang, state, robot, action);
                }
            }
        }
    };
    match title {
        Some(title) => {
            egui::CollapsingHeader::new(title).default_open(true).show(ui, rows);
        }
        None => rows(ui),
    }
}

/// The links of a robot as components of their own, nested like the kinematic chain, each with an origin
/// folder (a joint origin can be picked on any of them).
fn robot_links(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &ParamsUi,
    robot: gc_core::Id<gc_doc::Robot>,
    action: &mut Option<BrowserAction>,
) {
    let links = session.robot_links(robot);
    if links.is_empty() {
        return;
    }
    let depth = |node: &gc_doc::ComponentNode| {
        let mut depth = 0;
        let mut parent = node.parent;
        while let Some(gc_doc::Occurrence::Link { index, .. }) = parent {
            depth += 1;
            parent = links
                .iter()
                .find(|l| l.occurrence == gc_doc::Occurrence::Link { robot, index })
                .and_then(|l| l.parent);
        }
        depth
    };
    ui.indent(("gc_robot_links", robot.raw()), |ui| {
        egui::CollapsingHeader::new(tr!(lang, BrowserLinks))
            .id_salt(("gc_robot_links_folder", robot.raw()))
            .default_open(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for node in &links {
                    let text = format!("{}{}:1", "  ".repeat(depth(node)), node.name);
                    let row = TreeRow {
                        icon: Icon::Robot,
                        text: &text,
                        selected: false,
                        visible: None,
                        delete: None,
                        edit: None,
                        focus: false,
                        dim: false,
                    };
                    gc_ui::panel::tree_row(ui, row);
                    origin_folder(ui, session, lang, state, node.occurrence, action);
                }
            });
    });
}

/// A small dot at the right of a row: filled (fully constrained), hollow (can still move) or a dash
/// (not satisfied), as Inventor marks components and constraints.
fn status_dot(ui: &Ui, row: egui::Rect, full: Option<bool>, sick: bool) {
    let at = egui::pos2(row.right() - 34.0, row.center().y);
    let painter = ui.painter();
    if sick {
        painter.line_segment(
            [at - egui::vec2(4.0, 0.0), at + egui::vec2(4.0, 0.0)],
            egui::Stroke::new(2.0, colors::ERROR),
        );
    } else if let Some(full) = full {
        if full {
            painter.circle_filled(at, 4.0, colors::text());
        } else {
            painter.circle_stroke(at, 3.5, egui::Stroke::new(1.3, colors::text()));
        }
    }
}

/// The name of an origin element.
fn origin_name(lang: Lang, element: gc_doc::OriginElement) -> &'static str {
    use gc_doc::{OriginElement, OriginPlane};
    match element {
        OriginElement::Plane(OriginPlane::Yz) => tr!(lang, OriginYz),
        OriginElement::Plane(OriginPlane::Xz) => tr!(lang, OriginXz),
        OriginElement::Plane(OriginPlane::Xy) => tr!(lang, OriginXy),
        OriginElement::Axis(0) => tr!(lang, OriginAxisX),
        OriginElement::Axis(1) => tr!(lang, OriginAxisY),
        OriginElement::Axis(_) => tr!(lang, OriginAxisZ),
        OriginElement::Point => tr!(lang, OriginPoint),
    }
}

/// The "origin" folder of a component: three planes, three axes and the centre point, hidden until the
/// user shows them (a check box each; the folder's menu shows or hides all seven).
fn origin_folder(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &ParamsUi,
    occurrence: gc_doc::Occurrence,
    action: &mut Option<BrowserAction>,
) {
    use gc_doc::OriginElement;
    let elements = gc_viewport_elements();
    let all_shown = elements.iter().all(|e| session.origin_visible(occurrence, *e));
    ui.indent(("gc_origin_indent", occurrence), |ui| {
        let header = egui::CollapsingHeader::new(tr!(lang, BrowserOrigin))
            .id_salt(("gc_origin_folder", occurrence))
            .default_open(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for element in elements {
                    let name = origin_name(lang, element);
                    let mut visible = session.origin_visible(occurrence, element);
                    let show_label = tr!(lang, ShowItem, name);
                    let icon = match element {
                        OriginElement::Plane(_) => Icon::Sketch,
                        OriginElement::Axis(_) => Icon::Line,
                        OriginElement::Point => Icon::Waypoint,
                    };
                    let row = TreeRow {
                        icon,
                        text: name,
                        selected: state.origin_selected == Some((occurrence, element)),
                        visible: Some((&mut visible, show_label.as_str())),
                        delete: None,
                        edit: None,
                        focus: false,
                        dim: false,
                    };
                    let result = gc_ui::panel::tree_row(ui, row);
                    if result.toggled {
                        *action = Some(BrowserAction::OriginVisible(occurrence, vec![element], visible));
                    }
                    if result.row.clicked() || result.row.secondary_clicked() {
                        *action = Some(BrowserAction::OriginSelect(Some((occurrence, element))));
                    }
                    result.row.context_menu(|ui| {
                        if let OriginElement::Plane(plane) = element
                            && ui.button(tr!(lang, CmdSketchHere)).clicked()
                        {
                            *action = Some(BrowserAction::SketchOnComponent(occurrence, plane));
                            ui.close();
                        }
                        let text = if visible { tr!(lang, CmdHide) } else { tr!(lang, CmdShow) };
                        if ui.button(text).clicked() {
                            *action = Some(BrowserAction::OriginVisible(occurrence, vec![element], !visible));
                            ui.close();
                        }
                    });
                }
            });
        header.header_response.context_menu(|ui| {
            let text = if all_shown { tr!(lang, CmdHideOrigin) } else { tr!(lang, CmdShowOrigin) };
            if ui.button(text).clicked() {
                *action = Some(BrowserAction::OriginVisible(occurrence, elements.to_vec(), !all_shown));
                ui.close();
            }
        });
    });
}

/// The seven origin elements in the order of the folder.
fn gc_viewport_elements() -> [gc_doc::OriginElement; 7] {
    use gc_doc::{OriginElement, OriginPlane};
    [
        OriginElement::Plane(OriginPlane::Yz),
        OriginElement::Plane(OriginPlane::Xz),
        OriginElement::Plane(OriginPlane::Xy),
        OriginElement::Axis(0),
        OriginElement::Axis(1),
        OriginElement::Axis(2),
        OriginElement::Point,
    ]
}

/// The id of a cell item, whatever its kind.
fn cell_any(item: gc_session::CellItem) -> gc_core::AnyId {
    use gc_session::CellItem;
    match item {
        CellItem::Body(id) => id.erase(),
        CellItem::Robot(id) => id.erase(),
        CellItem::Waypoint(id) => id.erase(),
        CellItem::Frame(id) => id.erase(),
        CellItem::Path(id) => id.erase(),
    }
}

/// The name of a renamable browser item.
fn name_of(session: &Session, item: gc_core::AnyId) -> Option<String> {
    use gc_core::Kind;
    let doc = session.doc();
    match item.kind {
        Kind::Sketch => doc.get(item.downcast()?).map(|s: &Sketch| s.name.clone()),
        Kind::Body => doc.get(item.downcast()?).map(|b: &gc_doc::Body| b.name.clone()),
        Kind::RobotInstance => doc.get(item.downcast()?).map(|r: &gc_doc::Robot| r.name.clone()),
        Kind::Waypoint => doc.get(item.downcast()?).map(|w: &gc_doc::Waypoint| w.name.clone()),
        Kind::Path => doc.get(item.downcast()?).map(|p: &gc_doc::MotionPath| p.name.clone()),
        Kind::WorkFrame => doc.get(item.downcast()?).map(|f: &gc_doc::WorkFrame| f.name.clone()),
        _ => None,
    }
}

/// A row of the browser tree.
struct Node<'a> {
    icon: Icon,
    /// The name, for commands and accessible names.
    name: &'a str,
    /// The text shown (the name, and for paths the plan status).
    text: &'a str,
    any: gc_core::AnyId,
    selected: bool,
    /// The edit that deletes it.
    delete: gc_doc::Command,
    /// Whether it can be suppressed.
    suppressible: bool,
}

/// One row of the tree with everything the browser offers on a node: a visibility check box, rename in
/// place (F2, a second click, the menu), suppress, delete and the node's own menu entries.
fn node_row(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    state: &mut ParamsUi,
    node: &Node<'_>,
    action: &mut Option<BrowserAction>,
    menu: impl FnOnce(&mut Ui, &mut Option<BrowserAction>),
) -> egui::Response {
    let doc = session.doc();
    let mut visible = doc.is_visible(node.any);
    let inactive = doc.is_inactive(node.any);
    let show_label = tr!(lang, ShowItem, node.name);
    let delete_label = tr!(lang, CmdDeleteItem, node.name);
    let renaming = state.rename.as_ref().is_some_and(|(id, _)| *id == node.any);
    let fresh = renaming && std::mem::take(&mut state.rename_fresh);
    let row = TreeRow {
        icon: node.icon,
        text: node.text,
        selected: node.selected,
        visible: Some((&mut visible, show_label.as_str())),
        delete: Some(&delete_label),
        edit: state.rename.as_mut().filter(|_| renaming).map(|(_, text)| text),
        focus: fresh,
        dim: inactive,
    };
    let result = gc_ui::panel::tree_row(ui, row);
    if let Some(&status) = state.statuses.get(&node.any) {
        use gc_doc::ComponentStatus;
        status_dot(
            ui,
            result.row.rect,
            Some(status == ComponentStatus::Full),
            status == ComponentStatus::Unknown,
        );
    }
    match result.edited {
        Some(true) => {
            if let Some((item, name)) = state.rename.take() {
                *action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Rename { item, name })));
            }
        }
        Some(false) => state.rename = None,
        None => {}
    }
    if result.toggled {
        let command = gc_doc::Command::SetVisible { item: node.any, visible };
        *action = Some(BrowserAction::Edit(Box::new(command)));
    }
    if result.row.clicked() && node.selected && !result.row.double_clicked() && !renaming {
        // A second click on the selected node edits its name.
        state.rename = Some((node.any, node.name.to_owned()));
        state.rename_fresh = true;
    }
    result.row.context_menu(|ui| {
        ui.label(RichText::new(node.name).strong());
        ui.separator();
        menu(ui, action);
        if name_of(session, node.any).is_some() && ui.button(tr!(lang, CmdRename)).clicked() {
            state.rename = Some((node.any, node.name.to_owned()));
            state.rename_fresh = true;
            ui.close();
        }
        let text = if visible { tr!(lang, CmdHide) } else { tr!(lang, CmdShow) };
        if ui.button(text).clicked() {
            let command = gc_doc::Command::SetVisible { item: node.any, visible: !visible };
            *action = Some(BrowserAction::Edit(Box::new(command)));
            ui.close();
        }
        if node.suppressible {
            let text = if inactive { tr!(lang, CmdUnsuppress) } else { tr!(lang, CmdSuppress) };
            if ui.button(text).clicked() {
                let op = gc_doc::CellOp::SetSuppressed { item: node.any, suppressed: !inactive };
                *action = Some(BrowserAction::Edit(Box::new(gc_doc::Command::Cell(op))));
                ui.close();
            }
        }
        if ui.button(delete_label.as_str()).clicked() {
            *action = Some(BrowserAction::Edit(Box::new(node.delete.clone())));
            ui.close();
        }
    });
    if result.deleted {
        *action = Some(BrowserAction::Edit(Box::new(node.delete.clone())));
    }
    result.row
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
        Prompt::PointPlace => Msg::PromptPointPlace,
        Prompt::PolygonCentre => Msg::PromptPolygonCentre,
        Prompt::PolygonCorner => Msg::PromptPolygonCorner,
        Prompt::SlotFirst => Msg::PromptSlotFirst,
        Prompt::SlotSecond => Msg::PromptSlotSecond,
        Prompt::SlotWidth => Msg::PromptSlotWidth,
        Prompt::OffsetPick => Msg::PromptOffsetPick,
        Prompt::OffsetPlace => Msg::PromptOffsetPlace,
    };
    message.text(lang)
}

/// The status bar: what the tool expects next, why the last edit was refused, a notice from the
/// application (file saved, file error) and how constrained the sketch is.
pub fn status_bar(
    ui: &mut Ui,
    session: &Session,
    lang: Lang,
    gpu: bool,
    notice: Option<&str>,
    slice: Option<&mut bool>,
) {
    ui.horizontal(|ui| {
        if session.active_sketch().is_some() {
            ui.label(prompt_text(lang, session.prompt()));
        } else {
            ui.label(tr!(lang, StatusReady));
        }
        // Slice Graphics (F7): offered while a 2D sketch is edited.
        if let Some(slice) = slice {
            ui.separator();
            let label = tr!(lang, StatusSlice);
            let button = ui.selectable_label(*slice, label);
            button.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, *slice, label));
            if button.clicked() {
                *slice = !*slice;
            }
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
                gc_doc::EditError::NameTaken => Msg::ErrNameTaken,
                gc_doc::EditError::Unsolvable { .. } => Msg::ErrUnsolvable,
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
            // At the far right, as in Inventor: the number of occurrences, and of open documents.
            ui.weak(tr!(lang, StatusDocuments, 1));
            ui.separator();
            ui.weak(tr!(lang, StatusOccurrences, session.components().len()));
            ui.separator();
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
