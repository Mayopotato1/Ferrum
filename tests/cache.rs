use spotlight_prototype::{
    actions,
    cache::{self, SearchSnapshot},
    config::Config,
    model::{Action, Item},
    search::Index,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ferrum-cache-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> PathBuf {
        self.0.join("cache.json")
    }
    fn item(&self) -> Item {
        Item::new(
            "Example document",
            "File",
            "fixture",
            Action::Document(self.0.join("example.txt")),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_file(self.0.join("example.txt"));
        let _ = std::fs::remove_dir(&self.0);
    }
}
#[test]
fn valid_missing_stale_and_irrelevant_settings() {
    let f = Fixture::new();
    let c = Config::default();
    let cancel = AtomicBool::new(false);
    assert!(cache::load(&f.path(), &c, &cancel).unwrap().is_none());
    std::fs::write(f.0.join("example.txt"), "fixture").unwrap();
    cache::save(&f.path(), &c, &[f.item()], &cancel).unwrap();
    let items = cache::load(&f.path(), &c, &cancel).unwrap().unwrap();
    assert_eq!(items.len(), 1);
    assert!(actions::prepare(&items[0]).is_ok());
    std::fs::remove_file(f.0.join("example.txt")).unwrap();
    assert!(actions::prepare(&items[0]).is_err());
    let mut changed = c.clone();
    changed.background_opacity = 85;
    assert!(cache::load(&f.path(), &changed, &cancel).unwrap().is_some());
    changed.max_depth += 1;
    assert!(cache::load(&f.path(), &changed, &cancel).is_err());
}
#[test]
fn corrupt_version_expired_and_untrusted_actions_fail_safely() {
    let f = Fixture::new();
    let c = Config::default();
    let cancel = AtomicBool::new(false);
    cache::save(&f.path(), &c, &[f.item()], &cancel).unwrap();
    let original = std::fs::read(f.path()).unwrap();
    for (field, value) in [
        ("schema", serde_json::json!(999)),
        ("application", serde_json::json!("other")),
        ("created", serde_json::json!(0)),
        ("checksum", serde_json::json!(0)),
    ] {
        let mut data: serde_json::Value = serde_json::from_slice(&original).unwrap();
        data[field] = value;
        std::fs::write(f.path(), serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(cache::load(&f.path(), &c, &cancel).is_err());
    }
    for bytes in [b"{partial".as_slice(), b"[]", b"null", b"\xff\0"] {
        std::fs::write(f.path(), bytes).unwrap();
        assert!(cache::load(&f.path(), &c, &cancel).is_err());
    }
    let web = Item::new(
        "malicious",
        "Website",
        "fixture",
        Action::OpenUrl("file:///C:/test.cmd".into()),
    );
    assert!(cache::save(&f.path(), &c, &[web], &cancel).is_err());
    let remote = Item::new(
        "remote",
        "Folder",
        "fixture",
        Action::Folder(PathBuf::from(r"\\server\share")),
    );
    assert!(cache::save(&f.path(), &c, &[remote], &cancel).is_err());
}
#[test]
fn atomic_failure_and_cancel_preserve_previous_snapshot() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let c = Config::default();
    let cancel = AtomicBool::new(false);
    cache::save(&f.path(), &c, &[f.item()], &cancel).unwrap();
    let before = std::fs::read(f.path()).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(f.path())
        .unwrap();
    assert!(cache::save(&f.path(), &c, &[], &cancel).is_err());
    drop(lock);
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
    cancel.store(true, Ordering::Relaxed);
    assert!(!cache::save(&f.path(), &c, &[], &cancel).unwrap());
    assert!(cache::load(&f.path(), &c, &cancel).unwrap().is_none());
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
    assert_eq!(std::fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn refresh_publication_retains_readers_and_shutdown_blocks_replacement() {
    let cancel = AtomicBool::new(false);
    let old = Arc::new(SearchSnapshot {
        index: Index::new(vec![]),
        status: "cached".into(),
        cached: true,
    });
    let slot = Mutex::new(old.clone());
    for _ in 0..32 {
        let next = Arc::new(SearchSnapshot {
            index: Index::new(vec![]),
            status: "fresh".into(),
            cached: false,
        });
        assert!(cache::publish(&slot, next.clone(), &cancel));
        assert!(Arc::ptr_eq(&slot.lock().unwrap(), &next));
    }
    assert!(old.cached);
    cancel.store(true, Ordering::Relaxed);
    assert!(!cache::publish(&slot, old, &cancel));
    assert!(!slot.lock().unwrap().cached);
}
#[test]
fn oversize_files_are_rejected_before_reading_contents() {
    let f = Fixture::new();
    let file = std::fs::File::create(f.path()).unwrap();
    file.set_len(cache::MAX_BYTES as u64 + 1).unwrap();
    drop(file);
    assert!(cache::load(&f.path(), &Config::default(), &AtomicBool::new(false)).is_err());
}
#[test]
fn launch_arguments_and_profiles_are_not_persisted() {
    let f = Fixture::new();
    let cancel = AtomicBool::new(false);
    let c = Config::default();
    let sensitive = Item::new(
        "Parameterized app",
        "Windows Application",
        "fixture",
        Action::Shortcut {
            path: f.0.join("app.lnk"),
            target: f.0.join("app.exe").to_string_lossy().into(),
            arguments: "--token=FAKE_TEST_SECRET".into(),
            working_dir: String::new(),
        },
    );
    let profile = Item::new(
        "Profile",
        "Modrinth Profile",
        "fixture",
        Action::Unavailable("test".into()),
    );
    cache::save(&f.path(), &c, &[sensitive, profile, f.item()], &cancel).unwrap();
    let bytes = std::fs::read(f.path()).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("FAKE_TEST_SECRET"));
    assert_eq!(
        cache::load(&f.path(), &c, &cancel).unwrap().unwrap().len(),
        1
    );
}
