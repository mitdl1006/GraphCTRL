//! Putting time on a joint-space path.
//!
//! This first timer moves from waypoint to waypoint with a synchronised trapezoidal speed profile and
//! comes to rest at every waypoint. All joints start and stop together; the slowest joint (relative to
//! its limits) sets the pace.
//!
//! A trajectory is born [`Unvalidated`]. Only [`Trajectory::validate`], which samples it densely against
//! the limits and a state checker, turns it into a [`Validated`] one, and only validated trajectories are
//! accepted for display and export. The type system keeps the two apart.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod blend;

use std::marker::PhantomData;

/// Speed and acceleration limits per joint (rad/s and rad/s², or m/s and m/s²).
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// Maximum speed per joint.
    pub max_vel: Vec<f64>,
    /// Maximum acceleration per joint.
    pub max_acc: Vec<f64>,
}

/// Why a path could not be timed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TimingError {
    /// The path has no states, or states of differing length.
    #[error("the path is empty or its states differ in length")]
    BadPath,
    /// The limits do not match the path, or a limit is not a positive number.
    #[error("joint {0} has no usable speed or acceleration limit")]
    BadLimit(usize),
    /// No time law within the limits was found (a numerical failure; the caller falls back to stopping
    /// at every waypoint).
    #[error("the path could not be timed within the limits")]
    Infeasible,
}

/// Why a trajectory failed validation.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ValidationError {
    /// A joint exceeds its speed limit.
    #[error("joint {joint} exceeds its speed limit at {time:.3} s")]
    Speed {
        /// Index of the joint.
        joint: usize,
        /// When.
        time: f64,
    },
    /// A joint exceeds its acceleration limit.
    #[error("joint {joint} exceeds its acceleration limit at {time:.3} s")]
    Acceleration {
        /// Index of the joint.
        joint: usize,
        /// When.
        time: f64,
    },
    /// The state checker rejected a sampled state (collision, joint limit).
    #[error("the state at {time:.3} s is not valid")]
    State {
        /// When.
        time: f64,
    },
}

/// A trapezoidal (or, for short moves, triangular) law for the path fraction `s` going from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trapezoid {
    /// Total time.
    pub duration: f64,
    /// Time spent accelerating (and, at the end, decelerating).
    pub ramp: f64,
    /// Acceleration of `s` during the ramps.
    pub accel: f64,
}

impl Trapezoid {
    /// The fastest synchronised profile for a move by `delta` within the limits.
    /// A move of zero length takes no time.
    pub fn for_move(delta: &[f64], limits: &Limits) -> Result<Self, TimingError> {
        let mut speed = f64::INFINITY; // of s, per second
        let mut accel = f64::INFINITY;
        for (joint, d) in delta.iter().enumerate() {
            let (Some(v), Some(a)) = (limits.max_vel.get(joint), limits.max_acc.get(joint)) else {
                return Err(TimingError::BadLimit(joint));
            };
            if !(v.is_finite() && *v > 0.0 && a.is_finite() && *a > 0.0) {
                return Err(TimingError::BadLimit(joint));
            }
            if d.abs() > 0.0 {
                speed = speed.min(v / d.abs());
                accel = accel.min(a / d.abs());
            }
        }
        if !speed.is_finite() {
            return Ok(Self { duration: 0.0, ramp: 0.0, accel: 0.0 });
        }
        if speed * speed <= accel {
            // Accelerate to cruising speed, cruise, decelerate.
            Ok(Self { duration: 1.0 / speed + speed / accel, ramp: speed / accel, accel })
        } else {
            // Too short to reach cruising speed: accelerate half the way, decelerate the other half.
            let ramp = (1.0 / accel).sqrt();
            Ok(Self { duration: 2.0 * ramp, ramp, accel })
        }
    }

    /// Path fraction, its speed and its acceleration at time `t` (clamped to the profile's duration).
    pub fn at(&self, t: f64) -> (f64, f64, f64) {
        if self.duration <= 0.0 {
            return (1.0, 0.0, 0.0);
        }
        let t = t.clamp(0.0, self.duration);
        let peak = self.accel * self.ramp;
        if t < self.ramp {
            (0.5 * self.accel * t * t, self.accel * t, self.accel)
        } else if t <= self.duration - self.ramp {
            (0.5 * self.accel * self.ramp * self.ramp + peak * (t - self.ramp), peak, 0.0)
        } else {
            let left = self.duration - t;
            (1.0 - 0.5 * self.accel * left * left, self.accel * left, -self.accel)
        }
    }
}

/// Marker: the trajectory has not been checked.
#[derive(Clone, Copy, Debug)]
pub struct Unvalidated;
/// Marker: the trajectory passed [`Trajectory::validate`].
#[derive(Clone, Copy, Debug)]
pub struct Validated;

