use std::fs;

use serde_json::{Value, json};

use super::*;
use crate::core::types::{DiagnosticSource, Severity};

fn fixture() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/claude-settings.json");
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn bundled_presets_load() {
    let r = PresetRegistry::bundled();
    let info = r.info("claude-code-settings").unwrap();
    assert_eq!(info.display_name, "Claude Code Settings");
    assert_eq!(
        info.groups
            .iter()
            .map(|g| g.id.as_str())
            .collect::<Vec<_>>(),
        [
            "general",
            "permissions",
            "environment",
            "hooks",
            "status-line",
            "plugins"
        ]
    );
    assert!(
        r.schema("claude-code-settings").is_some(),
        "vendored schema compiles"
    );
    assert!(r.info("generic").is_some());
    assert!(r.schema("generic").is_none());
    assert!(r.info("nope").is_none());
}

#[test]
fn ui_json_only_references_schema_properties() {
    let r = PresetRegistry::bundled();
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../presets/claude-code/settings.schema.json"
    ))
    .unwrap();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.len() > 100);
    let p = r.get("claude-code-settings").unwrap();
    let top = |ptr: &str| decode_pointer(ptr).into_iter().next().unwrap();
    for g in &p.info.groups {
        for ptr in &g.pointers {
            assert!(
                props.contains_key(&top(ptr)),
                "group pointer {ptr} not in schema"
            );
        }
    }
    for ptr in
        p.ui.sensitive_pointers
            .iter()
            .chain(p.ui.descriptions.keys())
    {
        assert!(props.contains_key(&top(ptr)), "{ptr} not in schema");
    }
}

