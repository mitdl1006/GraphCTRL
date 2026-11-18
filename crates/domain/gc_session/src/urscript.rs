//! URScript programs from planned tasks (plan 9.6: offline programs; nothing is sent to a robot).
//!
//! Facts used (docs/research/robotics-domain.md, URScript): SI units (m, rad, s); `movej(q, a, v, r)` is
//! joint-interpolated with defaults a = 1.4 rad/s², v = 1.05 rad/s; `movel(pose, a, v, r)` moves the tool
//! centre point in a straight line with defaults a = 1.2 m/s², v = 0.25 m/s; a pose is
//! `p[x, y, z, rx, ry, rz]` with the rotation as a rotation vector, in the robot's base frame; `r` is a
//! blend radius in metres. Free motions are written as the joint values the plan found, so that the
//! robot takes the same arm configuration and the same way round obstacles as in the simulation.
//!
//! Not verified against URSim or a robot.

use gc_core::Id;
use gc_doc::{IoDirection, Robot};
use gc_math::DPose3;

use crate::{ProgramStep, Schedule, Session, TaskPlan};

/// Joint moves: acceleration (rad/s²) and speed (rad/s), URScript's defaults.
const JOINT: (f64, f64) = (1.4, 1.05);
/// Straight moves: acceleration (m/s²) and speed (m/s), URScript's defaults.
const LINEAR: (f64, f64) = (1.2, 0.25);
/// Blend radius at points a motion passes through without stopping: joint way points and curve points.
const BLEND: (f64, f64) = (0.01, 0.001);

