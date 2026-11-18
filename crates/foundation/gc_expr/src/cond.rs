//! Conditions over states, e.g. `HoleA.state == Fastened and count < 3` (plan 9.4: preconditions of tasks).
//!
//! - Values are truths (`true`, `false`), numbers and names (a state such as `Fastened`).
//! - A bare word is a state variable when the scope knows one of that name, otherwise a name.
//!   `Object.field` reads a field of an object (its `state`). A name with spaces or other characters is
//!   written in double quotes: `"Hole A".state == "fastened tight"`.
//! - `==` and `!=` compare two values of one kind; `<`, `<=`, `>`, `>=` compare numbers; `and`, `or`, `not`
//!   (also `&&`, `||`, `!`) combine truths. `not` binds tighter than `and`, which binds tighter than `or`;
//!   comparisons bind tightest.
//!
//! No input makes these functions panic or loop.

use crate::Span;

/// A value a condition works with.
#[derive(Clone, Debug, PartialEq)]
pub enum CondValue {
    /// True or false.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A name, e.g. a state of an object or a choice of an enumeration.
    Name(String),
}

/// A comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// A parsed condition.
#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    /// `true`, `false` or a number.
    Literal(CondValue, Span),
    /// A bare word: a state variable, or else a name.
    Word(String, Span),
    /// `object.field`.
    Member {
        /// The object's name.
        object: String,
        /// Where the object's name is.
        object_span: Span,
        /// The field.
        field: String,
        /// The whole reference.
        span: Span,
    },
    /// `not x`.
    Not(Box<Condition>, Span),
    /// `a and b`.
    And(Box<Condition>, Box<Condition>),
    /// `a or b`.
    Or(Box<Condition>, Box<Condition>),
    /// `a == b` and the like.
    Compare(Compare, Box<Condition>, Box<Condition>),
}

/// What is wrong with a condition, and where (bytes of the source).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct CondError {
    /// What is wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

fn error<T>(message: impl Into<String>, span: Span) -> Result<T, CondError> {
    Err(CondError { message: message.into(), span })
}

/// What a condition's words stand for.
pub trait CondScope {
    /// The value of the state variable `name`; `None` when there is none (the word is then a name).
    fn variable(&self, name: &str) -> Option<CondValue>;
    /// The value of `object.field`; `None` when the object or the field does not exist.
    fn member(&self, object: &str, field: &str) -> Option<CondValue>;
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    /// A word, and whether it was quoted (a quoted word is never a keyword).
    Word(String, bool),
    Dot,
    LParen,
    RParen,
    Not,
    And,
    Or,
    Compare(Compare),
}

