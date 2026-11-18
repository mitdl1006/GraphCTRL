//! The Measure panel, as Inventor's Measure command in an assembly with component priority
//! (https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-1805AB8F-F782-4B24-A332-B4F2E10A3D00.htm):
//! the first and second selection keep their own colours, two components show their minimum distance
//! (or the distance between their centres), clicking a blank area restarts, Copy All copies the values
//! and Done closes the panel.

use gc_i18n::{Lang, tr};
use gc_session::{CellItem, Feature, FeaturePair, PairMeasure, Session};
use gc_viewport::Viewport;

/// Colour of the first selection.
const FIRST: egui::Color32 = egui::Color32::from_rgb(0x3f, 0xa9, 0xf5);
/// Colour of the second selection.
const SECOND: egui::Color32 = egui::Color32::from_rgb(0x4c, 0xd9, 0x64);

/// The distance shown for the pair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DistanceKind {
    /// The smallest gap between the shapes.
    #[default]
    Minimum,
    /// The distance between the origins.
    CentreToCentre,
    /// The largest distance between the shapes.
    Maximum,
}

/// What a click selects (Inventor's selection priority in an assembly).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Priority {
    /// Whole components: robots, bodies, waypoints, frames.
    #[default]
    Component,
    /// Corners, straight edges and flat faces of the solids.
    Features,
}

/// The groups of accumulated values (Inventor's Accumulated Properties).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Group {
    /// Lengths and distances (metres).
    Distance,
    /// Areas (square metres).
    Area,
    /// Angles (radians).
    Angle,
}

impl Group {
    fn show(self, value: f64) -> String {
        match self {
            Self::Distance => format!("{} mm", mm(value)),
            Self::Area => format!("{:.2} mm²", value * 1e6),
            Self::Angle => format!("{:.3}°", value.to_degrees()),
        }
    }
}

/// One line of the panel: its colour (`PLACEHOLDER` for results), text, and the value it can add to the
/// accumulated values.
struct Line {
    colour: egui::Color32,
    text: String,
    value: Option<(Group, f64)>,
}

fn line(colour: egui::Color32, text: String) -> Line {
    Line { colour, text, value: None }
}

fn valued(colour: egui::Color32, text: String, group: Group, value: f64) -> Line {
    Line { colour, text, value: Some((group, value)) }
}

/// The Measure panel's state.
#[derive(Default)]
pub(crate) struct Measure {
    /// Whether the panel is open.
    pub open: bool,
    first: Option<CellItem>,
    second: Option<CellItem>,
    /// The selection seen last frame, to notice new picks.
    seen: Option<CellItem>,
    kind: DistanceKind,
    /// The pair's values, computed when the selection changes (a robot's shapes take a moment).
    pair: Option<PairMeasure>,
    priority: Priority,
    /// Features picked with the Edges and Faces priority.
    features: Vec<Feature>,
    /// Accumulated values, signed (a subtraction is negative), in the order they were added.
    accumulated: Vec<(Group, f64)>,
}

fn mm(v: f64) -> String {
    format!("{:.3}", v * 1000.0)
}

impl Measure {
    /// Opens the panel with nothing selected yet.
    pub(crate) fn start(&mut self, session: &Session) {
        *self = Self { open: true, seen: session.selected_item(), ..Self::default() };
    }

    fn restart(&mut self) {
        self.first = None;
        self.second = None;
        self.pair = None;
        self.features.clear();
    }

    /// Keeps the viewport's picking in step with the priority and takes its feature picks: a third pick
    /// starts again, a click on a blank area restarts.
    /// With two points picked, a pick with Shift held adds a third point: the angle at the second.
    pub(crate) fn follow_view(&mut self, viewport: &mut Viewport, shift: bool) {
        viewport.measure_features = self.open && self.priority == Priority::Features;
        if let Some(pick) = viewport.take_measure_pick() {
            match pick {
                None => self.restart(),
                Some((_, feature)) => {
                    let two_points =
                        matches!(self.features.as_slice(), [Feature::Point(_), Feature::Point(_)]);
                    let third = shift && two_points && matches!(feature, Feature::Point(_));
                    if self.features.len() >= 2 && !third {
                        self.features.clear();
                    }
                    self.features.push(feature);
                }
            }
        }
    }

