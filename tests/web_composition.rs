use spotlight_prototype::{
    composition::{self, SubmitPlan},
    config::Config,
    model::{Action, Item},
    query::{self, Accept, Aliases, Intent, Selection},
    search::Index,
    validation,
    web::SearchProvider as P,
};
fn names(rows: &[Selection]) -> Vec<&str> {
    rows.iter()
        .filter_map(|r| {
            if let Selection::Launch(i) = r {
                Some(i.name.as_str())
            } else {
                None
            }
        })
        .collect()
}
fn is_web(row: &Selection) -> bool {
    matches!(
        row,
        Selection::Launch(Item {
            action: Action::WebSearch(_),
            ..
        })
    )
}
#[test]
fn strong_locals_aliases_multiword_games_and_typos_stay_above_web() {
    let index = Index::new(validation::fixture());
    let aliases = Aliases::new(&Config::default());
    for q in [
        "minecraft",
        "spotify",
        "downloads",
        "cs2",
        "tf2",
        "spotfy",
        "disocrd",
        "minecrfat",
        "Baldur's Gate 3",
    ] {
        let local = index.search(q, 6);
        let rows = composition::results(&index, &aliases, P::Google, q).unwrap();
        assert!(!is_web(&rows[0]), "{q}: {:?}", names(&rows));
        assert!(rows.iter().any(is_web));
        assert_eq!(names(&rows)[0], index.items[local[0].index].name);
    }
    let mut data = validation::fixture();
    let mut game = Item::new(
        "Geometry Dash",
        "Steam Game",
        "fixture",
        Action::Unavailable("fixture".into()),
    );
    game.available = true;
    data.push(game);
    let index = Index::new(data);
    let rows = composition::results(&index, &aliases, P::Google, "geometry dash").unwrap();
    assert_eq!(names(&rows)[0], "Geometry Dash");
}
#[test]
fn questions_and_sparse_fuzzy_hits_get_web_first_and_preserve_local_order() {
    let index = Index::new(validation::fixture());
    let aliases = Aliases::new(&Config::default());
    for q in [
        "best minecraft shaders",
        "how to fix discord microphone",
        "RTX 4060 benchmark",
        "what is rust programming",
        "stdew",
    ] {
        let rows = composition::results(&index, &aliases, P::Google, q).unwrap();
        if q == "stdew" {
            assert!(
                !index.search(q, 6).is_empty(),
                "fixture must exercise a real fuzzy-only match"
            );
        }
        assert!(is_web(&rows[0]), "{q}");
        let locals: Vec<_> = rows
            .iter()
            .filter_map(|r| {
                if let Selection::Launch(i) = r {
                    if !matches!(i.action, Action::WebSearch(_) | Action::OpenUrl(_)) {
                        Some(&i.id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        let expected: Vec<_> = index
            .search(q, 6)
            .iter()
            .take(locals.len())
            .map(|h| &index.items[h.index].id)
            .collect();
        assert_eq!(locals, expected);
    }
}
#[test]
fn direct_urls_are_first_and_explicit_prefix_bypasses_locals() {
    let index = Index::new(validation::fixture());
    let aliases = Aliases::new(&Config::default());
    let rows = composition::results(&index, &aliases, P::Google, "github.com").unwrap();
    assert!(
        matches!(&rows[0],Selection::Launch(Item{action:Action::OpenUrl(url),..}) if url=="https://github.com/")
    );
    let rows = composition::results(&index, &aliases, P::Google, "  ?  minecraft  ").unwrap();
    assert_eq!(rows.len(), 1);
    assert!(
        matches!(&rows[0],Selection::Launch(Item{action:Action::WebSearch(url),..}) if url=="https://www.google.com/search?q=minecraft")
    );
    assert!(composition::explicit_text("is rust fast?").is_none());
    for q in ["?", "? "] {
        assert!(composition::results(&index, &aliases, P::Google, q).is_err());
    }
    for q in ["", "   "] {
        assert!(
            composition::results(&index, &aliases, P::Google, q)
                .unwrap()
                .is_empty()
        );
    }
}
#[test]
fn shift_enter_is_one_general_web_dispatch_independent_of_selection() {
    let SubmitPlan::Execute(item) = composition::submit("minecraft", true, P::DuckDuckGo).unwrap()
    else {
        panic!()
    };
    assert!(
        matches!(item.action,Action::WebSearch(url) if url=="https://duckduckgo.com/?q=minecraft")
    );
    assert!(matches!(
        composition::submit("minecraft", false, P::Google).unwrap(),
        SubmitPlan::Selected
    ));
    for q in ["", " ", "?", "? ", "@", "@y", "@yt ", "@yt minecraft"] {
        assert!(
            matches!(
                composition::submit(q, true, P::Google).unwrap(),
                SubmitPlan::None
            ),
            "{q}"
        );
    }
    let SubmitPlan::Execute(item) = composition::submit("?  RTX 4060  ", true, P::Bing).unwrap()
    else {
        panic!()
    };
    assert!(
        matches!(item.action,Action::WebSearch(url) if url=="https://www.bing.com/search?q=RTX+4060")
    );
}
#[test]
fn aliases_keep_completion_and_their_original_website_action() {
    let index = Index::new(validation::fixture());
    let aliases = Aliases::new(&Config::default());
    let rows = composition::results(&index, &aliases, P::Bing, "@y").unwrap();
    assert!(matches!(
        query::intent(rows.first(), Accept::Enter),
        Intent::Complete(_)
    ));
    let rows = composition::results(&index, &aliases, P::Bing, "@yt minecraft shaders").unwrap();
    assert_eq!(rows.len(), 1);
    assert!(
        matches!(&rows[0],Selection::Launch(Item{action:Action::Website(url),..}) if url=="https://www.youtube.com/results?search_query=minecraft+shaders")
    );
    assert_eq!(query::intent(rows.first(), Accept::Enter), Intent::Launch);
}
#[test]
fn bounded_input_never_reaches_the_local_matcher() {
    let index = Index::new(validation::fixture());
    let aliases = Aliases::new(&Config::default());
    for input in ["x".repeat(4097), format!("@yt {}", "a".repeat(5000))] {
        assert!(composition::results(&index, &aliases, P::Google, &input).is_err());
    }
}

#[test]
fn every_original_fixture_query_keeps_relative_local_order_and_a_visible_web_row() {
    let index = Index::new(validation::fixture());
    let aliases = Aliases::new(&Config::default());
    for (q, _, _) in validation::cases() {
        let rows = composition::results(&index, &aliases, P::Google, q).unwrap();
        assert!(rows.len() <= 6 && rows.iter().any(is_web), "{q}");
        let local_ids: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                Selection::Launch(item)
                    if !matches!(item.action, Action::WebSearch(_) | Action::OpenUrl(_)) =>
                {
                    Some(&item.id)
                }
                _ => None,
            })
            .collect();
        let expected: Vec<_> = index
            .search(q, 6)
            .iter()
            .take(local_ids.len())
            .map(|h| &index.items[h.index].id)
            .collect();
        assert_eq!(local_ids, expected, "{q}");
    }
}
