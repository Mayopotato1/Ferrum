use spotlight_prototype::{
    actions::{self, Prepared},
    config::Config,
    query::{self, Accept, Aliases, Intent, Mode, Selection},
    search::Index,
    validation,
};

#[test]
fn all_defaults_come_from_config_and_filter_deterministically() {
    let config = Config::default();
    let aliases = Aliases::new(&config);
    let all = aliases.suggestions("@");
    assert_eq!(all.len(), config.websites.len());
    assert_eq!(
        all.iter().map(|a| a.token.as_str()).collect::<Vec<_>>(),
        ["google", "reddit", "steam", "wiki", "yt"]
    );
    for (q, expected) in [
        ("@y", "yt"),
        ("@w", "wiki"),
        ("@YT", "yt"),
        ("@you", "yt"),
        ("@WIKIPEDIA", "wiki"),
    ] {
        assert_eq!(aliases.suggestions(q)[0].token, expected);
    }
    assert!(aliases.suggestions("@unknown").is_empty());
}
#[test]
fn custom_entries_are_available_and_invalid_templates_or_tokens_are_excluded() {
    let mut c = Config::default();
    c.websites
        .insert("GH".into(), "https://github.com/search?q={query}".into());
    c.websites
        .insert("bad".into(), "javascript:run({query})".into());
    c.websites
        .insert("missing".into(), "https://example.com/search".into());
    c.websites
        .insert("has space".into(), "https://example.com/?q={query}".into());
    c.websites
        .insert("malformed".into(), "https:example.com/{query}".into());
    let a = Aliases::new(&c);
    assert_eq!(a.rejected, 4);
    assert_eq!(a.suggestions("@gH")[0].provider, "GitHub");
    assert_eq!(a.suggestions("@").len(), 6);
    assert!(a.search("@bad test").is_err());
    assert!(
        matches!(actions::prepare(&a.search("@gh rust").unwrap()),Ok(Prepared::Shell(url)) if url=="https://github.com/search?q=rust")
    );
}
#[test]
fn tab_enter_and_click_complete_without_launching_and_put_cursor_at_end() {
    let a = Aliases::new(&Config::default());
    for q in ["@y", "@yt", "@YT", "@yt ", "@yt    "] {
        let selected = Selection::Complete(a.suggestions(q)[0].clone());
        for key in [Accept::Tab, Accept::Enter, Accept::Click] {
            let Intent::Complete(result) = query::intent(Some(&selected), key) else {
                panic!("must only complete")
            };
            assert_eq!(result.text, "@yt ");
            assert_eq!(result.cursor_byte, 4);
            assert_eq!(result.text.as_bytes()[result.cursor_byte - 1], b' ');
        }
    }
    assert_eq!(query::intent(None, Accept::Enter), Intent::None);
}
#[test]
fn complete_queries_use_existing_url_encoding_and_never_launch_empty_or_unknown() {
    let a = Aliases::new(&Config::default());
    let action = a.search("@YT minecraft shaders").unwrap();
    assert_eq!(
        actions::prepare(&action).unwrap(),
        Prepared::Shell("https://www.youtube.com/results?search_query=minecraft+shaders".into())
    );
    let selected = Selection::Launch(action);
    assert_eq!(
        query::intent(Some(&selected), Accept::Enter),
        Intent::Launch
    );
    assert_eq!(query::intent(Some(&selected), Accept::Tab), Intent::None);
    for q in ["@yt", "@yt ", "@yt \t", "@unknown", "@unknown query"] {
        assert!(a.search(q).is_err(), "{q}");
    }
    let action = a.search("@yt café & 日本語+#").unwrap();
    let Prepared::Shell(uri) = actions::prepare(&action).unwrap() else {
        panic!()
    };
    assert_eq!(
        url::Url::parse(&uri)
            .unwrap()
            .query_pairs()
            .next()
            .unwrap()
            .1,
        "café & 日本語+#"
    );
}
#[test]
fn unicode_alias_completion_uses_utf8_byte_offsets_and_normal_search_is_untouched() {
    let mut c = Config::default();
    c.websites
        .insert("日本語".into(), "https://example.com/?q={query}".into());
    let a = Aliases::new(&c);
    let result = a.suggestions("@日")[0].complete();
    assert_eq!(result.cursor_byte, result.text.len());
    assert!(result.text.is_char_boundary(result.cursor_byte));
    assert_eq!(query::mode("@"), Mode::AliasDiscovery);
    assert_eq!(query::mode("@yt "), Mode::AliasAutocomplete);
    assert_eq!(query::mode("@yt abc"), Mode::WebsiteSearch);
    let index = Index::new(validation::fixture());
    for q in ["spotify", "minecrfat", "cs2"] {
        assert_eq!(query::mode(q), Mode::Normal);
        assert!(!index.search(q, 3).is_empty());
        assert!(a.suggestions(q).is_empty());
    }
}
