//! Opt-in release notification only. No assets, credentials, or installer operations.
use anyhow::{Result, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const DAY: u64 = 86_400;
pub const MAX_BODY: usize = 1_048_576;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}
#[derive(Clone, Debug)]
pub struct Source {
    owner: String,
    repo: String,
    pub channel: Channel,
}
impl Source {
    pub fn new(owner: &str, repo: &str, channel: Channel) -> Result<Self> {
        let valid = |s: &str| {
            !s.is_empty()
                && s.len() <= 100
                && s != "."
                && s != ".."
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        };
        ensure!(valid(owner) && valid(repo), "Invalid update source");
        Ok(Self {
            owner: owner.into(),
            repo: repo.into(),
            channel,
        })
    }
    pub fn configured() -> Option<Self> {
        Self::new(
            env!("FERRUM_UPDATE_OWNER"),
            env!("FERRUM_UPDATE_REPO"),
            if env!("FERRUM_UPDATE_CHANNEL") == "beta" {
                Channel::Beta
            } else {
                Channel::Stable
            },
        )
        .ok()
    }
    pub fn api_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=100",
            self.owner, self.repo
        )
    }
    pub fn release_url(&self, tag: &str) -> Result<String> {
        version_tag(tag)?;
        let mut u = url::Url::parse(&format!(
            "https://github.com/{}/{}/releases/tag/",
            self.owner, self.repo
        ))?;
        u.path_segments_mut().unwrap().pop_if_empty().push(tag);
        Ok(u.into())
    }
    pub fn validate_release(&self, r: &Release, installed: &Version) -> bool {
        let Ok(v) = version_tag(&r.tag) else {
            return false;
        };
        let beta = !v.pre.is_empty();
        v.cmp_precedence(installed).is_gt()
            && (!beta || self.channel == Channel::Beta)
            && self.release_url(&r.tag).is_ok_and(|u| u == r.url)
    }
}
pub fn version_tag(tag: &str) -> Result<Version> {
    ensure!(tag.len() <= 128, "Version tag too long");
    let v = Version::parse(tag.strip_prefix('v').unwrap_or(tag))?;
    // Ferrum currently publishes stable and beta tags; unrelated preview channels are ignored.
    ensure!(
        v.pre.is_empty()
            || (v.pre.as_str().starts_with("beta.") && v.pre.as_str()[5..].parse::<u64>().is_ok()),
        "Unrecognized Ferrum release channel"
    );
    Ok(v)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub url: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub automatic: bool,
    pub last_attempt: Option<u64>,
    pub last_checked: Option<u64>,
    pub retry_until: u64,
    pub failures: u8,
    pub release: Option<Release>,
    pub dismissed: Option<String>,
    pub source: String,
}
impl Preferences {
    pub fn due(&self, source: Option<&Source>, now: u64, manual: bool) -> bool {
        source.is_some()
            && (manual || self.automatic)
            && now >= self.retry_until
            && (manual
                || self
                    .last_attempt
                    .is_none_or(|last| now.checked_sub(last).is_some_and(|d| d >= DAY)))
    }
    /// Recover from wall-clock rollback conservatively: wait a fresh day, not forever.
    pub fn normalize_clock(&mut self, now: u64) {
        if self.last_attempt.is_some_and(|last| last > now) {
            self.last_attempt = Some(now);
            let delay = self.retry_until.saturating_sub(now).clamp(DAY, 7 * DAY);
            self.retry_until = now.saturating_add(delay);
        }
    }
    pub fn applicable(&self, source: Option<&Source>, installed: &Version) -> Option<&Release> {
        let s = source?;
        self.release
            .as_ref()
            .filter(|r| self.source == s.api_url() && s.validate_release(r, installed))
    }
    pub fn notification(&self, source: Option<&Source>, installed: &Version) -> Option<&Release> {
        self.applicable(source, installed)
            .filter(|r| self.dismissed.as_deref() != Some(&r.tag))
    }
    pub fn status(&self, source: Option<&Source>, installed: &Version) -> String {
        let Some(s) = source else {
            return "Update source not configured.".into();
        };
        if let Some(r) = self.applicable(Some(s), installed) {
            return format!("Update available: {}", r.tag);
        }
        if self.failures > 0 {
            return "Unable to check. Try again after the retry delay.".into();
        }
        if self.last_checked.is_some() && self.source == s.api_url() {
            return "Up to date (last successful check).".into();
        }
        if !self.automatic {
            "Update checking disabled.".into()
        } else {
            "Waiting for the next update check.".into()
        }
    }
}
#[derive(Deserialize)]
struct Record {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    html_url: String,
    published_at: Option<String>,
}
/// Deserialize one record at a time; ignore bodies, assets and malformed records.
pub fn select(body: &[u8], source: &Source, installed: &Version) -> Result<Option<Release>> {
    ensure!(body.len() <= MAX_BODY, "Release response too large");
    struct List<'a> {
        source: &'a Source,
        installed: &'a Version,
    }
    impl<'de> serde::de::Visitor<'de> for List<'_> {
        type Value = Option<Release>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("release list")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut best: Option<(Version, Release)> = None;
            let mut count = 0;
            while let Some(value) = seq.next_element::<serde_json::Value>()? {
                count += 1;
                if count > 100 {
                    return Err(serde::de::Error::custom("Too many releases"));
                }
                let Ok(raw) = serde_json::from_value::<Record>(value) else {
                    continue;
                };
                let Ok(v) = version_tag(&raw.tag_name) else {
                    continue;
                };
                let r = Release {
                    tag: raw.tag_name,
                    url: raw.html_url,
                };
                if raw.draft
                    || raw
                        .published_at
                        .as_deref()
                        .is_none_or(|s| chrono::DateTime::parse_from_rfc3339(s).is_err())
                    || raw.prerelease != !v.pre.is_empty()
                    || !self.source.validate_release(&r, self.installed)
                {
                    continue;
                }
                if best
                    .as_ref()
                    .is_none_or(|(b, _)| v.cmp_precedence(b).is_gt())
                {
                    best = Some((v, r));
                }
            }
            Ok(best.map(|(_, r)| r))
        }
    }
    let mut de = serde_json::Deserializer::from_slice(body);
    let selected = serde::de::Deserializer::deserialize_seq(&mut de, List { source, installed })?;
    de.end()?;
    Ok(selected)
}
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub retry_after: Option<String>,
    pub reset: Option<u64>,
}
pub trait Transport {
    fn get(&self, source: &Source) -> Result<Response>;
}
pub struct Https;
impl Transport for Https {
    fn get(&self, source: &Source) -> Result<Response> {
        // Native TLS uses Windows' trust store. Redirects are forbidden; source is build-pinned.
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(8))
            .pool_max_idle_per_host(0)
            .build()?;
        let response = client
            .get(source.api_url())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .header("User-Agent", concat!("Ferrum/", env!("CARGO_PKG_VERSION")))
            .send()?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let reset = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());
        ensure!(
            status != 200
                || response
                    .content_length()
                    .is_none_or(|n| n <= MAX_BODY as u64),
            "Release response too large"
        );
        let body = if status == 200 {
            bounded_read(response)?
        } else {
            Vec::new()
        };
        Ok(Response {
            status,
            body,
            retry_after,
            reset,
        })
    }
}
pub fn bounded_read(reader: impl Read) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    reader.take(MAX_BODY as u64 + 1).read_to_end(&mut body)?;
    ensure!(body.len() <= MAX_BODY, "Release response too large");
    Ok(body)
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn retry_delay(value: Option<&str>, reset: Option<u64>, now: u64) -> u64 {
    let after = value
        .and_then(|v| {
            v.parse::<u64>().ok().or_else(|| {
                httpdate::parse_http_date(v)
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_secs()
                    .checked_sub(now)
            })
        })
        .unwrap_or(0);
    after.max(reset.unwrap_or(0).saturating_sub(now)).max(3600)
}
/// One request at most; callers persist the attempt before performing network IO.
pub fn check(
    p: &mut Preferences,
    source: Option<&Source>,
    installed: &Version,
    transport: &impl Transport,
    now: u64,
) -> Result<()> {
    let Some(source) = source else {
        return Ok(());
    };
    p.last_attempt = Some(now);
    let result = transport.get(source).and_then(|r| {
        if r.status == 403 || r.status == 429 {
            p.retry_until = now.saturating_add(retry_delay(r.retry_after.as_deref(), r.reset, now));
        }
        ensure!(r.status == 200, "GitHub returned HTTP {}", r.status);
        select(&r.body, source, installed)
    });
    match result {
        Ok(release) => {
            p.release = release;
            p.source = source.api_url();
            p.last_checked = Some(now);
            p.failures = 0;
            p.retry_until = 0;
            Ok(())
        }
        Err(e) => {
            p.failures = p.failures.saturating_add(1);
            p.retry_until = p
                .retry_until
                .max(now.saturating_add((300u64 << p.failures.min(8)).min(DAY)));
            Err(e)
        }
    }
}
pub fn browser_action(
    source: &Source,
    release: &Release,
    installed: &Version,
) -> Result<crate::model::Item> {
    ensure!(
        source.validate_release(release, installed),
        "Untrusted or obsolete release"
    );
    Ok(crate::model::Item::new(
        "View Ferrum update",
        "Website",
        "Official GitHub release",
        crate::model::Action::OpenUrl(source.release_url(&release.tag)?),
    ))
}

/// Only compiled into explicitly requested debug playtest builds. Never performs HTTP.
#[cfg(feature = "update-fixture")]
pub struct LocalFixture(pub std::path::PathBuf);
#[cfg(feature = "update-fixture")]
impl Transport for LocalFixture {
    fn get(&self, _: &Source) -> Result<Response> {
        Ok(Response {
            status: 200,
            body: bounded_read(std::fs::File::open(&self.0)?)?,
            retry_after: None,
            reset: None,
        })
    }
}
