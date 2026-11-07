//! Local URL construction/classification. This module performs no network or IO.
use crate::model::{Action, Item};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use url::Url;

pub const MAX_QUERY_BYTES: usize = 4096;
pub const MAX_URL_BYTES: usize = MAX_QUERY_BYTES * 3 + 128;
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchProvider {
    #[default]
    Google,
    DuckDuckGo,
    Bing,
    Brave,
}
impl SearchProvider {
    pub const ALL: [Self; 4] = [Self::Google, Self::DuckDuckGo, Self::Bing, Self::Brave];
    pub fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Bing => "Bing",
            Self::Brave => "Brave Search",
        }
    }
    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Google => "https://www.google.com/search",
            Self::DuckDuckGo => "https://duckduckgo.com/",
            Self::Bing => "https://www.bing.com/search",
            Self::Brave => "https://search.brave.com/search",
        }
    }
    pub fn search(self, text: &str) -> Result<Option<Item>> {
        let Some(text) = meaningful(text)? else {
            return Ok(None);
        };
        let mut url = Url::parse(self.endpoint())?;
        url.query_pairs_mut().append_pair("q", text);
        Ok(Some(Item::new(
            &format!("Search the web for \"{text}\""),
            "General web search",
            self.name(),
            Action::WebSearch(url.into()),
        )))
    }
}
pub fn meaningful(text: &str) -> Result<Option<&str>> {
    ensure!(
        text.len() <= MAX_QUERY_BYTES,
        "Query is too long (maximum {MAX_QUERY_BYTES} UTF-8 bytes)"
    );
    let text = text.trim();
    if text.is_empty()
        || !text.chars().any(|c| {
            c.is_alphanumeric() || (!c.is_ascii() && !c.is_whitespace() && !c.is_control())
        })
    {
        return Ok(None);
    }
    Ok(Some(text))
}
fn safe_characters(text: &str) -> bool {
    if text.is_empty()
        || text
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return false;
    }
    let bytes = text.as_bytes();
    bytes.iter().enumerate().all(|(i, c)| {
        *c != b'%'
            || (bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
    })
}
/// Canonicalize only explicitly addressed HTTP(S) sites; never allow shell/file/app protocols.
pub fn validate_open_url(text: &str) -> Result<Url> {
    ensure!(text.len() <= MAX_URL_BYTES, "Website URL is too long");
    ensure!(
        safe_characters(text),
        "Invalid website URL characters or length"
    );
    let (scheme, _) = text
        .split_once("://")
        .ok_or_else(|| anyhow::anyhow!("Website URL needs HTTP or HTTPS"))?;
    ensure!(
        scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"),
        "Only HTTP and HTTPS website URLs are permitted"
    );
    let url = Url::parse(text)?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "Invalid website URL or embedded credentials"
    );
    Ok(url)
}
/// Conservative schemeless recognition: common public suffixes/IDNA, not arbitrary dotted filenames.
/// No DNS requests. Unlisted suffixes can always be opened with an explicit HTTP(S) URL.
pub fn detect_url(text: &str) -> Option<Url> {
    let text = text.trim();
    if text.len() > MAX_QUERY_BYTES || !safe_characters(text) {
        return None;
    }
    if text.contains("://") {
        return validate_open_url(text).ok();
    }
    let candidate = validate_open_url(&format!("https://{text}")).ok()?;
    let url::Host::Domain(host) = candidate.host()? else {
        return None;
    };
    let host = host.trim_end_matches('.');
    let labels: Vec<_> = host.split('.').collect();
    if labels.len() < 2
        || host.len() > 253
        || !labels.iter().all(|s| {
            !s.is_empty()
                && s.len() <= 63
                && !s.starts_with('-')
                && !s.ends_with('-')
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return None;
    }
    let suffix = *labels.last()?;
    if ![
        "com", "org", "net", "edu", "gov", "io", "dev", "app", "rs", "co", "nz", "uk", "au", "de",
        "fr", "jp", "ca", "us", "me", "info", "tech", "gg", "ai", "tv", "xyz", "online", "site",
    ]
    .contains(&suffix)
        && !suffix.starts_with("xn--")
    {
        return None;
    }
    Some(candidate)
}
pub fn open_item(url: Url) -> Item {
    Item::new(
        &format!("Open {}", url.host_str().unwrap_or("website")),
        "Website",
        "Web",
        Action::OpenUrl(url.into()),
    )
}
pub fn validate_search_url(text: &str) -> Result<Url> {
    // Encoded output can be up to three times the input byte bound.
    ensure!(text.len() <= MAX_URL_BYTES, "Search URL is too long");
    let url = Url::parse(text)?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "Invalid HTTPS search action"
    );
    let valid = SearchProvider::ALL.iter().any(|p| {
        let endpoint = Url::parse(p.endpoint()).unwrap();
        url.origin() == endpoint.origin() && url.path() == endpoint.path()
    });
    ensure!(valid, "Unknown search endpoint");
    let pairs: Vec<_> = url.query_pairs().collect();
    ensure!(
        pairs.len() == 1 && pairs[0].0 == "q",
        "Search action needs one q parameter"
    );
    if meaningful(&pairs[0].1)?.is_none() {
        bail!("Search query is empty");
    }
    Ok(url)
}