/// One sampled instant of a trajectory.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// Joint positions.
    pub q: Vec<f64>,
    /// Joint velocities.
    pub qd: Vec<f64>,
    /// Joint accelerations.
    pub qdd: Vec<f64>,
}

/// A path with a time law. `State` is [`Unvalidated`] or [`Validated`].
#[derive(Clone, Debug)]
pub struct Trajectory<State> {
    waypoints: Vec<Vec<f64>>,
    profiles: Vec<Trapezoid>,
    /// Start time of each segment, plus the total duration at the end.
    times: Vec<f64>,
    /// The motion when it passes the waypoints on blends instead of stopping at them.
    blended: Option<blend::Timed>,
    /// How much slower than computed the motion is driven (1 = as computed).
    scale: f64,
    state: PhantomData<State>,
}

impl Trajectory<Unvalidated> {
    /// Times a path so that the motion stops at every waypoint.
    pub fn stopping_at_waypoints(path: &[Vec<f64>], limits: &Limits) -> Result<Self, TimingError> {
        let dof = path.first().map_or(0, Vec::len);
        if dof == 0 || path.iter().any(|q| q.len() != dof || q.iter().any(|v| !v.is_finite())) {
            return Err(TimingError::BadPath);
        }
        let mut profiles = Vec::with_capacity(path.len().saturating_sub(1));
        let mut times = vec![0.0];
        for pair in path.windows(2) {
            let delta: Vec<f64> = pair[0].iter().zip(&pair[1]).map(|(a, b)| b - a).collect();
            let profile = Trapezoid::for_move(&delta, limits)?;
            times.push(times[times.len() - 1] + profile.duration);
            profiles.push(profile);
        }
        Ok(Self { waypoints: path.to_vec(), profiles, times, blended: None, scale: 1.0, state: PhantomData })
    }

    /// Times a motion that passes the interior waypoints without stopping: every corner is rounded by a
    /// blend that stays within `deviation` of it (Euclidean distance in joint space), and the rounded
    /// path is driven as fast as the limits allow. `state_valid` is asked about the states on the blends;
    /// a corner whose blend it rejects is driven as a stop instead.
    pub fn blended(
        path: &[Vec<f64>],
        limits: &Limits,
        deviation: f64,
        state_valid: &dyn Fn(&[f64]) -> bool,
    ) -> Result<Self, TimingError> {
        let dof = path.first().map_or(0, Vec::len);
        if dof == 0 || path.iter().any(|q| q.len() != dof || q.iter().any(|v| !v.is_finite())) {
            return Err(TimingError::BadPath);
        }
        let geometry = blend::BlendedPath::new(path, deviation.max(0.0), state_valid);
        if geometry.length() <= 0.0 {
            return Self::stopping_at_waypoints(path, limits);
        }
        let waypoint_s = geometry.waypoint_s.clone();
        let timed = blend::Timed::new(geometry, limits)?;
        let times = waypoint_s.iter().map(|s| timed.time_at(*s)).collect();
        Ok(Self {
            waypoints: path.to_vec(),
            profiles: Vec::new(),
            times,
            blended: Some(timed),
            scale: 1.0,
            state: PhantomData,
        })
    }

    /// Samples the trajectory every `step` seconds (and at every waypoint) and checks speed and
    /// acceleration limits and `state_valid`. Only a trajectory that passes becomes [`Validated`].
    pub fn validate(
        self,
        limits: &Limits,
        step: f64,
        state_valid: &dyn Fn(&[f64]) -> bool,
    ) -> Result<Trajectory<Validated>, ValidationError> {
        let step = if step.is_finite() && step > 0.0 { step } else { 0.002 };
        let mut time = 0.0_f64;
        loop {
            let t = time.min(self.duration());
            let sample = self.sample(t);
            for joint in 0..sample.q.len() {
                let (v, a) = (
                    limits.max_vel.get(joint).copied().unwrap_or(0.0),
                    limits.max_acc.get(joint).copied().unwrap_or(0.0),
                );
                if sample.qd[joint].abs() > v * (1.0 + 1e-9) + 1e-12 {
                    return Err(ValidationError::Speed { joint, time: t });
                }
                if sample.qdd[joint].abs() > a * (1.0 + 1e-9) + 1e-12 {
                    return Err(ValidationError::Acceleration { joint, time: t });
                }
            }
            if !state_valid(&sample.q) {
                return Err(ValidationError::State { time: t });
            }
            if t >= self.duration() {
                break;
            }
            time += step;
        }
        Ok(Trajectory {
            waypoints: self.waypoints,
            profiles: self.profiles,
            times: self.times,
            blended: self.blended,
            scale: self.scale,
            state: PhantomData,
        })
    }
}

