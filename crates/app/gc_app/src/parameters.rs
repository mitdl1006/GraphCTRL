//! The Parameters dialog, as Inventor's (Manage tab > Parameters panel > Parameters;
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-595F9D8E-9EEA-4446-91AB-830E5BAE548E.htm):
//! Model Parameters (the sketch dimensions) and User Parameters in tables with Name, Unit, Equation and
//! Nominal Value; a search box that filters every field; Add Numeric; right-click a row to Delete
//! Parameter or Copy to User Parameter (a copy named `<name>_copy`); Enter applies an equation.

use gc_core::Id;
use gc_doc::{Command, Param, SketchOp};
use gc_i18n::{Lang, tr};
use gc_session::Session;
use gc_sketch::{DimKind, Dimension};

/// A row of either table.
struct Row {
    name: String,
    unit: &'static str,
    source: String,
    value: String,
    /// What editing the equation changes; `None` for a reference dimension.
    target: Option<Target>,
}

#[derive(Clone, Copy)]
enum Target {
    Dimension(Id<gc_doc::Sketch>, Id<Dimension>),
    User(Id<Param>),
}

/// The Parameters dialog's state.
#[derive(Default)]
pub(crate) struct Parameters {
    /// Whether the dialog is open.
    pub open: bool,
    search: String,
    /// Equations being typed, by row name.
    editing: std::collections::BTreeMap<String, String>,
    /// User parameter names being typed, by parameter.
    naming: std::collections::BTreeMap<Id<Param>, String>,
    /// Whether a new numeric parameter is an angle.
    angle: bool,
    error: Option<String>,
}

fn unit_and_value(kind: &DimKind, value: f64) -> (&'static str, String) {
    if matches!(kind, DimKind::Angle { .. }) {
        ("deg", format!("{:.6}", value.to_degrees()))
    } else {
        ("mm", format!("{:.6}", value * 1000.0))
    }
}

fn trim(number: String) -> String {
    let trimmed = number.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" { "0".to_owned() } else { trimmed.to_owned() }
}

impl Parameters {
    fn model_rows(session: &Session, lang: Lang) -> Vec<Row> {
        let mut rows = Vec::new();
        for (sketch_id, sketch) in session.doc().sketches().iter() {
            for (index, (id, dimension)) in sketch.dimensions.iter().enumerate() {
                let kind = match dimension.kind {
                    DimKind::Angle { .. } => tr!(lang, ParamKindAngle),
                    DimKind::Radius(_) => tr!(lang, ParamKindRadius),
                    DimKind::Diameter(_) => tr!(lang, ParamKindDiameter),
                    DimKind::Distance { .. }
                    | DimKind::HDistance { .. }
                    | DimKind::VDistance { .. }
                    | DimKind::PointLine { .. }
                    | DimKind::PlaneDistance { .. } => tr!(lang, ParamKindLength),
                };
                let (unit, value) = unit_and_value(&dimension.kind, dimension.value);
                let driving = dimension.driving;
                rows.push(Row {
                    name: tr!(lang, ParamDimension, sketch.name, kind, index + 1),
                    unit,
                    source: if driving {
                        dimension.source.clone()
                    } else {
                        tr!(lang, ParamDriven).to_owned()
                    },
                    value: trim(value),
                    target: driving.then_some(Target::Dimension(sketch_id, *id)),
                });
            }
        }
        rows
    }

    fn user_rows(session: &Session) -> Vec<Row> {
        session
            .doc()
            .params()
            .iter()
            .map(|(id, param)| {
                let angle = param.unit == gc_expr::Unit::Deg;
                let (unit, value) = if angle {
                    ("deg", format!("{:.6}", param.value.to_degrees()))
                } else {
                    ("mm", format!("{:.6}", param.value * 1000.0))
                };
                Row {
                    name: param.name.clone(),
                    unit,
                    source: param.source.clone(),
                    value: trim(value),
                    target: Some(Target::User(id)),
                }
            })
            .collect()
    }

    fn matches(&self, row: &Row) -> bool {
        let needle = self.search.trim().to_lowercase();
        needle.is_empty()
            || [&row.name, &row.source, &row.value, &row.unit.to_owned()]
                .iter()
                .any(|field| field.to_lowercase().contains(&needle))
    }

    /// Runs a command; a refusal is shown in the dialog.
    fn run(&mut self, session: &mut Session, command: Command) {
        self.error = session.execute(&command).err().map(|e| e.to_string());
    }

    /// A free name for a new or copied parameter.
    fn free_name(session: &Session, base: &str) -> String {
        let taken = |name: &str| session.doc().params().iter().any(|(_, p)| p.name == name);
        if !taken(base) {
            return base.to_owned();
        }
        (2..).map(|n| format!("{base}{n}")).find(|name| !taken(name)).unwrap_or_else(|| base.to_owned())
    }

