//! The sketch Mirror and Pattern dialogs, and the option windows of the Polygon and Offset tools, as
//! Inventor's (Inventor 2026 help: To Create and Edit Sketch Patterns,
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-AAD3A699-FED8-4614-A31D-0A2316328CEE.htm;
//! To Create 2D Shape Geometry, GUID-D489CE6D-7299-4211-A43A-F3580A4BA357; To Stretch, Scale, or Offset 2D
//! Sketch Geometry, GUID-962CEE71-8BE4-4DA6-B51E-341319C64FA4).
//!
//! - Mirror: geometry, Mirror Line; Apply (and continue) or Done.
//! - Rectangular: geometry, Direction 1 and 2 each with Count and Spacing (an equation), Flip, Fitted.
//! - Circular: geometry, Axis (a point or a circle's centre), Count, Angle (an equation), Flip, Fitted.
//! - Polygon: Inscribed or Circumscribed and the number of edges, while the tool runs.
//! - Offset: Loop Select and Constrain Offset (Inventor's right-click toggles), while the tool runs.
//!
//! As in Inventor the dialog stays open while geometry is picked in the graphics window: the pressed
//! selector (Geometry, Mirror Line, Direction 1/2, Axis) takes the clicks (`Session::set_sketch_pick`).
//! Geometry starts as the selection the dialog was opened with.
//!
//! Opened on an existing pattern (a selected original or copy) the dialog edits it: the values are the
//! pattern's, a Suppress selector picks elements to suppress, OK rebuilds the pattern and Delete Pattern
//! removes it (Edit Rectangular and Circular Patterns in the same help page).

use gc_core::Id;
use gc_doc::{SketchOp, SketchPatternDirection};
use gc_i18n::{Lang, tr};
use gc_session::{Session, SketchPick};
use gc_sketch::{Curve, Item, PatternDefinition, SkPoint, SketchPattern};
use gc_tools::ToolKind;

/// A sketch command's window: at the right, below the ribbon, so that it covers neither the ribbon nor
/// the middle of the sketch.
fn sketch_window(title: &str, ctx: &egui::Context) -> egui::Window<'static> {
    let at = ctx.content_rect().right_top() + egui::vec2(-340.0, 150.0);
    egui::Window::new(title.to_owned()).default_pos(at)
}

/// Which dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Mirror,
    Rectangular,
    Circular,
}

/// A selector of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Geometry,
    Line,
    Line2,
    Axis,
    Suppress,
}

impl Field {
    fn pick(self) -> SketchPick {
        match self {
            Field::Geometry | Field::Suppress => SketchPick::Curves,
            Field::Line | Field::Line2 => SketchPick::Line,
            Field::Axis => SketchPick::Point,
        }
    }
}

/// A pattern's elements: the curves of each copy.
type Elements = Vec<Vec<Id<Curve>>>;

/// A sketch Mirror or Pattern dialog.
pub(crate) struct SketchDialog {
    kind: Kind,
    active: Field,
    geometry: Vec<Id<Curve>>,
    line: Option<Id<Curve>>,
    line2: Option<Id<Curve>>,
    axis: Option<Id<SkPoint>>,
    count: u32,
    count2: u32,
    spacing: String,
    spacing2: String,
    angle: String,
    flip: bool,
    flip2: bool,
    fitted: bool,
    error: Option<String>,
    /// The pattern being edited: its elements and which are suppressed.
    editing: Option<(Elements, Vec<usize>)>,
}

impl SketchDialog {
    /// A dialog whose Geometry selector is active and holds the current selection.
    pub(crate) fn new(kind: Kind, session: &mut Session) -> Self {
        let mut dialog = Self {
            kind,
            active: Field::Geometry,
            geometry: session.selected_curves(),
            line: None,
            line2: None,
            axis: None,
            count: if kind == Kind::Circular { 6 } else { 2 },
            count2: 1,
            spacing: "10 mm".to_owned(),
            spacing2: "10 mm".to_owned(),
            angle: "360 deg".to_owned(),
            flip: false,
            flip2: false,
            fitted: kind == Kind::Circular,
            error: None,
            editing: None,
        };
        dialog.activate(Field::Geometry, session);
        dialog
    }

