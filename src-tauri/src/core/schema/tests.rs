use serde_json::{Value, json};

use super::*;

fn compile(schema: Value, origin: SchemaOrigin) -> CompiledSchema {
    CompiledSchema::compile(&schema, origin).expect("schema compiles")
}

fn restrictive() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": {"type": "string", "minLength": 3},
            "mode": {"enum": ["a", "b"]},
            "port": {"type": "integer", "minimum": 1, "maximum": 65535, "default": 8080},
            "token": {"type": "string", "format": "password"}
        },
        "additionalProperties": false
    })
}

#[test]
fn severity_follows_origin() {
    let doc = json!({"name": "ab"});
    let user = compile(restrictive(), SchemaOrigin::User).validate(&doc);
    assert_eq!(user.len(), 1);
    assert_eq!(
        (user[0].severity, user[0].source),
        (Severity::Error, DiagnosticSource::UserSchema)
    );
    let preset = compile(restrictive(), SchemaOrigin::Preset).validate(&doc);
    assert_eq!(
        (preset[0].severity, preset[0].source),
        (Severity::Warning, DiagnosticSource::PresetSchema)
    );
}

#[test]
fn user_schema_errors_land_on_the_right_field() {
    let s = compile(restrictive(), SchemaOrigin::User);
    let d = s.validate(&json!({"name": "abc", "mode": "z", "port": 0, "extra": 1}));
    let at = |p: &str| d.iter().filter(|d| d.pointer.as_deref() == Some(p)).count();
    assert_eq!(at("/mode"), 1);
    assert_eq!(at("/port"), 1);
    assert_eq!(at("/name"), 0);
    assert!(
        d.iter().any(|d| d.message.contains("extra")),
        "additional property named: {d:?}"
    );
    let mode = d
        .iter()
        .find(|d| d.pointer.as_deref() == Some("/mode"))
        .unwrap();
    assert_eq!(mode.message, "must be one of: \"a\", \"b\"");
}

#[test]
fn pointer_is_rfc6901_escaped_and_root_is_empty() {
    let s = compile(
        json!({"type": "object", "properties": {"a/b": {"type": "object", "properties": {"m~n": {"type": "integer"}}}}, "required": ["x"]}),
        SchemaOrigin::User,
    );
    let d = s.validate(&json!({"a/b": {"m~n": "str"}}));
    let ptrs: Vec<_> = d.iter().filter_map(|d| d.pointer.as_deref()).collect();
    assert!(ptrs.contains(&"/a~1b/m~0n"), "{ptrs:?}");
    assert!(ptrs.contains(&""), "{ptrs:?}");
}

#[test]
fn messages_never_echo_values() {
    let s = compile(restrictive(), SchemaOrigin::User);
    let d = s.validate(&json!({"name": "ab", "port": "hunter2-secret", "mode": "hunter2-secret"}));
    assert!(!d.is_empty());
    assert!(d.iter().all(|d| !d.message.contains("hunter2")), "{d:?}");
}

#[test]
fn remote_and_file_refs_are_rejected() {
    for r in [
        "https://example.invalid/other.json",
        "http://127.0.0.1:1/x.json",
        "other.json",
        "file:///etc/passwd",
    ] {
        let schema = json!({"type": "object", "properties": {"a": {"$ref": r}}});
        assert!(
            CompiledSchema::compile(&schema, SchemaOrigin::User).is_err(),
            "{r} must be rejected"
        );
    }
}

#[test]
fn local_refs_work() {
    let s = compile(
        json!({"$defs": {"n": {"type": "integer"}}, "properties": {"a": {"$ref": "#/$defs/n"}}}),
        SchemaOrigin::User,
    );
    assert!(s.validate(&json!({"a": 1})).is_empty());
    assert_eq!(s.validate(&json!({"a": "x"})).len(), 1);
}

#[test]
fn invalid_schema_is_a_compile_error() {
    let err = CompiledSchema::compile(&json!({"type": "nonsense"}), SchemaOrigin::User)
        .err()
        .expect("error");
    assert_eq!(err.code, ErrorCode::SchemaValidationFailed);
    assert!(CompiledSchema::compile(&json!({"properties": 5}), SchemaOrigin::User).is_err());
}

