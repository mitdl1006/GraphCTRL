//! Evaluation with unit checking.

use std::collections::BTreeMap;

use crate::{BinaryOp, Dim, Expr, Func, Quantity, Span, Unit};

/// Supplies the values of named parameters.
pub trait Scope {
    /// The value of `name`, or `None` when no such parameter exists (or it has no valid value).
    fn lookup(&self, name: &str) -> Option<Quantity>;

    /// The units a bare number means in a length field and an angle field (a document's units), when they
    /// are not millimetres and degrees.
    fn default_units(&self) -> Option<(Unit, Unit)> {
        None
    }
}

/// A scope without parameters.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoParams;

impl Scope for NoParams {
    fn lookup(&self, _: &str) -> Option<Quantity> {
        None
    }
}

impl Scope for BTreeMap<String, Quantity> {
    fn lookup(&self, name: &str) -> Option<Quantity> {
        self.get(name).copied()
    }
}

/// Why an expression has no value.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EvalErrorKind {
    /// A name that is not a parameter.
    #[error("unknown parameter `{0}`")]
    UnknownName(String),
    /// Two values that cannot be combined, such as a length plus an angle.
    #[error("units do not match")]
    UnitMismatch {
        /// Dimension that was needed.
        expected: Dim,
        /// Dimension that was found.
        found: Dim,
    },
    /// Division by zero.
    #[error("division by zero")]
    DivisionByZero,
    /// An exponent that would give a meaningless unit, such as `(5 mm) ^ 0.3`.
    #[error("a value with a unit can only be raised to a whole number")]
    BadExponent,
    /// The square root of a value whose unit has no square root, such as `sqrt(5 mm)`.
    #[error("this unit has no square root")]
    BadRoot,
    /// A function argument outside its domain, or a result too large to represent.
    #[error("the result is not a number")]
    NotFinite,
}

/// An evaluation error and the source range to underline.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("{kind} (at {}..{})", span.start, span.end)]
pub struct EvalError {
    /// What is wrong.
    pub kind: EvalErrorKind,
    /// Where.
    pub span: Span,
}

fn mismatch(expected: Dim, found: Dim, span: Span) -> EvalError {
    EvalError { kind: EvalErrorKind::UnitMismatch { expected, found }, span }
}

/// Brings two values to a common dimension for `+`, `-`, `min` and `max`.
///
/// A unit-less operand next to one in the field's dimension is read in the field's unit, so `10 + 5 mm`
/// in a millimetre field is 15 mm, as CAD users expect.
fn align(a: Quantity, b: Quantity, field: Unit, span: Span) -> Result<(f64, f64, Dim), EvalError> {
    if a.dim == b.dim {
        Ok((a.si, b.si, a.dim))
    } else if a.dim.is_scalar() && b.dim == field.dim() {
        Ok((field.to_si(a.si), b.si, b.dim))
    } else if b.dim.is_scalar() && a.dim == field.dim() {
        Ok((a.si, field.to_si(b.si), a.dim))
    } else {
        Err(mismatch(a.dim, b.dim, span))
    }
}

impl Expr {
    /// Evaluates for an input field of unit `field` and returns the value in SI units.
    ///
    /// A unit-less result takes the field's unit (`25` in a millimetre field is 0.025 m). A result of
    /// another dimension is an error.
    pub fn eval_as(&self, scope: &dyn Scope, field: Unit) -> Result<f64, EvalError> {
        let value = self.eval(scope, field)?;
        if value.dim == field.dim() {
            Ok(value.si)
        } else if value.dim.is_scalar() {
            Ok(field.to_si(value.si))
        } else {
            Err(mismatch(field.dim(), value.dim, self.span()))
        }
    }

    /// Evaluates to a value with its dimension. `field` only decides how unit-less numbers mix with
    /// values that have a unit (see [`Expr::eval_as`]).
    pub fn eval(&self, scope: &dyn Scope, field: Unit) -> Result<Quantity, EvalError> {
        let value = self.eval_inner(scope, field)?;
        if value.si.is_finite() {
            Ok(value)
        } else {
            Err(EvalError { kind: EvalErrorKind::NotFinite, span: self.span() })
        }
    }

