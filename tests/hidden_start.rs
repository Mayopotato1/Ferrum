//! Source/state contracts only: these never initialize Slint or control a window.
#[test]
fn startup_has_no_show_focus_or_animation_and_instance_precedes_ui() {
    let app = include_str!("../src/app.rs");
    let start = app
        .split("let result = (|| -> Result<()> {")
        .nth(1)
        .unwrap()
        .split("cancel.store")
        .next()
        .unwrap();
    assert!(!start.contains("window::show"));
    assert!(!start.contains("invoke_focus_search"));
    assert!(!start.contains("invoke_start_opening"));
    assert!(start.contains("if open_settings"));
    assert!(start.contains("invoke_settings"));
    let main = include_str!("../src/main.rs");
    let branch = main
        .split("None | Some(\"--settings\") =>")
        .nth(1)
        .unwrap()
        .split("Some(\"--version\")")
        .next()
        .unwrap();
    assert!(branch.find("Instance::acquire").unwrap() < branch.find("app::run").unwrap());
    let native = include_str!("../src/ui/window.rs");
    assert!(native.contains(".with_visible(false)"));
    assert!(native.contains(".with_skip_taskbar(true)"));
    let ui = include_str!("../src/ui/launcher.slint");
    assert!(ui.contains("palette-visible: false"));
    assert!(ui.contains("duration: root.animate-opening ? 170ms : 0ms"));
    let settings = include_str!("../src/ui/settings.slint");
    assert!(!settings.to_lowercase().contains("start hidden"));
    assert!(settings.contains("Exit application"));
    assert!(ui.contains("text: \"Settings\""));
}
#[test]
fn opacity_is_background_only_and_shutdown_does_not_remove_startup_registration() {
    let ui = include_str!("../src/ui/launcher.slint");
    assert_eq!(ui.matches("root.panel-background-opacity").count(), 1);
    assert!(ui.contains("background: rgba(34,37,43,root.panel-background-opacity)"));
    let app = include_str!("../src/app.rs");
    let cleanup = app.split("cancel.store(true").nth(1).unwrap();
    assert!(cleanup.contains("drop(hotkey)"));
    assert!(cleanup.contains("worker.join()"));
    assert!(cleanup.contains("discovery_worker.join()"));
    assert!(!cleanup.contains("Registry"));
}
