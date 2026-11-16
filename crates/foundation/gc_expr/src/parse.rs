//! Lexer and Pratt parser.

use std::collections::BTreeSet;

use crate::Unit;

/// A byte range of the source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    fn to(self, other: Self) -> Self {
        Self { start: self.start, end: other.end }
    }
}

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `^`
    Pow,
}

/// Built-in functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs, reason = "the names are the documentation")]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sqrt,
    Abs,
    Min,
    Max,
}

impl Func {
    fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "asin" => Self::Asin,
            "acos" => Self::Acos,
            "atan" => Self::Atan,
            "sqrt" => Self::Sqrt,
            "abs" => Self::Abs,
            "min" => Self::Min,
            "max" => Self::Max,
            _ => return None,
        })
    }

    /// Number of arguments the function takes.
    pub fn arity(self) -> usize {
        match self {
            Self::Min | Self::Max => 2,
            _ => 1,
        }
    }
}

/// A parsed expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A number, optionally with a unit: `25`, `25 mm`.
    Number {
        /// The number as written (not yet converted to SI).
        value: f64,
        /// The unit written after it, if any.
        unit: Option<Unit>,
        /// Where it is in the source.
        span: Span,
    },
    /// The constant `pi`.
    Pi(Span),
    /// A reference to a named parameter.
    Name {
        /// The parameter name.
        name: String,
        /// Where it is in the source.
        span: Span,
    },
    /// `-x`
    Neg(Box<Expr>, Span),
    /// `lhs op rhs`
    Binary {
        /// The operator.
        op: BinaryOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
        /// Where the whole operation is in the source.
        span: Span,
    },
    /// `func(args...)`
    Call {
        /// The function.
        func: Func,
        /// Its arguments; the count matches [`Func::arity`].
        args: Vec<Expr>,
        /// Where the whole call is in the source.
        span: Span,
    },
}

impl Expr {
    /// The source range of this expression.
    pub fn span(&self) -> Span {
        match self {
            Self::Number { span, .. }
            | Self::Pi(span)
            | Self::Name { span, .. }
            | Self::Neg(_, span)
            | Self::Binary { span, .. }
            | Self::Call { span, .. } => *span,
        }
    }

    /// The parameter names this expression refers to.
    pub fn names(&self) -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        self.collect_names(&mut out);
        out
    }

    fn collect_names<'a>(&'a self, out: &mut BTreeSet<&'a str>) {
        match self {
            Self::Number { .. } | Self::Pi(_) => {}
            Self::Name { name, .. } => {
                out.insert(name);
            }
            Self::Neg(inner, _) => inner.collect_names(out),
            Self::Binary { lhs, rhs, .. } => {
                lhs.collect_names(out);
                rhs.collect_names(out);
            }
            Self::Call { args, .. } => args.iter().for_each(|arg| arg.collect_names(out)),
        }
    }
}

/// Why text is not an expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseErrorKind {
    /// Nothing was typed.
    #[error("the expression is empty")]
    Empty,
    /// A character that cannot appear in an expression.
    #[error("unexpected character")]
    UnexpectedChar,
    /// A number that cannot be read, such as `1.2.3`.
    #[error("malformed number")]
    BadNumber,
    /// A value was expected (a number, a name or a parenthesis).
    #[error("a value is expected here")]
    ExpectedValue,
    /// An operator was expected between two values.
    #[error("an operator is expected here")]
    ExpectedOperator,
    /// A `(` is never closed.
    #[error("missing closing parenthesis")]
    UnclosedParen,
    /// A function is called with the wrong number of arguments.
    #[error("wrong number of arguments")]
    WrongArgumentCount,
    /// A function name is not followed by `(`.
    #[error("a function needs arguments in parentheses")]
    FunctionWithoutCall,
    /// Parentheses are nested too deeply.
    #[error("the expression is nested too deeply")]
    TooDeep,
}

/// A parse error and the source range to underline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind} (at {}..{})", span.start, span.end)]
pub struct ParseError {
    /// What is wrong.
    pub kind: ParseErrorKind,
    /// Where.
    pub span: Span,
}