impl<State> Trajectory<State> {
    /// The same motion driven `factor` times slower (a factor below 1 is taken as 1). Speeds divide by
    /// the factor and accelerations by its square, so a trajectory within its limits stays within them.
    #[must_use]
    pub fn slowed(mut self, factor: f64) -> Self {
        if factor.is_finite() && factor > 1.0 {
            self.scale *= factor;
        }
        self
    }

    /// Total time in seconds.
    pub fn duration(&self) -> f64 {
        self.scale * self.computed_duration()
    }

    /// Total time as computed, before any slowing down.
    fn computed_duration(&self) -> f64 {
        match &self.blended {
            Some(timed) => timed.duration(),
            None => self.times.last().copied().unwrap_or(0.0),
        }
    }

    /// True when the motion passes the interior waypoints on blends instead of stopping at them.
    pub fn is_blended(&self) -> bool {
        self.blended.is_some()
    }

    /// The waypoints the trajectory passes through.
    pub fn waypoints(&self) -> &[Vec<f64>] {
        &self.waypoints
    }

    /// The time at which each waypoint is reached.
    pub fn waypoint_times(&self) -> Vec<f64> {
        self.times.iter().map(|t| t * self.scale).collect()
    }

    /// Position, velocity and acceleration at time `t` (clamped to the trajectory).
    pub fn sample(&self, t: f64) -> Sample {
        let k = self.scale;
        let mut sample = self.computed_sample(t / k);
        sample.qd.iter_mut().for_each(|v| *v /= k);
        sample.qdd.iter_mut().for_each(|a| *a /= k * k);
        sample
    }

