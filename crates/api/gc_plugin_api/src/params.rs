//! Parameters of a planner or importer, described as data so the application can build an editor for them
//! and store the chosen values in the document.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Text;

/// A parameter value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    /// On or off.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A real number.
    Float(f64),
    /// One of a fixed set of options.
    Choice(String),
}

/// Parameter values by key.
pub type ParamValues = BTreeMap<String, ParamValue>;

/// The type, default and range of one parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamKind {
    /// On or off.
    Bool {
        /// Value when the user sets nothing.
        default: bool,
    },
    /// A whole number in `min..=max`.
    Int {
        /// Value when the user sets nothing.
        default: i64,
        /// Smallest allowed value.
        min: i64,
        /// Largest allowed value.
        max: i64,
    },
    /// A real number in `min..=max`.
    Float {
        /// Value when the user sets nothing.
        default: f64,
        /// Smallest allowed value.
        min: f64,
        /// Largest allowed value.
        max: f64,
    },
    /// One of `options`.
    Choice {
        /// Value when the user sets nothing.
        default: String,
        /// The allowed values.
        options: Vec<String>,
    },
}

/// One parameter of a schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Key under which the value is stored.
    pub key: String,
    /// Name shown to the user.
    pub label: Text,
    /// Type, default and range.
    pub kind: ParamKind,
}

/// The parameters something accepts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamSchema {
    /// The parameters, in the order an editor shows them.
    pub params: Vec<ParamSpec>,
}

/// Why values do not fit a schema.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ParamError {
    /// A value for a key the schema does not have.
    #[error("unknown parameter `{0}`")]
    UnknownKey(String),
    /// A value of the wrong type.
    #[error("parameter `{0}` has the wrong type")]
    WrongType(String),
    /// A number outside its range, or a choice that is not an option.
    #[error("parameter `{0}` is out of range")]
    OutOfRange(String),
}

impl ParamKind {
    fn default_value(&self) -> ParamValue {
        match self {
            Self::Bool { default } => ParamValue::Bool(*default),
            Self::Int { default, .. } => ParamValue::Int(*default),
            Self::Float { default, .. } => ParamValue::Float(*default),
            Self::Choice { default, .. } => ParamValue::Choice(default.clone()),
        }
    }

    fn check(&self, key: &str, value: &ParamValue) -> Result<(), ParamError> {
        let in_range = match (self, value) {
            (Self::Bool { .. }, ParamValue::Bool(_)) => true,
            (Self::Int { min, max, .. }, ParamValue::Int(v)) => (min..=max).contains(&v),
            (Self::Float { min, max, .. }, ParamValue::Float(v)) => v >= min && v <= max,
            (Self::Choice { options, .. }, ParamValue::Choice(v)) => options.contains(v),
            _ => return Err(ParamError::WrongType(key.to_owned())),
        };
        if in_range { Ok(()) } else { Err(ParamError::OutOfRange(key.to_owned())) }
    }
}

impl ParamSchema {
    /// The default value of every parameter.
    pub fn defaults(&self) -> ParamValues {
        self.params.iter().map(|spec| (spec.key.clone(), spec.kind.default_value())).collect()
    }

    /// Checks `values` against the schema and fills in defaults for keys that are missing.
    pub fn complete(&self, values: &ParamValues) -> Result<ParamValues, ParamError> {
        if let Some(unknown) = values.keys().find(|key| !self.params.iter().any(|spec| &spec.key == *key)) {
            return Err(ParamError::UnknownKey(unknown.clone()));
        }
        let mut complete = self.defaults();
        for spec in &self.params {
            if let Some(value) = values.get(&spec.key) {
                spec.kind.check(&spec.key, value)?;
                complete.insert(spec.key.clone(), value.clone());
            }
        }
        Ok(complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> ParamSchema {
        let spec = |key: &str, kind| ParamSpec { key: key.to_owned(), label: Text::new(key, key), kind };
        ParamSchema {
            params: vec![
                spec("range", ParamKind::Float { default: 0.5, min: 0.01, max: 3.0 }),
                spec("max_nodes", ParamKind::Int { default: 10_000, min: 1, max: 1_000_000 }),
                spec("bias", ParamKind::Bool { default: true }),
                spec(
                    "metric",
                    ParamKind::Choice { default: "l2".into(), options: vec!["l2".into(), "linf".into()] },
                ),
            ],
        }
    }

    #[test]
    fn missing_values_take_defaults() {
        let given = ParamValues::from([("range".to_owned(), ParamValue::Float(1.0))]);
        let complete = schema().complete(&given).unwrap();
        assert_eq!(complete.len(), 4);
        assert_eq!(complete["range"], ParamValue::Float(1.0));
        assert_eq!(complete["max_nodes"], ParamValue::Int(10_000));
        assert_eq!(schema().complete(&ParamValues::new()).unwrap(), schema().defaults());
    }

    #[test]
    fn bad_values_are_rejected_with_the_key() {
        let check = |key: &str, value| schema().complete(&ParamValues::from([(key.to_owned(), value)]));
        assert_eq!(check("nope", ParamValue::Bool(true)), Err(ParamError::UnknownKey("nope".into())));
        assert_eq!(check("range", ParamValue::Int(1)), Err(ParamError::WrongType("range".into())));
        assert_eq!(check("range", ParamValue::Float(0.0)), Err(ParamError::OutOfRange("range".into())));
        assert_eq!(check("range", ParamValue::Float(f64::NAN)), Err(ParamError::OutOfRange("range".into())));
        assert_eq!(check("max_nodes", ParamValue::Int(0)), Err(ParamError::OutOfRange("max_nodes".into())));
        assert_eq!(
            check("metric", ParamValue::Choice("l1".into())),
            Err(ParamError::OutOfRange("metric".into()))
        );
    }

    #[test]
    fn values_round_trip_through_json() {
        let values = schema().defaults();
        let json = serde_json::to_string(&values).unwrap();
        assert_eq!(serde_json::from_str::<ParamValues>(&json).unwrap(), values);
    }
}
