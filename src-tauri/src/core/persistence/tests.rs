use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::core::MAX_EDITABLE_BYTES;

struct Fx {
    dir: TempDir,
    backups: BackupStore,
}

impl Fx {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let backups = BackupStore::new(dir.path().join("backups"), 20);
        Self { dir, backups }
    }

    fn work(&self) -> PathBuf {
        let w = self.dir.path().join("work");
        fs::create_dir_all(&w).unwrap();
        w
    }

    fn file(&self, name: &str, content: &str) -> PathBuf {
        let p = self.work().join(name);
        fs::write(&p, content).unwrap();
        p
    }

    fn save(
        &self,
        path: &Path,
        expected: &str,
        candidate: &str,
        conf: LinkConfirmations,
    ) -> CoreResult<SaveOutcome> {
        let expected = fingerprint(expected.as_bytes());
        save(&SaveRequest {
            file_id: "fid",
            path,
            expected: &expected,
            candidate: candidate.as_bytes(),
            confirmations: conf,
            validate: &|_| Ok(()),
            backups: &self.backups,
            change_count: Some(1),
        })
    }
}

const NONE: LinkConfirmations = LinkConfirmations {
    symlink: false,
    hardlink: false,
};

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

fn assert_no_temp(dir: &Path) {
    assert!(
        names(dir).iter().all(|n| !n.contains(".configdeck-")),
        "temp left behind: {:?}",
        names(dir)
    );
}

#[test]
fn a13_reads_write_nothing_and_make_no_backup() {
    let fx = Fx::new();
    let p = fx.file("a.json", "{\"a\":1}\n");
    let before = names(&fx.work());
    let bytes = read_checked(&p, MAX_EDITABLE_BYTES).unwrap();
    assert_eq!(bytes, b"{\"a\":1}\n");
    assert_eq!(names(&fx.work()), before);
    assert_eq!(fs::read(&p).unwrap(), bytes);
    assert!(fx.backups.list("fid").unwrap().is_empty());
    assert!(!fx.backups.root().exists());
}

#[test]
fn read_checked_enforces_size_limit() {
    let fx = Fx::new();
    let p = fx.file("big.json", &"x".repeat(100));
    assert_eq!(
        read_checked(&p, 99).unwrap_err().code,
        ErrorCode::FileTooLarge
    );
    assert_eq!(read_checked(&p, 100).unwrap().len(), 100);
}

#[test]
fn read_checked_rejects_directory() {
    let fx = Fx::new();
    assert_eq!(
        read_checked(&fx.work(), 10).unwrap_err().code,
        ErrorCode::NotARegularFile
    );
}

#[test]
fn a16_save_creates_backup_and_verified_content() {
    let fx = Fx::new();
    let p = fx.file("a.json", "{\"a\":1}\n");
    let out = fx.save(&p, "{\"a\":1}\n", "{\"a\":2}\n", NONE).unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"{\"a\":2}\n");
    assert_eq!(out.fingerprint, fingerprint(b"{\"a\":2}\n"));
    assert_eq!(
        fx.backups.read("fid", &out.backup_id).unwrap(),
        b"{\"a\":1}\n"
    );
    let list = fx.backups.list("fid").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].change_count, Some(1));
    assert_no_temp(&fx.work());
}

#[test]
fn a14_external_change_conflicts_and_source_unchanged() {
    let fx = Fx::new();
    let p = fx.file("a.json", "{\"a\":1}\n");
    fs::write(&p, "{\"a\":99}\n").unwrap(); // external edit after load
    let e = fx.save(&p, "{\"a\":1}\n", "{\"a\":2}\n", NONE).unwrap_err();
    assert_eq!(e.code, ErrorCode::ExternalChangeConflict);
    assert_eq!(fs::read(&p).unwrap(), b"{\"a\":99}\n");
    assert!(fx.backups.list("fid").unwrap().is_empty());
    assert_no_temp(&fx.work());
}

#[test]
fn change_between_temp_and_replace_aborts_and_cleans_up() {
    let fx = Fx::new();
    let p = fx.file("a.json", "{\"a\":1}\n");
    let p2 = p.clone();
    test_hooks::set_after_temp(move || fs::write(&p2, "{\"a\":99}\n").unwrap());
    let e = fx.save(&p, "{\"a\":1}\n", "{\"a\":2}\n", NONE).unwrap_err();
    assert_eq!(e.code, ErrorCode::ExternalChangeConflict);
    assert_eq!(fs::read(&p).unwrap(), b"{\"a\":99}\n");
    assert_no_temp(&fx.work());
    assert!(
        fx.backups.list("fid").unwrap().is_empty(),
        "backup of an unsaved change must be dropped"
    );
}

