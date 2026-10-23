//! Document engine tests. Helpers marked `pub(super)` (op builders, the
//! serde reference model `model_apply`, `gen_op`/`sel_strategy`) are shared
//! with the YAML and TOML syntax tests via `super::tests::…`.

use proptest::prelude::*;
use serde_json::{Value, json};

use super::*;
use crate::core::types::{DiagnosticSource, DiffLineKind, NodeKind};

// ---------------------------------------------------------------- helpers

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn kind_of_name(name: &str) -> DocKind {
    if name.ends_with(".jsonc") {
        DocKind::Jsonc
    } else {
        DocKind::Json
    }
}

fn open(name: &str) -> DocumentSession {
    DocumentSession::load(name, fixture(name), kind_of_name(name))
}

pub(super) fn load_text(text: &str, kind: DocKind) -> DocumentSession {
    DocumentSession::load("t", text.as_bytes().to_vec(), kind)
}

pub(super) fn text_of(sess: &DocumentSession) -> String {
    String::from_utf8(sess.candidate_bytes()).unwrap()
}

pub(super) fn set(path: &str, v: &str) -> DraftOp {
    DraftOp::Set {
        path: path.into(),
        value_json: v.into(),
    }
}
pub(super) fn add(path: &str, v: &str) -> DraftOp {
    DraftOp::Add {
        path: path.into(),
        value_json: v.into(),
    }
}
pub(super) fn remove(path: &str) -> DraftOp {
    DraftOp::Remove { path: path.into() }
}
pub(super) fn insert(path: &str, index: u32, v: &str) -> DraftOp {
    DraftOp::ArrayInsert {
        path: path.into(),
        index,
        value_json: v.into(),
    }
}
pub(super) fn mv(path: &str, from: u32, to: u32) -> DraftOp {
    DraftOp::ArrayMove {
        path: path.into(),
        from,
        to,
    }
}
pub(super) fn rename(path: &str, new_key: &str) -> DraftOp {
    DraftOp::RenameKey {
        path: path.into(),
        new_key: new_key.into(),
    }
}

pub(super) fn no_secrets(_: &str) -> bool {
    false
}

fn err_code<T: std::fmt::Debug>(r: CoreResult<T>) -> ErrorCode {
    r.unwrap_err().code
}

fn assert_send<T: Send>() {}

// ------------------------------------------------------- loading fixtures

#[test]
fn session_is_send() {
    assert_send::<DocumentSession>();
}

#[test]
fn every_fixture_loads_with_expected_state() {
    // (name, editable, diagnostic code when blocked)
    let table: &[(&str, bool, Option<&str>)] = &[
        ("bom.json", true, None),
        ("claude-settings.json", true, None),
        ("commented.jsonc", true, None),
        ("crlf-no-eol.json", true, None),
        ("deep.json", true, None),
        ("duplicate-keys.json", false, Some("DUPLICATE_KEYS")),
        ("duplicate-nested.json", false, Some("DUPLICATE_KEYS")),
        (
            "invalid-missing-comma-secret.json",
            false,
            Some("INVALID_JSON"),
        ),
        (
            "invalid-unterminated-secret.json",
            false,
            Some("INVALID_JSON"),
        ),
        ("empty-file.json", false, Some("EMPTY_FILE")),
        ("empty-object.json", true, None),
        ("feature-flags.json", true, None),
        ("invalid-comment.json", false, Some("INVALID_JSON")),
        ("invalid-missing-comma.json", false, Some("INVALID_JSON")),
        ("invalid-utf8.json", false, Some("UNSUPPORTED_ENCODING")),
        ("numbers.json", true, None),
        ("pointer-escapes.json", true, None),
        ("secret.json", true, None),
    ];
    for (name, editable, code) in table {
        let d = open(name);
        assert_eq!(d.is_editable(), *editable, "{name}");
        let diags = d.diagnostics();
        match code {
            Some(code) => assert_eq!(diags[0].code, *code, "{name}"),
            None => assert!(diags.is_empty(), "{name}: {diags:?}"),
        }
        let snap = d.snapshot(false);
        assert_eq!(snap.editable, *editable, "{name}");
        assert_eq!(snap.root.is_some(), *editable, "{name}");
        assert_eq!(snap.fingerprint, fingerprint(&fixture(name)));
        // Every fixture round-trips byte for byte (A13), blocked or not.
        assert_eq!(d.candidate_bytes(), fixture(name), "{name}");
        if *editable {
            assert_eq!(
                d.root.as_ref().unwrap().render().as_bytes(),
                &fixture(name)[d.bom_len()..],
                "{name}"
            );
            assert!(!d.is_dirty());
            assert!(d.to_value().is_some());
        } else {
            assert!(d.to_value().is_none());
        }
    }
}

#[test]
fn a22_invalid_json_reports_exact_position_and_masked_source() {
    let d = open("invalid-missing-comma.json");
    let diag = &d.diagnostics()[0];
    // The parser points at where the comma is missing: just after `1` on line 2.
    assert_eq!(
        (diag.line, diag.column, diag.source),
        (Some(2), Some(9), DiagnosticSource::Syntax)
    );
    let view = d.snapshot(false).source_view.unwrap();
    assert_eq!(view.highlight_lines, vec![2]);
    assert_eq!(view.caret.map(|c| c.line), Some(2));
    assert_eq!(
        err_code(open("invalid-missing-comma.json").apply(set("/a", "2"))),
        ErrorCode::NotEditable
    );
}

#[test]
fn a11_comment_in_strict_json_is_reported_and_not_rewritten() {
    let mut d = open("invalid-comment.json");
    let diag = &d.diagnostics()[0];
    assert_eq!(diag.message, "Comments are not allowed in .json files");
    assert_eq!(diag.line, Some(2));
    assert_eq!(err_code(d.apply(set("/a", "2"))), ErrorCode::NotEditable);
    assert_eq!(d.candidate_bytes(), fixture("invalid-comment.json"));
    // The same bytes are fine as .jsonc.
    let ok = DocumentSession::load("x", fixture("invalid-comment.json"), DocKind::Jsonc);
    assert!(ok.is_editable());
}

#[test]
fn trailing_comma_is_strict_json_error_but_jsonc_ok() {
    let text = "{\"a\": [1,],}";
    let d = load_text(text, DocKind::Json);
    assert_eq!(
        d.diagnostics()[0].message,
        "Trailing commas are not allowed in .json files"
    );
    assert!(load_text(text, DocKind::Jsonc).is_editable());
    // Nothing JSON5-ish, even in .jsonc.
    for bad in [
        "{'a': 1}",
        "{a: 1}",
        "{\"a\": 0x10}",
        "{\"a\": +1}",
        "{\"a\": .5}",
        "{\"a\": NaN}",
        "{\"a\": 1 \"b\": 2}",
    ] {
        assert!(!load_text(bad, DocKind::Jsonc).is_editable(), "{bad}");
    }
}

#[test]
fn a23_duplicate_keys_are_blocked_with_every_occurrence() {
    let d = open("duplicate-keys.json");
    assert!(!d.is_editable());
    let diag = &d.diagnostics()[0];
    assert_eq!(
        (diag.code.as_str(), diag.source),
        ("DUPLICATE_KEYS", DiagnosticSource::DuplicateKey)
    );
    assert_eq!(diag.pointer.as_deref(), Some("/a"));
    assert!(
        diag.message.contains("`a`") && diag.message.contains("2 times"),
        "{}",
        diag.message
    );
    assert_eq!(d.candidate_bytes(), fixture("duplicate-keys.json"));
}

#[test]
fn duplicates_nested_and_multiline_list_every_line() {
    let text = "{\n  \"x\": 1,\n  \"o\": {\n    \"k\": 1,\n    \"j\": [{\"d\": 1,\n \"d\": 2}],\n    \"k\": 2,\n    \"k\": 3\n  },\n  \"x\": 4\n}\n";
    let d = load_text(text, DocKind::Json);
    let diags = d.diagnostics();
    assert_eq!(diags.len(), 3, "{diags:?}");
    let by_ptr = |p: &str| {
        diags
            .iter()
            .find(|d| d.pointer.as_deref() == Some(p))
            .unwrap()
    };
    assert_eq!(
        by_ptr("/x").message,
        "Key `x` appears 2 times: lines 2 and 10"
    );
    assert_eq!(
        by_ptr("/o/k").message,
        "Key `k` appears 3 times: lines 4, 7 and 8"
    );
    assert_eq!(
        by_ptr("/o/j/0/d").message,
        "Key `d` appears 2 times: lines 5 and 6"
    );
    assert_eq!(by_ptr("/o/k").column, Some(5));
    let view = d.snapshot(false).source_view.unwrap();
    assert_eq!(view.highlight_lines, vec![2, 4, 5, 6, 7, 8, 10]);
    // Different objects with the same key are fine.
    assert!(
        load_text(
            "{\"a\": {\"k\": 1}, \"b\": {\"k\": 2}, \"c\": [{\"k\": 1}, {\"k\": 2}]}",
            DocKind::Json
        )
        .is_editable()
    );
}

