//! TOML syntax tests: fidelity of every primitive edit, round trips, the
//! reference-model property tests, session behaviour and blocked files.

use proptest::prelude::*;
use serde_json::Value;

use super::super::tests::{
    MODEL_DOC, add, gen_op, insert, load_text, model_apply, mv, no_secrets, remove, rename,
    sel_strategy, set, text_of,
};
use super::super::*;
use super::mask;
use crate::core::types::{DiagnosticSource, NodeKind};

fn fx(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../fixtures/toml/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn toml(text: &str) -> DocumentSession {
    load_text(text, DocKind::Toml)
}

/// Apply `ops` in order and return the text.
fn run(text: &str, ops: &[DraftOp]) -> String {
    let mut d = toml(text);
    assert!(d.is_editable(), "{:?}", d.diagnostics());
    for op in ops {
        d.apply(op.clone())
            .unwrap_or_else(|e| panic!("{op:?}: {e:?}"));
    }
    text_of(&d)
}

/// What a fresh session reads back from `text`.
fn value_of(text: &str) -> Value {
    let d = toml(text);
    assert!(d.is_editable(), "{text}\n{:?}", d.diagnostics());
    d.to_value().unwrap()
}

// ------------------------------------------------------------ fidelity

/// Tables, `[[array of tables]]`, an inline table, dotted keys, a multi-line
/// array, a datetime and comments in every position.
const FIX: &str = r#"# Service config
title = "Demo" # name
port = 8080 # http
ratio = 1.50
started = 1979-05-27T07:32:00Z
tags = ["a", "b"]
ports = [
  8000, # primary
  8001,
]
owner.name = "Ann"
point = { x = 1, y = 2 }

[db]
host = "localhost" # where
# retry policy
retries = 3

[db.pool]
size = 10

[[plugins]]
name = "alpha"

# second
[[plugins]]
name = "beta"

[plugins.opts]
level = "debug"
"#;

#[track_caller]
fn expect(ops: &[DraftOp], expected: &str) {
    assert_eq!(run(FIX, ops), expected);
}

#[test]
fn set_scalar_keeps_comments_and_neighbours() {
    expect(
        &[set("/port", "9090")],
        &FIX.replacen("port = 8080 # http", "port = 9090 # http", 1),
    );
    expect(
        &[set("/title", "\"New\"")],
        &FIX.replacen("title = \"Demo\" # name", "title = \"New\" # name", 1),
    );
    expect(
        &[set("/db/host", "\"db.local\"")],
        &FIX.replacen(
            "host = \"localhost\" # where",
            "host = \"db.local\" # where",
            1,
        ),
    );
    expect(
        &[set("/plugins/1/name", "\"gamma\"")],
        &FIX.replacen("name = \"beta\"", "name = \"gamma\"", 1),
    );
}

#[test]
fn set_through_dotted_keys_and_inline_tables() {
    expect(
        &[set("/owner/name", "\"Bob\"")],
        &FIX.replacen("owner.name = \"Ann\"", "owner.name = \"Bob\"", 1),
    );
    expect(
        &[set("/point/y", "20")],
        &FIX.replacen("point = { x = 1, y = 2 }", "point = { x = 1, y = 20 }", 1),
    );
    expect(&[set("/ports/1", "9")], &FIX.replacen("  8001,", "  9,", 1));
}

#[test]
fn set_number_and_datetime_lexemes() {
    expect(
        &[set("/ratio", "2.5")],
        &FIX.replacen("ratio = 1.50", "ratio = 2.5", 1),
    );
    expect(
        &[set("/started", "\"2020-01-02T03:04:05Z\"")],
        &FIX.replacen("1979-05-27T07:32:00Z", "2020-01-02T03:04:05Z", 1),
    );
}

#[test]
fn add_appends_after_key_values_before_sub_tables() {
    expect(
        &[add("/db/timeout", "30")],
        &FIX.replacen("retries = 3\n", "retries = 3\ntimeout = 30\n", 1),
    );
    expect(
        &[add("/extra", "true")],
        &FIX.replacen(
            "point = { x = 1, y = 2 }\n",
            "point = { x = 1, y = 2 }\nextra = true\n",
            1,
        ),
    );
    expect(
        &[add("/owner/age", "5")],
        &FIX.replacen(
            "owner.name = \"Ann\"\n",
            "owner.name = \"Ann\"\nowner.age = 5\n",
            1,
        ),
    );
    expect(
        &[add("/point/z", "3")],
        &FIX.replacen(
            "point = { x = 1, y = 2 }",
            "point = { x = 1, y = 2, z = 3 }",
            1,
        ),
    );
}

#[test]
fn add_objects_become_sub_tables_after_the_parents_last_sub_table() {
    expect(
        &[add("/db/ssl", "{\"on\": true, \"ca\": \"x\"}")],
        &FIX.replacen(
            "[db.pool]\nsize = 10\n",
            "[db.pool]\nsize = 10\n\n[db.ssl]\non = true\nca = \"x\"\n",
            1,
        ),
    );
    // The root takes inline tables.
    expect(
        &[add("/extra", "{\"a\": 1}")],
        &FIX.replacen(
            "point = { x = 1, y = 2 }\n",
            "point = { x = 1, y = 2 }\nextra = { a = 1 }\n",
            1,
        ),
    );
    // Under an array-of-tables element, after that element's sub-tables.
    expect(
        &[add("/plugins/1/meta", "{\"v\": 1}")],
        &FIX.replacen(
            "[plugins.opts]\nlevel = \"debug\"\n",
            "[plugins.opts]\nlevel = \"debug\"\n\n[plugins.meta]\nv = 1\n",
            1,
        ),
    );
}

#[test]
fn add_array_of_objects_becomes_array_of_tables() {
    expect(
        &[add("/db/replicas", "[{\"h\": \"a\"}, {\"h\": \"b\"}]")],
        &FIX.replacen(
            "[db.pool]\nsize = 10\n",
            "[db.pool]\nsize = 10\n\n[[db.replicas]]\nh = \"a\"\n\n[[db.replicas]]\nh = \"b\"\n",
            1,
        ),
    );
    // Other arrays stay inline.
    expect(
        &[add("/db/names", "[\"x\", 1]")],
        &FIX.replacen("retries = 3\n", "retries = 3\nnames = [\"x\", 1]\n", 1),
    );
}

#[test]
fn remove_drops_the_whole_line_or_block() {
    expect(
        &[remove("/port")],
        &FIX.replacen("port = 8080 # http\n", "", 1),
    );
    expect(
        &[remove("/owner/name")],
        // The last dotted key leaves an empty table behind.
        &FIX.replacen("owner.name = \"Ann\"\n", "owner = {}\n", 1),
    );
    expect(
        &[remove("/db/pool")],
        &FIX.replacen("[db.pool]\nsize = 10\n\n", "", 1),
    );
    expect(
        &[remove("/plugins/0")],
        &FIX.replacen("[[plugins]]\nname = \"alpha\"\n\n", "", 1),
    );
    // A removed element takes its sub-tables and leading comment with it.
    expect(
        &[remove("/plugins/1")],
        &FIX.replacen(
            "\n# second\n[[plugins]]\nname = \"beta\"\n\n[plugins.opts]\nlevel = \"debug\"\n",
            "",
            1,
        ),
    );
}

#[test]
fn array_items_insert_remove_and_move() {
    expect(
        &[insert("/tags", 1, "\"mid\"")],
        &FIX.replacen("[\"a\", \"b\"]", "[\"a\", \"mid\", \"b\"]", 1),
    );
    expect(
        &[insert("/tags", 0, "\"z\"")],
        &FIX.replacen("[\"a\", \"b\"]", "[\"z\", \"a\", \"b\"]", 1),
    );
    expect(
        &[remove("/tags/0")],
        &FIX.replacen("[\"a\", \"b\"]", "[\"b\"]", 1),
    );
    expect(
        &[mv("/tags", 0, 1)],
        &FIX.replacen("[\"a\", \"b\"]", "[\"b\", \"a\"]", 1),
    );
    // Multi-line array: new items copy the neighbours' layout.
    expect(
        &[insert("/ports", 2, "8002")],
        &FIX.replacen("  8001,\n]", "  8001,\n  8002,\n]", 1),
    );
    expect(&[remove("/ports/1")], &FIX.replacen("  8001,\n", "", 1));
}

#[test]
fn array_of_tables_insert_and_move_keep_blocks_whole() {
    expect(
        &[insert("/plugins", 2, "{\"name\": \"gamma\"}")],
        &format!("{FIX}\n[[plugins]]\nname = \"gamma\"\n"),
    );
    expect(
        &[insert("/plugins", 0, "{\"name\": \"zero\"}")],
        &FIX.replacen(
            "[[plugins]]\nname = \"alpha\"",
            "[[plugins]]\nname = \"zero\"\n\n[[plugins]]\nname = \"alpha\"",
            1,
        ),
    );
    let moved = run(FIX, &[mv("/plugins", 0, 1)]);
    assert_eq!(
        moved,
        FIX.replacen(
            "[[plugins]]\nname = \"alpha\"\n\n# second\n[[plugins]]\nname = \"beta\"\n\n[plugins.opts]\nlevel = \"debug\"\n",
            "# second\n[[plugins]]\nname = \"beta\"\n\n[plugins.opts]\nlevel = \"debug\"\n\n[[plugins]]\nname = \"alpha\"\n",
            1,
        )
    );
}

#[test]
fn rename_keeps_position_decor_and_sub_tables() {
    expect(
        &[rename("/port", "http_port")],
        &FIX.replacen("port = 8080", "http_port = 8080", 1),
    );
    expect(
        &[rename("/db", "database")],
        &FIX.replacen("[db]", "[database]", 1)
            .replacen("[db.pool]", "[database.pool]", 1),
    );
    expect(
        &[rename("/owner", "who")],
        &FIX.replacen("owner.name", "who.name", 1),
    );
    expect(
        &[rename("/title", "the title")],
        &FIX.replacen("title = ", "\"the title\" = ", 1),
    );
    expect(
        &[rename("/db/host", "addr")],
        &FIX.replacen("host = ", "addr = ", 1),
    );
}

#[test]
fn set_replaces_structures_in_place() {
    // A table's content is replaced under its own header.
    expect(
        &[set("/db/pool", "{\"size\": 1, \"max\": 2}")],
        &FIX.replacen(
            "[db.pool]\nsize = 10\n",
            "[db.pool]\nsize = 1\nmax = 2\n",
            1,
        ),
    );
    // A scalar becomes an inline table where it stands.
    expect(
        &[set("/port", "{\"a\": 1}")],
        &FIX.replacen("port = 8080 # http", "port = { a = 1 } # http", 1),
    );
    // A header block collapses into a value among the key/values.
    let flat = run(FIX, &[set("/db/pool", "5")]);
    assert!(
        flat.contains("retries = 3\npool = 5\n") && !flat.contains("[db.pool]"),
        "{flat}"
    );
}

#[test]
fn string_styles_survive_a_value_change() {
    let text = "a = 'lit'\nb = \"\"\"\nline1\nline2\"\"\"\nc = \"basic\"\nd = '''\nraw\n'''\n";
    let out = run(
        text,
        &[
            set("/a", "\"other\""),
            set("/b", "\"x\\ny\""),
            set("/c", "\"it's\""),
            set("/d", "\"q\\nr\""),
        ],
    );
    assert_eq!(
        out,
        "a = 'other'\nb = \"\"\"\nx\ny\"\"\"\nc = \"it's\"\nd = '''\nq\nr'''\n"
    );
    // A literal string cannot hold a quote: it falls back to basic.
    assert_eq!(
        run("a = 'lit'\n", &[set("/a", "\"it's\"")]),
        "a = \"it's\"\n"
    );
    // Escapes for what basic strings cannot hold raw.
    assert_eq!(
        run("a = \"x\"\n", &[set("/a", "\"q\\\"t\\\\b\\u0001\"")]),
        "a = \"q\\\"t\\\\b\\u0001\"\n"
    );
}

#[test]
fn datetimes_stay_datetimes_and_other_strings_do_not() {
    let text = "when = 1979-05-27T07:32:00Z # utc\nname = \"x\"\n";
    let out = run(
        text,
        &[
            set("/when", "\"1980-01-01\""),
            set("/name", "\"1980-01-01\""),
        ],
    );
    assert_eq!(out, "when = 1980-01-01 # utc\nname = \"1980-01-01\"\n");
    // A non-date string replaces a datetime with a string.
    assert_eq!(
        run(text, &[set("/when", "\"soon\"")]),
        "when = \"soon\" # utc\nname = \"x\"\n"
    );
}

#[test]
fn numbers_write_their_json_lexeme_and_reject_what_toml_cannot_hold() {
    let d = |ops: &[DraftOp]| run("a = 1\n", ops);
    assert_eq!(d(&[set("/a", "1e3")]), "a = 1e3\n");
    assert_eq!(d(&[set("/a", "-0.50")]), "a = -0.50\n");
    assert_eq!(
        d(&[set("/a", "9223372036854775807")]),
        "a = 9223372036854775807\n"
    );
    let mut s = toml("a = 1\n");
    let err = s.apply(set("/a", "9223372036854775808")).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidOperation);
    assert_eq!(text_of(&s), "a = 1\n");
}

