use spotlight_prototype::{
    config,
    preferences::{Change, Document},
    web::SearchProvider,
};
#[test]
fn product_and_binary_are_ferrum_without_breaking_legacy_identity() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("name = \"ferrum\""));
    assert!(include_str!("../src/main.rs").contains("env!(\"CARGO_PKG_VERSION\")"));
    assert!(include_str!("../src/main.rs").contains("Ferrum {} (headless)"));
    assert!(include_str!("../src/ui/launcher.slint").contains("title: \"Ferrum\""));
    let settings = include_str!("../src/ui/settings.slint");
    assert!(settings.contains("Ferrum Settings"));
    assert!(!settings.contains("Spotlight Launcher"));
    assert!(include_str!("../resources/ferrum.rc").contains("\"OriginalFilename\", \"ferrum.exe"));
    assert_eq!(
        spotlight_prototype::instance::IDENTITY,
        "Local\\SpotlightLauncherPrototype.35C83E18-76F3-4F51-A9C6-2E8C071833B5"
    );
}
#[test]
fn retained_config_path_preserves_legacy_values_and_does_not_overwrite_another_location() {
    let root = std::env::temp_dir().join(format!("ferrum-brand-test-{}", std::process::id()));
    std::fs::create_dir_all(root.join("Ferrum")).unwrap();
    let other = root.join("Ferrum/config.json");
    std::fs::write(&other, "{\"unrelated\":true}").unwrap();
    let path = config::path_in(&root);
    assert!(path.ends_with("SpotlightLauncherPrototype/config.json"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = r#"{"search_provider":"brave","hotkey":"Ctrl+Shift+Space","background_opacity":89,"launch_on_startup":true,"websites":{"gh":"https://github.com/search?q={query}"}}"#;
    std::fs::write(&path, old).unwrap();
    let mut doc = Document::open(path.clone()).unwrap();
    assert_eq!(doc.config.search_provider, SearchProvider::Brave);
    assert_eq!(doc.config.hotkey, "Ctrl+Shift+Space");
    assert_eq!(doc.config.background_opacity, 89);
    assert!(doc.config.launch_on_startup);
    assert!(doc.config.websites.contains_key("gh"));
    doc.update(Change::Opacity(90)).unwrap();
    assert!(
        Document::open(path.clone())
            .unwrap()
            .config
            .launch_on_startup
    );
    assert_eq!(
        std::fs::read_to_string(&other).unwrap(),
        "{\"unrelated\":true}"
    );
    std::fs::write(&path, "{broken").unwrap();
    assert!(Document::open(path.clone()).is_err());
    assert!(doc.update(Change::Opacity(92)).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(other).unwrap();
    std::fs::remove_dir(root.join("Ferrum")).unwrap();
    std::fs::remove_dir(root.join("SpotlightLauncherPrototype")).unwrap();
    std::fs::remove_dir(root).unwrap();
}
