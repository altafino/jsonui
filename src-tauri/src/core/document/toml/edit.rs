//! Tree surgery on a `toml_edit` document. Every function edits a *clone*;
//! the caller renders, re-parses and checks the result before keeping it.

use toml_edit::{
    Array, ArrayOfTables, Decor, DocumentMut, InlineTable, Item, Key, Table, TableLike, Value,
};

use crate::core::{CoreError, CoreResult};

use super::super::value::Val;
use super::layout;
use super::write::{StrStyle, datetime_value, invalid, to_value};

fn missing() -> CoreError {
    invalid("The path does not exist in the document")
}

fn index(seg: &str) -> Option<usize> {
    seg.parse::<usize>().ok().filter(|i| i.to_string() == seg)
}

// ------------------------------------------------------------- navigation

/// A node of the document, wherever it lives.
enum N<'a> {
    Item(&'a mut Item),
    Val(&'a mut Value),
    Tab(&'a mut Table),
}

fn descend<'a>(node: N<'a>, seg: &str) -> Option<N<'a>> {
    match node {
        N::Item(Item::Table(t)) | N::Tab(t) => t.get_mut(seg).map(N::Item),
        N::Item(Item::ArrayOfTables(a)) => a.get_mut(index(seg)?).map(N::Tab),
        N::Item(Item::Value(v)) | N::Val(v) => match v {
            Value::InlineTable(t) => t.get_mut(seg).map(N::Val),
            Value::Array(a) => a.get_mut(index(seg)?).map(N::Val),
            _ => None,
        },
        N::Item(Item::None) => None,
    }
}

fn resolve<'a>(doc: &'a mut DocumentMut, path: &[String]) -> Option<N<'a>> {
    let mut node = N::Item(doc.as_item_mut());
    for seg in path {
        node = descend(node, seg)?;
    }
    Some(node)
}

fn table_like(node: N<'_>) -> Option<&mut dyn TableLike> {
    match node {
        N::Item(Item::Table(t)) | N::Tab(t) => Some(t),
        N::Item(Item::Value(Value::InlineTable(t))) | N::Val(Value::InlineTable(t)) => Some(t),
        _ => None,
    }
}

fn array_of(node: N<'_>) -> Option<&mut Array> {
    match node {
        N::Item(Item::Value(Value::Array(a))) | N::Val(Value::Array(a)) => Some(a),
        _ => None,
    }
}

// ----------------------------------------------------------- small helpers

fn prefix_of(decor: &Decor) -> String {
    decor
        .prefix()
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string()
}

fn is_blank(s: &str) -> bool {
    s.chars().all(char::is_whitespace)
}

/// The spacing of a prefix without its comments: a newline plus indentation
/// when it had line breaks, else its blanks (or one space).
fn layout_prefix(prefix: &str) -> String {
    match prefix.rfind('\n') {
        Some(i) => format!("\n{}", &prefix[i + 1..]),
        None if is_blank(prefix) => prefix.to_string(),
        None => " ".to_string(),
    }
}

/// A neighbour's decor, minus its comments, for a value placed next to it.
fn layout_decor(of: &Decor) -> Decor {
    let suffix = of.suffix().and_then(|s| s.as_str()).unwrap_or("");
    Decor::new(
        layout_prefix(&prefix_of(of)),
        if is_blank(suffix) { suffix } else { "" },
    )
}

fn table_from(members: &[(String, Val)], crlf: bool) -> CoreResult<Table> {
    let mut table = Table::new();
    for (k, v) in members {
        table.insert(k, Item::Value(to_value(v, None, crlf)?));
    }
    Ok(table)
}

fn all_objects(items: &[Val]) -> bool {
    !items.is_empty() && items.iter().all(|v| matches!(v, Val::Object(_)))
}