    /// The sample at time `t` of the motion as computed.
    fn computed_sample(&self, t: f64) -> Sample {
        if let Some(timed) = &self.blended {
            // The ends are the given waypoints exactly, not what the path's arithmetic arrives at.
            let end = if t <= 0.0 {
                self.waypoints.first()
            } else if t >= timed.duration() {
                self.waypoints.last()
            } else {
                None
            };
            return match end {
                Some(q) => Sample { q: q.clone(), qd: vec![0.0; q.len()], qdd: vec![0.0; q.len()] },
                None => timed.sample(t),
            };
        }
        let t = t.clamp(0.0, self.computed_duration());
        // The segment that contains t: the last one starting at or before it.
        let segment = self
            .times
            .partition_point(|start| *start <= t)
            .saturating_sub(1)
            .min(self.profiles.len().saturating_sub(1));
        let Some(profile) = self.profiles.get(segment) else {
            let q = self.waypoints.first().cloned().unwrap_or_default();
            return Sample { qd: vec![0.0; q.len()], qdd: vec![0.0; q.len()], q };
        };
        let (s, ds, dds) = profile.at(t - self.times[segment]);
        let (from, to) = (&self.waypoints[segment], &self.waypoints[segment + 1]);
        let delta = |k: usize| to[k] - from[k];
        let dof = from.len();
        Sample {
            q: (0..dof).map(|k| from[k] + delta(k) * s).collect(),
            qd: (0..dof).map(|k| delta(k) * ds).collect(),
            qdd: (0..dof).map(|k| delta(k) * dds).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn limits(v: &[f64], a: f64) -> Limits {
        Limits { max_vel: v.to_vec(), max_acc: vec![a; v.len()] }
    }

    fn duration(delta: &[f64], limits: &Limits) -> f64 {
        Trapezoid::for_move(delta, limits).unwrap().duration
    }

    #[test]
    fn durations_match_the_closed_form_values() {
        // UR5e: 180 deg/s on every joint, 5 rad/s^2.
        let ur5e = limits(&[PI; 6], 5.0);
        assert!((duration(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0], &ur5e) - 0.894_427).abs() < 1e-6);
        assert!((duration(&[PI, 0.0, 0.0, 0.0, 0.0, 0.0], &ur5e) - 1.628_319).abs() < 1e-6);
        // The controller's default speed and acceleration.
        let urscript = limits(&[1.05; 6], 1.4);
        assert!((duration(&[PI / 2.0, 0.0, 0.0, 0.0, 0.0, 0.0], &urscript) - 2.245_997).abs() < 1e-6);
        assert!((duration(&[0.5, 0.0, 0.0, 0.0, 0.0, 0.0], &urscript) - 1.195_229).abs() < 1e-6);
        // UR10e: slower shoulder joints. The joint that is slowest relative to its limit sets the pace.
        let slow = 120.0_f64.to_radians();
        let ur10e = limits(&[slow, slow, PI, PI, PI, PI], 5.0);
        assert!((duration(&[1.0, 0.0, 2.0, 0.0, 0.0, 0.0], &ur10e) - 1.264_938).abs() < 1e-6);
        assert!((duration(&[2.0, 0.0, 1.0, 0.0, 0.0, 0.0], &ur10e) - 1.373_809).abs() < 1e-6);
        assert_eq!(duration(&[0.0; 6], &ur5e), 0.0);
    }

    #[test]
    fn the_profile_starts_and_ends_at_rest_and_respects_its_limits() {
        for delta in [0.3, 1.0, 4.0] {
            let limits = limits(&[2.0], 5.0);
            let profile = Trapezoid::for_move(&[delta], &limits).unwrap();
            let (s0, v0, _) = profile.at(0.0);
            let (s1, v1, _) = profile.at(profile.duration);
            assert_eq!((s0, v0), (0.0, 0.0));
            assert!((s1 - 1.0).abs() < 1e-12 && v1.abs() < 1e-12);
            let mut previous = 0.0;
            for k in 0..=1000 {
                let (s, ds, dds) = profile.at(profile.duration * f64::from(k) / 1000.0);
                assert!(s >= previous - 1e-12, "the path fraction never goes back");
                assert!(ds * delta <= 2.0 + 1e-9 && (dds * delta).abs() <= 5.0 + 1e-9);
                previous = s;
            }
        }
    }

    #[test]
    fn a_path_is_timed_segment_by_segment_and_sampled_continuously() {
        let limits = limits(&[1.0, 1.0], 2.0);
        let path = vec![vec![0.0, 0.0], vec![1.0, 0.5], vec![1.0, 2.0]];
        let trajectory = Trajectory::stopping_at_waypoints(&path, &limits).unwrap();
        let expected = duration(&[1.0, 0.5], &limits) + duration(&[0.0, 1.5], &limits);
        assert!((trajectory.duration() - expected).abs() < 1e-12);
        assert_eq!(trajectory.sample(0.0).q, path[0]);
        assert_eq!(trajectory.sample(1e9).q, path[2]);
        let at_waypoint = trajectory.sample(trajectory.waypoint_times()[1]);
        assert!((at_waypoint.q[0] - 1.0).abs() < 1e-12 && at_waypoint.qd.iter().all(|v| v.abs() < 1e-12));
        // Positions are continuous: small time steps give small position steps.
        let mut previous = trajectory.sample(0.0).q;
        for k in 1..=2000 {
            let q = trajectory.sample(trajectory.duration() * f64::from(k) / 2000.0).q;
            assert!(q.iter().zip(&previous).all(|(a, b)| (a - b).abs() < 0.01));
            previous = q;
        }
    }

    #[test]
    fn only_validation_produces_a_validated_trajectory() {
        let limits = limits(&[1.0, 1.0], 2.0);
        let path = vec![vec![0.0, 0.0], vec![1.0, 0.5]];
        let trajectory = Trajectory::stopping_at_waypoints(&path, &limits).unwrap();
        let checked: Trajectory<Validated> = trajectory.clone().validate(&limits, 0.002, &|_| true).unwrap();
        assert_eq!(checked.duration(), trajectory.duration());
        // A forbidden region in the middle is found by the sampling.
        let blocked = trajectory.clone().validate(&limits, 0.002, &|q| !(0.4..0.6).contains(&q[0]));
        assert!(matches!(blocked, Err(ValidationError::State { time }) if time > 0.0));
        // Tighter limits than the ones the trajectory was timed for are reported with the joint.
        let tighter = Limits { max_vel: vec![0.5, 1.0], max_acc: vec![2.0, 2.0] };
        assert!(matches!(
            trajectory.validate(&tighter, 0.002, &|_| true),
            Err(ValidationError::Speed { joint: 0, .. })
        ));
    }

    #[test]
    fn bad_input_is_refused() {
        let good = limits(&[1.0], 1.0);
        assert_eq!(Trajectory::stopping_at_waypoints(&[], &good).err(), Some(TimingError::BadPath));
        assert_eq!(
            Trajectory::stopping_at_waypoints(&[vec![0.0], vec![0.0, 1.0]], &good).err(),
            Some(TimingError::BadPath)
        );
        assert_eq!(
            Trajectory::stopping_at_waypoints(&[vec![f64::NAN]], &good).err(),
            Some(TimingError::BadPath)
        );
        let two = vec![vec![0.0, 0.0], vec![1.0, 1.0]];
        assert_eq!(Trajectory::stopping_at_waypoints(&two, &good).err(), Some(TimingError::BadLimit(1)));
        let zero = Limits { max_vel: vec![0.0], max_acc: vec![1.0] };
        assert_eq!(
            Trajectory::stopping_at_waypoints(&[vec![0.0], vec![1.0]], &zero).err(),
            Some(TimingError::BadLimit(0))
        );
        let single = Trajectory::stopping_at_waypoints(&[vec![0.3]], &good).unwrap();
        assert_eq!((single.duration(), single.sample(1.0).q), (0.0, vec![0.3]));
    }
}
