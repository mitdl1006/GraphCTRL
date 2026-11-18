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
/// User text as it may stand in a comment: on one line, without control characters (a line break in a
/// name must not start a new command).
fn plain(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// A name as a function name. Letters outside ASCII cannot be kept, so a name that has any gets the
/// item's id appended: two such names stay apart.
fn function_name(name: &str, id: u64) -> String {
    let base = identifier(name);
    if name.chars().any(|c| !c.is_ascii()) { format!("{base}_{id}") } else { base }
}

fn identifier(name: &str) -> String {
    let mut out: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// A number with a fixed count of decimals (the same text on every machine; no "-0.000").
fn fixed(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') {
        return text.trim_start_matches('-').to_owned();
    }
    text
}

/// Joint values: six decimals (radians).
fn numbers(values: &[f64]) -> String {
    values.iter().map(|v| fixed(*v, 6)).collect::<Vec<_>>().join(", ")
}

/// Seconds, speeds, accelerations and blend radii: three decimals.
fn short(value: f64) -> String {
    fixed(value, 3)
}

/// A pose as URScript writes it: position (metres, five decimals) and rotation vector (six).
fn pose(pose: &DPose3) -> String {
    let turn = gc_math::rotvec_from_quat(pose.rotation);
    let at = [pose.translation.x, pose.translation.y, pose.translation.z].map(|v| fixed(v, 5));
    let turned = [turn.x, turn.y, turn.z].map(|v| fixed(v, 6));
    format!("p[{}, {}]", at.join(", "), turned.join(", "))
}

/// A joint move to `q` with a blend radius.
fn joint_move(q: &[f64], blend: f64) -> String {
    format!("movej([{}], a={}, v={}, r={})", numbers(q), short(JOINT.0), short(JOINT.1), short(blend))
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
            // One command or comment per line, whatever names and conditions hold.
            out.push_str(&plain(&text));
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
                        line(joint_move(q, blend));
                    }
                }
                ProgramStep::Linear { poses, speed } => {
                    let v = speed.unwrap_or(LINEAR.1);
                    for (index, world) in poses.iter().enumerate() {
                        let blend = if index + 1 == poses.len() { 0.0 } else { BLEND.1 };
                        line(format!(
                            "movel({}, a={}, v={}, r={})",
                            pose(&(base * *world)),
                            short(LINEAR.0),
                            short(v),
                            short(blend)
                        ));
                    }
                }
                ProgramStep::Pause(seconds) => line(format!("sleep({})", short(*seconds))),
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
                            let limit = short(action.timeout);
                            line("waited = 0.0".to_owned());
                            line(format!(
                                "while (get_digital_in({input}) != {} and waited < {limit}):",
                                boolean(value)
                            ));
                            line("  sleep(0.01)".to_owned());
                            line("  waited = waited + 0.01".to_owned());
                            line("end".to_owned());
                            // No confirmation in time: say so and stop, never carry on.
                            line(format!("if (waited >= {limit}):"));
                            line(format!(
                                "  popup(\"{}: no confirmation on input {input}\", \"GraphCTRL\", False, True)",
                                identifier(name)
                            ));
                            line("  halt".to_owned());
                            line("end".to_owned());
                        }
                        _ if action.duration > 0.0 => line(format!("sleep({})", short(action.duration))),
                        _ => {}
                    }
                }
                ProgramStep::WaitFor { condition, timeout } => {
                    let limit = short(*timeout);
                    let wait = gc_doc::signal_condition(condition)
                        .and_then(|(name, value)| Some((pin(&name, IoDirection::In)?, name, value)));
                    let Some((input, name, value)) = wait else {
                        line(format!(
                            "# TODO wait for: {condition} (at most {limit} s): no such input on this robot"
                        ));
                        continue;
                    };
                    line(format!("# wait for {name}"));
                    line("waited = 0.0".to_owned());
                    line(format!(
                        "while (get_digital_in({input}) != {} and waited < {limit}):",
                        boolean(value)
                    ));
                    line("  sleep(0.01)".to_owned());
                    line("  waited = waited + 0.01".to_owned());
                    line("end".to_owned());
                    line(format!("if (waited >= {limit}):"));
                    line(format!(
                        "  popup(\"{}: not seen on input {input}\", \"GraphCTRL\", False, True)",
                        identifier(&name)
                    ));
                    line("  halt".to_owned());
                    line("end".to_owned());
                }
            }
        }
    }

    fn urscript_head(&self, robot: Id<Robot>, name: &str, id: u64, out: &mut String) {
        out.push_str(&format!(
            "# Generated by GraphCTRL {}, document revision {}\n",
            env!("CARGO_PKG_VERSION"),
            self.doc().revision().0
        ));
        let tcp = self.doc().get(robot).map(|r| gc_math::pose_from_xyz_rpy(r.tcp.xyz, r.tcp.rpy));
        out.push_str(&format!("def {}():\n", function_name(name, id)));
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
        self.urscript_head(robot, &name, task.raw().get(), &mut out);
        out.push_str(&format!("  {}\n", joint_move(&plan.start, 0.0)));
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
            self.urscript_head(robot, &data.name, robot.raw().get(), &mut out);
            out.push_str(&format!("  {}\n", joint_move(&entries[0].3.start, 0.0)));
            let mut clock = 0.0;
            for (start, end, title, plan) in entries {
                if start > clock + 1e-9 {
                    out.push_str(&format!("  sleep({})\n", short(start - clock)));
                }
                out.push_str(&format!("  # {}\n", plain(&title)));
                self.urscript_lines(plan, &mut out);
                clock = end;
            }
            out.push_str("end\n");
            programs.push((robot, out));
        }
        programs
    }
}
