//! Dedicated update worker: never uses the search queue or touches startup registration.
use super::{Launcher, Settings};
use slint::ComponentHandle;
use spotlight_prototype::{
    preferences::{Change, Document},
    updates::{self, Preferences, Source},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

enum Command {
    Manual,
    Automatic(bool),
    Dismiss(String),
}
fn publish(
    ui: &slint::Weak<Launcher>,
    settings: &slint::Weak<Settings>,
    p: &Preferences,
    source: Option<&Source>,
    error: Option<String>,
    announced: &mut Option<String>,
) {
    let installed = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let status = error.unwrap_or_else(|| p.status(source, &installed));
    let tag = p
        .applicable(source, &installed)
        .map(|r| r.tag.clone())
        .unwrap_or_default();
    let notification = p
        .notification(source, &installed)
        .map(|r| r.tag.clone())
        .filter(|t| announced.as_ref() != Some(t));
    if let Some(t) = &notification {
        *announced = Some(t.clone());
    }
    let last = p
        .last_checked
        .map(|t| {
            format!(
                "Last successful check: {}",
                httpdate::fmt_http_date(
                    std::time::UNIX_EPOCH + Duration::from_secs(t.min(253_402_300_799))
                )
            )
        })
        .unwrap_or_else(|| "No successful check yet.".into());
    let automatic = p.automatic;
    let owner = ui.clone();
    let _ = settings.upgrade_in_event_loop(move |s| {
        s.set_updates_automatic(automatic);
        s.set_updates_status(status.into());
        s.set_updates_last(last.into());
        s.set_update_version(tag.into());
        s.set_updates_busy(false);
        if let Some(ui) = owner.upgrade()
            && let Some(t) = notification
        {
            ui.set_pending_update(t.into());
        }
    });
}
pub fn connect(ui: &Launcher, settings: &Settings, document: Arc<Mutex<Option<Document>>>) {
    let (tx, rx) = mpsc::sync_channel(4);
    let busy = Arc::new(AtomicBool::new(false));
    let sender = tx.clone();
    let active = busy.clone();
    let weak = settings.as_weak();
    settings.on_updates_check(move || {
        if !active.swap(true, Ordering::Relaxed) {
            if sender.try_send(Command::Manual).is_err() {
                active.store(false, Ordering::Relaxed);
            } else if let Some(s) = weak.upgrade() {
                s.set_updates_busy(true);
                s.set_updates_status("Checking…".into());
            }
        }
    });
    let sender = tx.clone();
    settings.on_updates_changed(move |enabled| {
        let _ = sender.try_send(Command::Automatic(enabled));
    });
    ui.on_update_later(move |tag| {
        let _ = tx.try_send(Command::Dismiss(tag.to_string()));
    });
    let doc = document.clone();
    let weak = settings.as_weak();
    settings.on_update_view(move || {
        if let Some(s) = weak.upgrade()
            && let Err(e) = open(&doc, s.get_update_version().as_str())
        {
            s.set_updates_status(format!("Unable to open update: {e:#}").into());
        }
    });
    let doc = document.clone();
    let weak = ui.as_weak();
    ui.on_update_view(move |tag| {
        if let Err(e) = open(&doc, tag.as_str())
            && let Some(ui) = weak.upgrade()
        {
            ui.set_error(format!("Unable to open update: {e:#}").into());
        }
    });
    let owner = ui.as_weak();
    let weak = settings.as_weak();
    std::thread::Builder::new()
        .name("ferrum-updates".into())
        .spawn(move || {
            let source = Source::configured();
            let installed = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
            let mut announced = None;
            let mut first = true;
            loop {
                let Some(mut p) = document
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|d| d.config.updates.clone())
                else {
                    break;
                };
                let before = p.last_attempt;
                p.normalize_clock(updates::now());
                if p.last_attempt != before {
                    let _ = save(&document, p.clone());
                }
                if first {
                    publish(&owner, &weak, &p, source.as_ref(), None, &mut announced);
                }
                let delay = if p.automatic && source.is_some() {
                    let due = p
                        .last_attempt
                        .map(|t| t.saturating_add(updates::DAY))
                        .unwrap_or(0)
                        .max(p.retry_until);
                    Some(Duration::from_secs(
                        due.saturating_sub(updates::now())
                            .max(if first { 10 } else { 1 }),
                    ))
                } else {
                    None
                };
                first = false;
                let command = match delay {
                    Some(d) => match rx.recv_timeout(d) {
                        Ok(c) => Some(c),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(_) => break,
                    },
                    None => match rx.recv() {
                        Ok(c) => Some(c),
                        Err(_) => break,
                    },
                };
                // Reload to merge ordinary Settings edits made while waiting.
                let Some(current) = document
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|d| d.config.updates.clone())
                else {
                    break;
                };
                p = current;
                let manual = matches!(command, Some(Command::Manual));
                let mut error = None;
                match command {
                    Some(Command::Automatic(on)) => {
                        p.automatic = on;
                        error = save(&document, p.clone())
                            .err()
                            .map(|e| format!("Update preference was not saved: {e:#}"));
                    }
                    Some(Command::Dismiss(tag)) => {
                        p.dismissed = Some(tag);
                        error = save(&document, p.clone())
                            .err()
                            .map(|e| format!("Dismissal was not saved: {e:#}"));
                    }
                    _ => {
                        let now = updates::now();
                        if p.due(source.as_ref(), now, manual) {
                            p.last_attempt = Some(now);
                            // Preserve cadence across crashes; don't make a request if persistence fails.
                            match save(&document, p.clone()) {
                                Ok(()) => {
                                    let _ = updates::check(
                                        &mut p,
                                        source.as_ref(),
                                        &installed,
                                        &transport(),
                                        now,
                                    );
                                    error = save(&document, p.clone())
                                        .err()
                                        .map(|e| format!("Update result was not saved: {e:#}"));
                                }
                                Err(e) => {
                                    error = Some(format!(
                                        "Unable to check: last attempt could not be saved: {e:#}"
                                    ))
                                }
                            }
                        } else if manual && source.is_some() {
                            error = Some("Unable to check: retry delay is active.".into());
                        }
                    }
                }
                // Read back persisted values after failures, including preference rollback.
                if let Some(saved) = document.lock().unwrap().as_ref() {
                    p = saved.config.updates.clone();
                }
                publish(&owner, &weak, &p, source.as_ref(), error, &mut announced);
                busy.store(false, Ordering::Relaxed);
            }
        })
        .expect("Unable to start update worker");
}
fn save(doc: &Arc<Mutex<Option<Document>>>, p: Preferences) -> anyhow::Result<()> {
    doc.lock()
        .unwrap()
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("Configuration unavailable"))?
        .update(Change::Updates(p))
}
fn open(doc: &Arc<Mutex<Option<Document>>>, tag: &str) -> anyhow::Result<()> {
    let source =
        Source::configured().ok_or_else(|| anyhow::anyhow!("Update source not configured"))?;
    let installed = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let action = {
        let guard = doc.lock().unwrap();
        let p = &guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Configuration unavailable"))?
            .config
            .updates;
        let release = p
            .applicable(Some(&source), &installed)
            .filter(|r| r.tag == tag)
            .ok_or_else(|| anyhow::anyhow!("Release no longer applicable"))?;
        updates::browser_action(&source, release, &installed)?
    };
    spotlight_prototype::actions::execute(&action)
}

#[cfg(not(feature = "update-fixture"))]
fn transport() -> updates::Https {
    updates::Https
}
#[cfg(feature = "update-fixture")]
fn transport() -> updates::LocalFixture {
    updates::LocalFixture(
        std::env::var_os("FERRUM_UPDATE_FIXTURE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_default(),
    )
}