#[test]
fn null_is_refused_everywhere_it_could_enter() {
    let mut s = toml("a = 1\nlist = [1]\n");
    for op in [
        set("/a", "null"),
        add("/b", "null"),
        insert("/list", 0, "null"),
        set("/a", "[1, null]"),
        add("/b", "{\"x\": {\"y\": null}}"),
    ] {
        let err = s.apply(op.clone()).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidOperation, "{op:?}");
        assert!(err.message.contains("no null value"), "{}", err.message);
    }
    assert_eq!(text_of(&s), "a = 1\nlist = [1]\n");
    assert!(!s.is_dirty());
}

#[test]
fn comments_and_spacing_survive_untouched_regions() {
    let text =
        "# top\n\n  a   =   1   # one\nb = [ 1,2 ,3 ]\n\n\n# lonely\n[t]   # hdr\n  x = 'y'\n";
    let out = run(text, &[set("/a", "2"), set("/t/x", "\"z\"")]);
    assert_eq!(
        out,
        "# top\n\n  a   =   2   # one\nb = [ 1,2 ,3 ]\n\n\n# lonely\n[t]   # hdr\n  x = 'z'\n"
    );
    // Adding to the indented table copies the indent.
    assert_eq!(
        run(text, &[add("/t/w", "1")]),
        "# top\n\n  a   =   1   # one\nb = [ 1,2 ,3 ]\n\n\n# lonely\n[t]   # hdr\n  x = 'y'\n  w = 1\n"
    );
}

