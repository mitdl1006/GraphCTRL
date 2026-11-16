//! Expressions typed into dimension and parameter fields, e.g. `d0 * 2 + 5 mm`.
//!
//! - [`parse`] turns text into an [`Expr`]; errors carry the byte range to underline.
//! - [`Expr::eval_as`] computes the value in SI units for a field of a given unit, checking units on the
//!   way (`5 mm + 3 deg` is an error).
//! - [`eval_order`] sorts parameters so that each is evaluated after the ones it uses, and reports cycles.
//!
//! No input makes these functions panic or loop.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::float_cmp, reason = "tests compare exactly representable values"))]

mod eval;
mod order;
mod parse;
mod units;

pub use eval::{EvalError, EvalErrorKind, NoParams, Scope};
pub use order::eval_order;
pub use parse::{BinaryOp, Expr, Func, ParseError, ParseErrorKind, Span, is_valid_param_name, parse};
pub use units::{Dim, Quantity, Unit};