    /// A dialog on an existing pattern (Edit Pattern), holding its values.
    pub(crate) fn edit(pattern: &SketchPattern, session: &mut Session) -> Self {
        let kind = match pattern.definition {
            PatternDefinition::Rectangular { .. } => Kind::Rectangular,
            PatternDefinition::Circular { .. } => Kind::Circular,
        };
        let mut dialog = Self::new(kind, session);
        dialog.geometry = pattern.definition.curves().to_vec();
        match &pattern.definition {
            PatternDefinition::Rectangular { first, second, fitted, .. } => {
                dialog.line = Some(first.line);
                dialog.count = first.count;
                dialog.spacing.clone_from(&first.spacing);
                dialog.flip = first.flip;
                if let Some(second) = second {
                    dialog.line2 = Some(second.line);
                    dialog.count2 = second.count;
                    dialog.spacing2.clone_from(&second.spacing);
                    dialog.flip2 = second.flip;
                }
                dialog.fitted = *fitted;
            }
            PatternDefinition::Circular { axis, count, angle, flip, fitted, .. } => {
                dialog.axis = Some(*axis);
                dialog.count = *count;
                dialog.angle.clone_from(angle);
                dialog.flip = *flip;
                dialog.fitted = *fitted;
            }
        }
        dialog.editing = Some((pattern.elements.clone(), pattern.suppressed.clone()));
        dialog.activate(Field::Suppress, session);
        dialog
    }

    /// What `field` holds, as selection items.
    fn items(&self, field: Field) -> Vec<Item> {
        match field {
            Field::Geometry => self.geometry.iter().map(|c| Item::Curve(*c)).collect(),
            Field::Line => self.line.map(Item::Curve).into_iter().collect(),
            Field::Line2 => self.line2.map(Item::Curve).into_iter().collect(),
            Field::Axis => self.axis.map(Item::Point).into_iter().collect(),
            Field::Suppress => match &self.editing {
                Some((elements, suppressed)) => suppressed
                    .iter()
                    .filter_map(|e| elements.get(*e))
                    .flat_map(|e| e.iter().map(|c| Item::Curve(*c)))
                    .collect(),
                None => Vec::new(),
            },
        }
    }

    /// Presses a selector: the viewport's clicks go to it from now on.
    fn activate(&mut self, field: Field, session: &mut Session) {
        self.active = field;
        session.set_sketch_pick(Some(field.pick()), self.items(field));
    }

    /// Reads the active selector's picks back from the session.
    fn take_picks(&mut self, session: &Session) {
        match self.active {
            Field::Geometry => self.geometry = session.selected_curves(),
            Field::Line => self.line = session.selected_curves().first().copied(),
            Field::Line2 => {
                self.line2 = session.selected_curves().first().copied();
                if self.line2.is_some() {
                    self.count2 = self.count2.max(2);
                }
            }
            Field::Axis => self.axis = session.selected_points().first().copied(),
            Field::Suppress => {
                let picked = session.selected_curves();
                if let Some((elements, suppressed)) = &mut self.editing {
                    // An element is suppressed when any of its curves is picked.
                    *suppressed = (0..elements.len())
                        .filter(|e| elements[*e].iter().any(|c| picked.contains(c)))
                        .collect();
                }
            }
        }
    }

    /// The pattern's values (rectangular and circular).
    fn definition(&self) -> Option<PatternDefinition> {
        match self.op()? {
            SketchOp::RectangularPattern { curves, first, second, fitted } => {
                Some(PatternDefinition::Rectangular { curves, first, second, fitted })
            }
            SketchOp::CircularPattern { curves, axis, count, angle, flip, fitted } => {
                Some(PatternDefinition::Circular { curves, axis, count, angle, flip, fitted })
            }
            _ => None,
        }
    }

    fn op(&self) -> Option<SketchOp> {
        Some(match self.kind {
            Kind::Mirror => SketchOp::Mirror { curves: self.geometry.clone(), axis: self.line? },
            Kind::Rectangular => SketchOp::RectangularPattern {
                curves: self.geometry.clone(),
                first: SketchPatternDirection {
                    line: self.line?,
                    count: self.count,
                    spacing: self.spacing.clone(),
                    flip: self.flip,
                },
                second: self.line2.filter(|_| self.count2 >= 2).map(|line| SketchPatternDirection {
                    line,
                    count: self.count2,
                    spacing: self.spacing2.clone(),
                    flip: self.flip2,
                }),
                fitted: self.fitted,
            },
            Kind::Circular => SketchOp::CircularPattern {
                curves: self.geometry.clone(),
                axis: self.axis?,
                count: self.count,
                angle: self.angle.clone(),
                flip: self.flip,
                fitted: self.fitted,
            },
        })
    }

