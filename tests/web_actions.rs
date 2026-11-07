use spotlight_prototype::{
    actions::{self, Prepared},
    config::Config,
    model::{Action, Item},
    web::{self, SearchProvider as P},
};

#[test]
fn providers_generate_encoded_https_searches_without_io() {
    for (p, host, path) in [
        (P::Google, "www.google.com", "/search"),
        (P::DuckDuckGo, "duckduckgo.com", "/"),
        (P::Bing, "www.bing.com", "/search"),
        (P::Brave, "search.brave.com", "/search"),
    ] {
        let item = p.search(" café & 日本語 + C++?!#% ").unwrap().unwrap();
        assert!(matches!(item.action, Action::WebSearch(_)));
        let Prepared::Shell(uri) = actions::prepare(&item).unwrap() else {
            panic!()
        };
        let url = url::Url::parse(&uri).unwrap();
        assert_eq!(url.host_str(), Some(host));
        assert_eq!(url.path(), path);
        assert_eq!(url.scheme(), "https");
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            [("q".into(), "café & 日本語 + C++?!#%".into())]
        );
    }
    assert_eq!(
        actions::prepare(&P::Google.search("minecraft shaders").unwrap().unwrap()).unwrap(),
        Prepared::Shell("https://www.google.com/search?q=minecraft+shaders".into())
    );
}
#[test]
fn empty_bounded_and_backward_compatible_configuration() {
    for text in ["", "  \t\n", "...", "???"] {
        assert!(P::Google.search(text).unwrap().is_none());
    }
    assert!(
        P::Google
            .search(&"x".repeat(web::MAX_QUERY_BYTES + 1))
            .is_err()
    );
    let old: Config = serde_json::from_str("{\"hotkey\":\"Alt+Space\"}").unwrap();
    assert_eq!(old.search_provider, P::Google);
    assert!(old.translucent_background);
    let custom: Config = serde_json::from_str("{\"search_provider\":\"duckduckgo\"}").unwrap();
    assert_eq!(custom.search_provider, P::DuckDuckGo);
    assert!(serde_json::from_str::<Config>("{\"search_provider\":\"shell\"}").is_err());
}
#[test]
fn direct_addresses_support_paths_queries_idna_and_explicit_http() {
    for (text, expected) in [
        ("github.com", "https://github.com/"),
        ("docs.rs", "https://docs.rs/"),
        ("www.wikipedia.org", "https://www.wikipedia.org/"),
        ("https://rust-lang.org", "https://rust-lang.org/"),
        (
            "HTTP://example.com/test?q=a%2Bb#hi",
            "http://example.com/test?q=a%2Bb#hi",
        ),
        (
            "sub.example.com/a%20b?q=x%26y",
            "https://sub.example.com/a%20b?q=x%26y",
        ),
        (
            "bücher.de/日本語",
            "https://xn--bcher-kva.de/%E6%97%A5%E6%9C%AC%E8%AA%9E",
        ),
    ] {
        let url = web::detect_url(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(url.as_str(), expected);
        let item = web::open_item(url);
        assert!(matches!(item.action, Action::OpenUrl(_)));
        assert_eq!(
            actions::prepare(&item).unwrap(),
            Prepared::Shell(expected.into())
        );
    }
}
#[test]
fn dotted_filenames_phrases_and_unsafe_schemes_are_not_direct_actions() {
    for text in [
        "report.pdf",
        "notes.txt",
        "settings.json",
        "photo.jpg",
        "file.exe",
        "archive.zip",
        "v1.2 release notes",
        "best example.com tips",
        "C:\\Users\\file.txt",
        "javascript:alert(1)",
        "file:///C:/test.txt",
        "data:text/html,test",
        "shell:AppsFolder",
        "ms-settings:display",
        "steam://rungameid/620",
        "custom://host",
        "https://user:pass@example.com",
        "https://example.com/invalid%XX",
        "https://exa_mple.com/path words",
    ] {
        assert!(web::detect_url(text).is_none(), "{text}");
    }
    for text in [
        "javascript:alert(1)",
        "file:///C:/test.txt",
        "steam://rungameid/620",
        "https://user@example.com",
        "http://example.com/\nfoo",
    ] {
        let item = Item::new("Bad", "Website", "fixture", Action::OpenUrl(text.into()));
        assert!(actions::prepare(&item).is_err());
    }
    let alias = Item::new(
        "Alias",
        "Website search",
        "fixture",
        Action::Website("http://example.com/".into()),
    );
    assert!(actions::prepare(&alias).is_err());
}
#[test]
fn crafted_general_search_actions_fail_validation() {
    for uri in [
        "https://evil.example/search?q=hi",
        "http://www.google.com/search?q=hi",
        "https://www.google.com/search?q=",
        "https://www.google.com/search?q=a&q=b",
        "https://www.google.com/search?q=a&other=b",
        "https://www.google.com/search?q=a#fragment",
    ] {
        assert!(
            actions::prepare(&Item::new(
                "Bad",
                "General web search",
                "fixture",
                Action::WebSearch(uri.into())
            ))
            .is_err(),
            "{uri}"
        );
    }
}

#[test]
fn unicode_url_expansion_is_bounded_without_rejecting_valid_encoded_output() {
    let input = format!("https://example.com/{}", "日".repeat(1000));
    assert!(input.len() < web::MAX_QUERY_BYTES);
    let item = web::open_item(web::detect_url(&input).unwrap());
    let Prepared::Shell(uri) = actions::prepare(&item).unwrap() else {
        panic!()
    };
    assert!(uri.len() > web::MAX_QUERY_BYTES && uri.len() < web::MAX_URL_BYTES);
}