/// The value to write over `old`: same string style, datetimes stay datetimes.
fn replacement(old: &Value, new: &Val, crlf: bool) -> CoreResult<Value> {
    let mut style = None;
    match (old, new) {
        (Value::Datetime(_), Val::String(s)) => {
            if let Some(dt) = datetime_value(s) {
                return Ok(dt);
            }
        }
        (Value::String(f), Val::String(_)) => style = Some(StrStyle::of(&f.display_repr())),
        _ => {}
    }
    to_value(new, style, crlf)
}

fn with_decor_of(mut new: Value, old: &Value) -> Value {
    *new.decor_mut() = old.decor().clone();
    new
}

// -------------------------------------------------------------------- set

pub(super) fn set(doc: &mut DocumentMut, path: &[String], new: &Val, crlf: bool) -> CoreResult<()> {
    let Some((last, parent_path)) = path.split_last() else {
        let Val::Object(members) = new else {
            return Err(invalid("The TOML document root must be a table"));
        };
        let root = doc.as_table_mut();
        root.clear();
        for (k, v) in members {
            root.insert(k, Item::Value(to_value(v, None, crlf)?));
        }
        return Ok(());
    };
    // An implicit table gets a header of its own once it holds something:
    // fix the print order first so that header lands where the table is.
    layout::normalize(doc);
    let parent = resolve(doc, parent_path).ok_or_else(missing)?;
    // The path whose key must read like `key = value` afterwards, if a header turned into one.
    let rekey = match parent {
        N::Item(item @ Item::ArrayOfTables(_)) => {
            set_in_aot(item, index(last), new, crlf)?.then_some(parent_path)
        }
        parent @ (N::Item(Item::Value(Value::Array(_))) | N::Val(Value::Array(_))) => {
            let array = array_of(parent).ok_or_else(missing)?;
            let i = index(last)
                .filter(|i| *i < array.len())
                .ok_or_else(missing)?;
            let old = array.get(i).ok_or_else(missing)?;
            let value = with_decor_of(replacement(old, new, crlf)?, old);
            array.replace_formatted(i, value);
            None
        }
        parent => {
            let table = table_like(parent).ok_or_else(missing)?;
            set_in_table(table, last, new, crlf)?.then_some(path)
        }
    };
    if let Some(p) = rekey {
        kv_key(doc, p);
    }
    Ok(())
}

/// Spacing of a key that now stands in a `key = value` line.
fn kv_key(doc: &mut DocumentMut, path: &[String]) {
    let Some((key, parent)) = path.split_last() else {
        return;
    };
    if let Some(table) = resolve(doc, parent).and_then(table_like)
        && let Some(mut k) = table.key_mut(key)
    {
        *k.leaf_decor_mut() = Decor::new("", " ");
    }
}

/// Returns whether a header block or array of tables became a plain value.
fn set_in_table(table: &mut dyn TableLike, key: &str, new: &Val, crlf: bool) -> CoreResult<bool> {
    let slot = table.get_mut(key).ok_or_else(missing)?;
    match slot {
        Item::Value(old) => {
            let value = with_decor_of(replacement(old, new, crlf)?, old);
            *old = value;
        }
        Item::Table(t) if !t.is_dotted() && matches!(new, Val::Object(_)) => {
            if let Val::Object(members) = new {
                refill(t, members, crlf)?;
            }
        }
        // A header block, dotted keys or an array of tables become one inline value.
        Item::Table(_) | Item::ArrayOfTables(_) => {
            *slot = Item::Value(to_value(new, None, crlf)?);
            return Ok(true);
        }
        Item::None => return Err(missing()),
    }
    Ok(false)
}

fn refill(table: &mut Table, members: &[(String, Val)], crlf: bool) -> CoreResult<()> {
    table.clear();
    table.set_implicit(false);
    for (k, v) in members {
        table.insert(k, Item::Value(to_value(v, None, crlf)?));
    }
    Ok(())
}

