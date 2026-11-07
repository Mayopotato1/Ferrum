//! Settings wiring; no startup writes outside explicit startup-changed callbacks.
use super::{Launcher, Settings, Shared, hotkey::Client, material, window};
use slint::{ComponentHandle, ModelRc, VecModel};
use spotlight_prototype::{
    hotkeys::Key,
    preferences::{Change, Document},
    startup, startup_migration as migration,
    web::SearchProvider,
};
use std::sync::{Arc, Mutex, atomic::AtomicU64};
pub struct Context {
    pub document: Arc<Mutex<Option<Document>>>,
    pub provider: Arc<Mutex<SearchProvider>>,
    pub hotkey: Client,
    pub shared: Shared,
    pub generation: Arc<AtomicU64>,
    pub software: bool,
}
fn startup_state(settings: &Settings) {
    let result = (|| -> anyhow::Result<(bool, bool, String)> {
        let exe = std::env::current_exe()?;
        let expected = startup::command(&exe)?;
        let state = migration::inspect(&migration::Native, &expected)?;
        Ok((state.enabled, state.removable, state.message))
    })();
    match result {
        Ok((on, can_remove, text)) => {
            settings.set_startup_can_remove(can_remove);
            settings.set_startup_enabled(on);
            settings.set_startup_status(text.into());
        }
        Err(e) => {
            settings.set_startup_enabled(false);
            settings.set_startup_can_remove(false);
            settings.set_startup_status(format!("Startup status unavailable: {e:#}").into());
        }
    }
}
fn opacity_state(ui: &Launcher, settings: &Settings, software: bool, percent: u8, enabled: bool) {
    let actual = material::for_platform(software, enabled, percent);
    ui.set_panel_background_opacity(actual);
    settings.set_opacity_percent(f32::from(percent));
    settings.set_opacity_status(
        if actual >= 1.0 && percent < 100 {
            format!(
                "Saved: {percent}%. Effective: 100% — opaque fallback for this renderer/compositor."
            )
        } else {
            format!(
                "Effective: {}%. Applied to the panel background; text and icons stay opaque.",
                (actual * 100.).round() as u8
            )
        }
        .into(),
    );
}
pub fn connect(ui: &Launcher, settings: &Settings, context: Context) {
    let Context {
        document,
        provider,
        hotkey,
        shared,
        generation,
        software,
    } = context;
    settings.set_providers(ModelRc::new(VecModel::from(
        SearchProvider::ALL
            .iter()
            .map(|p| p.name().into())
            .collect::<Vec<_>>(),
    )));
    settings.set_shortcuts(ModelRc::new(VecModel::from(
        Key::ALL
            .iter()
            .map(|k| k.label().into())
            .collect::<Vec<_>>(),
    )));
    let owner = ui.as_weak();
    let weak = settings.as_weak();
    let doc = document.clone();
    ui.on_settings(move||{
        let (Some(ui),Some(s))=(owner.upgrade(),weak.upgrade())else{return;};
        if let Some(doc)=doc.lock().unwrap().as_ref(){
            s.set_editable(true);
            s.set_provider_index(SearchProvider::ALL.iter().position(|p|*p==doc.config.search_provider).unwrap() as i32);
            s.set_shortcut_index(Key::ALL.iter().position(|k|k.label()==doc.config.hotkey).unwrap_or(0) as i32);
            opacity_state(&ui,&s,software,doc.config.background_opacity,doc.config.translucent_background);
            s.set_message(doc.warnings.join("\n").into());
        }else{s.set_editable(false);s.set_message("Config could not be read. Repair config.json and restart; the original file has been preserved.".into());}
        s.set_shortcut_status(ui.get_shortcut()); startup_state(&s);
        let _=window::hide(&ui); let _=s.show();
        spotlight_prototype::diagnostics::record("settings_opened",0);
    });
    let doc = document.clone();
    let weak = settings.as_weak();
    let owner = ui.as_weak();
    settings.on_provider_changed(move |index| {
        let (Some(p), Some(s)) = (
            SearchProvider::ALL.get(index.max(0) as usize).copied(),
            weak.upgrade(),
        ) else {
            return;
        };
        let mut guard = doc.lock().unwrap();
        let Some(doc) = guard.as_mut() else {
            return;
        };
        match doc.update(Change::Provider(p)) {
            Ok(()) => {
                *provider.lock().unwrap() = p;
                s.set_message("".into());
                if let Some(ui) = owner.upgrade() {
                    ui.set_pending(true);
                    super::submit(&shared, &generation, ui.get_query().to_string());
                }
            }
            Err(e) => {
                s.set_provider_index(
                    SearchProvider::ALL
                        .iter()
                        .position(|p| *p == doc.config.search_provider)
                        .unwrap() as i32,
                );
                s.set_message(format!("Search engine was not saved: {e:#}").into());
            }
        }
    });
    let weak = settings.as_weak();
    settings.on_shortcut_changed(move |index| {
        let (Some(key), Some(s)) = (Key::ALL.get(index.max(0) as usize).copied(), weak.upgrade())
        else {
            return;
        };
        s.set_shortcut_busy(true);
        if let Err(e) = hotkey.change(key) {
            s.set_shortcut_busy(false);
            s.set_message(format!("Shortcut request failed: {e:#}").into());
        }
    });
    let doc = document.clone();
    let weak = settings.as_weak();
    let owner = ui.as_weak();
    settings.on_opacity_changed(move |percent| {
        let (Some(s), Some(ui)) = (weak.upgrade(), owner.upgrade()) else {
            return;
        };
        let mut guard = doc.lock().unwrap();
        let Some(doc) = guard.as_mut() else {
            return;
        };
        let result = u8::try_from(percent)
            .map_err(anyhow::Error::from)
            .and_then(|p| doc.update(Change::Opacity(p)));
        match result {
            Ok(()) => s.set_message("".into()),
            Err(e) => s.set_message(format!("Opacity was not saved: {e:#}").into()),
        }
        opacity_state(
            &ui,
            &s,
            software,
            doc.config.background_opacity,
            doc.config.translucent_background,
        );
    });
    let weak = settings.as_weak();
    settings.on_startup_changed(move |enabled| {
        let Some(s) = weak.upgrade() else {
            return;
        };
        let result = (|| -> anyhow::Result<()> {
            let exe = std::env::current_exe()?;
            if !exe.is_file() {
                anyhow::bail!("Executable no longer exists at its current path");
            }
            let command = startup::command(&exe)?;
            let mut guard = document.lock().unwrap();
            let doc = guard
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Repair configuration first"))?;
            migration::change(&mut migration::Native, &command, enabled, || {
                doc.update(Change::Startup(enabled))
            })
        })();
        s.set_message(
            result
                .err()
                .map(|e| format!("Startup setting failed: {e:#}"))
                .unwrap_or_default()
                .into(),
        );
        startup_state(&s);
    });
    let weak = settings.as_weak();
    settings.on_dismiss(move || {
        if let Some(s) = weak.upgrade() {
            let _ = s.hide();
            spotlight_prototype::diagnostics::record("settings_closed", 0);
        }
    });
    settings
        .window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    let weak = ui.as_weak();
    settings.on_quit(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_quit();
        }
    });
}
