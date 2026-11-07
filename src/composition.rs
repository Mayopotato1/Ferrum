//! Merge independent web actions with existing local hits; never rescore/reorder locals.
use crate::{
    model::{Item, normalize},
    query::{self, Aliases, Mode, Selection},
    search::{Hit, Index},
    web::{self, MAX_QUERY_BYTES, SearchProvider},
};
use anyhow::{Result, ensure};

fn application(item: &Item) -> bool {
    matches!(
        item.category.as_str(),
        "Windows Application"
            | "Packaged Application"
            | "Steam Game"
            | "Steam Game / Tool"
            | "Modrinth Profile"
    )
}
pub fn local_is_confident(index: &Index, hits: &[Hit], text: &str) -> bool {
    let q = normalize(text);
    let length = q.chars().count();
    if q.is_empty() {
        return false;
    }
    hits.iter().any(|hit| {
        let item = &index.items[hit.index];
        if !item.available {
            return false;
        }
        // Exact filenames/folders, multiword app/game names and aliases always win.
        if item.normalized == q || item.aliases.contains(&q) {
            return true;
        }
        let native = application(item);
        if (native || item.category == "Folder") && hit.score >= 399_000 {
            return true;
        }
        // Preserve plausible short application typos, not sparse fuzzy filename collisions.
        native
            && q.split_whitespace().count() == 1
            && length >= 4
            && ((197_000..201_000).contains(&hit.score)
                || ((300_000..400_000).contains(&hit.score)
                    && length * 10 >= item.normalized.chars().count() * 7))
    })
}
pub fn explicit_text(input: &str) -> Option<&str> {
    input.trim_start().strip_prefix('?').map(str::trim)
}
pub fn results(
    index: &Index,
    aliases: &Aliases,
    provider: SearchProvider,
    input: &str,
) -> Result<Vec<Selection>> {
    ensure!(
        input.len() <= MAX_QUERY_BYTES,
        "Query is too long (maximum {MAX_QUERY_BYTES} UTF-8 bytes)"
    );
    match query::mode(input) {
        Mode::AliasDiscovery | Mode::AliasAutocomplete => {
            return Ok(aliases
                .suggestions(input)
                .into_iter()
                .map(Selection::Complete)
                .collect());
        }
        Mode::WebsiteSearch => return Ok(vec![Selection::Launch(aliases.search(input)?)]),
        Mode::Normal => {}
    }
    if let Some(text) = explicit_text(input) {
        return provider
            .search(text)?
            .map(|i| vec![Selection::Launch(i)])
            .ok_or_else(|| anyhow::anyhow!("Type search words after ?"));
    }
    let hits = index.search(input, 6);
    let web = provider.search(input)?;
    let direct = web::detect_url(input).map(web::open_item);
    let confidence = local_is_confident(index, &hits, input);
    let local_limit = 6 - usize::from(web.is_some()) - usize::from(direct.is_some());
    let mut rows: Vec<_> = hits
        .iter()
        .take(local_limit)
        .map(|h| Selection::Launch(index.items[h.index].clone()))
        .collect();
    if let Some(web) = web {
        if confidence || direct.is_some() {
            rows.push(Selection::Launch(web));
        } else {
            rows.insert(0, Selection::Launch(web));
        }
    }
    if let Some(direct) = direct {
        rows.insert(0, Selection::Launch(direct));
    }
    Ok(rows)
}

#[derive(Debug)]
pub enum SubmitPlan {
    Selected,
    Execute(Box<Item>),
    None,
}
/// Exactly one dispatch decision. @ retains its existing Tab/Enter flow; Shift does nothing there.
pub fn submit(input: &str, shift: bool, provider: SearchProvider) -> Result<SubmitPlan> {
    if !shift {
        return Ok(SubmitPlan::Selected);
    }
    if input.starts_with('@') {
        return Ok(SubmitPlan::None);
    }
    let text = explicit_text(input).unwrap_or(input);
    match provider.search(text)? {
        Some(item) => Ok(SubmitPlan::Execute(Box::new(item))),
        None => Ok(SubmitPlan::None),
    }
}
