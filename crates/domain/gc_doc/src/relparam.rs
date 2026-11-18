//! Model parameters of assembly constraints.
//!
//! Inventor 2026 help, About Parameters in Models (GUID-8E5C63EA-0540-4EBE-9B17-B8159C7FE40C): "Model
//! parameters are created automatically when you define a sketch dimension or other measurement, add an
//! assembly constraint (Inventor Professional), or create a feature. These parameters are given default
//! names, such as d0, d1, d2, but you can override the default names with more meaningful names. You can
//! enter expressions in edit boxes that define the parameter names and values." To Work with Parameters
//! in Models (GUID-595F9D8E-9EEA-4446-91AB-830E5BAE548E): "If you entered a dimension or other value in
//! an edit box, the value is listed here."
//!
//! The offset of a Mate, Flush, Tangent or Insert constraint and the angle of an Angle constraint each
//! have a parameter: a name (shared number space with sketch dimensions), the text as typed and the value
//! that text gave. Parameters and dimensions may name it; it may name them, and then follows them.

use gc_core::Id;
use gc_expr::Unit;
use serde::{Deserialize, Serialize};

use crate::command::{
    evaluate, name_free, named_source, names_in, propagate, rename_in, rewrite_references, round_for_display,
    scope_of,
};
use crate::relate::{RelationKind, Relationship, value_of, with_value};
use crate::{Created, Document, EditError, Txn};

/// The model parameter of a constraint's offset or angle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationParam {
    /// Its name (`d7`, or what the user called it).
    pub name: String,
    /// The value as typed: a number, a unit, an expression.
    pub source: String,
    /// What the text evaluated to: metres or radians.
    pub value: f64,
}

/// The unit a bare number means in the constraint's value box: a length or an angle.
pub(crate) fn field_of(kind: &RelationKind) -> Unit {
    if matches!(kind, RelationKind::Angle { .. }) { Unit::Deg } else { Unit::Mm }
}