    fn eval_inner(&self, scope: &dyn Scope, field: Unit) -> Result<Quantity, EvalError> {
        let span = self.span();
        match self {
            Self::Number { value, unit, .. } => Ok(match unit {
                Some(unit) => Quantity { si: unit.to_si(*value), dim: unit.dim() },
                None => Quantity::scalar(*value),
            }),
            Self::Pi(_) => Ok(Quantity::scalar(std::f64::consts::PI)),
            Self::Name { name, .. } => scope
                .lookup(name)
                .ok_or_else(|| EvalError { kind: EvalErrorKind::UnknownName(name.clone()), span }),
            Self::Neg(inner, _) => {
                let value = inner.eval(scope, field)?;
                Ok(Quantity { si: -value.si, dim: value.dim })
            }
            Self::Binary { op, lhs, rhs, .. } => {
                let (a, b) = (lhs.eval(scope, field)?, rhs.eval(scope, field)?);
                binary(*op, a, b, field, span)
            }
            Self::Call { func, args, .. } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(arg.eval(scope, field)?);
                }
                match (values.first().copied(), values.get(1).copied()) {
                    (Some(a), b) => call(*func, a, b, field, span),
                    (None, _) => Err(EvalError { kind: EvalErrorKind::NotFinite, span }),
                }
            }
        }
    }
}

fn binary(op: BinaryOp, a: Quantity, b: Quantity, field: Unit, span: Span) -> Result<Quantity, EvalError> {
    let overflow = || EvalError { kind: EvalErrorKind::BadExponent, span };
    match op {
        BinaryOp::Add | BinaryOp::Sub => {
            let (x, y, dim) = align(a, b, field, span)?;
            Ok(Quantity { si: if op == BinaryOp::Add { x + y } else { x - y }, dim })
        }
        BinaryOp::Mul => Ok(Quantity { si: a.si * b.si, dim: a.dim.combine(b.dim, 1).ok_or_else(overflow)? }),
        BinaryOp::Div => {
            if b.si == 0.0 {
                return Err(EvalError { kind: EvalErrorKind::DivisionByZero, span });
            }
            Ok(Quantity { si: a.si / b.si, dim: a.dim.combine(b.dim, -1).ok_or_else(overflow)? })
        }
        BinaryOp::Pow => {
            if !b.dim.is_scalar() {
                return Err(mismatch(Dim::SCALAR, b.dim, span));
            }
            if a.dim.is_scalar() {
                return Ok(Quantity::scalar(libm::pow(a.si, b.si)));
            }
            // A value with a unit: only small whole exponents keep the unit meaningful.
            if b.si.fract() != 0.0 || b.si.abs() > 8.0 {
                return Err(overflow());
            }
            // The range check above makes the conversion exact.
            #[allow(clippy::cast_possible_truncation, reason = "whole number within -8..=8")]
            let exponent = b.si as i8;
            Ok(Quantity { si: libm::pow(a.si, b.si), dim: a.dim.scaled(exponent).ok_or_else(overflow)? })
        }
    }
}

