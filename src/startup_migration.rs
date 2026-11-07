//! Explicit user-controlled, two-name migration. Never called during startup/config loading.
use crate::startup::{self, Backend};
use anyhow::{Result, bail};
pub const PRODUCT_VALUE_NAME: &str = "Ferrum";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Legacy,
    Ferrum,
}
impl Slot {
    pub fn name(self) -> &'static str {
        match self {
            Self::Legacy => startup::VALUE_NAME,
            Self::Ferrum => PRODUCT_VALUE_NAME,
        }
    }
}
pub trait Entries {
    fn read(&self, slot: Slot) -> Result<Option<String>>;
    fn write(&mut self, slot: Slot, value: Option<&str>) -> Result<()>;
}
pub struct Native;
impl Entries for Native {
    fn read(&self, slot: Slot) -> Result<Option<String>> {
        startup::NamedValue(slot.name()).read()
    }
    fn write(&mut self, slot: Slot, value: Option<&str>) -> Result<()> {
        startup::NamedValue(slot.name()).write(value)
    }
}
pub struct State {
    pub enabled: bool,
    pub removable: bool,
    pub message: String,
}
fn owned(slot: Slot, value: &str, expected: &str) -> bool {
    if value == expected {
        return true;
    }
    let Some(path) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return false;
    };
    let path = std::path::Path::new(path);
    if !path.is_absolute() || path.to_string_lossy().contains(['"', '\r', '\n', '\0']) {
        return false;
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    match slot {
        Slot::Ferrum => name == "ferrum.exe",
        Slot::Legacy => [
            "spotlight-launcher.exe",
            "spotlight-launcher-phase1-0.exe",
            "spotlight-launcher-phase1-1.exe",
            "spotlight-launcher-phase1-2.exe",
            "spotlight-launcher-phase1-3.exe",
        ]
        .contains(&name.as_str()),
    }
}
fn values(backend: &impl Entries) -> Result<[Option<String>; 2]> {
    Ok([backend.read(Slot::Legacy)?, backend.read(Slot::Ferrum)?])
}
fn validate(before: &[Option<String>; 2], expected: &str) -> Result<()> {
    for (slot, value) in [Slot::Legacy, Slot::Ferrum].into_iter().zip(before) {
        if let Some(value) = value
            && !owned(slot, value, expected)
        {
            bail!(
                "The {} startup value contains an unrecognized command; both values were left untouched.",
                slot.name()
            );
        }
    }
    Ok(())
}
pub fn inspect(backend: &impl Entries, expected: &str) -> Result<State> {
    let before = values(backend)?;
    validate(&before, expected)?;
    let (legacy, current) = (&before[0], &before[1]);
    let message=match (legacy,current){
        (None,None)=>"Off — Ferrum has no startup registration.".into(),
        (Some(_),_)=>"Legacy startup registration retained. Enable to explicitly migrate/consolidate it to Ferrum, or Remove startup registration to disable it. No automatic migration occurs.".into(),
        (None,Some(v)) if v==expected=>"On — Ferrum registration matches this executable.".into(),
        _=>"Ferrum startup path is stale. Enable to repair it, or remove the registration.".into(),
    };
    Ok(State {
        enabled: current.as_deref() == Some(expected) && legacy.is_none(),
        removable: legacy.is_some() || current.is_some(),
        message,
    })
}
pub fn change(
    backend: &mut impl Entries,
    expected: &str,
    enabled: bool,
    save: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let before = values(backend)?;
    validate(&before, expected)?;
    let desired = [None, enabled.then(|| expected.to_string())];
    let apply = |backend: &mut _, values: &[Option<String>; 2]| -> Result<()> {
        // First create/verify Ferrum, then retire the recognized legacy entry.
        for (slot, value) in [(Slot::Ferrum, &values[1]), (Slot::Legacy, &values[0])] {
            backend_write(backend, slot, value.as_deref())?;
        }
        Ok(())
    };
    let result = apply(backend, &desired).and_then(|()| save());
    if let Err(error) = result {
        let mut errors = Vec::new();
        // Attempt both restorations even if one name becomes unwritable.
        for (slot, value) in [(Slot::Ferrum, &before[1]), (Slot::Legacy, &before[0])] {
            if let Err(e) = backend_write(backend, slot, value.as_deref()) {
                errors.push(format!("{}: {e:#}", slot.name()));
            }
        }
        let detail = if errors.is_empty() {
            "Previous startup registrations restored".into()
        } else {
            format!("Startup rollback failed: {}", errors.join("; "))
        };
        return Err(error.context(format!(
            "{detail}. Reopen Settings to inspect actual registration."
        )));
    }
    Ok(())
}
fn backend_write(backend: &mut impl Entries, slot: Slot, value: Option<&str>) -> Result<()> {
    backend.write(slot, value)?;
    if backend.read(slot)?.as_deref() != value {
        bail!(
            "Windows did not retain the requested {} startup value",
            slot.name()
        );
    }
    Ok(())
}