#[test]
fn claude_fixture_has_no_diagnostics_for_unknown_key() {
    // A24: `unknownFutureVendorSetting` is allowed (additionalProperties true).
    let r = PresetRegistry::bundled();
    let d = r
        .schema("claude-code-settings")
        .unwrap()
        .validate(&fixture());
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn claude_schema_violations_are_warnings() {
    let r = PresetRegistry::bundled();
    let d = r.schema("claude-code-settings").unwrap().validate(
        &json!({"permissions": {"defaultMode": "totally-new-mode"}, "cleanupPeriodDays": "x"}),
    );
    assert!(
        d.iter()
            .any(|d| d.pointer.as_deref() == Some("/permissions/defaultMode")),
        "{d:?}"
    );
    assert!(
        d.iter()
            .all(|d| d.severity == Severity::Warning && d.source == DiagnosticSource::PresetSchema)
    );
    assert!(d.iter().all(|d| !d.message.contains("totally-new-mode")));
}

#[test]
fn claude_hints_on_fixture() {
    let r = PresetRegistry::bundled();
    let s = r.schema("claude-code-settings").unwrap();
    let doc = fixture();
    let h = s.hints(
        &doc,
        &[
            "".into(),
            "/permissions".into(),
            "/permissions/defaultMode".into(),
            "/env".into(),
            "/model".into(),
            "/unknownFutureVendorSetting".into(),
        ],
    );
    assert!(
        h[0].addable_properties.contains(&"hooks".to_owned())
            && !h[0].addable_properties.contains(&"model".to_owned())
    );
    assert!(
        h[1].addable_properties.contains(&"defaultMode".to_owned())
            && !h[1].addable_properties.contains(&"allow".to_owned())
    );
    assert!(
        h[2].enum_values
            .as_ref()
            .unwrap()
            .contains(&"\"plan\"".to_owned())
    );
    assert!(
        !h[3].open_map,
        "schema lists fixed env keys; the preset layer opens the map"
    );
    assert!(r.ui_hints("claude-code-settings", &["/env".into()])[0].open_map);
    assert!(h[4].description.is_some());
    assert_eq!(
        h[5],
        FieldHint {
            pointer: "/unknownFutureVendorSetting".into(),
            ..FieldHint::default()
        }
    );
}

#[test]
fn ui_hints_add_summary_sensitivity_and_secret() {
    let r = PresetRegistry::bundled();
    let ptrs: Vec<String> = [
        "/hooks/PreToolUse",
        "/permissions/allow",
        "/permissions",
        "/env/GITHUB_TOKEN",
        "/env/PATH",
        "/model",
    ]
    .map(String::from)
    .to_vec();
    let h = r.ui_hints("claude-code-settings", &ptrs);
    assert_eq!(h[0].item_summary.as_deref(), Some("matcher"));
    assert!(h[1].sensitive && h[2].sensitive);
    assert!(h[2].description.is_some());
    assert!(h[3].secret && !h[4].secret);
    assert!(!h[5].sensitive && h[5].description.is_none());
    assert_eq!(r.ui_hints("nope", &ptrs).len(), ptrs.len());
}

#[test]
fn is_secret_globs() {
    let r = PresetRegistry::bundled();
    let id = "claude-code-settings";
    for yes in [
        "/env/GITHUB_TOKEN",
        "/env/api_key",
        "/env/MY_SECRET_THING",
        "/env/db_Password",
        "/env/ANTHROPIC_API_KEY",
    ] {
        assert!(r.is_secret(id, yes), "{yes}");
    }
    for no in ["/env/PATH", "/env", "/env/A/KEY", "/model", "/env"] {
        assert!(!r.is_secret(id, no), "{no}");
    }
    assert!(!r.is_secret("generic", "/env/TOKEN"));
    assert!(!r.is_secret("missing", "/env/TOKEN"));
}

#[test]
fn wildcard_basics() {
    let w = |p: &str, s: &str| {
        wildcard(
            &p.chars().collect::<Vec<_>>(),
            &s.chars().collect::<Vec<_>>(),
        )
    };
    assert!(w("*", "") && w("a*b*c", "aXXbYYc") && w("?x", "ax") && w("*key*", "key"));
    assert!(!w("a*b", "ac") && !w("?", "") && !w("abc", "ab"));
}

#[test]
fn detect_rules() {
    let r = PresetRegistry::bundled();
    let id = Some("claude-code-settings".to_owned());
    for p in [
        "/home/u/.claude/settings.json",
        "/proj/.claude/settings.local.json",
        "/etc/claude-code/managed-settings.json",
        "/etc/claude-code/managed-settings.d/10-a.json",
    ] {
        assert_eq!(r.detect(Path::new(p), None), id, "{p}");
    }
    for p in [
        "/home/u/settings.json",
        "/proj/settings.local.json",
        "/home/u/.claude.json",
        "/etc/claude-code/managed-settings.d/readme.txt",
        "/home/u/.claude/other.json",
    ] {
        assert_eq!(r.detect(Path::new(p), None), None, "{p}");
    }
    let by_schema = json!({"$schema": "https://json.schemastore.org/claude-code-settings.json"});
    assert_eq!(r.detect(Path::new("/x/custom.json"), Some(&by_schema)), id);
    assert_eq!(
        r.detect(
            Path::new("/x/custom.json"),
            Some(&json!({"$schema": "https://example.com/s.json"}))
        ),
        None
    );
    assert_eq!(
        r.detect(Path::new("/x/custom.json"), Some(&fixture())),
        None
    );
}

fn env(root: &Path, config_dir: Option<&str>) -> ClaudeEnv {
    ClaudeEnv {
        home: root.join("home"),
        config_dir: config_dir.map(|c| root.join(c)),
        managed_dir: root.join("etc-claude"),
    }
}

#[test]
fn discovery_user_and_managed() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path();
    fs::create_dir_all(root.join("home/.claude")).unwrap();
    fs::write(root.join("home/.claude/settings.json"), "{}").unwrap();
    fs::write(root.join("home/.claude.json"), "{}").unwrap(); // runtime state: never listed
    fs::create_dir_all(root.join("etc-claude/managed-settings.d")).unwrap();
    fs::write(root.join("etc-claude/managed-settings.json"), "{}").unwrap();
    fs::write(root.join("etc-claude/managed-settings.d/20-b.json"), "{}").unwrap();
    fs::write(root.join("etc-claude/managed-settings.d/10-a.json"), "{}").unwrap();
    fs::write(root.join("etc-claude/managed-settings.d/notes.txt"), "x").unwrap();

    let c = discover_claude(&env(root, None), None);
    assert_eq!(c.len(), 4);
    assert_eq!(
        (c[0].scope, c[0].exists, c[0].read_only),
        (Scope::User, true, false)
    );
    assert!(c[0].path.ends_with("home/.claude/settings.json"));
    assert!(
        c[1..]
            .iter()
            .all(|c| c.scope == Scope::Managed && c.read_only && c.exists && !c.already_registered)
    );
    assert!(c[2].path.ends_with("10-a.json") && c[3].path.ends_with("20-b.json"));
    assert!(c.iter().all(|c| !c.path.ends_with(".claude.json")));
    let ids: std::collections::HashSet<_> = c.iter().map(|c| &c.candidate_id).collect();
    assert_eq!(ids.len(), c.len());
    assert_eq!(
        discover_claude(&env(root, None), None)[0].candidate_id,
        c[0].candidate_id,
        "ids are stable"
    );
}

#[test]
fn discovery_config_dir_overrides_home_and_missing_is_honest() {
    let t = tempfile::tempdir().unwrap();
    let c = discover_claude(&env(t.path(), Some("cfg")), None);
    assert_eq!(c.len(), 1, "no managed dir: only the user file is listed");
    assert!(c[0].path.ends_with("cfg/settings.json") && !c[0].exists);
}

#[test]
fn discovery_project_dir() {
    let t = tempfile::tempdir().unwrap();
    let proj = t.path().join("proj");
    fs::create_dir_all(proj.join(".claude")).unwrap();
    fs::write(proj.join(".claude/settings.json"), "{}").unwrap();
    let c = discover_claude(&env(t.path(), None), Some(&proj));
    let shared = c.iter().find(|c| c.scope == Scope::ProjectShared).unwrap();
    let local = c.iter().find(|c| c.scope == Scope::ProjectLocal).unwrap();
    assert!(shared.exists && !shared.read_only && shared.note.is_none());
    assert!(
        !local.exists,
        "missing local file is listed so the UI can offer Create"
    );
    let note = local.note.as_deref().unwrap();
    assert!(note.contains("2.1.211") && note.contains("version control"));
    assert!(local.path.ends_with("proj/.claude/settings.local.json"));
    // Discovery never writes.
    assert!(!proj.join(".claude/settings.local.json").exists());
}
