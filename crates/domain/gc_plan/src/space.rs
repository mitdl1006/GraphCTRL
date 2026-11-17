//! The configuration space: a weighted box.

use crate::rng::Rng;

/// A box-shaped space with a weighted Euclidean metric.
///
/// Bounds may be infinite (a joint that turns without limit); sampling then needs a finite window, which
/// [`Space::window`] derives from the states that matter.
#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    /// Lower bound per dimension.
    pub lo: Vec<f64>,
    /// Upper bound per dimension.
    pub hi: Vec<f64>,
    /// Weight per dimension: a unit step in a dimension with weight 2 costs twice as much.
    pub weights: Vec<f64>,
}

impl Space {
    /// Number of dimensions.
    pub fn dof(&self) -> usize {
        self.lo.len()
    }

    /// True when the description is usable: equal lengths, ordered bounds, positive finite weights.
    pub fn is_consistent(&self) -> bool {
        let n = self.lo.len();
        n > 0
            && self.hi.len() == n
            && self.weights.len() == n
            && self.lo.iter().zip(&self.hi).all(|(lo, hi)| lo <= hi)
            && self.weights.iter().all(|w| w.is_finite() && *w > 0.0)
    }

    /// True when the state lies inside the bounds.
    pub fn contains(&self, q: &[f64]) -> bool {
        q.len() == self.dof()
            && q.iter().zip(self.lo.iter().zip(&self.hi)).all(|(v, (lo, hi))| v >= lo && v <= hi)
    }

    /// Weighted Euclidean distance.
    pub fn distance(&self, a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).zip(&self.weights).map(|((x, y), w)| (w * (x - y)).powi(2)).sum::<f64>().sqrt()
    }

    /// The point a fraction `t` of the way from `a` to `b`.
    pub fn interpolate(&self, a: &[f64], b: &[f64], t: f64) -> Vec<f64> {
        a.iter().zip(b).map(|(x, y)| x + (y - x) * t).collect()
    }

    /// The point reached from `from` towards `to` without moving any dimension by more than `range`.
    /// Returns the point and whether it is `to` itself.
    pub fn steer(&self, from: &[f64], to: &[f64], range: f64) -> (Vec<f64>, bool) {
        let largest = from.iter().zip(to).fold(0.0_f64, |acc, (x, y)| acc.max((y - x).abs()));
        if largest <= range {
            (to.to_vec(), true)
        } else {
            (self.interpolate(from, to, range / largest), false)
        }
    }

    /// The part of the space to sample: the bounding box of the given states grown by `margin` in every
    /// dimension, cut to the bounds. Keeps a search near the start and the goals, so that a joint with a
    /// range of two turns is not sent the long way round.
    pub fn window<'a>(&self, states: impl IntoIterator<Item = &'a [f64]>, margin: f64) -> Self {
        let mut lo = vec![f64::INFINITY; self.dof()];
        let mut hi = vec![f64::NEG_INFINITY; self.dof()];
        for state in states {
            for (k, value) in state.iter().enumerate().take(self.dof()) {
                lo[k] = lo[k].min(*value);
                hi[k] = hi[k].max(*value);
            }
        }
        for k in 0..self.dof() {
            lo[k] = (lo[k] - margin).max(self.lo[k]);
            hi[k] = (hi[k] + margin).min(self.hi[k]);
        }
        Self { lo, hi, weights: self.weights.clone() }
    }

    /// A uniform sample. Dimensions with an infinite bound sample within one turn of zero.
    pub fn sample(&self, rng: &mut Rng) -> Vec<f64> {
        self.lo
            .iter()
            .zip(&self.hi)
            .map(|(lo, hi)| {
                let lo = if lo.is_finite() { *lo } else { -std::f64::consts::PI };
                let hi = if hi.is_finite() { *hi } else { std::f64::consts::PI };
                lo + (hi - lo) * rng.unit()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Space {
        Space { lo: vec![-1.0, -1.0], hi: vec![1.0, 1.0], weights: vec![1.0, 2.0] }
    }

    #[test]
    fn metric_interpolation_and_steering() {
        let s = square();
        assert!(s.is_consistent());
        assert_eq!(s.distance(&[0.0, 0.0], &[3.0, 2.0]), 5.0);
        assert_eq!(s.interpolate(&[0.0, 0.0], &[1.0, -1.0], 0.25), vec![0.25, -0.25]);
        assert_eq!(s.steer(&[0.0, 0.0], &[0.3, -0.1], 0.5), (vec![0.3, -0.1], true));
        let (stepped, reached) = s.steer(&[0.0, 0.0], &[2.0, -1.0], 0.5);
        assert_eq!((stepped, reached), (vec![0.5, -0.25], false));
        assert!(s.contains(&[1.0, -1.0]) && !s.contains(&[1.0, -1.1]) && !s.contains(&[0.0]));
    }

    #[test]
    fn the_window_hugs_the_given_states_within_the_bounds() {
        let wide =
            Space { lo: vec![-6.0, f64::NEG_INFINITY], hi: vec![6.0, f64::INFINITY], weights: vec![1.0; 2] };
        let window = wide.window([[5.0, 10.0].as_slice(), &[5.5, 12.0]], 1.0);
        assert_eq!(window.lo, vec![4.0, 9.0]);
        assert_eq!(window.hi, vec![6.0, 13.0]);
        let mut rng = Rng::new(3);
        for _ in 0..100 {
            assert!(window.contains(&window.sample(&mut rng)));
            assert!(wide.sample(&mut rng)[1].abs() <= std::f64::consts::PI);
        }
        assert!(!Space { lo: vec![1.0], hi: vec![0.0], weights: vec![1.0] }.is_consistent());
    }
}