#[test]
fn removing_the_last_member_keeps_an_empty_table() {
    // Implicit parent of a removed header.
    let out = run("[a.b]\nx = 1\n", &[remove("/a/b")]);
    assert_eq!(value_of(&out), serde_json::json!({"a": {}}));
    // Dotted keys.
    let out = run("a.b = 1\n", &[remove("/a/b")]);
    assert_eq!(out, "a = {}\n");
    // The last array-of-tables element.
    let out = run("[[p]]\nx = 1\n", &[remove("/p/0")]);
    assert_eq!(out, "p = []\n");
}

#[test]
fn array_of_tables_edge_ops() {
    // A non-table item turns the array inline.
    let out = run("[[p]]\nx = 1\n\n[[p]]\nx = 2\n", &[insert("/p", 1, "7")]);
    assert_eq!(
        value_of(&out),
        serde_json::json!({"p": [{"x": 1}, 7, {"x": 2}]})
    );
    let out = run("[[p]]\nx = 1\n", &[set("/p/0", "5")]);
    assert_eq!(out, "p = [5]\n");
}

// ------------------------------------------------------------ round trips

#[test]
fn every_fixture_round_trips_byte_for_byte() {
    for name in ["app.toml", "secret.toml", "empty.toml"] {
        let text = fx(name);
        let d = toml(&text);
        assert!(d.is_editable(), "{name}: {:?}", d.diagnostics());
        assert_eq!(text_of(&d), text, "{name}");
    }
    for (name, text) in [
        (
            "crlf",
            "title = \"x\"\r\nport = 1 # c\r\n\r\n[a]\r\nk = \"v\"\r\n[[b]]\r\nx = 1",
        ),
        ("no final newline", "a = 1\n[t]\nb = 2"),
        ("only comments", "# nothing\n# here\n"),
        ("blank", "\n\n"),
        ("trailing comment", "a = 1\n# end"),
        ("crlf trailing", "a = 1\r\n"),
        ("tabs", "a\t=\t1\n[\tt\t]\n\tb = 2\n"),
        (
            "quoted keys",
            "\"a b\" = 1\n'c.d' = 2\n[ \"x y\" . z ]\nk = 1\n",
        ),
        (
            "multiline strings",
            "a = \"\"\"\n  x\n\"\"\"\nb = '''\ny'''\n",
        ),
        (
            "nested inline",
            "a = { b = { c = [1, { d = 2 }] }, e = [] }\n",
        ),
        ("dotted tables", "a.b.c = 1\na.b.d = 2\n[t]\nx.y = 3\n"),
        ("out of order", "[a.b]\nx = 1\n[c]\ny = 2\n[a.d]\nz = 3\n"),
        (
            "special floats",
            "a = inf\nb = -nan\nc = 0xDEAD_beef\nd = 1e-3\ne = 0b101\n",
        ),
        (
            "dates",
            "a = 1979-05-27\nb = 07:32:00\nc = 1979-05-27 07:32:00-08:00\n",
        ),
    ] {
        let d = toml(text);
        assert!(d.is_editable(), "{name}: {:?}", d.diagnostics());
        assert_eq!(text_of(&d), text, "{name}");
        // And an edit-free load of the *saved* bytes is stable too.
        assert_eq!(text_of(&toml(&text_of(&d))), text, "{name}");
    }
}