#[test]
fn a17_restore_is_a_save_of_backup_bytes() {
    let fx = Fx::new();
    let p = fx.file("a.json", "v1\n");
    let first = fx.save(&p, "v1\n", "v2\n", NONE).unwrap();
    let old = fx.backups.read("fid", &first.backup_id).unwrap();
    assert_eq!(old, b"v1\n");
    let out = fx
        .save(&p, "v2\n", std::str::from_utf8(&old).unwrap(), NONE)
        .unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"v1\n");
    assert_eq!(fx.backups.read("fid", &out.backup_id).unwrap(), b"v2\n");
    assert_eq!(fx.backups.list("fid").unwrap().len(), 2);
    // Restore goes through the same conflict check.
    fs::write(&p, "external\n").unwrap();
    let e = fx.save(&p, "v1\n", "v1\n", NONE).unwrap_err();
    assert_eq!(e.code, ErrorCode::ExternalChangeConflict);
}

#[test]
fn a18_backup_failure_means_no_write() {
    let fx = Fx::new();
    let p = fx.file("a.json", "orig\n");
    let blocker = fx.dir.path().join("blocker");
    fs::write(&blocker, "x").unwrap();
    let backups = BackupStore::new(blocker.join("sub"), 20);
    let expected = fingerprint(b"orig\n");
    let e = save(&SaveRequest {
        file_id: "fid",
        path: &p,
        expected: &expected,
        candidate: b"new\n",
        confirmations: NONE,
        validate: &|_| Ok(()),
        backups: &backups,
        change_count: None,
    })
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::BackupFailed);
    assert_eq!(fs::read(&p).unwrap(), b"orig\n");
    assert_no_temp(&fx.work());
}

#[test]
fn validation_failure_writes_nothing() {
    let fx = Fx::new();
    let p = fx.file("a.json", "orig\n");
    let expected = fingerprint(b"orig\n");
    let e = save(&SaveRequest {
        file_id: "fid",
        path: &p,
        expected: &expected,
        candidate: b"bad",
        confirmations: NONE,
        validate: &|_| Err(CoreError::new(ErrorCode::InvalidJson, "nope")),
        backups: &fx.backups,
        change_count: None,
    })
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidJson);
    assert_eq!(fs::read(&p).unwrap(), b"orig\n");
    assert!(fx.backups.list("fid").unwrap().is_empty());
}

