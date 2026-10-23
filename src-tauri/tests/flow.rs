//! Cross-lane flow tests: registry → document engine → schema → save pipeline
//! → backups → conflict/reapply → restore. Temp dirs and fixtures only.

use std::fs;
use std::path::{Path, PathBuf};

use configdeck_lib::core::backup::BackupStore;
use configdeck_lib::core::document::DocumentSession;
use configdeck_lib::core::persistence::{self, LinkConfirmations, SaveRequest};
use configdeck_lib::core::presets::PresetRegistry;
use configdeck_lib::core::registry::Registry;
use configdeck_lib::core::types::{DocKind, DraftOp, Scope, Severity};
use configdeck_lib::core::{CoreResult, ErrorCode, MAX_EDITABLE_BYTES};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures")
            .join(name),
    )
    .unwrap()
}

struct Env {
    _dir: tempfile::TempDir,
    work: PathBuf,
    config: PathBuf,
    backups: BackupStore,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let config = dir.path().join("config");
        fs::create_dir_all(&work).unwrap();
        let backups = BackupStore::new(dir.path().join("data/backups"), 20);
        Self {
            work,
            config,
            backups,
            _dir: dir,
        }
    }

    fn put(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.work.join(name);
        fs::write(&p, bytes).unwrap();
        p
    }

    fn save(
        &self,
        id: &str,
        path: &Path,
        session: &DocumentSession,
    ) -> CoreResult<persistence::SaveOutcome> {
        let candidate = session.candidate_bytes();
        persistence::save(&SaveRequest {
            file_id: id,
            path,
            expected: session.base_fingerprint(),
            candidate: &candidate,
            confirmations: LinkConfirmations::default(),
            expected_target: None,
            validate: &|bytes: &[u8]| {
                if DocumentSession::load(id, bytes.to_vec(), DocKind::Json).is_editable() {
                    Ok(())
                } else {
                    Err(configdeck_lib::core::CoreError::new(
                        ErrorCode::InvalidJson,
                        "invalid",
                    ))
                }
            },
            backups: &self.backups,
            change_count: Some(session.ops().len() as u32),
        })
    }
}

fn set(path: &str, json: &str) -> DraftOp {
    DraftOp::Set {
        path: path.into(),
        value_json: json.into(),
    }
}

#[test]
fn edit_save_backup_restore_round_trip() {
    let env = Env::new();
    let original = fixture("feature-flags.json");
    let path = env.put("feature-flags.json", &original);
    let (mut registry, notices) = Registry::load(&env.config);
    assert!(notices.is_empty());
    let entry = registry.add(&path, None, None, None).unwrap();

    let bytes = persistence::read_checked(&path, MAX_EDITABLE_BYTES).unwrap();
    let mut session = DocumentSession::load(&entry.id, bytes, entry.kind);
    session.apply(set("/debug", "true")).unwrap();

    // A02: only the boolean token changes.
    let expected = String::from_utf8(original.clone()).unwrap().replacen(
        "\"debug\": false",
        "\"debug\": true",
        1,
    );
    assert_eq!(session.candidate_bytes(), expected.as_bytes());

    // A16: save writes the candidate and keeps the original as a backup.
    let outcome = env.save(&entry.id, &path, &session).unwrap();
    assert_eq!(fs::read(&path).unwrap(), expected.as_bytes());
    assert_eq!(
        env.backups.read(&entry.id, &outcome.backup_id).unwrap(),
        original
    );
    session.adopt_saved(expected.clone().into_bytes());
    assert!(!session.is_dirty());

    // A17: restoring the backup goes through the same safe save.
    let backup = env.backups.read(&entry.id, &outcome.backup_id).unwrap();
    let restored = persistence::save(&SaveRequest {
        file_id: &entry.id,
        path: &path,
        expected: &outcome.fingerprint,
        candidate: &backup,
        confirmations: LinkConfirmations::default(),
        expected_target: None,
        validate: &|_| Ok(()),
        backups: &env.backups,
        change_count: None,
    })
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(env.backups.list(&entry.id).unwrap().len(), 2);
    assert_ne!(restored.fingerprint, outcome.fingerprint);
}

