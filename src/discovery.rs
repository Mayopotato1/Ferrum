use crate::{
    config::Config,
    model::{Action, Item},
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        System::{Com::*, Registry::*},
        UI::Shell::*,
    },
    core::{GUID, HSTRING, Interface},
};

pub struct Com;
impl Com {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
pub fn known(id: &GUID) -> Result<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DONT_VERIFY, None)?;
        let s = p.to_string();
        CoTaskMemFree(Some(p.0.cast()));
        Ok(s?.into())
    }
}
pub fn local_path(p: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    use std::path::{Component, Prefix};
    // A mapped drive may be remote even when its path is not UNC.
    let drive = match p.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => d.to_ascii_uppercase(),
            _ => return false,
        },
        _ => return false,
    };
    thread_local! { static DRIVES: std::cell::RefCell<std::collections::BTreeMap<u8,bool>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) }; }
    let local = DRIVES.with(|cache| {
        *cache.borrow_mut().entry(drive).or_insert_with(|| unsafe {
            let root = HSTRING::from(format!("{}:\\", drive as char));
            matches!(
                windows::Win32::Storage::FileSystem::GetDriveTypeW(&root),
                2 | 3 | 6
            )
        })
    });
    // No remote, reparse, offline or recall-on-access entries. Never hydrate a cloud file.
    local
        && !p.to_string_lossy().starts_with(r"\\")
        && p.symlink_metadata()
            .is_ok_and(|m| m.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x400000) == 0)
}
fn walk(
    root: &Path,
    depth: usize,
    cap: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((p, d)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) || Instant::now() > deadline || out.len() >= cap {
            break;
        }
        if !local_path(&p) {
            continue;
        }
        let Ok(rd) = p.read_dir() else {
            continue;
        };
        for e in rd.flatten() {
            if out.len() >= cap || cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
                break;
            }
            let p = e.path();
            if !local_path(&p) {
                continue;
            }
            if p.is_dir()
                && d < depth
                && !["target", ".git", "node_modules", ".venv"]
                    .contains(&e.file_name().to_string_lossy().as_ref())
            {
                stack.push((p.clone(), d + 1));
            }
            out.push(p);
        }
    }
    out.sort();
    out
}
fn wide_text(b: &[u16]) -> String {
    String::from_utf16_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())])
}
pub fn shortcut(path: &Path) -> Result<Item> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let persist: IPersistFile = link.cast()?;
        persist.Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;
        let mut target = vec![0u16; 32768];
        let mut args = vec![0u16; 32768];
        let mut dir = vec![0u16; 32768];
        link.GetPath(&mut target, std::ptr::null_mut(), 0)?;
        link.GetArguments(&mut args)?;
        link.GetWorkingDirectory(&mut dir)?;
        let target = wide_text(&target);
        let arguments = wide_text(&args);
        let working_dir = wide_text(&dir);
        let available = !target.is_empty() && Path::new(&target).exists();
        let mut item = Item::new(
            &path.file_stem().unwrap_or_default().to_string_lossy(),
            "Windows Application",
            "Windows shortcut",
            Action::Shortcut {
                path: path.into(),
                target: target.clone(),
                arguments: arguments.clone(),
                working_dir: working_dir.clone(),
            },
        );
        item.available = available;
        if available {
            item.id = format!(
                "shortcut:{}|{}|{}",
                target.to_lowercase(),
                arguments,
                working_dir.to_lowercase()
            );
        }
        Ok(item)
    }
}
fn windows_apps(cancel: &AtomicBool) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    for id in [
        &FOLDERID_Programs,
        &FOLDERID_CommonPrograms,
        &FOLDERID_Desktop,
        &FOLDERID_PublicDesktop,
    ] {
        if let Ok(root) = known(id) {
            for p in walk(
                &root,
                5,
                3000,
                Instant::now() + Duration::from_secs(3),
                cancel,
            ) {
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
                    && let Ok(i) = shortcut(&p)
                {
                    items.push(i);
                }
            }
        }
    }
    Ok(items)
}
fn packaged(cancel: &AtomicBool) -> Result<Vec<Item>> {
    unsafe {
        let folder: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from("shell:AppsFolder"), None)?;
        let en: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems)?;
        let mut out = Vec::new();
        for _ in 0..3000 {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let mut arr = [None];
            let mut count = 0;
            en.Next(&mut arr, Some(&mut count))?;
            if count == 0 {
                break;
            }
            let item = arr[0].take().unwrap();
            let name = item.GetDisplayName(SIGDN_NORMALDISPLAY)?;
            let text = name.to_string();
            CoTaskMemFree(Some(name.0.cast()));
            let id = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)?;
            let identity = id.to_string();
            CoTaskMemFree(Some(id.0.cast()));
            let identity = identity?;
            let aumid = identity.rsplit('\\').next().unwrap_or(&identity);
            if !aumid.contains('!') {
                continue;
            }
            out.push(Item::new(
                &text?,
                "Packaged Application",
                "Windows AppsFolder (game classification unverified)",
                Action::Packaged {
                    aumid: aumid.into(),
                },
            ));
        }
        Ok(out)
    }
}
fn reg_string(key: HKEY, sub: &str, value: &str) -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; 32768];
        let mut size = (buf.len() * 2) as u32;
        RegGetValueW(
            key,
            &HSTRING::from(sub),
            &HSTRING::from(value),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
        Some(wide_text(&buf))
    }
}
pub fn steam_manifest(text: &str, manifest: &Path, steamapps: &Path) -> Result<Item> {
    let v = keyvalues_parser::parse(text)?;
    let obj = v.value.get_obj().context("Manifest must be an object")?;
    let get = |key: &str| {
        obj.get(key)
            .and_then(|v| v.first())
            .and_then(|v| v.get_str())
            .context(format!("Missing {key}"))
    };
    let id: u32 = get("appid")?.parse()?;
    anyhow::ensure!(
        manifest.file_stem().and_then(|s| s.to_str()) == Some(format!("appmanifest_{id}").as_str()),
        "Manifest ID mismatch"
    );
    let dir = get("installdir")?;
    anyhow::ensure!(
        !dir.is_empty() && !dir.contains(['/', '\\']) && dir != "..",
        "Unsafe Steam install directory"
    );
    let install_dir = steamapps.join("common").join(dir);
    let flags: u32 = get("StateFlags")?.parse()?;
    let mut item = Item::new(
        get("name")?,
        "Steam Game / Tool",
        "Steam installed manifest",
        Action::Steam {
            app_id: id,
            manifest: manifest.into(),
            install_dir: install_dir.clone(),
        },
    );
    item.available = flags & 4 != 0 && install_dir.is_dir();
    item.id = format!("steam:{id}");
    Ok(item)
}
pub fn steam_libraries(text: &str) -> Result<Vec<PathBuf>> {
    let v = keyvalues_parser::parse(text)?;
    let obj = v.value.get_obj().context("Libraries must be an object")?;
    let mut paths = Vec::new();
    for (key, entries) in obj.iter() {
        if key.parse::<u32>().is_err() {
            continue;
        }
        for entry in entries {
            let path = entry.get_str().or_else(|| {
                entry
                    .get_obj()
                    .and_then(|o| o.get("path"))
                    .and_then(|v| v.first())
                    .and_then(|v| v.get_str())
            });
            if let Some(p) = path {
                paths.push(p.into());
            }
        }
    }
    Ok(paths)
}
fn steam(c: &Config, cancel: &AtomicBool) -> Result<Vec<Item>> {
    let mut roots = c.steam_roots.clone();
    for (key, sub, val) in [
        (HKEY_CURRENT_USER, "Software\\Valve\\Steam", "SteamPath"),
        (
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\WOW6432Node\\Valve\\Steam",
            "InstallPath",
        ),
    ] {
        if let Some(p) = reg_string(key, sub, val) {
            roots.push(p.into());
        }
    }
    let initial = roots.clone();
    for r in initial {
        let p = r.join("steamapps/libraryfolders.vdf");
        if local_path(&p)
            && let Ok(t) = std::fs::read_to_string(p)
        {
            roots.extend(steam_libraries(&t)?);
        }
    }
    let mut unique = BTreeSet::new();
    let mut out = Vec::new();
    for root in roots {
        if !unique.insert(
            root.canonicalize()
                .unwrap_or_else(|_| root.clone())
                .to_string_lossy()
                .to_lowercase(),
        ) || !local_path(&root)
        {
            continue;
        }
        let apps = root.join("steamapps");
        let Ok(rd) = apps.read_dir() else {
            continue;
        };
        for e in rd.flatten().take(4000) {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let p = e.path();
            if p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("appmanifest_"))
                && p.extension().is_some_and(|e| e == "acf")
                && local_path(&p)
                && let Ok(t) = std::fs::read_to_string(&p)
                && let Ok(i) = steam_manifest(&t, &p, &apps)
                && i.available
            {
                out.push(i);
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    Ok(out)
}
fn modrinth(c: &Config, apps: &[Item], cancel: &AtomicBool) -> Result<Vec<Item>> {
    let mut roots = c.modrinth_roots.clone();
    let roaming = known(&FOLDERID_RoamingAppData)?;
    roots.extend([
        roaming.join("ModrinthApp"),
        roaming.join("com.modrinth.theseus"),
    ]);
    for root in roots.clone() {
        let db = root.join("app.db");
        if !local_path(&db) {
            continue;
        }
        if let Ok(conn) = rusqlite::Connection::open_with_flags(
            &db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            // Read only the storage setting, never account/credential rows.
            if let Ok(Some(path)) =
                conn.query_row("SELECT custom_dir FROM settings WHERE id=0", [], |r| {
                    r.get::<_, Option<String>>(0)
                })
                && !path.is_empty()
            {
                roots.push(path.into());
            }
        }
    }
    let mut out = Vec::new();
    let app = apps
        .iter()
        .find(|i| i.available && (i.normalized == "modrinth app" || i.normalized == "modrinth"));
    let mut seen = BTreeSet::new();
    for root in roots {
        if !seen.insert(root.clone()) || !local_path(&root) || cancel.load(Ordering::Relaxed) {
            continue;
        }
        let db = root.join("app.db");
        if local_path(&db) {
            let conn = rusqlite::Connection::open_with_flags(
                &db,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            conn.busy_timeout(Duration::from_millis(100))?;
            // Only inspect profile schema and select non-account profile metadata.
            let table: String = conn.query_row("SELECT name FROM sqlite_master WHERE type='table' AND name IN ('instances','profiles') ORDER BY name LIMIT 1", [], |r| r.get(0)).unwrap_or_else(|_| "profiles".into());
            let mut schema = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let cols: Vec<String> = schema
                .query_map([], |r| r.get(1))?
                .filter_map(Result::ok)
                .collect();
            if cols.iter().any(|c| c == "name") && cols.iter().any(|c| c == "path") {
                let mut stmt =
                    conn.prepare(&format!("SELECT name, path FROM {table} LIMIT 1000"))?;
                let rows =
                    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
                for row in rows.flatten() {
                    let (name, path) = row;
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let mut i = Item::new(
                        &name,
                        "Modrinth Profile",
                        "Modrinth database — opens app only",
                        app.map(|i| i.action.clone()).unwrap_or(Action::Unavailable(
                            "Modrinth app entry not found; direct profile activation unverified"
                                .into(),
                        )),
                    );
                    i.id = format!("modrinth:{}:{path}", root.display());
                    out.push(i);
                }
            }
        }
        // Legacy installations stored individual profile.json metadata.
        let profiles = root.join("profiles");
        if let Ok(rd) = profiles.read_dir() {
            for e in rd.flatten().take(1000) {
                if !local_path(&e.path()) {
                    continue;
                }
                let p = e.path().join("profile.json");
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if !local_path(&p) {
                    continue;
                }
                let Ok(t) = std::fs::read_to_string(&p) else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) else {
                    continue;
                };
                if let Some(name) = v
                    .get("metadata")
                    .and_then(|m| m.get("name"))
                    .or_else(|| v.get("name"))
                    .and_then(|n| n.as_str())
                {
                    let mut i = Item::new(
                        name,
                        "Modrinth Profile",
                        "Modrinth legacy metadata — opens app only",
                        app.map(|i| i.action.clone())
                            .unwrap_or(Action::Unavailable("Modrinth app entry not found".into())),
                    );
                    i.id = format!("modrinth:{}", e.path().display());
                    out.push(i);
                }
            }
        }
    }
    Ok(out)
}
pub fn safe_document(p: &Path) -> bool {
    // Conservative allowlist: default associations for these document formats only.
    p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        [
            "txt", "md", "pdf", "doc", "docx", "xls", "xlsx", "csv", "ppt", "pptx", "png", "jpg",
            "jpeg", "gif", "webp", "mp3", "mp4", "wav", "flac", "odt", "ods", "rtf",
        ]
        .contains(&s.to_lowercase().as_str())
    })
}
fn files(c: &Config, cancel: &AtomicBool) -> Result<Vec<Item>> {
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(6);
    let cap = c.max_files.min(50_000);
    let depth = c.max_depth.min(6);
    for (id, name) in [
        (&FOLDERID_Desktop, "Desktop"),
        (&FOLDERID_Downloads, "Downloads"),
        (&FOLDERID_Documents, "Documents"),
    ] {
        let root = known(id)?;
        if !local_path(&root) {
            continue;
        }
        out.push(Item::new(
            name,
            "Folder",
            "Windows known folder",
            Action::Folder(root.clone()),
        ));
        for p in walk(
            &root,
            depth,
            cap.saturating_sub(out.len()),
            deadline,
            cancel,
        ) {
            let dir = p.is_dir();
            if dir || safe_document(&p) {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                out.push(Item::new(
                    &name,
                    if dir { "Folder" } else { "File" },
                    "Common-folder filename",
                    if dir {
                        Action::Folder(p.clone())
                    } else {
                        Action::Document(p.clone())
                    },
                ));
            }
        }
    }
    Ok(out)
}
#[derive(Serialize, Default)]
pub struct Discovery {
    pub items: Vec<Item>,
    pub providers: Vec<Provider>,
}
#[derive(Serialize)]
pub struct Provider {
    pub name: String,
    pub count: usize,
    pub milliseconds: f64,
    pub error: Option<String>,
}
impl Discovery {
    fn add(&mut self, name: &str, f: impl FnOnce() -> Result<Vec<Item>>) {
        let now = Instant::now();
        let result = f();
        let (count, error) = match result {
            Ok(items) => {
                let n = items.len();
                self.items.extend(items);
                (n, None)
            }
            Err(e) => (0, Some(format!("{e:#}"))),
        };
        self.providers.push(Provider {
            name: name.into(),
            count,
            milliseconds: now.elapsed().as_secs_f64() * 1000.,
            error,
        });
    }
}
pub fn discover(c: &Config, cancel: &AtomicBool) -> Result<Discovery> {
    let _com = Com::new()?;
    let mut d = Discovery::default();
    d.add("Windows shortcuts", || windows_apps(cancel));
    d.add("Packaged applications", || packaged(cancel));
    d.add("Steam", || steam(c, cancel));
    let apps = d.items.clone();
    d.add("Modrinth (open app only)", || modrinth(c, &apps, cancel));
    d.add("Common folders", || files(c, cancel));
    for item in &mut d.items {
        if let Some(a) = c.user_aliases.get(&item.id) {
            item.aliases
                .extend(a.iter().map(|s| crate::model::normalize(s)));
        }
    }
    Ok(d)
}