fn call(
    func: Func,
    a: Quantity,
    b: Option<Quantity>,
    field: Unit,
    span: Span,
) -> Result<Quantity, EvalError> {
    // Trigonometric functions take an angle; a bare number is read as radians.
    let angle = |q: Quantity| {
        if q.dim == Dim::ANGLE || q.dim.is_scalar() {
            Ok(q.si)
        } else {
            Err(mismatch(Dim::ANGLE, q.dim, span))
        }
    };
    let ratio =
        |q: Quantity| if q.dim.is_scalar() { Ok(q.si) } else { Err(mismatch(Dim::SCALAR, q.dim, span)) };
    Ok(match func {
        Func::Sin => Quantity::scalar(libm::sin(angle(a)?)),
        Func::Cos => Quantity::scalar(libm::cos(angle(a)?)),
        Func::Tan => Quantity::scalar(libm::tan(angle(a)?)),
        Func::Asin => Quantity::angle(libm::asin(ratio(a)?)),
        Func::Acos => Quantity::angle(libm::acos(ratio(a)?)),
        Func::Atan => Quantity::angle(libm::atan(ratio(a)?)),
        Func::Abs => Quantity { si: a.si.abs(), dim: a.dim },
        Func::Sqrt => {
            if a.dim.length % 2 != 0 || a.dim.angle % 2 != 0 {
                return Err(EvalError { kind: EvalErrorKind::BadRoot, span });
            }
            Quantity { si: a.si.sqrt(), dim: Dim { length: a.dim.length / 2, angle: a.dim.angle / 2 } }
        }
        Func::Min | Func::Max => {
            let Some(b) = b else { return Err(EvalError { kind: EvalErrorKind::NotFinite, span }) };
            let (x, y, dim) = align(a, b, field, span)?;
            Quantity { si: if func == Func::Min { x.min(y) } else { x.max(y) }, dim }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn scope() -> BTreeMap<String, Quantity> {
        BTreeMap::from([
            ("d0".to_owned(), Quantity::length(0.010)),
            ("d1".to_owned(), Quantity::length(0.004)),
            ("a0".to_owned(), Quantity::angle(std::f64::consts::FRAC_PI_6)),
            ("n".to_owned(), Quantity::scalar(3.0)),
        ])
    }

    fn mm(source: &str) -> Result<f64, EvalErrorKind> {
        parse(source).unwrap().eval_as(&scope(), Unit::Mm).map(|si| si * 1000.0).map_err(|e| e.kind)
    }

    fn deg(source: &str) -> Result<f64, EvalErrorKind> {
        parse(source).unwrap().eval_as(&scope(), Unit::Deg).map(f64::to_degrees).map_err(|e| e.kind)
    }

    fn close(got: Result<f64, EvalErrorKind>, want: f64) {
        let value = got.unwrap();
        assert!((value - want).abs() < 1e-9, "got {value}, want {want}");
    }

    #[test]
    fn bare_numbers_take_the_field_unit() {
        close(mm("25"), 25.0);
        close(mm("25 mm"), 25.0);
        close(mm("2.5 cm"), 25.0);
        close(mm("1 in"), 25.4);
        close(deg("90"), 90.0);
        close(deg("pi / 2 * 1 rad"), 90.0);
        assert!(
            matches!(deg("pi / 2 rad"), Err(EvalErrorKind::UnitMismatch { .. })),
            "a unit binds to its number"
        );
    }

    #[test]
    fn parameters_and_arithmetic() {
        close(mm("d0 * 2"), 20.0);
        close(mm("d0 * 2 + 5 mm"), 25.0);
        close(mm("d0 + 5"), 15.0);
        close(mm("(d0 + d1) / 2"), 7.0);
        close(mm("d0 / d1 * 1 mm"), 2.5);
        close(mm("-d0 + 30"), 20.0);
        close(mm("n * d1"), 12.0);
        close(mm("2 ^ 3"), 8.0);
        close(mm("-2 ^ 2 + 10"), 6.0);
    }

    #[test]
    fn functions_check_their_units() {
        close(mm("d0 * sin(a0)"), 5.0);
        close(mm("d0 * sin(30 deg)"), 5.0);
        close(mm("sqrt(d0 * d0)"), 10.0);
        close(mm("sqrt(d0 ^ 2 + (2 * d0) ^ 2 - 4 * d0 * d0)"), 10.0);
        close(mm("max(d0, d1)"), 10.0);
        close(mm("min(d0, 3)"), 3.0);
        close(mm("abs(d1 - d0)"), 6.0);
        close(deg("asin(0.5)"), 30.0);
        close(deg("atan(d0 / d0)"), 45.0);
        assert!(matches!(mm("sin(d0)"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert!(matches!(deg("asin(d0)"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert_eq!(mm("sqrt(d0)"), Err(EvalErrorKind::BadRoot));
    }

    #[test]
    fn mixed_units_are_rejected() {
        assert!(matches!(mm("5 mm + 3 deg"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert!(matches!(mm("a0"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert!(matches!(mm("d0 * d1"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert!(matches!(deg("d0"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert!(matches!(deg("a0 + 1 mm"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert!(matches!(mm("2 ^ d0"), Err(EvalErrorKind::UnitMismatch { .. })));
        assert_eq!(mm("d0 ^ 0.5"), Err(EvalErrorKind::BadExponent));
        assert_eq!(mm("d0 ^ 100"), Err(EvalErrorKind::BadExponent));
    }

    #[test]
    fn failures_are_errors_never_nan() {
        assert_eq!(mm("1 / 0"), Err(EvalErrorKind::DivisionByZero));
        assert_eq!(mm("d0 / (d1 - d1)"), Err(EvalErrorKind::DivisionByZero));
        assert_eq!(mm("sqrt(-1)"), Err(EvalErrorKind::NotFinite));
        assert_eq!(deg("acos(2)"), Err(EvalErrorKind::NotFinite));
        assert_eq!(mm("10 ^ 400"), Err(EvalErrorKind::NotFinite));
        assert_eq!(mm("missing + 1"), Err(EvalErrorKind::UnknownName("missing".to_owned())));
    }

    #[test]
    fn errors_carry_the_span_of_the_offending_part() {
        let error = parse("d0 + sqrt(-1)").unwrap().eval_as(&scope(), Unit::Mm).unwrap_err();
        assert_eq!((error.span.start, error.span.end), (5, 13));
        let error = parse("5 + nope").unwrap().eval_as(&NoParams, Unit::Mm).unwrap_err();
        assert_eq!((error.span.start, error.span.end), (4, 8));
    }
}