#[test]
fn empty_and_non_utf8_and_whitespace_only() {
    for text in ["", "  \n\t ", "// only a comment\n"] {
        let d = load_text(text, DocKind::Jsonc);
        assert!(!d.is_editable());
        assert_eq!(d.diagnostics()[0].code, "EMPTY_FILE");
        assert_eq!(d.diagnostics()[0].source, DiagnosticSource::File);
    }
    let d = open("invalid-utf8.json");
    assert_eq!(d.diagnostics()[0].code, "UNSUPPORTED_ENCODING");
    assert!(d.snapshot(false).source_view.is_none());
    // A BOM-only file is empty.
    assert_eq!(
        DocumentSession::load("b", vec![0xEF, 0xBB, 0xBF], DocKind::Json).diagnostics()[0].code,
        "EMPTY_FILE"
    );
}

#[test]
fn line_ending_detection() {
    assert_eq!(
        open("crlf-no-eol.json").snapshot(false).line_ending,
        LineEnding::Crlf
    );
    assert_eq!(
        open("feature-flags.json").snapshot(false).line_ending,
        LineEnding::Lf
    );
    assert_eq!(
        open("deep.json").snapshot(false).line_ending,
        LineEnding::Lf
    );
    assert_eq!(
        load_text("{}", DocKind::Json).snapshot(false).line_ending,
        LineEnding::None
    );
    assert_eq!(
        load_text("{\r\n\"a\":\n1}", DocKind::Json)
            .snapshot(false)
            .line_ending,
        LineEnding::Mixed
    );
}

// -------------------------------------------------- byte-exact editing

#[test]
fn a02_toggle_changes_exactly_one_token() {
    let original = String::from_utf8(fixture("feature-flags.json")).unwrap();
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    assert_eq!(
        text_of(&d),
        original.replacen("\"enabled\": true,", "\"enabled\": false,", 1)
    );
    d.apply(set("/logging/toFile", "true")).unwrap();
    assert_eq!(
        text_of(&d),
        original
            .replacen("\"enabled\": true,", "\"enabled\": false,", 1)
            .replacen("\"toFile\": false", "\"toFile\": true", 1)
    );
}

#[test]
fn a03_toggle_back_is_reversible_without_coercion() {
    let original = fixture("feature-flags.json");
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(set("/enabled", "true")).unwrap();
    assert_eq!(d.candidate_bytes(), original);
    assert_eq!(d.to_value().unwrap()["enabled"], Value::Bool(true));
    d.undo().unwrap();
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), original);
    assert!(!d.is_dirty());
}

#[test]
fn a04_absent_optional_stays_absent_until_added() {
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    assert!(d.to_value().unwrap().get("beta").is_none());
    assert!(!text_of(&d).contains("beta"));
    d.apply(add("/beta", "false")).unwrap();
    assert_eq!(d.to_value().unwrap()["beta"], Value::Bool(false));
}

#[test]
fn a12_jsonc_scalar_edit_keeps_comments_and_trailing_commas() {
    let original = String::from_utf8(fixture("commented.jsonc")).unwrap();
    let mut d = open("commented.jsonc");
    d.apply(set("/enabled", "false")).unwrap();
    assert_eq!(
        text_of(&d),
        original.replacen("\"enabled\": true", "\"enabled\": false", 1)
    );
    d.apply(set("/items/0", "\"changed\"")).unwrap();
    assert_eq!(
        text_of(&d),
        original
            .replacen("\"enabled\": true", "\"enabled\": false", 1)
            .replacen("\"first\", //", "\"changed\", //", 1)
    );
    // Trailing commas and both comments survive.
    assert!(text_of(&d).contains("// Keep this explanation next to the value."));
    assert!(text_of(&d).contains("],\n}"));
}

#[test]
fn a13_noop_round_trip_all_editable_fixtures() {
    for name in [
        "bom.json",
        "commented.jsonc",
        "crlf-no-eol.json",
        "feature-flags.json",
        "numbers.json",
        "pointer-escapes.json",
    ] {
        let d = open(name);
        assert_eq!(d.candidate_bytes(), fixture(name), "{name}");
        assert_eq!(
            d.root.as_ref().unwrap().render().as_bytes(),
            &fixture(name)[d.bom_len()..],
            "{name}"
        );
    }
}

#[test]
fn crlf_and_missing_final_newline_survive_edits() {
    let original = String::from_utf8(fixture("crlf-no-eol.json")).unwrap();
    let mut d = open("crlf-no-eol.json");
    d.apply(set("/enabled", "false")).unwrap();
    assert_eq!(text_of(&d), original.replacen("true", "false", 1));
    d.apply(add("/added", "{\"x\": [1, 2]}")).unwrap();
    d.apply(remove("/n")).unwrap();
    let out = text_of(&d);
    assert!(!out.ends_with('\n'), "{out:?}");
    assert!(
        out.matches('\n').count() == out.matches("\r\n").count(),
        "bare LF in {out:?}"
    );
    assert_eq!(d.snapshot(false).line_ending, LineEnding::Crlf);
}

#[test]
fn r10_bom_is_stripped_for_parsing_and_restored_on_save() {
    let original = fixture("bom.json");
    let mut d = open("bom.json");
    let snap = d.snapshot(false);
    assert!(snap.has_bom && snap.editable);
    assert_eq!(snap.size_bytes, original.len() as u64);
    assert_eq!(d.candidate_bytes(), original);
    d.apply(set("/a", "false")).unwrap();
    let out = d.candidate_bytes();
    assert_eq!(&out[..3], &[0xEF, 0xBB, 0xBF]);
    let mut want = original.clone();
    let at = want.windows(4).position(|w| w == b"true").unwrap();
    want.splice(at..at + 4, b"false".iter().copied());
    assert_eq!(out, want);
    assert_eq!(d.base_fingerprint(), &fingerprint(&original));
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), original);
}

#[test]
fn numbers_keep_their_lexemes_through_tree_and_unrelated_edit() {
    let mut d = open("numbers.json");
    let root = d.snapshot(false).root.unwrap();
    let texts: Vec<_> = root
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| (c.key.clone().unwrap(), c.text.clone().unwrap()))
        .collect();
    assert_eq!(
        texts,
        [
            ("precise", "1.50"),
            ("exp", "1e3"),
            ("big", "9007199254740993"),
            ("neg", "-0.0")
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()))
    );
    assert!(
        root.children
            .unwrap()
            .iter()
            .all(|c| c.kind == NodeKind::Number)
    );
    d.apply(add("/other", "true")).unwrap();
    let out = text_of(&d);
    for lexeme in ["1.50", "1e3", "9007199254740993", "-0.0"] {
        assert!(out.contains(lexeme), "{lexeme} lost in {out}");
    }
    // The tree after the edit still shows exact lexemes.
    assert_eq!(
        d.subtree("/big", 0).unwrap().text.as_deref(),
        Some("9007199254740993")
    );
    // Editing a number keeps the new lexeme exactly (A05).
    d.apply(set("/precise", "2.50")).unwrap();
    d.apply(set("/big", "9007199254740995")).unwrap();
    assert!(text_of(&d).contains("\"precise\": 2.50") && text_of(&d).contains("9007199254740995"));
    for bad in ["abc", "1.", "01", "", "NaN", "0x1", "1 2"] {
        assert!(d.apply(set("/precise", bad)).is_err(), "{bad}");
    }
}

// ------------------------------------------------------------- tree DTO

#[test]
fn pointer_escapes_in_tree_and_edits() {
    let mut d = open("pointer-escapes.json");
    let root = d.snapshot(false).root.unwrap();
    let pointers: Vec<_> = root
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| c.pointer.clone())
        .collect();
    assert_eq!(pointers, ["/a~0b", "/c~1d", "/unicode", "/empty", "/list"]);
    assert_eq!(root.child_count, Some(5));
    assert_eq!(
        d.subtree("/unicode", 0).unwrap().text.as_deref(),
        Some("Grüße 😀")
    );
    d.apply(set("/a~0b", "10")).unwrap();
    d.apply(set("/c~1d", "20")).unwrap();
    let v = d.to_value().unwrap();
    assert_eq!((v["a~b"].clone(), v["c/d"].clone()), (json!(10), json!(20)));
    let r = d.apply(rename("/c~1d", "e/f~g")).unwrap();
    assert_eq!(r.removed, ["/c~1d"]);
    assert!(
        r.changed
            .iter()
            .any(|n| n.pointer == "/e~1f~0g" && n.key.as_deref() == Some("e/f~g"))
    );
    assert_eq!(
        err_code(d.apply(set("/c~1d", "1"))),
        ErrorCode::InvalidOperation
    );
}

