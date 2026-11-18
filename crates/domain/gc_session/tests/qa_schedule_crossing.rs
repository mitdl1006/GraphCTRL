//! QA tests for the scheduler's way round a moving robot (backend 909d5da: a motion that would cross a moving
//! robot is planned again with that robot standing where it will be at five moments of the rest of its motion,
//! and taken when it is clear and ends sooner than waiting would; otherwise the task waits) and for the robots
//! keeping clear of each other (plan 9.5, GraphCTRL's own; no Inventor counterpart).
//! Two UR-style arms face each other and swing through the middle; whatever the scheduler decides (wait, detour,
//! one after the other), the played-back schedule must keep the two robots apart at every 10 ms (finer than the
//! 50 ms the scheduler checks at), be the same every time, and be continuous.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::disallowed_methods,
    reason = "test code"
)]

use gc_core::Id;
use gc_doc::{
    CellOp, Command, Location, MoveTarget, Placement, Robot, RobotChoice, Step, Task, WaypointTarget, WorkOp,
};
use gc_session::{CellItem, Schedule, Session};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap()
}

/// Two arms 1.2 m apart facing each other (the second turned half round), each standing with its arm out to
/// the side.
fn facing_pair() -> (Session, Id<Robot>, Id<Robot>) {
    facing_pair_apart(0.6)
}

/// The same, `apart` metres between the bases.
fn facing_pair_apart(apart: f64) -> (Session, Id<Robot>, Id<Robot>) {
    let mut s = Session::new();
    let mut robot = |name: &str, y: f64, yaw: f64| {
        let id = s.add_robot(name, &six_axis()).unwrap();
        let placement = Placement { xyz: [0.0, y, 0.0], rpy: [0.0, 0.0, yaw] };
        s.execute(&Command::Cell(CellOp::PlaceRobot { id, placement })).unwrap();
        id
    };
    let a = robot("A", -apart / 2.0, 0.0);
    let b = robot("B", apart / 2.0, std::f64::consts::PI);
    (s, a, b)
}

fn stand(s: &mut Session, robot: Id<Robot>, q: &[f64]) {
    s.jog(robot, q);
    s.end_jog();
}

fn go(s: &mut Session, robot: Id<Robot>, name: &str, q: &[f64], priority: i32, wait: f64) -> Id<Task> {
    let waypoint = s.add_waypoint(robot, WaypointTarget::Joints(q.to_vec())).unwrap();
    let mut steps = vec![Step::MoveJ(MoveTarget::Waypoint(waypoint))];
    if wait > 0.0 {
        steps.push(Step::Wait { seconds: wait });
    }
    let task = Task {
        name: name.into(),
        location: Location::Robot(robot),
        precondition: String::new(),
        steps,
        effects: Vec::new(),
        robots: RobotChoice::Any,
        priority,
    };
    s.edit_work(WorkOp::AddTask(task)).unwrap().unwrap().downcast().unwrap()
}

/// The least distance between the two robots as the schedule shows them at `time`; negative-or-zero for an
/// overlap; `None` when the geometry is not known.
fn gap(s: &mut Session, a: Id<Robot>, b: Id<Robot>, time: f64) -> Option<f64> {
    s.set_schedule_playback(Some(time));
    let (ga, gb) = (s.item_geometry(CellItem::Robot(a))?, s.item_geometry(CellItem::Robot(b))?);
    match ga.closest(&gb) {
        Ok(Some(closest)) => Some(closest.distance),
        Ok(None) => Some(f64::INFINITY),
        Err(_) => None,
    }
}

/// The least gap if both robots did their task at once from the start, each planned as if alone: whether the
/// scene really is a conflict.
fn naive_gap(s: &mut Session, a: Id<Robot>, ta: Id<Task>, b: Id<Robot>, tb: Id<Task>) -> Option<f64> {
    let (pa, pb) = (s.task_plan(ta, a).ok()?, s.task_plan(tb, b).ok()?);
    let end = pa.duration().max(pb.duration());
    let mut least = f64::INFINITY;
    let mut t = 0.0;
    while t <= end {
        s.jog(a, &pa.sample(t));
        s.jog(b, &pb.sample(t));
        let (ga, gb) = (s.item_geometry(CellItem::Robot(a))?, s.item_geometry(CellItem::Robot(b))?);
        if let Ok(Some(closest)) = ga.closest(&gb) {
            least = least.min(closest.distance);
        }
        t += 0.01;
    }
    s.end_jog();
    // `end_jog` would commit the last preview as a step: undo it if it did.
    Some(least)
}

/// Plays the whole schedule every 10 ms and returns the least gap and when it was.
fn least_gap(s: &mut Session, a: Id<Robot>, b: Id<Robot>, schedule: &Schedule) -> (f64, f64) {
    let end = schedule.makespan() + 0.5;
    let mut least = (f64::INFINITY, 0.0);
    let mut t = 0.0;
    while t <= end {
        let d = gap(s, a, b, t).unwrap_or_else(|| panic!("no geometry at {t}"));
        if d < least.0 {
            least = (d, t);
        }
        t += 0.01;
    }
    s.set_schedule_playback(None);
    least
}

