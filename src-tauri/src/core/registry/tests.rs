use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::core::persistence::atomic_write;

struct Fx {
    dir: TempDir,
}

impl Fx {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }
    fn cfg(&self) -> PathBuf {
        self.dir.path().join("config")
    }
    fn data(&self) -> PathBuf {
        self.dir.path().join("data")
    }
    fn user_file(&self, name: &str) -> PathBuf {
        let p = self.dir.path().join("user").join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "{\"keep\":true}\n").unwrap();
        p
    }
    fn registry(&self) -> Registry {
        Registry::load(&self.cfg()).0
    }
}

#[test]
fn fresh_load_is_empty_with_default_preferences() {
    let fx = Fx::new();
    let (r, notices) = Registry::load(&fx.cfg());
    assert!(notices.is_empty());
    assert!(r.list().is_empty());
    assert_eq!(r.preferences(), Preferences::default());
}

#[test]
fn add_persists_and_reloads() {
    let fx = Fx::new();
    let f = fx.user_file("a.jsonc");
    let mut r = fx.registry();
    let e = r
        .add(&f, Some("  Mine ".into()), Some(Scope::User), None)
        .unwrap();
    assert_eq!(e.display_name, "Mine");
    assert_eq!(e.kind, DocKind::Jsonc);
    assert_eq!(e.entry_kind, EntryKind::File);
    assert_eq!(e.discovered_scope, Some(Scope::User));
    assert!(!e.read_only);
    assert_eq!(fx.registry().list(), vec![e.clone()]);
    assert_eq!(r.get(&e.id).unwrap(), e);
    assert_eq!(r.get("nope").unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn duplicate_add_returns_existing_entry_even_via_symlink() {
    let fx = Fx::new();
    let f = fx.user_file("a.json");
    let mut r = fx.registry();
    let first = r.add(&f, None, None, None).unwrap();
    let dotted = f.parent().unwrap().join(".").join("a.json");
    assert_eq!(r.add(&dotted, None, None, None).unwrap().id, first.id);
    #[cfg(unix)]
    {
        let link = f.parent().unwrap().join("link.json");
        std::os::unix::fs::symlink(&f, &link).unwrap();
        assert_eq!(r.add(&link, None, None, None).unwrap().id, first.id);
    }
    assert_eq!(r.list().len(), 1);
}

#[cfg(unix)]
#[test]
fn symlink_is_stored_as_registered() {
    let fx = Fx::new();
    let f = fx.user_file("real.json");
    let link = f.parent().unwrap().join("link.json");
    std::os::unix::fs::symlink(&f, &link).unwrap();
    let e = fx.registry().add(&link, None, None, None).unwrap();
    assert_eq!(PathBuf::from(&e.absolute_path), link);
}

#[test]
fn add_rejects_missing_and_directories() {
    let fx = Fx::new();
    let mut r = fx.registry();
    assert_eq!(
        r.add(&fx.dir.path().join("none.json"), None, None, None)
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        r.add(fx.dir.path(), None, None, None).unwrap_err().code,
        ErrorCode::NotARegularFile
    );
}

#[test]
fn a26_managed_scope_is_policy_read_only() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let managed = r
        .add(
            &fx.user_file("managed.json"),
            None,
            Some(Scope::Managed),
            None,
        )
        .unwrap();
    assert!(managed.read_only);
    assert_eq!(managed.discovered_scope, Some(Scope::Managed));
    let user = r
        .add(&fx.user_file("user.json"), None, Some(Scope::User), None)
        .unwrap();
    assert!(!user.read_only);
    assert!(fx.registry().get(&managed.id).unwrap().read_only);
}

#[test]
fn claude_preset_defaults_collection() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let e = r
        .add(
            &fx.user_file("c.json"),
            None,
            Some(Scope::User),
            Some("claude-code-settings".into()),
        )
        .unwrap();
    assert_eq!(e.collection_id.as_deref(), Some("claude-code"));
    assert_eq!(e.preset_id.as_deref(), Some("claude-code-settings"));
    let other = r
        .add(
            &fx.user_file("o.json"),
            None,
            None,
            Some("something-else".into()),
        )
        .unwrap();
    assert_eq!(other.collection_id, None);
    let none = r.add(&fx.user_file("n.json"), None, None, None).unwrap();
    assert_eq!(none.collection_id, None);
}