#[test]
fn external_change_blocks_save_then_reapply_keeps_both_edits() {
    let env = Env::new();
    let path = env.put("settings.json", &fixture("claude-settings.json"));
    let bytes = persistence::read_checked(&path, MAX_EDITABLE_BYTES).unwrap();
    let mut session = DocumentSession::load("f1", bytes, DocKind::Json);
    session.apply(set("/model", "\"another-model\"")).unwrap();

    // Claude Code appends an allow rule while the draft is open.
    let disk = String::from_utf8(fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "[\"Bash(git status)\"]",
            "[\"Bash(git status)\", \"Bash(ls)\"]",
        );
    fs::write(&path, &disk).unwrap();

    // A14: the save is refused and the external bytes stay.
    let err = env.save("f1", &path, &session).unwrap_err();
    assert_eq!(err.code, ErrorCode::ExternalChangeConflict);
    assert_eq!(fs::read_to_string(&path).unwrap(), disk);

    // DD16/R16: no overlap → reapply builds a draft on the new bytes, writes nothing.
    let conflict = session.conflict(disk.as_bytes(), &|_| false);
    assert!(conflict.can_reapply, "{conflict:?}");
    session.reapply_onto(disk.clone().into_bytes()).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), disk);
    env.save("f1", &path, &session).unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"Bash(ls)\""), "disk change kept");
    assert!(saved.contains("\"another-model\""), "draft change kept");
}

#[test]
fn section_replace_overlapping_disk_change_cannot_reapply() {
    let env = Env::new();
    let path = env.put("settings.json", &fixture("claude-settings.json"));
    let bytes = fs::read(&path).unwrap();
    let mut session = DocumentSession::load("f1", bytes, DocKind::Json);
    session
        .apply(set("/permissions", "{\"allow\": []}"))
        .unwrap();
    let disk = String::from_utf8(fs::read(&path).unwrap())
        .unwrap()
        .replace("\"Read(.env)\"", "\"Read(.env)\", \"Read(.ssh)\"");
    let conflict = session.conflict(disk.as_bytes(), &|_| false);
    assert!(!conflict.can_reapply);
    assert!(session.reapply_onto(disk.into_bytes()).is_err());
}

#[test]
fn claude_preset_warns_but_allows_unknown_keys() {
    let presets = PresetRegistry::bundled();
    let env = Env::new();
    let dir = env.work.join(".claude");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    fs::write(&path, fixture("claude-settings.json")).unwrap();
    let preset = presets.detect(&path, None).expect("claude preset detected");
    let session = DocumentSession::load("f1", fs::read(&path).unwrap(), DocKind::Json);
    let value = session.to_value().unwrap();
    let schema = presets.schema(&preset).expect("bundled schema compiles");
    // A24: unknown vendor keys are not errors.
    assert!(
        schema
            .validate(&value)
            .iter()
            .all(|d| d.severity != Severity::Error)
    );
    // Wrong type on a known key is a warning, never an error (eng R2).
    let bad: serde_json::Value = serde_json::json!({ "permissions": 5 });
    let diags = schema.validate(&bad);
    assert!(!diags.is_empty());
    assert!(diags.iter().all(|d| d.severity == Severity::Warning));
}

#[test]
fn managed_scope_is_read_only_and_invalid_files_are_not_editable() {
    let env = Env::new();
    let (mut registry, _) = Registry::load(&env.config);
    let managed = env.put("managed-settings.json", b"{}\n");
    assert!(
        registry
            .add(&managed, None, Some(Scope::Managed), None)
            .unwrap()
            .read_only
    );

    for name in [
        "invalid-comment.json",
        "invalid-missing-comma.json",
        "duplicate-keys.json",
    ] {
        let session = DocumentSession::load("x", fixture(name), DocKind::Json);
        assert!(!session.is_editable(), "{name} must not be editable");
        assert!(
            session.snapshot(false).source_view.is_some(),
            "{name} shows the source view (DD4)"
        );
    }
    // The JSONC fixture is editable as .jsonc and keeps every other byte (A12).
    let src = fixture("commented.jsonc");
    let mut session = DocumentSession::load("j", src.clone(), DocKind::Jsonc);
    session.apply(set("/enabled", "false")).unwrap();
    let expected = String::from_utf8(src).unwrap().replacen("true", "false", 1);
    assert_eq!(session.candidate_bytes(), expected.as_bytes());
}