/// Returns whether the array of tables became an inline array.
fn set_in_aot(item: &mut Item, i: Option<usize>, new: &Val, crlf: bool) -> CoreResult<bool> {
    let i = i.ok_or_else(missing)?;
    if let (Val::Object(members), Item::ArrayOfTables(aot)) = (new, &mut *item) {
        let table = aot.get_mut(i).ok_or_else(missing)?;
        refill(table, members, crlf)?;
        return Ok(false);
    }
    // A non-table element: the whole array becomes an inline array.
    let mut array = inline_array(item)?;
    let old = array.get(i).ok_or_else(missing)?;
    let value = with_decor_of(replacement(old, new, crlf)?, old);
    array.replace_formatted(i, value);
    *item = Item::Value(Value::Array(array));
    Ok(true)
}

fn inline_array(item: &mut Item) -> CoreResult<Array> {
    match std::mem::take(item) {
        Item::ArrayOfTables(aot) => Ok(aot.into_array()),
        _ => Err(missing()),
    }
}

// -------------------------------------------------------------------- add

/// A `[header]` table other than the root: objects added there get headers too.
fn is_header_table(doc: &mut DocumentMut, parent: &[String]) -> CoreResult<bool> {
    Ok(match resolve(doc, parent).ok_or_else(missing)? {
        N::Item(Item::Table(t)) => !parent.is_empty() && !t.is_dotted(),
        N::Tab(_) => true,
        _ => false,
    })
}

pub(super) fn add(
    doc: &mut DocumentMut,
    parent: &[String],
    key: &str,
    new: &Val,
    crlf: bool,
) -> CoreResult<()> {
    let infos = layout::normalize(doc);
    let as_table = is_header_table(doc, parent)?
        && match new {
            Val::Object(_) => true,
            Val::Array(items) => all_objects(items),
            _ => false,
        };
    if as_table {
        // `[parent.key]` / `[[parent.key]]` right after the parent's last sub-table.
        let after = layout::last_position_under(&infos, parent).ok_or_else(missing)?;
        let item = match new {
            Val::Object(members) => {
                let mut table = table_from(members, crlf)?;
                table.set_position(Some(after + 1));
                Item::Table(table)
            }
            Val::Array(items) => {
                let mut aot = ArrayOfTables::new();
                for (n, member) in items.iter().enumerate() {
                    let Val::Object(members) = member else {
                        continue;
                    };
                    let mut table = table_from(members, crlf)?;
                    table.set_position(Some(after + 1 + n as isize));
                    aot.push(table);
                }
                Item::ArrayOfTables(aot)
            }
            _ => return Err(missing()),
        };
        let table = table_like(resolve(doc, parent).ok_or_else(missing)?).ok_or_else(missing)?;
        table.insert(key, item);
        return Ok(());
    }
    let inline = matches!(
        resolve(doc, parent).ok_or_else(missing)?,
        N::Item(Item::Value(_)) | N::Val(_)
    );
    let value = to_value(new, None, crlf)?;
    let node = resolve(doc, parent).ok_or_else(missing)?;
    if let N::Item(Item::Table(t)) = node
        && !t.is_dotted()
    {
        t.set_implicit(false);
        insert_kv(t, key, value, inline);
    } else {
        let table = table_like(resolve(doc, parent).ok_or_else(missing)?).ok_or_else(missing)?;
        insert_kv(table, key, value, inline);
    }
    Ok(())
}

/// Append `key = value` after the table's other key/values, indented like them.
fn insert_kv(table: &mut dyn TableLike, key: &str, mut value: Value, inline: bool) {
    let mut new_key = Key::new(key);
    let mut last_suffix = None;
    let last = table.iter().map(|(k, _)| k.to_string()).last();
    if let Some(last) = last
        && let Some(existing) = table.key(&last)
    {
        let indent = prefix_of(existing.leaf_decor());
        if indent.chars().all(|c| c == ' ' || c == '\t') {
            new_key.leaf_decor_mut().set_prefix(indent);
        }
    }
    if inline {
        // `{ a = 1 }`: the space before `}` moves to the new last value.
        if let Some((_, Item::Value(v))) = table.iter_mut().last() {
            last_suffix = v.decor().suffix().cloned();
            v.decor_mut().set_suffix("");
        }
    }
    if let Some(suffix) = last_suffix {
        value.decor_mut().set_suffix(suffix);
    }
    table.entry_format(&new_key).or_insert(Item::Value(value));
}

