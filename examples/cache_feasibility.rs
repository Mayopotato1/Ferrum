//! Bounded private-snapshot experiment. Never discovers, launches, or touches preferences.
use anyhow::{Context, Result};
use spotlight_prototype::{model::Item, search::Index};
use std::{hint::black_box, path::PathBuf, time::Instant};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let input = PathBuf::from(
        args.first()
            .context("Pass private discovery JSON and aggregate report folder")?,
    );
    let output = PathBuf::from(args.get(1).context("Pass aggregate report folder")?);
    let source: serde_json::Value = serde_json::from_slice(&std::fs::read(input)?)?;
    let items: Vec<Item> = serde_json::from_value(source["items"].clone())?;
    let payload = serde_json::to_vec(&items)?;
    let mut rows = String::from("operation,iteration,items,bytes,elapsed_ms\n");
    for iteration in 0..6 {
        let start = Instant::now();
        let encoded = serde_json::to_vec(black_box(&items))?;
        let serialize = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let decoded: Vec<Item> = serde_json::from_slice(black_box(&payload))?;
        let deserialize = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let index = Index::new(decoded);
        let build = start.elapsed().as_secs_f64() * 1000.;
        if iteration > 0 {
            for (operation, ms) in [
                ("serialize", serialize),
                ("deserialize", deserialize),
                ("index_build", build),
            ] {
                rows += &format!(
                    "{operation},{iteration},{},{},{ms:.4}\n",
                    items.len(),
                    encoded.len()
                );
            }
        }
        black_box(index.search("spotify", 6));
    }
    std::fs::create_dir_all(&output)?;
    std::fs::write(output.join("CACHE_FEASIBILITY.csv"), rows)?;
    Ok(())
}
