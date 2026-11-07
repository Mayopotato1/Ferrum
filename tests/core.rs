use spotlight_prototype::{
    actions,
    config::{self, Config},
    discovery,
    model::{Action, Item, normalize},
    search::Index,
    validation,
};

#[test]
fn relevance_gate() {
    let idx = Index::new(validation::fixture());
    let mut failures = Vec::new();
    for (q, expected, _) in validation::cases() {
        let h = idx.search(q, 3);
        if h.first().map(|h| idx.items[h.index].name.as_str()) != Some(expected) {
            failures.push((
                q,
                expected,
                h.iter()
                    .map(|h| idx.items[h.index].name.clone())
                    .collect::<Vec<_>>(),
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
#[test]
fn deterministic_and_no_weak_short_matches() {
    let data = validation::fixture();
    let a = Index::new(data.clone());
    let b = Index::new(data.into_iter().rev().collect());
    for q in ["mc", "portal", "spotfy", "project"] {
        assert_eq!(
            a.search(q, 8)
                .iter()
                .map(|h| &a.items[h.index].id)
                .collect::<Vec<_>>(),
            b.search(q, 8)
                .iter()
                .map(|h| &b.items[h.index].id)
                .collect::<Vec<_>>()
        );
    }
    assert!(a.search("", 8).is_empty());
    assert!(a.search("zx", 8).is_empty());
    assert_eq!(normalize("Café—ÉDITION"), "cafe edition");
}

#[test]
fn exact_app_beats_identical_file_and_unavailable_app() {
    let mut data = validation::fixture();
    let good = data.iter().find(|i| i.name == "Spotify").unwrap().clone();
    let mut file = good.clone();
    file.id = "collision-file".into();
    file.category = "File".into();
    data.push(file);
    let mut broken = good.clone();
    broken.id = "broken-exact".into();
    broken.available = false;
    data.push(broken);
    let idx = Index::new(data);
    assert_eq!(idx.items[idx.search("spotify", 1)[0].index].id, good.id);
}

#[test]
fn app_typo_beats_sparse_filename_collision_from_local_audit() {
    let mut data = validation::fixture();
    let mut collision = Item::new(
        "Minecraft-Options.javap.txt",
        "File",
        "Regression from local discovery",
        Action::Document("C:/fixture/Minecraft-Options.javap.txt".into()),
    );
    collision.available = true;
    data.push(collision);
    let idx = Index::new(data);
    assert_eq!(
        idx.items[idx.search("minecrfat", 1)[0].index].name,
        "Minecraft Launcher"
    );
}
#[test]
fn website_encoding_and_validation() {
    let c = Config::default();
    let (_, u) = config::web(&c, "@yt café & shaders#1").unwrap().unwrap();
    let url = url::Url::parse(&u).unwrap();
    assert_eq!(url.query_pairs().next().unwrap().1, "café & shaders#1");
    for bad in [
        "http://example.com?q={query}",
        "https://{query}.example.com/",
        "https://user:pass@example.com/?q={query}",
        "cmd.exe {query}",
        "https://example.com/",
    ] {
        assert!(config::validate_template(bad).is_err(), "{bad}");
    }
    assert!(config::web(&c, "@unknown abc").is_err());
    assert!(config::web(&c, "@yt").is_err());
}
#[test]
fn action_safety() {
    for name in [
        "a.ps1", "b.cmd", "c.exe", "d.lnk", "e.url", "f.hta", "g.js", "h.msi", "i.scr",
    ] {
        assert!(!discovery::safe_document(std::path::Path::new(name)));
    }
    let missing = Item::new(
        "Missing",
        "File",
        "fixture",
        Action::Document("C:/does-not-exist.pdf".into()),
    );
    assert!(actions::prepare(&missing).is_err());
    let invalid = Item::new(
        "unsafe",
        "Web",
        "fixture",
        Action::Website("javascript:alert(1)".into()),
    );
    assert!(actions::prepare(&invalid).is_err());
    let web = Item::new(
        "YouTube",
        "Web",
        "fixture",
        Action::Website("https://www.youtube.com/results?search_query=rust".into()),
    );
    assert_eq!(
        actions::prepare(&web).unwrap(),
        actions::Prepared::Shell("https://www.youtube.com/results?search_query=rust".into())
    );
}
#[test]
fn steam_real_format_fixture_and_action_association() {
    let root = std::env::temp_dir().join(format!("spotlight-fixture-{}", std::process::id()));
    let apps = root.join("steamapps");
    std::fs::create_dir_all(apps.join("common/Portal 2")).unwrap();
    let manifest = apps.join("appmanifest_620.acf");
    let text = r#""AppState" { "appid" "620" "name" "Portal 2" "StateFlags" "4" "installdir" "Portal 2" }"#;
    std::fs::write(&manifest, text).unwrap();
    let item = discovery::steam_manifest(text, &manifest, &apps).unwrap();
    assert_eq!(
        actions::prepare(&item).unwrap(),
        actions::Prepared::Shell("steam://rungameid/620".into())
    );
    assert!(discovery::steam_manifest(&text.replace("620", "730"), &manifest, &apps).is_err());
    std::fs::write(&manifest, text.replace("\"4\"", "\"0\"")).unwrap();
    assert!(actions::prepare(&item).is_err());
    // Delete only this exact process-owned fixture tree.
    std::fs::remove_dir_all(&root).unwrap();
    let libraries = discovery::steam_libraries(r#""libraryfolders" { "0" { "path" "D:\\SteamLibrary" "apps" { "620" "1" } } "1" "E:\\Games" }"#).unwrap();
    assert_eq!(libraries.len(), 2);
}

#[test]
fn native_shortcut_retains_arguments_and_rejects_changes() {
    use windows::{
        Win32::{System::Com::*, UI::Shell::*},
        core::{HSTRING, Interface},
    };
    let _com = discovery::Com::new().unwrap();
    let root = std::env::temp_dir().join(format!("spotlight-link-fixture-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Fixture app.lnk");
    let exe = std::env::current_exe().unwrap();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
        link.SetPath(&HSTRING::from(exe.as_os_str())).unwrap();
        link.SetArguments(windows::core::w!("--fixture-only \"two words\""))
            .unwrap();
        link.SetWorkingDirectory(&HSTRING::from(root.as_os_str()))
            .unwrap();
        let file: IPersistFile = link.cast().unwrap();
        file.Save(&HSTRING::from(path.as_os_str()), true).unwrap();
        let item = discovery::shortcut(&path).unwrap();
        if let Action::Shortcut {
            arguments,
            working_dir,
            ..
        } = &item.action
        {
            assert_eq!(arguments, "--fixture-only \"two words\"");
            assert_eq!(working_dir, &root.to_string_lossy());
        } else {
            panic!("wrong action");
        }
        assert!(matches!(
            actions::prepare(&item),
            Ok(actions::Prepared::Shell(_))
        ));
        link.SetArguments(windows::core::w!("--changed")).unwrap();
        file.Save(&HSTRING::from(path.as_os_str()), true).unwrap();
        assert!(actions::prepare(&item).is_err());
    }
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&root).unwrap();
}