    /// Follows the selection: a new pick fills selection 1, then 2, then starts again; selecting nothing
    /// (a click on a blank area) restarts.
    pub(crate) fn follow(&mut self, session: &Session) {
        if !self.open {
            return;
        }
        let selected = session.selected_item();
        if self.priority == Priority::Features {
            self.seen = selected;
            return;
        }
        if selected != self.seen {
            self.seen = selected;
            match selected {
                None => self.restart(),
                Some(item) => match (self.first, self.second) {
                    (None, _) => self.first = Some(item),
                    (Some(first), None) if first != item => {
                        self.second = Some(item);
                        self.pair = session.measure_pair(first, item);
                    }
                    _ => {
                        self.restart();
                        self.first = Some(item);
                    }
                },
            }
        }
        // Items deleted meanwhile are dropped.
        if self.first.is_some_and(|item| session.measure_item(item).is_none()) {
            self.restart();
        }
    }

    /// The values of the picked features, one per line.
    fn feature_lines(&self, lang: Lang) -> Vec<Line> {
        let mut lines = Vec::new();
        for (colour, title, feature) in [
            (FIRST, tr!(lang, MeasureFirst), self.features.first()),
            (SECOND, tr!(lang, MeasureSecond), self.features.get(1)),
        ] {
            let Some(feature) = feature else { continue };
            lines.push(match feature {
                Feature::Point(p) => {
                    let [x, y, z] = p.map(mm);
                    line(colour, format!("{title}: {}", tr!(lang, MeasurePoint, x, y, z)))
                }
                Feature::Edge(a, b) => {
                    let length = gc_math::DVec3::from_array(*a).distance(gc_math::DVec3::from_array(*b));
                    valued(
                        colour,
                        format!("{title}: {}", tr!(lang, MeasureEdge, mm(length))),
                        Group::Distance,
                        length,
                    )
                }
                Feature::Cylinder { radius, length, .. } => valued(
                    colour,
                    format!(
                        "{title}: {}",
                        tr!(lang, MeasureCylinder, mm(radius * 2.0), mm(*radius), mm(*length))
                    ),
                    Group::Distance,
                    radius * 2.0,
                ),
                // A cone is never picked for Measure (the viewport leaves it out).
                Feature::Cone { apex, .. } => {
                    let [x, y, z] = apex.map(mm);
                    line(colour, format!("{title}: {}", tr!(lang, MeasurePoint, x, y, z)))
                }
                Feature::Face { area, .. } => valued(
                    colour,
                    format!("{title}: {}", tr!(lang, MeasureFace, format!("{:.2}", area * 1e6))),
                    Group::Area,
                    *area,
                ),
                // A circular edge or a sphere is listed by its centre (the frontend may refine this).
                Feature::Circle { centre, .. } | Feature::Sphere { centre, .. } => {
                    let [x, y, z] = centre.map(mm);
                    line(colour, format!("{title}: {}", tr!(lang, MeasurePoint, x, y, z)))
                }
            });
        }
        let text = egui::Color32::PLACEHOLDER;
        if let [Feature::Point(a), Feature::Point(vertex), Feature::Point(c)] = self.features.as_slice() {
            // Three points: the angle at the second one (none when two points coincide).
            if let Some(angle) = gc_session::angle_at(*a, *vertex, *c) {
                lines.push(valued(
                    text,
                    tr!(lang, MeasureFeatureAngle, format!("{:.3}", angle.to_degrees())),
                    Group::Angle,
                    angle,
                ));
            }
        } else if let [a, b] = self.features.as_slice() {
            let pair: FeaturePair = gc_session::measure_features(a, b);
            if let Some(distance) = pair.distance {
                lines.push(valued(
                    text,
                    format!("{}: {} mm", tr!(lang, MeasureDistance), mm(distance)),
                    Group::Distance,
                    distance,
                ));
            }
            if let Some(centre) = pair.centre_distance {
                lines.push(valued(
                    text,
                    format!("{}: {} mm", tr!(lang, MeasureCentres), mm(centre)),
                    Group::Distance,
                    centre,
                ));
            }
            if let Some(delta) = pair.delta {
                let [dx, dy, dz] = delta.map(mm);
                lines.push(line(text, tr!(lang, MeasureDelta, dx, dy, dz)));
            }
            if let Some(angle) = pair.angle {
                lines.push(valued(
                    text,
                    tr!(lang, MeasureFeatureAngle, format!("{:.3}", angle.to_degrees())),
                    Group::Angle,
                    angle,
                ));
            }
        }
        lines
    }