    /// OK (rebuild with these values) and Delete Pattern for a pattern being edited. True when done.
    fn edit_buttons(&mut self, ui: &mut egui::Ui, session: &mut Session, lang: Lang) -> bool {
        let Some(element) = self.geometry.first().copied() else { return false };
        let mut done = false;
        ui.horizontal(|ui| {
            let ok = ui.add_enabled(self.definition().is_some(), egui::Button::new(tr!(lang, DialogOk)));
            let op = if ok.clicked() {
                let suppressed = self.editing.as_ref().map(|(_, s)| s.clone()).unwrap_or_default();
                self.definition().map(|definition| SketchOp::EditSketchPattern {
                    element,
                    definition,
                    suppressed,
                })
            } else if ui.button(tr!(lang, DeletePattern)).clicked() {
                Some(SketchOp::DeleteSketchPattern { element })
            } else {
                None
            };
            if let Some(op) = op {
                match session.edit_active_sketch(op) {
                    Ok(_) => done = true,
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        });
        done
    }

    /// A selector button: pressed while it takes the clicks.
    fn selector(&mut self, ui: &mut egui::Ui, session: &mut Session, field: Field, text: String) {
        if ui.selectable_label(self.active == field, text).clicked() && self.active != field {
            self.activate(field, session);
        }
    }

    /// Shows the dialog. Returns false when it closes (picking ends then too); another command closes it.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) -> bool {
        if session.sketch_pick().is_none() {
            return false;
        }
        self.take_picks(session);
        let title = match self.kind {
            Kind::Mirror => tr!(lang, SkMirrorTitle),
            Kind::Rectangular => tr!(lang, SkRectTitle),
            Kind::Circular => tr!(lang, SkCircTitle),
        };
        let mut open = true;
        let mut done = false;
        let state = |set: bool| if set { tr!(lang, DialogPicked) } else { tr!(lang, DialogNotPicked) };
        sketch_window(title, ctx).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.weak(tr!(lang, DialogSelectorHint));
            self.selector(ui, session, Field::Geometry, tr!(lang, DialogGeometry, self.geometry.len()));
            match self.kind {
                Kind::Mirror => {
                    self.selector(
                        ui,
                        session,
                        Field::Line,
                        tr!(lang, MirrorLine, state(self.line.is_some())),
                    );
                }
                Kind::Rectangular => {
                    let text = tr!(lang, PatternDirection1, state(self.line.is_some()));
                    self.selector(ui, session, Field::Line, text);
                    ui.horizontal(|ui| {
                        let label = ui.label(tr!(lang, PatternCount)).id;
                        ui.add(egui::DragValue::new(&mut self.count).range(2..=500)).labelled_by(label);
                        let label = ui.label(tr!(lang, SkPatternSpacing)).id;
                        ui.text_edit_singleline(&mut self.spacing).labelled_by(label);
                        ui.checkbox(&mut self.flip, tr!(lang, PatternFlip));
                    });
                    let text = tr!(lang, PatternDirection2, state(self.line2.is_some()));
                    self.selector(ui, session, Field::Line2, text);
                    ui.horizontal(|ui| {
                        let label = ui.label(tr!(lang, SkPatternCount2)).id;
                        ui.add(egui::DragValue::new(&mut self.count2).range(1..=500)).labelled_by(label);
                        let label = ui.label(tr!(lang, SkPatternSpacing2)).id;
                        ui.text_edit_singleline(&mut self.spacing2).labelled_by(label);
                        ui.checkbox(&mut self.flip2, tr!(lang, SkPatternFlip2));
                    });
                    ui.checkbox(&mut self.fitted, tr!(lang, PatternFitted));
                }
                Kind::Circular => {
                    self.selector(
                        ui,
                        session,
                        Field::Axis,
                        tr!(lang, SkPatternAxis, state(self.axis.is_some())),
                    );
                    ui.horizontal(|ui| {
                        let label = ui.label(tr!(lang, PatternCount)).id;
                        ui.add(egui::DragValue::new(&mut self.count).range(2..=500)).labelled_by(label);
                        let label = ui.label(tr!(lang, SkPatternAngle)).id;
                        ui.text_edit_singleline(&mut self.angle).labelled_by(label);
                        ui.checkbox(&mut self.flip, tr!(lang, PatternFlip));
                    });
                    ui.checkbox(&mut self.fitted, tr!(lang, PatternFitted));
                }
            }
            if let Some((_, suppressed)) = &self.editing {
                let text = tr!(lang, SuppressElements, suppressed.len());
                self.selector(ui, session, Field::Suppress, text);
            }
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, tr!(lang, DialogRefused, error));
            }
            if self.editing.is_some() {
                done = self.edit_buttons(ui, session, lang);
                return;
            }
            ui.horizontal(|ui| {
                let ready = self.op().is_some() && !self.geometry.is_empty();
                let apply = ui.add_enabled(ready, egui::Button::new(tr!(lang, DialogApply))).clicked();
                let finish = ui.button(tr!(lang, DialogDone)).clicked();
                if (apply || (finish && ready))
                    && let Some(op) = self.op()
                {
                    match session.edit_active_sketch(op) {
                        Ok(_) => {
                            self.error = None;
                            // Apply continues with new geometry (Inventor's Apply).
                            self.geometry.clear();
                            self.activate(Field::Geometry, session);
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
                done = finish;
            });
        });
        if !open || done {
            session.set_sketch_pick(None, Vec::new());
            return false;
        }
        true
    }
}

