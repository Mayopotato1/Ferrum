use crate::model::{Item, normalize};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};

pub struct Index {
    pub items: Vec<Item>,
    search_order: Vec<usize>,
    lengths: Vec<usize>,
}
#[derive(Debug, Clone)]
pub struct Hit {
    pub index: usize,
    pub score: i64,
}
impl Index {
    pub fn new(mut items: Vec<Item>) -> Self {
        for item in &mut items {
            item.aliases = item.aliases.iter().map(|a| normalize(a)).collect();
        }
        items.sort_by(|a, b| a.id.cmp(&b.id).then(a.name.cmp(&b.name)));
        items.dedup_by(|a, b| a.id == b.id);
        let mut search_order: Vec<usize> = (0..items.len()).collect();
        // Reserve bounded typo work for applications/games before filename collisions.
        search_order.sort_by_key(|i| matches!(items[*i].category.as_str(), "File" | "Folder"));
        let lengths = items.iter().map(|i| i.normalized.chars().count()).collect();
        Self {
            items,
            search_order,
            lengths,
        }
    }
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let q = normalize(query);
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }
        let pattern = Pattern::parse(&q, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut buf = Vec::new();
        let mut best: Vec<Hit> = Vec::with_capacity(limit + 1);
        let mut typo_budget = 256;
        let qlen = q.chars().count();
        for &index in &self.search_order {
            let item = &self.items[index];
            let n = &item.normalized;
            let nlen = self.lengths[index];
            let mut score = if n == &q {
                700_000
            } else if item.aliases.iter().any(|a| a == &q) {
                600_000
            } else if n.starts_with(&q) {
                500_000
            } else if n.split_whitespace().any(|w| w.starts_with(&q)) {
                400_000
            } else {
                0
            };
            if score == 0 {
                if qlen >= 3
                    && qlen * 3 >= nlen
                    && let Some(s) = pattern.score(Utf32Str::new(n, &mut buf), &mut matcher)
                {
                    // Reject sparse matches: long filenames shouldn't match tiny arbitrary fragments.
                    score = 300_000 + i64::from(s);
                }
                if score == 0
                    && qlen >= 4
                    && typo_budget > 0
                    && n.chars().next() == q.chars().next()
                {
                    for word in std::iter::once(n.as_str()).chain(n.split_whitespace()) {
                        let len = word.chars().count();
                        let threshold = if qlen >= 8 { 2 } else { 1 };
                        if len.abs_diff(qlen) <= threshold {
                            typo_budget -= 1;
                            let d = strsim::osa_distance(&q, word);
                            if d <= threshold {
                                score = 200_000 - d as i64 * 1000;
                                break;
                            }
                            if typo_budget == 0 {
                                break;
                            }
                        }
                    }
                }
                if score == 0 && qlen >= 3 && n.contains(&q) {
                    score = 100_000;
                }
            }
            if score == 0 {
                continue;
            }
            if !item.available {
                score -= 80_000;
            }
            if item.category == "Windows Application" || item.category == "Packaged Application" {
                score += 500;
            }
            if item.category == "File" {
                // Local discovery exposed transposed app names matching scattered letters
                // in long filenames. Such file fuzzy matches should lose to an app typo.
                if (300_000..400_000).contains(&score) {
                    score -= 120_000;
                }
                score -= 2000;
            }
            score -= nlen.min(500) as i64;
            let compare = |a: &Hit, b: &Hit| {
                b.score
                    .cmp(&a.score)
                    .then(self.items[a.index].name.cmp(&self.items[b.index].name))
                    .then(self.items[a.index].id.cmp(&self.items[b.index].id))
            };
            let candidate = Hit { index, score };
            if best.len() == limit
                && compare(&candidate, best.last().unwrap()) != std::cmp::Ordering::Less
            {
                continue;
            }
            best.push(candidate);
            best.sort_by(compare);
            best.truncate(limit);
        }
        best
    }
}
