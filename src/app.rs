use anyhow::Result;
use slint::{ComponentHandle, ModelRc, VecModel};
use spotlight_prototype::{
    actions, cache, composition,
    config::{self, Config},
    diagnostics, discovery,
    model::{Action, Item},
    query::{self, Accept, Aliases, Intent, Mode, Selection},
    search::Index,
};
use std::{
    cell::RefCell,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

slint::slint! { export { Launcher, Row } from "ui/launcher.slint"; export { Settings } from "ui/settings.slint"; }
#[path = "ui/hotkey.rs"]
mod hotkey;
#[path = "ui/preferences.rs"]
mod preferences;
#[path = "ui/updates.rs"]
mod updates;
use hotkey::Hotkey;

#[path = "ui/icons.rs"]
mod icons;
#[path = "ui/material.rs"]
mod material;
#[path = "ui/window.rs"]
mod window;

fn source_label(item: &Item) -> &str {
    match item.category.as_str() {
        "Steam Game / Tool" | "Steam Game" => "Steam",
        "Modrinth Profile" => "Modrinth",
        "Website search" | "Website" => "Web",
        "General web search" => &item.source,
        "Folder" | "File" => "Local",
        "Packaged Application" if item.name.eq_ignore_ascii_case("Xbox") => "Xbox",
        _ => "Windows",
    }
}
fn icon_kind(item: &Item) -> &'static str {
    match item.category.as_str() {
        "Steam Game / Tool" | "Steam Game" | "Modrinth Profile" => "game",
        "Folder" => "folder",
        "File" => "file",
        "Website search" | "General web search" | "Website" => "web",
        _ => "app",
    }
}
fn row_detail(item: &Item) -> String {
    let detail = match item.category.as_str() {
        "Modrinth Profile" => "Profile · Opens Modrinth app only".into(),
        "Steam Game / Tool" | "Steam Game" => "Installed game or tool".into(),
        "Packaged Application" | "Windows Application" => "Application".into(),
        "Website search" => "Search in your default browser".into(),
        "General web search" => "Web search · Shift+Enter".into(),
        "Website" => {
            if let Action::OpenUrl(url) = &item.action {
                url.clone()
            } else {
                "Website".into()
            }
        }
        "File" | "Folder" => match &item.action {
            Action::Folder(path) | Action::Document(path) => path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| item.category.clone()),
            _ => item.category.clone(),
        },
        _ => item.category.clone(),
    };
    if item.available {
        detail
    } else {
        format!("{detail} · Unavailable")
    }
}

fn local_action(item: &Item) -> bool {
    !matches!(
        &item.action,
        Action::Website(_) | Action::WebSearch(_) | Action::OpenUrl(_)
    )
}
fn displayed_detail(item: &Item, cached: bool) -> String {
    let detail = row_detail(item);
    if cached && local_action(item) {
        format!("Cached - {detail}")
    } else {
        detail
    }
}
#[derive(Default)]
struct Work {
    query: String,
    generation: u64,
    search: bool,
    refresh: bool,
    stop: bool,
}
type Shared = Arc<(Mutex<Work>, Condvar)>;

fn submit(shared: &Shared, generation: &AtomicU64, q: String) {
    let n = generation.fetch_add(1, Ordering::SeqCst) + 1;
    diagnostics::record("query_submitted", n);
    let (lock, cv) = &**shared;
    let mut w = lock.lock().unwrap();
    w.query = q;
    w.generation = n;
    w.search = true;
    cv.notify_all();
}