#[test]
fn snapshot_is_lazy_and_subtree_loads_on_demand() {
    let d = open("feature-flags.json");
    let root = d.snapshot(false).root.unwrap();
    assert_eq!(
        (
            root.kind,
            root.pointer.as_str(),
            root.key.clone(),
            root.index
        ),
        (NodeKind::Object, "", None, None)
    );
    let kids = root.children.unwrap();
    assert_eq!(kids.len(), 11);
    assert!(kids.iter().all(|k| k.children.is_none()));
    let logging = kids
        .iter()
        .find(|k| k.key.as_deref() == Some("logging"))
        .unwrap();
    assert_eq!(
        (logging.kind, logging.child_count),
        (NodeKind::Object, Some(2))
    );
    let services = d.subtree("/services", 2).unwrap();
    let first = &services.children.as_ref().unwrap()[0];
    assert_eq!(
        (first.index, first.key.clone(), first.pointer.as_str()),
        (Some(0), None, "/services/0")
    );
    assert_eq!(
        first.children.as_ref().unwrap()[0].pointer,
        "/services/0/name"
    );
    assert_eq!(
        first.children.as_ref().unwrap()[0].text.as_deref(),
        Some("frontend")
    );
    assert_eq!(d.subtree("/optionalValue", 1).unwrap().kind, NodeKind::Null);
    assert_eq!(
        d.subtree("/optionalValue", 1).unwrap().text.as_deref(),
        Some("null")
    );
    assert_eq!(
        d.subtree("/debug", 1).unwrap().text.as_deref(),
        Some("false")
    );
    assert_eq!(
        d.subtree("/missing", 1).unwrap_err().code,
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        d.subtree("bad", 1).unwrap_err().code,
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        open("duplicate-keys.json").subtree("", 1).unwrap_err().code,
        ErrorCode::NotEditable
    );
    // Deep file: depth bounds the load.
    let deep = open("deep.json");
    let n = deep.subtree("/a/b", 2).unwrap();
    assert!(n.children.as_ref().unwrap()[0].children.is_some());
    assert!(
        n.children.as_ref().unwrap()[0].children.as_ref().unwrap()[0]
            .children
            .is_none()
    );
}

#[test]
fn changed_flag_follows_edits() {
    let mut d = open("feature-flags.json");
    let before = d.snapshot(false).root.unwrap();
    assert!(!before.changed && before.children.unwrap().iter().all(|c| !c.changed));
    d.apply(set("/logging/toFile", "true")).unwrap();
    let after = d.snapshot(false).root.unwrap();
    assert!(after.changed);
    let changed: Vec<_> = after
        .children
        .unwrap()
        .into_iter()
        .filter(|c| c.changed)
        .map(|c| c.pointer)
        .collect();
    assert_eq!(changed, ["/logging"]);
    let logging = d.subtree("/logging", 1).unwrap();
    assert!(
        logging
            .children
            .unwrap()
            .iter()
            .any(|c| c.pointer == "/logging/toFile" && c.changed)
    );
    d.undo().unwrap();
    assert!(!d.snapshot(false).root.unwrap().changed);
}

// ---------------------------------------------------------------- ops