    /// The values as text, one per line (what Copy All copies).
    fn lines(&self, session: &Session, lang: Lang) -> Vec<Line> {
        if self.priority == Priority::Features {
            return self.feature_lines(lang);
        }
        let mut lines = Vec::new();
        for (colour, title, item) in
            [(FIRST, tr!(lang, MeasureFirst), self.first), (SECOND, tr!(lang, MeasureSecond), self.second)]
        {
            let Some(measured) = item.and_then(|item| session.measure_item(item)) else { continue };
            let [x, y, z] = measured.position.map(mm);
            lines.push(line(colour, format!("{title}: {}", measured.name)));
            lines.push(line(colour, tr!(lang, MeasurePosition, x, y, z)));
        }
        if let Some(pair) = &self.pair {
            let text = egui::Color32::PLACEHOLDER;
            match (self.kind, pair.minimum) {
                (DistanceKind::Minimum, Some(minimum)) => lines.push(valued(
                    text,
                    format!("{}: {} mm", tr!(lang, MeasureMinimum), mm(minimum.distance)),
                    Group::Distance,
                    minimum.distance,
                )),
                (DistanceKind::Minimum, None) => {
                    lines.push(line(text, tr!(lang, MeasureNoMinimum).to_owned()))
                }
                (DistanceKind::CentreToCentre, _) => lines.push(valued(
                    text,
                    format!("{}: {} mm", tr!(lang, MeasureCentres), mm(pair.centre_to_centre)),
                    Group::Distance,
                    pair.centre_to_centre,
                )),
                (DistanceKind::Maximum, _) => match pair.maximum {
                    Some(maximum) => lines.push(valued(
                        text,
                        format!("{}: {} mm", tr!(lang, MeasureMaximum), mm(maximum.distance)),
                        Group::Distance,
                        maximum.distance,
                    )),
                    None => lines.push(line(text, tr!(lang, MeasureNoMinimum).to_owned())),
                },
            }
            let [dx, dy, dz] = pair.delta.map(mm);
            lines.push(line(text, tr!(lang, MeasureDelta, dx, dy, dz)));
            if let Some(angle) = pair.angle {
                lines.push(valued(
                    text,
                    tr!(lang, MeasureAngle, format!("{:.3}", angle.to_degrees())),
                    Group::Angle,
                    angle,
                ));
            }
        }
        lines
    }

