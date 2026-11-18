//! Document Settings (Inventor 2026 help, To Change Document Settings, GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD):
//! settings saved with the file. Units tab: "Units of measure, Modeling Dimension Display, Default Parameter
//! Input Display"; Sketch tab (Sketch Tab Reference (Document Settings), GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F):
//! snap spacing, grid display, "3D Sketch Auto-Bend Radius". About Units of Measure
//! (GUID-671A8679-AEC1-413A-A23B-428967648CE6): "Unit value settings are saved with the file."
//!
//! A file saved before these existed reads with the defaults.

use serde::{Deserialize, Serialize};

/// The length unit of a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    /// Millimetres.
    #[default]
    Mm,
    /// Centimetres.
    Cm,
    /// Metres.
    M,
    /// Inches.
    In,
}

/// The angle unit of a document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AngleUnit {
    /// Degrees.
    #[default]
    Deg,
    /// Radians.
    Rad,
}

/// How sketch dimensions show (Modeling Dimension Display; the status bar's dimension display icons, To Work
/// with Sketch Dimensions, GUID-026FE8D3-AFBB-4834-BC18-73D9CFF78185). Tolerance is not offered: dimensions
/// carry none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionDisplay {
    /// "Displays sketch dimensions as the calculated value", to the document's precision.
    #[default]
    Value,
    /// "Displays sketch dimensions as the parameter name."
    Name,
    /// "Displays sketch dimensions as the specified parametric expression."
    Expression,
    /// "Displays sketch dimensions as a value, ignoring any precision."
    PreciseValue,
}

/// The settings saved with a document.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocSettings {
    /// Units tab: length.
    pub length_unit: LengthUnit,
    /// Units tab: angle.
    pub angle_unit: AngleUnit,
    /// Units tab, Modeling Dimension Display: decimals shown for lengths (0 to 8).
    pub linear_precision: u8,
    /// Units tab, Modeling Dimension Display: decimals shown for angles (0 to 8).
    pub angular_precision: u8,
    /// How dimensions show.
    pub dimension_display: DimensionDisplay,
    /// Units tab, Default Parameter Input Display: an input box shows a dimension's expression (as typed)
    /// rather than its value.
    pub input_as_expression: bool,
    /// Sketch tab: 3D Sketch Auto-Bend Radius, metres.
    pub auto_bend_radius: f64,
    /// Sketch tab: snap spacing along the sketch's X and Y, metres.
    pub snap_spacing: [f64; 2],
    /// Sketch tab, Grid Display: snaps per minor grid line.
    pub snaps_per_minor: u32,
    /// Sketch tab, Grid Display: minor lines per major line.
    pub minor_per_major: u32,
    /// Constraint inference while sketching (Constraint Settings, Inference): new geometry takes the
    /// constraints its placement implies.
    pub infer_constraints: bool,
    /// Inferred constraints are kept (Persist Constraints); off, they only guide the placement.
    pub persist_constraints: bool,
}

impl Default for DocSettings {
    fn default() -> Self {
        Self {
            length_unit: LengthUnit::Mm,
            angle_unit: AngleUnit::Deg,
            linear_precision: 3,
            angular_precision: 3,
            dimension_display: DimensionDisplay::Value,
            input_as_expression: true,
            auto_bend_radius: 0.01,
            snap_spacing: [0.001, 0.001],
            snaps_per_minor: 10,
            minor_per_major: 10,
            infer_constraints: true,
            persist_constraints: true,
        }
    }
}

impl DocSettings {
    /// True when every setting is its default (nothing to save).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// True when the settings can be used: precisions 0 to 8, lengths positive and finite, grid counts 1
    /// to 100.
    pub fn is_valid(&self) -> bool {
        let length = |v: f64| v.is_finite() && v > 0.0;
        self.linear_precision <= 8
            && self.angular_precision <= 8
            && length(self.auto_bend_radius)
            && self.snap_spacing.iter().all(|v| length(*v))
            && (1..=100).contains(&self.snaps_per_minor)
            && (1..=100).contains(&self.minor_per_major)
    }

    /// The document's length unit as an expression unit (what a typed number without a unit means).
    pub fn length(&self) -> gc_expr::Unit {
        match self.length_unit {
            LengthUnit::Mm => gc_expr::Unit::Mm,
            LengthUnit::Cm => gc_expr::Unit::Cm,
            LengthUnit::M => gc_expr::Unit::M,
            LengthUnit::In => gc_expr::Unit::In,
        }
    }

