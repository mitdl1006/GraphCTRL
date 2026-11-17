//! The component pattern dialog (Inventor's Assemble tab, Pattern panel, Pattern): pick the components to
//! pattern, then either columns and rows (Rectangular) or an axis, count and angle (Circular); the copies
//! follow the parents.
//! Reference: Inventor 2026 help, Create rectangular patterns of components
//! (GUID-CF627358-120F-4142-A9D0-13217BD3432D) and Create circular patterns of components
//! (GUID-FF7B08E3-624B-4382-B4E5-304C0354C048).

use gc_core::AnyId;
use gc_doc::{Geometry, Occurrence, OriginElement, PatternDirection, PatternLayout, Selection};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// The direction or axis the next pick in the view fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Column,
    Row,
    Axis,
}

/// One direction being typed.
#[derive(Clone, Debug)]
struct Direction {
    along: Option<Selection>,
    flip: bool,
    count: String,
    /// Millimetres.
    spacing: String,
}

impl Direction {
    fn new(count: &str, spacing: &str) -> Self {
        Self { along: None, flip: false, count: count.to_owned(), spacing: spacing.to_owned() }
    }

    fn value(&self) -> Option<PatternDirection> {
        Some(PatternDirection {
            along: self.along?,
            flip: self.flip,
            count: self.count.trim().parse().ok()?,
            spacing: self.spacing.trim().parse::<f64>().ok()? / 1000.0,
        })
    }
}

/// The dialog's state.
pub(crate) struct Pattern {
    pub(crate) open: bool,
    circular: bool,
    parents: Vec<AnyId>,
    column: Direction,
    row: Direction,
    /// Whether the row direction takes part.
    rows: bool,
    slot: Slot,
    axis: Option<Selection>,
    axis_flip: bool,
    count: String,
    /// Degrees.
    angle: String,
    fitted: bool,
    rotational: bool,
    midplane: bool,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            open: false,
            circular: false,
            parents: Vec::new(),
            column: Direction::new("3", "50"),
            row: Direction::new("2", "50"),
            rows: false,
            slot: Slot::Column,
            axis: None,
            axis_flip: false,
            count: "4".to_owned(),
            angle: "90".to_owned(),
            fitted: false,
            rotational: true,
            midplane: false,
        }
    }
}

impl Pattern {
    /// Opens the dialog; the selected components are the parents.
    pub(crate) fn start(&mut self, parents: Vec<AnyId>) {
        *self = Self { open: true, parents, ..Self::default() };
    }

    /// A pick in the view: an origin axis fills the active direction.
    pub(crate) fn pick(&mut self, occurrence: Occurrence, element: OriginElement) {
        if !matches!(element, OriginElement::Axis(_)) {
            return;
        }
        let selection = Some(Selection { occurrence, geometry: Geometry::origin(element) });
        match self.slot {
            Slot::Column => {
                self.column.along = selection;
                self.slot = if self.circular { Slot::Axis } else { Slot::Row };
            }
            Slot::Row => self.row.along = selection,
            Slot::Axis => self.axis = selection,
        }
    }

    /// The layout the fields describe.
    fn layout(&self) -> Option<PatternLayout> {
        if self.circular {
            Some(PatternLayout::Circular {
                axis: self.axis?,
                flip: self.axis_flip,
                count: self.count.trim().parse().ok()?,
                angle: self.angle.trim().parse::<f64>().ok()?.to_radians(),
                fitted: self.fitted,
                rotational: self.rotational,
                midplane: self.midplane,
            })
        } else {
            Some(PatternLayout::Rectangular {
                column: self.column.value()?,
                row: if self.rows { Some(self.row.value()?) } else { None },
            })
        }
    }

