//! YAML syntax tests.

use proptest::prelude::*;
use serde_json::json;

use super::super::DocumentSession;
use super::super::syntax::{Edit, Syntax};
use super::super::tests::{
    add, gen_op, insert, load_text, model_apply, mv, no_secrets, remove, rename, sel_strategy, set,
    text_of,
};
use super::super::value::Val;
use super::mask::masked_view;
use super::parse;
use crate::core::ErrorCode;
use crate::core::persistence::fingerprint;
use crate::core::security::redact::default_is_secret;
use crate::core::types::{DiagnosticSource, DocKind, DraftOp, LineEnding};

const DOC: &str = "# header comment
name: \"api\"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
";

const ITEMS: &str = "endpoints:
  - name: health   # first
    public: true
  # the second one
  - name: orders
    public: false
  - plain
";

const EMPTIES: &str = "a: []
b: {}
c:   # nothing yet
d: x
";

#[test]
fn set_keeps_double_quotes_and_comment() {
    check(
        DOC,
        set("/name", "\"web\""),
        r##"# header comment
name: "web"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_quotes_a_string_that_needs_it() {
    check(
        DOC,
        set("/name", "\"my: val\""),
        r##"# header comment
name: "my: val"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_number_keeps_the_rest_of_the_line() {
    check(
        DOC,
        set("/port", "9090"),
        r##"# header comment
name: "api"   # service name
port: 9090
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_string_over_number_is_plain() {
    check(
        DOC,
        set("/port", "\"x\""),
        r##"# header comment
name: "api"   # service name
port: x
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_string_that_reads_as_a_number_is_quoted() {
    check(
        DOC,
        set("/port", "\"123\""),
        r##"# header comment
name: "api"   # service name
port: "123"
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_keeps_single_quotes() {
    check(
        DOC,
        set("/tags/1", "\"it's\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'it''s'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_bool() {
    check(
        DOC,
        set("/server/tls", "true"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: true

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_null() {
    check(
        DOC,
        set("/server/tls", "null"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: null

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_block_scalar_stays_literal() {
    check(
        DOC,
        set("/note", "\"a\\nb\\nc\\n\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  a
  b
  c
"##,
    );
}

#[test]
fn set_flow_item() {
    check(
        DOC,
        set("/flags/0", "\"z\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [z, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_flow_member() {
    check(
        DOC,
        set("/limits/mem", "8"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 8}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_block_collection_to_scalar_drops_it_whole() {
    check(
        DOC,
        set("/tags", "5"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags: 5
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_scalar_to_block_map_keeps_key_comment() {
    check(
        DOC,
        set("/name", "{\"k\":1,\"l\":[true]}"),
        r##"# header comment
name:   # service name
  k: 1
  l:
    - true
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn set_block_collection_to_empty_flow() {
    check(
        DOC,
        set("/server", "{}"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server: {}

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn add_appends_to_a_nested_mapping() {
    check(
        DOC,
        add("/server/user", "\"bob\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false
  user: bob

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn add_appends_at_the_end_of_the_document() {
    check(
        DOC,
        add("/extra", "1"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
extra: 1
"##,
    );
}

#[test]
fn add_nested_value_matches_indentation() {
    check(
        DOC,
        add("/server/limits", "{\"a\":1,\"b\":[1,2]}"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false
  limits:
    a: 1
    b:
      - 1
      - 2

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn add_into_flow_mapping() {
    check(
        DOC,
        add("/limits/disk", "10"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4, disk: 10}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn remove_entry_takes_its_line_and_comment() {
    check(
        DOC,
        remove("/server/host"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn remove_block_scalar_entry() {
    check(
        DOC,
        remove("/note"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
"##,
    );
}

#[test]
fn remove_block_sequence_item() {
    check(
        DOC,
        remove("/tags/1"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn remove_flow_item() {
    check(
        DOC,
        remove("/flags/0"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn remove_last_flow_item() {
    check(
        DOC,
        remove("/flags/1"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn remove_flow_member() {
    check(
        DOC,
        remove("/limits/cpu"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn insert_between_items() {
    check(
        DOC,
        insert("/tags", 1, "\"mid\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - mid
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn insert_appends() {
    check(
        DOC,
        insert("/tags", 2, "\"end\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
  - end
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn insert_first() {
    check(
        DOC,
        insert("/tags", 0, "\"first\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - first
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn insert_into_flow_sequence() {
    check(
        DOC,
        insert("/flags", 1, "\"x\""),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, x, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn insert_mapping_item() {
    check(
        DOC,
        insert("/tags", 1, "{\"k\":1,\"l\":2}"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - k: 1
    l: 2
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn move_item_carries_its_comment() {
    check(
        DOC,
        mv("/tags", 0, 1),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - 'edge'   # keep
  - web
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn move_flow_item() {
    check(
        DOC,
        mv("/flags", 1, 0),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [b, a]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn rename_key_keeps_value_and_comment() {
    check(
        DOC,
        rename("/server/host", "hostname"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  hostname: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn rename_flow_key() {
    check(
        DOC,
        rename("/limits/cpu", "cores"),
        r##"# header comment
name: "api"   # service name
port: 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cores: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn rename_to_a_key_that_needs_quotes() {
    check(
        DOC,
        rename("/port", "yes"),
        r##"# header comment
name: "api"   # service name
"yes": 8080
tags:
  - web
  - 'edge'   # keep
server:
  host: example.test   # primary
  tls: false

# trailing section
flags: [a, b]
limits: {cpu: 2, mem: 4}   # flow map
note: |
  hello
  world
"##,
    );
}

#[test]
fn items_remove_first_key_of_a_compact_item() {
    check(
        ITEMS,
        remove("/endpoints/0/name"),
        r##"endpoints:
  - public: true
  # the second one
  - name: orders
    public: false
  - plain
"##,
    );
}

#[test]
fn items_remove_whole_first_item() {
    check(
        ITEMS,
        remove("/endpoints/0"),
        r##"endpoints:
  # the second one
  - name: orders
    public: false
  - plain
"##,
    );
}

#[test]
fn items_remove_last_item() {
    check(
        ITEMS,
        remove("/endpoints/2"),
        r##"endpoints:
  - name: health   # first
    public: true
  # the second one
  - name: orders
    public: false
"##,
    );
}

#[test]
fn items_insert_mapping_before_an_item_with_a_comment() {
    check(
        ITEMS,
        insert("/endpoints", 1, "{\"name\":\"new\",\"public\":true}"),
        r##"endpoints:
  - name: health   # first
    public: true
  - name: new
    public: true
  # the second one
  - name: orders
    public: false
  - plain
"##,
    );
}

#[test]
fn items_move_carries_comment_lines_above() {
    check(
        ITEMS,
        mv("/endpoints", 1, 0),
        r##"endpoints:
  # the second one
  - name: orders
    public: false
  - name: health   # first
    public: true
  - plain
"##,
    );
}

#[test]
fn items_move_to_the_end() {
    check(
        ITEMS,
        mv("/endpoints", 0, 2),
        r##"endpoints:
  # the second one
  - name: orders
    public: false
  - plain
  - name: health   # first
    public: true
"##,
    );
}

#[test]
fn items_add_to_item_mapping() {
    check(
        ITEMS,
        add("/endpoints/1/tls", "true"),
        r##"endpoints:
  - name: health   # first
    public: true
  # the second one
  - name: orders
    public: false
    tls: true
  - plain
"##,
    );
}

#[test]
fn items_replace_item_with_mapping() {
    check(
        ITEMS,
        set("/endpoints/2", "{\"a\":1,\"b\":2}"),
        r##"endpoints:
  - name: health   # first
    public: true
  # the second one
  - name: orders
    public: false
  - a: 1
    b: 2
"##,
    );
}

#[test]
fn empties_insert_into_empty_sequence() {
    check(
        EMPTIES,
        insert("/a", 0, "1"),
        r##"a: [1]
b: {}
c:   # nothing yet
d: x
"##,
    );
}

#[test]
fn empties_add_to_empty_mapping() {
    check(
        EMPTIES,
        add("/b/k", "\"v\""),
        r##"a: []
b: {k: v}
c:   # nothing yet
d: x
"##,
    );
}

#[test]
fn empties_set_empty_value_keeps_comment() {
    check(
        EMPTIES,
        set("/c", "5"),
        r##"a: []
b: {}
c: 5   # nothing yet
d: x
"##,
    );
}

#[test]
fn empties_set_empty_value_to_a_block() {
    check(
        EMPTIES,
        set("/c", "[1,2]"),
        r##"a: []
b: {}
c:   # nothing yet
  - 1
  - 2
d: x
"##,
    );
}

// ------------------------------------------------------------------ helpers

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../fixtures/yaml/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn open(name: &str) -> DocumentSession {
    DocumentSession::load(name, fixture(name), DocKind::Yaml)
}

fn text_fixture(name: &str) -> String {
    String::from_utf8(fixture(name)).unwrap()
}

/// Apply `op` to `doc` and require exactly `expected`, which must itself read
/// back to the same data.
fn check(doc: &str, op: DraftOp, expected: &str) {
    let mut d = load_text(doc, DocKind::Yaml);
    d.apply(op.clone()).unwrap();
    assert_eq!(text_of(&d), expected, "{op:?}");
    assert_eq!(load_text(expected, DocKind::Yaml).to_value(), d.to_value());
}

fn val(text: &str) -> Val {
    match parse(text) {
        Ok(d) => Syntax::value(&d),
        Err(b) => panic!("not editable: {:?}", b.diagnostics),
    }
}

fn s(x: &str) -> Val {
    Val::String(x.into())
}

fn n(x: &str) -> Val {
    Val::Number(x.into())
}

fn obj(members: Vec<(&str, Val)>) -> Val {
    Val::Object(members.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

// ---------------------------------------------------------------- round trip

#[test]
fn every_editable_fixture_round_trips_byte_for_byte() {
    for name in [
        "app-config.yaml",
        "anchors.yaml",
        "crlf-no-eol.yaml",
        "bom.yaml",
        "empty.yaml",
    ] {
        let bytes = fixture(name);
        let d = DocumentSession::load(name, bytes.clone(), DocKind::Yaml);
        assert!(d.is_editable(), "{name}: {:?}", d.diagnostics());
        assert_eq!(d.candidate_bytes(), bytes, "{name}");
        assert!(d.diagnostics().is_empty(), "{name}");
    }
    let bom = open("bom.yaml");
    assert!(bom.snapshot(false).has_bom && bom.candidate_bytes().starts_with(&[0xEF, 0xBB, 0xBF]));
    let crlf = open("crlf-no-eol.yaml");
    assert_eq!(crlf.snapshot(false).line_ending, LineEnding::Crlf);
    assert!(!crlf.candidate_bytes().ends_with(b"\n"));
}

#[test]
fn awkward_documents_round_trip_byte_for_byte() {
    for text in [
        "",
        "\n\n",
        "# only a comment",
        "a: 1",
        "a: 1   \n\n\n",
        "---\n# c\na: 1\n...\n",
        "a: [1,\n  2]\nb: {x: 1}\n",
        "- - a\n  - b\n- c\n",
        "k:\n- a\n- b\nx: 1\n",
        "a:\n  b:\n    c: [1, {d: e}]   # tail\n",
        "? complex\n: val\n",
        "a: !!str 5\nb: &x [1]\nc: *x\n",
        "a: plain\n  continued\nb: \"q\n  w\"\n",
        "a: |+\n  k\n\nb: 1\n",
        "a: >-\n  folded\n   more\nb: 'it''s'\n",
        "  \n# lead\n\nkey:   value   # c\n\n# end\n",
        "key: \"emoji \u{1F600} and \\u00e9 escape\"\n",
        "a:\r\n  - 1\r\n  - 2\r\n",
        "-\n- x\n-   y   # c\n",
        "list: [a,b ,  c]\nmap: {a: 1,b: 2}\n",
        "x: 1 # no space comment\ny: a#b\n",
    ] {
        let d = load_text(text, DocKind::Yaml);
        assert!(d.is_editable(), "{text:?}: {:?}", d.diagnostics());
        assert_eq!(text_of(&d), text, "{text:?}");
    }
}

#[test]
fn plain_scalars_follow_the_core_schema() {
    let v = val(
        "a: true\nb: False\nc: null\nd: ~\ne:\nf: 12\ng: 0x1F\nh: 0o17\ni: -1.5e3\nj: .inf\nk: .NaN\nl: yes\nm: '12'\nn: \"true\"\no: 1_000\np: 0b1\nq: 1e\nr: ''\nt: 'null'\n",
    );
    assert_eq!(
        v,
        obj(vec![
            ("a", Val::Bool(true)),
            ("b", Val::Bool(false)),
            ("c", Val::Null),
            ("d", Val::Null),
            ("e", Val::Null),
            ("f", n("12")),
            ("g", n("0x1F")),
            ("h", n("0o17")),
            ("i", n("-1.5e3")),
            ("j", n(".inf")),
            ("k", n(".NaN")),
            ("l", s("yes")),
            ("m", s("12")),
            ("n", s("true")),
            ("o", s("1_000")),
            ("p", s("0b1")),
            ("q", s("1e")),
            ("r", s("")),
            ("t", s("null")),
        ])
    );
}

#[test]
fn block_and_multi_line_scalars_decode() {
    let v = val(
        "lit: |\n  a\n  b\nfold: >\n  x\n  y\n\n  z\nstrip: |-\n  k\nplain: one\n  two\n\n  three\nq: \"a\n  b\"\nlast: 1\n",
    );
    assert_eq!(
        v,
        obj(vec![
            ("lit", s("a\nb\n")),
            ("fold", s("x y\nz\n")),
            ("strip", s("k")),
            ("plain", s("one two\nthree")),
            ("q", s("a b")),
            ("last", n("1")),
        ])
    );
}

#[test]
fn empty_and_comment_only_files_are_an_empty_mapping() {
    for text in ["", "\n", "# nothing\n", "---\n"] {
        let d = load_text(text, DocKind::Yaml);
        assert!(d.is_editable(), "{text:?}");
        assert_eq!(d.to_value(), Some(json!({})), "{text:?}");
    }
    let mut d = open("empty.yaml");
    d.apply(add("/name", "\"x\"")).unwrap();
    d.apply(add("/list", "[1,2]")).unwrap();
    assert_eq!(
        text_of(&d),
        "# Nothing here yet, only a comment.\nname: x\nlist:\n  - 1\n  - 2\n"
    );
    let mut d = load_text("# no newline", DocKind::Yaml);
    d.apply(add("/a", "1")).unwrap();
    assert_eq!(text_of(&d), "# no newline\na: 1\n");
    let mut d = load_text("", DocKind::Yaml);
    d.apply(set("", "[true]")).unwrap();
    assert_eq!(text_of(&d), "- true\n");
}

// --------------------------------------------------------------- fidelity++

#[test]
fn crlf_and_a_missing_final_newline_survive_edits() {
    let mut d = open("crlf-no-eol.yaml");
    d.apply(add("/added", "\"v\"")).unwrap();
    d.apply(insert("/items", 1, "\"mid\"")).unwrap();
    d.apply(remove("/nested/key")).unwrap();
    d.apply(set("/last", "\"done\"")).unwrap();
    d.apply(add("/obj", "{\"a\":1,\"b\":[1]}")).unwrap();
    d.apply(remove("/flow/0")).unwrap();
    d.apply(set("/block", "\"x\\ny\\n\"")).unwrap();
    let want = "# CRLF file without a final newline\r\nname: demo   # trailing\r\nitems:\r\n  - one\r\n  - mid\r\n  - 'two'\r\nnested:\r\n  secret_key: sk-test-8888888888888888\r\nflow: [2]\r\nblock: |\r\n  x\r\n  y\r\nlast: done\r\nadded: v\r\nobj:\r\n  a: 1\r\n  b:\r\n    - 1";
    assert_eq!(text_of(&d), want);
    // Undoing everything lands on the original bytes.
    while d.undo().is_ok() {}
    assert_eq!(d.candidate_bytes(), fixture("crlf-no-eol.yaml"));
}

#[test]
fn the_bom_stays_and_only_the_touched_value_changes() {
    let mut d = open("bom.yaml");
    d.apply(set("/name", "\"changed\"")).unwrap();
    let mut want = vec![0xEF, 0xBB, 0xBF];
    want.extend_from_slice(b"# UTF-8 BOM file\nname: changed\nlist:\n  - a\n  - b\n");
    assert_eq!(d.candidate_bytes(), want);
}

#[test]
fn removing_the_last_child_leaves_an_empty_flow_collection() {
    let mut d = load_text("a:\n  b: 1   # c\nl:\n  - x\nroot: 1\n", DocKind::Yaml);
    d.apply(remove("/a/b")).unwrap();
    d.apply(remove("/l/0")).unwrap();
    assert_eq!(text_of(&d), "a: {}\nl: []\nroot: 1\n");
    let mut d = load_text("- a: 1\n- b: 2\n", DocKind::Yaml);
    d.apply(remove("/0/a")).unwrap();
    assert_eq!(text_of(&d), "- {}\n- b: 2\n");
    let mut d = load_text("only: 1\n", DocKind::Yaml);
    d.apply(remove("/only")).unwrap();
    assert_eq!(text_of(&d), "{}\n");
    let mut d = load_text("a: [x]\n", DocKind::Yaml);
    d.apply(remove("/a/0")).unwrap();
    assert_eq!(text_of(&d), "a: []\n");
}

#[test]
fn removing_the_last_line_without_a_final_newline() {
    let mut d = load_text("a: 1\nb: 2", DocKind::Yaml);
    d.apply(remove("/b")).unwrap();
    assert_eq!(text_of(&d), "a: 1");
}

#[test]
fn tab_indentation_is_a_syntax_error_not_a_crash() {
    let d = load_text("a:\n\tb: 1\n", DocKind::Yaml);
    assert!(!d.is_editable());
    assert_eq!(d.diagnostics()[0].code, "INVALID_YAML");
}

/// Strings that must survive a write and a re-read, in a block mapping, a
/// block sequence, a flow sequence and as a key.
#[test]
fn awkward_strings_round_trip_in_every_position() {
    let strings = [
        "",
        " ",
        " lead",
        "trail ",
        "a: b",
        "a:b",
        "#x",
        "x #y",
        "- x",
        "-x",
        "-",
        "?",
        "? x",
        ": x",
        "yes",
        "No",
        "on",
        "null",
        "~",
        "True",
        "1e3",
        "0x1F",
        "12",
        "-1.5",
        ".inf",
        "2024-01-02",
        "é ü \u{1F600}",
        "line\nbreak",
        "tab\there",
        "\u{7f}",
        "\u{85}",
        "[x]",
        "{x}",
        "a, b",
        "a'b\"c",
        "---",
        "...",
        "<<",
        "@x",
        "`x",
        "%x",
        "!x",
        "&x",
        "*x",
        "|",
        ">",
        "'",
        "\"",
        "\\",
        "a\\nb",
        "trailing newline\n",
        "\n",
        "x\r\ny",
    ];
    for text in strings {
        let json = serde_json::to_string(text).unwrap();
        for doc in [
            "k: old\n",
            "- old\n",
            "k: [old]\n",
            "k: 'old'\n",
            "k: \"old\"\n",
            "k: |\n  old\n",
        ] {
            let mut d = load_text(doc, DocKind::Yaml);
            let path = if doc.starts_with('-') {
                "/0"
            } else if doc.contains('[') {
                "/k/0"
            } else {
                "/k"
            };
            d.apply(set(path, &json))
                .unwrap_or_else(|e| panic!("{text:?} in {doc:?}: {}", e.message));
            let back = load_text(&text_of(&d), DocKind::Yaml).to_value().unwrap();
            let got = back.pointer(path).unwrap();
            assert_eq!(
                got,
                &json!(text),
                "{text:?} in {doc:?} -> {:?}",
                text_of(&d)
            );
        }
        // As an added key and as a renamed key, next to a sibling.
        let mut d = load_text("a: 1\n", DocKind::Yaml);
        d.apply(add(&format!("/{}", super::super::tests::esc(text)), "2"))
            .unwrap();
        d.apply(rename("/a", &format!("{text}x"))).unwrap();
        let back = load_text(&text_of(&d), DocKind::Yaml).to_value().unwrap();
        assert_eq!(
            back.get(text),
            Some(&json!(2)),
            "{text:?} -> {:?}",
            text_of(&d)
        );
        assert_eq!(back.get(format!("{text}x")), Some(&json!(1)));
    }
}

// ------------------------------------------------------------- model tests

const MODEL_YAML: &str = "# model document
flag: true
n: 1.50
s: x   # a string
list: [1, 2, 3]
empty: []
objs:
  - name: a
    on: true
  # second
  - name: b
    on: false
map: {k1: 1, k2: two}
nested:
  deep:
    x: [true, null]
a/b: 0
";

fn cases() -> u32 {
    std::env::var("YAML_PROP_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128)
}

#[test]
fn the_yaml_model_document_holds_the_json_model_data() {
    let json_doc = load_text(super::super::tests::MODEL_DOC, DocKind::Json);
    let yaml_doc = load_text(MODEL_YAML, DocKind::Yaml);
    assert!(yaml_doc.is_editable());
    assert_eq!(yaml_doc.to_value(), json_doc.to_value());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases()))]

    /// Every op behaves like the serde reference model, and the candidate
    /// always re-parses to exactly the model.
    #[test]
    fn ops_match_the_reference_model(sels in sel_strategy()) {
        let mut d = load_text(MODEL_YAML, DocKind::Yaml);
        prop_assert!(d.is_editable());
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            let mut next = model.clone();
            let ok = model_apply(&mut next, &op);
            let before = text_of(&d);
            let result = d.apply(op.clone());
            prop_assert_eq!(result.is_ok(), ok, "{:?}\n{}", op, before);
            if ok { model = next; }
            prop_assert_eq!(d.to_value().unwrap(), model.clone(), "{:?}", op);
            let reloaded = DocumentSession::load("r", d.candidate_bytes(), DocKind::Yaml);
            prop_assert!(reloaded.is_editable(), "{:?}: {}", op, text_of(&d));
            prop_assert_eq!(reloaded.to_value().unwrap(), model.clone());
        }
    }

    /// Apply N, undo k, redo k equals apply N; undo lands on the exact earlier
    /// bytes; a fresh session replaying the op list agrees.
    #[test]
    fn undo_redo_roundtrip(sels in sel_strategy(), k in 0usize..25) {
        let mut d = load_text(MODEL_YAML, DocKind::Yaml);
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
        let mut fresh = load_text(MODEL_YAML, DocKind::Yaml);
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
        let mut d = load_text(MODEL_YAML, DocKind::Yaml);
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
                let mut fresh = load_text(MODEL_YAML, DocKind::Yaml);
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

    /// Edits never touch bytes outside the edited node's lines: the lines
    /// before the first and after the last changed line are identical.
    #[test]
    fn edits_only_touch_their_own_lines(sels in sel_strategy()) {
        let mut d = load_text(MODEL_YAML, DocKind::Yaml);
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            let before = text_of(&d);
            if d.apply(op.clone()).is_ok() {
                model_apply(&mut model, &op);
                // Comments on untouched lines stay: the header always survives
                // unless the whole root was replaced.
                let root_replaced = matches!(&op, DraftOp::Set { path, .. } if path.is_empty());
                if !root_replaced {
                    prop_assert!(text_of(&d).starts_with("# model document\n"), "{:?}\n{}", op, before);
                }
            }
        }
    }
}

/// Layouts the model document does not cover: other indentation, indentless
/// sequences, compact items, explicit keys, block scalars, multi-line flow,
/// empty values, scalar roots, CRLF and no final newline.
const LAYOUTS: [&str; 8] = [
    "server:\n    host: a\n    ports:\n    - 80\n    - 443\n    opts: {a: 1}\nlist:\n- x: 1\n  y: 2\n-   z: 3\n- [a, b]\n",
    "# crlf\r\nflag: true\r\nlist: [1, 2]\r\nobjs:\r\n  - name: a\r\n    on: true\r\nmap: {k1: 1}   # c\r\nlast: end",
    "\"quoted key\": 'single'\n? explicit\n: \"double\"\nplain: |\n  block\n  text\nfolded: >-\n  f\n  g\nseq:\n  - 'a'\n  - \"b\"\n",
    "a: [1,\n  2, {k: v,\n  w: x}]\nb: {x: [1, 2], y: {z: 1}}\nc:   # empty\nd: ~\n",
    "- - a\n  - b\n- k: v\n  l:\n    - 1\n    - 2\n-\n- last\n",
    "just text\n",
    "# top\n\nname: x   # n\n\n# between\nlist:\n  # first\n  - 1\n  # second\n  - 2\n\n# end\n",
    "a:\n  b:\n    c:\n      - d: 1\n        e: [x]\n      - f\nz: 'q'\n",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases()))]

    /// The reference-model property on layouts other than the model document.
    #[test]
    fn ops_match_the_reference_model_on_other_layouts(sels in sel_strategy(), layout in 0..LAYOUTS.len()) {
        let text = LAYOUTS[layout];
        let mut d = load_text(text, DocKind::Yaml);
        prop_assert!(d.is_editable(), "{:?}", d.diagnostics());
        let mut model = d.to_value().unwrap();
        for sel in sels {
            let op = gen_op(&model, sel);
            let mut next = model.clone();
            let ok = model_apply(&mut next, &op);
            let before = text_of(&d);
            let result = d.apply(op.clone());
            prop_assert_eq!(result.is_ok(), ok, "{:?}\n{}", op, before);
            if ok { model = next; }
            prop_assert_eq!(d.to_value().unwrap(), model.clone(), "{:?}", op);
            let reloaded = DocumentSession::load("r", d.candidate_bytes(), DocKind::Yaml);
            prop_assert!(reloaded.is_editable(), "{:?}: {}", op, text_of(&d));
            prop_assert_eq!(reloaded.to_value().unwrap(), model.clone());
        }
    }
}

// ------------------------------------------------------------- session level

#[test]
fn undo_redo_and_revert_on_a_real_file() {
    let original = text_fixture("app-config.yaml");
    let mut d = open("app-config.yaml");
    d.apply(set("/name", "\"web\"")).unwrap();
    d.apply(add("/server/user", "\"bob\"")).unwrap();
    d.apply(remove("/tags/1")).unwrap();
    d.apply(insert("/features", 1, "\"mid\"")).unwrap();
    let all = text_of(&d);
    assert_ne!(all, original);
    for _ in 0..4 {
        d.undo().unwrap();
    }
    assert_eq!(text_of(&d), original);
    for _ in 0..4 {
        d.redo().unwrap();
    }
    assert_eq!(text_of(&d), all);
    // Revert one independent change (the second): the others stay.
    d.revert_op(1).unwrap();
    let want = original
        .replacen("name: \"api\"", "name: \"web\"", 1)
        .replacen("  - 'internal'   # keep the quotes\n", "", 1)
        .replacen(
            "features: [alpha, beta, 3]",
            "features: [alpha, mid, beta, 3]",
            1,
        );
    assert_eq!(text_of(&d), want);
    // A change that depends on an earlier one cannot be reverted alone.
    d.apply(set("/server/tls/enabled", "true")).unwrap();
    d.apply(set("/server/tls", "{\"enabled\": false}")).unwrap();
    assert_eq!(
        d.revert_op(3).unwrap_err().code,
        ErrorCode::InvalidOperation
    );
}

#[test]
fn review_masks_secret_values_in_the_diff() {
    let mut d = open("app-config.yaml");
    d.apply(set("/auth/API_TOKEN", "\"sk-new-1111222233334444\""))
        .unwrap();
    d.apply(set("/auth/password", "\"hunter2-new-fake\""))
        .unwrap();
    d.apply(add("/auth/DB_PASSWORD", "\"hunter2hunter2\""))
        .unwrap();
    d.apply(set("/name", "\"web\"")).unwrap();
    let review = d.review(&default_is_secret);
    let masked: Vec<bool> = review.changes.iter().map(|c| c.masked).collect();
    assert_eq!(masked, [true, true, true, false]);
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
        "hunter2-fake",
        "hunter2-new-fake",
        "hunter2hunter2",
    ] {
        assert!(!shown.contains(secret), "{secret} leaked:\n{shown}");
    }
    assert!(
        shown.contains("API_TOKEN") && shown.contains("name: \"web\""),
        "{shown}"
    );
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
        "hunter2hunter2",
    ] {
        assert!(revealed.iter().any(|r| r == want), "{want}: {revealed:?}");
    }
}

#[test]
fn review_masks_block_flow_and_quoted_secrets() {
    let doc = "token_a: |\n  sk-test-block-line-1\n  sk-test-block-line-2\ntoken_b: [sk-test-flow-1, \"sk-test-flow-2\"]\ntoken_c: 'sk-test-single'\ntoken_d: \"multi\n  sk-test-multi-line\"\nplain: ok\n";
    let mut d = load_text(doc, DocKind::Yaml);
    for k in ["a", "b", "c", "d"] {
        d.apply(rename(&format!("/token_{k}"), &format!("tok_{k}")))
            .unwrap();
    }
    d.apply(set("/plain", "\"changed\"")).unwrap();
    let review = d.review(&|p: &str| p.starts_with("/tok_"));
    let shown: String = review
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(shown.contains("changed"), "{shown}");
    assert!(!shown.contains("sk-test"), "leaked:\n{shown}");
}

#[test]
fn secret_lexemes_are_the_raw_source_text() {
    let doc = "a: sk-test-plain\nb: \"sk-test-dq\"\nc: 'sk-test-sq'\nd: |\n  sk-test-l1\n  sk-test-l2\ne: [sk-test-f]\nf: 12345\ng: true\nh: ''\nopen: visible\n";
    let d = parse(doc).ok().unwrap();
    let mut got = d.secret_lexemes(&|p: &str| p != "/open" && !p.is_empty());
    got.sort();
    assert_eq!(
        got,
        [
            "\"sk-test-dq\"",
            "'sk-test-sq'",
            "12345",
            "sk-test-f",
            "sk-test-l1",
            "sk-test-l2",
            "sk-test-plain",
        ]
    );
}

#[test]
fn conflict_and_reapply_keep_the_disk_change_byte_identical() {
    let mut d = open("app-config.yaml");
    d.apply(set("/port", "9090")).unwrap();
    d.apply(add("/server/user", "\"bob\"")).unwrap();
    let disk = text_fixture("app-config.yaml")
        .replacen(
            "log_level: info   # one of",
            "log_level: warn   # one of",
            1,
        )
        .into_bytes();
    let info = d.conflict(&disk, &no_secrets);
    assert!(info.can_reapply && info.overlapping.is_empty() && info.disk_parse_error.is_none());
    assert_eq!(info.disk_fingerprint, fingerprint(&disk));
    assert_eq!(info.disk_changes.len(), 1);
    assert_eq!(info.disk_changes[0].pointer, "/log_level");
    assert_eq!(info.draft_changes.len(), 2);
    d.reapply_onto(disk.clone()).unwrap();
    // `user` is appended after the last line of the whole `server` mapping.
    let want = String::from_utf8(disk.clone())
        .unwrap()
        .replacen("port: 8080", "port: 9090", 1)
        .replacen(
            "    cert: '/etc/ssl/certs/server.pem'\n",
            "    cert: '/etc/ssl/certs/server.pem'\n  user: bob\n",
            1,
        );
    assert_eq!(text_of(&d), want);
    assert_eq!(d.original_bytes(), &disk[..]);
    d.undo().unwrap();
    d.undo().unwrap();
    assert_eq!(d.candidate_bytes(), disk);
}

#[test]
fn overlapping_disk_change_blocks_reapply_and_keeps_the_draft() {
    let mut d = open("app-config.yaml");
    d.apply(set("/auth", "{\"x\": 1}")).unwrap();
    let disk = text_fixture("app-config.yaml")
        .replacen("hunter2-fake", "other-fake", 1)
        .into_bytes();
    let info = d.conflict(&disk, &no_secrets);
    assert!(!info.can_reapply);
    assert_eq!(info.overlapping, ["/auth"]);
    let before = d.candidate_bytes();
    assert!(d.reapply_onto(disk).is_err());
    assert_eq!(d.candidate_bytes(), before);
}

#[test]
fn a_disk_version_that_is_not_yaml_cannot_be_reapplied() {
    let mut d = open("app-config.yaml");
    d.apply(set("/port", "1")).unwrap();
    let disk = b"a: [unclosed\n".to_vec();
    let info = d.conflict(&disk, &no_secrets);
    assert!(!info.can_reapply);
    assert_eq!(info.disk_parse_error.unwrap().code, "INVALID_YAML");
    assert!(d.reapply_onto(disk).is_err());
}

// ------------------------------------------------------------------ blocked

const SECRETS: [&str; 8] = [
    "sk-test-1111111111111111",
    "ghx_fake_token_value",
    "hunter2-fake",
    "sk-test-2222",
    "sk-test-3333333333333333",
    "sk-test-block-4444",
    "sk-test-flow-5555",
    "sk-test-6666666666666666",
];

fn assert_masked_source(name: &str) {
    let d = open(name);
    let view = d.snapshot(false).source_view.unwrap();
    let original = text_fixture(name);
    let all = format!("{} {:?}", view.text, d.diagnostics());
    for token in SECRETS.iter().chain(&["sk-test-7777"]) {
        assert!(!all.contains(token), "{name}: {token} leaked:\n{all}");
    }
    // Same lines; keys and structure stay readable.
    assert_eq!(view.text.lines().count(), original.lines().count());
    for line in original.lines() {
        let key = line
            .trim_start()
            .trim_start_matches("- ")
            .split(':')
            .next()
            .unwrap();
        if line.contains(": ") && !key.contains(' ') {
            assert!(view.text.contains(key), "{name}: key {key} hidden");
        }
    }
    // Every mask reveals text from the original.
    assert!(!view.masks.is_empty());
    for m in &view.masks {
        let text = d.reveal_masked(m.id).unwrap();
        assert!(original.contains(&text), "{name}: {text}");
    }
}

#[test]
fn invalid_syntax_is_blocked_with_a_position_and_a_masked_view() {
    let d = open("invalid-syntax.yaml");
    assert!(!d.is_editable());
    let diag = &d.diagnostics()[0];
    assert_eq!(
        (diag.code.as_str(), diag.source),
        ("INVALID_YAML", DiagnosticSource::Syntax)
    );
    assert!(diag.line.is_some() && diag.column.is_some());
    assert_masked_source("invalid-syntax.yaml");
    let mut d = d;
    assert_eq!(
        d.apply(set("/x", "1")).unwrap_err().code,
        ErrorCode::NotEditable
    );
}

#[test]
fn duplicate_keys_are_blocked_with_one_diagnostic_each() {
    let d = open("duplicate-keys.yaml");
    assert!(!d.is_editable());
    let diags = d.diagnostics();
    assert_eq!(diags.len(), 1);
    assert_eq!(
        (
            diags[0].code.as_str(),
            diags[0].source,
            diags[0].pointer.as_deref()
        ),
        (
            "DUPLICATE_KEYS",
            DiagnosticSource::DuplicateKey,
            Some("/service/name")
        )
    );
    assert_eq!((diags[0].line, diags[0].column), (Some(2), Some(3)));
    assert_eq!(
        d.snapshot(false).source_view.unwrap().highlight_lines,
        [2, 4]
    );
    assert_masked_source("duplicate-keys.yaml");
    // Nested and flow duplicates too: one diagnostic per duplicated key.
    let d = load_text(
        "a: {x: 1, x: 2}\nb:\n  - k: 1\n    k: 2\nc: 1\nc: 2\n",
        DocKind::Yaml,
    );
    let pointers: Vec<_> = d
        .diagnostics()
        .into_iter()
        .map(|g| g.pointer.unwrap())
        .collect();
    assert_eq!(pointers, ["/a/x", "/b/0/k", "/c"]);
}

#[test]
fn several_documents_are_blocked() {
    let d = open("multi-document.yaml");
    assert!(!d.is_editable());
    assert_eq!(d.diagnostics()[0].code, "MULTIPLE_DOCUMENTS");
    assert_eq!(
        d.diagnostics()[0].message,
        "Files with several YAML documents can't be edited here yet."
    );
    assert_masked_source("multi-document.yaml");
}

#[test]
fn complex_keys_are_blocked_not_misread() {
    let d = load_text("? [a, b]\n: 1\n", DocKind::Yaml);
    assert!(!d.is_editable());
    assert_eq!(d.diagnostics()[0].code, "UNSUPPORTED_YAML");
    for bad in [
        "a: 1\nb: *nope\n",
        "a: \"bad \\q escape\"\n",
        "a: b: c\n",
        "a: [1, 2\n",
        "- a\nb: 1\n",
    ] {
        let d = load_text(bad, DocKind::Yaml);
        assert!(!d.is_editable(), "{bad:?}");
        assert!(d.snapshot(false).source_view.is_some(), "{bad:?}");
    }
}

/// The masked view of a broken file never shows a value, whatever the layout.
#[test]
fn masked_source_never_leaks_values() {
    let docs = [
        "key: SECRETVALUE\n",
        "key:   SECRETVALUE   # visible comment\n",
        "- SECRETVALUE\n- k: SECRETVALUE\n- - SECRETVALUE\n",
        "k: \"SECRETVALUE\"\nk2: 'SECRETVALUE'\n",
        "k: \"first\n  SECRETVALUE: more\n  SECRETVALUE\"\nafter: SECRETVALUE\n",
        "k: [SECRETVALUE,\n  SECRETVALUE: 1,\n  {SECRETVALUE: 2}]\n",
        "k: {a: SECRETVALUE,\n  b: SECRETVALUE}\nz: SECRETVALUE\n",
        "k: |\n  SECRETVALUE: x\n  # SECRETVALUE\n  SECRETVALUE\nz: SECRETVALUE\n",
        "k: >-\n    SECRETVALUE\n\n    SECRETVALUE\nz: 1\n",
        "- |\n  SECRETVALUE\n- >\n  SECRETVALUE\n",
        "k: &a SECRETVALUE\nz: !tag SECRETVALUE\nw: *a\n",
        "\"quoted key\": SECRETVALUE\n? SECRETVALUE_key\n: SECRETVALUE\n",
        "k: plain\n  SECRETVALUE continued\n  SECRETVALUE\n",
        "---\nk: SECRETVALUE\n--- SECRETVALUE\nz: SECRETVALUE\n",
        "k: 'it''s SECRETVALUE'  # c\nm: \"a \\\" SECRETVALUE\"\n",
        "url: http://SECRETVALUE:SECRETVALUE@host/x\nt: a#SECRETVALUE\n",
        "k: [a, \"unclosed SECRETVALUE\nm: SECRETVALUE\n",
        "k: 'unclosed\nSECRETVALUE: SECRETVALUE\n",
    ];
    for doc in docs {
        let (view, originals) = masked_view(doc, vec![], None);
        let leaked = view.text.replace("SECRETVALUE_key", "");
        assert!(
            !leaked.contains("SECRETVALUE"),
            "leaked in {doc:?}:\n{}",
            view.text
        );
        assert_eq!(view.text.lines().count(), doc.lines().count(), "{doc:?}");
        assert!(!originals.is_empty(), "{doc:?}");
        // Keys and comments stay.
        if doc.contains("# visible comment") {
            assert!(view.text.contains("# visible comment"));
        }
    }
    let (view, _) = masked_view("name: x\n# keep me\nlist:\n  - a\n", vec![], None);
    assert!(
        view.text.contains("name:")
            && view.text.contains("# keep me")
            && view.text.contains("list:")
    );
}

// ----------------------------------------------- anchors, aliases and tags

fn refused(d: &mut DocumentSession, op: DraftOp, what: &str) {
    let before = d.candidate_bytes();
    let err = d.apply(op.clone()).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidOperation, "{op:?}");
    assert!(
        err.message.contains("text editor") && err.message.contains(what),
        "{op:?}: {}",
        err.message
    );
    assert_eq!(d.candidate_bytes(), before);
    assert!(d.ops().is_empty());
}

#[test]
fn anchors_and_aliases_load_and_resolve() {
    let d = open("anchors.yaml");
    let v = d.to_value().unwrap();
    assert_eq!(v["development"]["<<"], v["defaults"]);
    assert_eq!(v["production"]["<<"]["host"], json!("db.example.test"));
    assert_eq!(v["mirror"], json!([80, 443]));
    assert_eq!(v["other"], json!("service-one"));
    assert_eq!(v["stack"]["Bucket"], json!("BucketName"));
    assert_eq!(v["stack"]["Label"], json!("123"));
    assert_eq!(v["stack"]["Role"], json!(["RoleA", "Arn"]));
}

#[test]
fn edits_through_or_onto_shared_nodes_are_refused() {
    let mut d = open("anchors.yaml");
    for op in [
        set("/development/<</host", "\"x\""),
        set("/development/<<", "{}"),
        remove("/development/<<"),
        rename("/development/<<", "merge"),
        add("/development/<</extra", "1"),
        set("/mirror", "[]"),
        set("/mirror/0", "1"),
        insert("/mirror", 0, "1"),
        remove("/other"),
        set("/other", "\"x\""),
        // Targets that define an anchor others use.
        set("/defaults", "{}"),
        remove("/defaults"),
        rename("/defaults", "base"),
        set("/defaults/host", "\"x\""),
        add("/defaults/extra", "1"),
        remove("/defaults/adapter"),
        rename("/defaults/host", "hostname"),
        insert("/shared_list", 0, "1"),
        remove("/shared_list/0"),
        mv("/shared_list", 0, 1),
        set("/plain_anchor", "\"x\""),
        remove("/plain_anchor"),
        rename("/plain_anchor", "other_name"),
        // Removing a parent that holds the anchor.
        set("", "{}"),
    ] {
        refused(&mut d, op, "shared");
    }
}

#[test]
fn tagged_nodes_are_refused_but_untagged_siblings_edit() {
    let mut d = open("anchors.yaml");
    for op in [
        set("/stack/Bucket", "\"x\""),
        remove("/stack/Bucket"),
        set("/stack/Label", "\"x\""),
        set("/stack/Role", "[]"),
        insert("/stack/Role", 0, "\"x\""),
    ] {
        let before = d.candidate_bytes();
        let err = d.apply(op.clone()).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidOperation, "{op:?}");
        assert!(
            err.message.contains("tag") || err.message.contains("text editor"),
            "{op:?}: {}",
            err.message
        );
        assert_eq!(d.candidate_bytes(), before);
    }
    d.apply(set("/stack/Free", "\"changed\"")).unwrap();
    d.apply(add("/production/port", "5432")).unwrap();
    d.apply(rename("/stack/Free", "Cheap")).unwrap();
    // Inside a tagged collection the untagged items edit normally.
    d.apply(remove("/stack/Role/0")).unwrap();
    let want = text_fixture("anchors.yaml")
        .replacen("  Free: plain", "  Cheap: changed", 1)
        .replacen("!GetAtt [RoleA, Arn]", "!GetAtt [Arn]", 1)
        .replacen(
            "  host: prod.example.test\n",
            "  host: prod.example.test\n  port: 5432\n",
            1,
        );
    assert_eq!(text_of(&d), want);
    // Unreferenced anchors are harmless below them.
    let mut d = load_text("a: &x\n  k: 1\n", DocKind::Yaml);
    d.apply(set("/a/k", "2")).unwrap();
    assert_eq!(text_of(&d), "a: &x\n  k: 2\n");
}

#[test]
fn emptying_a_collection_keeps_its_anchor_in_front_and_refuses_a_tagged_one() {
    let mut d = load_text("a: &x\n  k: 1\nb: 1\n", DocKind::Yaml);
    d.apply(remove("/a/k")).unwrap();
    assert_eq!(text_of(&d), "a: &x {}\nb: 1\n");
    // A tagged mapping is never rewritten: removing its last member is refused.
    let mut d = load_text("a: !!map\n  k: 1\nb: 1\n", DocKind::Yaml);
    assert!(d.apply(remove("/a/k")).is_err());
    assert_eq!(text_of(&d), "a: !!map\n  k: 1\nb: 1\n");
}

/// Files as people write them load, round-trip, and take edits.
#[test]
fn real_world_layouts_load_and_edit() {
    let workflow = "name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [ main, 'release/**' ]
env:
  RUST_LOG: info   # verbose
jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - name: Test
        run: |
          cargo test --all
          echo \"done: ok\"
        env: { FOO: bar }
      - name: Deploy
        if: github.ref == 'refs/heads/main' && matrix.os == 'ubuntu-latest'
        run: >
          ./deploy.sh
          --prod
";
    let compose = "version: \"3.8\"
services:
  web:
    image: \"nginx:1.25\"
    ports:
      - \"8080:80\"
      - 443:443
    environment:
      - KEY=value
      - URL=http://host:80/path?a=b#frag
    healthcheck:
      test: [\"CMD\", \"curl\", \"-f\", \"http://localhost\"]
      interval: 30s
    command: [\"sh\", \"-c\", \"echo hi && sleep 1\"]
volumes:
  data: {}
";
    let k8s = "apiVersion: apps/v1
kind: Deployment
metadata:
  name: demo
  labels: {app: demo, tier: \"backend\"}
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: app
          image: demo:1.0
          args: [\"--port=8080\", \"--verbose\"]
          resources:
            limits: {cpu: 500m, memory: 128Mi}
";
    let helm = "# Default values
replicaCount: 1
image:
  repository: nginx
  tag: \"\"
  pullPolicy: IfNotPresent
nameOverride: \"\"
podAnnotations: {}
ingress:
  enabled: false
  hosts:
    - host: chart-example.local
      paths:
        - path: /
          pathType: ImplementationSpecific
  tls: []
tolerations: []
";
    let directive = "%YAML 1.2\n---\nname: x\nlist:\n  - 1\n";
    for (name, text) in [
        ("workflow", workflow),
        ("compose", compose),
        ("k8s", k8s),
        ("helm", helm),
        ("directive", directive),
    ] {
        let mut d = load_text(text, DocKind::Yaml);
        assert!(d.is_editable(), "{name}: {:?}", d.diagnostics());
        assert_eq!(text_of(&d), text, "{name}");
        let value = d.to_value().unwrap();
        // Toggle, rename and add inside each; the rest stays byte-identical.
        let first_key = value.as_object().unwrap().keys().next().unwrap().clone();
        d.apply(add("/added_key", "\"added\"")).unwrap();
        assert_eq!(text_of(&d), format!("{text}added_key: added\n"), "{name}");
        d.apply(rename(&format!("/{first_key}"), "renamed_first"))
            .unwrap();
        assert_eq!(
            text_of(&d),
            format!("{text}added_key: added\n").replacen(&first_key, "renamed_first", 1),
            "{name}"
        );
    }
    let mut d = load_text(workflow, DocKind::Yaml);
    d.apply(set("/env/RUST_LOG", "\"debug\"")).unwrap();
    d.apply(insert(
        "/jobs/build/strategy/matrix/os",
        1,
        "\"macos-latest\"",
    ))
    .unwrap();
    d.apply(set("/jobs/build/steps/1/env/FOO", "\"baz\""))
        .unwrap();
    d.apply(set("/jobs/build/steps/2/run", "\"./deploy.sh --stage\\n\""))
        .unwrap();
    let want = workflow
        .replacen(
            "RUST_LOG: info   # verbose",
            "RUST_LOG: debug   # verbose",
            1,
        )
        .replacen(
            "[ubuntu-latest, windows-latest]",
            "[ubuntu-latest, macos-latest, windows-latest]",
            1,
        )
        .replacen("{ FOO: bar }", "{ FOO: baz }", 1)
        .replacen(
            "run: >\n          ./deploy.sh\n          --prod\n",
            "run: |\n          ./deploy.sh --stage\n",
            1,
        );
    assert_eq!(text_of(&d), want);
}

// ------------------------------------------------------------- primitives

#[test]
fn the_primitives_report_their_own_errors() {
    // `apply_edit` validates before the primitives run; a path the node tree
    // cannot walk (here: through an alias) is refused, never panics.
    let mut doc = parse("a: &x [1]\nb: *x\n").ok().unwrap();
    assert!(Edit::set(&mut doc, &["b".into(), "0".into()], &Val::Null).is_err());
    assert!(Edit::remove(&mut doc, &["nope".into()]).is_err());
    assert_eq!(Syntax::render(&doc), "a: &x [1]\nb: *x\n");
}