#[test]
fn crlf_edits_keep_every_line_ending_crlf() {
    let text = "title = \"x\"\r\nport = 1 # c\r\n\r\n[a]\r\nk = \"v\"\r\n[[b]]\r\nx = 1";
    let mut d = toml(text);
    d.apply(set("/port", "2")).unwrap();
    d.apply(add("/a/n", "{\"deep\": 1}")).unwrap();
    d.apply(add("/a/z", "3")).unwrap();
    d.apply(insert("/b", 1, "{\"x\": 2}")).unwrap();
    let out = text_of(&d);
    assert!(!out.ends_with('\n'), "{out:?}");
    assert_eq!(
        out.matches('\n').count(),
        out.matches("\r\n").count(),
        "{out:?}"
    );
    assert!(
        out.starts_with("title = \"x\"\r\nport = 2 # c\r\n"),
        "{out:?}"
    );
    assert_eq!(
        value_of(&out),
        serde_json::json!({"title": "x", "port": 2, "a": {"k": "v", "z": 3, "n": {"deep": 1}}, "b": [{"x": 1}, {"x": 2}]})
    );
}

#[test]
fn bom_and_missing_final_newline_through_the_session() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"# with bom\nname = \"bom\"\n[t]\nk = 1");
    let mut d = DocumentSession::load("t", bytes.clone(), DocKind::Toml);
    assert!(d.is_editable());
    assert_eq!(d.candidate_bytes(), bytes);
    d.apply(set("/t/k", "2")).unwrap();
    d.apply(add("/t/j", "1")).unwrap();
    let out = d.candidate_bytes();
    assert_eq!(&out[..3], &[0xEF, 0xBB, 0xBF]);
    assert_eq!(&out[3..], b"# with bom\nname = \"bom\"\n[t]\nk = 2\nj = 1");
    d.undo().unwrap();
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), bytes);
}

#[test]
fn empty_file_is_an_editable_empty_table() {
    let mut d = toml("");
    assert!(d.is_editable());
    assert_eq!(d.to_value().unwrap(), serde_json::json!({}));
    d.apply(add("/a", "1")).unwrap();
    d.apply(add("/t", "{\"x\": 1}")).unwrap();
    assert_eq!(text_of(&d), "a = 1\nt = { x = 1 }\n");
}

#[test]
fn typing_in_the_neutral_value() {
    let d = toml(&fx("app.toml"));
    let tree = d.subtree("", 3).unwrap();
    assert_eq!(tree.kind, NodeKind::Object);
    let kinds: std::collections::HashMap<String, (NodeKind, Option<String>)> = tree
        .children
        .unwrap()
        .into_iter()
        .map(|c| (c.key.unwrap(), (c.kind, c.text)))
        .collect();
    assert_eq!(kinds["big"], (NodeKind::Number, Some("1_000".into())));
    assert_eq!(kinds["mask"].1.as_deref(), Some("0x1F"));
    assert_eq!(
        kinds["started"],
        (NodeKind::String, Some("1979-05-27T07:32:00Z".into()))
    );
    assert_eq!(kinds["ratio"].1.as_deref(), Some("1.50"));
    assert_eq!(kinds["owner"].0, NodeKind::Object);
    assert_eq!(kinds["point"].0, NodeKind::Object);
    assert_eq!(kinds["plugins"].0, NodeKind::Array);
    assert_eq!(kinds["servers"].0, NodeKind::Object);
    assert_eq!(kinds["debug"].0, NodeKind::Boolean);
    assert_eq!(kinds["odd key"].1.as_deref(), Some("3"));
}

// ---------------------------------------------------------- property tests

/// `MODEL_DOC` as TOML: root scalars, arrays, an array of tables, nested
/// tables and a key that needs quotes. The two `null`s are gone.
const MODEL_TOML: &str = r#"flag = true
n = 1.50
s = "x"
list = [1, 2, 3]
empty = []
"a/b" = 0

