//! The planners' random numbers: xoshiro256++ seeded through SplitMix64. Own code, so that results never
//! change with a dependency upgrade.

/// A deterministic pseudo-random generator.
#[derive(Clone, Debug)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// A generator whose whole sequence is determined by `seed`.
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            x ^ (x >> 31)
        };
        Self { state: [next(), next(), next(), next()] }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.state;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Uniform in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the product is below n")]
        let index = (self.unit() * n as f64) as usize;
        index.min(n - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sequence_depends_only_on_the_seed() {
        let (mut a, mut b, mut c) = (Rng::new(7), Rng::new(7), Rng::new(8));
        let first: Vec<u64> = (0..5).map(|_| a.next_u64()).collect();
        assert_eq!(first, (0..5).map(|_| b.next_u64()).collect::<Vec<_>>());
        assert_ne!(first, (0..5).map(|_| c.next_u64()).collect::<Vec<_>>());
        // Reference values: xoshiro256++ seeded with SplitMix64(0).
        let mut zero = Rng::new(0);
        assert_eq!(zero.next_u64(), 0x53175d61490b23df);
    }

    #[test]
    fn samples_stay_in_range_and_cover_it() {
        let mut rng = Rng::new(1);
        let mut buckets = [0_u32; 10];
        for _ in 0..10_000 {
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u));
            buckets[rng.below(10)] += 1;
        }
        assert!(buckets.iter().all(|count| (800..1200).contains(count)), "{buckets:?}");
        assert_eq!(rng.below(0), 0);
    }
}
