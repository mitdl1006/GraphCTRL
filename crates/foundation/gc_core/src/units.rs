//! Unit newtypes and comparison tolerances. Internal values are SI: metres and radians.

use serde::{Deserialize, Serialize};

/// A length in metres.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Meters(pub f64);

impl Meters {
    /// Converts from millimetres, the unit shown to the user.
    pub fn from_mm(mm: f64) -> Self {
        Self(mm / 1000.0)
    }

    /// The length in millimetres.
    pub fn as_mm(self) -> f64 {
        self.0 * 1000.0
    }
}

/// An angle in radians.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Radians(pub f64);

impl Radians {
    /// Converts from degrees, the unit shown to the user.
    pub fn from_deg(deg: f64) -> Self {
        Self(deg.to_radians())
    }

    /// The angle in degrees.
    pub fn as_deg(self) -> f64 {
        self.0.to_degrees()
    }
}

/// Absolute tolerances for deciding that two modelling values are the same.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    /// Lengths closer than this (metres) are equal.
    pub length: f64,
    /// Angles closer than this (radians) are equal.
    pub angle: f64,
}

impl Default for Tolerance {
    /// One nanometre and one nanoradian: far below anything a robot cell can resolve.
    fn default() -> Self {
        Self { length: 1e-9, angle: 1e-9 }
    }
}

impl Tolerance {
    /// True when two lengths in metres are equal within the tolerance.
    pub fn same_length(&self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.length
    }

    /// True when two angles in radians are equal within the tolerance (no wrap-around).
    pub fn same_angle(&self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.angle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_units_round_trip() {
        assert!((Meters::from_mm(25.0).0 - 0.025).abs() < 1e-15);
        assert!((Meters(0.025).as_mm() - 25.0).abs() < 1e-12);
        assert!((Radians::from_deg(180.0).0 - std::f64::consts::PI).abs() < 1e-15);
        assert!((Radians(std::f64::consts::FRAC_PI_2).as_deg() - 90.0).abs() < 1e-12);
    }

    #[test]
    fn tolerance_compares_absolute_differences() {
        let tol = Tolerance::default();
        assert!(tol.same_length(1.0, 1.0 + 5e-10));
        assert!(!tol.same_length(1.0, 1.0 + 5e-9));
        assert!(tol.same_angle(0.5, 0.5 - 5e-10));
    }
}