#[test]
fn load_user_reads_file_and_reports_bad_json() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("s.json");
    std::fs::write(&good, restrictive().to_string()).unwrap();
    let s = CompiledSchema::load_user(&good).unwrap();
    assert_eq!(s.origin(), SchemaOrigin::User);
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, "{ nope").unwrap();
    assert_eq!(
        CompiledSchema::load_user(&bad).err().unwrap().code,
        ErrorCode::SchemaValidationFailed
    );
    assert_eq!(
        CompiledSchema::load_user(&dir.path().join("missing.json"))
            .err()
            .unwrap()
            .code,
        ErrorCode::NotFound
    );
}

#[test]
fn draft_2020_12_is_detected() {
    let s = compile(
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "prefixItems": [{"type": "integer"}, {"type": "string"}]}),
        SchemaOrigin::User,
    );
    assert!(s.validate(&json!([1, "a"])).is_empty());
    assert_eq!(s.validate(&json!(["a", 1])).len(), 2);
}

#[test]
fn hints_describe_fields() {
    let s = compile(
        json!({
            "type": "object",
            "properties": {
                "mode": {"title": "Mode", "description": "Pick one", "enum": ["a", 2]},
                "port": {"type": "integer", "minimum": 1, "maximum": 9, "default": 8080},
                "name": {"type": "string", "minLength": 2, "maxLength": 5, "pattern": "^a", "format": "uri"},
                "pw": {"type": "string", "writeOnly": true},
                "tags": {"type": "object", "additionalProperties": {"type": "string"}},
                "list": {"type": "array", "items": {"$ref": "#/$defs/item"}},
                "opt": {"type": "string"}
            },
            "$defs": {"item": {"type": "object", "title": "Item", "properties": {"id": {"type": "integer"}}}}
        }),
        SchemaOrigin::User,
    );
    let doc = json!({"mode": "a", "list": [{"id": 1}]});
    let h = s.hints(
        &doc,
        &[
            "/mode".into(),
            "/port".into(),
            "/name".into(),
            "/pw".into(),
            "/tags".into(),
            "/list/0".into(),
            "".into(),
            "/nope/x".into(),
        ],
    );
    assert_eq!(h[0].title.as_deref(), Some("Mode"));
    assert_eq!(h[0].description.as_deref(), Some("Pick one"));
    assert_eq!(
        h[0].enum_values.as_deref(),
        Some(&["\"a\"".to_owned(), "2".to_owned()][..])
    );
    assert_eq!(
        (
            h[1].default_json.as_deref(),
            h[1].minimum.as_deref(),
            h[1].maximum.as_deref()
        ),
        (Some("8080"), Some("1"), Some("9"))
    );
    assert_eq!(
        (
            h[2].min_length,
            h[2].max_length,
            h[2].pattern.as_deref(),
            h[2].format.as_deref()
        ),
        (Some(2), Some(5), Some("^a"), Some("uri"))
    );
    assert!(h[3].secret && !h[2].secret);
    assert!(h[4].open_map && h[4].addable_properties.is_empty());
    assert_eq!(h[5].title.as_deref(), Some("Item"));
    assert_eq!(h[5].addable_properties, Vec::<String>::new());
    let mut addable = h[6].addable_properties.clone();
    addable.sort();
    assert_eq!(addable, ["name", "opt", "port", "pw", "tags"]);
    assert!(!h[6].open_map);
    assert_eq!(
        h[7],
        FieldHint {
            pointer: "/nope/x".into(),
            ..FieldHint::default()
        }
    );
}

#[test]
fn hints_merge_all_of_and_password_format() {
    let s = compile(
        json!({"allOf": [{"properties": {"a": {"format": "password"}}}, {"properties": {"b": {"description": "B"}}}]}),
        SchemaOrigin::User,
    );
    let h = s.hints(&json!({}), &["/a".into(), "/b".into(), "".into()]);
    assert!(h[0].secret);
    assert_eq!(h[1].description.as_deref(), Some("B"));
    assert_eq!(h[2].addable_properties, ["a", "b"]);
}

#[test]
fn hints_report_value_type_and_required() {
    let s = compile(
        json!({"required": ["a"], "properties": {"a": {"type": "boolean"}, "b": {"type": ["null", "integer"]}}}),
        SchemaOrigin::User,
    );
    let h = s.hints(&json!({}), &["/a".into(), "/b".into(), "/c".into()]);
    assert_eq!(
        (h[0].value_type.as_deref(), h[0].required),
        (Some("boolean"), true)
    );
    assert_eq!(
        (h[1].value_type.as_deref(), h[1].required),
        (Some("integer"), false)
    );
    assert_eq!((h[2].value_type.as_deref(), h[2].required), (None, false));
}
