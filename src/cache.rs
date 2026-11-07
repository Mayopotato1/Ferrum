//! Bounded local metadata snapshot, never executable code. Every selected action
//! still goes through the unchanged actions::prepare/execute validation.
use crate::{
    config::Config,
    model::{Action, Item, normalize},
    search::Index,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf, Prefix},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
pub const SCHEMA: u32 = 1;
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ITEMS: usize = 50_000;
const MAX_AGE: u64 = 7 * 24 * 60 * 60;
#[derive(Serialize, Deserialize)]
struct Stored<T> {
    schema: u32,
    application: String,
    key: u64,
    created: u64,
    checksum: u64,
    items: T,
}
struct Hash(u64);
impl Default for Hash {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}
impl Write for Hash {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn hash(value: &impl Serialize) -> Result<u64> {
    let mut h = Hash::default();
    serde_json::to_writer(&mut h, value)?;
    Ok(h.0)
}
fn key(c: &Config) -> Result<u64> {
    hash(&(
        &c.steam_roots,
        &c.modrinth_roots,
        &c.user_aliases,
        c.max_files,
        c.max_depth,
    ))
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
pub fn path(config_path: &Path) -> PathBuf {
    config_path.with_file_name("ferrum-index-v1.json")
}
fn local_absolute(path: &Path) -> bool {
    matches!(path.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
        && path.is_absolute()
        && !path.components().any(|c| matches!(c, Component::ParentDir))
        && path
            .to_str()
            .is_some_and(|s| s.len() <= 131072 && !s.contains('\0'))
}
fn local_store(path: &Path) -> Result<()> {
    ensure!(local_absolute(path), "Cache needs an absolute local path");
    let parent = path.parent().context("Cache has no parent")?;
    ensure!(
        crate::discovery::local_path(parent) && parent.is_dir(),
        "Cache folder is not an available local folder"
    );
    match path.symlink_metadata() {
        Ok(metadata) => ensure!(
            metadata.is_file() && crate::discovery::local_path(path),
            "Cache target is not a regular local file"
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
fn valid(item: &Item) -> bool {
    if item.id.len() > 131072
        || item.name.len() > 32768
        || item.category.len() > 256
        || item.source.len() > 256
        || item.aliases.len() > 128
        || item.aliases.iter().any(|a| a.len() > 4096)
        || item.normalized != normalize(&item.name)
        || item.category == "Modrinth Profile"
        || !item.available
    {
        return false;
    }
    match &item.action {
        Action::Shortcut {
            path,
            target,
            arguments,
            working_dir,
        } => {
            local_absolute(path)
                && local_absolute(Path::new(target))
                && arguments.is_empty()
                && (working_dir.is_empty() || local_absolute(Path::new(working_dir)))
        }
        Action::Packaged { aumid } => {
            aumid.len() <= 512 && aumid.contains('!') && !aumid.contains(['\0', '\\', '/'])
        }
        Action::Steam {
            app_id,
            manifest,
            install_dir,
        } => *app_id > 0 && local_absolute(manifest) && local_absolute(install_dir),
        Action::Folder(p) => local_absolute(p),
        Action::Document(p) => local_absolute(p) && crate::discovery::safe_document(p),
        Action::Unavailable(_) => false,
        Action::Website(_) | Action::WebSearch(_) | Action::OpenUrl(_) => false,
    }
}
pub fn load(path: &Path, c: &Config, cancel: &AtomicBool) -> Result<Option<Vec<Item>>> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    local_store(path)?;
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        file.metadata()?.len() <= MAX_BYTES as u64,
        "Cache exceeds size bound"
    );
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_BYTES, "Cache grew beyond size bound");
    let stored: Stored<Vec<Item>> = serde_json::from_slice(&bytes).context("Invalid cache JSON")?;
    ensure!(
        stored.schema == SCHEMA
            && stored.application == env!("CARGO_PKG_VERSION")
            && stored.key == key(c)?,
        "Cache version/config mismatch"
    );
    let current = now()?;
    ensure!(
        stored.created <= current + 300 && current.saturating_sub(stored.created) <= MAX_AGE,
        "Cache expired or timestamp invalid"
    );
    ensure!(stored.items.len() <= MAX_ITEMS, "Too many cached items");
    ensure!(
        stored.checksum
            == hash(&(
                stored.schema,
                &stored.application,
                stored.key,
                stored.created,
                &stored.items
            ))?,
        "Cache checksum mismatch"
    );
    ensure!(
        stored.items.iter().all(valid),
        "Unsafe or inconsistent cached metadata"
    );
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    Ok(Some(stored.items))
}
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_BYTES {
            return Err(std::io::Error::other("Cache exceeds size bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn save(path: &Path, c: &Config, items: &[Item], cancel: &AtomicBool) -> Result<bool> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(false);
    }
    local_store(path)?;
    // Profile actions carry only the Modrinth app target, not the profile path;
    // keep those fresh-only instead of claiming profile revalidation from cache.
    let items: Vec<_> = items
        .iter()
        .filter(|i| i.category != "Modrinth Profile" && i.available && !matches!(&i.action, Action::Unavailable(_)) && !matches!(&i.action, Action::Shortcut { target, arguments, working_dir, .. } if !arguments.is_empty() || !local_absolute(Path::new(target)) || !working_dir.is_empty() && !local_absolute(Path::new(working_dir))))
        .collect();
    ensure!(
        items.len() <= MAX_ITEMS && items.iter().all(|i| valid(i)),
        "Cache item bound/validation failed"
    );
    let application = env!("CARGO_PKG_VERSION").to_string();
    let key = key(c)?;
    let created = now()?;
    let checksum = hash(&(SCHEMA, &application, key, created, &items))?;
    let stored = Stored {
        schema: SCHEMA,
        application,
        key,
        created,
        checksum,
        items,
    };
    let mut bytes = Limited(Vec::new());
    serde_json::to_writer(&mut bytes, &stored)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(false);
    }
    crate::preferences::atomic_write(path, &bytes.0)?;
    Ok(true)
}
pub struct SearchSnapshot {
    pub index: Index,
    pub status: String,
    pub cached: bool,
}
pub fn publish(
    target: &Mutex<Arc<SearchSnapshot>>,
    next: Arc<SearchSnapshot>,
    cancel: &AtomicBool,
) -> bool {
    let mut slot = target.lock().unwrap();
    if cancel.load(Ordering::Relaxed) {
        return false;
    }
    *slot = next;
    true
}
