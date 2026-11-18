//! The Limits tab of a joint (Inventor 2026 help, To Define and Manage Joint Relationships,
//! GUID-21DC3336-5C51-42C1-90FB-4299CD66E0C6, Limits tab): Start, Current and End, each switched on with a
//! check box; angles in degrees and slides in millimetres. The joint stays within Start..End; Current
//! holds it there. The current position is shown beside the values.

use gc_core::Id;
use gc_doc::{JointRange, Relationship};
use gc_i18n::{Lang, tr};
use gc_session::Session;

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

/// The dialog's state.
#[derive(Default)]
pub(crate) struct JointLimits {
    pub(crate) open: bool,
    joint: Option<Id<Relationship>>,
    angular: Option<RangeEdit>,
    linear: Option<RangeEdit>,
}

impl JointLimits {
    /// Opens the dialog on a joint with its stored limits.
    pub(crate) fn start(&mut self, session: &Session, id: Id<Relationship>) {
        let Some(kind) = session.doc().get(id).map(|r| r.kind) else { return };
        let (angular, linear) = session.joint_limits(id);
        *self = Self {
            open: true,
            joint: Some(id),
            angular: gc_doc::takes_limits(&kind, true)
                .then(|| RangeEdit::from(angular, 180.0 / std::f64::consts::PI)),
            linear: gc_doc::takes_limits(&kind, false).then(|| RangeEdit::from(linear, 1000.0)),
        };
    }

    /// The dialog window. Apply keeps it open; OK applies and closes.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let Some(id) = self.joint else { return };
        let mut open = self.open;
        let (mut ok, mut apply, mut cancel) = (false, false, false);
        let deg = 180.0 / std::f64::consts::PI;
        let position = session.joint_position(id);
        let valid = self.angular.as_ref().is_none_or(|r| r.valid(deg))
            && self.linear.as_ref().is_none_or(|r| r.valid(1000.0));
        let screen = ctx.content_rect();
        egui::Window::new(tr!(lang, JointLimitsTitle))
            .open(&mut open)
            .default_pos(egui::pos2(screen.right() - 330.0, screen.top() + 200.0))
            .default_width(300.0)
            .show(ctx, |ui| {
                for (edit, title, unit, now) in [
                    (
                        &mut self.angular,
                        tr!(lang, LimitAngular),
                        "°",
                        position.map(|(rad, _)| format!("{:.3}", rad * deg)),
                    ),
                    (
                        &mut self.linear,
                        tr!(lang, LimitLinear),
                        "mm",
                        position.map(|(_, m)| format!("{:.3}", m * 1000.0)),
                    ),
                ] {
                    let Some(edit) = edit else { continue };
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
                            ui.add(egui::TextEdit::singleline(&mut edit.end).desired_width(80.0))
                                .labelled_by(end.id);
                            ui.label(unit);
                            ui.end_row();
                        });
                    });
                    if let Some(now) = now {
                        ui.weak(tr!(lang, LimitNow, format!("{now} {unit}")));
                    }
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ok = ui.add_enabled(valid, egui::Button::new(tr!(lang, ConstrainOk))).clicked();
                    apply = ui.add_enabled(valid, egui::Button::new(tr!(lang, ConstrainApply))).clicked();
                    cancel = ui.button(tr!(lang, ConstrainCancel)).clicked();
                });
            });
        if (ok || apply) && valid {
            let angular = self.angular.as_ref().and_then(|r| r.range(deg));
            let linear = self.linear.as_ref().and_then(|r| r.range(1000.0));
            // A refusal is explained in the status bar.
            drop(session.set_joint_limits(id, angular, linear));
        }
        if ok || cancel {
            open = false;
        }
        self.open = open;
    }
}
