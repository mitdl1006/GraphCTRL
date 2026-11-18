//! Analyze Interference, as in Inventor (Inspect tab > Interference panel > Analyze Interference;
//! https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-80DED0A6-AC65-4C7C-AD85-0144EE6DAB5B.htm):
//! define Set #1 (and optionally Set #2) by picking components, OK lists each interference with its
//! volume and centroid and shows the overlap in the view; entries can be ignored and un-ignored,
//! double-clicked to zoom, and copied.

use std::collections::BTreeSet;

use gc_i18n::{Lang, tr};
use gc_session::{CellItem, InterferenceRow, Session};
use gc_viewport::Viewport;

/// Colour of the overlaps in the view.
const OVERLAP: egui::Color32 = egui::Color32::from_rgb(0xe8, 0x30, 0x30);
/// At most this many sample points are drawn per overlap.
const DRAWN_POINTS: usize = 400;

/// Which set picks go to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Defining {
    First,
    Second,
}

/// The Analyze Interference window.
#[derive(Default)]
pub(crate) struct Interference {
    /// Whether the window is open.
    pub open: bool,
    first: Vec<CellItem>,
    second: Vec<CellItem>,
    defining: Option<Defining>,
    seen: Option<CellItem>,
    /// The results once analysed; `None` while defining the sets.
    results: Option<Vec<InterferenceRow>>,
    /// Treat subassemblies as components: what interferes within a subassembly is not listed (Inventor 2026 help,
    /// Analyze Interference, GUID-80DED0A6).
    subassemblies_as_components: bool,
    ignored: BTreeSet<usize>,
    show_ignored: bool,
    highlighted: Option<usize>,
    /// Where to zoom, asked for by a double click (lowest and highest corner).
    zoom: Option<gc_math::Aabb>,
}

fn mm(v: f64) -> String {
    format!("{:.1}", v * 1000.0)
}

impl Interference {
    /// Opens the window with Set #1 being defined.
    pub(crate) fn start(&mut self, session: &Session) {
        *self = Self {
            open: true,
            defining: Some(Defining::First),
            seen: session.selected_item(),
            ..Self::default()
        };
    }

    /// Adds picked components to the set being defined (a second pick removes it).
    pub(crate) fn follow(&mut self, session: &Session) {
        let selected = session.selected_item();
        if !self.open || selected == self.seen {
            return;
        }
        self.seen = selected;
        let (Some(item), Some(defining)) = (selected, self.defining) else { return };
        if self.results.is_some() || !matches!(item, CellItem::Body(_) | CellItem::Robot(_)) {
            return;
        }
        let set = match defining {
            Defining::First => &mut self.first,
            Defining::Second => &mut self.second,
        };
        if let Some(at) = set.iter().position(|i| *i == item) {
            set.remove(at);
        } else {
            set.push(item);
        }
    }

    fn name(session: &Session, item: CellItem) -> String {
        session.item_name(item).unwrap_or_default()
    }

    fn row_text(session: &Session, lang: Lang, row: &InterferenceRow) -> String {
        let [x, y, z] = row.overlap.centroid.map(mm);
        tr!(
            lang,
            InterferenceRow,
            Self::name(session, row.first),
            Self::name(session, row.second),
            format!("{:.1}", row.overlap.volume * 1e9),
            x,
            y,
            z
        )
    }

