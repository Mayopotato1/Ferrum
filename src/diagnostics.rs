//! Opt-in aggregate lifecycle counters only. No queries, targets, names, wall-clock
//! history, timers or UI interaction. Disabled path is a single OnceLock lookup.
use serde::Serialize;
use std::{
    io::Write,
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Default, Serialize)]
pub struct Resources {
    pub working_set_bytes: usize,
    pub peak_working_set_bytes: usize,
    pub private_bytes: usize,
    pub cpu_ms: f64,
}
pub fn resources() -> Resources {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{ProcessStatus::*, Threading::*},
    };
    unsafe {
        let process = GetCurrentProcess();
        let mut memory = PROCESS_MEMORY_COUNTERS_EX::default();
        memory.cb = std::mem::size_of_val(&memory) as u32;
        let _ = GetProcessMemoryInfo(
            process,
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            memory.cb,
        );
        let (mut creation, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let _ = GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user);
        let ticks = |t: FILETIME| {
            ((u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)) as f64 / 10000.
        };
        Resources {
            working_set_bytes: memory.WorkingSetSize,
            peak_working_set_bytes: memory.PeakWorkingSetSize,
            private_bytes: memory.PrivateUsage,
            cpu_ms: ticks(kernel) + ticks(user),
        }
    }
}
struct Recorder {
    start: Instant,
    file: Mutex<std::fs::File>,
}
static RECORDER: OnceLock<Option<Recorder>> = OnceLock::new();
pub fn begin() {
    RECORDER.get_or_init(|| {
        if std::env::var("FERRUM_DIAGNOSTICS").as_deref() != Ok("1") {
            return None;
        }
        let path = crate::config::path()
            .with_file_name(format!("ferrum-diagnostics-{}.csv", std::process::id()));
        std::fs::create_dir_all(path.parent()?).ok()?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .ok()?;
        writeln!(
            file,
            "event,elapsed_us,generation_or_count,working_set_bytes,private_bytes,cpu_ms"
        )
        .ok()?;
        Some(Recorder {
            start: Instant::now(),
            file: Mutex::new(file),
        })
    });
}
pub fn record(event: &'static str, count: u64) {
    let Some(Some(recorder)) = RECORDER.get() else {
        return;
    };
    let r = resources();
    if let Ok(mut file) = recorder.file.lock() {
        let _ = writeln!(
            file,
            "{event},{},{count},{},{},{:.3}",
            recorder.start.elapsed().as_micros(),
            r.working_set_bytes,
            r.private_bytes,
            r.cpu_ms
        );
    }
}
