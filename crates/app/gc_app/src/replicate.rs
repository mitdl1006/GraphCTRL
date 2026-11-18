//! The Mirror, Copy and Replace component dialog (Inventor's Assemble tab: Pattern panel Mirror and Copy,
//! Component panel Replace): mirror components across an origin plane, copy them moved by an offset, or
//! replace a component (or every component of the same source) with a new shape. Relationships that could
//! not be kept are suppressed by the backend; the dialog lists them.
//! Reference: Inventor 2026 help, To Mirror Assembly Components (GUID-C5B3D3DF-27B3-4B4C-9058-A949138D8B8A),
//! Copy assembly components (GUID-5AAA6CB1-7BB2-4AF0-9A3D-AB729E5160C5), Replace assembly components
//! (GUID-E5FAA224-7332-4AC6-A78E-793FDACFEEDD).

use gc_core::AnyId;
use gc_doc::{Geometry, Occurrence, OriginElement, Placement, Replacement, Selection, Solid};
use gc_i18n::{Lang, tr};
use gc_session::Session;

/// What the dialog does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Mirror,
    Copy,
    Replace,
}

/// The shape a component is replaced with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Box,
    Cylinder,
    Sphere,
}

/// The dialog's state.
pub(crate) struct Replicate {
    pub(crate) open: bool,
    mode: Mode,
    items: Vec<AnyId>,
    plane: Option<Selection>,
    relationships: bool,
    ground: bool,
    /// Mirror: "Create mirror pattern" (associative, a Mirror node in the browser) instead of "Create flat
    /// structure" (Inventor 2026 help, Mirror Components: Reference, GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336).
    pattern: bool,
    /// The copy's offset in millimetres.
    offset: [String; 3],
    shape: Shape,
    /// Millimetres: edge lengths of a box, or radius and length of a cylinder, or the radius of a sphere.
    size: [String; 3],
    all: bool,
    /// Relationships the last run suppressed.
    report: Option<Vec<String>>,
}

impl Default for Replicate {
    fn default() -> Self {
        Self {
            open: false,
            mode: Mode::Mirror,
            items: Vec::new(),
            plane: None,
            relationships: true,
            ground: false,
            pattern: false,
            offset: ["100".to_owned(), "0".to_owned(), "0".to_owned()],
            shape: Shape::Box,
            size: ["100".to_owned(), "100".to_owned(), "100".to_owned()],
            all: false,
            report: None,
        }
    }
}

impl Replicate {
    /// Opens the dialog in a mode.
    pub(crate) fn start(&mut self, mode: Mode) {
        *self = Self { open: true, mode, ..Self::default() };
    }

    /// A pick in the view: an origin plane is the mirror plane.
    pub(crate) fn pick(&mut self, occurrence: Occurrence, element: OriginElement) {
        if self.mode == Mode::Mirror && matches!(element, OriginElement::Plane(_)) {
            self.plane = Some(Selection { occurrence, geometry: Geometry::origin(element) });
        }
    }

    /// Whether the view is picking for the dialog.
    pub(crate) fn picking(&self) -> bool {
        self.open && self.mode == Mode::Mirror
    }

    fn numbers<const N: usize>(fields: &[String; N]) -> Option<[f64; N]> {
        let mut out = [0.0; N];
        for (value, field) in out.iter_mut().zip(fields) {
            *value = field.trim().parse::<f64>().ok()? / 1000.0;
        }
        Some(out)
    }

    fn replacement(&self) -> Option<Replacement> {
        let size = Self::numbers(&self.size)?;
        let positive = |v: f64| v.is_finite() && v > 0.0;
        match self.shape {
            Shape::Box if size.iter().all(|v| positive(*v)) => Some(Replacement::Solid(Solid::Box { size })),
            Shape::Cylinder if positive(size[0]) && positive(size[1]) => {
                Some(Replacement::Solid(Solid::Cylinder { radius: size[0], length: size[1] }))
            }
            Shape::Sphere if positive(size[0]) => Some(Replacement::Solid(Solid::Sphere { radius: size[0] })),
            _ => None,
        }
    }