#[test]
fn create_new_writes_empty_object_and_refuses_overwrite() {
    let fx = Fx::new();
    let p = fx.dir.path().join("new.json");
    let mut r = fx.registry();
    let e = r.create_new(&p).unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"{}\n");
    assert_eq!(e.display_name, "new.json");
    fs::write(&p, "precious").unwrap();
    let err = r.create_new(&p).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidOperation);
    assert_eq!(fs::read(&p).unwrap(), b"precious");
    assert_eq!(r.list().len(), 1);
}

#[test]
fn kind_follows_extension_and_new_yaml_toml_files_start_empty() {
    for (name, kind) in [
        ("a.JSON", DocKind::Json),
        ("a.jsonc", DocKind::Jsonc),
        ("a.yaml", DocKind::Yaml),
        ("a.YML", DocKind::Yaml),
        ("a.toml", DocKind::Toml),
        ("Makefile", DocKind::Json),
    ] {
        assert_eq!(Registry::kind_for(Path::new(name)), kind, "{name}");
    }
    let fx = Fx::new();
    let mut r = fx.registry();
    for name in ["new.yaml", "new.toml"] {
        let p = fx.dir.path().join(name);
        assert!(r.create_new(&p).unwrap().kind != DocKind::Json);
        assert_eq!(fs::read(&p).unwrap(), b"", "{name}");
    }
}

#[test]
fn sample_create_unique_names_and_remove_deletes_only_copy() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let a = r.create_sample(&fx.data(), b"{\"s\":1}\n").unwrap();
    let b = r.create_sample(&fx.data(), b"{\"s\":1}\n").unwrap();
    assert_eq!(a.entry_kind, EntryKind::Sample);
    assert_eq!(a.display_name, "Sample: feature-flags.json");
    assert_eq!(a.collection_id.as_deref(), Some("sample"));
    assert_eq!(b.display_name, "Sample: feature-flags-2.json");
    let (pa, pb) = (
        PathBuf::from(&a.absolute_path),
        PathBuf::from(&b.absolute_path),
    );
    assert_eq!(fs::read(&pa).unwrap(), b"{\"s\":1}\n");
    assert!(pa.starts_with(fx.data().join("samples")));
    let user = fx.user_file("mine.json");
    r.add(&user, None, None, None).unwrap();
    r.remove(&a.id, &fx.data()).unwrap();
    assert!(!pa.exists());
    assert!(pb.exists());
    assert!(user.exists());
    assert_eq!(r.list().len(), 2);
}