/// True when `name` can name a parameter: an identifier that is not a unit, function or constant.
pub fn is_valid_param_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts_well = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
    starts_well
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && Unit::from_word(name).is_none()
        && Func::from_word(name).is_none()
        && name != "pi"
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Token<'a> {
    Number(f64),
    Word(&'a str),
    Op(BinaryOp),
    LParen,
    RParen,
    Comma,
}

fn lex(source: &str) -> Result<Vec<(Token<'_>, Span)>, ParseError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let simple = match c {
            '+' => Some(Token::Op(BinaryOp::Add)),
            '-' => Some(Token::Op(BinaryOp::Sub)),
            '*' => Some(Token::Op(BinaryOp::Mul)),
            '/' => Some(Token::Op(BinaryOp::Div)),
            '^' => Some(Token::Op(BinaryOp::Pow)),
            '(' => Some(Token::LParen),
            ')' => Some(Token::RParen),
            ',' => Some(Token::Comma),
            _ => None,
        };
        if let Some(token) = simple {
            chars.next();
            tokens.push((token, Span { start, end: start + 1 }));
        } else if c.is_ascii_digit() || c == '.' {
            let digit_at = |i: usize| bytes.get(i).is_some_and(u8::is_ascii_digit);
            let mut end = start;
            while digit_at(end) {
                end += 1;
            }
            if bytes.get(end) == Some(&b'.') {
                end += 1;
                while digit_at(end) {
                    end += 1;
                }
            }
            // An exponent only counts when digits follow, so `2e` stays "2" followed by the name `e`.
            if matches!(bytes.get(end), Some(b'e' | b'E')) {
                let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
                if digit_at(end + 1 + sign) {
                    end += 1 + sign;
                    while digit_at(end) {
                        end += 1;
                    }
                }
            }
            let span = Span { start, end };
            // A second decimal point glued to the number (`1.2.3`) is a typo, not two numbers.
            let glued_dot = bytes.get(end) == Some(&b'.');
            let value = source.get(start..end).and_then(|text| text.parse::<f64>().ok());
            match value {
                Some(value) if value.is_finite() && !glued_dot => tokens.push((Token::Number(value), span)),
                _ => return Err(ParseError { kind: ParseErrorKind::BadNumber, span }),
            }
            while chars.peek().is_some_and(|&(i, _)| i < end) {
                chars.next();
            }
        } else if c.is_alphabetic() || c == '_' {
            let mut end = start;
            while let Some(&(i, ch)) = chars.peek() {
                if !(ch.is_alphanumeric() || ch == '_') {
                    break;
                }
                end = i + ch.len_utf8();
                chars.next();
            }
            tokens.push((Token::Word(&source[start..end]), Span { start, end }));
        } else {
            let span = Span { start, end: start + c.len_utf8() };
            return Err(ParseError { kind: ParseErrorKind::UnexpectedChar, span });
        }
    }
    Ok(tokens)
}

const MAX_DEPTH: usize = 64;
/// Binding power of unary minus: tighter than `*`, looser than `^`, so `-2^2` is `-(2^2)`.
const NEG_POWER: u8 = 30;

