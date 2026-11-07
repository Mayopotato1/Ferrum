use semver::Version;
use serde_json::{Value, json};
use spotlight_prototype::updates::*;
use std::{cell::Cell, io};
fn source(channel: Channel) -> Source {
    Source::new("fixture-owner", "fixture-repo", channel).unwrap()
}
fn installed(v: &str) -> Version {
    Version::parse(v).unwrap()
}
fn record(tag: &str) -> Value {
    json!({"tag_name":tag,"draft":false,"prerelease":tag.contains('-'),"html_url":source(Channel::Beta).release_url(tag).unwrap(),"published_at":"2026-11-07T00:00:00Z","body":"ignored","assets":[]})
}
fn pick(records: Vec<Value>, channel: Channel, current: &str) -> Option<Release> {
    select(
        &serde_json::to_vec(&records).unwrap(),
        &source(channel),
        &installed(current),
    )
    .unwrap()
}
#[test]
fn beta_to_beta() {
    assert_eq!(
        pick(vec![record("v0.1.0-beta.2")], Channel::Beta, "0.1.0-beta.1")
            .unwrap()
            .tag,
        "v0.1.0-beta.2"
    );
}
#[test]
fn beta_to_stable() {
    assert!(pick(vec![record("v0.1.0")], Channel::Beta, "0.1.0-beta.2").is_some());
}
#[test]
fn stable_excludes_prerelease() {
    assert!(pick(vec![record("v0.2.0-beta.1")], Channel::Stable, "0.1.0").is_none());
}
#[test]
fn older_equal_and_build_metadata_are_not_updates() {
    for tag in ["v0.0.9", "v0.1.0-beta.1", "v0.1.0", "v0.1.0+build.9"] {
        assert!(pick(vec![record(tag)], Channel::Beta, "0.1.0+local.1").is_none());
    }
}
#[test]
fn newer_build_metadata_is_valid() {
    assert!(pick(vec![record("v0.1.1+build.9")], Channel::Stable, "0.1.0").is_some());
}
#[test]
fn mixed_order_multiple_compatible_empty() {
    assert!(pick(vec![], Channel::Beta, "0.1.0-beta.1").is_none());
    assert_eq!(
        pick(
            vec![
                record("v0.1.0-beta.10"),
                record("v0.1.0-beta.2"),
                record("v0.1.0"),
                record("v0.0.9")
            ],
            Channel::Beta,
            "0.1.0-beta.1"
        )
        .unwrap()
        .tag,
        "v0.1.0"
    );
}
#[test]
fn draft_unpublished_malformed_and_inconsistent_records_ignored() {
    let mut draft = record("v0.1.1");
    draft["draft"] = json!(true);
    let mut unpublished = record("v0.1.1");
    unpublished["published_at"] = Value::Null;
    let mut inconsistent = record("v0.1.1");
    inconsistent["prerelease"] = json!(true);
    assert!(
        pick(
            vec![
                draft,
                unpublished,
                inconsistent,
                json!({"tag_name":"garbage"}),
                json!(null),
                json!({"tag_name":2})
            ],
            Channel::Beta,
            "0.1.0"
        )
        .is_none()
    );
    for tag in [
        "vnot-semver",
        "v01.2.3",
        "v0.2.0-rc.1",
        "v0.2.0-beta.x",
        "v0.2.0-beta.1/../../evil",
    ] {
        assert!(version_tag(tag).is_err());
    }
}
#[test]
fn source_and_canonical_urls_are_exact() {
    let s = source(Channel::Beta);
    assert_eq!(
        s.api_url(),
        "https://api.github.com/repos/fixture-owner/fixture-repo/releases?per_page=100"
    );
    assert_eq!(
        s.release_url("v0.1.1").unwrap(),
        "https://github.com/fixture-owner/fixture-repo/releases/tag/v0.1.1"
    );
    for (o, r) in [
        ("", "repo"),
        ("owner", "../evil"),
        ("owner@evil", "repo"),
        ("owner", "repo?x"),
        ("owner", ".."),
    ] {
        assert!(Source::new(o, r, Channel::Beta).is_err());
    }
}
#[test]
fn malicious_release_urls_rejected() {
    for url in [
        "http://github.com/fixture-owner/fixture-repo/releases/tag/v0.1.1",
        "https://evil.test/fixture-owner/fixture-repo/releases/tag/v0.1.1",
        "https://github.com/other/repo/releases/tag/v0.1.1",
        "https://github.com@evil.test/fixture-owner/fixture-repo/releases/tag/v0.1.1",
        "https://github.com/fixture-owner/fixture-repo/releases/tag/v0.1.2",
        "https://github.com/fixture-owner/fixture-repo/releases/tag/v0.1.1?redirect=evil",
        "https://github.com/fixture-owner/fixture-repo/releases/tag/v0.1.1#evil",
        "https://github.com:443/fixture-owner/fixture-repo/releases/tag/v0.1.1",
    ] {
        let mut r = record("v0.1.1");
        r["html_url"] = json!(url);
        assert!(pick(vec![r], Channel::Stable, "0.1.0").is_none(), "{url}");
    }
}
struct Mock {
    calls: Cell<usize>,
    status: u16,
    body: Vec<u8>,
    error: Option<io::ErrorKind>,
    retry: Option<String>,
    reset: Option<u64>,
}
impl Mock {
    fn ok() -> Self {
        Self {
            calls: Cell::new(0),
            status: 200,
            body: serde_json::to_vec(&vec![record("v0.1.1")]).unwrap(),
            error: None,
            retry: None,
            reset: None,
        }
    }
}
impl Transport for Mock {
    fn get(&self, s: &Source) -> anyhow::Result<Response> {
        self.calls.set(self.calls.get() + 1);
        assert_eq!(s.api_url(), source(Channel::Beta).api_url());
        if let Some(e) = self.error {
            return Err(io::Error::from(e).into());
        }
        Ok(Response {
            status: self.status,
            body: self.body.clone(),
            retry_after: self.retry.clone(),
            reset: self.reset,
        })
    }
}
#[test]
fn unconfigured_source_never_calls_transport() {
    let m = Mock::ok();
    let mut p = Preferences::default();
    check(&mut p, None, &installed("0.1.0"), &m, 1).unwrap();
    assert!(!p.due(None, 1, true));
    assert_eq!(m.calls.get(), 0);
    assert_eq!(
        p.status(None, &installed("0.1.0")),
        "Update source not configured."
    );
}
#[test]
fn automatic_off_manual_on_success_and_interval() {
    let mut p = Preferences::default();
    let s = source(Channel::Stable);
    let m = Mock::ok();
    assert!(!p.automatic);
    assert!(!p.due(Some(&s), 100, false));
    assert!(p.due(Some(&s), 100, true));
    check(&mut p, Some(&s), &installed("0.1.0"), &m, 100).unwrap();
    assert_eq!(p.last_checked, Some(100));
    p.automatic = true;
    assert!(!p.due(Some(&s), 101, false));
    assert!(!p.due(Some(&s), 100 + DAY - 1, false));
    assert!(p.due(Some(&s), 100 + DAY, false));
    p.automatic = false;
    assert!(!p.due(Some(&s), 100 + 2 * DAY, false));
}
#[test]
fn empty_success_is_up_to_date_but_failure_is_not() {
    let s = source(Channel::Stable);
    let mut p = Preferences::default();
    let mut m = Mock::ok();
    m.body = b"[]".to_vec();
    check(&mut p, Some(&s), &installed("0.1.0"), &m, 100).unwrap();
    assert!(
        p.status(Some(&s), &installed("0.1.0"))
            .starts_with("Up to date")
    );
    m.error = Some(io::ErrorKind::NotConnected);
    assert!(check(&mut p, Some(&s), &installed("0.1.0"), &m, 101).is_err());
    assert!(
        p.status(Some(&s), &installed("0.1.0"))
            .starts_with("Unable to check")
    );
}
#[test]
fn timeout_and_offline_backoff() {
    for error in [io::ErrorKind::TimedOut, io::ErrorKind::NotConnected] {
        let mut m = Mock::ok();
        m.error = Some(error);
        let mut p = Preferences::default();
        let s = source(Channel::Stable);
        assert!(check(&mut p, Some(&s), &installed("0.1.0"), &m, 100).is_err());
        assert!(!p.due(Some(&s), 101, true));
        assert!(p.retry_until >= 700);
    }
}
#[test]
fn malformed_oversized_and_unsolicited_304_fail_safely() {
    let s = source(Channel::Beta);
    for (status, body) in [
        (200, b"{oops".to_vec()),
        (200, b"[] trailing".to_vec()),
        (200, vec![b' '; MAX_BODY + 1]),
        (304, vec![]),
    ] {
        let mut m = Mock::ok();
        m.status = status;
        m.body = body;
        let mut p = Preferences::default();
        assert!(check(&mut p, Some(&s), &installed("0.1.0"), &m, 100).is_err());
        assert_eq!(p.last_checked, None);
    }
    assert!(bounded_read(io::Cursor::new(vec![0; MAX_BODY + 1])).is_err());
    assert_eq!(
        bounded_read(io::Cursor::new(vec![0; MAX_BODY]))
            .unwrap()
            .len(),
        MAX_BODY
    );
}
#[test]
fn rate_limits_retry_after_and_reset() {
    for status in [403, 429] {
        let mut m = Mock::ok();
        m.status = status;
        m.retry = Some("7200".into());
        m.reset = Some(9000);
        let mut p = Preferences::default();
        let s = source(Channel::Beta);
        assert!(check(&mut p, Some(&s), &installed("0.1.0"), &m, 1000).is_err());
        assert_eq!(p.retry_until, 9000);
        assert!(!p.due(Some(&s), 8999, true));
        assert!(p.due(Some(&s), 9000, true));
    }
    assert_eq!(
        retry_delay(Some("Thu, 01 Jan 1970 03:00:00 GMT"), None, 1000),
        9800
    );
    assert_eq!(retry_delay(Some("malformed"), None, 1000), 3600);
}
#[test]
fn rollback_defers_and_does_not_disable_forever() {
    let mut p = Preferences {
        automatic: true,
        last_attempt: Some(10000),
        ..Default::default()
    };
    p.normalize_clock(100);
    assert!(!p.due(Some(&source(Channel::Beta)), 101, false));
    assert!(p.due(Some(&source(Channel::Beta)), 100 + DAY, false));
}
#[test]
fn notification_dismissal_new_version_and_browser_preparation() {
    let s = source(Channel::Beta);
    let v = installed("0.1.0-beta.1");
    let mut p = Preferences::default();
    check(&mut p, Some(&s), &v, &Mock::ok(), 100).unwrap();
    let r = p.notification(Some(&s), &v).unwrap().clone();
    let action = browser_action(&s, &r, &v).unwrap();
    assert_eq!(
        spotlight_prototype::actions::prepare(&action).unwrap(),
        spotlight_prototype::actions::Prepared::Shell(s.release_url("v0.1.1").unwrap())
    );
    p.dismissed = Some(r.tag);
    assert!(p.notification(Some(&s), &v).is_none());
    p.release = Some(Release {
        tag: "v0.1.2".into(),
        url: s.release_url("v0.1.2").unwrap(),
    });
    assert!(p.notification(Some(&s), &v).is_some());
    assert!(p.notification(Some(&s), &installed("0.1.2")).is_none());
    p.source = "other".into();
    assert!(p.notification(Some(&s), &v).is_none());
}
#[test]
fn update_preferences_migrate_and_persist_without_touching_other_fields() {
    use spotlight_prototype::preferences::{Change, Document};
    let path = std::env::temp_dir().join(format!("ferrum-update-test-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{"hotkey":"Alt+Space","future":{"keep":1},"max_depth":9}"#,
    )
    .unwrap();
    let mut d = Document::open(path.clone()).unwrap();
    assert!(!d.config.updates.automatic);
    let s = source(Channel::Beta);
    let mut p = Preferences {
        automatic: true,
        dismissed: Some("v0.1.1".into()),
        ..Default::default()
    };
    check(&mut p, Some(&s), &installed("0.1.0"), &Mock::ok(), 100).unwrap();
    d.update(Change::Updates(p)).unwrap();
    let loaded = Document::open(path.clone()).unwrap();
    assert!(loaded.config.updates.automatic);
    assert_eq!(loaded.config.updates.last_checked, Some(100));
    assert_eq!(loaded.config.updates.dismissed.as_deref(), Some("v0.1.1"));
    assert_eq!(loaded.config.max_depth, 9);
    let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(raw["future"]["keep"], 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn malformed_publication_time_and_excessive_record_count_rejected() {
    let mut r = record("v0.1.1");
    r["published_at"] = json!("not a date");
    assert!(pick(vec![r], Channel::Stable, "0.1.0").is_none());
    let body = serde_json::to_vec(&vec![record("v0.1.1"); 101]).unwrap();
    assert!(select(&body, &source(Channel::Stable), &installed("0.1.0")).is_err());
}

#[test]
fn compiled_source_can_be_inspected_without_gui_or_profile_io() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ferrum"))
        .arg("--update-source")
        .output()
        .unwrap();
    assert!(output.status.success());
    let info: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(info["configured"], Source::configured().is_some());
    assert_eq!(info["local_fixture"], cfg!(feature = "update-fixture"));
}

#[cfg(feature = "update-fixture")]
#[test]
fn local_fixture_transport_reads_only_local_json_and_reports_missing_file() {
    let transport = LocalFixture(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/update-newer.json"),
    );
    let s = source(Channel::Beta);
    let mut p = Preferences::default();
    check(
        &mut p,
        Some(&s),
        &installed("0.1.0-beta.1"),
        &transport,
        100,
    )
    .unwrap();
    assert_eq!(p.release.unwrap().tag, "v0.1.0-beta.2");
    let missing = LocalFixture(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/nonexistent-fixture-for-update-test.json"),
    );
    assert!(missing.get(&s).is_err());
}

#[cfg(not(feature = "update-fixture"))]
#[test]
fn official_build_is_pinned_and_automatic_checks_stay_off() {
    let s = Source::configured().expect("Official source must be configured");
    assert_eq!(
        s.api_url(),
        "https://api.github.com/repos/Mayopotato1/Ferrum/releases?per_page=100"
    );
    assert_eq!(s.channel, Channel::Beta);
    assert!(
        !spotlight_prototype::config::Config::default()
            .updates
            .automatic
    );
    let release = Release {
        tag: "v0.1.0-beta.2".into(),
        url: "https://github.com/Mayopotato1/Ferrum/releases/tag/v0.1.0-beta.2".into(),
    };
    assert!(s.validate_release(&release, &installed("0.1.0-beta.1")));
    let foreign = Release {
        url: "https://github.com/other/Ferrum/releases/tag/v0.1.0-beta.2".into(),
        ..release
    };
    assert!(!s.validate_release(&foreign, &installed("0.1.0-beta.1")));
}