    /// The dialog window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut ok = false;
        let screen = ctx.content_rect();
        let start = egui::pos2(screen.right() - 330.0, screen.top() + 200.0);
        let components: Vec<(AnyId, String)> = session
            .components()
            .iter()
            .filter_map(|node| match node.occurrence {
                Occurrence::Item(id) => Some((id, node.name.clone())),
                _ => None,
            })
            .collect();
        egui::Window::new(tr!(lang, PatternTitle))
            .open(&mut open)
            .default_pos(start)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (circular, name) in
                        [(false, tr!(lang, PatternRectangular)), (true, tr!(lang, PatternCircular))]
                    {
                        if ui.selectable_label(self.circular == circular, name).clicked() {
                            self.circular = circular;
                            self.slot = if circular { Slot::Axis } else { Slot::Column };
                        }
                    }
                });
                ui.label(egui::RichText::new(tr!(lang, PatternComponents)).strong());
                for (id, name) in &components {
                    let mut chosen = self.parents.contains(id);
                    if ui.checkbox(&mut chosen, name).changed() {
                        if chosen {
                            self.parents.push(*id);
                        } else {
                            self.parents.retain(|parent| parent != id);
                        }
                    }
                }
                ui.separator();
                if self.circular {
                    self.axis_row(ui, session, lang);
                    ui.checkbox(&mut self.axis_flip, tr!(lang, PatternFlip));
                    ui.checkbox(&mut self.midplane, tr!(lang, PatternMidplane));
                    labelled(ui, tr!(lang, PatternCount), &mut self.count);
                    labelled(ui, tr!(lang, PatternAngle), &mut self.angle);
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.rotational, true, tr!(lang, PatternRotational));
                        ui.selectable_value(&mut self.rotational, false, tr!(lang, PatternFixed));
                    });
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.fitted, false, tr!(lang, PatternIncremental));
                        ui.selectable_value(&mut self.fitted, true, tr!(lang, PatternFitted));
                    });
                } else {
                    for (slot, title) in
                        [(Slot::Column, tr!(lang, PatternColumn)), (Slot::Row, tr!(lang, PatternRow))]
                    {
                        let along = if slot == Slot::Column { self.column.along } else { self.row.along };
                        if slot == Slot::Row {
                            ui.checkbox(&mut self.rows, title);
                            if !self.rows {
                                continue;
                            }
                        } else {
                            ui.label(egui::RichText::new(title).strong());
                        }
                        let text = match along {
                            Some(selection) => axis_name(session, &selection),
                            None => tr!(lang, ConstrainPick).to_owned(),
                        };
                        let name = format!("{title}: {text}");
                        let button = ui.selectable_label(self.slot == slot, name.clone());
                        button.widget_info(|| {
                            egui::WidgetInfo::selected(egui::Role::Button, true, self.slot == slot, &name)
                        });
                        if button.clicked() {
                            self.slot = slot;
                        }
                        let direction = if slot == Slot::Column { &mut self.column } else { &mut self.row };
                        ui.checkbox(&mut direction.flip, tr!(lang, PatternFlip));
                        labelled(ui, tr!(lang, PatternCount), &mut direction.count);
                        labelled(ui, tr!(lang, PatternSpacing), &mut direction.spacing);
                    }
                }
                let ready = !self.parents.is_empty() && self.layout().is_some();
                ui.horizontal(|ui| {
                    ok = ui.add_enabled(ready, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
                });
            });
        if ok && let Some(layout) = self.layout() {
            let name = format!("Pattern:{}", session.patterns().len() + 1);
            // A refused pattern leaves the dialog open; the status bar says why.
            if session.add_pattern(&name, &self.parents, layout).is_ok() {
                open = false;
            }
        }
        self.open = open;
    }

    fn axis_row(&mut self, ui: &mut egui::Ui, session: &Session, lang: Lang) {
        let text = match self.axis {
            Some(selection) => axis_name(session, &selection),
            None => tr!(lang, ConstrainPick).to_owned(),
        };
        let title = tr!(lang, PatternAxis);
        let name = format!("{title}: {text}");
        let button = ui.selectable_label(self.slot == Slot::Axis, name.clone());
        button.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, true, self.slot == Slot::Axis, &name)
        });
        if button.clicked() {
            self.slot = Slot::Axis;
        }
    }
}

/// A labelled text field.
fn labelled(ui: &mut egui::Ui, label: &str, text: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(text).desired_width(70.0))
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
    });
}

/// "Body1:1 · X axis".
fn axis_name(session: &Session, selection: &Selection) -> String {
    let owner = session
        .components()
        .iter()
        .find(|node| node.occurrence == selection.occurrence)
        .map_or_else(|| "?".to_owned(), |node| format!("{}:1", node.name));
    let axis = match selection.geometry {
        Geometry::Axis { direction, .. } => {
            let largest =
                (0..3).max_by(|a, b| direction[*a].abs().total_cmp(&direction[*b].abs())).unwrap_or(2);
            ["X", "Y", "Z"][largest]
        }
        _ => "?",
    };
    format!("{owner} · {axis}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rectangular pattern of three along the picked X axis, and a circular one of four round Z.
    #[test]
    fn the_fields_make_a_layout_that_the_session_accepts() {
        let mut session = Session::new();
        let solid = gc_doc::Solid::Box { size: [0.05, 0.05, 0.05] };
        let floor = session.add_body(solid.clone(), gc_doc::Placement::default()).unwrap();
        let part =
            session.add_body(solid, gc_doc::Placement { xyz: [0.0, 0.0, 0.3], rpy: [0.0; 3] }).unwrap();
        let bodies = session.components().len();

        let mut dialog = Pattern::default();
        dialog.start(vec![part.erase()]);
        assert!(dialog.layout().is_none(), "no direction picked yet");
        dialog.pick(Occurrence::Item(floor.erase()), OriginElement::Plane(gc_doc::OriginPlane::Xy));
        assert!(dialog.layout().is_none(), "a plane is not a direction");
        dialog.pick(Occurrence::Item(floor.erase()), OriginElement::Axis(0));
        let layout = dialog.layout().expect("the column is set");
        session.add_pattern("Pattern:1", &[part.erase()], layout).unwrap();
        assert_eq!(session.components().len(), bodies + 2, "two copies");
        assert_eq!(session.patterns().len(), 1);

        dialog.circular = true;
        dialog.slot = Slot::Axis;
        dialog.pick(Occurrence::Item(floor.erase()), OriginElement::Axis(2));
        let layout = dialog.layout().expect("the axis is set");
        session.add_pattern("Pattern:2", &[part.erase()], layout).unwrap();
        assert_eq!(session.patterns().len(), 2);
    }
}