struct Parser<'a> {
    tokens: Vec<(Token<'a>, Span)>,
    position: usize,
    end_of_input: Span,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<(Token<'a>, Span)> {
        self.tokens.get(self.position).copied()
    }

    fn error_here(&self, kind: ParseErrorKind) -> ParseError {
        ParseError { kind, span: self.peek().map_or(self.end_of_input, |(_, span)| span) }
    }

    /// Parses an expression whose operators all bind at least as tightly as `min_power`.
    fn expression(&mut self, min_power: u8, depth: usize) -> Result<Expr, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error_here(ParseErrorKind::TooDeep));
        }
        let mut lhs = self.prefix(depth)?;
        while let Some((Token::Op(op), _)) = self.peek() {
            let (left, right) = match op {
                BinaryOp::Add | BinaryOp::Sub => (10, 11),
                BinaryOp::Mul | BinaryOp::Div => (20, 21),
                BinaryOp::Pow => (41, 40), // right associative
            };
            if left < min_power {
                break;
            }
            self.position += 1;
            let rhs = self.expression(right, depth + 1)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
        Ok(lhs)
    }

    fn prefix(&mut self, depth: usize) -> Result<Expr, ParseError> {
        let Some((token, span)) = self.peek() else {
            return Err(self.error_here(ParseErrorKind::ExpectedValue));
        };
        self.position += 1;
        match token {
            Token::Number(value) => {
                let mut span = span;
                let mut unit = None;
                if let Some((Token::Word(word), unit_span)) = self.peek()
                    && let Some(found) = Unit::from_word(word)
                {
                    self.position += 1;
                    unit = Some(found);
                    span = span.to(unit_span);
                }
                Ok(Expr::Number { value, unit, span })
            }
            Token::Word("pi") => Ok(Expr::Pi(span)),
            Token::Word(word) => match Func::from_word(word) {
                Some(func) => self.call(func, span, depth),
                None => Ok(Expr::Name { name: word.to_owned(), span }),
            },
            Token::Op(BinaryOp::Sub) => {
                let inner = self.expression(NEG_POWER, depth + 1)?;
                let span = span.to(inner.span());
                Ok(Expr::Neg(Box::new(inner), span))
            }
            Token::Op(BinaryOp::Add) => self.expression(NEG_POWER, depth + 1),
            Token::LParen => {
                let inner = self.expression(0, depth + 1)?;
                match self.peek() {
                    Some((Token::RParen, _)) => {
                        self.position += 1;
                        Ok(inner)
                    }
                    _ => Err(ParseError { kind: ParseErrorKind::UnclosedParen, span }),
                }
            }
            Token::Op(_) | Token::RParen | Token::Comma => {
                Err(ParseError { kind: ParseErrorKind::ExpectedValue, span })
            }
        }
    }

    fn call(&mut self, func: Func, name_span: Span, depth: usize) -> Result<Expr, ParseError> {
        let Some((Token::LParen, open)) = self.peek() else {
            return Err(ParseError { kind: ParseErrorKind::FunctionWithoutCall, span: name_span });
        };
        self.position += 1;
        let mut args = Vec::new();
        loop {
            args.push(self.expression(0, depth + 1)?);
            match self.peek() {
                Some((Token::Comma, _)) => self.position += 1,
                Some((Token::RParen, close)) => {
                    self.position += 1;
                    let span = name_span.to(close);
                    if args.len() != func.arity() {
                        return Err(ParseError { kind: ParseErrorKind::WrongArgumentCount, span });
                    }
                    return Ok(Expr::Call { func, args, span });
                }
                _ => return Err(ParseError { kind: ParseErrorKind::UnclosedParen, span: open }),
            }
        }
    }
}