[[objs]]
name = "a"
on = true

[[objs]]
name = "b"
on = false

[map]
k1 = 1
k2 = "two"

[nested.deep]
x = [true]
"#;

fn has_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.iter().any(has_null),
        Value::Object(m) => m.values().any(has_null),
        _ => false,
    }
}

/// The oracle: the model accepts the op, the op brings no null, and the root
/// stays a table. `None` means TOML must refuse the op.
fn oracle(model: &Value, op: &DraftOp) -> Option<Value> {
    let mut next = model.clone();
    if !model_apply(&mut next, op) || !next.is_object() {
        return None;
    }
    let json = match op {
        DraftOp::Set { value_json, .. }
        | DraftOp::Add { value_json, .. }
        | DraftOp::ArrayInsert { value_json, .. } => Some(value_json),
        _ => None,
    };
    if json.is_some_and(|j| has_null(&serde_json::from_str(j).unwrap())) {
        return None;
    }
    Some(next)
}

#[test]
fn the_toml_model_holds_the_json_models_data() {
    let mut json: Value = serde_json::from_str(MODEL_DOC).unwrap();
    json["nested"]["deep"]["x"] = serde_json::json!([true]);
    assert_eq!(value_of(MODEL_TOML), json);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Every op behaves like the serde reference model (minus null and
    /// non-table roots), and what would be saved re-parses to exactly it.
    #[test]
    fn toml_ops_match_the_reference_model(sels in sel_strategy()) {
        let mut d = toml(MODEL_TOML);
        prop_assert!(d.is_editable());
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            let want = oracle(&model, &op);
            let result = d.apply(op.clone());
            prop_assert_eq!(result.is_ok(), want.is_some(), "{:?}: {:?}\n{}", op, result.err(), text_of(&d));
            if let Some(next) = want { model = next; }
            prop_assert_eq!(d.to_value().unwrap(), model.clone(), "{:?}", op);
            let reloaded = DocumentSession::load("r", d.candidate_bytes(), DocKind::Toml);
            prop_assert!(reloaded.is_editable(), "{:?}: {:?}\n{}", op, reloaded.diagnostics(), text_of(&d));
            prop_assert_eq!(reloaded.to_value().unwrap(), model.clone(), "{:?}\n{}", op, text_of(&d));
            prop_assert_eq!(text_of(&reloaded), text_of(&d));
        }
    }

    /// Apply N, undo k, redo k equals apply N; undo lands on the exact earlier
    /// bytes; a fresh session replaying the op list agrees.
    #[test]
    fn toml_undo_redo_roundtrip(sels in sel_strategy(), k in 0usize..25) {
        let mut d = toml(MODEL_TOML);
        let mut model = d.to_value().unwrap();
        let mut states = vec![(d.candidate_bytes(), model.clone())];
        for sel in sels {
            let op = gen_op(&model, sel);
            if d.apply(op.clone()).is_ok() {
                model = oracle(&model, &op).expect("applied op is accepted by the model");
                states.push((d.candidate_bytes(), model.clone()));
            }
        }
        let n = states.len() - 1;
        let k = k.min(n);
        for i in 1..=k {
            d.undo().unwrap();
            prop_assert_eq!(&d.candidate_bytes(), &states[n - i].0);
            prop_assert_eq!(d.to_value().unwrap(), states[n - i].1.clone());
            prop_assert_eq!(d.ops().len(), n - i);
        }
        let mut fresh = toml(MODEL_TOML);
        for op in d.ops().to_vec() { fresh.apply(op).unwrap(); }
        prop_assert_eq!(fresh.candidate_bytes(), d.candidate_bytes());
        for i in 1..=k {
            d.redo().unwrap();
            prop_assert_eq!(&d.candidate_bytes(), &states[n - k + i].0);
        }
        prop_assert_eq!(&d.candidate_bytes(), &states[n].0);
        prop_assert!(!d.snapshot(false).can_redo);
        prop_assert_eq!(d.is_dirty(), n > 0);
    }

    /// Reverting an op that nothing later depends on equals never having applied it.
    #[test]
    fn toml_revert_equals_never_applied(sels in sel_strategy(), pick in any::<u16>()) {
        let mut d = toml(MODEL_TOML);
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            if d.apply(op.clone()).is_ok() { model = oracle(&model, &op).unwrap(); }
        }
        if d.ops().is_empty() { return Ok(()); }
        let i = pick as usize % d.ops().len();
        let ops = d.ops().to_vec();
        let revertible = d.review(&no_secrets).changes[i].revertible;
        match d.revert_op(i as u32) {
            Ok(_) => {
                prop_assert!(revertible);
                let mut fresh = toml(MODEL_TOML);
                for (j, op) in ops.iter().enumerate() { if j != i { fresh.apply(op.clone()).unwrap(); } }
                prop_assert_eq!(fresh.candidate_bytes(), d.candidate_bytes());
            }
            Err(e) => {
                prop_assert!(!revertible);
                prop_assert_eq!(e.code, ErrorCode::InvalidOperation);
                prop_assert_eq!(d.ops(), &ops[..]);
            }
        }
    }
}

// ------------------------------------------------------------- sessions

