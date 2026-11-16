//! Licence check: every dependency must carry an allowed licence and come from crates.io.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::meta;

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// Contents of `licences.toml`.
#[derive(Debug, Deserialize)]
pub struct Policy {
    /// SPDX licence identifiers that are acceptable.
    pub allowed: Vec<String>,
    /// SPDX exception identifiers acceptable after `WITH`.
    #[serde(default)]
    pub allowed_exceptions: Vec<String>,
    /// Crates that must never appear, whatever their licence field says.
    #[serde(default)]
    pub banned_crates: Vec<String>,
    /// Target triples whose dependency graphs are checked.
    pub targets: Vec<String>,
    /// Crates without a machine-readable licence that were reviewed by hand: name -> reason.
    #[serde(default)]
    pub reviewed: BTreeMap<String, String>,
}

/// Evaluates an SPDX expression: `OR` needs one acceptable side, `AND` needs both.
///
/// Returns an error for text that is not a licence expression.
pub fn acceptable(expression: &str, policy: &Policy) -> Result<bool, String> {
    // Old manifests write `MIT/Apache-2.0` for `MIT OR Apache-2.0`.
    let spaced = expression.replace('/', " OR ").replace('(', " ( ").replace(')', " ) ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut position = 0;
    let verdict = parse_or(&tokens, &mut position, policy)?;
    match tokens.get(position) {
        None => Ok(verdict),
        Some(extra) => Err(format!("unexpected `{extra}` in licence expression `{expression}`")),
    }
}

fn parse_or(tokens: &[&str], position: &mut usize, policy: &Policy) -> Result<bool, String> {
    let mut verdict = parse_and(tokens, position, policy)?;
    while tokens.get(*position).is_some_and(|t| t.eq_ignore_ascii_case("OR")) {
        *position += 1;
        // Evaluate the right side first so a parse error is never short-circuited away.
        let right = parse_and(tokens, position, policy)?;
        verdict = verdict || right;
    }
    Ok(verdict)
}

fn parse_and(tokens: &[&str], position: &mut usize, policy: &Policy) -> Result<bool, String> {
    let mut verdict = parse_atom(tokens, position, policy)?;
    while tokens.get(*position).is_some_and(|t| t.eq_ignore_ascii_case("AND")) {
        *position += 1;
        let right = parse_atom(tokens, position, policy)?;
        verdict = verdict && right;
    }
    Ok(verdict)
}

fn parse_atom(tokens: &[&str], position: &mut usize, policy: &Policy) -> Result<bool, String> {
    let token = *tokens.get(*position).ok_or("licence expression ends unexpectedly")?;
    *position += 1;
    if token == "(" {
        let verdict = parse_or(tokens, position, policy)?;
        return match tokens.get(*position) {
            Some(&")") => {
                *position += 1;
                Ok(verdict)
            }
            _ => Err("missing `)` in licence expression".to_owned()),
        };
    }
    if token == ")" || ["OR", "AND", "WITH"].iter().any(|k| token.eq_ignore_ascii_case(k)) {
        return Err(format!("unexpected `{token}` in licence expression"));
    }
    let mut verdict = policy.allowed.iter().any(|l| l == token);
    if tokens.get(*position).is_some_and(|t| t.eq_ignore_ascii_case("WITH")) {
        let exception = *tokens.get(*position + 1).ok_or("`WITH` without an exception name")?;
        *position += 2;
        verdict = verdict && policy.allowed_exceptions.iter().any(|e| e == exception);
    }
    Ok(verdict)
}

/// Checks the dependency graphs of every target in `licences.toml`.
pub fn check(root: &Path) -> Result<()> {
    let text = std::fs::read_to_string(root.join("licences.toml")).context("could not read licences.toml")?;
    let policy: Policy = toml::from_str(&text).context("could not parse licences.toml")?;

    let mut problems = BTreeSet::new();
    let mut checked = BTreeSet::new();
    for target in &policy.targets {
        let metadata = meta::load(root, Some(target))?;
        let resolved = metadata.resolved_ids();
        for package in metadata.packages.iter().filter(|p| resolved.contains(p.id.as_str())) {
            let label = format!("{} {}", package.name, package.version);
            if !checked.insert(label.clone()) {
                continue;
            }
            if policy.banned_crates.contains(&package.name) {
                problems.insert(format!("{label}: banned crate"));
            }
            let is_workspace_member = metadata.workspace_members.contains(&package.id);
            if !is_workspace_member && package.source.as_deref() != Some(CRATES_IO) {
                problems.insert(format!("{label}: source is {:?}, not crates.io", package.source));
            }
            match (&package.license, policy.reviewed.contains_key(&package.name)) {
                (_, true) => {}
                (Some(expression), false) => match acceptable(expression, &policy) {
                    Ok(true) => {}
                    Ok(false) => {
                        problems.insert(format!("{label}: licence `{expression}` is not allowed"));
                    }
                    Err(reason) => {
                        problems.insert(format!("{label}: {reason}"));
                    }
                },
                (None, false) => {
                    problems
                        .insert(format!("{label}: no licence field; review it and list it under [reviewed]"));
                }
            }
        }
    }
    if !problems.is_empty() {
        let lines: Vec<String> = problems.into_iter().collect();
        bail!("licence problems:\n  {}", lines.join("\n  "));
    }
    println!(
        "check-licences: {} packages across {} targets, no problems",
        checked.len(),
        policy.targets.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        Policy {
            allowed: ["MIT", "Apache-2.0", "BSD-3-Clause", "Zlib"].map(str::to_owned).to_vec(),
            allowed_exceptions: vec!["LLVM-exception".to_owned()],
            banned_crates: Vec::new(),
            targets: Vec::new(),
            reviewed: BTreeMap::new(),
        }
    }

    #[test]
    fn single_licences() {
        assert_eq!(acceptable("MIT", &policy()), Ok(true));
        assert_eq!(acceptable("MPL-2.0", &policy()), Ok(false));
    }

    #[test]
    fn or_needs_one_side_and_needs_both() {
        assert_eq!(acceptable("MPL-2.0 OR MIT", &policy()), Ok(true));
        assert_eq!(acceptable("MIT/Apache-2.0", &policy()), Ok(true));
        assert_eq!(acceptable("MIT AND MPL-2.0", &policy()), Ok(false));
        assert_eq!(acceptable("GPL-3.0-only OR LGPL-3.0-only", &policy()), Ok(false));
    }

    #[test]
    fn and_binds_tighter_than_or() {
        assert_eq!(acceptable("MIT OR Apache-2.0 AND MPL-2.0", &policy()), Ok(true));
        assert_eq!(acceptable("(MIT OR Apache-2.0) AND MPL-2.0", &policy()), Ok(false));
        assert_eq!(acceptable("(MIT OR Apache-2.0) AND Zlib", &policy()), Ok(true));
    }

    #[test]
    fn exceptions_must_be_allowed_too() {
        assert_eq!(acceptable("Apache-2.0 WITH LLVM-exception", &policy()), Ok(true));
        assert_eq!(acceptable("Apache-2.0 WITH Classpath-exception-2.0", &policy()), Ok(false));
        assert_eq!(acceptable("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT", &policy()), Ok(true));
    }

    #[test]
    fn malformed_expressions_are_errors() {
        assert!(acceptable("", &policy()).is_err());
        assert!(acceptable("MIT OR", &policy()).is_err());
        assert!(acceptable("(MIT", &policy()).is_err());
        assert!(acceptable("MIT Apache-2.0", &policy()).is_err());
        assert!(acceptable("MIT WITH", &policy()).is_err());
    }
}