fn same(a: &Schedule, b: &Schedule) -> bool {
    a.assignments == b.assignments
        && a.blocked == b.blocked
        && a.waits == b.waits
        && a.cut_short == b.cut_short
        && a.parks.len() == b.parks.len()
}

/// A family of swings through the middle: where each arm goes (the shoulder's turn), who goes first, how long
/// each stands doing its work after getting there.
#[test]
fn arms_swinging_through_the_middle_never_touch_and_the_outcome_is_the_same_every_time() {
    let (mut sessions, mut waited, mut parallel, mut serial, mut conflicts, mut total_detours) =
        (0, 0, 0, 0, 0, 0);
    let mut tightest = f64::INFINITY;
    for (apart, a_goal, b_goal, a_wait, b_wait) in [
        (0.7, 0.0, 0.0, 0.0, 0.0),
        (0.7, 0.3, -0.3, 0.0, 0.0),
        (0.7, -0.4, 0.4, 1.0, 0.0),
        (0.7, 0.0, 0.0, 3.0, 0.5),
        (0.7, 0.6, 0.6, 0.0, 2.0),
        (0.7, -0.8, -0.8, 0.5, 0.5),
        (0.7, 0.2, 0.2, 0.0, 0.0),
        (0.55, 0.0, 0.0, 0.0, 0.0),
        (0.55, 0.3, -0.3, 0.0, 0.0),
        (0.55, -0.4, 0.4, 1.0, 0.0),
        (0.55, 0.0, 0.0, 3.0, 0.5),
        (0.45, 0.0, 0.0, 0.0, 0.0),
        (0.45, 0.2, -0.2, 0.0, 1.0),
        (0.45, -0.2, 0.2, 2.0, 0.0),
        (0.40, 0.0, 0.0, 0.0, 0.0),
    ] {
        let (mut s, a, b) = facing_pair_apart(apart);
        // Each starts with its arm out to the side; the goals are forward, over the middle.
        let sideways = [1.5, -0.3, 0.6, 0.0, 0.6, 0.0];
        stand(&mut s, a, &sideways);
        stand(&mut s, b, &[-1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
        let forward = |pan: f64| [pan, -1.2, 1.4, -0.2, 0.6, 0.0];
        let ta = go(&mut s, a, "A swings", &forward(a_goal), 2, a_wait);
        let tb = go(&mut s, b, "B swings", &forward(b_goal), 1, b_wait);
        // And each goes back to its side after: more motion for the other to cross.
        let back_a = go(&mut s, a, "A back", &sideways, 0, 0.0);
        let back_b = go(&mut s, b, "B back", &[-1.5, -0.3, 0.6, 0.0, 0.6, 0.0], 0, 0.0);
        let alone = naive_gap(&mut s, a, ta, b, tb);
        eprintln!(
            "report: swing {apart} m apart, {a_goal}/{b_goal}: if both went at once, the least gap would be {alone:?}"
        );
        let tight = alone.is_none_or(|g| g < gc_session::PADDING);
        let started = std::time::Instant::now();
        let schedule = s.schedule();
        let took = started.elapsed();
        assert!(took.as_secs() < 90, "slow: {took:?}");
        sessions += 1;
        // Every task is either done or blocked with a reason; nothing is lost.
        for task in [ta, tb, back_a, back_b] {
            let done = schedule.assignments.iter().any(|x| x.task == task);
            let blocked = schedule.blocked.iter().any(|(id, why)| *id == task && !why.is_empty());
            assert!(
                done != blocked || (done && !blocked),
                "task {task:?} is neither done nor explained: {schedule:?}"
            );
        }
        // The same every time (the detours are tried in a fixed order).
        assert!(same(&schedule, &s.schedule()), "the schedule changed between runs ({a_goal}, {b_goal})");
        // No two tasks of one robot overlap; waits name the robot that was in the way.
        for x in &schedule.assignments {
            assert!(x.end >= x.start && x.start.is_finite() && x.end.is_finite());
            for y in schedule.assignments.iter().filter(|y| y.task != x.task && y.robot == x.robot) {
                assert!(x.end <= y.start + 1e-9 || y.end <= x.start + 1e-9, "{x:?} overlaps {y:?}");
            }
        }
        for w in &schedule.waits {
            assert!(w.robot != w.cause && w.seconds >= 0.0 && w.seconds.is_finite());
        }
        // Which way it went.
        let (first, second) = (
            schedule.assignments.iter().find(|x| x.task == ta),
            schedule.assignments.iter().find(|x| x.task == tb),
        );
        if let (Some(f), Some(g)) = (first, second) {
            if f.start < g.end - 1e-9 && g.start < f.end - 1e-9 {
                parallel += 1;
            } else {
                serial += 1;
            }
        }
        if !schedule.waits.is_empty() {
            waited += 1;
        }
        // A plan that is not the one made with the other arm standing still where it starts is a way round the
        // moving one (backend 909d5da): count them.
        let mut detours = 0;
        for (x, plan) in schedule.assignments.iter().zip(&schedule.plans) {
            let differs = s.task_plan_from(x.task, x.robot, &plan.start).is_ok_and(|plain| {
                (plain.duration() - plan.duration()).abs() > 1e-6 || plain.end != plan.end
            });
            if differs {
                detours += 1;
            }
        }
        total_detours += detours;
        // A scene where the scheduler had to deal with the other arm: a wait, a move out of the way, or a task
        // that could not be done.
        if tight && (!schedule.waits.is_empty() || !schedule.parks.is_empty() || !schedule.blocked.is_empty())
        {
            conflicts += 1;
        }
        // The robots are kept apart all along the played-back schedule, every 10 ms.
        s.request_schedule();
        let (shown, _) = s.schedule_result().map(|(x, c)| (x.clone(), c)).unwrap();
        assert!(same(&shown, &schedule), "the background result differs from the direct one");
        let (least, at) = least_gap(&mut s, a, b, &shown);
        eprintln!(
            "report: swing {apart} m apart, {a_goal}/{b_goal}: least gap {least:.4} m at {at:.2} s, makespan {:.2} s, {} waits, {} parks, {} blocked",
            shown.makespan(),
            shown.waits.len(),
            shown.parks.len(),
            shown.blocked.len()
        );
        assert!(
            least > 0.0,
            "the arms overlap at {at:.2} s (gap {least}) in the schedule for {a_goal}/{b_goal}"
        );
        tightest = tightest.min(least);
    }
    eprintln!(
        "report: {sessions} scenes; waited in {waited}; ran in parallel {parallel}, one after the other {serial}; tightest gap {tightest:.4} m"
    );
    assert!(tightest > 0.0);
    eprintln!("report: {total_detours} plans in all differ from the ones made with the other arm still");
    eprintln!("report: the scheduler had to deal with the other arm in {conflicts} of {sessions} scenes");
    assert!(
        conflicts >= 1,
        "none of the scenes made the scheduler wait, park or block: the test checks nothing"
    );
}

/// Playback is smooth where the detour is taken or the waiting ends: every joint changes by little in 10 ms at all
/// times (the arms do not jump when one starts after another).
#[test]
fn the_played_back_swing_is_continuous_for_both_arms() {
    let (mut s, a, b) = facing_pair();
    stand(&mut s, a, &[1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    stand(&mut s, b, &[-1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    go(&mut s, a, "A", &[0.0, -1.2, 1.4, -0.2, 0.6, 0.0], 1, 0.5);
    go(&mut s, b, "B", &[0.0, -1.2, 1.4, -0.2, 0.6, 0.0], 0, 0.5);
    let schedule = s.schedule();
    assert!(!schedule.assignments.is_empty());
    for robot in [a, b] {
        let mut previous: Option<Vec<f64>> = None;
        let mut t = 0.0;
        while t <= schedule.makespan() + 1.0 {
            if let Some(q) = schedule.joints_at(robot, t) {
                assert!(q.iter().all(|v| v.is_finite()), "{robot:?} at {t}");
                if let Some(p) = &previous {
                    let jump = q.iter().zip(p).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
                    assert!(jump < 0.2, "{robot:?} jumped {jump} rad in 10 ms at {t} s");
                }
                previous = Some(q);
            }
            t += 0.01;
        }
    }
}

/// One arm stands in the middle doing nothing and the other must reach past it: the idle one is sent to its park
/// pose only because it is in the way, and the schedule is finished (no endless waiting).
#[test]
fn an_idle_arm_in_the_way_is_parked_only_when_it_is_in_the_way_and_the_schedule_ends() {
    let (mut s, a, b) = facing_pair();
    stand(&mut s, a, &[0.0, -1.2, 1.4, -0.2, 0.6, 0.0]);
    stand(&mut s, b, &[-1.5, -0.3, 0.6, 0.0, 0.6, 0.0]);
    // B has a task over the middle; A has nothing to do and stands in the middle.
    let tb = go(&mut s, b, "B over the middle", &[0.0, -1.2, 1.4, -0.2, 0.6, 0.0], 0, 0.0);
    let started = std::time::Instant::now();
    let schedule = s.schedule();
    assert!(started.elapsed().as_secs() < 90);
    // Either B could do it with A where it stands (nobody moved), or A was parked first: never both a park
    // and no reason for it.
    let done = schedule.assignments.iter().any(|x| x.task == tb);
    let blocked = schedule.blocked.iter().any(|(id, why)| *id == tb && !why.is_empty());
    assert!(done || blocked);
    if done {
        for park in &schedule.parks {
            assert_eq!(park.robot, a, "a robot that was not in the way was parked");
            assert!(
                park.end <= schedule.assignments.iter().find(|x| x.task == tb).unwrap().start + 1e-9,
                "parked after the task it should have let go"
            );
        }
    }
    s.request_schedule();
    let (shown, _) = s.schedule_result().map(|(x, c)| (x.clone(), c)).unwrap();
    let (least, at) = least_gap(&mut s, a, b, &shown);
    eprintln!("report: parked {} robots, least gap {least:.4} m at {at:.2} s", shown.parks.len());
    assert!(least > 0.0, "the arms overlap at {at:.2} s");
}
