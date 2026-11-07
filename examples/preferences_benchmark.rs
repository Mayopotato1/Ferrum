//! Isolated filesystem/mutex + mocked hotkeys only. Never calls the GUI or startup registry.
use anyhow::Result;
use spotlight_prototype::{
    hotkeys::{Backend, Key, Manager},
    instance::Instance,
    preferences::{Change, Document},
    web::SearchProvider,
};
use std::{hint::black_box, path::PathBuf, time::Instant};
struct Fake;
impl Backend for Fake {
    fn register(&mut self, _: i32, _: Key) -> Result<()> {
        Ok(())
    }
    fn unregister(&mut self, _: i32) -> Result<()> {
        Ok(())
    }
}
fn measure(name: &str, n: usize, mut f: impl FnMut() -> Result<()>) -> Result<String> {
    for _ in 0..3 {
        f()?;
    }
    let mut times = Vec::with_capacity(n);
    for _ in 0..n {
        let start = Instant::now();
        f()?;
        times.push(start.elapsed().as_secs_f64() * 1_000_000.);
    }
    times.sort_by(f64::total_cmp);
    Ok(format!(
        "{name},{n},{:.3},{:.3}\n",
        times[n / 2],
        times[(n * 95 / 100).min(n - 1)]
    ))
}
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("reports/phase1-3-validation"));
    std::fs::create_dir_all(&output)?;
    let fixture = std::env::temp_dir().join(format!(
        "spotlight-preferences-benchmark-{}",
        std::process::id()
    ));
    std::fs::create_dir(&fixture)?;
    let path = fixture.join("config.json");
    let result = (|| -> Result<()> {
        std::fs::write(
            &path,
            serde_json::to_vec(&spotlight_prototype::config::Config::default())?,
        )?;
        let mut csv = String::from("operation,repetitions,median_us,p95_us\n");
        csv += &measure("config_load_fixture", 200, || {
            black_box(Document::open(path.clone())?);
            Ok(())
        })?;
        let mut doc = Document::open(path.clone())?;
        let mut n = 0;
        csv += &measure("config_atomic_save_fixture", 30, || {
            n += 1;
            doc.update(Change::Provider(if n % 2 == 0 {
                SearchProvider::Google
            } else {
                SearchProvider::Brave
            }))
        })?;
        let mut manager = Manager::start(Fake, Key::Alt);
        csv += &measure("hotkey_transition_mock_no_os", 1000, || {
            let key = if manager.active.unwrap().1 == Key::Alt {
                Key::ControlAlt
            } else {
                Key::Alt
            };
            manager.switch(key, |_| Ok(()))
        })?;
        let identity = format!("Local\\Spotlight-Benchmark-Isolated-{}", std::process::id());
        csv += &measure("isolated_session_mutex_acquire_release", 200, || {
            let guard = Instance::named(&identity)?
                .ok_or_else(|| anyhow::anyhow!("Isolated benchmark mutex unexpectedly in use"))?;
            black_box(&guard);
            drop(guard);
            Ok(())
        })?;
        std::fs::write(output.join("PREFERENCES_BENCHMARKS.csv"), csv)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&fixture);
    result
}
