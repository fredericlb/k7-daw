//! rig — a capture over a scratch workspace and a clock the test moves by hand, with the native k7x seam.
//! What a reader gets wrong: the clock never reads behind the real one (a file's mtime is real), it only jumps ahead.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use indexmap::IndexMap;
use k7_capture::capture::{Baseline, Clock, LiveCapture, Options, SystemClock};
use k7_sync::settle::Native;
use serde_json::Value;

use crate::support::Scratch;

/// The native k7x half, one for the whole test binary.
pub fn settler() -> Arc<Native> {
    static POOL: OnceLock<Arc<Native>> = OnceLock::new();
    POOL.get_or_init(|| Arc::new(Native)).clone()
}

pub struct FakeClock(Mutex<f64>);

impl FakeClock {
    pub fn new() -> Arc<FakeClock> {
        Arc::new(FakeClock(Mutex::new(SystemClock.now_ms())))
    }

    /// Jump ahead of the real clock by `ms`.
    pub fn advance(&self, ms: f64) {
        let mut now = self.0.lock().unwrap();
        *now = now.max(SystemClock.now_ms()) + ms;
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> f64 {
        *self.0.lock().unwrap()
    }
}

/// The room's text as a test holds it, shared so a test can change it after the capture took it.
#[derive(Clone, Default)]
pub struct MemBaseline(pub Arc<Mutex<IndexMap<String, String>>>);

impl MemBaseline {
    pub fn of(files: &[(&str, &str)]) -> Self {
        let b = MemBaseline::default();
        for (f, t) in files {
            b.set(f, t);
        }
        b
    }

    pub fn set(&self, file: &str, text: &str) {
        self.0.lock().unwrap().insert(file.into(), text.into());
    }

    pub fn arc(&self) -> Option<Arc<dyn Baseline>> {
        Some(Arc::new(self.clone()))
    }
}

impl Baseline for MemBaseline {
    fn files(&self) -> std::io::Result<Vec<String>> {
        Ok(self.0.lock().unwrap().keys().cloned().collect())
    }

    fn get(&self, file: &str) -> std::io::Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(file).cloned())
    }
}

pub struct Rig {
    #[allow(dead_code)]
    pub dir: Scratch,
    pub root: String,
    pub state: String,
    pub clock: Arc<FakeClock>,
    pub cap: LiveCapture,
    blobs: bool,
    name: String,
}

pub fn put(root: &str, rel: &str, text: impl AsRef<[u8]>) {
    let abs = format!("{root}/{rel}");
    std::fs::create_dir_all(std::path::Path::new(&abs).parent().unwrap()).unwrap();
    std::fs::write(abs, text).unwrap();
}

pub fn get(root: &str, rel: &str) -> String {
    std::fs::read_to_string(format!("{root}/{rel}")).unwrap()
}

/// Gives a file an mtime `secs` seconds before now, as one written long ago has.
pub fn age(root: &str, rel: &str, secs: u64) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("{root}/{rel}"))
        .unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(secs))
        .unwrap();
}

impl Rig {
    /// A workspace of `files` and a capture over it, not started.
    pub fn new(name: &str, files: &[(&str, &str)], blobs: bool) -> Rig {
        let dir = Scratch::new(name);
        let (root, state) = (dir.at("ws"), dir.at("state"));
        std::fs::create_dir_all(&root).unwrap();
        for (rel, text) in files {
            put(&root, rel, text);
        }
        let clock = FakeClock::new();
        let cap = LiveCapture::new(Rig::options(name, &root, &state, &clock, blobs));
        Rig {
            dir,
            root,
            state,
            clock,
            cap,
            blobs,
            name: name.to_string(),
        }
    }

    /// A workspace with its capture started, nothing emitted.
    pub fn started(name: &str, files: &[(&str, &str)], blobs: bool) -> Rig {
        let mut r = Rig::new(name, files, blobs);
        r.cap.start(None).unwrap();
        r
    }

    fn options(
        name: &str,
        root: &str,
        state: &str,
        clock: &Arc<FakeClock>,
        blobs: bool,
    ) -> Options {
        let mut o = Options::new(root, name, settler());
        o.clock = clock.clone();
        o.binary_settle_ms = 50.0;
        if blobs {
            o.blob_dir = Some(format!("{state}/blobs"));
        }
        o
    }

    /// A capture over the same workspace and state, as a restart makes; the old one is stopped.
    pub fn reopen(&mut self) {
        self.cap.stop();
        self.fresh();
    }

    /// A new capture over the same state, the old one dropped without its stop (which would save its ledger).
    pub fn fresh(&mut self) {
        self.cap = LiveCapture::new(Rig::options(
            &self.name,
            &self.root,
            &self.state,
            &self.clock,
            self.blobs,
        ));
    }

    /// The quiet period passes: what was touched is emitted.
    pub fn quiet(&mut self) -> Option<Value> {
        self.clock.advance(100.0);
        self.cap.on_quiet()
    }

    /// The watcher reports `rels`, the quiet period passes.
    pub fn touched(&mut self, rels: &[&str]) -> Option<Value> {
        for r in rels {
            self.cap.touch(r);
        }
        self.quiet()
    }

    /// The watcher reports `rels` and the quiet period passes with no time for a binary to settle.
    pub fn touched_now(&mut self, rels: &[&str]) -> Option<Value> {
        for r in rels {
            self.cap.touch(r);
        }
        self.cap.on_quiet()
    }

    /// A full sweep.
    pub fn sweep(&mut self) -> Option<Value> {
        self.cap.flush_now(true).unwrap()
    }
}

pub fn ops(batch: &Value) -> Vec<Value> {
    batch["ops"].as_array().unwrap().clone()
}

/// The files a batch's ops name, sorted.
pub fn files(batch: &Option<Value>) -> Vec<String> {
    let mut f: Vec<String> = batch
        .iter()
        .flat_map(ops)
        .map(|o| o["file"].as_str().unwrap().to_string())
        .collect();
    f.sort();
    f
}

/// Bytes that are not text and differ per seed.
pub fn bytes(n: usize, seed: u8) -> Vec<u8> {
    (0..n)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed) | 1)
        .chain([0])
        .collect()
}
