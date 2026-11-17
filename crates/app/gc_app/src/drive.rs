//! Drive, as Inventor's Drive dialog for a relationship
//! (https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-B78E59C9-F9FB-4ED5-B60E-C9198770B1AF.htm,
//! reference GUID-267E6CE6-7B8D-4972-A20A-350477A447DF): a robot joint is driven from Start to End
//! (default Start + 10) in increments (an amount, or a total number of steps), with a pause between
//! steps, repeated Start/End or Start/End/Start; Collision Detection stops the motion at a collision.
//! Driving shows the motion only: closing the dialog puts the robot back as the document has it.

use gc_i18n::{Lang, tr};
use gc_session::{CellItem, Session, StateProblem};

/// How the step size is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Increment {
    /// Each step moves by the amount.
    Amount,
    /// Start to End in this many equal steps.
    Steps,
}

/// How the sequence repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Repeat {
    /// Start to End, then back at Start, as many times as asked.
    StartEnd,
    /// Start to End, then End to Start, and so on: each one-way motion is one cycle.
    StartEndStart,
}

/// Playing state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Playing {
    Stopped,
    Forward,
    Reverse,
}

/// The values of a drive sequence, in the joint's display unit: every step of every repetition.
pub(crate) fn sequence(
    start: f64,
    end: f64,
    increment: Increment,
    step: f64,
    repeat: Repeat,
    count: u32,
) -> Vec<f64> {
    let span = end - start;
    let steps = match increment {
        Increment::Amount if step.abs() > 1e-12 => (span.abs() / step.abs()).ceil(),
        Increment::Steps => step.round().max(1.0),
        Increment::Amount => 1.0,
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to a sane count")]
    let steps = steps.clamp(1.0, 100_000.0) as usize;
    #[allow(clippy::cast_precision_loss, reason = "at most 100 000 steps")]
    let at = |i: usize| match increment {
        Increment::Amount => start + (step.abs() * span.signum() * i as f64).clamp(-span.abs(), span.abs()),
        Increment::Steps => start + span * i as f64 / steps as f64,
    };
    let forward: Vec<f64> = (0..=steps).map(at).collect();
    let mut values = Vec::new();
    let count = count.clamp(1, 1000);
    for cycle in 0..count {
        match repeat {
            Repeat::StartEnd => values.extend(&forward),
            Repeat::StartEndStart => {
                let pass: Box<dyn Iterator<Item = &f64>> =
                    if cycle % 2 == 0 { Box::new(forward.iter()) } else { Box::new(forward.iter().rev()) };
                // Each pass starts where the last one ended.
                values.extend(pass.skip(usize::from(cycle > 0)));
            }
        }
    }
    values
}

/// The Drive window.
pub(crate) struct Drive {
    /// Whether the window is open.
    pub open: bool,
    robot: Option<gc_core::Id<gc_doc::Robot>>,
    joint: usize,
    start: f64,
    end: f64,
    pause: f64,
    more: bool,
    collision: bool,
    increment: Increment,
    step: f64,
    repeat: Repeat,
    count: u32,
    playing: Playing,
    /// Where in the sequence the robot is.
    index: usize,
    /// When the last step was taken (egui time).
    last_step: f64,
    /// What stopped the drive, and the value where it stopped.
    collided: Option<(StateProblem, String)>,
}

impl Default for Drive {
    fn default() -> Self {
        Self {
            open: false,
            robot: None,
            joint: 0,
            start: 0.0,
            end: 10.0,
            pause: 0.0,
            more: false,
            collision: false,
            increment: Increment::Amount,
            step: 1.0,
            repeat: Repeat::StartEnd,
            count: 1,
            playing: Playing::Stopped,
            index: 0,
            last_step: 0.0,
            collided: None,
        }
    }
}

/// Display factor and unit of a joint: degrees for turning joints, millimetres for sliding ones.
fn unit(session: &Session, robot: gc_core::Id<gc_doc::Robot>, joint: usize) -> (f64, &'static str) {
    let sliding = session
        .robot_model(robot)
        .and_then(|loaded| loaded.chain.kinds().get(joint).copied())
        .is_some_and(|kind| kind == gc_robot::JointKind::Prismatic);
    if sliding { (1000.0, "mm") } else { (180.0 / std::f64::consts::PI, "°") }
}

impl Drive {
    /// Opens the dialog for a robot (the selected one, else the first), joint 1, starting where it is.
    pub(crate) fn start(&mut self, session: &mut Session) {
        let robot = match session.selected_item() {
            Some(CellItem::Robot(id)) => Some(id),
            _ => session.doc().robots().iter().next().map(|(id, _)| id),
        };
        *self = Self { open: true, robot, ..Self::default() };
        self.reset_values(session);
    }

    /// Start at the joint's value, End 10 further (Inventor's defaults).
    fn reset_values(&mut self, session: &mut Session) {
        session.end_drive();
        let Some(robot) = self.robot else { return };
        let (factor, _) = unit(session, robot, self.joint);
        let now = session.robot_joints(robot).and_then(|q| q.get(self.joint).copied()).unwrap_or(0.0);
        self.start = (now * factor * 1000.0).round() / 1000.0;
        self.end = self.start + 10.0;
        self.index = 0;
        self.playing = Playing::Stopped;
        self.collided = None;
    }

    fn values(&self) -> Vec<f64> {
        sequence(self.start, self.end, self.increment, self.step, self.repeat, self.count)
    }

    /// Shows the robot at a step; with Collision Detection, a collision stops the motion. Returns false
    /// when it stopped.
    fn show(&mut self, session: &mut Session, index: usize) -> bool {
        let Some(robot) = self.robot else { return false };
        let values = self.values();
        let Some(value) = values.get(index).copied() else { return false };
        let Some(mut joints) = session.robot_joints(robot) else { return false };
        let (factor, unit_name) = unit(session, robot, self.joint);
        if let Some(q) = joints.get_mut(self.joint) {
            *q = value / factor;
        }
        session.drive_show(robot, &joints);
        self.index = index;
        if self.collision
            && let Some(problem @ (StateProblem::Collision { .. } | StateProblem::WithinPadding { .. })) =
                session.robot_problem(robot)
        {
            self.collided = Some((problem, format!("{value:.3}{unit_name}")));
            self.playing = Playing::Stopped;
            return false;
        }
        true
    }

    /// Advances a playing sequence by one step when the pause has passed.
    pub(crate) fn tick(&mut self, ctx: &egui::Context, session: &mut Session) {
        if !self.open || self.playing == Playing::Stopped {
            return;
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_step < self.pause {
            ctx.request_repaint();
            return;
        }
        self.last_step = now;
        let last = self.values().len().saturating_sub(1);
        let next = match self.playing {
            Playing::Forward if self.index < last => Some(self.index + 1),
            Playing::Reverse if self.index > 0 => Some(self.index - 1),
            _ => None,
        };
        match next {
            Some(index) if self.show(session, index) => ctx.request_repaint(),
            _ => self.playing = Playing::Stopped,
        }
    }

    /// Sets End.
    pub(crate) fn set_end(&mut self, end: f64) {
        self.end = end;
    }

    /// Closes the dialog and puts the robot back as the document has it.
    pub(crate) fn close(&mut self, session: &mut Session) {
        session.end_drive();
        self.open = false;
    }

    /// The dialog. Closing it puts the robot back as the document has it.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        egui::Window::new(tr!(lang, MenuDrive)).open(&mut open).default_width(380.0).resizable(false).show(
            ctx,
            |ui| {
                self.contents(ui, session, lang);
            },
        );
        if !open {
            session.end_drive();
        }
        self.open = open;
    }

    fn contents(&mut self, ui: &mut egui::Ui, session: &mut Session, lang: Lang) {
        let robots: Vec<(gc_core::Id<gc_doc::Robot>, String)> =
            session.doc().robots().iter().map(|(id, r)| (id, r.name.clone())).collect();
        let Some(robot) = self.robot.filter(|id| robots.iter().any(|(r, _)| r == id)) else {
            ui.label(tr!(lang, DriveRobot));
            return;
        };
        let joint_names: Vec<String> =
            session.robot_model(robot).map(|l| l.chain.joint_names().to_vec()).unwrap_or_default();
        let (before_robot, before_joint) = (self.robot, self.joint);
        egui::Grid::new("drive_target").num_columns(2).show(ui, |ui| {
            let label = ui.label(tr!(lang, DriveRobot));
            let name = robots.iter().find(|(id, _)| *id == robot).map(|(_, n)| n.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("drive_robot")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for (id, name) in &robots {
                        ui.selectable_value(&mut self.robot, Some(*id), name);
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
            let label = ui.label(tr!(lang, DriveJoint));
            let joint = joint_names.get(self.joint).cloned().unwrap_or_default();
            egui::ComboBox::from_id_salt("drive_joint")
                .selected_text(joint)
                .show_ui(ui, |ui| {
                    for (index, name) in joint_names.iter().enumerate() {
                        ui.selectable_value(&mut self.joint, index, name);
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
        });
        if (self.robot, self.joint) != (before_robot, before_joint) {
            self.reset_values(session);
        }
        let (_, unit_name) = unit(session, robot, self.joint);
        let stopped = self.playing == Playing::Stopped;
        ui.add_enabled_ui(stopped, |ui| {
            egui::Grid::new("drive_values").num_columns(2).show(ui, |ui| {
                let label = ui.label(tr!(lang, DriveStart));
                ui.add(egui::DragValue::new(&mut self.start).speed(1.0).suffix(unit_name))
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(tr!(lang, DriveEnd));
                ui.add(egui::DragValue::new(&mut self.end).speed(1.0).suffix(unit_name))
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(tr!(lang, DrivePause));
                ui.add(egui::DragValue::new(&mut self.pause).speed(0.01).range(0.0..=10.0))
                    .labelled_by(label.id);
                ui.end_row();
            });
        });
        ui.horizontal(|ui| {
            let values_len = self.values().len();
            let last = values_len.saturating_sub(1);
            if ui.button(tr!(lang, DriveToStart)).on_hover_text(tr!(lang, DriveToStart)).clicked() {
                self.playing = Playing::Stopped;
                self.collided = None;
                self.show(session, 0);
            }
            if ui
                .add_enabled(stopped && self.index > 0, egui::Button::new(tr!(lang, DriveStepReverse)))
                .clicked()
            {
                self.show(session, self.index.saturating_sub(1));
            }
            if ui.button(tr!(lang, DriveReverse)).clicked() {
                self.collided = None;
                self.playing = Playing::Reverse;
            }
            if ui.button(tr!(lang, DriveStop)).clicked() {
                self.playing = Playing::Stopped;
            }
            if ui.button(tr!(lang, DriveForward)).clicked() {
                self.collided = None;
                if self.index >= last {
                    self.index = 0;
                }
                self.playing = Playing::Forward;
            }
            if ui
                .add_enabled(stopped && self.index < last, egui::Button::new(tr!(lang, DriveStepForward)))
                .clicked()
            {
                self.show(session, self.index + 1);
            }
            if ui.button(tr!(lang, DriveToEnd)).clicked() {
                self.playing = Playing::Stopped;
                self.show(session, last);
            }
        });
        if let Some(value) = self.values().get(self.index) {
            ui.label(tr!(lang, DriveValue, format!("{value:.3}{unit_name}")));
        }
        if let Some((problem, at)) = &self.collided {
            let what = gc_panels::problem_text(lang, problem);
            ui.colored_label(ui.visuals().error_fg_color, tr!(lang, DriveCollided, what, at));
        }
        ui.toggle_value(&mut self.more, tr!(lang, DriveMore));
        if self.more {
            ui.add_enabled_ui(stopped, |ui| {
                ui.checkbox(&mut self.collision, tr!(lang, DriveCollision));
                ui.label(egui::RichText::new(tr!(lang, DriveIncrement)).strong());
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.increment, Increment::Amount, tr!(lang, DriveByValue));
                    ui.radio_value(&mut self.increment, Increment::Steps, tr!(lang, DriveBySteps));
                    ui.add(egui::DragValue::new(&mut self.step).speed(0.1).range(0.001..=100_000.0))
                        .on_hover_text(tr!(lang, DriveIncrement));
                });
                ui.label(egui::RichText::new(tr!(lang, DriveRepetitions)).strong());
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.repeat, Repeat::StartEnd, tr!(lang, DriveStartEnd));
                    ui.radio_value(&mut self.repeat, Repeat::StartEndStart, tr!(lang, DriveStartEndStart));
                    ui.add(egui::DragValue::new(&mut self.count).range(1..=1000))
                        .on_hover_text(tr!(lang, DriveRepetitions));
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_follow_the_inventor_reference() {
        // Start 0, End 10, increment 1: eleven values.
        let once = sequence(0.0, 10.0, Increment::Amount, 1.0, Repeat::StartEnd, 1);
        assert_eq!(once.len(), 11);
        assert_eq!(once.last().copied(), Some(10.0));
        // An increment that does not divide the span ends exactly at End.
        let uneven = sequence(0.0, 10.0, Increment::Amount, 3.0, Repeat::StartEnd, 1);
        assert_eq!(uneven, vec![0.0, 3.0, 6.0, 9.0, 10.0]);
        // Total number of steps; driving downwards.
        let steps = sequence(10.0, 0.0, Increment::Steps, 4.0, Repeat::StartEnd, 1);
        assert_eq!(steps, vec![10.0, 7.5, 5.0, 2.5, 0.0]);
        // Start/End twice: back at Start between repetitions.
        let twice = sequence(0.0, 2.0, Increment::Amount, 1.0, Repeat::StartEnd, 2);
        assert_eq!(twice, vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0]);
        // Start/End/Start with three cycles: out, in, out.
        let cycles = sequence(0.0, 2.0, Increment::Amount, 1.0, Repeat::StartEndStart, 3);
        assert_eq!(cycles, vec![0.0, 1.0, 2.0, 1.0, 0.0, 1.0, 2.0]);
    }
}