    /// The document's angle unit as an expression unit.
    pub fn angle(&self) -> gc_expr::Unit {
        match self.angle_unit {
            AngleUnit::Deg => gc_expr::Unit::Deg,
            AngleUnit::Rad => gc_expr::Unit::Rad,
        }
    }

    /// A length in the document's unit, to its precision (trailing zeros dropped), e.g. `"12.5 mm"`.
    pub fn format_length(&self, metres: f64) -> String {
        let unit = self.length();
        format!("{} {}", trimmed(metres / unit.si_factor(), self.linear_precision), unit.word())
    }

    /// An angle in the document's unit, to its precision, e.g. `"30 deg"`.
    pub fn format_angle(&self, radians: f64) -> String {
        let unit = self.angle();
        format!("{} {}", trimmed(radians / unit.si_factor(), self.angular_precision), unit.word())
    }

    /// A dimension's text as the display setting shows it: the value (to the precision, or precise) with
    /// `prefix` (`"R"` for a radius), the parameter name, or the name with its expression. An angle when
    /// `angle`. A dimension the document has not named shows its value instead of a name.
    pub fn dimension_text(&self, angle: bool, prefix: &str, value: f64, name: &str, source: &str) -> String {
        let shown = |precise: bool| {
            let mut s = *self;
            if precise {
                s.linear_precision = 8;
                s.angular_precision = 8;
            }
            let text = if angle { s.format_angle(value) } else { s.format_length(value) };
            format!("{prefix}{text}")
        };
        match self.dimension_display {
            DimensionDisplay::Value => shown(false),
            DimensionDisplay::PreciseValue => shown(true),
            DimensionDisplay::Name if !name.is_empty() => name.to_owned(),
            DimensionDisplay::Expression if !name.is_empty() => format!("{name} = {}", source.trim()),
            DimensionDisplay::Expression => source.trim().to_owned(),
            DimensionDisplay::Name => shown(false),
        }
    }

    /// The Auto-Bend radius as an expression (`"10 mm"`), for a Bend.
    pub fn auto_bend_text(&self) -> String {
        let unit = self.length();
        format!("{} {}", trimmed(self.auto_bend_radius / unit.si_factor(), 8), unit.word())
    }
}

/// `value` to `decimals` places, trailing zeros and a bare point dropped, never "-0".
fn trimmed(value: f64, decimals: u8) -> String {
    if !value.is_finite() {
        return format!("{value}");
    }
    let text = format!("{value:.*}", usize::from(decimals));
    let text =
        if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { text.as_str() };
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_follows_the_units_and_precision() {
        let mut s = DocSettings::default();
        assert_eq!(s.format_length(0.0125), "12.5 mm");
        assert_eq!(s.format_angle(std::f64::consts::FRAC_PI_6), "30 deg");
        s.length_unit = LengthUnit::In;
        s.linear_precision = 2;
        assert_eq!(s.format_length(0.0254), "1 in");
        assert_eq!(s.format_length(-0.0001), "0 in");
        s.angle_unit = AngleUnit::Rad;
        s.angular_precision = 4;
        assert_eq!(s.format_angle(1.0), "1 rad");
        assert_eq!(s.auto_bend_text(), "0.39370079 in");
        let mut d = DocSettings::default();
        assert_eq!(d.dimension_text(false, "R", 0.0123456, "d3", "12.3456 mm"), "R12.346 mm");
        d.dimension_display = DimensionDisplay::PreciseValue;
        assert_eq!(d.dimension_text(false, "", 0.0123456, "d3", "12.3456 mm"), "12.3456 mm");
        d.dimension_display = DimensionDisplay::Name;
        assert_eq!(d.dimension_text(false, "", 0.01, "d3", "10"), "d3");
        assert_eq!(d.dimension_text(false, "", 0.01, "", "10"), "10 mm");
        d.dimension_display = DimensionDisplay::Expression;
        assert_eq!(d.dimension_text(true, "", 0.5, "ANG", "d3 * 2"), "ANG = d3 * 2");
        assert!(DocSettings::default().is_valid());
        assert!(!DocSettings { linear_precision: 9, ..DocSettings::default() }.is_valid());
        assert!(!DocSettings { auto_bend_radius: 0.0, ..DocSettings::default() }.is_valid());
    }
}