pub fn run(open_settings: bool) -> Result<()> {
    let (document, config_error) =
        match spotlight_prototype::preferences::Document::open(config::path()) {
            Ok(doc) => {
                let warnings = doc.warnings.join("; ");
                (Some(doc), warnings)
            }
            Err(e) => (
                None,
                format!("Config error; file preserved, saving disabled: {e:#}"),
            ),
        };
    let c = document
        .as_ref()
        .map(|d| d.config.clone())
        .unwrap_or_else(Config::default);
    let document = Arc::new(Mutex::new(document));
    diagnostics::record("config_loaded", 0);
    let software = std::env::var("SLINT_BACKEND").is_ok_and(|value| value == "winit-software");
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(if software { "software" } else { "femtovg" }.into())
        .with_winit_window_attributes_hook(Box::new(window::attributes))
        .select()?;
    let ui = Launcher::new()?;
    ui.set_animate_opening(!software && window::animations_enabled());
    window::install(&ui);
    let settings = Settings::new()?;
    settings.set_application_version(env!("CARGO_PKG_VERSION").into());
    window::watch_settings(&settings, ui.as_weak());
    diagnostics::record("ui_objects_ready", 0);
    ui.set_error(config_error.into());
    ui.set_panel_background_opacity(material::for_platform(
        software,
        c.translucent_background,
        c.background_opacity,
    ));
    let provider = Arc::new(Mutex::new(c.search_provider));
    let aliases = Aliases::new(&c);
    if aliases.rejected > 0 {
        ui.set_error(
            format!(
                "{} invalid website alias entries excluded. Check config.json.",
                aliases.rejected
            )
            .into(),
        );
    }
    let generation = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let shared: Shared = Arc::new((
        Mutex::new(Work {
            refresh: true,
            ..Work::default()
        }),
        Condvar::new(),
    ));
    let (hotkey, label) = Hotkey::start(
        spotlight_prototype::hotkeys::Key::parse(&c.hotkey)?,
        ui.as_weak(),
        settings.as_weak(),
        document.clone(),
    )?;
    diagnostics::record(
        if label.starts_with("No shortcut") {
            "hotkey_unavailable"
        } else {
            "hotkey_ready"
        },
        0,
    );
    // File status is non-modal and remains available if every shortcut conflicts.
    let status_path = config::path().with_file_name("hotkey-status.txt");
    if let Some(parent) = status_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(status_path, &label);
    updates::connect(&ui, &settings, document.clone());
    preferences::connect(
        &ui,
        &settings,
        preferences::Context {
            document,
            provider: provider.clone(),
            hotkey: hotkey.client.clone(),
            shared: shared.clone(),
            generation: generation.clone(),
            software,
        },
    );
    ui.set_shortcut(label.into());
    let icons = icons::Loader::start(ui.as_weak(), generation.clone());
    // Discovery publishes immutable snapshots; search never waits for filesystem work.
    let snapshot = Arc::new(Mutex::new(Arc::new(cache::SearchSnapshot {
        index: Index::new(Vec::new()),
        status: String::from("Initial discovery in progress"),
        cached: false,
    })));
    let discovery_worker = {
        let c = c.clone();
        let shared = shared.clone();
        let cancel = cancel.clone();
        let snapshot = snapshot.clone();
        let weak = ui.as_weak();
        thread::spawn(move || {
            let cache_path = cache::path(&config::path());
            if let Ok(Some(items)) = cache::load(&cache_path, &c, &cancel) {
                let cached = Arc::new(cache::SearchSnapshot {
                    index: Index::new(items),
                    status: "Cached snapshot; refreshing sources".into(),
                    cached: true,
                });
                let count = cached.index.items.len();
                if cache::publish(&snapshot, cached, &cancel) {
                    diagnostics::record("cached_index_ready", count as u64);
                    let (lock, cv) = &*shared;
                    lock.lock().unwrap().search = true;
                    cv.notify_all();
                }
            }
            loop {
                {
                    let (lock, cv) = &*shared;
                    let mut w = lock.lock().unwrap();
                    while !w.stop && !w.refresh {
                        w = cv.wait(w).unwrap();
                    }
                    if w.stop {
                        break;
                    }
                    w.refresh = false;
                }
                let mut cache_candidate = None;
                let error = match discovery::discover(&c, &cancel) {
                    Ok(d) => {
                        let status = d
                            .providers
                            .iter()
                            .map(|p| {
                                format!(
                                    "{}: {}{}",
                                    p.name,
                                    p.count,
                                    if p.error.is_some() { " (error)" } else { "" }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" · ");
                        let error = d
                            .providers
                            .iter()
                            .filter_map(|p| p.error.as_ref().map(|e| format!("{}: {e}", p.name)))
                            .collect::<Vec<_>>()
                            .join("; ");
                        diagnostics::record("discovery_finished", d.items.len() as u64);
                        let next = Arc::new(cache::SearchSnapshot {
                            index: Index::new(d.items),
                            status,
                            cached: false,
                        });
                        if cache::publish(&snapshot, next.clone(), &cancel) {
                            diagnostics::record("fresh_index_ready", next.index.items.len() as u64);
                            if error.is_empty() {
                                cache_candidate = Some(next);
                            }
                        }
                        error
                    }
                    Err(e) => format!("Discovery failed: {e:#}"),
                };
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_indexing(false);
                    }
                    if let Some(ui) = weak.upgrade()
                        && !error.is_empty()
                    {
                        ui.set_error(error.into());
                    }
                });
                let (lock, cv) = &*shared;
                lock.lock().unwrap().search = true;
                cv.notify_all();
                if let Some(next) = cache_candidate {
                    let saved = cache::save(&cache_path, &c, &next.index.items, &cancel);
                    diagnostics::record(
                        if saved.is_ok() {
                            "cache_write_finished"
                        } else {
                            "cache_write_failed"
                        },
                        0,
                    );
                }
            }
        })
    };
    let worker = {
        let provider = provider.clone();
        let icon_requests = icons.requests.clone();
        let shared = shared.clone();
        let generation = generation.clone();
        let weak = ui.as_weak();
        thread::spawn(move || {
            loop {
                let (query, request_generation) = {
                    let (lock, cv) = &*shared;
                    let mut w = lock.lock().unwrap();
                    while !w.stop && !w.search {
                        w = cv.wait(w).unwrap();
                    }
                    if w.stop {
                        break;
                    }
                    w.search = false;
                    (w.query.clone(), w.generation)
                };
                let snapshot = snapshot.lock().unwrap().clone();
                let index = &snapshot.index;
                let provider_status = &snapshot.status;
                let cached = snapshot.cached;
                let now = std::time::Instant::now();
                let mode = query::mode(&query);
                let alias_mode = matches!(mode, Mode::AliasDiscovery | Mode::AliasAutocomplete);
                let current_provider = *provider.lock().unwrap();
                let (selections, error) =
                    match composition::results(index, &aliases, current_provider, &query) {
                        Ok(rows) => (rows, String::new()),
                        Err(error) => (Vec::new(), error.to_string()),
                    };
                let status = format!(
                    "{} indexed · search {:.2} ms · {}",
                    index.items.len(),
                    now.elapsed().as_secs_f64() * 1000.,
                    provider_status
                );
                let weak = weak.clone();
                let generation = generation.clone();
                let icon_requests = icon_requests.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if generation.load(Ordering::SeqCst) == request_generation
                        && let Some(ui) = weak.upgrade()
                    {
                        let rows: Vec<Row> = selections
                            .iter()
                            .map(|selection| match selection {
                                Selection::Launch(i) => Row {
                                    id: i.id.clone().into(),
                                    title: i.name.clone().into(),
                                    detail: displayed_detail(i, cached).into(),
                                    source: source_label(i).into(),
                                    kind: icon_kind(i).into(),
                                    icon: slint::Image::default(),
                                    available: i.available,
                                    completion: false,
                                },
                                Selection::Complete(alias) => Row {
                                    id: format!("alias:{}", alias.token).into(),
                                    title: format!("@{}", alias.token).into(),
                                    detail: format!("Search {}", alias.provider).into(),
                                    source: "Web".into(),
                                    kind: "web".into(),
                                    icon: slint::Image::default(),
                                    available: true,
                                    completion: true,
                                },
                            })
                            .collect();
                        let icon_items = selections
                            .iter()
                            .filter_map(|s| {
                                if let Selection::Launch(i) = s {
                                    Some(i.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        RESULT_ITEMS.with(|r| *r.borrow_mut() = selections);
                        ui.set_cached_results(cached);
                        diagnostics::record("results_applied", request_generation);
                        ui.set_alias_mode(alias_mode);
                        ui.set_rows(ModelRc::new(VecModel::from(rows)));
                        ui.set_selected(0);
                        ui.set_pending(false);
                        ui.set_status(status.into());
                        icon_requests.send(request_generation, icon_items);
                        if !error.is_empty() {
                            ui.set_error(error.into());
                        }
                    }
                });
            }
        })
    };
    let on_changed = {
        let shared = shared.clone();
        let generation = generation.clone();
        let weak = ui.as_weak();
        move |q: slint::SharedString| {
            if let Some(ui) = weak.upgrade() {
                ui.set_pending(true);
                ui.set_error("".into());
            }
            submit(&shared, &generation, q.to_string());
        }
    };
    ui.on_changed(on_changed);
    let weak = ui.as_weak();
    ui.on_conceal(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = window::hide(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_activate(move |selected| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_pending() || !ui.window().is_visible() {
            return;
        }
        let selection = RESULT_ITEMS.with(|r| r.borrow().get(selected.max(0) as usize).cloned());
        match query::intent(selection.as_ref(), Accept::Enter) {
            Intent::Complete(completion) => {
                ui.invoke_complete_alias(completion.text.into(), completion.cursor_byte as i32)
            }
            Intent::Launch => {
                if let Some(Selection::Launch(item)) = selection {
                    match actions::execute(&item) {
                        Ok(()) => {
                            let _ = window::hide(&ui);
                        }
                        Err(e) => ui.set_error(if ui.get_cached_results() && local_action(&item) {format!("Cached target could not be opened. Refresh metadata and retry: {e:#}").into()} else {format!("{e:#}").into()}),
                    }
                }
            }
            Intent::None => {}
        }
    });
    let force_owner = ui.as_weak();
    ui.on_force_web(move || {
        let Some(ui) = force_owner.upgrade() else {
            return;
        };
        if !ui.window().is_visible() {
            return;
        }
        match composition::submit(ui.get_query().as_str(), true, *provider.lock().unwrap()) {
            Ok(composition::SubmitPlan::Execute(item)) => match actions::execute(&item) {
                Ok(()) => {
                    let _ = window::hide(&ui);
                }
                Err(error) => ui.set_error(format!("{error:#}").into()),
            },
            Ok(composition::SubmitPlan::None | composition::SubmitPlan::Selected) => {}
            Err(error) => ui.set_error(error.to_string().into()),
        }
    });
    let weak = ui.as_weak();
    let refresh_shared = shared.clone();
    ui.on_refresh(move || {
        let (lock, cv) = &*refresh_shared;
        lock.lock().unwrap().refresh = true;
        cv.notify_all();
        if let Some(ui) = weak.upgrade() {
            ui.set_indexing(true);
            ui.set_status("Refreshing metadata in background…".into());
        }
    });
    let quitting = ui.as_weak();
    let settings_owner = settings.as_weak();
    ui.on_quit(move || {
        if let Some(settings) = settings_owner.upgrade() {
            let _ = settings.hide();
        }
        if let Some(ui) = quitting.upgrade() {
            let _ = window::hide(&ui);
        }
        let _ = slint::quit_event_loop();
    });
    ui.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    let result = (|| -> Result<()> {
        if open_settings {
            ui.invoke_settings();
        }
        // Always hidden: only the shortcut (or explicit --settings) shows UI.
        slint::run_event_loop_until_quit()?;
        Ok(())
    })();
    diagnostics::record("shutdown_requested", 0);
    cancel.store(true, Ordering::Relaxed);
    {
        let (lock, cv) = &*shared;
        lock.lock().unwrap().stop = true;
        cv.notify_all();
    }
    drop(hotkey);
    let _ = worker.join();
    let _ = discovery_worker.join();
    drop(icons);
    result
}
thread_local! { static RESULT_ITEMS: RefCell<Vec<Selection>>=const {RefCell::new(Vec::new())}; }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_marker_never_labels_live_web_actions() {
        let folder = Item::new(
            "Example",
            "Folder",
            "fixture",
            Action::Folder("C:\\Example".into()),
        );
        assert!(displayed_detail(&folder, true).starts_with("Cached - "));
        assert!(!displayed_detail(&folder, false).starts_with("Cached - "));
        let web = spotlight_prototype::web::SearchProvider::Google
            .search("example")
            .unwrap()
            .unwrap();
        assert_eq!(displayed_detail(&web, true), displayed_detail(&web, false));
    }
}