/// The Polygon tool's options while it runs; Done or closing leaves the tool.
pub(crate) fn polygon_window(ctx: &egui::Context, session: &mut Session, lang: Lang) {
    let (mut sides, mut circumscribed) = session.polygon_settings();
    let mut open = true;
    let mut done = false;
    sketch_window(tr!(lang, SkPolygonTitle), ctx).open(&mut open).resizable(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut circumscribed, false, tr!(lang, PolygonInscribed));
            ui.radio_value(&mut circumscribed, true, tr!(lang, PolygonCircumscribed));
        });
        ui.horizontal(|ui| {
            let label = ui.label(tr!(lang, PolygonSides)).id;
            ui.add(egui::DragValue::new(&mut sides).range(3..=120)).labelled_by(label);
        });
        done = ui.button(tr!(lang, DialogDone)).clicked();
    });
    session.set_polygon_settings(sides, circumscribed);
    if !open || done {
        session.set_tool(ToolKind::Select);
    }
}

/// The Offset tool's options while it runs.
pub(crate) fn offset_window(ctx: &egui::Context, session: &mut Session, lang: Lang) {
    let (mut loop_select, mut constrain) = session.offset_options();
    sketch_window(tr!(lang, SkOffsetTitle), ctx).resizable(false).show(ctx, |ui| {
        ui.checkbox(&mut loop_select, tr!(lang, OffsetLoopSelect));
        ui.checkbox(&mut constrain, tr!(lang, OffsetConstrain));
    });
    if (loop_select, constrain) != session.offset_options() {
        session.set_offset_options(loop_select, constrain);
    }
}

/// The 2D Chamfer dialog while the Chamfer tool runs (Inventor 2026 help, To Create 2D Shape Geometry,
/// GUID-D489CE6D-7299-4211-A43A-F3580A4BA357, Create Chamfers): Create Dimensions, and the type with its
/// sizes. OK or closing leaves the tool. ("Equal to Parameters" is not offered: each chamfer takes the
/// values as they stand.)
pub(crate) fn chamfer_window(ctx: &egui::Context, session: &mut Session, lang: Lang) {
    use gc_doc::ChamferKind;
    let (mut kind, mut dimensions) = session.chamfer_options();
    let mut open = true;
    let mut done = false;
    sketch_window(tr!(lang, ChamferTitle), ctx).open(&mut open).resizable(false).show(ctx, |ui| {
        ui.checkbox(&mut dimensions, tr!(lang, ChamferCreateDimensions));
        let first = |kind: &ChamferKind| match kind {
            ChamferKind::Equal(d) | ChamferKind::Unequal(d, _) => d.clone(),
            ChamferKind::DistanceAngle { distance, .. } => distance.clone(),
        };
        ui.horizontal(|ui| {
            let d = first(&kind);
            if ui.radio(matches!(kind, ChamferKind::Equal(_)), tr!(lang, ChamferEqual)).clicked() {
                kind = ChamferKind::Equal(d.clone());
            }
            if ui.radio(matches!(kind, ChamferKind::Unequal(..)), tr!(lang, ChamferUnequal)).clicked() {
                kind = ChamferKind::Unequal(d.clone(), d.clone());
            }
            if ui
                .radio(matches!(kind, ChamferKind::DistanceAngle { .. }), tr!(lang, ChamferDistanceAngle))
                .clicked()
            {
                kind = ChamferKind::DistanceAngle { distance: d, angle: "45 deg".to_owned() };
            }
        });
        let field = |ui: &mut egui::Ui, label: &str, text: &mut String| {
            ui.horizontal(|ui| {
                let label = ui.label(label).id;
                ui.text_edit_singleline(text).labelled_by(label);
            });
        };
        match &mut kind {
            ChamferKind::Equal(d) => field(ui, tr!(lang, ChamferDistance), d),
            ChamferKind::Unequal(d1, d2) => {
                field(ui, tr!(lang, ChamferDistance1), d1);
                field(ui, tr!(lang, ChamferDistance2), d2);
            }
            ChamferKind::DistanceAngle { distance, angle } => {
                field(ui, tr!(lang, ChamferDistance), distance);
                field(ui, tr!(lang, ChamferAngle), angle);
            }
        }
        done = ui.button(tr!(lang, DialogOk)).clicked();
    });
    if (kind.clone(), dimensions) != session.chamfer_options() {
        session.set_chamfer_options(kind, dimensions);
    }
    if !open || done {
        session.set_tool(ToolKind::Select);
    }
}