#[test]
fn undo_redo_restore_exact_bytes() {
    let mut d = toml(FIX);
    d.apply(set("/port", "1")).unwrap();
    d.apply(add("/db/ssl", "{\"on\": true}")).unwrap();
    d.apply(remove("/plugins/0")).unwrap();
    d.apply(rename("/db", "database")).unwrap();
    let all = d.candidate_bytes();
    for _ in 0..4 {
        d.undo().unwrap();
    }
    assert_eq!(d.candidate_bytes(), FIX.as_bytes());
    assert!(!d.is_dirty());
    for _ in 0..4 {
        d.redo().unwrap();
    }
    assert_eq!(d.candidate_bytes(), all);
}

#[test]
fn revert_one_op_equals_never_applying_it() {
    let mut d = toml(FIX);
    d.apply(set("/port", "1")).unwrap();
    d.apply(set("/title", "\"T\"")).unwrap();
    d.apply(add("/db/timeout", "3")).unwrap();
    d.revert_op(1).unwrap();
    assert_eq!(
        text_of(&d),
        run(FIX, &[set("/port", "1"), add("/db/timeout", "3")])
    );
}

#[test]
fn review_masks_secret_values_in_hunks_and_changes() {
    let original = fx("secret.toml");
    let mut d = toml(&original);
    d.apply(set("/api_key", "\"sk-test-new-key-7777\""))
        .unwrap();
    d.apply(set("/auth/token", "\"sk-test-new-token-8888\""))
        .unwrap();
    d.apply(set("/auth/password", "\"sk-test-new-pw-6666\""))
        .unwrap();
    d.apply(set("/hosts/0/secret", "\"sk-test-new-host-5555\""))
        .unwrap();
    d.apply(set("/name", "\"visible-name\"")).unwrap();
    let review = d.review(&|p: &str| crate::core::security::redact::default_is_secret(p));
    let shown: String = review
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    for leak in [
        "sk-test-aaaa1111bbbb2222",
        "sk-test-literal-9999",
        "sk-test-line-one",
        "sk-test-line-two",
        "sk-test-in-array-0001",
        "sk-test-new-key-7777",
        "sk-test-new-token-8888",
        "sk-test-new-pw-6666",
        "sk-test-new-host-5555",
    ] {
        assert!(!shown.contains(leak), "{leak} leaked:\n{shown}");
    }
    assert!(
        shown.contains("api_key") && shown.contains("visible-name"),
        "{shown}"
    );
    for c in &review.changes {
        let secret = c.pointer != "/name";
        assert_eq!(c.masked, secret, "{c:?}");
    }
    let revealed: Vec<String> = review
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .flat_map(|l| &l.masks)
        .map(|m| d.reveal_masked(m.id).unwrap())
        .collect();
    for want in [
        "sk-test-aaaa1111bbbb2222",
        "sk-test-new-key-7777",
        "sk-test-line-one",
    ] {
        assert!(revealed.iter().any(|r| r == want), "{want}: {revealed:?}");
    }
}

#[test]
fn secret_lexemes_cover_every_scalar_form() {
    let text = "a = \"sk-1\"\nb = 'sk-2'\nc = \"\"\"\nsk-3a\nsk-3b\"\"\"\nd = 12345\n[t]\ne = [\"sk-4\", {f = \"sk-5\"}]\nplain = \"keep\"\nflag = true\n";
    let d = super::parse(text).ok().unwrap();
    let secrets = |p: &str| p != "/plain" && p != "/t/plain" && p != "/t" && !p.is_empty();
    let mut got = Syntax::secret_lexemes(&d, &secrets);
    got.sort();
    assert_eq!(
        got,
        [
            "\"sk-1\"", "\"sk-4\"", "\"sk-5\"", "'sk-2'", "12345", "sk-3a", "sk-3b"
        ]
    );
}

#[test]
fn conflict_and_reapply_keep_the_disk_change_byte_identical() {
    let mut d = toml(FIX);
    d.apply(set("/port", "1")).unwrap();
    d.apply(add("/db/timeout", "3")).unwrap();
    let disk = FIX
        .replacen("ratio = 1.50", "ratio = 2.50 # edited elsewhere", 1)
        .into_bytes();
    let info = d.conflict(&disk, &no_secrets);
    assert!(info.can_reapply && info.overlapping.is_empty(), "{info:?}");
    assert_eq!(info.disk_changes.len(), 1);
    assert_eq!(info.disk_changes[0].pointer, "/ratio");
    d.reapply_onto(disk.clone()).unwrap();
    let want = String::from_utf8(disk.clone())
        .unwrap()
        .replacen("port = 8080", "port = 1", 1)
        .replacen("retries = 3\n", "retries = 3\ntimeout = 3\n", 1);
    assert_eq!(text_of(&d), want);
    d.undo().unwrap();
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), disk);
    // The same key changed on both sides is an overlap.
    let mut e = toml(FIX);
    e.apply(set("/port", "1")).unwrap();
    let clash = FIX.replacen("port = 8080", "port = 2", 1).into_bytes();
    let info = e.conflict(&clash, &no_secrets);
    assert!(
        !info.can_reapply && info.overlapping == ["/port"],
        "{info:?}"
    );
    assert_eq!(
        e.reapply_onto(clash).unwrap_err().code,
        ErrorCode::ExternalChangeConflict
    );
}

#[test]
fn conflict_masks_secret_values() {
    let original = fx("secret.toml");
    let mut d = toml(&original);
    d.apply(set("/api_key", "\"sk-test-mine-3333\"")).unwrap();
    let disk = original
        .replacen("sk-test-aaaa1111bbbb2222", "sk-test-disk-4444", 1)
        .into_bytes();
    let info = d.conflict(&disk, &|p: &str| p == "/api_key");
    let blob = format!("{:?}", (&info.disk_changes, &info.draft_changes));
    for leak in [
        "sk-test-mine-3333",
        "sk-test-disk-4444",
        "sk-test-aaaa1111bbbb2222",
    ] {
        assert!(!blob.contains(leak), "{leak} in {blob}");
    }
    assert!(info.disk_changes[0].masked && info.draft_changes[0].masked);
}