/// A value written out in the document's unit, e.g. `"12.5 mm"`.
fn written(doc: &Document, kind: &RelationKind, value: f64) -> String {
    let settings = doc.settings();
    let unit = if field_of(kind) == Unit::Deg { settings.angle() } else { settings.length() };
    format!("{} {}", round_for_display(value / unit.si_factor()), unit.word())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

impl Document {
    /// The model parameter of a constraint's offset or angle; `None` for a relationship without a value
    /// (joints, Symmetry).
    pub fn relation_param(&self, id: Id<Relationship>) -> Option<&RelationParam> {
        self.timeline().and_then(|(_, t)| t.components.relation_params.get(&id))
    }

    /// Every constraint parameter, in the order the constraints were made: the rows of the Parameters
    /// table's model parameters that come from the assembly. The flag is true for an angle.
    pub fn relation_params(&self) -> Vec<(Id<Relationship>, &RelationParam, bool)> {
        let Some((_, timeline)) = self.timeline() else { return Vec::new() };
        timeline
            .components
            .relation_params
            .iter()
            .filter_map(|(id, p)| {
                let kind = self.get(*id)?.kind;
                Some((*id, p, field_of(&kind) == Unit::Deg))
            })
            .collect()
    }
}

/// True when a constraint parameter other than `except` has the name.
pub(crate) fn name_taken(doc: &Document, name: &str, except: Option<Id<Relationship>>) -> bool {
    doc.timeline().is_some_and(|(_, t)| {
        t.components.relation_params.iter().any(|(id, p)| p.name == name && Some(*id) != except)
    })
}

/// Keeps the parameters in step with the constraints after a command: a constraint with a value and no
/// parameter gets the next free `dN`; one whose value was set as a number has its text written anew; the
/// parameter of a constraint that is gone (or lost its value) goes, and expressions that named it take
/// its last value instead.
pub(crate) fn sync(txn: &mut Txn<'_>) -> Result<(), EditError> {
    let doc = txn.doc();
    let valued: Vec<(Id<Relationship>, RelationKind, f64)> = doc
        .relationships()
        .iter()
        .filter_map(|(id, r)| value_of(&r.kind).map(|value| (id, r.kind, value)))
        .collect();
    let existing = doc.timeline().map(|(_, t)| t.components.relation_params.clone()).unwrap_or_default();
    let gone: Vec<(Id<Relationship>, RelationParam)> = existing
        .iter()
        .filter(|(id, _)| valued.iter().all(|(other, ..)| other != *id))
        .map(|(id, p)| (*id, p.clone()))
        .collect();
    let stale: Vec<(Id<Relationship>, String, f64)> = valued
        .iter()
        .filter(|(id, _, value)| existing.get(id).is_some_and(|p| p.value.to_bits() != value.to_bits()))
        .map(|(id, kind, value)| (*id, written(doc, kind, *value), *value))
        .collect();
    let first = crate::command::highest_default_name(doc).map_or(0, |h| h + 1);
    let fresh: Vec<(Id<Relationship>, RelationParam)> = (first..)
        .zip(valued.iter().filter(|(id, ..)| !existing.contains_key(id)))
        .map(|(next, (id, kind, value))| {
            let source = written(doc, kind, *value);
            (*id, RelationParam { name: format!("d{next}"), source, value: *value })
        })
        .collect();
    if gone.is_empty() && stale.is_empty() && fresh.is_empty() {
        return Ok(());
    }
    // What named a parameter that goes keeps its size.
    for (id, param) in &gone {
        let angle = txn.doc().get(*id).is_some_and(|r| field_of(&r.kind) == Unit::Deg) || param.angle_text();
        let settings = txn.doc().settings();
        let unit = if angle { settings.angle() } else { settings.length() };
        let text = format!("({} {})", round_for_display(param.value / unit.si_factor()), unit.word());
        rewrite_references(txn, &param.name, &text)?;
    }
    crate::cell::set_timeline(txn, |t| {
        let params = &mut t.components.relation_params;
        for (id, _) in &gone {
            params.remove(id);
        }
        for (id, source, value) in stale {
            if let Some(p) = params.get_mut(&id) {
                p.source = source;
                p.value = value;
            }
        }
        params.extend(fresh);
    })
}

impl RelationParam {
    /// True when the text itself says it is an angle (the constraint is gone and cannot be asked).
    fn angle_text(&self) -> bool {
        let text = self.source.trim_end();
        text.ends_with("deg") || text.ends_with("rad")
    }
}

/// Changes a constraint's offset or angle to what `source` says: a number, a unit, an expression with
/// parameter and dimension names, or `NAME = expression` to name the parameter as well.
pub(crate) fn set_value(
    txn: &mut Txn<'_>,
    id: Id<Relationship>,
    source: &str,
) -> Result<(Vec<Created>, &'static str), EditError> {
    let relationship = txn.doc().get(id).ok_or(crate::DocError::Missing(id.erase()))?.clone();
    if value_of(&relationship.kind).is_none() {
        return Err(EditError::BadValue);
    }
    let (name, source) = match named_source(source) {
        Some((name, rest)) => (Some(name), rest),
        None => (None, source.trim().to_owned()),
    };
    if let Some(name) = &name {
        rename(txn, id, name)?;
    }
    let value = evaluate(&source, &scope_of(txn.doc()), field_of(&relationship.kind))?;
    let changed = Relationship { kind: with_value(&relationship.kind, value), ..relationship };
    if !value.is_finite() || !changed.is_valid() {
        return Err(EditError::BadValue);
    }
    let before = txn.doc().protected_freedom();
    let kind = changed.kind;
    txn.modify(id, |r: &mut Relationship, _| r.kind = kind)?;
    let fallback = txn.doc().relation_param(id).map(|p| p.name.clone());
    let name = match fallback {
        Some(name) => name,
        None => format!("d{}", crate::command::highest_default_name(txn.doc()).map_or(0, |h| h + 1)),
    };
    crate::cell::set_timeline(txn, |t| {
        t.components.relation_params.insert(id, RelationParam { name, source, value });
    })?;
    // What names this parameter follows; a loop is refused.
    propagate(txn)?;
    crate::cell::settle(txn)?;
    crate::cell::check_protected(txn, &before)?;
    Ok((Vec::new(), "Edit constraint"))
}

/// Renames a constraint's parameter and every reference to it.
pub(crate) fn rename(txn: &mut Txn<'_>, id: Id<Relationship>, name: &str) -> Result<(), EditError> {
    let name = name.trim();
    let relationship = txn.doc().get(id).ok_or(crate::DocError::Missing(id.erase()))?;
    let Some(value) = value_of(&relationship.kind) else { return Err(EditError::BadValue) };
    let kind = relationship.kind;
    let old = txn.doc().relation_param(id).map(|p| p.name.clone());
    if old.as_deref() == Some(name) {
        return Ok(());
    }
    if !gc_expr::is_valid_param_name(name) || !name_free(txn.doc(), name, None) {
        return Err(EditError::BadName(name.to_owned()));
    }
    if let Some(old) = &old {
        rewrite_references(txn, old, name)?;
    }
    let source = written(txn.doc(), &kind, value);
    crate::cell::set_timeline(txn, |t| {
        let entry = t.components.relation_params.entry(id).or_insert(RelationParam {
            name: String::new(),
            source,
            value,
        });
        name.clone_into(&mut entry.name);
    })
}

/// The constraint parameters whose text names something, re-evaluated: the changed values, to be written
/// by [`write_values`].
pub(crate) fn reevaluated(doc: &Document, id: Id<Relationship>) -> Result<Option<f64>, EditError> {
    let (Some(param), Some(relationship)) = (doc.relation_param(id), doc.get(id)) else { return Ok(None) };
    // Plain numbers stay as they are (their text may be rounded).
    if names_in(&param.source).is_empty() {
        return Ok(None);
    }
    let value = evaluate(&param.source, &scope_of(doc), field_of(&relationship.kind))?;
    if !value.is_finite() {
        return Err(EditError::BadValue);
    }
    Ok((!close(value, param.value)).then_some(value))
}

/// Writes a re-evaluated value into the constraint and its parameter.
pub(crate) fn write_value(txn: &mut Txn<'_>, id: Id<Relationship>, value: f64) -> Result<(), EditError> {
    txn.modify(id, |r: &mut Relationship, _| r.kind = with_value(&r.kind, value))?;
    crate::cell::set_timeline(txn, |t| {
        if let Some(p) = t.components.relation_params.get_mut(&id) {
            p.value = value;
        }
    })
}

/// `source` of every constraint parameter with references to `old` renamed to `new`.
pub(crate) fn rewrite(txn: &mut Txn<'_>, old: &str, new: &str) -> Result<(), EditError> {
    let changed: Vec<(Id<Relationship>, String)> = txn
        .doc()
        .timeline()
        .map(|(_, t)| {
            t.components
                .relation_params
                .iter()
                .filter_map(|(id, p)| rename_in(&p.source, old, new).map(|source| (*id, source)))
                .collect()
        })
        .unwrap_or_default();
    if changed.is_empty() {
        return Ok(());
    }
    crate::cell::set_timeline(txn, |t| {
        for (id, source) in changed {
            if let Some(p) = t.components.relation_params.get_mut(&id) {
                p.source = source;
            }
        }
    })
}

/// Keeps every constraint's size when the document's units change: a text that would mean something
/// else in the new units is written out in the old ones.
pub(crate) fn keep_sizes(txn: &mut Txn<'_>, after: &dyn gc_expr::Scope) -> Result<(), EditError> {
    let doc = txn.doc();
    let rewrites: Vec<(Id<Relationship>, String)> = doc
        .relation_params()
        .into_iter()
        .filter_map(|(id, param, _)| {
            let kind = doc.get(id)?.kind;
            let same = evaluate(&param.source, after, field_of(&kind)).is_ok_and(|v| close(v, param.value));
            (!same).then(|| (id, written(doc, &kind, param.value)))
        })
        .collect();
    if rewrites.is_empty() {
        return Ok(());
    }
    crate::cell::set_timeline(txn, |t| {
        for (id, source) in rewrites {
            if let Some(p) = t.components.relation_params.get_mut(&id) {
                p.source = source;
            }
        }
    })
}
