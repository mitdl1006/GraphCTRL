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
//! Inventor picks the geometry and the line or axis inside the dialog; here each field takes the current
//! sketch selection with its "← selection" button.

use gc_core::Id;
use gc_doc::{SketchOp, SketchPatternDirection};
use gc_i18n::{Lang, tr};
use gc_session::Session;
use gc_sketch::{Curve, SkPoint};
use gc_tools::ToolKind;

/// Which dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Mirror,
    Rectangular,
    Circular,
}

/// A sketch Mirror or Pattern dialog.
pub(crate) struct SketchDialog {
    kind: Kind,
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
}

impl SketchDialog {
    /// A dialog that starts with the current selection as its geometry.
    pub(crate) fn new(kind: Kind, session: &Session) -> Self {
        Self {
            kind,
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

    /// Shows the dialog. Returns false when it closes.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) -> bool {
        let title = match self.kind {
            Kind::Mirror => tr!(lang, SkMirrorTitle),
            Kind::Rectangular => tr!(lang, SkRectTitle),
            Kind::Circular => tr!(lang, SkCircTitle),
        };
        let mut open = true;
        let mut done = false;
        let state = |set: bool| if set { tr!(lang, DialogPicked) } else { tr!(lang, DialogNotPicked) };
        egui::Window::new(title).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr!(lang, DialogGeometry, self.geometry.len()));
                if ui.button(tr!(lang, TakeGeometry)).clicked() {
                    self.geometry = session.selected_curves();
                }
            });
            let first_line = |session: &Session| {
                let lines: Vec<Id<Curve>> = session
                    .selected_curves()
                    .into_iter()
                    .filter(|c| session.active_sketch().is_some_and(|(_, s)| s.line(*c).is_some()))
                    .collect();
                lines.first().copied()
            };
            match self.kind {
                Kind::Mirror => {
                    ui.horizontal(|ui| {
                        ui.label(tr!(lang, MirrorLine, state(self.line.is_some())));
                        if ui.button(tr!(lang, TakeMirrorLine)).clicked() {
                            self.line = first_line(session);
                        }
                    });
                }
                Kind::Rectangular => {
                    ui.horizontal(|ui| {
                        ui.label(tr!(lang, PatternDirection1, state(self.line.is_some())));
                        if ui.button(tr!(lang, TakeDirection1)).clicked() {
                            self.line = first_line(session);
                        }
                    });
                    ui.horizontal(|ui| {
                        let label = ui.label(tr!(lang, PatternCount)).id;
                        ui.add(egui::DragValue::new(&mut self.count).range(2..=500)).labelled_by(label);
                        let label = ui.label(tr!(lang, SkPatternSpacing)).id;
                        ui.text_edit_singleline(&mut self.spacing).labelled_by(label);
                        ui.checkbox(&mut self.flip, tr!(lang, PatternFlip));
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr!(lang, PatternDirection2, state(self.line2.is_some())));
                        if ui.button(tr!(lang, TakeDirection2)).clicked() {
                            self.line2 = first_line(session);
                            self.count2 = self.count2.max(2);
                        }
                    });
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
                    ui.horizontal(|ui| {
                        ui.label(tr!(lang, SkPatternAxis, state(self.axis.is_some())));
                        if ui.button(tr!(lang, TakeAxis)).clicked() {
                            self.axis = session.selected_points().first().copied();
                        }
                    });
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
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, tr!(lang, DialogRefused, error));
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
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
                done = finish;
            });
        });
        open && !done
    }
}

/// The Polygon tool's options while it runs; Done or closing leaves the tool.
pub(crate) fn polygon_window(ctx: &egui::Context, session: &mut Session, lang: Lang) {
    let (mut sides, mut circumscribed) = session.polygon_settings();
    let mut open = true;
    let mut done = false;
    egui::Window::new(tr!(lang, SkPolygonTitle)).open(&mut open).resizable(false).show(ctx, |ui| {
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
    egui::Window::new(tr!(lang, SkOffsetTitle)).resizable(false).show(ctx, |ui| {
        ui.checkbox(&mut loop_select, tr!(lang, OffsetLoopSelect));
        ui.checkbox(&mut constrain, tr!(lang, OffsetConstrain));
    });
    if (loop_select, constrain) != session.offset_options() {
        session.set_offset_options(loop_select, constrain);
    }
}
