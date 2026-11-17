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
    pub(crate) fn follow_view(&mut self, viewport: &mut Viewport) {
        viewport.measure_features = self.open && self.priority == Priority::Features;
        if let Some(pick) = viewport.take_measure_pick() {
            match pick {
                None => self.restart(),
                Some((_, feature)) => {
                    if self.features.len() >= 2 {
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
    fn feature_lines(&self, lang: Lang) -> Vec<(egui::Color32, String)> {
        let mut lines = Vec::new();
        for (colour, title, feature) in [
            (FIRST, tr!(lang, MeasureFirst), self.features.first()),
            (SECOND, tr!(lang, MeasureSecond), self.features.get(1)),
        ] {
            let Some(feature) = feature else { continue };
            let text = match feature {
                Feature::Point(p) => {
                    let [x, y, z] = p.map(mm);
                    tr!(lang, MeasurePoint, x, y, z)
                }
                Feature::Edge(a, b) => {
                    tr!(
                        lang,
                        MeasureEdge,
                        mm(gc_math::DVec3::from_array(*a).distance(gc_math::DVec3::from_array(*b)))
                    )
                }
                Feature::Face { area, .. } => tr!(lang, MeasureFace, format!("{:.2}", area * 1e6)),
            };
            lines.push((colour, format!("{title}: {text}")));
        }
        if let [a, b] = self.features.as_slice() {
            let pair: FeaturePair = gc_session::measure_features(a, b);
            let text = egui::Color32::PLACEHOLDER;
            if let Some(distance) = pair.distance {
                lines.push((text, format!("{}: {} mm", tr!(lang, MeasureDistance), mm(distance))));
            }
            if let Some(delta) = pair.delta {
                let [dx, dy, dz] = delta.map(mm);
                lines.push((text, tr!(lang, MeasureDelta, dx, dy, dz)));
            }
            if let Some(angle) = pair.angle {
                lines.push((text, tr!(lang, MeasureFeatureAngle, format!("{:.3}", angle.to_degrees()))));
            }
        }
        lines
    }

    /// The values as text, one per line (what Copy All copies).
    fn lines(&self, session: &Session, lang: Lang) -> Vec<(egui::Color32, String)> {
        if self.priority == Priority::Features {
            return self.feature_lines(lang);
        }
        let mut lines = Vec::new();
        for (colour, title, item) in
            [(FIRST, tr!(lang, MeasureFirst), self.first), (SECOND, tr!(lang, MeasureSecond), self.second)]
        {
            let Some(measured) = item.and_then(|item| session.measure_item(item)) else { continue };
            let [x, y, z] = measured.position.map(mm);
            lines.push((colour, format!("{title}: {}", measured.name)));
            lines.push((colour, tr!(lang, MeasurePosition, x, y, z)));
        }
        if let Some(pair) = &self.pair {
            let text = egui::Color32::PLACEHOLDER;
            match (self.kind, pair.minimum) {
                (DistanceKind::Minimum, Some(minimum)) => {
                    lines.push((text, format!("{}: {} mm", tr!(lang, MeasureMinimum), mm(minimum.distance))));
                }
                (DistanceKind::Minimum, None) => lines.push((text, tr!(lang, MeasureNoMinimum).to_owned())),
                (DistanceKind::CentreToCentre, _) => {
                    lines.push((
                        text,
                        format!("{}: {} mm", tr!(lang, MeasureCentres), mm(pair.centre_to_centre)),
                    ));
                }
            }
            let [dx, dy, dz] = pair.delta.map(mm);
            lines.push((text, tr!(lang, MeasureDelta, dx, dy, dz)));
            if let Some(angle) = pair.angle {
                lines.push((text, tr!(lang, MeasureAngle, format!("{:.3}", angle.to_degrees()))));
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
                    });
                }
                ui.separator();
                let lines = self.lines(session, lang);
                if self.second.is_none() && self.features.len() < 2 {
                    ui.label(egui::RichText::new(tr!(lang, MeasureHint)).weak());
                }
                for (colour, line) in &lines {
                    let text = egui::RichText::new(line).monospace();
                    ui.label(if *colour == egui::Color32::PLACEHOLDER {
                        text.strong()
                    } else {
                        text.color(*colour)
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!lines.is_empty(), egui::Button::new(tr!(lang, MeasureCopyAll)))
                        .clicked()
                    {
                        let all: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
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

    /// Draws the selections' origins in their colours and, for a pair, the measured distance as a line in
    /// the view.
    pub(crate) fn overlay(&self, painter: &egui::Painter, viewport: &Viewport, session: &Session) {
        if !self.open {
            return;
        }
        let screen = |p: [f64; 3]| viewport.screen_of(gc_math::DVec3::from_array(p));
        if self.priority == Priority::Features {
            for (colour, feature) in [(FIRST, self.features.first()), (SECOND, self.features.get(1))] {
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
                    Some(Feature::Face { point, normal, .. }) => {
                        // The face's point with a short normal arrow.
                        let tip = [0, 1, 2].map(|i| point[i] + normal[i] * 0.05);
                        if let (Some(a), Some(b)) = (screen(*point), screen(tip)) {
                            painter.arrow(a, b - a, egui::Stroke::new(2.5, colour));
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