/// Parses an expression.
pub fn parse(source: &str) -> Result<Expr, ParseError> {
    let tokens = lex(source)?;
    let end_of_input = Span { start: source.len(), end: source.len() };
    if tokens.is_empty() {
        return Err(ParseError { kind: ParseErrorKind::Empty, span: end_of_input });
    }
    let mut parser = Parser { tokens, position: 0, end_of_input };
    let expr = parser.expression(0, 0)?;
    match parser.peek() {
        None => Ok(expr),
        Some((Token::RParen | Token::Comma, span)) => {
            Err(ParseError { kind: ParseErrorKind::UnexpectedChar, span })
        }
        Some((_, span)) => Err(ParseError { kind: ParseErrorKind::ExpectedOperator, span }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(source: &str) -> ParseErrorKind {
        parse(source).unwrap_err().kind
    }

    #[test]
    fn numbers_and_units() {
        assert_eq!(
            parse("25 mm").unwrap(),
            Expr::Number { value: 25.0, unit: Some(Unit::Mm), span: Span { start: 0, end: 5 } }
        );
        assert!(
            matches!(parse("2.5e-3m").unwrap(), Expr::Number { value, unit: Some(Unit::M), .. } if value == 2.5e-3)
        );
        assert!(matches!(parse(".5").unwrap(), Expr::Number { value, unit: None, .. } if value == 0.5));
        assert!(matches!(parse("3.").unwrap(), Expr::Number { value, .. } if value == 3.0));
    }

    #[test]
    fn precedence_and_associativity() {
        let Expr::Binary { op, rhs, .. } = parse("1 + 2 * 3").unwrap() else { panic!("not binary") };
        assert_eq!(op, BinaryOp::Add);
        assert!(matches!(*rhs, Expr::Binary { op: BinaryOp::Mul, .. }));

        let Expr::Binary { op, lhs, .. } = parse("8 - 3 - 2").unwrap() else { panic!("not binary") };
        assert_eq!(op, BinaryOp::Sub);
        assert!(matches!(*lhs, Expr::Binary { op: BinaryOp::Sub, .. }), "subtraction is left associative");

        let Expr::Binary { op, rhs, .. } = parse("2 ^ 3 ^ 2").unwrap() else { panic!("not binary") };
        assert_eq!(op, BinaryOp::Pow);
        assert!(matches!(*rhs, Expr::Binary { op: BinaryOp::Pow, .. }), "power is right associative");

        assert!(matches!(parse("-2 ^ 2").unwrap(), Expr::Neg(..)), "-2^2 is -(2^2)");
        assert!(matches!(parse("-2 * 3").unwrap(), Expr::Binary { op: BinaryOp::Mul, .. }));
    }

    #[test]
    fn names_functions_and_spans() {
        let expr = parse("d0*2 + max(폭, 5 mm)").unwrap();
        assert_eq!(expr.names().into_iter().collect::<Vec<_>>(), vec!["d0", "폭"]);
        let Expr::Binary { rhs, .. } = expr else { panic!("not binary") };
        let Expr::Call { func: Func::Max, args, span } = *rhs else { panic!("not a call") };
        assert_eq!(args.len(), 2);
        assert_eq!(&"d0*2 + max(폭, 5 mm)"[span.start..span.end], "max(폭, 5 mm)");
    }

    #[test]
    fn errors_point_at_the_problem() {
        assert_eq!(kind(""), ParseErrorKind::Empty);
        assert_eq!(kind("   "), ParseErrorKind::Empty);
        assert_eq!(kind("1.2.3"), ParseErrorKind::BadNumber);
        assert_eq!(kind("2 $ 3"), ParseErrorKind::UnexpectedChar);
        assert_eq!(kind("2 +"), ParseErrorKind::ExpectedValue);
        assert_eq!(kind("2 3"), ParseErrorKind::ExpectedOperator);
        assert_eq!(kind("(2 + 3"), ParseErrorKind::UnclosedParen);
        assert_eq!(kind("2 + 3)"), ParseErrorKind::UnexpectedChar);
        assert_eq!(kind("sin 2"), ParseErrorKind::FunctionWithoutCall);
        assert_eq!(kind("max(1)"), ParseErrorKind::WrongArgumentCount);
        assert_eq!(kind("sin(1, 2)"), ParseErrorKind::WrongArgumentCount);
        assert_eq!(parse("12 + * 3").unwrap_err().span, Span { start: 5, end: 6 });
        assert_eq!(parse("2 +").unwrap_err().span, Span { start: 3, end: 3 });
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_stack_overflow() {
        let deep = format!("{}1{}", "(".repeat(10_000), ")".repeat(10_000));
        assert_eq!(kind(&deep), ParseErrorKind::TooDeep);
        assert_eq!(kind(&"-".repeat(10_000)), ParseErrorKind::TooDeep);
        let fine = format!("{}1{}", "(".repeat(40), ")".repeat(40));
        assert!(parse(&fine).is_ok());
    }

    #[test]
    fn parameter_names() {
        for good in ["d0", "폭", "_tmp", "width_2"] {
            assert!(is_valid_param_name(good), "{good}");
        }
        for bad in ["", "2d", "mm", "in", "sin", "pi", "a b", "a-b"] {
            assert!(!is_valid_param_name(bad), "{bad}");
        }
    }

    #[test]
    fn arbitrary_text_never_panics() {
        let alphabet = ['1', '.', 'e', '+', '-', '*', '/', '^', '(', ')', ',', ' ', 'm', 'd', '폭', '$', '9'];
        let mut state = 12345_u64;
        for _ in 0..5000 {
            let mut text = String::new();
            for _ in 0..12 {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                text.push(alphabet[(state >> 33) as usize % alphabet.len()]);
            }
            if let Ok(expr) = parse(&text) {
                drop(expr.eval_as(&crate::NoParams, Unit::Mm));
            }
        }
    }
}