    fn set_ui(&mut self, ui: &mut egui::Ui, session: &Session, lang: Lang, which: Defining) {
        let title = match which {
            Defining::First => tr!(lang, InterferenceSet1),
            Defining::Second => tr!(lang, InterferenceSet2),
        };
        if ui.selectable_label(self.defining == Some(which), title).clicked() {
            self.defining = Some(which);
        }
        let set = match which {
            Defining::First => &mut self.first,
            Defining::Second => &mut self.second,
        };
        let mut remove = None;
        for (index, item) in set.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("  {}", Self::name(session, *item)));
                if ui.small_button("×").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            set.remove(index);
        }
    }

    /// The window.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &Session, lang: Lang) {
        let mut open = self.open;
        let mut close = false;
        egui::Window::new(tr!(lang, MenuInterference)).open(&mut open).default_width(460.0).show(ctx, |ui| {
            match self.results.take() {
                None => {
                    ui.label(egui::RichText::new(tr!(lang, InterferenceHint)).weak());
                    ui.separator();
                    self.set_ui(ui, session, lang, Defining::First);
                    ui.add_space(4.0);
                    self.set_ui(ui, session, lang, Defining::Second);
                    ui.add_space(4.0);
                    if ui.button(tr!(lang, InterferenceAll)).clicked() {
                        let doc = session.doc();
                        self.first = doc
                            .robots()
                            .iter()
                            .map(|(id, _)| CellItem::Robot(id))
                            .chain(doc.bodies().iter().map(|(id, _)| CellItem::Body(id)))
                            .collect();
                        self.second.clear();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.subassemblies_as_components, tr!(lang, InterferenceSubassemblies));
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.first.is_empty(), egui::Button::new(tr!(lang, Ok))).clicked()
                        {
                            self.results = Some(session.interference_with(
                                &self.first,
                                &self.second,
                                gc_session::InterferenceOptions {
                                    subassemblies_as_components: self.subassemblies_as_components,
                                },
                            ));
                            self.ignored.clear();
                            self.highlighted = None;
                            self.defining = None;
                        }
                        if ui.button(tr!(lang, Cancel)).clicked() {
                            close = true;
                        }
                    });
                }
                Some(rows) => {
                    self.results_ui(ui, session, lang, &rows);
                    if ui.button(tr!(lang, InterferenceAgain)).clicked() {
                        self.defining = Some(Defining::First);
                    } else {
                        self.results = Some(rows);
                    }
                }
            }
        });
        self.open = open && !close;
    }

    fn results_ui(&mut self, ui: &mut egui::Ui, session: &Session, lang: Lang, rows: &[InterferenceRow]) {
        if rows.is_empty() {
            ui.label(tr!(lang, InterferenceNone));
            ui.separator();
            return;
        }
        let total: f64 = rows.iter().map(|r| r.overlap.volume).sum();
        ui.label(
            egui::RichText::new(tr!(lang, InterferenceFound, rows.len(), format!("{:.1}", total * 1e9)))
                .strong(),
        );
        ui.checkbox(&mut self.show_ignored, tr!(lang, InterferenceShowIgnored));
        ui.label(egui::RichText::new(tr!(lang, InterferenceEstimate)).weak().size(11.0));
        ui.separator();
        let mut lines = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let ignored = self.ignored.contains(&index);
            if ignored && !self.show_ignored {
                continue;
            }
            let line = Self::row_text(session, lang, row);
            let mut text = egui::RichText::new(&line).monospace().size(11.0);
            if ignored {
                text = text.strikethrough().weak();
            }
            let response = ui.selectable_label(self.highlighted == Some(index), text);
            if response.clicked() {
                self.highlighted = Some(index);
            }
            if response.double_clicked() {
                let points = row.overlap.points.iter().map(|p| gc_math::DVec3::from_array(*p));
                let bounds = gc_math::Aabb::from_points(points);
                // A small overlap is shown with some of its surroundings.
                let pad = gc_math::DVec3::splat(0.05);
                self.zoom = Some(gc_math::Aabb { min: bounds.min - pad, max: bounds.max + pad });
            }
            response.context_menu(|ui| {
                if ignored {
                    if ui.button(tr!(lang, InterferenceUnignore)).clicked() {
                        self.ignored.remove(&index);
                        ui.close();
                    }
                } else {
                    if ui.button(tr!(lang, InterferenceIgnore)).clicked() {
                        self.ignored.insert(index);
                        ui.close();
                    }
                    if ui.button(tr!(lang, InterferenceIgnoreSmaller)).clicked() {
                        let limit = row.overlap.volume;
                        self.ignored.extend(
                            rows.iter()
                                .enumerate()
                                .filter(|(_, r)| r.overlap.volume <= limit)
                                .map(|(i, _)| i),
                        );
                        ui.close();
                    }
                }
                if ui.button(tr!(lang, InterferenceUnignoreAll)).clicked() {
                    self.ignored.clear();
                    ui.close();
                }
            });
            lines.push(line);
        }
        ui.separator();
        if ui.button(tr!(lang, MeasureCopyAll)).clicked() {
            ui.ctx().copy_text(lines.join("\n"));
        }
    }

    /// Zooms the view where a double click asked.
    pub(crate) fn zoom(&mut self, viewport: &mut Viewport) {
        if let Some(bounds) = self.zoom.take() {
            viewport.camera.fit(&bounds, [1000.0, 700.0]);
        }
    }

    /// Shows the overlaps (not ignored) as red points; the highlighted one larger.
    pub(crate) fn overlay(&self, painter: &egui::Painter, viewport: &Viewport) {
        let Some(rows) = self.results.as_ref().filter(|_| self.open) else { return };
        for (index, row) in rows.iter().enumerate() {
            if self.ignored.contains(&index) {
                continue;
            }
            let radius = if self.highlighted == Some(index) { 3.0 } else { 2.0 };
            let step = row.overlap.points.len().div_ceil(DRAWN_POINTS).max(1);
            for point in row.overlap.points.iter().step_by(step) {
                if let Some(at) = viewport.screen_of(gc_math::DVec3::from_array(*point)) {
                    painter.circle_filled(at, radius, OVERLAP);
                }
            }
        }
    }
}