    fn table(&mut self, ui: &mut egui::Ui, session: &mut Session, lang: Lang, id: &str, rows: &[Row]) {
        let mut command = None;
        egui::Grid::new(id).num_columns(4).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for heading in [
                tr!(lang, ParamColName),
                tr!(lang, ParamColUnit),
                tr!(lang, ParamColEquation),
                tr!(lang, ParamColValue),
            ] {
                ui.label(egui::RichText::new(heading).strong());
            }
            ui.end_row();
            let shown: Vec<&Row> = rows.iter().filter(|r| self.matches(r)).collect();
            for row in shown {
                let name = ui
                    .horizontal(|ui| {
                        let name = ui.label(&row.name);
                        // A user parameter's name is edited in place, as in Inventor (Enter renames it, and every
                        // equation that uses it follows).
                        if let Some(Target::User(id)) = row.target {
                            let text = self.naming.entry(id).or_insert_with(|| row.name.clone());
                            let field = ui
                                .add(egui::TextEdit::singleline(text).desired_width(90.0))
                                .on_hover_text(tr!(lang, ParamColName));
                            field.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::TextInput,
                                    true,
                                    format!("{} name", row.name),
                                )
                            });
                            if field.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                && *text != row.name
                            {
                                command = Some(Command::RenameParam { id, name: text.clone() });
                            } else if !field.has_focus() && *text != row.name {
                                text.clone_from(&row.name);
                            }
                        }
                        name
                    })
                    .inner;
                ui.label(row.unit);
                match row.target {
                    Some(target) => {
                        let text = self.editing.entry(row.name.clone()).or_insert_with(|| row.source.clone());
                        let field = ui
                            .add(egui::TextEdit::singleline(text).desired_width(140.0))
                            .labelled_by(name.id);
                        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let source = text.clone();
                            command = Some(match target {
                                Target::Dimension(sketch, id) => {
                                    Command::Sketch { sketch, op: SketchOp::SetDimension { id, source } }
                                }
                                Target::User(id) => Command::SetParam { id, source },
                            });
                        } else if !field.has_focus() && *text != row.source {
                            // Not being typed: follow the document.
                            text.clone_from(&row.source);
                        }
                        if let Target::User(id) = target {
                            name.context_menu(|ui| {
                                if ui.button(tr!(lang, ParamDelete)).clicked() {
                                    command = Some(Command::DeleteParam { id });
                                    ui.close();
                                }
                                if ui.button(tr!(lang, ParamCopyToUser)).clicked() {
                                    command = Some(Command::AddParam {
                                        name: Self::free_name(session, &format!("{}_copy", row.name)),
                                        source: row.source.clone(),
                                        angle: row.unit == "deg",
                                    });
                                    ui.close();
                                }
                            });
                        } else {
                            name.context_menu(|ui| {
                                if ui.button(tr!(lang, ParamCopyToUser)).clicked() {
                                    let base = row.name.split_whitespace().next().unwrap_or("d").to_owned();
                                    command = Some(Command::AddParam {
                                        name: Self::free_name(session, &format!("{base}_copy")),
                                        source: format!("{} {}", row.value, row.unit),
                                        angle: row.unit == "deg",
                                    });
                                    ui.close();
                                }
                            });
                        }
                    }
                    None => {
                        ui.label(egui::RichText::new(&row.source).weak());
                    }
                }
                ui.label(egui::RichText::new(&row.value).monospace());
                ui.end_row();
            }
        });
        if let Some(command) = command {
            self.run(session, command);
        }
    }

    /// The dialog.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut done = false;
        egui::Window::new(tr!(lang, MenuParameters)).open(&mut open).default_width(560.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let label = ui.label(tr!(lang, ParamSearch));
                ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(200.0))
                    .labelled_by(label.id);
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                let model = Self::model_rows(session, lang);
                egui::CollapsingHeader::new(tr!(lang, ParamModel)).default_open(true).show(ui, |ui| {
                    self.table(ui, session, lang, "params_model", &model);
                });
                let user = Self::user_rows(session);
                egui::CollapsingHeader::new(tr!(lang, ParamUser)).default_open(true).show(ui, |ui| {
                    self.table(ui, session, lang, "params_user", &user);
                });
            });
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr!(lang, ParamAddNumeric)).clicked() {
                    let name = Self::free_name(session, "Param");
                    let source = if self.angle { "0 deg" } else { "0 mm" }.to_owned();
                    let angle = self.angle;
                    self.run(session, Command::AddParam { name, source, angle });
                }
                ui.selectable_value(&mut self.angle, false, tr!(lang, ParamUnitLength));
                ui.selectable_value(&mut self.angle, true, tr!(lang, ParamUnitAngle));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr!(lang, ParamDone)).clicked() {
                        done = true;
                    }
                });
            });
        });
        self.open = open && !done;
    }
}