    /// The panel.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &Session, lang: Lang) {
        let mut open = self.open;
        let mut done = false;
        egui::Window::new(tr!(lang, MenuMeasure)).open(&mut open).default_width(340.0).resizable(false).show(
            ctx,
            |ui| {
                ui.horizontal(|ui| {
                    let before = self.priority;
                    ui.selectable_value(
                        &mut self.priority,
                        Priority::Component,
                        tr!(lang, MeasureByComponent),
                    );
                    ui.selectable_value(&mut self.priority, Priority::Features, tr!(lang, MeasureByFeature));
                    // Changing the priority resets the command, as in Inventor.
                    if self.priority != before {
                        self.restart();
                    }
                });
                if self.priority == Priority::Component {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.kind, DistanceKind::Minimum, tr!(lang, MeasureMinimum));
                        ui.selectable_value(
                            &mut self.kind,
                            DistanceKind::CentreToCentre,
                            tr!(lang, MeasureCentres),
                        );
                        ui.selectable_value(&mut self.kind, DistanceKind::Maximum, tr!(lang, MeasureMaximum));
                    });
                }
                ui.separator();
                let lines = self.lines(session, lang);
                if self.second.is_none() && self.features.len() < 2 {
                    ui.label(egui::RichText::new(tr!(lang, MeasureHint)).weak());
                }
                for shown in &lines {
                    let text = egui::RichText::new(&shown.text).monospace();
                    ui.horizontal(|ui| {
                        ui.label(if shown.colour == egui::Color32::PLACEHOLDER {
                            text.strong()
                        } else {
                            text.color(shown.colour)
                        });
                        // Plus and minus add the value to the accumulated values or subtract it.
                        if let Some((group, value)) = shown.value {
                            if ui.small_button("+").on_hover_text(tr!(lang, MeasureAdd)).clicked() {
                                self.accumulated.push((group, value));
                            }
                            if ui.small_button("−").on_hover_text(tr!(lang, MeasureSubtract)).clicked() {
                                self.accumulated.push((group, -value));
                            }
                        }
                    });
                }
                self.accumulated_ui(ui, lang);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!lines.is_empty(), egui::Button::new(tr!(lang, MeasureCopyAll)))
                        .clicked()
                    {
                        let all: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
                        ui.ctx().copy_text(all.join("\n"));
                    }
                    if ui.button(tr!(lang, MeasureRestart)).clicked() {
                        self.restart();
                    }
                    if ui.button(tr!(lang, MeasureDone)).clicked() {
                        done = true;
                    }
                });
            },
        );
        self.open = open && !done;
    }

    /// The accumulated values: per group its total, each value with a delete button, a button that
    /// clears the group, and Copy All for the totals.
    fn accumulated_ui(&mut self, ui: &mut egui::Ui, lang: Lang) {
        if self.accumulated.is_empty() {
            return;
        }
        ui.separator();
        ui.label(egui::RichText::new(tr!(lang, MeasureAccumulated)).strong());
        let mut remove = None;
        let mut clear = None;
        let mut totals = Vec::new();
        for group in [Group::Distance, Group::Area, Group::Angle] {
            let values: Vec<(usize, f64)> = self
                .accumulated
                .iter()
                .enumerate()
                .filter(|(_, (g, _))| *g == group)
                .map(|(i, (_, v))| (i, *v))
                .collect();
            if values.is_empty() {
                continue;
            }
            let name = match group {
                Group::Distance => tr!(lang, MeasureDistance),
                Group::Area => tr!(lang, MeasureArea),
                Group::Angle => tr!(lang, MeasureAngleGroup),
            };
            let total = group.show(values.iter().map(|(_, v)| v).sum());
            totals.push(format!("{name}: {total}"));
            ui.horizontal(|ui| {
                egui::CollapsingHeader::new(format!("{name} {total}")).id_salt(("accumulated", group)).show(
                    ui,
                    |ui| {
                        for (index, value) in &values {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(group.show(*value)).monospace());
                                if ui.small_button("×").on_hover_text(tr!(lang, MeasureDeleteValue)).clicked()
                                {
                                    remove = Some(*index);
                                }
                            });
                        }
                    },
                );
                if ui.small_button("×").on_hover_text(tr!(lang, MeasureDeleteGroup)).clicked() {
                    clear = Some(group);
                }
            });
        }
        if let Some(index) = remove {
            self.accumulated.remove(index);
        }
        if let Some(group) = clear {
            self.accumulated.retain(|(g, _)| *g != group);
        }
        if ui.small_button(tr!(lang, MeasureCopyAll)).clicked() {
            ui.ctx().copy_text(totals.join("\n"));
        }
    }

    /// Draws the selections' origins in their colours and, for a pair, the measured distance as a line in
    /// the view.
    pub(crate) fn overlay(&self, painter: &egui::Painter, viewport: &Viewport, session: &Session) {
        if !self.open {
            return;
        }
        let screen = |p: [f64; 3]| viewport.screen_of(gc_math::DVec3::from_array(p));
        if self.priority == Priority::Features {
            // A third point (the Shift angle) is drawn in the second selection's colour.
            for (colour, feature) in [
                (FIRST, self.features.first()),
                (SECOND, self.features.get(1)),
                (SECOND, self.features.get(2)),
            ] {
                match feature {
                    Some(Feature::Point(p)) => {
                        if let Some(at) = screen(*p) {
                            painter.circle(at, 5.0, colour, egui::Stroke::new(1.5, egui::Color32::WHITE));
                        }
                    }
                    Some(Feature::Edge(a, b)) => {
                        if let (Some(a), Some(b)) = (screen(*a), screen(*b)) {
                            painter.line_segment([a, b], egui::Stroke::new(4.0, colour));
                        }
                    }
                    Some(Feature::Cylinder { centre, axis, length, .. }) => {
                        let end = |sign: f64| [0, 1, 2].map(|i| centre[i] + axis[i] * length / 2.0 * sign);
                        if let (Some(a), Some(b)) = (screen(end(-1.0)), screen(end(1.0))) {
                            painter.line_segment([a, b], egui::Stroke::new(2.5, colour));
                        }
                    }
                    Some(Feature::Face { point, normal, .. }) => {
                        // The face's point with a short normal arrow.
                        let tip = [0, 1, 2].map(|i| point[i] + normal[i] * 0.05);
                        if let (Some(a), Some(b)) = (screen(*point), screen(tip)) {
                            painter.arrow(a, b - a, egui::Stroke::new(2.5, colour));
                        }
                    }
                    Some(Feature::Cone { apex, .. }) => {
                        if let Some(at) = screen(*apex) {
                            painter.circle(at, 5.0, colour, egui::Stroke::new(1.5, egui::Color32::WHITE));
                        }
                    }
                    Some(Feature::Circle { centre, .. } | Feature::Sphere { centre, .. }) => {
                        if let Some(at) = screen(*centre) {
                            painter.circle(at, 5.0, colour, egui::Stroke::new(1.5, egui::Color32::WHITE));
                        }
                    }
                    None => {}
                }
            }
            if let [a, b] = self.features.as_slice()
                && let Some((p, q)) = gc_session::measure_features(a, b).closest
                && let (Some(p), Some(q)) = (screen(p), screen(q))
            {
                painter
                    .line_segment([p, q], egui::Stroke::new(2.0, egui::Color32::from_rgb(0xff, 0x80, 0x00)));
            }
            return;
        }
        for (colour, item) in [(FIRST, self.first), (SECOND, self.second)] {
            if let Some(at) =
                item.and_then(|item| session.measure_item(item)).and_then(|m| screen(m.position))
            {
                painter.circle(at, 5.0, colour, egui::Stroke::new(1.5, egui::Color32::WHITE));
            }
        }
        let Some(pair) = &self.pair else { return };
        let ends = match (self.kind, pair.minimum) {
            (DistanceKind::Minimum, Some(minimum)) => Some((minimum.on_first, minimum.on_second)),
            (DistanceKind::Maximum, _) => pair.maximum.map(|m| (m.on_first, m.on_second)),
            (DistanceKind::CentreToCentre, _) => self.first.zip(self.second).and_then(|(a, b)| {
                Some((session.measure_item(a)?.position, session.measure_item(b)?.position))
            }),
            _ => None,
        };
        if let Some((a, b)) = ends
            && let (Some(a), Some(b)) = (screen(a), screen(b))
        {
            painter.line_segment([a, b], egui::Stroke::new(2.0, egui::Color32::from_rgb(0xff, 0x80, 0x00)));
            painter.circle_filled(a, 3.5, FIRST);
            painter.circle_filled(b, 3.5, SECOND);
        }
    }
}
