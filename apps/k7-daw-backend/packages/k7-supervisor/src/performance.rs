//! performance — measures this machine once, for the rating the heartbeat carries: one core's DSP throughput,
//! the cores, a durable write and an fsync rate on the state dir's disk, the memory. Run before the engine
//! starts, so the CPU loop does not measure a plugin scan. The SCORING is
//! `apps/k7-daw-frontend/packages/web-protocol/performance.ts`'s, held to its numbers by the tests.
//! What a reader gets wrong: compiled code runs the biquad faster than a JIT did, so `singleThread` is the one
//! number a change of supervisor moves.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};

const BLOCK: usize = 4096;
const CPU_BUDGET_MS: f64 = 200.0;
const WRITE_BYTES: usize = 16 * 1024 * 1024;
const WRITE_CHUNK: usize = 1024 * 1024;
const FSYNC_COUNT: u32 = 32;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Metrics {
    pub cores: f64,
    pub single_thread: f64,
    pub disk_write_mbps: Option<f64>,
    pub fsync_per_sec: Option<f64>,
    pub memory_gb: f64,
}

/// 1000 times the weighted geometric mean of each metric over the reference machine's; a metric not measured is left out.
pub fn score(m: &Metrics) -> i64 {
    let weighted = [
        (Some(m.single_thread), 60.0, 0.35),
        (Some(m.cores), 8.0, 0.25),
        (m.disk_write_mbps, 500.0, 0.15),
        (m.fsync_per_sec, 200.0, 0.10),
        (Some(m.memory_gb), 16.0, 0.15),
    ];
    let (mut log_sum, mut weight) = (0.0, 0.0);
    for (value, reference, w) in weighted {
        let Some(v) = value.filter(|v| *v > 0.0) else {
            continue;
        };
        log_sum += w * (v / reference).ln();
        weight += w;
    }
    if weight == 0.0 {
        return 0;
    }
    ((1000.0 * (log_sum / weight).exp()).round() as i64).max(1)
}

fn single_thread() -> f64 {
    let mut buf: Vec<f32> = (0..BLOCK).map(|i| (i as f32 * 0.05).sin()).collect();
    let (b0, b1, b2, a1, a2) = (0.0039f32, 0.0078f32, 0.0039f32, -1.8153f32, 0.8309f32);
    let (mut x1, mut x2, mut y1, mut y2) = (0f32, 0f32, 0f32, 0f32);
    let mut pass = |buf: &mut Vec<f32>| {
        for s in buf.iter_mut() {
            let x = *s;
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, x, y1, y);
            *s = y;
        }
    };
    for _ in 0..200 {
        pass(&mut buf);
    }
    let start = Instant::now();
    let mut samples = 0.0;
    let mut elapsed = 0.0;
    while elapsed < CPU_BUDGET_MS {
        for _ in 0..50 {
            pass(&mut buf);
        }
        samples += 50.0 * BLOCK as f64;
        elapsed = start.elapsed().as_secs_f64() * 1000.0;
    }
    // Keeps the loop observable, so it cannot be optimised away.
    if !std::hint::black_box(y1).is_finite() {
        return 0.0;
    }
    samples / elapsed / 1000.0
}

#[cfg(target_os = "macos")]
fn memory_gb() -> f64 {
    let mut bytes: i64 = 0;
    let mut len = std::mem::size_of::<i64>();
    // SAFETY: the name is NUL-terminated and `bytes` is `len` bytes, as `sysctlbyname` documents.
    let rc = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&raw mut bytes).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc == 0 {
        bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    } else {
        0.0
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn memory_gb() -> f64 {
    // SAFETY: `sysconf` reads a constant of the system.
    let (pages, size) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGE_SIZE),
        )
    };
    pages as f64 * size as f64 / (1024.0 * 1024.0 * 1024.0)
}

#[cfg(windows)]
fn memory_gb() -> f64 {
    crate::win::sys::memory_gb()
}

fn ms_since(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// A durable write and an fsync rate, or nothing where the disk cannot be written (unknown, never "slow").
fn measure_disk(dir: &Path) -> Option<(f64, f64)> {
    let _ = std::fs::create_dir_all(dir);
    let file = dir.join(format!(".perf-probe-{}", std::process::id()));
    let out = disk_probe(&file);
    let _ = std::fs::remove_file(&file);
    out
}

fn disk_probe(file: &Path) -> Option<(f64, f64)> {
    let chunk = vec![0x5au8; WRITE_CHUNK];
    let mut f = std::fs::File::create(file).ok()?;
    let start = Instant::now();
    for _ in 0..WRITE_BYTES / WRITE_CHUNK {
        f.write_all(&chunk).ok()?;
    }
    f.sync_all().ok()?;
    let write_ms = ms_since(start);
    drop(f);
    let mut f = std::fs::File::create(file).ok()?;
    let start = Instant::now();
    for _ in 0..FSYNC_COUNT {
        f.write_all(&chunk[..BLOCK]).ok()?;
        f.sync_all().ok()?;
    }
    let fsync_ms = ms_since(start);
    Some((
        (WRITE_BYTES as f64 / (1024.0 * 1024.0)) / (write_ms / 1000.0),
        f64::from(FSYNC_COUNT) / (fsync_ms / 1000.0),
    ))
}

/// Measure, well under a second; `dir` is where the disk probe writes. Blocking.
pub fn measure(dir: &Path) -> Metrics {
    let disk = measure_disk(dir);
    Metrics {
        cores: std::thread::available_parallelism().map_or(1, |n| n.get()) as f64,
        single_thread: single_thread(),
        disk_write_mbps: disk.map(|d| d.0),
        fsync_per_sec: disk.map(|d| d.1),
        memory_gb: memory_gb(),
    }
}

/// The heartbeat's `performance` object: the metrics and their score.
pub fn to_json(m: &Metrics) -> Value {
    json!({
        "cores": m.cores,
        "singleThread": m.single_thread,
        "diskWriteMBps": m.disk_write_mbps,
        "fsyncPerSec": m.fsync_per_sec,
        "memoryGB": m.memory_gb,
        "score": score(m),
    })
}