/// A name as a URScript identifier: letters, digits and underscores, not starting with a digit.
fn identifier(name: &str) -> String {
    let mut out: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn numbers(values: &[f64]) -> String {
    values.iter().map(|v| format!("{v:.6}")).collect::<Vec<_>>().join(", ")
}

/// A pose as URScript writes it: position and rotation vector.
fn pose(pose: &DPose3) -> String {
    let turn = gc_math::rotvec_from_quat(pose.rotation);
    format!(
        "p[{}]",
        numbers(&[pose.translation.x, pose.translation.y, pose.translation.z, turn.x, turn.y, turn.z])
    )
}

fn boolean(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

impl Session {
    /// The commands of a planned task as URScript lines (without the surrounding `def`).
    fn urscript_lines(&self, plan: &TaskPlan, out: &mut String) {
        let doc = self.doc();
        let kit = doc.robot_tool_kit(plan.robot).and_then(|(kit, _)| doc.get(kit));
        // Poses are given in the robot's base frame, as on the teach pendant.
        let base = self.robot_base_frame(plan.robot).unwrap_or(DPose3::IDENTITY).inverse();
        let pin = |name: &str, direction: IoDirection| {
            kit.and_then(|k| k.signals.iter().find(|s| s.name == name && s.direction == direction))
                .map(|s| s.pin)
        };
        let mut line = |text: String| {
            out.push_str("  ");
            out.push_str(&text);
            out.push('\n');
        };
        let set = |name: &str, value: bool| match pin(name, IoDirection::Out) {
            Some(pin) => format!("set_digital_out({pin}, {})", boolean(value)),
            None => format!("# no output signal `{name}` on this robot's tool: set it to {}", boolean(value)),
        };
        for step in &plan.program {
            match step {
                ProgramStep::Joints(points) => {
                    for (index, q) in points.iter().enumerate() {
                        let blend = if index + 1 == points.len() { 0.0 } else { BLEND.0 };
                        line(format!("movej([{}], a={}, v={}, r={blend})", numbers(q), JOINT.0, JOINT.1));
                    }
                }
                ProgramStep::Linear { poses, speed } => {
                    let v = speed.unwrap_or(LINEAR.1);
                    for (index, world) in poses.iter().enumerate() {
                        let blend = if index + 1 == poses.len() { 0.0 } else { BLEND.1 };
                        line(format!("movel({}, a={}, v={v}, r={blend})", pose(&(base * *world)), LINEAR.0));
                    }
                }
                ProgramStep::Pause(seconds) => line(format!("sleep({seconds})")),
                ProgramStep::Output { name, value } => line(set(name, *value)),
                ProgramStep::Action(name) => {
                    line(format!("# tool action: {name}"));
                    let Some(action) = kit.and_then(|k| k.action(name)) else {
                        line("# this robot's tool has no such action".to_owned());
                        continue;
                    };
                    if let Some((signal, value)) = &action.set {
                        line(set(signal, *value));
                    }
                    match action
                        .confirm
                        .as_ref()
                        .and_then(|(signal, value)| Some((pin(signal, IoDirection::In)?, *value)))
                    {
                        // Wait for the confirmation, at most as long as the action allows.
                        Some((input, value)) if action.timeout > 0.0 => {
                            line("waited = 0.0".to_owned());
                            line(format!(
                                "while (get_digital_in({input}) != {} and waited < {}):",
                                boolean(value),
                                action.timeout
                            ));
                            line("  sleep(0.01)".to_owned());
                            line("  waited = waited + 0.01".to_owned());
                            line("end".to_owned());
                        }
                        _ if action.duration > 0.0 => line(format!("sleep({})", action.duration)),
                        _ => {}
                    }
                }
                ProgramStep::WaitFor { condition, timeout } => {
                    line(format!(
                        "# TODO wait for: {condition} (at most {timeout} s); signals have no model yet"
                    ));
                }
            }
        }
    }

    fn urscript_head(&self, robot: Id<Robot>, name: &str, out: &mut String) {
        let tcp = self.doc().get(robot).map(|r| gc_math::pose_from_xyz_rpy(r.tcp.xyz, r.tcp.rpy));
        out.push_str(&format!("def {}():\n", identifier(name)));
        out.push_str(&format!("  set_tcp({})\n", pose(&tcp.unwrap_or(DPose3::IDENTITY))));
    }

    /// A task as a URScript program for a robot: the task is planned for it from where it stands
    /// ([`Session::task_plan`]) and written as `def <task>(): … end`, starting with a joint move to where
    /// the plan starts. Free motions are the planned joint values, straight and curve-following motions
    /// are `movel` poses in the robot's base frame, waits are `sleep`, signals and tool actions use the
    /// pins of the robot's tool kit. Nothing is sent anywhere; the program has not been run on URSim.
    pub fn task_urscript(&self, task: Id<gc_doc::Task>, robot: Id<Robot>) -> Result<String, String> {
        let plan = self.task_plan(task, robot)?;
        let name = self.doc().get(task).map(|t| t.name.clone()).unwrap_or_default();
        let mut out = String::new();
        self.urscript_head(robot, &name, &mut out);
        out.push_str(&format!("  movej([{}], a={}, v={}, r=0)\n", numbers(&plan.start), JOINT.0, JOINT.1));
        self.urscript_lines(&plan, &mut out);
        out.push_str("end\n");
        Ok(out)
    }

    /// A schedule as one URScript program per robot that has something to do, in the order the robots
    /// were added: its tasks and its moves out of the way in the order they start, with `sleep` for the
    /// time it stands idle in between. The robots are kept in step by time alone.
    pub fn schedule_urscript(&self, schedule: &Schedule) -> Vec<(Id<Robot>, String)> {
        let doc = self.doc();
        let mut programs = Vec::new();
        for (robot, data) in doc.robots().iter() {
            // What it does: (start, end, title, plan).
            let tasks =
                schedule.assignments.iter().zip(&schedule.plans).filter(|(a, _)| a.robot == robot).map(
                    |(a, plan)| {
                        let title = doc.get(a.task).map(|t| t.name.clone()).unwrap_or_default();
                        (a.start, a.end, title, plan)
                    },
                );
            let parks = schedule
                .parks
                .iter()
                .filter(|p| p.robot == robot)
                .map(|p| (p.start, p.end, "out of the way".to_owned(), &p.plan));
            let mut entries: Vec<(f64, f64, String, &TaskPlan)> = tasks.chain(parks).collect();
            if entries.is_empty() {
                continue;
            }
            entries.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut out = String::new();
            self.urscript_head(robot, &data.name, &mut out);
            out.push_str(&format!(
                "  movej([{}], a={}, v={}, r=0)\n",
                numbers(&entries[0].3.start),
                JOINT.0,
                JOINT.1
            ));
            let mut clock = 0.0;
            for (start, end, title, plan) in entries {
                if start > clock + 1e-9 {
                    out.push_str(&format!("  sleep({:.3})\n", start - clock));
                }
                out.push_str(&format!("  # {title}\n"));
                self.urscript_lines(plan, &mut out);
                clock = end;
            }
            out.push_str("end\n");
            programs.push((robot, out));
        }
        programs
    }
}
