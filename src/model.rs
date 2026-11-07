use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use unicode_normalization::UnicodeNormalization;

pub fn normalize(s: &str) -> String {
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| {
            if c.is_alphanumeric() || c == '+' || c == '#' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Action {
    Shortcut {
        path: PathBuf,
        target: String,
        arguments: String,
        working_dir: String,
    },
    Packaged {
        aumid: String,
    },
    Steam {
        app_id: u32,
        manifest: PathBuf,
        install_dir: PathBuf,
    },
    Folder(PathBuf),
    Document(PathBuf),
    Website(String),
    WebSearch(String),
    OpenUrl(String),
    Unavailable(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub category: String,
    pub source: String,
    pub aliases: Vec<String>,
    pub normalized: String,
    pub action: Action,
    pub available: bool,
}
impl Item {
    pub fn new(name: &str, category: &str, source: &str, action: Action) -> Self {
        let normalized = normalize(name);
        let mut aliases = Vec::new();
        for (needle, alias) in [
            ("minecraft launcher", "mc"),
            ("minecraft java edition", "mc"),
            ("counter strike 2", "cs2"),
            ("team fortress 2", "tf2"),
            ("valorant", "val"),
            ("baldur s gate 3", "bg3"),
        ] {
            if normalized == needle {
                aliases.push(alias.to_string());
            }
        }
        let initials: String = normalized
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .collect();
        if initials.len() >= 2 && normalized.split_whitespace().count() > 1 {
            aliases.push(initials);
        }
        let id = serde_json::to_string(&action).unwrap();
        let available = !matches!(action, Action::Unavailable(_));
        Self {
            id,
            name: name.into(),
            category: category.into(),
            source: source.into(),
            aliases,
            normalized,
            action,
            available,
        }
    }
}