#[test]
fn a19_replace_failure_leaves_source_intact_and_no_temp() {
    let fx = Fx::new();
    let p = fx.file("a.json", "orig\n");
    test_hooks::set_fail_replace(true);
    let e = fx.save(&p, "orig\n", "new\n", NONE).unwrap_err();
    test_hooks::set_fail_replace(false);
    assert_eq!(e.code, ErrorCode::AtomicWriteFailed);
    assert_eq!(fs::read(&p).unwrap(), b"orig\n");
    assert_no_temp(&fx.work());
    assert!(fx.backups.list("fid").unwrap().is_empty());
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    fn mode(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a21_symlink_unconfirmed_is_refused() {
        let fx = Fx::new();
        let real = fx.file("real.json", "v1\n");
        let link = fx.work().join("link.json");
        symlink(&real, &link).unwrap();
        let e = fx.save(&link, "v1\n", "v2\n", NONE).unwrap_err();
        assert_eq!(e.code, ErrorCode::SymlinkRequiresConfirmation);
        assert_eq!(fs::read(&real).unwrap(), b"v1\n");
        assert!(fx.backups.list("fid").unwrap().is_empty());
    }

    #[test]
    fn a21_symlink_confirmed_writes_through_and_link_survives() {
        let fx = Fx::new();
        let real = fx.file("real.json", "v1\n");
        let link = fx.work().join("link.json");
        symlink(&real, &link).unwrap();
        let conf = LinkConfirmations {
            symlink: true,
            hardlink: false,
        };
        fx.save(&link, "v1\n", "v2\n", conf).unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_link(&link).unwrap(), real);
        assert_eq!(fs::read(&real).unwrap(), b"v2\n");
        assert_no_temp(&fx.work());
    }

    #[test]
    fn a21_symlink_target_in_other_directory_gets_temp_there() {
        let fx = Fx::new();
        let other = fx.dir.path().join("dotfiles");
        fs::create_dir_all(&other).unwrap();
        let real = other.join("settings.json");
        fs::write(&real, "v1\n").unwrap();
        let link = fx.work().join("settings.json");
        symlink(&real, &link).unwrap();
        fx.save(
            &link,
            "v1\n",
            "v2\n",
            LinkConfirmations {
                symlink: true,
                hardlink: false,
            },
        )
        .unwrap();
        assert_eq!(fs::read(&real).unwrap(), b"v2\n");
        assert_eq!(names(&fx.work()), ["settings.json"]);
        assert_eq!(names(&other), ["settings.json"]);
    }

    #[test]
    fn a21_link_retargeted_during_draft_conflicts() {
        let fx = Fx::new();
        let a = fx.file("a.json", "A\n");
        let b = fx.file("b.json", "B\n");
        let link = fx.work().join("link.json");
        symlink(&a, &link).unwrap();
        let conf = LinkConfirmations {
            symlink: true,
            hardlink: false,
        };
        fs::remove_file(&link).unwrap();
        symlink(&b, &link).unwrap(); // retargeted after the draft loaded "A"
        let e = fx.save(&link, "A\n", "A2\n", conf).unwrap_err();
        assert_eq!(e.code, ErrorCode::ExternalChangeConflict);
        assert_eq!(fs::read(&a).unwrap(), b"A\n");
        assert_eq!(fs::read(&b).unwrap(), b"B\n");
    }

    #[test]
    fn a21_link_retargeted_to_identical_bytes_during_save_conflicts() {
        let fx = Fx::new();
        let a = fx.file("a.json", "same\n");
        let b = fx.file("b.json", "same\n");
        let link = fx.work().join("link.json");
        symlink(&a, &link).unwrap();
        let (l2, b2) = (link.clone(), b.clone());
        test_hooks::set_after_temp(move || {
            fs::remove_file(&l2).unwrap();
            symlink(&b2, &l2).unwrap();
        });
        let conf = LinkConfirmations {
            symlink: true,
            hardlink: false,
        };
        let e = fx.save(&link, "same\n", "new\n", conf).unwrap_err();
        assert_eq!(e.code, ErrorCode::ExternalChangeConflict);
        assert_eq!(fs::read(&a).unwrap(), b"same\n");
        assert_eq!(fs::read(&b).unwrap(), b"same\n");
        assert_no_temp(&fx.work());
    }

    #[test]
    fn hardlink_needs_confirmation_then_detaches() {
        let fx = Fx::new();
        let a = fx.file("a.json", "v1\n");
        let b = fx.work().join("b.json");
        fs::hard_link(&a, &b).unwrap();
        let e = fx.save(&a, "v1\n", "v2\n", NONE).unwrap_err();
        assert_eq!(e.code, ErrorCode::HardlinkRequiresConfirmation);
        assert_eq!(fs::read(&a).unwrap(), b"v1\n");
        fx.save(
            &a,
            "v1\n",
            "v2\n",
            LinkConfirmations {
                symlink: false,
                hardlink: true,
            },
        )
        .unwrap();
        assert_eq!(fs::read(&a).unwrap(), b"v2\n");
        assert_eq!(fs::read(&b).unwrap(), b"v1\n");
    }

    #[test]
    fn mode_bits_preserved_never_broadened() {
        let fx = Fx::new();
        for m in [0o600, 0o640, 0o644] {
            let p = fx.file(&format!("m{m:o}.json"), "v1\n");
            fs::set_permissions(&p, fs::Permissions::from_mode(m)).unwrap();
            fx.save(&p, "v1\n", "v2\n", NONE).unwrap();
            assert_eq!(mode(&p), m);
        }
    }

    #[test]
    fn read_only_file_is_refused_and_not_chmodded() {
        let fx = Fx::new();
        let p = fx.file("ro.json", "v1\n");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o444)).unwrap();
        let e = fx.save(&p, "v1\n", "v2\n", NONE).unwrap_err();
        assert_eq!(e.code, ErrorCode::ReadOnly);
        assert_eq!(mode(&p), 0o444);
        assert_eq!(fs::read(&p).unwrap(), b"v1\n");
        assert!(fx.backups.list("fid").unwrap().is_empty());
    }

    #[test]
    fn save_rejects_non_regular_file() {
        let fx = Fx::new();
        let e = fx.save(&fx.work(), "", "x", NONE).unwrap_err();
        assert_eq!(e.code, ErrorCode::NotARegularFile);
    }
}

