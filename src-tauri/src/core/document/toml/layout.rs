//! Where `toml_edit` prints each `[table]`.
//!
//! The writer sorts tables by `position`; a table without one inherits the
//! last seen position. Before an edit that adds, inserts or reorders tables we
//! give every table an explicit, widely spaced position in its current print
//! order, so a new table can be slotted in by choosing a number.

use toml_edit::{DocumentMut, Item, Table};

/// Gap between consecutive normalized positions: room for new tables.
pub(super) const GAP: isize = 1000;

#[derive(Debug, Clone)]
pub(super) struct TableInfo {
    /// Value pointer segments of the table (array-of-tables elements by index).
    pub path: Vec<String>,
    /// Normalized print position (`None` for dotted-key proxies, never printed).
    pub pos: Option<isize>,
}

struct Raw {
    path: Vec<String>,
    listed: bool,
    effective: isize,
}

fn survey_table(table: &Table, path: &mut Vec<String>, last: &mut isize, out: &mut Vec<Raw>) {
    let listed = !table.is_dotted();
    if listed && let Some(p) = table.position() {
        *last = p;
    }
    out.push(Raw {
        path: path.clone(),
        listed,
        effective: *last,
    });
    for (key, item) in table.iter() {
        match item {
            Item::Table(t) => {
                path.push(key.to_string());
                survey_table(t, path, last, out);
                path.pop();
            }
            Item::ArrayOfTables(a) => {
                for (i, t) in a.iter().enumerate() {
                    path.push(key.to_string());
                    path.push(i.to_string());
                    survey_table(t, path, last, out);
                    path.pop();
                    path.pop();
                }
            }
            _ => {}
        }
    }
}

fn walk_mut(table: &mut Table, id: &mut usize, f: &mut dyn FnMut(usize, &mut Table)) {
    f(*id, table);
    *id += 1;
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(t) => walk_mut(t, id, f),
            Item::ArrayOfTables(a) => a.iter_mut().for_each(|t| walk_mut(t, id, f)),
            _ => {}
        }
    }
}

/// Give every printed table an explicit position in its current print order.
/// Returns the tables in DFS order (index = DFS id) with their new positions.
pub(super) fn normalize(doc: &mut DocumentMut) -> Vec<TableInfo> {
    let mut raws = Vec::new();
    survey_table(doc.as_table(), &mut Vec::new(), &mut 0, &mut raws);
    // The writer's sort: the root first, then by (inherited) position, DFS order breaking ties.
    let mut order: Vec<usize> = (0..raws.len()).filter(|&i| raws[i].listed).collect();
    order.sort_by_key(|&i| (!raws[i].path.is_empty(), raws[i].effective));
    let mut pos = vec![None; raws.len()];
    for (rank, &i) in order.iter().enumerate() {
        pos[i] = Some(rank as isize * GAP);
    }
    set_positions(doc, &pos);
    raws.into_iter()
        .zip(pos)
        .map(|(r, pos)| TableInfo { path: r.path, pos })
        .collect()
}

/// Set the position of the table with DFS id `i` to `positions[i]` (when `Some`).
pub(super) fn set_positions(doc: &mut DocumentMut, positions: &[Option<isize>]) {
    walk_mut(doc.as_table_mut(), &mut 0, &mut |id, t| {
        if let Some(Some(p)) = positions.get(id) {
            t.set_position(Some(*p));
        }
    });
}

/// Largest position among the tables at or below `path`.
pub(super) fn last_position_under(infos: &[TableInfo], path: &[String]) -> Option<isize> {
    infos
        .iter()
        .filter(|t| t.path.starts_with(path))
        .filter_map(|t| t.pos)
        .max()
}

/// Position of exactly the table at `path`.
pub(super) fn position_of(infos: &[TableInfo], path: &[String]) -> Option<isize> {
    infos.iter().find(|t| t.path == path).and_then(|t| t.pos)
}
