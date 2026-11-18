//! What the link's tests are built from beside the fake engine: a host that records what it is asked, a rig of one
//! account's link over a fake engine, a document the real engine takes, and a wait for a condition.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::fake::FakeEngine;
use super::held;

/// A host that records what the link asks of it and answers from a script.
#[derive(Default)]
pub struct TestHost {
    pub doc: Mutex<Value>,
    pub compiled: Mutex<Vec<String>>,
    pub invalidated: AtomicU64,
    pub external: AtomicBool,
    pub catalogues: Mutex<Vec<Value>>,
    pub takes: Mutex<Vec<(String, k7_file_service::engine::wire::Take)>>,
    pub forwarded: AtomicU64,
    pub lines: Mutex<Vec<String>>,
    pub named: Mutex<Vec<String>>,
    pub patterns: Mutex<Vec<String>>,
    pub prefs: Mutex<super::Prefs>,
    pub panic_compile: AtomicBool,
    pub fail_compile: AtomicBool,
    /// Milliseconds a compile takes, set by a test that needs a slow one.
    pub slow_compile: AtomicU64,
    /// Every frame the link forwarded, as `(session, type)`, in the order told.
    pub forwards: Mutex<Vec<(String, String)>>,
    /// The files of the takes filed, in the order their filings ended; a file named `slow` takes 300 ms.
    pub filed: Mutex<Vec<String>>,
}

impl TestHost {
    pub fn with_doc(doc: Value) -> Arc<TestHost> {
        let h = TestHost::default();
        *held(&h.doc) = doc;
        Arc::new(h)
    }

    pub fn said(&self) -> Vec<String> {
        held(&self.lines).clone()
    }
}

impl super::Host for TestHost {
    fn compile(&self, _root: &str, project: &str) -> Result<Value, String> {
        held(&self.compiled).push(project.to_string());
        std::thread::sleep(Duration::from_millis(
            self.slow_compile.load(Ordering::Relaxed),
        ));
        if self.panic_compile.load(Ordering::Relaxed) {
            panic!("a compile that panics");
        }
        if self.fail_compile.load(Ordering::Relaxed) {
            return Err("compile: a broken project".into());
        }
        Ok(held(&self.doc).clone())
    }

    fn invalidate(&self, _root: &str) {
        self.invalidated.fetch_add(1, Ordering::Relaxed);
    }

    fn revision(&self, _root: &str) -> String {
        "rev".into()
    }

    fn names_external(&self, _root: &str, _project: &str) -> bool {
        self.external.load(Ordering::Relaxed)
    }

    fn named_files(&self, _root: &str, _project: &str) -> Vec<String> {
        held(&self.named).clone()
    }

    fn pattern_files(&self, _project: &str) -> Vec<String> {
        held(&self.patterns).clone()
    }

    fn preferences(&self) -> super::Prefs {
        held(&self.prefs).clone()
    }

    fn write_catalogue(&self, plugins: &Value) {
        held(&self.catalogues).push(plugins.clone());
    }

    fn file_take(
        &self,
        _root: &str,
        _project: &str,
        take: &k7_file_service::engine::wire::Take,
        _arm: Option<&k7_file_service::engine::docs::TakeArm>,
    ) -> Result<String, String> {
        if take.file.contains("slow") {
            std::thread::sleep(Duration::from_millis(300));
        }
        held(&self.filed).push(take.file.clone());
        Ok(format!("{}-clip", take.track))
    }
}

impl super::Tells for TestHost {
    fn recorded(&self, session: &str, take: k7_file_service::engine::wire::Take) {
        held(&self.takes).push((session.to_string(), take));
    }

    fn forward(&self, session: &str, kind: &str, _frame: &Value) {
        self.forwarded.fetch_add(1, Ordering::Relaxed);
        held(&self.forwards).push((session.to_string(), kind.to_string()));
    }

    fn say(&self, line: &str) {
        held(&self.lines).push(line.to_string());
    }
}

/// Wait for `f` to hold, polling every 20 ms; panics after `secs` with `what`.
pub async fn until(secs: u64, what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// A document the typed reader and the real engine both accept.
pub fn minimal_doc(master: f64) -> Value {
    json!({
        "name": "t", "tempo": 120, "sampleRate": 48000, "timeSigNumerator": 4, "timeSigDenominator": 4,
        "masterVolume": master, "masterPlugins": [], "recordDir": "", "tracks": [], "groups": [], "markers": [],
        "settings": { "metronome": false, "metronomeLevel": 0.5, "countIn": 0 }
    })
}

/// One account's link over a fake engine, with a workspace folder and a project folder on disk.
pub struct Rig {
    pub link: super::EngineLink,
    pub host: Arc<TestHost>,
    pub engine: FakeEngine,
    pub dir: std::path::PathBuf,
    pub ws: String,
    pub project: String,
}

impl Rig {
    /// The session name of the workspace: `k7:` and the owner, slugged.
    pub const SESSION: &'static str = "k7:a_b_c";

    pub async fn new(tag: &str) -> Rig {
        Rig::make(tag, |h| h).await
    }

    async fn make<T: super::Tells>(tag: &str, tells: impl FnOnce(Arc<TestHost>) -> Arc<T>) -> Rig {
        let dir = crate::routes::files::service::native::tests::tmp_root()
            .join(format!("k7-elink-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ws = dir.join("ws/songs");
        let project = ws.join("p.k7");
        std::fs::create_dir_all(&project).unwrap();
        let files_root = super::super::tests::slashed(&dir.join("ws"));
        let host = TestHost::with_doc(minimal_doc(-3.0));
        let link = super::EngineLink::new(
            super::Account {
                files_root: files_root.clone(),
                email: "a@b.c".into(),
                roots: vec![files_root],
            },
            host.clone(),
            tells(host.clone()),
        );
        let engine = FakeEngine::start().await;
        link.bind(&json!({ "url": engine.url, "engineId": "e1" }));
        Rig {
            link,
            host,
            engine,
            ws: super::super::tests::slashed(&ws),
            project: super::super::tests::slashed(&project),
            dir,
        }
    }

    pub fn session(&self) -> Arc<super::Session> {
        self.link.session(Some(&self.ws))
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.link.close();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