// ----------------------------------------------------------------- insert

pub(super) fn insert(
    doc: &mut DocumentMut,
    array: &[String],
    at: usize,
    new: &Val,
    crlf: bool,
) -> CoreResult<()> {
    let is_aot = matches!(
        resolve(doc, array).ok_or_else(missing)?,
        N::Item(Item::ArrayOfTables(_))
    );
    if is_aot {
        if let Val::Object(members) = new {
            return insert_table(doc, array, at, members, crlf);
        }
        // Not a table: the array becomes inline.
        let Some(N::Item(item)) = resolve(doc, array) else {
            return Err(missing());
        };
        let mut inline = inline_array(item)?;
        array_insert(&mut inline, at, to_value(new, None, crlf)?);
        *item = Item::Value(Value::Array(inline));
        kv_key(doc, array);
        return Ok(());
    }
    let target = array_of(resolve(doc, array).ok_or_else(missing)?).ok_or_else(missing)?;
    array_insert(target, at, to_value(new, None, crlf)?);
    Ok(())
}

fn insert_table(
    doc: &mut DocumentMut,
    array: &[String],
    at: usize,
    members: &[(String, Val)],
    crlf: bool,
) -> CoreResult<()> {
    let infos = layout::normalize(doc);
    let element = |i: usize| {
        let mut p = array.to_vec();
        p.push(i.to_string());
        p
    };
    let len = match resolve(doc, array) {
        Some(N::Item(Item::ArrayOfTables(a))) => a.len(),
        _ => return Err(missing()),
    };
    let mut table = table_from(members, crlf)?;
    let mut bump_next = false;
    if at < len {
        let next = layout::position_of(&infos, &element(at)).ok_or_else(missing)?;
        table.set_position(Some(next - 1));
        // The new header takes the blank line the next one had; that one gets its own.
        if let Some(N::Tab(old)) = resolve(doc, &element(at))
            && is_blank(&prefix_of(old.decor()))
        {
            *table.decor_mut() = old.decor().clone();
            bump_next = prefix_of(old.decor()).is_empty();
        }
    } else {
        let after = layout::last_position_under(&infos, &element(len - 1)).ok_or_else(missing)?;
        table.set_position(Some(after + 1));
    }
    if bump_next && let Some(N::Tab(old)) = resolve(doc, &element(at)) {
        old.decor_mut().set_prefix("\n");
    }
    match resolve(doc, array) {
        Some(N::Item(Item::ArrayOfTables(a))) => {
            a.insert(at, table);
            Ok(())
        }
        _ => Err(missing()),
    }
}

fn array_insert(array: &mut Array, at: usize, mut value: Value) {
    let len = array.len();
    if len == 0 {
        value.decor_mut().clear();
        array.push_formatted(value);
        return;
    }
    // Spacing comes from the neighbour on the side the value joins.
    let neighbour = if at == len { len - 1 } else { at };
    *value.decor_mut() = array
        .get(neighbour)
        .map(|v| layout_decor(v.decor()))
        .unwrap_or_default();
    value.decor_mut().set_suffix("");
    if at == len
        && let Some(last) = array.get_mut(len - 1)
    {
        // `[1, 2 ]`: the space before `]` moves to the new last value.
        let suffix = last.decor().suffix().cloned();
        last.decor_mut().set_suffix("");
        if let Some(suffix) = suffix {
            value.decor_mut().set_suffix(suffix);
        }
    }
    if at == len && array.trailing_comma() {
        // The last value's comment follows its comma, in the trailing text:
        // it stays with that value, ahead of the new one.
        let trailing = array.trailing().as_str().unwrap_or("").to_string();
        if let Some(i) = trailing.find('\n')
            && trailing[..i].contains('#')
        {
            let prefix = format!("{}{}", &trailing[..i], prefix_of(value.decor()));
            value.decor_mut().set_prefix(prefix);
            array.set_trailing(trailing[i..].to_string());
        }
    }
    if at == 0
        && let Some(first) = array.get_mut(0)
        && prefix_of(first.decor()).is_empty()
    {
        // `["a"]` -> `["z", "a"]`: the old first value now follows a comma.
        first.decor_mut().set_prefix(" ");
    }
    if at == len {
        array.push_formatted(value);
    } else {
        array.insert_formatted(at, value);
    }
}