#[test]
fn non_sample_removal_never_deletes_even_inside_data_dir() {
    let fx = Fx::new();
    let inside = fx.data().join("samples").join("looks-like-sample.json");
    fs::create_dir_all(inside.parent().unwrap()).unwrap();
    fs::write(&inside, "{}").unwrap();
    let outside = fx.user_file("u.json");
    let mut r = fx.registry();
    let e1 = r.add(&inside, None, None, None).unwrap();
    let e2 = r.add(&outside, None, None, None).unwrap();
    r.remove(&e1.id, &fx.data()).unwrap();
    r.remove(&e2.id, &fx.data()).unwrap();
    assert!(inside.exists() && outside.exists());
    assert!(r.list().is_empty());
    assert_eq!(
        r.remove(&e1.id, &fx.data()).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn tampered_sample_path_outside_data_dir_is_not_deleted() {
    let fx = Fx::new();
    let victim = fx.user_file("victim.json");
    let mut r = fx.registry();
    let s = r.create_sample(&fx.data(), b"{}").unwrap();
    // Tamper with the stored registry so the sample entry points at a user file.
    let reg_file = fx.cfg().join("registry.json");
    let text = fs::read_to_string(&reg_file)
        .unwrap()
        .replace(&s.absolute_path, &victim.to_string_lossy());
    fs::write(&reg_file, text).unwrap();
    let mut r = fx.registry();
    assert_eq!(r.get(&s.id).unwrap().entry_kind, EntryKind::Sample);
    r.remove(&s.id, &fx.data()).unwrap();
    assert!(victim.exists());
    assert!(r.list().is_empty());
}

#[cfg(unix)]
#[test]
fn sample_symlink_escaping_data_dir_is_not_deleted() {
    let fx = Fx::new();
    let victim = fx.user_file("victim.json");
    let dir = fx.data().join("samples");
    fs::create_dir_all(&dir).unwrap();
    let link = dir.join("feature-flags.json");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    let mut r = fx.registry();
    let mut s = r.create_sample(&fx.data(), b"{}").unwrap(); // gets -2 name since the link exists
    let reg_file = fx.cfg().join("registry.json");
    let text = fs::read_to_string(&reg_file)
        .unwrap()
        .replace(&s.absolute_path, &link.to_string_lossy());
    fs::write(&reg_file, text).unwrap();
    r = fx.registry();
    s = r.get(&s.id).unwrap();
    r.remove(&s.id, &fx.data()).unwrap();
    assert!(victim.exists());
}

#[test]
fn update_metadata_relocate_schema_confirmations_touch() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let e = r.add(&fx.user_file("a.json"), None, None, None).unwrap();
    let u = r
        .update_metadata(
            &e.id,
            MetadataPatch {
                display_name: Some("Renamed".into()),
                collection_id: Some("grp".into()),
            },
        )
        .unwrap();
    assert_eq!(
        (u.display_name.as_str(), u.collection_id.as_deref()),
        ("Renamed", Some("grp"))
    );
    assert_eq!(
        r.update_metadata(
            &e.id,
            MetadataPatch {
                display_name: Some("  ".into()),
                collection_id: None
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidOperation
    );
    assert_eq!(
        r.update_metadata(
            &e.id,
            MetadataPatch {
                display_name: None,
                collection_id: Some(String::new())
            }
        )
        .unwrap()
        .collection_id,
        None
    );

    let c = r.set_link_confirmed(&e.id, true, true).unwrap();
    assert!(c.link_write_confirmed && c.hardlink_write_confirmed);
    let other = fx.user_file("b.jsonc");
    let moved = r.relocate(&e.id, &other).unwrap();
    assert_eq!(moved.kind, DocKind::Jsonc);
    assert!(!moved.link_write_confirmed && !moved.hardlink_write_confirmed);
    let taken = r.add(&fx.user_file("c.json"), None, None, None).unwrap();
    assert_eq!(
        r.relocate(&taken.id, &other).unwrap_err().code,
        ErrorCode::InvalidOperation
    );

    let schema = fx.dir.path().join("s.schema.json");
    assert_eq!(
        r.set_schema(&e.id, Some(schema.clone()))
            .unwrap()
            .local_schema_path,
        Some(schema.to_string_lossy().into_owned())
    );
    assert_eq!(r.set_schema(&e.id, None).unwrap().local_schema_path, None);

    r.touch_opened(&e.id).unwrap();
    let ts = fx.registry().get(&e.id).unwrap().last_opened_at.unwrap();
    assert!(ts.len() == 20 && ts.ends_with('Z'), "{ts}");
    assert_eq!(
        r.touch_opened("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn relocating_a_sample_makes_it_a_plain_file() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let s = r.create_sample(&fx.data(), b"{}").unwrap();
    let dest = fx.user_file("elsewhere.json");
    let m = r.relocate(&s.id, &dest).unwrap();
    assert_eq!(m.entry_kind, EntryKind::File);
}

#[test]
fn preferences_roundtrip_and_validation() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let prefs = Preferences {
        backup_retention: 5,
        font_scale: 1.25,
        hide_secrets: false,
        ..Preferences::default()
    };
    r.set_preferences(prefs.clone()).unwrap();
    assert_eq!(fx.registry().preferences(), prefs);
    let bad = Preferences {
        backup_retention: 0,
        ..Preferences::default()
    };
    assert_eq!(
        r.set_preferences(bad).unwrap_err().code,
        ErrorCode::InvalidOperation
    );
    let bad = Preferences {
        font_scale: 3.0,
        ..Preferences::default()
    };
    assert_eq!(
        r.set_preferences(bad).unwrap_err().code,
        ErrorCode::InvalidOperation
    );
    assert_eq!(r.preferences(), prefs);
}

fn corrupt_asides(cfg: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(cfg)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("registry.corrupt-")
        })
        .collect()
}

#[test]
fn truncated_registry_recovers_from_bak_with_notice() {
    let fx = Fx::new();
    let user = fx.user_file("a.json");
    let mut r = fx.registry();
    r.add(&user, Some("first".into()), None, None).unwrap();
    let second = r
        .add(&fx.user_file("b.json"), Some("second".into()), None, None)
        .unwrap();
    // .bak now holds the registry with only "first"; truncate the main file.
    let reg = fx.cfg().join("registry.json");
    let full = fs::read(&reg).unwrap();
    fs::write(&reg, &full[..full.len() / 2]).unwrap();

    let (r2, notices) = Registry::load(&fx.cfg());
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].code, "registry_recovered");
    let asides = corrupt_asides(&fx.cfg());
    assert_eq!(asides.len(), 1);
    assert_eq!(fs::read(&asides[0]).unwrap(), &full[..full.len() / 2]);
    assert!(
        notices[0]
            .message
            .contains(&asides[0].display().to_string())
    );
    let names: Vec<_> = r2.list().into_iter().map(|e| e.display_name).collect();
    assert_eq!(names, ["first"]);
    assert!(r2.get(&second.id).is_err());
    // A valid main file exists again, and user files are untouched.
    let (_, again) = Registry::load(&fx.cfg());
    assert!(again.is_empty());
    assert_eq!(fs::read(&user).unwrap(), b"{\"keep\":true}\n");
}

