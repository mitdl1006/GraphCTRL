//! Making a found path shorter and smoother, and checking a path independently of how it was made.

use crate::Validity;
use crate::rng::Rng;
use crate::space::Space;

/// Weighted length of a path.
pub fn path_cost(space: &Space, path: &[Vec<f64>]) -> f64 {
    path.windows(2).map(|pair| space.distance(&pair[0], &pair[1])).sum()
}

/// True when every state of the path and every motion between consecutive states is valid.
///
/// Planners and simplifiers are trusted by nobody: whatever they return is checked with this before it
/// is used.
pub fn validate(path: &[Vec<f64>], validity: &dyn Validity) -> bool {
    !path.is_empty()
        && path.iter().all(|q| validity.state_valid(q))
        && path.windows(2).all(|pair| validity.motion_valid(&pair[0], &pair[1]))
}

/// A point on the path: `(segment index, fraction along that segment)`.
fn point_at(space: &Space, path: &[Vec<f64>], at: (usize, f64)) -> Vec<f64> {
    space.interpolate(&path[at.0], &path[at.0 + 1], at.1)
}

/// Tries `attempts` times to replace the stretch between two random points of the path by a straight
/// motion. Returns how many shortcuts were taken. The path stays valid and never gets longer.
pub fn shortcut(
    space: &Space,
    path: &mut Vec<Vec<f64>>,
    validity: &dyn Validity,
    rng: &mut Rng,
    attempts: usize,
) -> usize {
    let mut taken = 0;
    for _ in 0..attempts {
        if path.len() < 3 {
            break;
        }
        let segments = path.len() - 1;
        let mut a = (rng.below(segments), rng.unit());
        let mut b = (rng.below(segments), rng.unit());
        if b < a {
            std::mem::swap(&mut a, &mut b);
        }
        if a.0 == b.0 {
            continue; // both on one segment: already straight
        }
        let (from, to) = (point_at(space, path, a), point_at(space, path, b));
        let old: f64 = space.distance(&from, &path[a.0 + 1])
            + path_cost(space, &path[a.0 + 1..=b.0])
            + space.distance(&path[b.0], &to);
        // The two stubs left of the old segments are checked as well: a part of a valid motion is sampled
        // at other places than the whole and must not be assumed valid.
        if space.distance(&from, &to) >= old - 1e-12
            || !validity.motion_valid(&from, &to)
            || !validity.motion_valid(&path[a.0], &from)
            || !validity.motion_valid(&to, &path[b.0 + 1])
        {
            continue;
        }
        path.splice(a.0 + 1..=b.0, [from, to]);
        path.dedup();
        taken += 1;
    }
    taken
}

/// Tries `attempts` times to straighten one joint over a stretch of the path: between two random
/// waypoints, the chosen joint moves evenly (in proportion to the path length covered) while the other
/// joints keep their values. Removes detours a whole-state shortcut cannot, where one joint wanders while
/// the others are needed to clear an obstacle. Returns how many were taken. The path stays valid and
/// never gets longer.
pub fn partial_shortcut(
    space: &Space,
    path: &mut [Vec<f64>],
    validity: &dyn Validity,
    rng: &mut Rng,
    attempts: usize,
) -> usize {
    let mut taken = 0;
    let dof = space.dof();
    for _ in 0..attempts {
        if path.len() < 3 || dof == 0 {
            break;
        }
        let mut i = rng.below(path.len());
        let mut j = rng.below(path.len());
        if i > j {
            std::mem::swap(&mut i, &mut j);
        }
        let joint = rng.below(dof);
        if j < i + 2 {
            continue;
        }
        // Where each waypoint lies along the stretch, by path length.
        let mut along = vec![0.0; j - i + 1];
        for m in i + 1..=j {
            along[m - i] = along[m - i - 1] + space.distance(&path[m - 1], &path[m]);
        }
        let total = along[j - i];
        if total <= 0.0 {
            continue;
        }
        let (start, end) = (path[i][joint], path[j][joint]);
        let mut trial: Vec<Vec<f64>> = path[i..=j].to_vec();
        for (m, state) in trial.iter_mut().enumerate().take(j - i).skip(1) {
            state[joint] = start + (end - start) * along[m] / total;
        }
        let old = path_cost(space, &path[i..=j]);
        if path_cost(space, &trial) >= old - 1e-12
            || !trial.iter().all(|q| space.contains(q))
            || !trial[1..trial.len() - 1].iter().all(|q| validity.state_valid(q))
            || !trial.windows(2).all(|pair| validity.motion_valid(&pair[0], &pair[1]))
        {
            continue;
        }
        path[i..=j].clone_from_slice(&trial);
        taken += 1;
    }
    taken
}

/// Rounds corners: each interior state is replaced by two states a quarter of the way along its two
/// segments, where the cut is valid. `passes` repetitions make the path progressively rounder.
pub fn smooth_corners(space: &Space, path: &mut Vec<Vec<f64>>, validity: &dyn Validity, passes: usize) {
    for _ in 0..passes {
        if path.len() < 3 {
            return;
        }
        let mut rounded = vec![path[0].clone()];
        for i in 1..path.len() - 1 {
            let before = space.interpolate(&path[i], &path[i - 1], 0.25);
            let after = space.interpolate(&path[i], &path[i + 1], 0.25);
            let previous = &rounded[rounded.len() - 1];
            let accepted = validity.motion_valid(previous, &before)
                && validity.motion_valid(&before, &after)
                && validity.motion_valid(&after, &path[i + 1]);
            if accepted {
                rounded.push(before);
                rounded.push(after);
            } else if validity.motion_valid(previous, &path[i]) {
                rounded.push(path[i].clone());
            } else {
                return; // the previous cut cannot be joined to this corner: keep the path as it was
            }
        }
        rounded.push(path[path.len() - 1].clone());
        *path = rounded;
    }
}

/// Inserts states so that no dimension changes by more than `max_step` between consecutive states.
pub fn densify(space: &Space, path: &[Vec<f64>], max_step: f64) -> Vec<Vec<f64>> {
    let Some(first) = path.first() else { return Vec::new() };
    let mut dense = vec![first.clone()];
    for pair in path.windows(2) {
        let largest = pair[0].iter().zip(&pair[1]).fold(0.0_f64, |acc, (a, b)| acc.max((b - a).abs()));
        let pieces = if max_step > 0.0 { (largest / max_step).ceil().max(1.0) } else { 1.0 };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a small positive count")]
        let count = pieces.min(100_000.0) as usize;
        for k in 1..=count {
            dense.push(space.interpolate(&pair[0], &pair[1], k as f64 / count as f64));
        }
    }
    dense
}