// ----------------------------------------------------------------- remove

pub(super) fn remove(doc: &mut DocumentMut, path: &[String]) -> CoreResult<()> {
    let (last, parent_path) = path.split_last().ok_or_else(missing)?;
    let mut emptied_aot = false;
    match resolve(doc, parent_path).ok_or_else(missing)? {
        N::Item(item @ Item::ArrayOfTables(_)) => {
            let i = index(last).ok_or_else(missing)?;
            let Item::ArrayOfTables(aot) = item else {
                return Err(missing());
            };
            if i >= aot.len() {
                return Err(missing());
            }
            aot.remove(i);
            if aot.is_empty() {
                // TOML cannot spell an empty array of tables.
                *item = Item::Value(Value::Array(Array::new()));
                emptied_aot = true;
            }
        }
        node @ (N::Item(Item::Value(Value::Array(_))) | N::Val(Value::Array(_))) => {
            let array = array_of(node).ok_or_else(missing)?;
            let i = index(last)
                .filter(|i| *i < array.len())
                .ok_or_else(missing)?;
            array_remove(array, i);
        }
        node => {
            let table = table_like(node).ok_or_else(missing)?;
            table.remove(last).ok_or_else(missing)?;
        }
    }
    if emptied_aot {
        kv_key(doc, parent_path);
    }
    fix_emptied(doc, parent_path)
}

fn array_remove(array: &mut Array, at: usize) {
    let removed = array.remove(at);
    if array.is_empty() {
        return;
    }
    if at == array.len()
        && let Some(last) = array.get_mut(at - 1)
        && let Some(suffix) = removed.decor().suffix().cloned()
    {
        last.decor_mut().set_suffix(suffix);
    }
    if at == 0 {
        // The new first value takes the old first's leading spacing.
        let prefix = layout_prefix(&prefix_of(removed.decor()));
        if let Some(v) = array.get_mut(0) {
            v.decor_mut().set_prefix(prefix);
        }
        return;
    }
    // After a comma, the rest of that line (a comment) belongs to the value
    // before it, and sits in the next value's prefix: the removed value's
    // comment goes, the previous value's stays.
    let removed_prefix = prefix_of(removed.decor());
    let carried = match removed_prefix.split_once('\n') {
        Some((line, _)) if line.contains('#') => line.to_string(),
        _ => String::new(),
    };
    if let Some(next) = array.get_mut(at) {
        let prefix = prefix_of(next.decor());
        let rest = prefix.find('\n').map_or(prefix.as_str(), |i| &prefix[i..]);
        let merged = format!("{carried}{rest}");
        next.decor_mut().set_prefix(merged);
    } else {
        // The last value's own comment sits in the array's trailing text.
        let trailing = array.trailing().as_str().unwrap_or("").to_string();
        let rest = match trailing.find('\n') {
            Some(i) if trailing[..i].contains('#') => &trailing[i..],
            _ => trailing.as_str(),
        };
        array.set_trailing(format!("{carried}{rest}"));
    }
}

/// An implicit or dotted table that lost its last member still exists as `{}`.
fn fix_emptied(doc: &mut DocumentMut, path: &[String]) -> CoreResult<()> {
    let Some((key, grand)) = path.split_last() else {
        return Ok(());
    };
    let state = match resolve(doc, grand).and_then(table_like) {
        Some(t) => match t.get(key) {
            Some(Item::Table(inner)) if inner.is_empty() => Some(inner.is_dotted()),
            _ => None,
        },
        None => None,
    };
    match state {
        Some(true) => {
            if let Some(t) = resolve(doc, grand).and_then(table_like) {
                t.insert(key, Item::Value(Value::InlineTable(InlineTable::new())));
            }
        }
        Some(false) => {
            layout::normalize(doc);
            if let Some(N::Item(Item::Table(t))) = resolve(doc, path) {
                t.set_implicit(false);
            }
        }
        None => {}
    }
    Ok(())
}

