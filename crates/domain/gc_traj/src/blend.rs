//! Motion through waypoints without stopping: corners rounded by blends, timed by reachability analysis
//! (TOPP-RA).
//!
//! The geometric path is straight segments joined by parabolic blends, so its direction changes
//! continuously. The time law is found on a grid along the path. Going backwards from the end, each grid
//! point gets the highest speed from which the motion can still come to rest within the limits; going
//! forwards from rest, the motion then accelerates as hard as the limits and those speeds allow.

use crate::{Limits, Sample, TimingError};

/// On blends the path's direction changes within a grid step; the limits are tightened by this factor
/// there to absorb what the grid does not see.
const BLEND_SAFETY: f64 = 0.95;
/// Grid steps along the whole path, before the per-piece minimums.
const GRID: f64 = 300.0;

#[derive(Clone, Debug)]
enum Piece {
    /// A straight piece: `from + dir * s`.
    Line { from: Vec<f64>, dir: Vec<f64> },
    /// A parabolic blend round the corner `p`, from `a` to `c`: a quadratic Bézier curve.
    Blend { a: Vec<f64>, p: Vec<f64>, c: Vec<f64> },
}

/// Straight pieces and blends, parameterised by `s` from 0 to `length`.
#[derive(Clone, Debug)]
pub(crate) struct BlendedPath {
    pieces: Vec<Piece>,
    /// Where each piece starts; one more entry than pieces.
    starts: Vec<f64>,
    /// Corners that could not be blended: the motion stops there.
    stops: Vec<f64>,
    /// The path parameter of each original waypoint.
    pub(crate) waypoint_s: Vec<f64>,
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

impl BlendedPath {
    /// Builds the path through `points`. A blend stays within `deviation` of its corner (Euclidean
    /// distance in joint space) and uses at most half of each neighbouring segment. A blend with a state
    /// `state_valid` rejects is halved up to four times and then left out, which makes that corner a stop.
    pub(crate) fn new(points: &[Vec<f64>], deviation: f64, state_valid: &dyn Fn(&[f64]) -> bool) -> Self {
        // Segments of zero length carry no direction: drop repeated points.
        let mut corners: Vec<&Vec<f64>> = Vec::new();
        let mut corner_of = Vec::with_capacity(points.len());
        for point in points {
            if corners.last().is_none_or(|last| norm(&sub(point, last)) > 1e-12) {
                corners.push(point);
            }
            corner_of.push(corners.len() - 1);
        }
        let count = corners.len();
        let lengths: Vec<f64> = corners.windows(2).map(|w| norm(&sub(w[1], w[0]))).collect();
        let dirs: Vec<Vec<f64>> =
            corners.windows(2).zip(&lengths).map(|(w, len)| scale(&sub(w[1], w[0]), 1.0 / len)).collect();
        // The blend length on each side of every interior corner.
        let mut reach = vec![0.0; count];
        let mut hard = vec![false; count];
        for k in 1..count.saturating_sub(1) {
            let turn = norm(&sub(&dirs[k], &dirs[k - 1]));
            if turn < 1e-9 {
                continue; // straight on: nothing to round
            }
            let mut length = (4.0 * deviation / turn).min(0.5 * lengths[k - 1].min(lengths[k]));
            let mut accepted = false;
            for _ in 0..5 {
                let blend = Piece::Blend {
                    a: add(corners[k], &scale(&dirs[k - 1], -length)),
                    p: corners[k].clone(),
                    c: add(corners[k], &scale(&dirs[k], length)),
                };
                let checks = ((2.0 * length / 0.01).ceil() as usize).clamp(4, 400);
                if (0..=checks).all(|i| state_valid(&blend.eval(2.0 * length * i as f64 / checks as f64).0)) {
                    accepted = true;
                    break;
                }
                length *= 0.5;
            }
            if accepted {
                reach[k] = length;
            } else {
                hard[k] = true;
            }
        }
        let mut path =
            Self { pieces: Vec::new(), starts: vec![0.0], stops: Vec::new(), waypoint_s: vec![0.0] };
        for k in 0..count.saturating_sub(1) {
            let line = lengths[k] - reach[k] - reach[k + 1];
            if line > 1e-12 {
                let from = add(corners[k], &scale(&dirs[k], reach[k]));
                path.push(Piece::Line { from, dir: dirs[k].clone() }, line);
            }
            let end = path.length();
            if k + 2 < count {
                let length = reach[k + 1];
                if length > 0.0 {
                    let corner = corners[k + 1];
                    path.push(
                        Piece::Blend {
                            a: add(corner, &scale(&dirs[k], -length)),
                            p: corner.clone(),
                            c: add(corner, &scale(&dirs[k + 1], length)),
                        },
                        2.0 * length,
                    );
                    path.waypoint_s.push(end + length);
                } else {
                    if hard[k + 1] {
                        path.stops.push(end);
                    }
                    path.waypoint_s.push(end);
                }
            } else {
                path.waypoint_s.push(end);
            }
        }
        // A repeated point shares the place of the point it repeats.
        path.waypoint_s =
            corner_of.iter().map(|corner| path.waypoint_s.get(*corner).copied().unwrap_or(0.0)).collect();
        path
    }

