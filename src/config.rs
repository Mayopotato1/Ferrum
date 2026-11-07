use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub updates: crate::updates::Preferences,
    pub search_provider: crate::web::SearchProvider,
    pub translucent_background: bool,
    pub background_opacity: u8,
    pub launch_on_startup: bool,
    pub hotkey: String,
    pub websites: BTreeMap<String, String>,
    pub steam_roots: Vec<PathBuf>,
    pub modrinth_roots: Vec<PathBuf>,
    pub user_aliases: BTreeMap<String, Vec<String>>,
    pub max_files: usize,
    pub max_depth: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            updates: crate::updates::Preferences::default(),
            search_provider: crate::web::SearchProvider::default(),
            translucent_background: true,
            background_opacity: 92,
            launch_on_startup: false,
            hotkey: "Alt+Space".into(),
            websites: [
                ("yt", "https://www.youtube.com/results?search_query={query}"),
                (
                    "wiki",
                    "https://en.wikipedia.org/w/index.php?search={query}",
                ),
                ("google", "https://www.google.com/search?q={query}"),
                ("reddit", "https://www.reddit.com/search/?q={query}"),
                (
                    "steam",
                    "https://store.steampowered.com/search/?term={query}",
                ),
            ]
            .into_iter()
            .map(|(a, b)| (a.into(), b.into()))
            .collect(),
            steam_roots: vec![],
            modrinth_roots: vec![],
            user_aliases: BTreeMap::new(),
            max_files: 20_000,
            max_depth: 4,
        }
    }
}
pub fn path() -> PathBuf {
    path_in(std::path::Path::new(
        &std::env::var_os("LOCALAPPDATA").unwrap_or_default(),
    ))
}
/// Deliberately retain the established location; no copy/delete or conflict with other apps.
pub fn path_in(local_app_data: &std::path::Path) -> PathBuf {
    local_app_data.join("SpotlightLauncherPrototype/config.json")
}
pub fn load() -> Result<Config> {
    Ok(crate::preferences::Document::open(path())?.config)
}
pub fn validate_template(t: &str) -> Result<()> {
    if t.matches("{query}").count() != 1 {
        bail!("Website template needs exactly one {{query}} placeholder");
    }
    let authority = t
        .split_once("://")
        .context("Website template needs an explicit HTTPS URL")?
        .1
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    let u = url::Url::parse(&t.replace("{query}", "test"))?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || authority.contains("{query}")
    {
        bail!("Website must use HTTPS with a fixed host and no credentials");
    }
    Ok(())
}
pub fn web(c: &Config, q: &str) -> Result<Option<(String, String)>> {
    let Some(q) = q.strip_prefix('@') else {
        return Ok(None);
    };
    let (alias, text) = q.split_once(char::is_whitespace).unwrap_or((q, ""));
    let alias_folded = alias.to_lowercase();
    let template = c
        .websites
        .get(alias)
        .or_else(|| {
            c.websites
                .iter()
                .find(|(key, _)| key.to_lowercase() == alias_folded)
                .map(|(_, value)| value)
        })
        .with_context(|| format!("Unknown website alias @{alias}"))?;
    validate_template(template)?;
    if text.trim().is_empty() {
        bail!("Type search words after @{alias}");
    }
    let encoded: String =
        url::form_urlencoded::byte_serialize(text.trim_start().as_bytes()).collect();
    Ok(Some((
        format!("Search @{alias}: {}", text.trim_start()),
        template.replace("{query}", &encoded),
    )))
}