#[test]
fn retention_pruned_per_file_on_save() {
    let fx = Fx::new();
    let backups = BackupStore::new(fx.dir.path().join("b2"), 2);
    let p = fx.file("a.json", "v0\n");
    let mut cur = "v0\n".to_string();
    for i in 1..=4 {
        let next = format!("v{i}\n");
        let expected = fingerprint(cur.as_bytes());
        save(&SaveRequest {
            file_id: "fid",
            path: &p,
            expected: &expected,
            candidate: next.as_bytes(),
            confirmations: NONE,
            validate: &|_| Ok(()),
            backups: &backups,
            change_count: None,
        })
        .unwrap();
        cur = next;
    }
    let list = backups.list("fid").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(backups.read("fid", &list[0].id).unwrap(), b"v3\n");
}

#[test]
fn leftovers_match_exact_pattern_only() {
    let fx = Fx::new();
    let p = fx.file("settings.json", "{}\n");
    let w = fx.work();
    let mine = [
        ".settings.json.configdeck-abc123.tmp",
        ".settings.json.configdeck-0f0f0f0f0f0f.tmp",
    ];
    for n in mine {
        fs::write(w.join(n), "x").unwrap();
    }
    let unrelated = [
        ".other.json.configdeck-abc123.tmp",
        ".settings.json.configdeck-.tmp",
        ".settings.json.configdeck-abc.tmp.keep",
        "settings.json.configdeck-abc.tmp",
        ".settings.json.tmp",
        ".settings.json.configdeck-a-b.tmp",
        ".settings.jsonX.configdeck-abc.tmp",
        ".env",
        "settings.json.bak",
    ];
    for n in unrelated {
        fs::write(w.join(n), "x").unwrap();
    }
    fs::create_dir(w.join(".settings.json.configdeck-dir.tmp")).unwrap();
    let found: Vec<String> = find_leftovers(&p)
        .iter()
        .map(|f| f.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    let mut want: Vec<String> = mine.iter().map(ToString::to_string).collect();
    want.sort();
    assert_eq!(found, want);
    // Finding is read-only.
    assert!(w.join(mine[0]).exists());
    assert_eq!(remove_leftovers(&p).unwrap(), 2);
    assert!(find_leftovers(&p).is_empty());
    for n in unrelated {
        assert!(w.join(n).exists(), "{n} must survive");
    }
    assert!(w.join(".settings.json.configdeck-dir.tmp").is_dir());
    assert!(p.exists());
}

#[test]
fn atomic_write_creates_dirs_and_keeps_bak() {
    let fx = Fx::new();
    let p = fx.dir.path().join("cfg").join("sub").join("registry.json");
    atomic_write(&p, b"one", true).unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"one");
    assert!(!bak_path(&p).exists());
    atomic_write(&p, b"two", true).unwrap();
    atomic_write(&p, b"three", true).unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"three");
    assert_eq!(fs::read(bak_path(&p)).unwrap(), b"two");
    assert_no_temp(p.parent().unwrap());
    atomic_write(&p, b"four", false).unwrap();
    assert_eq!(fs::read(bak_path(&p)).unwrap(), b"two");
}

#[test]
fn atomic_write_failure_leaves_previous_file() {
    let fx = Fx::new();
    let p = fx.dir.path().join("r.json");
    atomic_write(&p, b"good", false).unwrap();
    test_hooks::set_fail_replace(true);
    let e = atomic_write(&p, b"bad", false).unwrap_err();
    test_hooks::set_fail_replace(false);
    assert_eq!(e.code, ErrorCode::AtomicWriteFailed);
    assert_eq!(fs::read(&p).unwrap(), b"good");
    assert_no_temp(fx.dir.path());
}

#[test]
fn fingerprint_is_sha256_hex() {
    assert_eq!(
        fingerprint(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