    fn push(&mut self, piece: Piece, length: f64) {
        self.pieces.push(piece);
        self.starts.push(self.length() + length);
    }

    pub(crate) fn length(&self) -> f64 {
        self.starts.last().copied().unwrap_or(0.0)
    }

    /// Position and its first and second derivative with respect to `s`.
    pub(crate) fn eval(&self, s: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let s = s.clamp(0.0, self.length());
        let index =
            self.starts.partition_point(|start| *start <= s).saturating_sub(1).min(self.pieces.len() - 1);
        self.pieces[index].eval(s - self.starts[index])
    }
}

impl Piece {
    fn eval(&self, s: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        match self {
            Self::Line { from, dir } => (add(from, &scale(dir, s)), dir.clone(), vec![0.0; dir.len()]),
            Self::Blend { a, p, c } => {
                // The curve parameter runs from 0 to 1 while s runs over the blend's length, which is
                // twice the distance from the corner to either end.
                let length = 2.0 * norm(&sub(p, a));
                let t = (s / length).clamp(0.0, 1.0);
                let (into, out) = (sub(p, a), sub(c, p));
                let q = (0..a.len())
                    .map(|k| (1.0 - t) * (1.0 - t) * a[k] + 2.0 * t * (1.0 - t) * p[k] + t * t * c[k])
                    .collect();
                let q1 = (0..a.len()).map(|k| 2.0 * ((1.0 - t) * into[k] + t * out[k]) / length).collect();
                let q2 = (0..a.len()).map(|k| 2.0 * (out[k] - into[k]) / (length * length)).collect();
                (q, q1, q2)
            }
        }
    }
}

fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

fn add(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

fn scale(a: &[f64], k: f64) -> Vec<f64> {
    a.iter().map(|x| x * k).collect()
}

/// One grid step of the time law: from `s` with squared speed `x`, at constant path acceleration `u`.
#[derive(Clone, Copy, Debug)]
struct Stage {
    s: f64,
    x: f64,
    u: f64,
    t: f64,
}

/// A blended path with its time law.
#[derive(Clone, Debug)]
pub(crate) struct Timed {
    path: BlendedPath,
    stages: Vec<Stage>,
    duration: f64,
}

/// A constraint `a*x + b*u <= c` on the squared speed `x` and the path acceleration `u` of one stage.
type Row = (f64, f64, f64);

/// The rows that keep one stage within the limits. `step` is the stage's length in `s`.
fn stage_rows(q1: &[f64], q2: &[f64], limits: &Limits, safety: f64, step: f64, rows: &mut Vec<Row>) {
    rows.clear();
    for joint in 0..q1.len() {
        let limit = limits.max_acc[joint] * safety;
        // The joint's acceleration q1*u + q2*x at the start of the stage, and at its end, where the
        // squared speed has become x + 2*step*u.
        for (a, b) in [(q2[joint], q1[joint]), (q2[joint], q1[joint] + 2.0 * step * q2[joint])] {
            rows.push((a, b, limit));
            rows.push((-a, -b, limit));
        }
    }
}

/// The highest squared speed a joint speed limit allows where the path's direction is `q1`.
fn speed_cap(q1: &[f64], limits: &Limits, safety: f64) -> f64 {
    q1.iter()
        .zip(&limits.max_vel)
        .filter(|(d, _)| d.abs() > 1e-12)
        .map(|(d, v)| (v * safety / d.abs()).powi(2))
        .fold(1e12, f64::min)
}

/// The largest `x` with all rows satisfied, found among the corners of the feasible polygon.
fn max_x(rows: &[Row]) -> Option<f64> {
    let mut best: Option<f64> = None;
    for (i, first) in rows.iter().enumerate() {
        for second in &rows[i + 1..] {
            let det = first.0 * second.1 - first.1 * second.0;
            if det.abs() < 1e-14 {
                continue;
            }
            let x = (first.2 * second.1 - first.1 * second.2) / det;
            let u = (first.0 * second.2 - first.2 * second.0) / det;
            if best.is_some_and(|b| x <= b) {
                continue;
            }
            let slack = |row: &Row| row.2 - row.0 * x - row.1 * u;
            if rows.iter().all(|row| slack(row) >= -1e-9 * (1.0 + row.2.abs())) {
                best = Some(x);
            }
        }
    }
    best
}

impl Timed {
    /// Times the path within the limits, starting and ending at rest.
    pub(crate) fn new(path: BlendedPath, limits: &Limits) -> Result<Self, TimingError> {
        let dof = path.pieces.first().map_or(0, |piece| piece.eval(0.0).0.len());
        if dof == 0 || limits.max_vel.len() < dof || limits.max_acc.len() < dof {
            return Err(TimingError::BadPath);
        }
        for joint in 0..dof {
            let (v, a) = (limits.max_vel[joint], limits.max_acc[joint]);
            if !(v.is_finite() && v > 0.0 && a.is_finite() && a > 0.0) {
                return Err(TimingError::BadLimit(joint));
            }
        }
        // The grid: every piece is cut into equal steps, blends more finely than lines.
        let target = path.length() / GRID;
        let mut grid = vec![0.0];
        let mut on_blend = Vec::new();
        for (index, piece) in path.pieces.iter().enumerate() {
            let (start, end) = (path.starts[index], path.starts[index + 1]);
            let blend = matches!(piece, Piece::Blend { .. });
            let steps = (((end - start) / target).ceil() as usize).clamp(if blend { 24 } else { 1 }, 2000);
            for k in 1..=steps {
                grid.push(start + (end - start) * k as f64 / steps as f64);
                on_blend.push(blend);
            }
        }
        let n = grid.len() - 1;
        let safety = |stage: usize| if on_blend[stage] { BLEND_SAFETY } else { 1.0 };
        // The speed cap at every grid point: what the joint speed limits allow, zero at forced stops.
        let cap: Vec<f64> = (0..=n)
            .map(|i| {
                if i == 0 || i == n || path.stops.iter().any(|stop| (stop - grid[i]).abs() < 1e-9) {
                    return 0.0;
                }
                let tight = if on_blend[i - 1] || on_blend[i.min(n - 1)] { BLEND_SAFETY } else { 1.0 };
                speed_cap(&path.eval(grid[i]).1, limits, tight)
            })
            .collect();

        // Backwards: the highest squared speed at each grid point from which the rest can be driven.
        let mut reach = vec![0.0; n + 1];
        let mut rows = Vec::new();
        let middle = |i: usize| path.eval(0.5 * (grid[i] + grid[i + 1]));
        for i in (0..n).rev() {
            let step = grid[i + 1] - grid[i];
            let (_, q1, q2) = middle(i);
            stage_rows(&q1, &q2, limits, safety(i), step, &mut rows);
            rows.extend([
                (1.0, 0.0, cap[i]),
                (-1.0, 0.0, 0.0),
                (1.0, 2.0 * step, reach[i + 1]),
                (-1.0, -2.0 * step, 0.0),
            ]);
            // Kept a hair below the corner the search found, so that rounding in the search never asks the
            // forward pass for a speed it cannot hold.
            let highest = max_x(&rows).ok_or(TimingError::Infeasible)?;
            reach[i] = (highest * (1.0 - 1e-7) - 1e-12).clamp(0.0, cap[i]);
        }

        // Forwards: from rest, the hardest acceleration that stays within the limits and those speeds.
        let mut stages = Vec::with_capacity(n + 1);
        let (mut x, mut t) = (0.0_f64, 0.0_f64);
        for i in 0..n {
            let step = grid[i + 1] - grid[i];
            let (_, q1, q2) = middle(i);
            stage_rows(&q1, &q2, limits, safety(i), step, &mut rows);
            let (mut low, mut high) = (-x / (2.0 * step), (reach[i + 1] - x) / (2.0 * step));
            for (a, b, c) in &rows {
                // a*x + b*u <= c with x known.
                let room = c - a * x;
                if b.abs() < 1e-14 {
                    continue;
                } else if *b > 0.0 {
                    high = high.min(room / b);
                } else {
                    low = low.max(room / b);
                }
            }
            if low > high + 1e-6 * (1.0 + high.abs()) {
                return Err(TimingError::Infeasible);
            }
            // A gap within rounding is closed in the middle.
            let u = if low > high { 0.5 * (low + high) } else { high };
            let next = (x + 2.0 * step * u).max(0.0);
            let speeds = x.sqrt() + next.sqrt();
            if speeds < 1e-12 {
                return Err(TimingError::Infeasible);
            }
            stages.push(Stage { s: grid[i], x, u, t });
            t += 2.0 * step / speeds;
            x = next;
        }
        stages.push(Stage { s: grid[n], x: 0.0, u: 0.0, t });
        Ok(Self { path, stages, duration: t })
    }

    pub(crate) fn duration(&self) -> f64 {
        self.duration
    }

    /// When the motion passes path parameter `s`.
    pub(crate) fn time_at(&self, s: f64) -> f64 {
        let index = self.stages.partition_point(|stage| stage.s <= s).saturating_sub(1);
        let stage = self.stages[index];
        let ds = (s - stage.s).max(0.0);
        // ds = v*tau + u*tau^2/2, solved for tau without cancellation.
        let v = stage.x.sqrt();
        let root = (stage.x + 2.0 * stage.u * ds).max(0.0).sqrt();
        stage.t + if v + root > 1e-12 { 2.0 * ds / (v + root) } else { 0.0 }
    }

    pub(crate) fn sample(&self, t: f64) -> Sample {
        let t = t.clamp(0.0, self.duration);
        let index =
            self.stages.partition_point(|stage| stage.t <= t).saturating_sub(1).min(self.stages.len() - 1);
        let stage = self.stages[index];
        let tau = t - stage.t;
        let speed = (stage.x.sqrt() + stage.u * tau).max(0.0);
        let s = stage.s + stage.x.sqrt() * tau + 0.5 * stage.u * tau * tau;
        let (q, q1, q2) = self.path.eval(s);
        Sample {
            qd: q1.iter().map(|d| d * speed).collect(),
            qdd: q1.iter().zip(&q2).map(|(d, dd)| dd * speed * speed + d * stage.u).collect(),
            q,
        }
    }
}