// -------------------------------------------------------- blocked files

fn blocked(name: &str) -> (DocumentSession, String) {
    let text = fx(name);
    let d = toml(&text);
    assert!(!d.is_editable(), "{name}");
    (d, text)
}

#[track_caller]
fn assert_masked(d: &DocumentSession, text: &str, leaks: &[&str]) {
    let view = d.snapshot(false).source_view.expect("a source view");
    for leak in leaks {
        assert!(!view.text.contains(leak), "{leak} leaked:\n{}", view.text);
    }
    assert!(!view.masks.is_empty());
    // Line numbers stay valid, and every mask reveals its original.
    assert_eq!(view.text.lines().count(), text.lines().count());
    let revealed: Vec<String> = view
        .masks
        .iter()
        .map(|m| d.reveal_masked(m.id).unwrap())
        .collect();
    assert!(
        leaks.iter().all(|l| revealed.iter().any(|r| r.contains(l))),
        "{revealed:?}"
    );
}

#[test]
fn invalid_syntax_reports_position_and_hides_every_value() {
    let (d, text) = blocked("invalid-syntax.toml");
    let diags = d.diagnostics();
    assert_eq!(diags.len(), 1);
    let diag = &diags[0];
    assert_eq!(
        (diag.code.as_str(), diag.source),
        ("INVALID_TOML", DiagnosticSource::Syntax)
    );
    assert_eq!(diag.line, Some(3), "{diag:?}");
    assert!(diag.column.is_some());
    for secret in [
        "sk-test-broken-0001",
        "sk-test-literal-0002",
        "sk-test-in-list-0003",
    ] {
        assert!(!diag.message.contains(secret), "{}", diag.message);
    }
    assert_masked(
        &d,
        &text,
        &[
            "sk-test-broken-0001",
            "sk-test-literal-0002",
            "sk-test-in-list-0003",
        ],
    );
    let view = d.snapshot(false).source_view.unwrap();
    assert!(view.text.contains("api_key") && view.text.contains("[auth]"));
    assert!(view.text.contains("# Broken on purpose"));
    assert_eq!(view.highlight_lines, vec![3]);
    assert_eq!(d.candidate_bytes(), text.as_bytes());
}

#[test]
fn duplicate_keys_and_tables_are_blocked_with_their_line() {
    let (d, text) = blocked("duplicate-key.toml");
    let diag = &d.diagnostics()[0];
    assert_eq!(
        (diag.code.as_str(), diag.source, diag.line),
        ("DUPLICATE_KEYS", DiagnosticSource::DuplicateKey, Some(4))
    );
    assert!(diag.column.is_some());
    assert_masked(&d, &text, &["sk-test-dup-0001", "sk-test-dup-0002"]);
    assert_eq!(
        d.snapshot(false).source_view.unwrap().highlight_lines,
        vec![4]
    );

    let (d, text) = blocked("duplicate-table.toml");
    let diag = &d.diagnostics()[0];
    assert_eq!((diag.code.as_str(), diag.line), ("DUPLICATE_KEYS", Some(4)));
    assert_masked(&d, &text, &["sk-test-host-0003"]);
    assert!(
        d.snapshot(false)
            .source_view
            .unwrap()
            .text
            .contains("[server]")
    );
}

#[test]
fn a_layout_toml_edit_cannot_reprint_opens_read_only() {
    let (d, text) = blocked("interleaved-dotted.toml");
    let diag = &d.diagnostics()[0];
    assert_eq!(diag.code, "UNSUPPORTED_TOML_LAYOUT");
    assert_masked(&d, &text, &["sk-test-dotted-0004"]);
    // Mixed line endings, too.
    let d = toml("a = 1\r\nb = 2\n");
    assert!(!d.is_editable());
    assert_eq!(d.diagnostics()[0].code, "UNSUPPORTED_TOML_LAYOUT");
}

#[test]
fn the_masker_hides_values_in_every_position() {
    let cases: &[(&str, &[&str])] = &[
        (
            "key = \"SECRET\"\n# keep this\n[tbl.sub]\nk2 = 'SECRET'\n",
            &["key", "# keep this", "[tbl.sub]", "k2"],
        ),
        (
            "list = [\n  \"SECRET\", # why\n  12345SECRET,\n]\n",
            &["list", "# why"],
        ),
        (
            "ml = \"\"\"\nSECRET one\nSECRET two\"\"\" # after\nnext = 1\n",
            &["ml", "# after", "next"],
        ),
        (
            "inline = { user = \"SECRET\", pw = 'SECRET' }\n",
            &["inline"],
        ),
        (
            "q = \"a # SECRET b\" # real comment\n",
            &["q", "# real comment"],
        ),
        ("\"quoted key\" = \"SECRET\"\n", &["\"quoted key\""]),
        ("dotted.key.path = SECRET\n", &["dotted.key.path"]),
        (
            "unterminated = \"SECRET\nafter = \"SECRET\"\n",
            &["unterminated", "after"],
        ),
        ("ml = \"\"\"never closed SECRET\nmore SECRET\n", &["ml"]),
        ("SECRET words here\n", &[]),
        (
            "[[arr]]\nname = \"SECRET\"\n[ spaced . header ]\nx = 1\n",
            &["[[arr]]", "[ spaced . header ]", "name"],
        ),
        ("e = \"a\\\"SECRET\\\"b\"\n", &["e"]),
        ("multi = '''\nSECRET\n''' # c\n", &["multi", "# c"]),
        ("d = 1979-05-27 SECRET\n", &["d"]),
        ("[table SECRET\nk = 1\n", &["k"]),
        ("a = [[1, \"SECRET\"], {b = \"SECRET\"}]\n", &["a"]),
        ("\"unclosed key = SECRET\n", &[]),
        ("k = \"\"\"\"SECRET\"\"\"\"\n", &["k"]),
    ];
    for (text, visible) in cases {
        let (view, originals) = mask::mask_source(text, Some(1), Some(1));
        assert!(!view.text.contains("SECRET"), "{text:?} -> {:?}", view.text);
        for v in *visible {
            assert!(
                view.text.contains(v),
                "{v} hidden in {text:?} -> {:?}",
                view.text
            );
        }
        assert!(originals.iter().any(|o| o.contains("SECRET")), "{text:?}");
        assert_eq!(view.text.lines().count(), text.lines().count(), "{text:?}");
    }
}

