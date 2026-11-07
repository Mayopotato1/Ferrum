//! Field-wise migration and atomic, optimistic writes. Never saves on load.
use crate::{config::Config, hotkeys::Key, web::SearchProvider};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone)]
pub enum Change {
    Provider(SearchProvider),
    Hotkey(Key),
    Opacity(u8),
    Startup(bool),
    Updates(crate::updates::Preferences),
}
pub struct Document {
    pub config: Config,
    pub warnings: Vec<String>,
    raw: Map<String, Value>,
    original: Option<Vec<u8>>,
    path: PathBuf,
}
impl Document {
    pub fn open(path: PathBuf) -> Result<Self> {
        let original = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let raw = match original.as_deref() {
            Some(bytes) => serde_json::from_slice::<Value>(bytes)?
                .as_object()
                .cloned()
                .context("Config must be a JSON object; repair it before saving settings")?,
            None => Map::new(),
        };
        let mut safe = serde_json::to_value(Config::default())?
            .as_object()
            .unwrap()
            .clone();
        let mut warnings = Vec::new();
        // Validate independently against Config's existing field types. Preserve raw
        // invalid/unknown fields until the user explicitly edits that preference.
        for (key, value) in &raw {
            if !safe.contains_key(key) {
                continue;
            }
            let mut probe = Map::new();
            probe.insert(key.clone(), value.clone());
            let valid = serde_json::from_value::<Config>(Value::Object(probe)).is_ok()
                && match key.as_str() {
                    "hotkey" => value.as_str().is_some_and(|s| Key::parse(s).is_ok()),
                    "background_opacity" => value.as_u64().is_some_and(|v| (85..=100).contains(&v)),
                    _ => true,
                };
            if valid {
                safe.insert(key.clone(), value.clone());
            } else {
                warnings.push(format!(
                    "Invalid {key}; using its default until corrected in Settings/config.json"
                ));
            }
        }
        // Phase 1.2 opaque preference remains opaque on migration.
        if !raw.contains_key("background_opacity")
            && raw.get("translucent_background") == Some(&Value::Bool(false))
        {
            safe.insert("background_opacity".into(), Value::from(100));
        }
        Ok(Self {
            config: serde_json::from_value(Value::Object(safe))?,
            warnings,
            raw,
            original,
            path,
        })
    }
    pub fn update(&mut self, change: Change) -> Result<()> {
        let mut raw = self.raw.clone();
        match &change {
            Change::Provider(v) => {
                raw.insert("search_provider".into(), serde_json::to_value(v)?);
            }
            Change::Hotkey(v) => {
                raw.insert("hotkey".into(), Value::from(v.label()));
            }
            Change::Opacity(v) => {
                if !(85..=100).contains(v) {
                    bail!("Opacity must be 85–100%");
                }
                raw.insert("background_opacity".into(), Value::from(*v));
                raw.insert("translucent_background".into(), Value::Bool(*v < 100));
            }
            Change::Updates(v) => {
                raw.insert("updates".into(), serde_json::to_value(v)?);
            }
            Change::Startup(v) => {
                raw.insert("launch_on_startup".into(), Value::Bool(*v));
            }
        }
        let current = match std::fs::read(&self.path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if current != self.original {
            bail!("Config changed outside this process. Restart to reload it before saving.");
        }
        let bytes = serde_json::to_vec_pretty(&raw)?;
        atomic_write(&self.path, &bytes)?;
        self.raw = raw;
        self.original = Some(bytes);
        match &change {
            Change::Updates(v) => self.config.updates = v.clone(),
            Change::Provider(v) => self.config.search_provider = *v,
            Change::Hotkey(v) => self.config.hotkey = v.label().into(),
            Change::Opacity(v) => {
                self.config.background_opacity = *v;
                self.config.translucent_background = *v < 100;
            }
            Change::Startup(v) => self.config.launch_on_startup = *v,
        }
        let field = match change {
            Change::Updates(_) => "updates",
            Change::Provider(_) => "search_provider",
            Change::Hotkey(_) => "hotkey",
            Change::Opacity(_) => "background_opacity",
            Change::Startup(_) => "launch_on_startup",
        };
        self.warnings
            .retain(|warning| !warning.starts_with(&format!("Invalid {field};")));
        Ok(())
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().context("Config has no parent folder")?;
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_file_name(format!(
        ".settings-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // Only clean up a temporary file that this call successfully created.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        use std::os::windows::ffi::OsStrExt;
        use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
        let src: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            MoveFileExW(
                PCWSTR(src.as_ptr()),
                PCWSTR(dst.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
