use crate::{
    model::{Action, Item},
    search::Index,
};
use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

pub fn fixture() -> Vec<Item> {
    let mut items = Vec::new();
    for (category, names) in [
        (
            "Windows Application",
            "Minecraft Launcher|Spotify|Discord|Steam|VALORANT|Visual Studio Code|Visual Studio|Notepad|Notepad++|Google Chrome|Mozilla Firefox|Microsoft Edge|Microsoft Word|Microsoft Excel|Microsoft PowerPoint|Calculator|Windows Terminal|PowerToys|Modrinth App|Xbox|OBS Studio|7-Zip|Blender|VLC media player|Paint",
        ),
        (
            "Steam Game",
            "Counter-Strike 2|Team Fortress 2|Portal 2|Portal|Stardew Valley|Baldur's Gate 3|Half-Life 2|Cyberpunk 2077|Elden Ring|Terraria",
        ),
        (
            "Modrinth Profile",
            "Minecraft 26.3|MC Performance Tester|Minecraft Shaders",
        ),
        (
            "Packaged Application",
            "Forza Horizon 5|Halo Infinite|Gaming Services",
        ),
        (
            "Folder",
            "Downloads|Documents|Desktop|Minecraft Screenshots|Steam Screenshots|Projects",
        ),
        (
            "File",
            "Spotify notes.txt|Discord backup.txt|Minecraft guide.pdf|Budget 2026.xlsx|Project proposal.docx|Portal 2 walkthrough.pdf|Counter strike tactics.pdf|Steam invoice.pdf|Vacation photo.jpg|spotify.exe.txt",
        ),
    ] {
        for name in names.split('|') {
            let mut i = Item::new(
                name,
                category,
                "Synthetic fixture",
                Action::Unavailable("Fixture; never executed".into()),
            );
            i.id = format!("fixture:{category}:{name}");
            i.available = true;
            if name == "Visual Studio Code" {
                i.aliases.extend(["vscode".into(), "code".into()]);
            }
            items.push(i);
        }
    }
    let mut broken = Item::new(
        "Spotify old",
        "Windows Application",
        "Unavailable fixture",
        Action::Unavailable("Uninstalled".into()),
    );
    broken.id = "broken-spotify".into();
    items.push(broken);
    items.push(items[0].clone()); // Stable identity duplicate must collapse.
    items
}
pub fn cases() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("minecraft", "Minecraft Launcher", "prefix"),
        ("minecrfat", "Minecraft Launcher", "typo"),
        ("mc", "Minecraft Launcher", "alias"),
        ("spotify", "Spotify", "exact"),
        ("spotfy", "Spotify", "typo"),
        ("spot", "Spotify", "prefix"),
        ("discord", "Discord", "exact"),
        ("disocrd", "Discord", "typo"),
        ("disc", "Discord", "prefix"),
        ("steam", "Steam", "exact"),
        ("cs2", "Counter-Strike 2", "alias"),
        ("counter strike", "Counter-Strike 2", "prefix"),
        ("tf2", "Team Fortress 2", "alias"),
        ("val", "VALORANT", "alias"),
        ("valorant", "VALORANT", "exact"),
        ("downloads", "Downloads", "exact"),
        ("documents", "Documents", "exact"),
        ("desktop", "Desktop", "exact"),
        ("minecraft screenshots", "Minecraft Screenshots", "exact"),
        ("minecraft guide", "Minecraft guide.pdf", "prefix"),
        ("portal", "Portal", "exact"),
        ("portal 2", "Portal 2", "exact"),
        ("portal 2 walkthrough", "Portal 2 walkthrough.pdf", "prefix"),
        ("vscode", "Visual Studio Code", "alias"),
        ("visual studio", "Visual Studio", "exact"),
        ("vsc", "Visual Studio Code", "alias"),
        ("notepad", "Notepad", "exact"),
        ("notepad++", "Notepad++", "exact"),
        ("google chrome", "Google Chrome", "exact"),
        ("chrome", "Google Chrome", "word"),
        ("firefox", "Mozilla Firefox", "word"),
        ("mozilla", "Mozilla Firefox", "prefix"),
        ("edge", "Microsoft Edge", "word"),
        ("word", "Microsoft Word", "word"),
        ("excel", "Microsoft Excel", "word"),
        ("powerpoint", "Microsoft PowerPoint", "word"),
        ("calculator", "Calculator", "exact"),
        ("terminal", "Windows Terminal", "word"),
        ("powertoys", "PowerToys", "exact"),
        ("modrinth", "Modrinth App", "prefix"),
        ("xbox", "Xbox", "exact"),
        ("obs", "OBS Studio", "prefix"),
        ("blender", "Blender", "exact"),
        ("vlc", "VLC media player", "prefix"),
        ("paint", "Paint", "exact"),
        ("stardew", "Stardew Valley", "prefix"),
        ("bg3", "Baldur's Gate 3", "alias"),
        ("half life", "Half-Life 2", "prefix"),
        ("cyberpunk", "Cyberpunk 2077", "prefix"),
        ("elden", "Elden Ring", "prefix"),
        ("terraria", "Terraria", "exact"),
        ("mc performance", "MC Performance Tester", "prefix"),
        ("forza", "Forza Horizon 5", "prefix"),
        ("halo", "Halo Infinite", "prefix"),
        ("budget", "Budget 2026.xlsx", "prefix"),
        ("project proposal", "Project proposal.docx", "prefix"),
        ("vacation", "Vacation photo.jpg", "prefix"),
        ("projects", "Projects", "exact"),
        ("spotify notes", "Spotify notes.txt", "prefix"),
        ("dscord", "Discord", "fuzzy"),
        ("sptfy", "Spotify", "fuzzy"),
        ("SPOTIFY", "Spotify", "exact"),
        ("counter-strike 2", "Counter-Strike 2", "exact"),
        ("steam screenshots", "Steam Screenshots", "exact"),
    ]
}
pub fn relevance(root: &Path) -> Result<(usize, usize, usize)> {
    let idx = Index::new(fixture());
    let mut text = String::from(
        "# Synthetic search relevance\n\nFixtures are separate from local discovery. Unavailable synthetic actions are never executed.\n\n|Query|Category|Expected|Actual top 3|Top 1|\n|---|---|---|---|---|\n",
    );
    let (mut one, mut three) = (0, 0);
    for (q, expected, category) in cases() {
        let hits = idx.search(q, 3);
        let names: Vec<_> = hits
            .iter()
            .map(|h| idx.items[h.index].name.as_str())
            .collect();
        let ok = names.first() == Some(&expected);
        one += usize::from(ok);
        three += usize::from(names.contains(&expected));
        text.push_str(&format!(
            "|{q}|{category}|{expected}|{}|{}|\n",
            names.join(", "),
            if ok { "PASS" } else { "FAIL" }
        ));
    }
    let n = cases().len();
    text.push_str(&format!(
        "\nTop-1: {one}/{n} ({:.1}%). Top-3: {three}/{n} ({:.1}%).\n",
        one as f64 / n as f64 * 100.,
        three as f64 / n as f64 * 100.
    ));
    for q in [
        "@yt minecraft shaders",
        "@wiki black holes",
        "@reddit gaming keyboards",
        "@steam portal 2",
        "@google café & rust",
    ] {
        let (_, u) = crate::config::web(&crate::config::Config::default(), q)?.unwrap();
        text.push_str(&format!("\n- `{q}` → `{u}`\n"));
    }
    std::fs::write(root.join("SEARCH_RELEVANCE_TESTS.md"), text)?;
    Ok((one, three, n))
}
pub fn benchmark(root: &Path) -> Result<()> {
    let mut csv = String::from(
        "entries,category,query,repetitions,median_ms,p95_ms,queries_per_second,index_build_ms\n",
    );
    for size in [500, 5000, 50_000, 100_000] {
        let mut data = Index::new(fixture()).items;
        for n in data.len()..size {
            let mut i = Item::new(
                &format!("Archive project document {n:06}.pdf"),
                "File",
                "Synthetic scale",
                Action::Document(PathBuf::from(format!("C:/fixture/{n}.pdf"))),
            );
            i.available = true;
            data.push(i);
        }
        let now = Instant::now();
        let idx = Index::new(data);
        let build = now.elapsed().as_secs_f64() * 1000.;
        for (category, q) in [
            ("exact", "Spotify"),
            ("alias", "cs2"),
            ("typo", "minecrfat"),
            ("prefix", "archive project"),
            ("fuzzy", "sptfy"),
            ("miss", "zzzzzzzzzz"),
        ] {
            for _ in 0..3 {
                std::hint::black_box(idx.search(q, 8));
            }
            let mut times = Vec::new();
            for _ in 0..25 {
                let now = Instant::now();
                std::hint::black_box(idx.search(q, 8));
                times.push(now.elapsed().as_secs_f64() * 1000.);
            }
            let throughput = 25_000. / times.iter().sum::<f64>();
            times.sort_by(f64::total_cmp);
            csv.push_str(&format!(
                "{size},{category},{q},25,{:.6},{:.6},{throughput:.2},{build:.3}\n",
                times[12], times[23]
            ));
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    std::fs::write(root.join("SEARCH_BENCHMARKS.csv"), csv)?;
    Ok(())
}
