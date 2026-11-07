//! Headless parser allocation and search-contention probe. No HTTP, GUI or profile IO.
use spotlight_prototype::{search::Index, updates::*, validation};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let n = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;
fn search_samples(index: &Index) -> (f64, f64) {
    let mut times = Vec::new();
    for _ in 0..30 {
        for (q, _, _) in validation::cases() {
            let t = Instant::now();
            std::hint::black_box(index.search(q, 8));
            times.push(t.elapsed().as_secs_f64() * 1e6);
        }
    }
    times.sort_by(f64::total_cmp);
    (times[times.len() / 2], times[times.len() * 95 / 100])
}
fn main() {
    let s = Source::new("fixture-owner", "fixture-repo", Channel::Beta).unwrap();
    let v = semver::Version::parse("0.1.0-beta.1").unwrap();
    let body = include_bytes!("../tests/fixtures/update-newer.json");
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let start = Instant::now();
    let r = select(body, &s, &v).unwrap().unwrap();
    let parse_us = start.elapsed().as_secs_f64() * 1e6;
    let peak = PEAK.load(Ordering::Relaxed) - before;
    let retained = LIVE.load(Ordering::Relaxed) - before;
    let state_bytes = std::mem::size_of::<Preferences>();
    println!(
        "fixture_parse_us={parse_us:.3} parser_peak_increment_bytes={peak} retained_release_heap_bytes={retained} preferences_inline_bytes={state_bytes} response_limit_bytes={MAX_BODY}"
    );
    std::hint::black_box(r);
    let mut worst: serde_json::Value = serde_json::from_slice(body).unwrap();
    worst[0]["body"] = serde_json::Value::String("x".repeat(900_000));
    let large = serde_json::to_vec(&worst).unwrap();
    drop(worst);
    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);
    let result = select(&large, &s, &v).unwrap();
    let large_peak = PEAK.load(Ordering::Relaxed) - baseline;
    println!(
        "large_fixture_response_bytes={} parser_peak_increment_bytes={large_peak} (excludes preallocated response)",
        large.len()
    );
    drop(result);
    drop(large);
    let index = Index::new(validation::fixture());
    let quiet = search_samples(&index);
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let worker_stop = stop.clone();
    let worker_ready = ready.clone();
    let worker = std::thread::spawn(move || {
        worker_ready.wait();
        while !worker_stop.load(Ordering::Relaxed) {
            std::hint::black_box(select(body, &s, &v).unwrap());
        }
    });
    ready.wait();
    let active = search_samples(&index);
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    println!(
        "search_quiet_median_us={:.3} search_quiet_p95_us={:.3} search_fixture_worker_median_us={:.3} search_fixture_worker_p95_us={:.3}",
        quiet.0, quiet.1, active.0, active.1
    );
}