    /// Whether the fields describe a command that can run.
    fn ready(&self) -> bool {
        if self.items.is_empty() || (self.mode == Mode::Replace && self.items.len() != 1) {
            return false;
        }
        match self.mode {
            Mode::Mirror => self.plane.is_some(),
            Mode::Copy => Self::numbers(&self.offset).is_some(),
            Mode::Replace => self.replacement().is_some(),
        }
    }

    fn run(&mut self, session: &mut Session) -> bool {
        let result = match self.mode {
            Mode::Mirror => match self.plane {
                Some(plane) if self.pattern => {
                    let name = format!("Mirror:{}", session.patterns().len() + 1);
                    session
                        .add_pattern(&name, &self.items, gc_doc::PatternLayout::Mirror { plane })
                        .map(|()| Vec::new())
                }
                Some(plane) => session.mirror_components(&self.items, plane, self.relationships, self.ground),
                None => return false,
            },
            Mode::Copy => match Self::numbers(&self.offset) {
                Some(xyz) => session.copy_components(
                    &self.items,
                    Placement { xyz, rpy: [0.0; 3] },
                    self.relationships,
                    self.ground,
                ),
                None => return false,
            },
            Mode::Replace => match (self.replacement(), self.items.first()) {
                (Some(with), Some(item)) => session.replace_component(*item, with, self.all),
                _ => return false,
            },
        };
        match result {
            Ok(suppressed) => {
                self.report = Some(suppressed);
                true
            }
            // A refusal is explained in the status bar.
            Err(_) => false,
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
        let title = match self.mode {
            Mode::Mirror => tr!(lang, ReplicateMirror),
            Mode::Copy => tr!(lang, ReplicateCopy),
            Mode::Replace => tr!(lang, ReplicateReplace),
        };
        egui::Window::new(title).open(&mut open).default_pos(start).default_width(300.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (mode, name) in [
                    (Mode::Mirror, tr!(lang, ReplicateMirror)),
                    (Mode::Copy, tr!(lang, ReplicateCopy)),
                    (Mode::Replace, tr!(lang, ReplicateReplace)),
                ] {
                    if ui.selectable_label(self.mode == mode, name).clicked() {
                        self.mode = mode;
                        self.report = None;
                    }
                }
            });
            ui.label(egui::RichText::new(tr!(lang, ReplicateComponents)).strong());
            for (id, name) in &components {
                let mut chosen = self.items.contains(id);
                if ui.checkbox(&mut chosen, name).changed() {
                    if self.mode == Mode::Replace {
                        self.items.clear();
                    }
                    if chosen {
                        self.items.push(*id);
                    } else {
                        self.items.retain(|item| item != id);
                    }
                }
            }
            ui.separator();
            match self.mode {
                Mode::Mirror => {
                    let text = match self.plane {
                        Some(selection) => plane_name(session, &selection),
                        None => tr!(lang, ConstrainPick).to_owned(),
                    };
                    let name = format!("{}: {text}", tr!(lang, ReplicatePlane));
                    let button = ui.selectable_label(true, name.clone());
                    button.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, true, &name));
                }
                Mode::Copy => {
                    for (field, axis) in self.offset.iter_mut().zip(["X", "Y", "Z"]) {
                        labelled(ui, &format!("{} {axis} (mm)", tr!(lang, ReplicateOffset)), field);
                    }
                }
                Mode::Replace => {
                    ui.horizontal(|ui| {
                        for (shape, name) in [
                            (Shape::Box, tr!(lang, ReplicateBox)),
                            (Shape::Cylinder, tr!(lang, ReplicateCylinder)),
                            (Shape::Sphere, tr!(lang, ReplicateSphere)),
                        ] {
                            ui.selectable_value(&mut self.shape, shape, name);
                        }
                    });
                    let labels: &[&str] = match self.shape {
                        Shape::Box => &["X", "Y", "Z"],
                        Shape::Cylinder => &["R", "L"],
                        Shape::Sphere => &["R"],
                    };
                    for (field, label) in self.size.iter_mut().zip(labels) {
                        labelled(ui, &format!("{label} (mm)"), field);
                    }
                    ui.checkbox(&mut self.all, tr!(lang, ReplicateAll));
                }
            }
            if self.mode == Mode::Mirror {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.pattern, false, tr!(lang, MirrorFlat));
                    ui.selectable_value(&mut self.pattern, true, tr!(lang, MirrorPattern));
                });
            }
            if self.mode != Mode::Replace && !(self.mode == Mode::Mirror && self.pattern) {
                ui.checkbox(&mut self.relationships, tr!(lang, ReplicateRelationships));
                ui.checkbox(&mut self.ground, tr!(lang, ReplicateGround));
            }
            if let Some(report) = &self.report {
                if report.is_empty() {
                    ui.label(tr!(lang, ReplicateDone));
                } else {
                    ui.label(tr!(lang, ReplicateSuppressed, report.join(", ")));
                }
            }
            ui.horizontal(|ui| {
                ok = ui.add_enabled(self.ready(), egui::Button::new(tr!(lang, ConstrainOk))).clicked();
            });
        });
        if ok {
            self.report = None;
            if self.run(session) {
                // A clean run closes; one that suppressed relationships stays to list them.
                if self.report.as_ref().is_some_and(Vec::is_empty) {
                    open = false;
                } else {
                    self.items.clear();
                }
            }
        }
        self.open = open;
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