/// Implicit tables, dotted keys, inline tables, nested arrays of tables.
const RICH_TOML: &str = r#"top.a = 1
top.b = { c = [1, 2] }
when = 1979-05-27

[x.y.z]
k = "v" # note

[x.w]
k = 2

[[x.list]]
n = 1

[[x.list]]
n = 2

[x.list.sub]
s = "t"

[other]
arr = [ { a = 1 }, { a = 2 } ]
"#;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// The same model check on documents with every awkward construct, a
    /// CRLF copy, and one without a final newline.
    #[test]
    fn toml_ops_match_the_model_on_rich_documents(sels in sel_strategy(), which in 0usize..4) {
        let text = match which {
            0 => RICH_TOML.to_string(),
            1 => RICH_TOML.replace('\n', "\r\n"),
            2 => FIX.to_string(),
            _ => FIX.trim_end().to_string(),
        };
        let mut d = toml(&text);
        prop_assert!(d.is_editable(), "{:?}", d.diagnostics());
        let mut model = d.to_value().unwrap();
        let crlf = which == 1;
        for sel in sels {
            let op = gen_op(&model, sel);
            let want = oracle(&model, &op);
            let result = d.apply(op.clone());
            prop_assert_eq!(result.is_ok(), want.is_some(), "{:?}: {:?}\n{}", op, result.err(), text_of(&d));
            if let Some(next) = want { model = next; }
            prop_assert_eq!(d.to_value().unwrap(), model.clone(), "{:?}", op);
            let out = text_of(&d);
            let reloaded = DocumentSession::load("r", d.candidate_bytes(), DocKind::Toml);
            prop_assert!(reloaded.is_editable(), "{:?}: {:?}\n{}", op, reloaded.diagnostics(), out);
            prop_assert_eq!(reloaded.to_value().unwrap(), model.clone(), "{:?}\n{}", op, out);
            if crlf {
                prop_assert_eq!(out.matches('\n').count(), out.matches("\r\n").count(), "{:?}", out);
            }
            if which == 3 {
                prop_assert!(!out.ends_with('\n') || out.is_empty(), "{:?}", out);
            }
        }
    }
}

#[test]
fn normalizing_positions_never_moves_a_table() {
    let docs = [
        FIX.to_string(),
        RICH_TOML.to_string(),
        MODEL_TOML.to_string(),
        fx("app.toml"),
        fx("secret.toml"),
        "[b]\n[a]\n[c.d]\n[a.e]\n[[z]]\n[c]\n".to_string(),
        "[a.b]\nx = 1\n[c]\ny = 2\n[a.d]\nz = 3\n[[a.l]]\n[[c.l]]\n[[a.l]]\n".to_string(),
        "x.y = 1\n[p]\n[[p.q]]\n[p.q.r]\n[[p.q]]\n".to_string(),
    ];
    for text in docs {
        let mut d = super::parse(&text).ok().unwrap();
        super::layout::normalize(&mut d.doc);
        assert_eq!(d.render(), text);
    }
}

#[test]
fn inline_array_spacing_follows_the_neighbours() {
    let text = "a = [ { x = 1 }, { x = 2 } ]\nb = [\n  1, # one\n  2, # two\n  3, # three\n]\n";
    assert_eq!(
        run(text, &[insert("/a", 2, "{\"x\": 3}")]),
        text.replacen("{ x = 2 } ]", "{ x = 2 }, { x = 3 } ]", 1)
    );
    assert_eq!(
        run(text, &[insert("/a", 1, "{\"x\": 9}")]),
        text.replacen("{ x = 1 },", "{ x = 1 }, { x = 9 },", 1)
    );
    assert_eq!(
        run(text, &[remove("/a/1")]),
        text.replacen(", { x = 2 } ]", " ]", 1)
    );
    assert_eq!(
        run(text, &[remove("/a/0")]),
        text.replacen("[ { x = 1 }, ", "[ ", 1)
    );
    // A removed value takes its own comment; its neighbours keep theirs.
    assert_eq!(
        run(text, &[remove("/b/1")]),
        text.replacen("  2, # two\n", "", 1)
    );
    assert_eq!(
        run(text, &[remove("/b/2")]),
        text.replacen("  3, # three\n", "", 1)
    );
    assert_eq!(
        run(text, &[insert("/b", 3, "4")]),
        text.replacen("  3, # three\n", "  3, # three\n  4,\n", 1)
    );
}
