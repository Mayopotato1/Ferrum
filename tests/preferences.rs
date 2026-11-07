use spotlight_prototype::{
    config::Config,
    hotkeys::Key,
    preferences::{Change, Document},
    web::SearchProvider,
};
use std::path::PathBuf;
struct Fixture(PathBuf);
impl Fixture {
    fn new(text: Option<&str>) -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "spotlight-settings-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.json");
        if let Some(t) = text {
            std::fs::write(&p, t).unwrap();
        }
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_dir(self.0.parent().unwrap());
    }
}
#[test]
fn old_configuration_migrates_fieldwise_without_losing_custom_data() {
    let f = Fixture::new(Some(
        r#"{"search_provider":"bing","hotkey":"Ctrl+Alt+Space","translucent_background":false,"websites":{"gh":"https://github.com/search?q={query}"},"future":{"keep":true},"max_depth":8}"#,
    ));
    let mut doc = Document::open(f.0.clone()).unwrap();
    assert_eq!(doc.config.background_opacity, 100);
    assert!(!doc.config.launch_on_startup);
    assert_eq!(doc.config.hotkey, "Ctrl+Alt+Space");
    assert_eq!(doc.config.search_provider, SearchProvider::Bing);
    doc.update(Change::Opacity(90)).unwrap();
    doc.update(Change::Hotkey(Key::AltShift)).unwrap();
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&f.0).unwrap()).unwrap();
    assert_eq!(raw["future"]["keep"], true);
    assert_eq!(raw["max_depth"], 8);
    assert!(raw["websites"]["gh"].is_string());
    let saved = Document::open(f.0.clone()).unwrap().config;
    assert_eq!(saved.background_opacity, 90);
    assert!(saved.translucent_background);
    assert_eq!(saved.hotkey, "Alt+Shift+Space");
}
#[test]
fn missing_invalid_and_partial_documents_are_safe() {
    let f = Fixture::new(None);
    let d = Document::open(f.0.clone()).unwrap();
    assert_eq!(d.config.background_opacity, 92);
    assert!(!d.config.launch_on_startup);
    assert!(!f.0.exists());
    std::fs::write(&f.0,r#"{"search_provider":"invalid","hotkey":"Ctrl+Q","background_opacity":2,"websites":{"gh":"https://github.com/?q={query}"},"max_files":"bad"}"#).unwrap();
    let mut d = Document::open(f.0.clone()).unwrap();
    assert_eq!(d.warnings.len(), 4);
    assert_eq!(d.config.search_provider, SearchProvider::Google);
    assert_eq!(d.config.hotkey, "Alt+Space");
    assert_eq!(d.config.background_opacity, 92);
    assert!(d.config.websites.contains_key("gh"));
    assert!(d.update(Change::Opacity(84)).is_err());
    assert!(d.update(Change::Opacity(101)).is_err());
    d.update(Change::Provider(SearchProvider::Brave)).unwrap();
    assert_eq!(
        Document::open(f.0.clone()).unwrap().config.search_provider,
        SearchProvider::Brave
    );
    std::fs::write(&f.0, "{\"websites\":").unwrap();
    assert!(Document::open(f.0.clone()).is_err());
    assert!(d.update(Change::Opacity(95)).is_err());
    assert_eq!(std::fs::read_to_string(&f.0).unwrap(), "{\"websites\":");
}
#[test]
fn save_failure_and_external_edits_preserve_preferences() {
    let f = Fixture::new(Some("{}"));
    let mut d = Document::open(f.0.clone()).unwrap();
    std::fs::write(&f.0, "{\"unknown\":1}").unwrap();
    assert!(d.update(Change::Provider(SearchProvider::Bing)).is_err());
    assert_eq!(d.config.search_provider, SearchProvider::Google);
    std::fs::remove_file(&f.0).unwrap();
    std::fs::create_dir(&f.0).unwrap();
    assert!(d.update(Change::Opacity(99)).is_err());
    std::fs::remove_dir(&f.0).unwrap();
}
#[test]
fn every_provider_persists_and_at_commands_remain_independent() {
    use spotlight_prototype::{
        composition,
        model::Action,
        query::{Aliases, Selection},
        search::Index,
    };
    let f = Fixture::new(None);
    let mut d = Document::open(f.0.clone()).unwrap();
    for (p, host) in [
        (SearchProvider::Google, "www.google.com"),
        (SearchProvider::DuckDuckGo, "duckduckgo.com"),
        (SearchProvider::Bing, "www.bing.com"),
        (SearchProvider::Brave, "search.brave.com"),
    ] {
        d.update(Change::Provider(p)).unwrap();
        let c = Document::open(f.0.clone()).unwrap().config;
        assert_eq!(c.search_provider, p);
        let aliases = Aliases::new(&c);
        let index = Index::new(vec![]);
        for q in ["hello world", "? hello world"] {
            let rows = composition::results(&index, &aliases, c.search_provider, q).unwrap();
            let Selection::Launch(item) = &rows[0] else {
                panic!()
            };
            let Action::WebSearch(url) = &item.action else {
                panic!()
            };
            assert_eq!(url::Url::parse(url).unwrap().host_str(), Some(host));
        }
        let composition::SubmitPlan::Execute(item) =
            composition::submit("hello world", true, c.search_provider).unwrap()
        else {
            panic!()
        };
        let Action::WebSearch(url) = item.action else {
            panic!()
        };
        assert_eq!(url::Url::parse(&url).unwrap().host_str(), Some(host));
        for (q, host) in [
            ("@yt minecraft", "www.youtube.com"),
            ("@wiki black holes", "en.wikipedia.org"),
        ] {
            let rows = composition::results(&index, &aliases, p, q).unwrap();
            let Selection::Launch(item) = &rows[0] else {
                panic!()
            };
            let Action::Website(url) = &item.action else {
                panic!()
            };
            assert_eq!(url::Url::parse(url).unwrap().host_str(), Some(host));
        }
    }
    assert_eq!(Config::default().background_opacity, 92);
}
#[test]
fn atomic_replace_failure_preserves_original_and_removes_its_temporary_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new(Some("{}"));
    let mut document = Document::open(fixture.0.clone()).unwrap();
    // Permit read, deny replace/delete on this isolated fixture only.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&fixture.0)
        .unwrap();
    assert!(document.update(Change::Opacity(85)).is_err());
    assert_eq!(document.config.background_opacity, 92);
    assert_eq!(std::fs::read_to_string(&fixture.0).unwrap(), "{}");
    assert_eq!(
        std::fs::read_dir(fixture.0.parent().unwrap())
            .unwrap()
            .count(),
        1
    );
    drop(lock);
    for value in 85..=100 {
        document.update(Change::Opacity(value)).unwrap();
        assert_eq!(
            Document::open(fixture.0.clone())
                .unwrap()
                .config
                .background_opacity,
            value
        );
    }
}
