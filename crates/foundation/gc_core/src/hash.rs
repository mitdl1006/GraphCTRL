//! A hash whose value is the same on every platform, in every run and in every build.
//!
//! `std::hash` makes no such promise, so cache keys and content hashes that end up in files or are
//! compared across runs use this instead. It is not a cryptographic hash.

use crate::{AnyId, Id, Kind};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over the written bytes, with a final avalanche step.
#[derive(Clone, Debug)]
pub struct StableHasher {
    state: u64,
}

impl Default for StableHasher {
    fn default() -> Self {
        Self { state: FNV_OFFSET }
    }
}

impl StableHasher {
    /// A hasher in its initial state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds raw bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state = (self.state ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
        }
    }

    /// Feeds an integer in little-endian byte order.
    pub fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    /// Feeds a float by bit pattern. `-0.0` hashes like `0.0`, and every NaN hashes alike.
    pub fn write_f64(&mut self, value: f64) {
        let canonical = if value == 0.0 {
            0.0
        } else if value.is_nan() {
            f64::NAN
        } else {
            value
        };
        self.write_u64(canonical.to_bits());
    }

    /// The hash of everything written so far.
    pub fn finish(&self) -> u64 {
        // SplitMix64 finaliser: spreads the last bytes over all output bits.
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// Values that can be fed to a [`StableHasher`].
pub trait StableHash {
    /// Feeds this value. Variable-length values write their length first so that sequences cannot collide
    /// by shifting a boundary.
    fn stable_hash(&self, hasher: &mut StableHasher);

    /// Convenience: the hash of this value alone.
    fn stable_hash_value(&self) -> u64 {
        let mut hasher = StableHasher::new();
        self.stable_hash(&mut hasher);
        hasher.finish()
    }
}

macro_rules! hash_as_u64 {
    ($($ty:ty),*) => {$(
        impl StableHash for $ty {
            fn stable_hash(&self, hasher: &mut StableHasher) {
                hasher.write_u64(u64::from(*self));
            }
        }
    )*};
}
hash_as_u64!(u8, u16, u32, u64, bool);

impl StableHash for usize {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        // usize is at most 64 bits on every supported target.
        hasher.write_u64(u64::try_from(*self).unwrap_or(u64::MAX));
    }
}

impl StableHash for i64 {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_bytes(&self.to_le_bytes());
    }
}

impl StableHash for f64 {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_f64(*self);
    }
}

impl StableHash for f32 {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_f64(f64::from(*self));
    }
}

impl StableHash for str {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.len().stable_hash(hasher);
        hasher.write_bytes(self.as_bytes());
    }
}

impl StableHash for String {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_str().stable_hash(hasher);
    }
}

impl<T: StableHash> StableHash for [T] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.len().stable_hash(hasher);
        for item in self {
            item.stable_hash(hasher);
        }
    }
}

impl<T: StableHash> StableHash for Vec<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_slice().stable_hash(hasher);
    }
}

impl<T: StableHash, const N: usize> StableHash for [T; N] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        for item in self {
            item.stable_hash(hasher);
        }
    }
}

impl<T: StableHash> StableHash for Option<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        match self {
            None => false.stable_hash(hasher),
            Some(value) => {
                true.stable_hash(hasher);
                value.stable_hash(hasher);
            }
        }
    }
}

impl<T: StableHash + ?Sized> StableHash for &T {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (**self).stable_hash(hasher);
    }
}

impl<T> StableHash for Id<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_u64(self.raw().get());
    }
}

impl StableHash for Kind {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        // Explicit numbers: reordering the enum must not change hashes stored in files.
        let tag: u8 = match self {
            Self::Param => 1,
            Self::Frame => 2,
            Self::Sketch => 3,
            Self::SketchPoint => 4,
            Self::SketchCurve => 5,
            Self::Constraint => 6,
            Self::Dimension => 7,
            Self::Body => 8,
            Self::RobotInstance => 9,
            Self::Tool => 10,
            Self::Waypoint => 11,
            Self::Path => 12,
            Self::WorkFrame => 13,
            Self::WorkFeature => 14,
            Self::Feature => 15,
            Self::Assembly => 16,
            Self::Timeline => 17,
            Self::Relationship => 18,
            Self::SketchRole => 19,
            Self::ObjectType => 20,
            Self::WorkObject => 21,
            Self::StateVar => 22,
            Self::Task => 23,
        };
        tag.stable_hash(hasher);
    }
}

impl StableHash for AnyId {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.kind.stable_hash(hasher);
        hasher.write_u64(self.raw.get());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_never_change() {
        // These constants are part of the file format: a failure here means stored hashes would break.
        assert_eq!(StableHasher::new().finish(), 0xf52a_15e9_a9b5_e89b);
        assert_eq!("abc".stable_hash_value(), 0xa1a4_33b5_4855_5370);
        assert_eq!(1.5_f64.stable_hash_value(), 0xbfe3_f4ac_3b0e_aaaa);
    }

    #[test]
    fn floats_hash_by_value_not_by_sign_of_zero() {
        assert_eq!(0.0_f64.stable_hash_value(), (-0.0_f64).stable_hash_value());
        assert_eq!(f64::NAN.stable_hash_value(), (-f64::NAN).stable_hash_value());
        assert_ne!(1.0_f64.stable_hash_value(), 1.000_000_000_000_000_2_f64.stable_hash_value());
    }

    #[test]
    fn length_prefix_separates_adjacent_sequences() {
        let split_a = (vec![1_u8, 2], vec![3_u8]);
        let split_b = (vec![1_u8], vec![2_u8, 3]);
        let hash = |(x, y): &(Vec<u8>, Vec<u8>)| {
            let mut hasher = StableHasher::new();
            x.stable_hash(&mut hasher);
            y.stable_hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(hash(&split_a), hash(&split_b));
        assert_ne!(Some(0_u8).stable_hash_value(), None::<u8>.stable_hash_value());
    }
}
