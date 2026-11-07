//! Query interaction only. Ordinary ranking and website URL encoding remain elsewhere.
use crate::{
    config::{self, Config},
    model::{Action, Item},
};
use anyhow::{Result, bail};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub token: String,
    pub provider: String,
    config_key: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub text: String,
    pub cursor_byte: usize,
}
impl Alias {
    pub fn complete(&self) -> Completion {
        let text = format!("@{} ", self.token);
        Completion {
            cursor_byte: text.len(),
            text,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    AliasDiscovery,
    AliasAutocomplete,
    WebsiteSearch,
}
pub fn mode(query: &str) -> Mode {
    let Some(rest) = query.strip_prefix('@') else {
        return Mode::Normal;
    };
    if rest.is_empty() {
        return Mode::AliasDiscovery;
    }
    if let Some((_, text)) = rest.split_once(char::is_whitespace)
        && !text.trim().is_empty()
    {
        return Mode::WebsiteSearch;
    }
    Mode::AliasAutocomplete
}

/// Built once from the same config used by config::web; no IO per keystroke.
pub struct Aliases {
    entries: Vec<Alias>,
    config: Config,
    pub rejected: usize,
}
impl Aliases {
    pub fn new(config: &Config) -> Self {
        let mut unique = BTreeMap::new();
        let mut rejected = 0;
        for (key, template) in &config.websites {
            if key.is_empty()
                || key.len() > 64
                || !key
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
                || config::validate_template(template).is_err()
            {
                rejected += 1;
                continue;
            }
            let token = key.to_lowercase();
            let url = url::Url::parse(&template.replace("{query}", "test")).unwrap();
            let host = url.host_str().unwrap().trim_start_matches("www.");
            let provider = match host {
                "youtube.com" => "YouTube",
                "en.wikipedia.org" | "wikipedia.org" => "Wikipedia",
                "google.com" => "Google",
                "reddit.com" => "Reddit",
                "store.steampowered.com" => "Steam Store",
                "github.com" => "GitHub",
                other => other,
            }
            .to_string();
            let entry = Alias {
                token: token.clone(),
                provider,
                config_key: key.clone(),
            };
            // An exact lowercase definition wins a case-fold collision, deterministically.
            if key == &token || !unique.contains_key(&token) {
                unique.insert(token, entry);
            }
        }
        Self {
            entries: unique.into_values().collect(),
            config: config.clone(),
            rejected,
        }
    }
    pub fn suggestions(&self, query: &str) -> Vec<Alias> {
        if !matches!(mode(query), Mode::AliasDiscovery | Mode::AliasAutocomplete) {
            return vec![];
        }
        let prefix = query.strip_prefix('@').unwrap().trim().to_lowercase();
        self.entries
            .iter()
            .filter(|a| {
                a.token.starts_with(&prefix) || a.provider.to_lowercase().starts_with(&prefix)
            })
            .cloned()
            .collect()
    }
    pub fn search(&self, query: &str) -> Result<Item> {
        if mode(query) != Mode::WebsiteSearch {
            bail!("Choose an @ command, then type search words");
        }
        let (token, text) = query
            .strip_prefix('@')
            .unwrap()
            .split_once(char::is_whitespace)
            .unwrap();
        let token_folded = token.to_lowercase();
        let Some(entry) = self.entries.iter().find(|a| a.token == token_folded) else {
            bail!("Unknown or unavailable website alias @{token}");
        };
        // Keep existing template validation, encoding and typed website action preparation.
        let canonical = format!("@{} {text}", entry.config_key);
        let (name, url) = config::web(&self.config, &canonical)?.unwrap();
        Ok(Item::new(
            &name,
            "Website search",
            "HTTPS search template",
            Action::Website(url),
        ))
    }
}

#[derive(Debug, Clone)]
pub enum Selection {
    Launch(Item),
    Complete(Alias),
}
#[derive(Debug, Clone, Copy)]
pub enum Accept {
    Tab,
    Enter,
    Click,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Complete(Completion),
    Launch,
    None,
}
/// Shared by Tab, Enter and row clicks; no action is executed by this layer.
pub fn intent(selection: Option<&Selection>, key: Accept) -> Intent {
    match selection {
        Some(Selection::Complete(alias)) => Intent::Complete(alias.complete()),
        Some(Selection::Launch(_)) if matches!(key, Accept::Enter) => Intent::Launch,
        _ => Intent::None,
    }
}
