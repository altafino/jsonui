//! `toml_edit` tree to the neutral value, and the secret lexemes.

use toml_edit::{Item, Table, Value};

use crate::core::patch::escape_segment;

use super::super::SecretPredicate;
use super::super::value::Val;

pub(super) fn table_val(table: &Table) -> Val {
    Val::Object(
        table
            .iter()
            .filter_map(|(k, item)| Some((k.to_string(), item_val(item)?)))
            .collect(),
    )
}

pub(super) fn item_val(item: &Item) -> Option<Val> {
    match item {
        Item::None => None,
        Item::Value(v) => Some(value_val(v)),
        Item::Table(t) => Some(table_val(t)),
        Item::ArrayOfTables(a) => Some(Val::Array(a.iter().map(table_val).collect())),
    }
}

pub(super) fn value_val(value: &Value) -> Val {
    match value {
        Value::String(f) => Val::String(f.value().clone()),
        Value::Integer(f) => Val::Number(f.display_repr().into_owned()),
        Value::Float(f) => Val::Number(f.display_repr().into_owned()),
        Value::Boolean(f) => Val::Bool(*f.value()),
        Value::Datetime(f) => Val::String(f.display_repr().into_owned()),
        Value::Array(a) => Val::Array(a.iter().map(value_val).collect()),
        Value::InlineTable(t) => Val::Object(
            t.iter()
                .map(|(k, v)| (k.to_string(), value_val(v)))
                .collect(),
        ),
    }
}

/// Objects compare as maps (TOML cannot keep key order across tables and
/// key/values); arrays compare in order; numbers by lexeme.
pub(super) fn same_data(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Object(x), Val::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && same_data(v, v2)))
        }
        (Val::Array(x), Val::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_data(p, q))
        }
        _ => a == b,
    }
}

// ------------------------------------------------------------ secret lexemes

pub(super) fn secret_lexemes(table: &Table, secret: SecretPredicate) -> Vec<String> {
    let mut out = Vec::new();
    walk_table(table, "", false, secret, &mut out);
    out
}

fn walk_table(
    table: &Table,
    pointer: &str,
    inherited: bool,
    secret: SecretPredicate,
    out: &mut Vec<String>,
) {
    for (k, item) in table.iter() {
        let p = format!("{pointer}/{}", escape_segment(k));
        walk_item(item, &p, inherited, secret, out);
    }
}

fn walk_item(
    item: &Item,
    pointer: &str,
    inherited: bool,
    secret: SecretPredicate,
    out: &mut Vec<String>,
) {
    let here = inherited || secret(pointer);
    match item {
        Item::None => {}
        Item::Value(v) => walk_value(v, pointer, here, secret, out),
        Item::Table(t) => walk_table(t, pointer, here, secret, out),
        Item::ArrayOfTables(a) => {
            for (i, t) in a.iter().enumerate() {
                walk_table(t, &format!("{pointer}/{i}"), here, secret, out);
            }
        }
    }
}

fn walk_value(
    value: &Value,
    pointer: &str,
    inherited: bool,
    secret: SecretPredicate,
    out: &mut Vec<String>,
) {
    let here = inherited || secret(pointer);
    match value {
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                walk_value(v, &format!("{pointer}/{i}"), here, secret, out);
            }
        }
        Value::InlineTable(t) => {
            for (k, v) in t.iter() {
                let p = format!("{pointer}/{}", escape_segment(k));
                walk_value(v, &p, here, secret, out);
            }
        }
        Value::Boolean(_) => {}
        scalar if here => push_scalar(scalar, out),
        _ => {}
    }
}

fn push_scalar(value: &Value, out: &mut Vec<String>) {
    let raw = match value {
        Value::String(f) => f.display_repr().into_owned(),
        Value::Integer(f) => f.display_repr().into_owned(),
        Value::Float(f) => f.display_repr().into_owned(),
        Value::Datetime(f) => f.display_repr().into_owned(),
        _ => return,
    };
    if raw.starts_with("\"\"\"") || raw.starts_with("'''") {
        // A multi-line string spans diff lines: each content line is its own lexeme.
        let body = &raw[3..raw.len().saturating_sub(3).max(3)];
        out.extend(
            body.lines()
                .map(|l| l.trim_end_matches('\\').trim())
                .filter(|l| !l.is_empty())
                .map(str::to_string),
        );
    } else if raw.trim_matches(['"', '\'']).is_empty() {
        // An empty string hides nothing.
    } else {
        out.push(raw);
    }
}
