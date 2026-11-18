//! The Limits tab of a joint (Inventor 2026 help, To Define and Manage Joint Relationships,
//! GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6, Limits tab): Start, Current and End, each switched on with a
//! check box; angles in degrees and slides in millimetres. The joint stays within Start..End; Current
//! holds it there. The current position is shown beside the values. The tab lives in the Joint dialog.

use gc_doc::JointRange;
use gc_i18n::{Lang, tr};

const DEG: f64 = 180.0 / std::f64::consts::PI;
const MM: f64 = 1000.0;

/// One kind of limit being edited.
#[derive(Clone, Debug)]
struct RangeEdit {
    enabled: bool,
    start: String,
    end: String,
    hold: bool,
    current: String,
}

impl RangeEdit {
    /// From the stored range; `factor` turns the stored unit (rad or m) into the shown one (deg or mm).
    fn from(range: Option<JointRange>, factor: f64) -> Self {
        let show = |v: f64| format!("{}", (v * factor * 1000.0).round() / 1000.0);
        match range {
            Some(r) => Self {
                enabled: true,
                start: show(r.start),
                end: show(r.end),
                hold: r.current.is_some(),
                current: r.current.map_or_else(|| "0".to_owned(), show),
            },
            None => Self {
                enabled: false,
                start: "0".to_owned(),
                end: "0".to_owned(),
                hold: false,
                current: "0".to_owned(),
            },
        }
    }

    /// The stored range, or `None` when switched off or a value is not a number.
    fn range(&self, factor: f64) -> Option<JointRange> {
        if !self.enabled {
            return None;
        }
        let number = |t: &str| t.trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v / factor);
        Some(JointRange {
            start: number(&self.start)?,
            end: number(&self.end)?,
            current: if self.hold { Some(number(&self.current)?) } else { None },
        })
    }

    fn valid(&self, factor: f64) -> bool {
        !self.enabled || self.range(factor).is_some_and(|r| r.is_valid())
    }
}

/// The values of a joint's Limits tab.
#[derive(Clone, Debug)]
pub(crate) struct LimitsEdit {
    angular: RangeEdit,
    linear: RangeEdit,
}

impl Default for LimitsEdit {
    fn default() -> Self {
        Self::from((None, None))
    }
}

impl LimitsEdit {
    /// From the stored limits: (angular, linear).
    pub(crate) fn from(limits: (Option<JointRange>, Option<JointRange>)) -> Self {
        Self { angular: RangeEdit::from(limits.0, DEG), linear: RangeEdit::from(limits.1, MM) }
    }

    /// The limits to store: (angular, linear); a kind the joint does not take is cleared.
    pub(crate) fn ranges(&self, angular: bool, linear: bool) -> (Option<JointRange>, Option<JointRange>) {
        (angular.then(|| self.angular.range(DEG)).flatten(), linear.then(|| self.linear.range(MM)).flatten())
    }

    /// Whether every value that is switched on is usable.
    pub(crate) fn valid(&self, angular: bool, linear: bool) -> bool {
        (!angular || self.angular.valid(DEG)) && (!linear || self.linear.valid(MM))
    }

    /// The tab's content. `angular` and `linear` say which limits the joint takes; `position` is where it
    /// stands now: (turn in radians, slide in metres).
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        angular: bool,
        linear: bool,
        position: Option<(f64, f64)>,
        lang: Lang,
    ) {
        if !angular && !linear {
            ui.label(tr!(lang, LimitNone));
            return;
        }
        for (edit, title, unit, now, on) in [
            (
                &mut self.angular,
                tr!(lang, LimitAngular),
                "°",
                position.map(|(rad, _)| format!("{:.3}", rad * DEG)),
                angular,
            ),
            (
                &mut self.linear,
                tr!(lang, LimitLinear),
                "mm",
                position.map(|(_, m)| format!("{:.3}", m * MM)),
                linear,
            ),
        ] {
            if !on {
                continue;
            }
            ui.checkbox(&mut edit.enabled, title);
            ui.add_enabled_ui(edit.enabled, |ui| {
                egui::Grid::new(title).num_columns(3).show(ui, |ui| {
                    let start = ui.label(tr!(lang, LimitStart));
                    ui.add(egui::TextEdit::singleline(&mut edit.start).desired_width(80.0))
                        .labelled_by(start.id);
                    ui.label(unit);
                    ui.end_row();
                    ui.checkbox(&mut edit.hold, tr!(lang, LimitCurrent));
                    ui.add_enabled(
                        edit.hold,
                        egui::TextEdit::singleline(&mut edit.current).desired_width(80.0),
                    );
                    ui.label(unit);
                    ui.end_row();
                    let end = ui.label(tr!(lang, LimitEnd));
                    ui.add(egui::TextEdit::singleline(&mut edit.end).desired_width(80.0)).labelled_by(end.id);
                    ui.label(unit);
                    ui.end_row();
                });
            });
            if let Some(now) = now {
                ui.weak(tr!(lang, LimitNow, format!("{now} {unit}")));
            }
        }
    }
}