fn tokens(source: &str) -> Result<Vec<(Token, Span)>, CondError> {
    let mut out = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let two = |next: char, chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>| {
            chars.next_if(|(_, n)| *n == next).is_some()
        };
        let token = match c {
            c if c.is_whitespace() => continue,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '.' => Token::Dot,
            '"' => {
                // A quoted name: everything up to the next quote (a name cannot hold one).
                let Some(length) = source[start + 1..].find('"') else {
                    return error("the closing quote is missing", Span { start, end: source.len() });
                };
                let end = start + 1 + length;
                while chars.next_if(|(at, _)| *at <= end).is_some() {}
                if length == 0 {
                    return error("a name is expected between the quotes", Span { start, end: end + 1 });
                }
                Token::Word(source[start + 1..end].to_owned(), true)
            }
            '=' if two('=', &mut chars) => Token::Compare(Compare::Eq),
            '!' if two('=', &mut chars) => Token::Compare(Compare::Ne),
            '!' => Token::Not,
            '<' if two('=', &mut chars) => Token::Compare(Compare::Le),
            '<' => Token::Compare(Compare::Lt),
            '>' if two('=', &mut chars) => Token::Compare(Compare::Ge),
            '>' => Token::Compare(Compare::Gt),
            '&' if two('&', &mut chars) => Token::And,
            '|' if two('|', &mut chars) => Token::Or,
            c if c.is_ascii_digit() || c == '-' => {
                let mut end = start + c.len_utf8();
                // Digits, then at most one fraction; a dot not followed by a digit is left alone.
                let mut seen_dot = false;
                while let Some((at, n)) = chars.peek().copied() {
                    let fraction = n == '.'
                        && !seen_dot
                        && source[at + 1..].chars().next().is_some_and(|d| d.is_ascii_digit());
                    if !(n.is_ascii_digit() || fraction) {
                        break;
                    }
                    seen_dot |= n == '.';
                    end = at + n.len_utf8();
                    chars.next();
                }
                match source[start..end].parse::<f64>() {
                    Ok(value) if value.is_finite() => Token::Number(value),
                    _ => return error("this is not a number", Span { start, end }),
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut end = start + c.len_utf8();
                while let Some((at, n)) = chars.peek().copied() {
                    if !(n.is_alphanumeric() || n == '_') {
                        break;
                    }
                    end = at + n.len_utf8();
                    chars.next();
                }
                match &source[start..end] {
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    word => Token::Word(word.to_owned(), false),
                }
            }
            other => {
                return error(
                    format!("`{other}` cannot be used here"),
                    Span { start, end: start + other.len_utf8() },
                );
            }
        };
        let end = chars.peek().map_or(source.len(), |(at, _)| *at);
        out.push((token, Span { start, end }));
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<(Token, Span)>,
    at: usize,
    end: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(token, _)| token)
    }

    fn here(&self) -> Span {
        self.tokens.get(self.at).map_or(Span { start: self.end, end: self.end }, |(_, span)| *span)
    }

    fn or(&mut self) -> Result<Condition, CondError> {
        let mut left = self.and()?;
        while self.peek() == Some(&Token::Or) {
            self.at += 1;
            left = Condition::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Condition, CondError> {
        let mut left = self.not()?;
        while self.peek() == Some(&Token::And) {
            self.at += 1;
            left = Condition::And(Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Condition, CondError> {
        if self.peek() == Some(&Token::Not) {
            let span = self.here();
            self.at += 1;
            // Nesting is bounded so that no input can exhaust the stack.
            self.depth += 1;
            if self.depth > 64 {
                return error("this is nested too deeply", span);
            }
            let inner = self.not()?;
            self.depth -= 1;
            return Ok(Condition::Not(Box::new(inner), span));
        }
        self.compare()
    }

    fn compare(&mut self) -> Result<Condition, CondError> {
        let left = self.primary()?;
        if let Some(Token::Compare(op)) = self.peek().cloned() {
            self.at += 1;
            let right = self.primary()?;
            return Ok(Condition::Compare(op, Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn primary(&mut self) -> Result<Condition, CondError> {
        let span = self.here();
        let Some((token, _)) = self.tokens.get(self.at).cloned() else {
            return error("something is missing here", span);
        };
        self.at += 1;
        match token {
            Token::Number(value) => Ok(Condition::Literal(CondValue::Number(value), span)),
            Token::Word(word, false) if word == "true" || word == "false" => {
                Ok(Condition::Literal(CondValue::Bool(word == "true"), span))
            }
            Token::Word(object, _) if self.peek() == Some(&Token::Dot) => {
                self.at += 1;
                let field_span = self.here();
                match self.tokens.get(self.at).cloned() {
                    Some((Token::Word(field, _), _)) => {
                        self.at += 1;
                        Ok(Condition::Member {
                            object,
                            object_span: span,
                            field,
                            span: Span { start: span.start, end: field_span.end },
                        })
                    }
                    _ => error("a field name is expected after the dot", field_span),
                }
            }
            Token::Word(word, _) => Ok(Condition::Word(word, span)),
            Token::LParen => {
                self.depth += 1;
                if self.depth > 64 {
                    return error("this is nested too deeply", span);
                }
                let inner = self.or()?;
                self.depth -= 1;
                if self.peek() != Some(&Token::RParen) {
                    return error("a closing bracket is missing", self.here());
                }
                self.at += 1;
                Ok(inner)
            }
            Token::Dot | Token::RParen | Token::Not | Token::And | Token::Or | Token::Compare(_) => {
                error("a value is expected here", span)
            }
        }
    }
}

impl Condition {
    /// Reads a condition. Empty text is refused.
    pub fn parse(source: &str) -> Result<Condition, CondError> {
        let tokens = tokens(source)?;
        let mut parser = Parser { tokens, at: 0, end: source.len(), depth: 0 };
        let condition = parser.or()?;
        if parser.at < parser.tokens.len() {
            return error("this is not expected here", parser.here());
        }
        Ok(condition)
    }

    /// The whole condition's place in the source.
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(_, span) | Self::Word(_, span) | Self::Member { span, .. } => *span,
            Self::Not(inner, span) => Span { start: span.start, end: inner.span().end },
            Self::And(a, b) | Self::Or(a, b) | Self::Compare(_, a, b) => {
                Span { start: a.span().start, end: b.span().end }
            }
        }
    }

    /// Visits every part, outermost first.
    pub fn walk<'a>(&'a self, visit: &mut dyn FnMut(&'a Condition)) {
        visit(self);
        match self {
            Self::Literal(..) | Self::Word(..) | Self::Member { .. } => {}
            Self::Not(inner, _) => inner.walk(visit),
            Self::And(a, b) | Self::Or(a, b) | Self::Compare(_, a, b) => {
                a.walk(visit);
                b.walk(visit);
            }
        }
    }

    /// The value of a part that is not a truth by itself.
    fn value(&self, scope: &dyn CondScope) -> Result<CondValue, CondError> {
        match self {
            Self::Literal(value, _) => Ok(value.clone()),
            Self::Word(word, _) => Ok(scope.variable(word).unwrap_or_else(|| CondValue::Name(word.clone()))),
            Self::Member { object, field, span, .. } => match scope.member(object, field) {
                Some(value) => Ok(value),
                None => error(format!("`{object}.{field}` is not known"), *span),
            },
            Self::Not(..) | Self::And(..) | Self::Or(..) | Self::Compare(..) => {
                Ok(CondValue::Bool(self.eval(scope)?))
            }
        }
    }

    /// True or false for the states the scope gives. Fails where a word is not known or values of
    /// different kinds are compared.
    pub fn eval(&self, scope: &dyn CondScope) -> Result<bool, CondError> {
        let truth = |part: &Condition| match part.value(scope)? {
            CondValue::Bool(b) => Ok(b),
            CondValue::Number(_) | CondValue::Name(_) => error("this is not true or false", part.span()),
        };
        match self {
            Self::Not(inner, _) => Ok(!truth(inner)?),
            Self::And(a, b) => Ok(truth(a)? && truth(b)?),
            Self::Or(a, b) => Ok(truth(a)? || truth(b)?),
            Self::Compare(op, a, b) => {
                let (left, right) = (a.value(scope)?, b.value(scope)?);
                match (op, &left, &right) {
                    (Compare::Eq, CondValue::Bool(x), CondValue::Bool(y)) => Ok(x == y),
                    (Compare::Ne, CondValue::Bool(x), CondValue::Bool(y)) => Ok(x != y),
                    (Compare::Eq, CondValue::Name(x), CondValue::Name(y)) => Ok(x == y),
                    (Compare::Ne, CondValue::Name(x), CondValue::Name(y)) => Ok(x != y),
                    (_, CondValue::Number(x), CondValue::Number(y)) => Ok(match op {
                        Compare::Eq => (x - y).abs() <= f64::EPSILON * x.abs().max(y.abs()).max(1.0),
                        Compare::Ne => (x - y).abs() > f64::EPSILON * x.abs().max(y.abs()).max(1.0),
                        Compare::Lt => x < y,
                        Compare::Le => x <= y,
                        Compare::Gt => x > y,
                        Compare::Ge => x >= y,
                    }),
                    _ => error("these two cannot be compared", self.span()),
                }
            }
            Self::Literal(..) | Self::Word(..) | Self::Member { .. } => truth(self),
        }
    }

    /// The source with a name replaced: every bare word `old` (`object: false`) or every object named
    /// `old` before a dot (`object: true`). `None` when nothing changes.
    pub fn renamed(&self, source: &str, object: bool, old: &str, new: &str) -> Option<String> {
        let mut spans: Vec<Span> = Vec::new();
        self.walk(&mut |part| match part {
            Condition::Word(word, span) if !object && word == old => spans.push(*span),
            Condition::Member { object: name, object_span, .. } if object && name == old => {
                spans.push(*object_span);
            }
            _ => {}
        });
        if spans.is_empty() {
            return None;
        }
        spans.sort_by_key(|span| span.start);
        let mut out = String::with_capacity(source.len());
        let mut at = 0;
        for span in spans {
            out.push_str(source.get(at..span.start)?);
            out.push_str(&quoted(new));
            at = span.end;
        }
        out.push_str(source.get(at..)?);
        Some(out)
    }
}

/// True when `name` can be written in a condition without quotes: a word that is not `true`, `false`,
/// `and`, `or` or `not`.
fn is_bare_word(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && !matches!(name, "true" | "false" | "and" | "or" | "not")
}

/// A name as it is written in a condition: bare when it is a plain word, in double quotes otherwise.
pub fn quoted(name: &str) -> String {
    if is_bare_word(name) { name.to_owned() } else { format!("\"{name}\"") }
}

/// True when `name` can name a state, an object or a state variable: not empty, no double quote, no
/// control character, no space at either end.
pub fn is_valid_state_name(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && !name.chars().any(|c| c == '"' || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World;
    impl CondScope for World {
        fn variable(&self, name: &str) -> Option<CondValue> {
            match name {
                "count" => Some(CondValue::Number(2.0)),
                "armed" => Some(CondValue::Bool(true)),
                _ => None,
            }
        }
        fn member(&self, object: &str, field: &str) -> Option<CondValue> {
            (object == "홀A" && field == "state").then(|| CondValue::Name("체결됨".to_owned()))
        }
    }

    fn eval(source: &str) -> Result<bool, CondError> {
        Condition::parse(source)?.eval(&World)
    }

    #[test]
    fn conditions_compare_and_combine() {
        assert_eq!(eval("홀A.state == 체결됨 and count < 3"), Ok(true));
        assert_eq!(eval("홀A.state != 체결됨 or not armed"), Ok(false));
        assert_eq!(eval("armed && (count >= 2.0 || false)"), Ok(true));
        assert_eq!(eval("!armed"), Ok(false));
        assert_eq!(eval("not armed or armed and count == 2"), Ok(true), "not, then and, then or");
        assert_eq!(eval("count > -1"), Ok(true));
    }

    #[test]
    fn mistakes_are_refused_with_their_place() {
        let at = |source: &str| {
            let e = eval(source).unwrap_err();
            (e.span.start, e.span.end)
        };
        assert_eq!(at(""), (0, 0));
        assert_eq!(at("count <"), (7, 7));
        assert_eq!(at("count == 체결됨"), (0, "count == 체결됨".len()));
        assert_eq!(at("B.state == x"), (0, 7));
        assert_eq!(at("count"), (0, 5));
        assert_eq!(at("armed and (count < 3"), (20, 20));
        assert_eq!(at("armed armed"), (6, 11));
        assert_eq!(at("armed # 1"), (6, 7));
        assert!(eval(&"(".repeat(10_000)).is_err());
        assert!(eval(&"not ".repeat(10_000)).is_err());
    }

    #[test]
    fn names_are_renamed_where_they_are_meant() {
        let source = "홀A.state == 체결됨 and 홀A2.state != 체결됨 or count < 3";
        let parsed = Condition::parse(source).unwrap();
        assert_eq!(
            parsed.renamed(source, true, "홀A", "Hole").unwrap(),
            "Hole.state == 체결됨 and 홀A2.state != 체결됨 or count < 3"
        );
        assert_eq!(
            parsed.renamed(source, false, "체결됨", "Done").unwrap(),
            "홀A.state == Done and 홀A2.state != Done or count < 3"
        );
        assert_eq!(parsed.renamed(source, false, "state", "x"), None);
        assert!(
            is_valid_state_name("체결 완료") && !is_valid_state_name(" x") && !is_valid_state_name("a\"b")
        );
        // Names with spaces are quoted, in the source and when a rename writes them.
        let source = "\"홀 A\".state == \"체결 완료\" and not \"true\".state == x";
        let parsed = Condition::parse(source).unwrap();
        assert_eq!(
            parsed.renamed(source, true, "홀 A", "HoleA").unwrap(),
            "HoleA.state == \"체결 완료\" and not \"true\".state == x"
        );
        assert_eq!(
            parsed.renamed(source, false, "체결 완료", "done now").unwrap(),
            "\"홀 A\".state == \"done now\" and not \"true\".state == x"
        );
        assert!(Condition::parse("\"open").is_err() && Condition::parse("\"\" == x").is_err());
    }
}