/// The Move, Copy and Rotate dialogs while their tool runs (Inventor 2026 help, To Copy, Move, or Rotate 2D
/// Sketch Geometry, GUID-F8114270-18F7-4C7A-947E-AC489EDACFA9): Copy (Move and Rotate), the More options
/// as one relax box (Always or Never), Done. The selection is what was selected before the command
/// (Inventor's dialog also has a Select mode; here the clicks set the base point straight away).
pub(crate) fn transform_window(ctx: &egui::Context, session: &mut Session, lang: Lang) {
    let tool = session.tool();
    let title = match tool {
        ToolKind::Copy => tr!(lang, ToolCopy),
        ToolKind::Rotate => tr!(lang, ToolRotate),
        ToolKind::Scale => tr!(lang, ToolScale),
        ToolKind::Stretch => tr!(lang, ToolStretch),
        _ => tr!(lang, ToolMove),
    };
    let (mut copy, mut relax) = session.transform_options();
    let mut open = true;
    let mut done = false;
    sketch_window(title, ctx).open(&mut open).resizable(false).show(ctx, |ui| {
        ui.label(tr!(lang, TransformSelection, session.selected_curves().len()));
        // Copy is an option of Move and Rotate (Inventor's dialogs); Scale and Stretch have none.
        if matches!(tool, ToolKind::Move | ToolKind::Rotate) {
            ui.checkbox(&mut copy, tr!(lang, TransformCopy));
        }
        ui.checkbox(&mut relax, tr!(lang, TransformRelax));
        done = ui.button(tr!(lang, DialogDone)).clicked();
    });
    if (copy, relax) != session.transform_options() {
        session.set_transform_options(copy, relax);
    }
    if !open || done {
        session.set_tool(ToolKind::Select);
    }
}

/// The Auto Dimension dialog (Inventor 2026 help, To Apply 2D Sketch Dimensions Automatically,
/// GUID-1DC71C6F-9F6D-4846-8E73-9CB7D3452882): Dimensions and Constraints (both on), Dimensions Required,
/// Apply, Remove, Done. The curves are the selection (none: the whole sketch). Returns false when it
/// closes.
pub(crate) struct AutoDimension {
    dimensions: bool,
    constraints: bool,
    error: Option<String>,
}

impl AutoDimension {
    pub(crate) fn new() -> Self {
        Self { dimensions: true, constraints: true, error: None }
    }

    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) -> bool {
        let mut open = true;
        let mut done = false;
        sketch_window(tr!(lang, AutoDimTitle), ctx).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label(tr!(lang, AutoDimCurves, session.selected_curves().len()));
            ui.checkbox(&mut self.dimensions, tr!(lang, AutoDimDimensions));
            ui.checkbox(&mut self.constraints, tr!(lang, AutoDimConstraints));
            ui.label(tr!(lang, AutoDimRequired, session.dimensions_required()));
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, tr!(lang, DialogRefused, error));
            }
            ui.horizontal(|ui| {
                if ui.button(tr!(lang, DialogApply)).clicked() {
                    self.error = session
                        .auto_dimension(self.dimensions, self.constraints)
                        .err()
                        .map(|e| e.to_string());
                }
                if ui.button(tr!(lang, AutoDimRemove)).clicked() {
                    self.error = session
                        .remove_auto_dimensions(self.dimensions, self.constraints)
                        .err()
                        .map(|e| e.to_string());
                }
                done = ui.button(tr!(lang, DialogDone)).clicked();
            });
        });
        open && !done && session.active_sketch().is_some()
    }
}
