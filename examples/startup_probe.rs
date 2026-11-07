//! Opt-in bounded backend-only measurements. Caller supplies a PRIVATE cache path.
//! Real config is read only; discovery uses its existing folder/deadline bounds.
use anyhow::{Context, Result};
use spotlight_prototype::{cache, config, diagnostics::resources, discovery, search::Index};
use std::{hint::black_box, path::PathBuf, sync::atomic::AtomicBool, time::Instant};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let cache_path = std::path::absolute(PathBuf::from(
        args.first()
            .context("Pass an isolated cache file and aggregate JSON output")?,
    ))?;
    let output = PathBuf::from(args.get(1).context("Pass an aggregate JSON output")?);
    let start = Instant::now();
    let c = config::load()?;
    let config_ms = start.elapsed().as_secs_f64() * 1000.;
    let cancel = AtomicBool::new(false);
    let cache_start = Instant::now();
    let cache_result = cache::load(&cache_path, &c, &cancel);
    let cache_rejected = cache_result.is_err();
    let cached = cache_result.ok().flatten();
    let cache_load_ms = cache_start.elapsed().as_secs_f64() * 1000.;
    let index_start = Instant::now();
    let cached_index = cached.map(Index::new);
    let cached_build_ms = index_start.elapsed().as_secs_f64() * 1000.;
    let cached_usable_ms = cached_index
        .as_ref()
        .map(|_| start.elapsed().as_secs_f64() * 1000.);
    let cached_entries = cached_index.as_ref().map(|i| i.items.len());
    let after_cache = resources();
    let discovery_start = Instant::now();
    let found = discovery::discover(&c, &cancel)?;
    let discovery_ms = discovery_start.elapsed().as_secs_f64() * 1000.;
    let providers:Vec<_>=found.providers.iter().map(|p|serde_json::json!({"provider":p.name,"count":p.count,"milliseconds":p.milliseconds,"had_error":p.error.is_some()})).collect();
    let build_start = Instant::now();
    let fresh = Index::new(found.items);
    let index_build_ms = build_start.elapsed().as_secs_f64() * 1000.;
    let fresh_usable_ms = start.elapsed().as_secs_f64() * 1000.;
    let with_both_indexes = resources();
    drop(cached_index);
    let cache_write_start = Instant::now();
    let cache_saved = if found.providers.iter().all(|p| p.error.is_none()) {
        cache::save(&cache_path, &c, &fresh.items, &cancel)?
    } else {
        false
    };
    let cache_write_ms = cache_write_start.elapsed().as_secs_f64() * 1000.;
    let cache_bytes = std::fs::metadata(&cache_path)?.len();
    let mut searches = Vec::new();
    for (label, q) in [
        ("fixed_exact", "spotify"),
        ("fixed_typo", "minecrfat"),
        ("fixed_alias", "cs2"),
        ("fixed_folder", "downloads"),
    ] {
        let mut times = Vec::new();
        for rep in 0..28 {
            let t = Instant::now();
            black_box(fresh.search(q, 6));
            if rep >= 3 {
                times.push(t.elapsed().as_secs_f64() * 1000.);
            }
        }
        times.sort_by(f64::total_cmp);
        searches.push(serde_json::json!({"case":label,"median_ms":times[12],"p95_ms":times[23]}));
    }
    let report = serde_json::json!({"method":"Backend only, observed OS cache state; no GUI/hotkey, no OS cache flushing", "config_ms":config_ms,"cache_rejected":cache_rejected,"cache_load_ms":cache_load_ms,"cached_index_build_ms":cached_build_ms,"cached_usable_ms":cached_usable_ms,"cached_entries":cached_entries,"discovery_ms":discovery_ms,"fresh_index_build_ms":index_build_ms,"fresh_usable_ms":fresh_usable_ms,"fresh_entries":fresh.items.len(),"providers":providers,"after_cache":after_cache,"with_both_indexes":with_both_indexes,"after_refresh":resources(),"cache_write_ms":cache_write_ms,"cache_saved":cache_saved,"cache_bytes":cache_bytes,"searches":searches});
    std::fs::create_dir_all(output.parent().context("Output needs a parent folder")?)?;
    std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