#[test]
fn set_errors_and_results() {
    let mut d = open("feature-flags.json");
    assert_eq!(
        err_code(d.apply(set("/nope", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(set("/features/9", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(set("/features/01", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(set("/enabled/x", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(set("enabled", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(d.revision(), 0);
    assert!(!d.is_dirty());
    let r = d.apply(set("/retryCount", "5")).unwrap();
    assert_eq!(
        (r.revision, r.dirty_count, r.can_undo, r.can_redo),
        (1, 1, true, false)
    );
    assert_eq!(r.changed.len(), 1);
    assert_eq!(
        (
            r.changed[0].pointer.as_str(),
            r.changed[0].text.as_deref(),
            r.changed[0].changed
        ),
        ("/retryCount", Some("5"), true)
    );
    assert!(r.removed.is_empty() && r.reshaped.is_empty() && r.diagnostics.is_empty());
    // Type changes are allowed (scalar -> object -> array -> null) and the node follows.
    let r = d.apply(set("/retryCount", "{\"a\": [1, 2]}")).unwrap();
    assert_eq!(r.changed[0].kind, NodeKind::Object);
    d.apply(set("/retryCount", "null")).unwrap();
    d.apply(set("/retryCount", "[]")).unwrap();
    d.apply(set("/services/1", "{\"name\": \"x\"}")).unwrap();
    assert_eq!(d.to_value().unwrap()["services"][1], json!({"name": "x"}));
}

#[test]
fn set_replaces_the_root() {
    let mut d = open("empty-object.json");
    d.apply(set("", "{\"a\": 1}")).unwrap();
    assert_eq!(d.to_value().unwrap(), json!({"a": 1}));
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), fixture("empty-object.json"));
}

#[test]
fn value_json_must_be_strict_json() {
    let mut d = open("commented.jsonc");
    for bad in [
        "{\"a\": 1,}",
        "[1,]",
        "// c\n1",
        "1 // c",
        "/* c */ 1",
        "{\"a\": 1, \"a\": 2}",
        "'x'",
        "{a: 1}",
        "undefined",
        "1, 2",
        "{\"k\": }",
    ] {
        assert!(d.apply(set("/enabled", bad)).is_err(), "{bad}");
        assert!(d.apply(add("/new", bad)).is_err(), "{bad}");
    }
    assert!(!d.is_dirty());
    assert_eq!(
        err_code(d.apply(set("/enabled", "{\"a\": 1, \"a\": 2}"))),
        ErrorCode::DuplicateKeys
    );
    // Whitespace around a value is fine; strings keep escapes and unicode.
    d.apply(set("/enabled", "  \"line\\nbreak \\u00e9 \\\"q\\\" 😀\"  "))
        .unwrap();
    assert_eq!(
        d.to_value().unwrap()["enabled"],
        json!("line\nbreak é \"q\" 😀")
    );
}

#[test]
fn a07_add_rename_remove_map_property() {
    let mut d = open("claude-settings.json");
    let r = d.apply(add("/env/NEW_FLAG", "\"1\"")).unwrap();
    assert_eq!(r.reshaped, ["/env"]);
    assert!(
        r.changed
            .iter()
            .any(|n| n.pointer == "/env" && n.child_count == Some(2))
    );
    assert!(r.changed.iter().any(|n| n.pointer == "/env/NEW_FLAG"));
    // Appended: last member of env.
    let keys: Vec<_> = d.to_value().unwrap()["env"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert!(keys.contains(&"NEW_FLAG".to_string()));
    let text = text_of(&d);
    assert!(text.find("EXAMPLE_NON_SECRET_FLAG").unwrap() < text.find("NEW_FLAG").unwrap());
    // Duplicates are rejected: add and rename.
    assert_eq!(
        err_code(d.apply(add("/env/NEW_FLAG", "\"2\""))),
        ErrorCode::DuplicateKeys
    );
    assert_eq!(
        err_code(d.apply(rename("/env/NEW_FLAG", "EXAMPLE_NON_SECRET_FLAG"))),
        ErrorCode::DuplicateKeys
    );
    // Add errors: parent missing, parent not an object, root.
    assert_eq!(
        err_code(d.apply(add("/nope/x", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(add("/permissions/allow/0", "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(err_code(d.apply(add("", "1"))), ErrorCode::InvalidOperation);
    d.apply(rename("/env/NEW_FLAG", "RENAMED")).unwrap();
    assert!(d.to_value().unwrap()["env"].get("NEW_FLAG").is_none());
    d.apply(remove("/env/RENAMED")).unwrap();
    d.apply(remove("/env/EXAMPLE_NON_SECRET_FLAG")).unwrap();
    assert_eq!(d.to_value().unwrap()["env"], json!({}));
    assert_eq!(d.ops().len(), 4);
    assert_eq!(
        err_code(d.apply(remove("/env/RENAMED"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(err_code(d.apply(remove(""))), ErrorCode::InvalidOperation);
}

#[test]
fn remove_result_lists_removed_and_reshaped() {
    let mut d = open("feature-flags.json");
    let r = d.apply(remove("/logging")).unwrap();
    assert_eq!(r.removed, ["/logging"]);
    assert_eq!(r.reshaped, [""]);
    assert!(
        r.changed
            .iter()
            .any(|n| n.pointer.is_empty() && n.child_count == Some(10))
    );
    let r = d.undo().unwrap();
    assert!(r.removed.is_empty());
    assert!(
        r.changed
            .iter()
            .any(|n| n.pointer == "/logging" && n.child_count == Some(2))
    );
    assert_eq!(d.candidate_bytes(), fixture("feature-flags.json"));
}

#[test]
fn a06_array_insert_remove_move() {
    let mut d = open("feature-flags.json");
    let r = d.apply(insert("/features", 1, "\"mid\"")).unwrap();
    assert_eq!(r.reshaped, ["/features"]);
    assert_eq!(
        d.to_value().unwrap()["features"],
        json!(["search", "mid", "analytics"])
    );
    d.apply(insert("/features", 3, "\"end\"")).unwrap();
    d.apply(insert("/features", 0, "{\"o\": 1}")).unwrap();
    assert_eq!(
        d.to_value().unwrap()["features"],
        json!([{"o": 1}, "search", "mid", "analytics", "end"])
    );
    assert_eq!(
        err_code(d.apply(insert("/features", 9, "1"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(insert("/enabled", 0, "1"))),
        ErrorCode::InvalidOperation
    );
    d.apply(mv("/features", 0, 4)).unwrap();
    assert_eq!(
        d.to_value().unwrap()["features"],
        json!(["search", "mid", "analytics", "end", {"o": 1}])
    );
    d.apply(mv("/features", 3, 1)).unwrap();
    assert_eq!(
        d.to_value().unwrap()["features"],
        json!(["search", "end", "mid", "analytics", {"o": 1}])
    );
    d.apply(mv("/features", 2, 2)).unwrap();
    assert_eq!(
        err_code(d.apply(mv("/features", 0, 5))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(mv("/features", 5, 0))),
        ErrorCode::InvalidOperation
    );
    d.apply(remove("/features/4")).unwrap();
    d.apply(remove("/features/1")).unwrap();
    assert_eq!(
        d.to_value().unwrap()["features"],
        json!(["search", "mid", "analytics"])
    );
    // Unrelated members are byte-identical.
    let original = String::from_utf8(fixture("feature-flags.json")).unwrap();
    let tail = &original[original.find("\"services\"").unwrap()..];
    assert!(text_of(&d).ends_with(tail));
}

#[test]
fn array_move_keeps_comments_with_their_elements() {
    let mut d = open("commented.jsonc");
    d.apply(mv("/items", 0, 1)).unwrap();
    let out = text_of(&d);
    assert!(
        out.contains("\"second\",\n    \"first\", // Comment attached to first item."),
        "{out}"
    );
    assert_eq!(d.to_value().unwrap()["items"], json!(["second", "first"]));
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), fixture("commented.jsonc"));
}

#[test]
fn rename_key_keeps_position_value_and_inner_comments() {
    let text = "{\n  \"a\": 1,\n  /* before */ \"b\": {\n    // inside\n    \"c\": 2\n  }, // after b\n  \"d\": 3\n}\n";
    let mut d = load_text(text, DocKind::Jsonc);
    let r = d.apply(rename("/b", "bb")).unwrap();
    assert_eq!(text_of(&d), text.replacen("\"b\"", "\"bb\"", 1));
    assert_eq!(r.removed, ["/b"]);
    assert_eq!(r.reshaped, [""]);
    assert!(
        r.changed
            .iter()
            .any(|n| n.pointer == "/bb" && n.key.as_deref() == Some("bb"))
    );
    d.apply(rename("/bb", "bb")).unwrap(); // same name: no-op, no error
    assert_eq!(
        err_code(d.apply(rename("/a", "d"))),
        ErrorCode::DuplicateKeys
    );
    assert_eq!(
        err_code(d.apply(rename("/zz", "q"))),
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        err_code(d.apply(rename("", "q"))),
        ErrorCode::InvalidOperation
    );
    let mut arr = load_text("[1]", DocKind::Json);
    assert_eq!(
        err_code(arr.apply(rename("/0", "q"))),
        ErrorCode::InvalidOperation
    );
    // Keys that need escaping.
    d.apply(rename("/d", "quote\"back\\slash")).unwrap();
    assert_eq!(
        d.to_value().unwrap().get("quote\"back\\slash"),
        Some(&json!(3))
    );
}

#[test]
fn a08_nested_edit_uses_the_right_pointer() {
    let mut d = open("claude-settings.json");
    d.apply(set("/unknownFutureVendorSetting/enabled", "true"))
        .unwrap();
    d.apply(remove("/permissions/deny/0")).unwrap();
    d.apply(insert("/permissions/allow", 1, "\"Bash(ls)\""))
        .unwrap();
    let v = d.to_value().unwrap();
    assert_eq!(v["unknownFutureVendorSetting"]["enabled"], json!(true));
    assert_eq!(v["permissions"]["deny"], json!([]));
    assert_eq!(
        v["permissions"]["allow"],
        json!(["Bash(git status)", "Bash(ls)"])
    );
    assert_eq!(v["model"], json!("example-model-name"));
}

// --------------------------------------------- undo / redo / revert / misc

#[test]
fn undo_redo_basics() {
    let mut d = open("feature-flags.json");
    assert_eq!(err_code(d.undo()), ErrorCode::InvalidOperation);
    assert_eq!(err_code(d.redo()), ErrorCode::InvalidOperation);
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(set("/debug", "true")).unwrap();
    let r = d.undo().unwrap();
    assert_eq!((r.dirty_count, r.can_undo, r.can_redo), (1, true, true));
    assert_eq!(r.changed[0].pointer, "/debug");
    assert_eq!(r.changed[0].text.as_deref(), Some("false"));
    assert_eq!(d.revision(), 3);
    let r = d.redo().unwrap();
    assert_eq!((r.dirty_count, r.can_undo, r.can_redo), (2, true, false));
    d.undo().unwrap();
    // A new edit clears the redo stack.
    d.apply(set("/retryCount", "1")).unwrap();
    assert_eq!(err_code(d.redo()), ErrorCode::InvalidOperation);
    assert_eq!(d.ops().len(), 2);
}

#[test]
fn discard_returns_to_the_original() {
    let mut d = open("commented.jsonc");
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(add("/x", "1")).unwrap();
    d.undo().unwrap();
    d.discard();
    assert!(!d.is_dirty());
    let snap = d.snapshot(false);
    assert!(!snap.can_undo && !snap.can_redo && snap.dirty_count == 0);
    assert_eq!(d.candidate_bytes(), fixture("commented.jsonc"));
    d.apply(set("/enabled", "false")).unwrap();
    assert!(text_of(&d).contains("false"));
}

#[test]
fn revert_op_removes_one_independent_op() {
    let original = String::from_utf8(fixture("feature-flags.json")).unwrap();
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(set("/debug", "true")).unwrap();
    d.apply(set("/logging/level", "\"warn\"")).unwrap();
    let r = d.revert_op(1).unwrap();
    assert_eq!(r.dirty_count, 2);
    assert_eq!(r.changed[0].pointer, "/debug");
    assert_eq!(
        text_of(&d),
        original
            .replacen("true,\n  \"debug\"", "false,\n  \"debug\"", 1)
            .replacen("\"info\"", "\"warn\"", 1)
    );
    assert_eq!(d.ops().len(), 2);
    assert_eq!(err_code(d.revert_op(5)), ErrorCode::InvalidOperation);
    d.revert_op(0).unwrap();
    d.revert_op(0).unwrap();
    assert_eq!(d.candidate_bytes(), fixture("feature-flags.json"));
}

#[test]
fn revert_is_blocked_by_dependent_later_ops() {
    let mut d = open("feature-flags.json");
    d.apply(add("/section", "{\"a\": 1}")).unwrap(); // 0
    d.apply(set("/section/a", "2")).unwrap(); // 1: child of 0
    d.apply(set("/enabled", "false")).unwrap(); // 2: independent
    d.apply(insert("/features", 0, "\"x\"")).unwrap(); // 3
    d.apply(remove("/features/2")).unwrap(); // 4: item of the same array
    d.apply(rename("/logging", "log")).unwrap(); // 5
    d.apply(set("/log/level", "\"warn\"")).unwrap(); // 6: under the new name
    let revertible: Vec<_> = d
        .review(&no_secrets)
        .changes
        .iter()
        .map(|c| c.revertible)
        .collect();
    assert_eq!(revertible, [false, true, true, false, true, false, true]);
    let before = d.candidate_bytes();
    let e = d.revert_op(0).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidOperation);
    assert!(
        e.message.contains("Revert later changes under /section"),
        "{}",
        e.message
    );
    assert_eq!(err_code(d.revert_op(3)), ErrorCode::InvalidOperation);
    assert_eq!(err_code(d.revert_op(5)), ErrorCode::InvalidOperation);
    assert_eq!(d.candidate_bytes(), before);
    assert_eq!(d.ops().len(), 7);
    // Reverting the dependent first frees the parent.
    d.revert_op(1).unwrap();
    d.revert_op(0).unwrap();
    assert!(d.to_value().unwrap().get("section").is_none());
    // Siblings under different parents are independent.
    let mut e = open("feature-flags.json");
    e.apply(set("/logging/level", "\"warn\"")).unwrap();
    e.apply(set("/logging/toFile", "true")).unwrap();
    assert!(e.revert_op(0).is_ok());
}

// ----------------------------------------------------- model-based tests

pub(super) fn esc(k: &str) -> String {
    k.replace('~', "~0").replace('/', "~1")
}

pub(super) fn all_pointers(v: &Value, p: &str, out: &mut Vec<String>) {
    out.push(p.to_string());
    match v {
        Value::Object(m) => m
            .iter()
            .for_each(|(k, c)| all_pointers(c, &format!("{p}/{}", esc(k)), out)),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .for_each(|(i, c)| all_pointers(c, &format!("{p}/{i}"), out)),
        _ => {}
    }
}

/// Reference semantics of the ops on a plain serde value. Returns false on error.
pub(super) fn model_apply(root: &mut Value, op: &DraftOp) -> bool {
    fn split(path: &str) -> Option<(String, String)> {
        let i = path.rfind('/')?;
        Some((
            path[..i].to_string(),
            path[i + 1..].replace("~1", "/").replace("~0", "~"),
        ))
    }
    let val = |s: &str| serde_json::from_str::<Value>(s).ok();
    match op {
        DraftOp::Set { path, value_json } => {
            let Some(v) = val(value_json) else {
                return false;
            };
            match root.pointer_mut(path) {
                Some(slot) => {
                    *slot = v;
                    true
                }
                None => false,
            }
        }
        DraftOp::Add { path, value_json } => {
            let (Some(v), Some((parent, key))) = (val(value_json), split(path)) else {
                return false;
            };
            match root.pointer_mut(&parent) {
                Some(Value::Object(m)) if !m.contains_key(&key) => {
                    m.insert(key, v);
                    true
                }
                _ => false,
            }
        }
        DraftOp::Remove { path } => {
            let Some((parent, key)) = split(path) else {
                return false;
            };
            match root.pointer_mut(&parent) {
                Some(Value::Object(m)) => m.remove(&key).is_some(),
                Some(Value::Array(a)) => match key.parse::<usize>() {
                    Ok(i) if i < a.len() && key == i.to_string() => {
                        a.remove(i);
                        true
                    }
                    _ => false,
                },
                _ => false,
            }
        }
        DraftOp::ArrayInsert {
            path,
            index,
            value_json,
        } => {
            let Some(v) = val(value_json) else {
                return false;
            };
            match root.pointer_mut(path) {
                Some(Value::Array(a)) if (*index as usize) <= a.len() => {
                    a.insert(*index as usize, v);
                    true
                }
                _ => false,
            }
        }
        DraftOp::ArrayMove { path, from, to } => match root.pointer_mut(path) {
            Some(Value::Array(a)) if (*from as usize) < a.len() && (*to as usize) < a.len() => {
                let item = a.remove(*from as usize);
                a.insert(*to as usize, item);
                true
            }
            _ => false,
        },
        DraftOp::RenameKey { path, new_key } => {
            let Some((parent, key)) = split(path) else {
                return false;
            };
            match root.pointer_mut(&parent) {
                Some(Value::Object(m)) if m.contains_key(&key) => {
                    if key == *new_key {
                        return true;
                    }
                    if m.contains_key(new_key) {
                        return false;
                    }
                    let v = m.remove(&key).unwrap_or(Value::Null);
                    m.insert(new_key.clone(), v);
                    true
                }
                _ => false,
            }
        }
    }
}

pub(super) const MODEL_DOC: &str = r#"{
  "flag": true,
  "n": 1.50,
  "s": "x",
  "list": [1, 2, 3],
  "empty": [],
  "objs": [
    { "name": "a", "on": true },
    { "name": "b", "on": false }
  ],
  "map": { "k1": 1, "k2": "two" },
  "nested": { "deep": { "x": [true, null] } },
  "a/b": 0
}
"#;

const MODEL_DOC_JSONC: &str = r#"{
  // leading comment
  "flag": true, // trailing
  "n": 1.50,
  "list": [1, 2, /* mid */ 3,],
  "objs": [
    { "name": "a", "on": true, },
    { "name": "b", "on": false },
  ],
  "map": { "k1": 1, "k2": "two" },
}
"#;

pub(super) const VALUES: [&str; 8] = [
    "true",
    "null",
    "1e3",
    "\"s\"",
    "[1, 2]",
    "{\"z\": 1}",
    "123",
    "{\"a\": {\"b\": [1, {\"c\": 2}]}}",
];
pub(super) const KEYS: [&str; 5] = ["k1", "new", "a~b", "x/y", "name"];

/// Turn selector bytes into a (possibly invalid) op against the current model.
pub(super) fn gen_op(model: &Value, sel: (u8, u16, u16, u16)) -> DraftOp {
    let mut pointers = Vec::new();
    all_pointers(model, "", &mut pointers);
    let pick = |n: u16| pointers[n as usize % pointers.len()].clone();
    let value = VALUES[sel.3 as usize % VALUES.len()];
    let key = esc(KEYS[sel.2 as usize % KEYS.len()]);
    match sel.0 % 6 {
        0 => set(&pick(sel.1), value),
        1 => add(&format!("{}/{key}", pick(sel.1)), value),
        2 => remove(&pick(sel.1)),
        3 => insert(&pick(sel.1), u32::from(sel.2 % 6), value),
        4 => mv(&pick(sel.1), u32::from(sel.2 % 5), u32::from(sel.3 % 5)),
        _ => rename(&pick(sel.1), KEYS[sel.2 as usize % KEYS.len()]),
    }
}

pub(super) fn sel_strategy() -> impl Strategy<Value = Vec<(u8, u16, u16, u16)>> {
    prop::collection::vec(
        (any::<u8>(), any::<u16>(), any::<u16>(), any::<u16>()),
        1..25,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Every op behaves like the serde reference model, and the candidate
    /// always re-parses to exactly the model (strict .json and lenient .jsonc).
    #[test]
    fn ops_match_the_reference_model(sels in sel_strategy(), jsonc in any::<bool>()) {
        let (text, kind) = if jsonc { (MODEL_DOC_JSONC, DocKind::Jsonc) } else { (MODEL_DOC, DocKind::Json) };
        let mut d = load_text(text, kind);
        prop_assert!(d.is_editable());
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            let mut next = model.clone();
            let ok = model_apply(&mut next, &op);
            let result = d.apply(op.clone());
            prop_assert_eq!(result.is_ok(), ok, "{:?}", op);
            if ok { model = next; }
            prop_assert_eq!(d.to_value().unwrap(), model.clone(), "{:?}", op);
            // What would be saved parses back to the same data, with no diagnostics.
            let reloaded = DocumentSession::load("r", d.candidate_bytes(), kind);
            prop_assert!(reloaded.is_editable(), "{:?}: {}", op, text_of(&d));
            prop_assert_eq!(reloaded.to_value().unwrap(), model.clone());
            if kind == DocKind::Json {
                prop_assert_eq!(serde_json::from_str::<Value>(&text_of(&d)).unwrap(), model.clone());
            }
        }
    }

    /// Apply N, undo k, redo k equals apply N (eng R4); undo lands on the
    /// exact earlier bytes; a fresh session replaying the op list agrees.
    #[test]
    fn undo_redo_roundtrip(sels in sel_strategy(), k in 0usize..25) {
        let mut d = load_text(MODEL_DOC, DocKind::Json);
        let mut model = d.to_value().unwrap();
        let mut states = vec![(d.candidate_bytes(), model.clone())];
        for sel in sels {
            let op = gen_op(&model, sel);
            if d.apply(op.clone()).is_ok() {
                prop_assert!(model_apply(&mut model, &op));
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
        let mut fresh = load_text(MODEL_DOC, DocKind::Json);
        for op in d.ops().to_vec() { fresh.apply(op).unwrap(); }
        prop_assert_eq!(fresh.candidate_bytes(), d.candidate_bytes());
        for i in 1..=k {
            d.redo().unwrap();
            prop_assert_eq!(&d.candidate_bytes(), &states[n - k + i].0);
        }
        prop_assert_eq!(&d.candidate_bytes(), &states[n].0);
        prop_assert_eq!(d.snapshot(false).can_redo, false);
        prop_assert_eq!(d.is_dirty(), n > 0);
    }

    /// Reverting an op that nothing later depends on equals never having applied it.
    #[test]
    fn revert_equals_never_applied(sels in sel_strategy(), pick in any::<u16>()) {
        let mut d = load_text(MODEL_DOC, DocKind::Json);
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            if d.apply(op.clone()).is_ok() { model_apply(&mut model, &op); }
        }
        if d.ops().is_empty() { return Ok(()); }
        let i = pick as usize % d.ops().len();
        let ops = d.ops().to_vec();
        let revertible = d.review(&no_secrets).changes[i].revertible;
        match d.revert_op(i as u32) {
            Ok(_) => {
                prop_assert!(revertible);
                let mut fresh = load_text(MODEL_DOC, DocKind::Json);
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

#[test]
fn secret_fixtures_never_render_the_token_in_the_source_view() {
    for name in [
        "invalid-missing-comma-secret.json",
        "invalid-unterminated-secret.json",
    ] {
        let d = open(name);
        let view = d.snapshot(false).source_view.unwrap();
        for token in ["sk-test-0000000000000000", "ghx_fake_token_value"] {
            assert!(!view.text.contains(token), "{name}: {}", view.text);
        }
        let revealed: Vec<_> = view
            .masks
            .iter()
            .map(|m| d.reveal_masked(m.id).unwrap())
            .collect();
        assert!(
            revealed
                .iter()
                .any(|r| r.contains("sk-test-0000000000000000")),
            "{name}"
        );
        assert_eq!(
            view.text.lines().count(),
            fixture(name).iter().filter(|b| **b == b'\n').count()
        );
    }
}

#[test]
fn nested_duplicate_fixture_lists_both_keys() {
    let d = open("duplicate-nested.json");
    let diags = d.diagnostics();
    assert_eq!(diags.len(), 2);
    let msgs: Vec<_> = diags
        .iter()
        .map(|d| (d.pointer.clone().unwrap(), d.message.clone()))
        .collect();
    assert!(
        msgs.contains(&(
            "/env/A".to_string(),
            "Key `A` appears 2 times: lines 4 and 6".to_string()
        )),
        "{msgs:?}"
    );
    assert!(
        msgs.contains(&(
            "/model".to_string(),
            "Key `model` appears 2 times: lines 2 and 8".to_string()
        )),
        "{msgs:?}"
    );
    assert_eq!(
        d.snapshot(false).source_view.unwrap().highlight_lines,
        vec![2, 4, 6, 8]
    );
}

// ----------------------------------------------------- review and masking

#[test]
fn review_lists_semantic_changes_and_a_one_line_diff() {
    let mut d = open("feature-flags.json");
    assert!(d.review(&no_secrets).changes.is_empty() && d.review(&no_secrets).hunks.is_empty());
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(remove("/features/0")).unwrap();
    d.apply(add("/extra", "\"v\"")).unwrap();
    d.apply(rename("/debug", "verbose")).unwrap();
    d.apply(mv("/services", 0, 1)).unwrap();
    let review = d.review(&no_secrets);
    let got: Vec<_> = review
        .changes
        .iter()
        .map(|c| {
            (
                c.pointer.as_str(),
                c.kind,
                c.old_text.as_deref(),
                c.new_text.as_deref(),
                c.op_index,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "/enabled",
                ChangeKind::Changed,
                Some("true"),
                Some("false"),
                Some(0)
            ),
            (
                "/features/0",
                ChangeKind::Removed,
                Some("search"),
                None,
                Some(1)
            ),
            ("/extra", ChangeKind::Added, None, Some("v"), Some(2)),
            (
                "/verbose",
                ChangeKind::Renamed,
                Some("debug"),
                Some("verbose"),
                Some(3)
            ),
            (
                "/services/1",
                ChangeKind::Moved,
                Some("position 1"),
                Some("position 2"),
                Some(4)
            ),
        ]
    );
    assert!(review.changes.iter().all(|c| !c.masked));
    assert_eq!(
        d.changed_pointers(),
        [
            "/enabled",
            "/features/0",
            "/extra",
            "/verbose",
            "/services/1"
        ]
    );
    assert!(!review.hunks.is_empty());
    // Hunk text is exactly original-vs-candidate.
    let removed: Vec<_> = review
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == DiffLineKind::Removed)
        .collect();
    assert!(removed.iter().any(|l| l.text == "  \"enabled\": true,"));
}

#[test]
fn review_diff_for_a_boolean_toggle_is_a_single_line_change() {
    let mut d = open("feature-flags.json");
    d.apply(set("/logging/toFile", "true")).unwrap();
    let review = d.review(&no_secrets);
    assert_eq!(review.hunks.len(), 1);
    let lines = &review.hunks[0].lines;
    let changed: Vec<_> = lines
        .iter()
        .filter(|l| l.kind != DiffLineKind::Context)
        .collect();
    assert_eq!(changed.len(), 2);
    assert_eq!(
        (
            changed[0].kind,
            changed[0].text.as_str(),
            changed[0].old_line
        ),
        (DiffLineKind::Removed, "    \"toFile\": false", Some(11))
    );
    assert_eq!(
        (
            changed[1].kind,
            changed[1].text.as_str(),
            changed[1].new_line
        ),
        (DiffLineKind::Added, "    \"toFile\": true", Some(11))
    );
    assert_eq!(lines.len(), 2 + 6);
}

#[test]
fn a30_secret_values_are_masked_in_changes_and_diff() {
    let mut d = open("secret.json");
    d.apply(set("/env/API_TOKEN", "\"sk-new-1111222233334444\""))
        .unwrap();
    d.apply(set("/env/auth", "\"ghx_replacement_value\""))
        .unwrap();
    d.apply(add("/env/DB_PASSWORD", "\"hunter2hunter2\""))
        .unwrap();
    let review = d.review(&|p: &str| p == "/env/auth");
    for c in &review.changes {
        assert!(c.masked, "{c:?}");
        for t in [&c.old_text, &c.new_text].into_iter().flatten() {
            assert_eq!(t, "••••");
        }
    }
    let shown: String = review
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    for secret in [
        "sk-test-0000000000000000",
        "sk-new-1111222233334444",
        "ghx_fake_token_value",
        "ghx_replacement_value",
        "hunter2hunter2",
    ] {
        assert!(!shown.contains(secret), "{secret} leaked:\n{shown}");
    }
    assert!(
        shown.contains("\"API_TOKEN\"") && shown.contains("\"auth\""),
        "keys stay visible:\n{shown}"
    );
    // Every mask id reveals its original, and spans address `•` runs in UTF-16 units.
    let mut revealed = Vec::new();
    for line in review.hunks.iter().flat_map(|h| &h.lines) {
        let units: Vec<u16> = line.text.encode_utf16().collect();
        for m in &line.masks {
            assert!(
                units[m.start as usize..m.end as usize]
                    .iter()
                    .all(|u| *u == '•' as u16)
            );
            revealed.push(d.reveal_masked(m.id).unwrap());
        }
    }
    for want in [
        "sk-test-0000000000000000",
        "sk-new-1111222233334444",
        "ghx_fake_token_value",
        "ghx_replacement_value",
        "hunter2hunter2",
    ] {
        assert!(
            revealed.iter().any(|r| r == want),
            "{want} not revealable: {revealed:?}"
        );
    }
    assert_eq!(
        d.reveal_masked(9999).unwrap_err().code,
        ErrorCode::InvalidOperation
    );
    // Non-secret edits in the same file are not masked.
    let mut e = open("secret.json");
    e.apply(add("/env/PLAIN", "\"visible\"")).unwrap();
    let review = e.review(&no_secrets);
    assert!(!review.changes[0].masked);
    assert!(
        review
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.text.contains("visible") && l.masks.is_empty())
    );
}

#[test]
fn secret_inherited_by_children_and_non_string_scalars() {
    let mut d = load_text(
        "{\"creds\": {\"pin\": 481516, \"names\": [\"alice\", \"bob\"]}, \"n\": 481516}",
        DocKind::Json,
    );
    d.apply(set("/n", "2")).unwrap();
    d.apply(set("/creds/pin", "99")).unwrap();
    let review = d.review(&|p: &str| p == "/creds");
    let diff: String = review
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("|");
    assert!(
        !diff.contains("alice") && !diff.contains("bob") && !diff.contains("481516"),
        "{diff}"
    );
    // The unrelated `n` keeps working: its own numbers are not secret and "99" stays.
    assert!(diff.contains("\"n\": 2"));
}

#[test]
fn invalid_file_source_view_masks_every_value_and_reveals_on_demand() {
    let secret = String::from_utf8(fixture("secret.json")).unwrap();
    let variants = [
        secret.replace("\"auth\": ", "\"auth\" "),
        secret.replacen(
            "\"sk-test-0000000000000000\"",
            "\"sk-test-0000000000000000",
            1,
        ),
        secret
            .replace(
                "\"API_TOKEN\": \"sk-test-0000000000000000\",",
                "\"API_TOKEN\": \"sk-test-0000000000000000\"",
            )
            .replacen("auth\":", "auth\": ,", 1),
    ];
    for text in variants {
        let d = load_text(&text, DocKind::Json);
        assert!(!d.is_editable(), "{text}");
        let view = d.snapshot(false).source_view.unwrap();
        assert!(
            !view.text.contains("sk-test-0000000000000000"),
            "{}",
            view.text
        );
        assert!(!view.text.contains("ghx_fake_token_value"), "{}", view.text);
        assert!(!view.masks.is_empty());
        let originals: Vec<_> = view
            .masks
            .iter()
            .map(|m| d.reveal_masked(m.id).unwrap())
            .collect();
        assert!(
            originals
                .iter()
                .any(|o| o.contains("sk-test-0000000000000000"))
        );
        // Same line count so highlight lines stay valid.
        assert_eq!(view.text.lines().count(), text.lines().count());
    }
    // Duplicate-key files are masked too.
    let dup = load_text(
        "{\"token\": \"sk-dup-secret-1\", \"token\": \"sk-dup-secret-2\"}",
        DocKind::Json,
    );
    let view = dup.snapshot(false).source_view.unwrap();
    assert!(!view.text.contains("sk-dup-secret") && view.highlight_lines == vec![1]);
}

// --------------------------------------------------- conflict and reapply

fn flags() -> Vec<u8> {
    fixture("feature-flags.json")
}

fn disk_with(from: &str, to: &str) -> Vec<u8> {
    String::from_utf8(flags())
        .unwrap()
        .replacen(from, to, 1)
        .into_bytes()
}

#[test]
fn reapply_non_overlapping_keeps_disk_change_byte_identical() {
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(set("/logging/toFile", "true")).unwrap();
    let disk = disk_with("\"retryCount\": 3", "\"retryCount\": 30");
    let info = d.conflict(&disk, &no_secrets);
    assert_eq!(info.disk_fingerprint, fingerprint(&disk));
    assert!(info.can_reapply && info.overlapping.is_empty() && info.disk_parse_error.is_none());
    assert_eq!(info.disk_changes.len(), 1);
    let c = &info.disk_changes[0];
    assert_eq!(
        (
            c.pointer.as_str(),
            c.old_text.as_deref(),
            c.new_text.as_deref(),
            c.op_index
        ),
        ("/retryCount", Some("3"), Some("30"), None)
    );
    assert_eq!(info.draft_changes.len(), 2);
    assert_eq!(info.draft_changes[0].op_index, Some(0));

    let before_rev = d.revision();
    let r = d.reapply_onto(disk.clone()).unwrap();
    assert!(r.revision > before_rev && r.dirty_count == 2 && r.can_undo && !r.can_redo);
    assert_eq!(r.changed[0].pointer, "");
    let want = String::from_utf8(disk.clone())
        .unwrap()
        .replacen("\"enabled\": true", "\"enabled\": false", 1)
        .replacen("\"toFile\": false", "\"toFile\": true", 1);
    assert_eq!(text_of(&d), want);
    // Rebased: the disk version is the new original.
    assert_eq!(d.original_bytes(), &disk[..]);
    assert_eq!(d.base_fingerprint(), &fingerprint(&disk));
    assert_eq!(d.snapshot(false).fingerprint, fingerprint(&disk));
    assert_eq!(d.ops().len(), 2);
    // Undo now returns to the disk version, not the old original.
    d.undo().unwrap();
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), disk);
}

#[test]
fn section_replace_vs_child_change_is_overlap() {
    let mut d = open("feature-flags.json");
    d.apply(set("/logging", "{\"level\": \"debug\"}")).unwrap();
    let disk = disk_with("\"level\": \"info\"", "\"level\": \"error\"");
    let info = d.conflict(&disk, &no_secrets);
    assert!(!info.can_reapply);
    assert_eq!(info.overlapping, ["/logging"]);
    assert_eq!(info.disk_changes[0].pointer, "/logging/level");
    let before = d.candidate_bytes();
    assert_eq!(
        err_code(d.reapply_onto(disk)),
        ErrorCode::ExternalChangeConflict
    );
    assert_eq!(d.candidate_bytes(), before);
    assert_eq!(d.base_fingerprint(), &fingerprint(&flags()));
}

#[test]
fn overlap_cases_remove_rename_array_and_same_key() {
    let cases: Vec<(DraftOp, Vec<u8>, bool)> = vec![
        // remove / rename of a key whose subtree changed on disk
        (remove("/logging"), disk_with("\"info\"", "\"warn\""), true),
        (
            rename("/logging", "log"),
            disk_with("\"info\"", "\"warn\""),
            true,
        ),
        // child edit vs ancestor replaced on disk
        (
            set("/logging/level", "\"x\""),
            disk_with("\"logging\": {", "\"logging\": 5, \"zz\": {"),
            true,
        ),
        // same key changed on both sides
        (
            set("/retryCount", "9"),
            disk_with("\"retryCount\": 3", "\"retryCount\": 4"),
            true,
        ),
        // arrays: any change marks the array itself
        (
            insert("/features", 0, "\"x\""),
            disk_with("\"analytics\"", "\"metrics\""),
            true,
        ),
        (
            set("/features/0", "\"x\""),
            disk_with("\"analytics\"", "\"metrics\""),
            true,
        ),
        (
            mv("/services", 0, 1),
            disk_with("\"frontend\"", "\"web\""),
            true,
        ),
        // both add the same key
        (
            add("/newKey", "1"),
            disk_with("\"enabled\": true,", "\"enabled\": true,\n  \"newKey\": 2,"),
            true,
        ),
        // siblings do not overlap, including `/a` vs `/ab` style prefixes
        (
            set("/enabled", "false"),
            disk_with("\"debug\": false", "\"debug\": true"),
            false,
        ),
        (
            set("/logging/toFile", "true"),
            disk_with("\"info\"", "\"warn\""),
            false,
        ),
        (
            set("/features/0", "\"x\""),
            disk_with("\"retryCount\": 3", "\"retryCount\": 4"),
            false,
        ),
        (
            rename("/debug", "dbg"),
            disk_with("\"enabled\": true", "\"enabled\": false"),
            false,
        ),
    ];
    for (op, disk, overlap) in cases {
        let mut d = open("feature-flags.json");
        d.apply(op.clone()).unwrap();
        let info = d.conflict(&disk, &no_secrets);
        assert_eq!(!info.overlapping.is_empty(), overlap, "{op:?}: {info:?}");
        assert_eq!(info.can_reapply, !overlap, "{op:?}");
        assert_eq!(d.reapply_onto(disk).is_ok(), !overlap, "{op:?}");
    }
}

#[test]
fn unparseable_disk_disables_reapply() {
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    let disk = b"{\"enabled\": true,\n \"x\": }".to_vec();
    let info = d.conflict(&disk, &no_secrets);
    assert!(!info.can_reapply && info.disk_changes.is_empty() && info.overlapping.is_empty());
    let diag = info.disk_parse_error.unwrap();
    assert_eq!((diag.code.as_str(), diag.line), ("INVALID_JSON", Some(2)));
    assert_eq!(info.draft_changes.len(), 1);
    assert_eq!(err_code(d.reapply_onto(disk)), ErrorCode::InvalidJson);
    assert!(d.is_dirty() && d.base_fingerprint() == &fingerprint(&flags()));
    // Duplicate keys and empty disk files also block.
    for bad in [&b"{\"a\":1,\"a\":2}"[..], b"", b"\xff\xfe{}"] {
        assert!(!d.conflict(bad, &no_secrets).can_reapply);
        assert!(d.reapply_onto(bad.to_vec()).is_err());
    }
}

#[test]
fn reapply_follows_disk_line_endings_and_bom() {
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    let mut disk = vec![0xEF, 0xBB, 0xBF];
    disk.extend(
        String::from_utf8(flags())
            .unwrap()
            .replace('\n', "\r\n")
            .replacen("\"retryCount\": 3", "\"retryCount\": 4", 1)
            .bytes(),
    );
    d.reapply_onto(disk.clone()).unwrap();
    let snap = d.snapshot(false);
    assert!(snap.has_bom);
    assert_eq!(snap.line_ending, LineEnding::Crlf);
    let out = d.candidate_bytes();
    assert_eq!(&out[..3], &[0xEF, 0xBB, 0xBF]);
    let mut fresh = DocumentSession::load("x", disk, DocKind::Json);
    fresh.apply(set("/enabled", "false")).unwrap();
    assert_eq!(out, fresh.candidate_bytes());
}

#[test]
fn conflict_masks_secret_disk_and_draft_values() {
    let mut d = open("secret.json");
    d.apply(set("/env/API_TOKEN", "\"sk-mine\"")).unwrap();
    let disk = String::from_utf8(fixture("secret.json"))
        .unwrap()
        .replace("ghx_fake_token_value", "ghx_disk_value")
        .into_bytes();
    let info = d.conflict(&disk, &|p: &str| p == "/env/auth");
    assert!(info.can_reapply);
    let blob = format!("{:?}", (&info.disk_changes, &info.draft_changes));
    for leak in [
        "sk-mine",
        "sk-test-0000000000000000",
        "ghx_disk_value",
        "ghx_fake_token_value",
    ] {
        assert!(!blob.contains(leak), "{leak} in {blob}");
    }
    assert!(info.disk_changes[0].masked && info.draft_changes[0].masked);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// C3: reapply output equals applying the ops to the disk bytes, and every
    /// disk-changed path stays byte-identical.
    #[test]
    fn reapply_equals_ops_on_disk(
        mine in prop::collection::vec((0u8..5, 0u8..4), 1..4),
        theirs in prop::collection::vec((0u8..5, 0u8..4), 1..4),
    ) {
        let base = "{\n  \"a0\": 1,\n  \"a1\": true,\n  \"a2\": \"s\",\n  \"a3\": [1, 2],\n  \"a4\": {\"x\": 1},\n  \"b0\": 1,\n  \"b1\": true,\n  \"b2\": \"s\",\n  \"b3\": [1, 2],\n  \"b4\": {\"x\": 1}\n}\n";
        let mut d = load_text(base, DocKind::Json);
        let mut applied = Vec::new();
        for (key, action) in mine {
            let path = format!("/a{key}");
            let op = match action { 0 => set(&path, "99"), 1 => set(&path, "\"changed\""), 2 => remove(&path), _ => add("/a_new", "true") };
            if d.apply(op.clone()).is_ok() { applied.push(op); }
        }
        // Disk side edits only b* keys, via a second session so bytes are valid.
        let mut disk_session = load_text(base, DocKind::Json);
        for (key, action) in theirs {
            let path = format!("/b{key}");
            let op = match action { 0 => set(&path, "77"), 1 => set(&path, "{\"deep\": [1]}"), 2 => remove(&path), _ => add("/b_new", "1") };
            let _ = disk_session.apply(op);
        }
        let disk = disk_session.candidate_bytes();
        let info = d.conflict(&disk, &no_secrets);
        // a* vs b* never share a path, so reapply is always allowed.
        prop_assert!(info.can_reapply, "{:?}", info.overlapping);
        d.reapply_onto(disk.clone()).unwrap();
        let mut fresh = DocumentSession::load("f", disk.clone(), DocKind::Json);
        for op in &applied { fresh.apply(op.clone()).unwrap(); }
        prop_assert_eq!(d.candidate_bytes(), fresh.candidate_bytes());
        // Disk-side b* members are untouched in the result.
        let (out, disk_v, out_v) = (text_of(&d), disk_session.to_value().unwrap(), d.to_value().unwrap());
        for (k, v) in disk_v.as_object().unwrap().iter().filter(|(k, _)| k.starts_with('b')) {
            prop_assert_eq!(out_v.get(k), Some(v));
        }
        let _ = out;
    }
}

#[test]
fn adopt_saved_rebases_to_the_saved_bytes() {
    let mut d = open("feature-flags.json");
    d.apply(set("/enabled", "false")).unwrap();
    d.apply(set("/debug", "true")).unwrap();
    d.undo().unwrap();
    let saved = d.candidate_bytes();
    let rev = d.revision();
    d.adopt_saved(saved.clone());
    assert!(!d.is_dirty() && d.revision() > rev);
    assert_eq!(d.original_bytes(), &saved[..]);
    assert_eq!(d.base_fingerprint(), &fingerprint(&saved));
    let snap = d.snapshot(false);
    assert_eq!(
        (snap.dirty_count, snap.can_undo, snap.can_redo),
        (0, false, false)
    );
    assert!(!snap.root.unwrap().changed);
    assert_eq!(err_code(d.undo()), ErrorCode::InvalidOperation);
    // Editing continues from the saved state.
    d.apply(set("/debug", "true")).unwrap();
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), saved);
}

// ----------------------------------------------------------- performance

#[test]
fn toggle_on_a_large_document_is_fast() {
    // About 4 MiB: 50k objects.
    let mut text = String::from("{\"flag\": true, \"items\": [\n");
    for i in 0..50_000 {
        text.push_str(&format!("  {{\"id\": {i}, \"name\": \"item-{i}\", \"enabled\": true, \"tags\": [\"a\", \"b\"]}}{}\n", if i == 49_999 { "" } else { "," }));
    }
    text.push_str("]}\n");
    assert!(text.len() > 3_500_000, "{}", text.len());
    let mut d = load_text(&text, DocKind::Json);
    assert!(d.is_editable());
    let start = std::time::Instant::now();
    d.apply(set("/flag", "false")).unwrap();
    let toggle_top = start.elapsed();
    let start = std::time::Instant::now();
    d.apply(set("/items/49999/enabled", "false")).unwrap();
    let toggle_deep = start.elapsed();
    let start = std::time::Instant::now();
    d.undo().unwrap();
    let undo = start.elapsed();
    println!("4 MiB: toggle {toggle_top:?}, deep toggle {toggle_deep:?}, undo(replay) {undo:?}");
    let budget = if cfg!(debug_assertions) { 3000 } else { 100 };
    assert!(
        toggle_top.as_millis() < budget && toggle_deep.as_millis() < budget,
        "{toggle_top:?} {toggle_deep:?}"
    );
}

#[test]
fn multiline_edits_keep_formatting_clean() {
    let mut d = load_text("{}\n", DocKind::Json);
    d.apply(add("/a", "1")).unwrap();
    d.apply(add("/b", "[1,2]")).unwrap();
    assert_eq!(text_of(&d), "{\n  \"a\": 1,\n  \"b\": [1, 2]\n}\n");

    let mut d = load_text("{\n  \"l\": [\n    1,\n    2\n  ]\n}\n", DocKind::Json);
    d.apply(insert("/l", 2, "3")).unwrap();
    d.apply(insert("/l", 0, "0")).unwrap();
    assert_eq!(
        text_of(&d),
        "{\n  \"l\": [\n    0,\n    1,\n    2,\n    3\n  ]\n}\n"
    );
    d.apply(remove("/l/1")).unwrap();
    assert_eq!(
        text_of(&d),
        "{\n  \"l\": [\n    0,\n    2,\n    3\n  ]\n}\n"
    );

    // JSONC: new items follow the file's trailing-comma style; comments stay with their items.
    let mut d = load_text(
        "{\n  \"l\": [\n    \"a\", // ca\n    \"b\", // cb\n  ],\n}\n",
        DocKind::Jsonc,
    );
    d.apply(insert("/l", 2, "\"c\"")).unwrap();
    d.apply(remove("/l/0")).unwrap();
    assert_eq!(
        text_of(&d),
        "{\n  \"l\": [\n    \"b\", // cb\n    \"c\",\n  ],\n}\n"
    );
}

#[test]
fn reapply_refuses_when_disk_inserted_an_earlier_array_item() {
    // The draft edits objs[0]; on disk a new item was inserted at the front, so
    // objs[0] is now a different item. Reapplying by index would edit it.
    let mut d = load_text(MODEL_DOC, DocKind::Json);
    d.apply(set("/objs/0/on", "false")).unwrap();
    let disk = MODEL_DOC.replace(
        "  \"objs\": [\n",
        "  \"objs\": [\n    { \"name\": \"z\", \"on\": true },\n",
    );
    assert!(!d.conflict(disk.as_bytes(), &no_secrets).can_reapply);
}

/// In-memory `Edit` over a neutral value: exercises the shared op layer
/// (`syntax::apply_edit`) that the YAML and TOML syntaxes build on.
struct ValDoc(value::Val);

impl ValDoc {
    fn at(&mut self, path: &[String]) -> &mut value::Val {
        let mut cur = &mut self.0;
        for seg in path {
            cur = match cur {
                value::Val::Object(m) => &mut m.iter_mut().find(|(k, _)| k == seg).unwrap().1,
                value::Val::Array(a) => &mut a[seg.parse::<usize>().unwrap()],
                _ => unreachable!(),
            };
        }
        cur
    }
}

impl syntax::Edit for ValDoc {
    fn value(&self) -> value::Val {
        self.0.clone()
    }
    fn set(&mut self, path: &[String], v: &value::Val) -> CoreResult<()> {
        *self.at(path) = v.clone();
        Ok(())
    }
    fn add(&mut self, parent: &[String], key: &str, v: &value::Val) -> CoreResult<()> {
        let value::Val::Object(m) = self.at(parent) else {
            unreachable!()
        };
        m.push((key.to_string(), v.clone()));
        Ok(())
    }
    fn remove(&mut self, path: &[String]) -> CoreResult<()> {
        let (last, parent) = path.split_last().unwrap();
        match self.at(parent) {
            value::Val::Object(m) => m.retain(|(k, _)| k != last),
            value::Val::Array(a) => {
                a.remove(last.parse::<usize>().unwrap());
            }
            _ => unreachable!(),
        }
        Ok(())
    }
    fn insert(&mut self, array: &[String], index: usize, v: &value::Val) -> CoreResult<()> {
        let value::Val::Array(a) = self.at(array) else {
            unreachable!()
        };
        a.insert(index, v.clone());
        Ok(())
    }
    fn move_item(&mut self, array: &[String], from: usize, to: usize) -> CoreResult<()> {
        let value::Val::Array(a) = self.at(array) else {
            unreachable!()
        };
        let item = a.remove(from);
        a.insert(to, item);
        Ok(())
    }
    fn rename(&mut self, path: &[String], new_key: &str) -> CoreResult<()> {
        let (last, parent) = path.split_last().unwrap();
        let value::Val::Object(m) = self.at(parent) else {
            unreachable!()
        };
        m.iter_mut().find(|(k, _)| k == last).unwrap().0 = new_key.to_string();
        Ok(())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// The shared op layer accepts and rejects exactly what the JSON engine
    /// does, with the same effects and error codes, and edits the same data.
    #[test]
    fn shared_op_layer_matches_the_json_engine(sels in sel_strategy()) {
        let mut json = load_text(MODEL_DOC, DocKind::Json);
        let mut generic = ValDoc(json.root.as_ref().unwrap().value());
        for sel in sels {
            let op = gen_op(&json.to_value().unwrap(), sel);
            let want = json.root.as_mut().unwrap().apply(&op);
            let got = syntax::apply_edit(&mut generic, &op);
            match (&want, &got) {
                (Ok(w), Ok(g)) => prop_assert_eq!(w, g, "{:?}", op),
                (Err(w), Err(g)) => prop_assert_eq!(w.code, g.code, "{:?}: {} vs {}", op, w.message, g.message),
                _ => prop_assert!(false, "{:?}: json {:?} vs shared {:?}", op, want, got),
            }
            prop_assert_eq!(generic.0.to_json(), json.to_value().unwrap(), "{:?}", op);
        }
    }
}
