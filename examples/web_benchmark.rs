//! Bounded headless measurements. Never initializes a GUI, registers hotkeys or executes actions.
use spotlight_prototype::{
    composition,
    config::Config,
    model::{Action, Item},
    query::Aliases,
    search::Index,
    validation,
    web::{self, SearchProvider},
};
use std::{hint::black_box, path::PathBuf, time::Instant};

fn measure(mut run: impl FnMut(), repetitions: usize) -> (f64, f64) {
    for _ in 0..3 {
        run();
    }
    let mut samples = Vec::with_capacity(repetitions);
    for _ in 0..repetitions {
        let now = Instant::now();
        run();
        samples.push(now.elapsed().as_secs_f64() * 1_000_000.);
    }
    samples.sort_by(f64::total_cmp);
    (
        samples[repetitions / 2],
        samples[(repetitions * 95).div_ceil(100) - 1],
    )
}
fn main() -> anyhow::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| "reports/phase1-2-validation".into());
    std::fs::create_dir_all(&output)?;
    let mut csv = String::from("operation,provider,query_label,repetitions,median_us,p95_us\n");
    for provider in SearchProvider::ALL {
        for (label, text) in [
            ("short", "minecraft"),
            ("question", "how to fix discord microphone"),
            ("unicode", "café & 日本語 + #?%!"),
        ] {
            let (median, p95) = measure(
                || {
                    black_box(provider.search(black_box(text)).unwrap());
                },
                500,
            );
            csv.push_str(&format!(
                "url_generation,{},{label},500,{median:.4},{p95:.4}\n",
                provider.name()
            ));
        }
    }
    for (label, text) in [
        ("domain", "github.com/docs?q=rust"),
        ("encoded", "https://example.com/a%20b?q=x%26y"),
        ("unsafe", "javascript:alert(1)"),
        ("filename", "report.pdf"),
    ] {
        let (median, p95) = measure(
            || {
                black_box(web::detect_url(black_box(text)));
            },
            500,
        );
        csv.push_str(&format!(
            "url_classification,none,{label},500,{median:.4},{p95:.4}\n"
        ));
    }
    let mut data = Index::new(validation::fixture()).items;
    for n in data.len()..5000 {
        data.push(Item::new(
            &format!("Archive project document {n:06}.pdf"),
            "File",
            "Synthetic scale",
            Action::Document(format!("C:/fixture/{n}.pdf").into()),
        ));
    }
    let index = Index::new(data);
    let aliases = Aliases::new(&Config::default());
    for (label, text) in [
        ("local", "spotify"),
        ("question", "how to fix discord microphone"),
        ("direct", "github.com"),
        ("explicit", "? minecraft"),
        ("alias", "@yt minecraft"),
        ("alias_discovery", "@"),
    ] {
        if !text.starts_with('@') && composition::explicit_text(text).is_none() {
            let (median, p95) = measure(
                || {
                    black_box(index.search(black_box(text), 6));
                },
                25,
            );
            csv.push_str(&format!(
                "local_only_5000,none,{label},25,{median:.4},{p95:.4}\n"
            ));
        }
        let (median, p95) = measure(
            || {
                black_box(
                    composition::results(&index, &aliases, SearchProvider::Google, black_box(text))
                        .unwrap(),
                );
            },
            25,
        );
        csv.push_str(&format!(
            "composition_5000,Google,{label},25,{median:.4},{p95:.4}\n"
        ));
    }
    std::fs::write(output.join("WEB_BENCHMARKS.csv"), csv)?;
    println!("Headless web measurements saved; no actions executed.");
    Ok(())
}