#[test]
fn truncated_registry_without_bak_starts_empty_with_notice() {
    let fx = Fx::new();
    let user = fx.user_file("a.json");
    fs::create_dir_all(fx.cfg()).unwrap();
    fs::write(fx.cfg().join("registry.json"), "{\"version\":1,\"entri").unwrap();
    let (mut r, notices) = Registry::load(&fx.cfg());
    assert!(r.list().is_empty());
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].code, "registry_reset");
    assert_eq!(corrupt_asides(&fx.cfg()).len(), 1);
    assert_eq!(fs::read(&user).unwrap(), b"{\"keep\":true}\n");
    // Usable afterwards.
    r.add(&user, None, None, None).unwrap();
    assert_eq!(fx.registry().list().len(), 1);
}

#[test]
fn corrupt_main_and_corrupt_bak_starts_empty() {
    let fx = Fx::new();
    fs::create_dir_all(fx.cfg()).unwrap();
    fs::write(fx.cfg().join("registry.json"), "garbage").unwrap();
    fs::write(fx.cfg().join("registry.json.bak"), "also garbage").unwrap();
    let (r, notices) = Registry::load(&fx.cfg());
    assert!(r.list().is_empty());
    assert_eq!(notices[0].code, "registry_reset");
}

#[test]
fn missing_main_with_good_bak_is_recovered() {
    let fx = Fx::new();
    let mut r = fx.registry();
    r.add(&fx.user_file("a.json"), None, None, None).unwrap();
    r.set_preferences(Preferences {
        backup_retention: 7,
        ..Preferences::default()
    })
    .unwrap();
    fs::remove_file(fx.cfg().join("registry.json")).unwrap();
    let (r2, notices) = Registry::load(&fx.cfg());
    assert_eq!(r2.list().len(), 1);
    assert_eq!(notices.len(), 1);
    assert!(corrupt_asides(&fx.cfg()).is_empty());
    assert!(fx.cfg().join("registry.json").exists());
}

#[test]
fn failed_write_does_not_change_in_memory_state() {
    let fx = Fx::new();
    let mut r = fx.registry();
    let f = fx.user_file("a.json");
    // Make the config dir unusable: replace it with a regular file.
    fs::write(fx.cfg(), "block").unwrap();
    assert!(r.add(&f, None, None, None).is_err());
    assert!(r.list().is_empty());
    let _ = atomic_write; // keep import used if tests above change
}
