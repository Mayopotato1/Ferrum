#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use anyhow::Result;
use spotlight_prototype::{actions, config, discovery, search::Index, validation};
use std::{path::Path, sync::atomic::AtomicBool};
mod app;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        // No modal error dialog or focus-changing APIs, including in headless mode.
        let p = config::path().with_file_name("last-error.txt");
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, format!("{e:#}"));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--settings") => {
            let Some(_instance) = spotlight_prototype::instance::Instance::acquire()? else {
                return Ok(());
            };
            spotlight_prototype::diagnostics::begin();
            spotlight_prototype::diagnostics::record("instance_ready", 0);
            app::run(!args.is_empty())
        }
        Some("--version") => {
            println!("Ferrum {} (headless)", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--update-source") => {
            let source = spotlight_prototype::updates::Source::configured();
            println!(
                "{}",
                serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "configured": source.is_some(),
                    "api_url": source.map(|s| s.api_url()),
                    "channel": env!("FERRUM_UPDATE_CHANNEL"),
                    "local_fixture": cfg!(feature = "update-fixture")
                })
            );
            Ok(())
        }
        Some("--relevance") => {
            let (one, three, n) = validation::relevance(Path::new("."))?;
            println!("Top-1 {one}/{n}; Top-3 {three}/{n}");
            Ok(())
        }
        Some("--bench") => validation::benchmark(Path::new(".")),
        Some("--idle-probe") => idle_probe(),
        Some("--audit") => audit(),
        Some("--write-config") => {
            use std::io::Write;
            let p = config::path();
            std::fs::create_dir_all(p.parent().unwrap())?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)?;
            file.write_all(serde_json::to_string_pretty(&config::Config::default())?.as_bytes())?;
            println!("{}", p.display());
            Ok(())
        }
        Some(mode @ ("--discover" | "--dry-run")) => {
            let c = config::load()?;
            let start = std::time::Instant::now();
            let d = discovery::discover(&c, &AtomicBool::new(false))?;
            for p in &d.providers {
                println!(
                    "{}: {} entries, {:.2} ms, error={:?}",
                    p.name, p.count, p.milliseconds, p.error
                );
            }
            println!(
                "Discovery total {:.2} ms",
                start.elapsed().as_secs_f64() * 1000.
            );
            if mode == "--discover" {
                std::fs::create_dir_all("reports")?;
                std::fs::write(
                    "reports/local-discovery.json",
                    serde_json::to_string_pretty(&d)?,
                )?;
            } else {
                let q = args
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("--dry-run requires a quoted query"))?;
                let selected = if let Some((name, url)) = config::web(&c, q)? {
                    spotlight_prototype::model::Item::new(
                        &name,
                        "Website search",
                        "Configured HTTPS template",
                        spotlight_prototype::model::Action::Website(url),
                    )
                } else {
                    let index = Index::new(d.items);
                    let hits = index.search(q, 1);
                    index.items[hits
                        .first()
                        .ok_or_else(|| anyhow::anyhow!("No matching result"))?
                        .index]
                        .clone()
                };
                println!(
                    "DRY RUN ONLY: {} [{}] {:?}",
                    selected.name,
                    selected.source,
                    actions::prepare(&selected)?
                );
            }
            Ok(())
        }
        Some(other) => anyhow::bail!(
            "Unknown option {other}; use --settings, --version, --relevance, --bench, --discover, --dry-run QUERY, --idle-probe or --write-config"
        ),
    }
}
fn audit() -> Result<()> {
    use spotlight_prototype::model::{Action, Item};
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("reports/local-discovery.json")?)?;
    let items: Vec<Item> = serde_json::from_value(v["items"].clone())?;
    let start = std::time::Instant::now();
    let index = Index::new(items);
    let build_ms = start.elapsed().as_secs_f64() * 1000.;
    let _com = discovery::Com::new()?;
    let mut audit = Vec::new();
    for item in index
        .items
        .iter()
        .filter(|i| !matches!(i.action, Action::Folder(_) | Action::Document(_)))
        .take(1000)
    {
        let result = actions::prepare(item);
        audit.push(serde_json::json!({"name":item.name,"source":item.source,"category":item.category,"id":item.id,"prepared":result.as_ref().ok().map(|a|format!("{a:?}")),"error":result.err().map(|e|format!("{e:#}"))}));
    }
    let mut queries = Vec::new();
    for q in [
        "spotify",
        "spotfy",
        "minecraft",
        "minecrfat",
        "mc",
        "steam",
        "geometry dash",
        "modrinth",
        "downloads",
        "xbox",
        "main",
    ] {
        let mut times = Vec::new();
        let hits = index.search(q, 3);
        for _ in 0..25 {
            let now = std::time::Instant::now();
            std::hint::black_box(index.search(q, 8));
            times.push(now.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        queries.push(serde_json::json!({"query":q,"median_ms":times[12],"p95_ms":times[23],"top3":hits.iter().map(|h|serde_json::json!({"name":index.items[h.index].name,"source":index.items[h.index].source})).collect::<Vec<_>>()}));
    }
    let report = serde_json::json!({"index_items":index.items.len(),"index_build_ms":build_ms,"actions":audit,"local_queries":queries});
    std::fs::write(
        "reports/action-audit.json",
        serde_json::to_string_pretty(&report)?,
    )?;
    println!(
        "Audited {} typed provider actions without executing; {} indexed items",
        audit.len(),
        index.items.len()
    );
    Ok(())
}
fn idle_probe() -> Result<()> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{ProcessStatus::*, Threading::*},
    };
    unsafe {
        let process = GetCurrentProcess();
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let ticks = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
        GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user)?;
        let before = ticks(kernel) + ticks(user);
        let now = std::time::Instant::now();
        std::thread::sleep(std::time::Duration::from_secs(2));
        GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user)?;
        let mut mem = PROCESS_MEMORY_COUNTERS::default();
        mem.cb = std::mem::size_of_val(&mem) as u32;
        GetProcessMemoryInfo(
            process,
            &mut mem,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )?;
        println!(
            "elapsed_ms={:.2} process_cpu_ms={:.3} working_set_bytes={} peak_working_set_bytes={}",
            now.elapsed().as_secs_f64() * 1000.,
            (ticks(kernel) + ticks(user) - before) as f64 / 10000.,
            mem.WorkingSetSize,
            mem.PeakWorkingSetSize
        );
    }
    Ok(())
}
