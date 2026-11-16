//! Units and run-time dimensions.

/// Exponents of the base dimensions of a value. `{length: 1}` is a length, `{length: 2}` an area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dim {
    /// Power of length.
    pub length: i8,
    /// Power of angle.
    pub angle: i8,
}

impl Dim {
    /// A pure number.
    pub const SCALAR: Self = Self { length: 0, angle: 0 };
    /// A length.
    pub const LENGTH: Self = Self { length: 1, angle: 0 };
    /// An angle.
    pub const ANGLE: Self = Self { length: 0, angle: 1 };

    /// True for a pure number.
    pub fn is_scalar(self) -> bool {
        self == Self::SCALAR
    }

    pub(crate) fn combine(self, other: Self, sign: i8) -> Option<Self> {
        Some(Self {
            length: self.length.checked_add(other.length.checked_mul(sign)?)?,
            angle: self.angle.checked_add(other.angle.checked_mul(sign)?)?,
        })
    }

    pub(crate) fn scaled(self, factor: i8) -> Option<Self> {
        Some(Self { length: self.length.checked_mul(factor)?, angle: self.angle.checked_mul(factor)? })
    }
}

/// A value in SI units (metres, radians) together with its dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    /// The value in SI units.
    pub si: f64,
    /// What kind of value it is.
    pub dim: Dim,
}

impl Quantity {
    /// A pure number.
    pub fn scalar(value: f64) -> Self {
        Self { si: value, dim: Dim::SCALAR }
    }

    /// A length in metres.
    pub fn length(metres: f64) -> Self {
        Self { si: metres, dim: Dim::LENGTH }
    }

    /// An angle in radians.
    pub fn angle(radians: f64) -> Self {
        Self { si: radians, dim: Dim::ANGLE }
    }
}

/// A unit a number can be written in, or the unit of an input field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// No unit: counts and ratios.
    Scalar,
    /// Millimetres.
    Mm,
    /// Centimetres.
    Cm,
    /// Metres.
    M,
    /// Inches.
    In,
    /// Degrees.
    Deg,
    /// Radians.
    Rad,
}

impl Unit {
    /// The unit written as `word`, if any.
    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "mm" => Self::Mm,
            "cm" => Self::Cm,
            "m" => Self::M,
            "in" => Self::In,
            "deg" => Self::Deg,
            "rad" => Self::Rad,
            _ => return None,
        })
    }

    /// How the unit is written (empty for [`Unit::Scalar`]).
    pub fn word(self) -> &'static str {
        match self {
            Self::Scalar => "",
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::M => "m",
            Self::In => "in",
            Self::Deg => "deg",
            Self::Rad => "rad",
        }
    }

    /// SI value of one of this unit.
    pub fn si_factor(self) -> f64 {
        match self {
            Self::Scalar | Self::M | Self::Rad => 1.0,
            Self::Mm => 1e-3,
            Self::Cm => 1e-2,
            Self::In => 0.0254,
            Self::Deg => std::f64::consts::PI / 180.0,
        }
    }

    /// The dimension of values in this unit.
    pub fn dim(self) -> Dim {
        match self {
            Self::Scalar => Dim::SCALAR,
            Self::Mm | Self::Cm | Self::M | Self::In => Dim::LENGTH,
            Self::Deg | Self::Rad => Dim::ANGLE,
        }
    }

    /// Converts a number written in this unit to SI.
    pub fn to_si(self, value: f64) -> f64 {
        value * self.si_factor()
    }

    /// Converts an SI value to this unit, for display.
    pub fn from_si(self, si: f64) -> f64 {
        si / self.si_factor()
    }
}