// ------------------------------------------------------------------- move

pub(super) fn move_item(
    doc: &mut DocumentMut,
    array: &[String],
    from: usize,
    to: usize,
) -> CoreResult<()> {
    let is_aot = matches!(
        resolve(doc, array).ok_or_else(missing)?,
        N::Item(Item::ArrayOfTables(_))
    );
    if !is_aot {
        let target = array_of(resolve(doc, array).ok_or_else(missing)?).ok_or_else(missing)?;
        // Values travel; each slot keeps its own spacing and comments.
        let mut values: Vec<Value> = target.iter().cloned().collect();
        let decors: Vec<Decor> = values.iter().map(|v| v.decor().clone()).collect();
        let item = values.remove(from);
        values.insert(to, item);
        target.clear();
        for (mut v, d) in values.into_iter().zip(decors) {
            *v.decor_mut() = d;
            target.push_formatted(v);
        }
        return Ok(());
    }
    // Array of tables: reorder the elements and hand their print slots over.
    let infos = layout::normalize(doc);
    let len = match resolve(doc, array) {
        Some(N::Item(Item::ArrayOfTables(a))) => a.len(),
        _ => return Err(missing()),
    };
    let block_of = |j: usize| -> Vec<usize> {
        let mut prefix = array.to_vec();
        prefix.push(j.to_string());
        let mut ids: Vec<usize> = (0..infos.len())
            .filter(|&id| infos[id].pos.is_some() && infos[id].path.starts_with(&prefix))
            .collect();
        ids.sort_by_key(|&id| infos[id].pos);
        ids
    };
    let blocks: Vec<Vec<usize>> = (0..len).map(block_of).collect();
    let mut perm: Vec<usize> = (0..len).collect();
    let moved = perm.remove(from);
    perm.insert(to, moved);
    let mut slots: Vec<usize> = blocks.iter().flatten().copied().collect();
    slots.sort_by_key(|&id| infos[id].pos);
    let sequence: Vec<usize> = perm
        .iter()
        .flat_map(|&j| blocks[j].iter().copied())
        .collect();
    let mut positions = vec![None; infos.len()];
    for (slot, id) in slots.iter().zip(sequence) {
        positions[id] = infos[*slot].pos;
    }
    layout::set_positions(doc, &positions);
    match resolve(doc, array) {
        Some(N::Item(Item::ArrayOfTables(a))) => {
            let mut tables: Vec<Table> = a.iter().cloned().collect();
            let t = tables.remove(from);
            tables.insert(to, t);
            a.clear();
            tables.into_iter().for_each(|t| a.push(t));
            Ok(())
        }
        _ => Err(missing()),
    }
}

// ----------------------------------------------------------------- rename

pub(super) fn rename(doc: &mut DocumentMut, path: &[String], new_key: &str) -> CoreResult<()> {
    let (old, parent) = path.split_last().ok_or_else(missing)?;
    let table = table_like(resolve(doc, parent).ok_or_else(missing)?).ok_or_else(missing)?;
    let entries: Vec<(Key, Item)> = table
        .iter()
        .filter_map(|(k, _)| table.get_key_value(k).map(|(k, v)| (k.clone(), v.clone())))
        .collect();
    table.clear();
    for (key, item) in entries {
        let key = if key.get() == old {
            Key::new(new_key)
                .with_leaf_decor(key.leaf_decor().clone())
                .with_dotted_decor(key.dotted_decor().clone())
        } else {
            key
        };
        table.entry_format(&key).or_insert(item);
    }
    Ok(())
}