/// "Body1:1 · XY plane".
fn plane_name(session: &Session, selection: &Selection) -> String {
    let owner = session
        .components()
        .iter()
        .find(|node| node.occurrence == selection.occurrence)
        .map_or_else(|| "?".to_owned(), |node| format!("{}:1", node.name));
    let plane = match selection.geometry {
        Geometry::Plane { normal, .. } => {
            let largest = (0..3).max_by(|a, b| normal[*a].abs().total_cmp(&normal[*b].abs())).unwrap_or(2);
            ["YZ", "XZ", "XY"][largest]
        }
        _ => "?",
    };
    format!("{owner} · {plane}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gc_doc::OriginPlane;

    fn session() -> (Session, gc_core::AnyId, gc_core::AnyId) {
        let mut session = Session::new();
        let solid = Solid::Box { size: [0.05; 3] };
        let floor = session.add_body(solid.clone(), Placement::default()).unwrap();
        let part = session.add_body(solid, Placement { xyz: [0.2, 0.0, 0.0], rpy: [0.0; 3] }).unwrap();
        (session, floor.erase(), part.erase())
    }

    /// Copy adds a component at the offset, replace swaps the shape and mirror needs a picked plane.
    #[test]
    fn the_fields_run_copy_replace_and_mirror() {
        let (mut session, floor, part) = session();
        let count = session.components().len();

        let mut dialog = Replicate::default();
        dialog.start(Mode::Copy);
        dialog.items = vec![part];
        assert!(dialog.ready());
        assert!(dialog.run(&mut session));
        assert_eq!(session.components().len(), count + 1);

        dialog.start(Mode::Replace);
        dialog.items = vec![part];
        dialog.shape = Shape::Sphere;
        dialog.size = ["30".to_owned(), "0".to_owned(), "0".to_owned()];
        assert!(dialog.ready());
        assert!(dialog.run(&mut session));

        dialog.start(Mode::Mirror);
        dialog.items = vec![part];
        assert!(!dialog.ready(), "no plane yet");
        dialog.pick(Occurrence::Item(floor), OriginElement::Axis(0));
        assert!(!dialog.ready(), "an axis is not a plane");
        dialog.pick(Occurrence::Item(floor), OriginElement::Plane(OriginPlane::Yz));
        assert!(dialog.ready());
        assert!(dialog.run(&mut session));
        assert_eq!(session.components().len(), count + 2);
    }
    /// "Create mirror pattern" makes a Mirror pattern node instead of a flat copy (GUID-7B19CA69-8D14-40DA-9543-115A8AFE6336).
    #[test]
    fn a_mirror_pattern_is_made_instead_of_a_flat_copy() {
        let (mut session, floor, part) = session();
        let mut dialog = Replicate::default();
        dialog.start(Mode::Mirror);
        dialog.items = vec![part];
        dialog.pattern = true;
        dialog.pick(Occurrence::Item(floor), OriginElement::Plane(gc_doc::OriginPlane::Yz));
        assert!(dialog.ready());
        assert!(dialog.run(&mut session));
        assert_eq!(session.patterns().len(), 1, "a Mirror node in the browser");
        assert!(matches!(session.patterns()[0].layout, gc_doc::PatternLayout::Mirror { .. }));
    }
}
