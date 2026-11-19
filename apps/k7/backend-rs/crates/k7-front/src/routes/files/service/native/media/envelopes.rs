//! envelopes.rs — `waveformCache.ts` held per account (2410 to 2417): the envelope memo and the freshness memo, one decode
//! per file and threshold through `flights`, the account's two-at-once gate, and decodes on the blocking pool.
//! What a reader gets wrong: the memo is set only after the sidecar's rename, and a sidecar that cannot be read is an error.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use k7_mediatek::analyse::measure_file_with;
use k7_mediatek::decode::Engine;
use k7_mediatek::fingerprint::file_fingerprint;
use k7_mediatek::sidecar::{self, DEFAULT_SENSITIVITY, Envelope, cache_path};
use tokio::sync::Semaphore;

use super::flights::{self, Env, Flight, Outcome};
use super::{Clock, Config, Counts, KEEP_IDLE};

/// The memo's bound in bytes of envelope (2410), and the freshness memo's in entries (2411).
pub const MEMO_BYTES: usize = 16 * 1024 * 1024;
pub const FRESH_ENTRIES: usize = 20_000;
/// How many files a prewarm decodes at once is the gate's width; one under `STUDIO_LOCAL_USER=1` (2413).
const GATE: usize = 2;

pub struct Settings {
    pub engine: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub wasm_cache: Option<PathBuf>,
    pub local_user: bool,
    pub memo_bytes: usize,
    pub idle: Duration,
    pub clock: Clock,
}

impl Settings {
    pub fn of(c: &Config) -> Settings {
        Settings {
            engine: c.engine.clone(),
            env: c.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            wasm_cache: c.wasm_cache.clone(),
            local_user: c.setting("STUDIO_LOCAL_USER") == Some("1"),
            memo_bytes: MEMO_BYTES,
            idle: KEEP_IDLE,
            clock: c.clock.clone(),
        }
    }
}

struct Entry {
    mtime_ms: f64,
    env: Env,
    bytes: usize,
    seq: u64,
    used: Instant,
}

#[derive(Default)]
struct State {
    memo: HashMap<String, Entry>,
    held: usize,
    seq: u64,
    fresh: HashMap<String, String>,
}

/// An envelope's weight in the memo.
fn weight(e: &Envelope) -> usize {
    let n = |v: &Option<Vec<f64>>| v.as_ref().map_or(0, Vec::len);
    (e.peak.len() + e.rms.len() + e.transients.len() + n(&e.low) + n(&e.mid) + n(&e.high)) * 8
        + e.fingerprint.len()
}

impl State {
    fn get(
        &mut self,
        source: &str,
        mtime_ms: f64,
        sens: Option<f64>,
        now: Instant,
        idle: Duration,
    ) -> Option<Env> {
        let e = self.memo.get_mut(source)?;
        if now.saturating_duration_since(e.used) >= idle {
            self.drop_entry(source);
            return None;
        }
        let usable = sens.is_none_or(|s| e.env.sensitivity == s);
        if e.mtime_ms != mtime_ms || !usable {
            return None;
        }
        self.seq += 1;
        e.seq = self.seq;
        e.used = now;
        Some(e.env.clone())
    }

    fn drop_entry(&mut self, source: &str) {
        if let Some(e) = self.memo.remove(source) {
            self.held -= e.bytes;
        }
    }

    fn put(&mut self, source: &str, mtime_ms: f64, env: Env, now: Instant, cap: usize) {
        self.drop_entry(source);
        let bytes = weight(&env);
        self.seq += 1;
        self.held += bytes;
        let seq = self.seq;
        self.memo.insert(
            source.to_string(),
            Entry {
                mtime_ms,
                env,
                bytes,
                seq,
                used: now,
            },
        );
        // Least recently used first, the newest kept even when it alone is past the bound.
        while self.held > cap && self.memo.len() > 1 {
            let Some(oldest) = self
                .memo
                .iter()
                .min_by_key(|(_, e)| e.seq)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.drop_entry(&oldest);
        }
    }
}

/// One account's envelopes.
#[derive(Clone)]
pub struct Envelopes {
    inner: Arc<Inner>,
}

struct Inner {
    id: u64,
    s: Settings,
    state: Mutex<State>,
    gate: Arc<Semaphore>,
    counts: Arc<Counts>,
}

/// What a prewarm's freshness check found: the sources to decode, and the ones whose sidecar could not be read (2415).
#[derive(Debug, Default, PartialEq)]
pub struct Stale {
    pub sources: Vec<PathBuf>,
    pub unreadable: Vec<PathBuf>,
}

fn mtime_ms(m: &std::fs::Metadata) -> f64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// `statSync(source)`'s refusal as node words it.
fn stat(source: &Path) -> Result<std::fs::Metadata, String> {
    std::fs::metadata(source).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "ENOENT: no such file or directory, stat '{}'",
                source.display()
            )
        } else {
            e.to_string()
        }
    })
}

impl Envelopes {
    pub fn new(s: Settings, counts: Arc<Counts>) -> Envelopes {
        static IDS: AtomicU64 = AtomicU64::new(1);
        let gate = Arc::new(Semaphore::new(if s.local_user { 1 } else { GATE }));
        Envelopes {
            inner: Arc::new(Inner {
                id: IDS.fetch_add(1, Ordering::Relaxed),
                s,
                state: Mutex::new(State::default()),
                gate,
                counts,
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `ensureFileEnvelope` for a source already resolved to its real path: the memo, a matching sidecar, else a decode.
    pub async fn ensure(&self, source: &Path, sensitivity: Option<f64>) -> Outcome {
        let m = mtime_ms(&stat(source)?);
        let name = source.display().to_string();
        let now = (self.inner.s.clock)();
        let hit = self
            .state()
            .get(&name, m, sensitivity, now, self.inner.s.idle);
        if let Some(env) = hit {
            return Ok(env);
        }
        self.flight(source, sensitivity, None).await
    }

    /// The sidecar's envelope while it is fresh, with no decode and nothing remembered (2404).
    pub async fn peek(&self, source: &Path, sensitivity: Option<f64>) -> Option<Env> {
        let src = source.to_path_buf();
        let got = tokio::task::spawn_blocking(move || check(&src, sensitivity)).await;
        got.ok()?.ok()?.0.map(Arc::new)
    }

    /// The prewarm's decode of one stale source at the threshold its sidecar carried; the source's freshness is read again after.
    pub async fn warm(&self, source: &Path, ws: &str) -> Result<(), String> {
        let out = self.flight(source, None, Some(ws)).await;
        self.state().fresh.remove(&source.display().to_string());
        out.map(drop)
    }

    async fn flight(&self, source: &Path, sens: Option<f64>, warming: Option<&str>) -> Outcome {
        let name = source.display().to_string();
        let key = flights::key(&name, sens);
        let joined = flights::join(
            &key,
            warming.is_none(),
            self.inner.id,
            warming.unwrap_or(""),
        );
        if let Some(tx) = joined.starts {
            // Its own task (2472): a client that leaves does not stop a decode another waits on.
            let me = self.clone();
            let flight = joined.flight.clone();
            let (source, key) = (source.to_path_buf(), key.clone());
            tokio::spawn(async move {
                let out = me.work(&source, sens, &flight).await;
                let _ = tx.send(Some(out));
                flights::leave(&key, &flight);
            });
        }
        let mut rx = joined.flight.rx.clone();
        let got = rx.wait_for(Option::is_some).await;
        drop(joined.waiter);
        match got {
            Ok(v) => v.clone().unwrap_or_else(|| Err("the decode ended".into())),
            Err(_) => Err("the decode failed".into()),
        }
    }

    /// The sidecar when it is fresh, else a decode; the memo is set once the sidecar is whole on disk.
    async fn work(&self, source: &Path, asked: Option<f64>, flight: &Arc<Flight>) -> Outcome {
        let m = mtime_ms(&stat(source)?);
        let name = source.display().to_string();
        // An ask that missed the memo just before a flight left lands here: the memo answers it.
        let held = self
            .state()
            .get(&name, m, asked, (self.inner.s.clock)(), self.inner.s.idle);
        if let Some(env) = held {
            return Ok(env);
        }
        let src = source.to_path_buf();
        let checked = tokio::task::spawn_blocking(move || check(&src, asked))
            .await
            .map_err(|e| e.to_string())??;
        let now = || (self.inner.s.clock)();
        if let (Some(env), _) = &checked {
            let env: Env = Arc::new(env.clone());
            self.state()
                .put(&name, m, env.clone(), now(), self.inner.s.memo_bytes);
            return Ok(env);
        }
        let wanted = checked.1;
        let _permit = self
            .inner
            .gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        if flight.stop.load(Ordering::Relaxed) {
            return Err(format!("decoding {name} was aborted"));
        }
        Counts::bump(&self.inner.counts.decodes);
        let (me, src, flight) = (self.clone(), source.to_path_buf(), flight.clone());
        let env = match tokio::task::spawn_blocking(move || me.decode(&src, wanted, &flight)).await
        {
            Ok(r) => r?,
            Err(_) => {
                Counts::bump(&self.inner.counts.panics);
                return Err("the decode panicked".into());
            }
        };
        let env: Env = Arc::new(env);
        self.state()
            .put(&name, m, env.clone(), now(), self.inner.s.memo_bytes);
        Ok(env)
    }

    /// The decode and the sidecar's atomic write, on a blocking thread; nothing recursive runs here (2417).
    fn decode(&self, source: &Path, sensitivity: f64, flight: &Flight) -> Result<Envelope, String> {
        let wf = super::direct::module(self.inner.s.wasm_cache.as_deref())?;
        let run = self.inner.s.engine.as_deref().map(|bin| Engine {
            bin,
            env: Some(&self.inner.s.env),
        });
        let stop = || flight.stop.load(Ordering::Relaxed);
        let run = run.as_ref();
        let env = measure_file_with(&wf, source, sensitivity, run, &stop)
            .map_err(|e| e.message().to_string())?;
        sidecar::write_atomic(source, &env).map_err(|e| e.to_string())?;
        Ok(env)
    }

    /// `staleWaveformSources`, on a blocking thread: the distinct sources whose sidecar is missing or stale; a missing file drops.
    pub fn stale(&self, sources: &[PathBuf]) -> Stale {
        let mut out = Stale::default();
        let mut seen = std::collections::HashSet::new();
        for s in sources.iter().filter(|s| seen.insert(s.as_path())) {
            if stat(s).is_err() {
                continue;
            }
            match self.fresh_sidecar(s) {
                Ok(true) => {}
                Ok(false) => out.sources.push(s.clone()),
                Err(_) => out.unreadable.push(s.clone()),
            }
        }
        out
    }

    /// `cacheFresh`: a YES is remembered against both files' size and mtime (2411), a NO never.
    fn fresh_sidecar(&self, source: &Path) -> Result<bool, String> {
        let name = source.display().to_string();
        let stamp = pair_stamp(source);
        if let Some(st) = &stamp
            && self.state().fresh.get(&name) == Some(st)
        {
            return Ok(true);
        }
        let have = sidecar::read_checked(source).map_err(|e| e.to_string())?;
        let ok = match have {
            Some(e) => file_fingerprint(source).is_ok_and(|f| f == e.fingerprint),
            None => false,
        };
        let mut st = self.state();
        match (ok, stamp) {
            (true, Some(stamp)) => {
                if st.fresh.len() >= FRESH_ENTRIES {
                    st.fresh.clear();
                }
                st.fresh.insert(name, stamp);
            }
            _ => {
                st.fresh.remove(&name);
            }
        }
        Ok(ok)
    }

    /// Stop the decodes no request waits on (a prewarm's cancel); how many were stopped.
    pub fn cancel_prewarm(&self, ws: &str) -> usize {
        flights::cancel_unwanted(self.inner.id, ws)
    }

    /// The decodes a cancel stopped that still run, for the cancel's wait.
    pub fn flying(&self) -> bool {
        flights::owns(self.inner.id)
    }

    pub fn stopping(&self) -> usize {
        flights::stopping(self.inner.id)
    }

    /// Every envelope let go, and what a source's change made stale (`invalidateEnvelope`).
    pub fn invalidate(&self, source: Option<&Path>) {
        let mut st = self.state();
        match source {
            Some(s) => {
                let name = s.display().to_string();
                st.drop_entry(&name);
                st.fresh.remove(&name);
            }
            None => {
                st.memo.clear();
                st.held = 0;
                st.fresh.clear();
            }
        }
    }

    /// A host short of memory (`forgetEnvelopes`): the envelopes go, the freshness verdicts too.
    pub fn shed(&self) {
        self.invalidate(None);
    }

    /// How many envelopes and bytes the memo holds, for a bound in a test.
    pub fn held(&self) -> (usize, usize) {
        let st = self.state();
        (st.memo.len(), st.held)
    }

    pub fn fresh_len(&self) -> usize {
        self.state().fresh.len()
    }
}

/// `checkSidecar`: the sidecar's envelope while it still answers, and the threshold to build at.
fn check(source: &Path, asked: Option<f64>) -> Result<(Option<Envelope>, f64), String> {
    let c = sidecar::check_read(source).map_err(|e| e.to_string())?;
    let wanted = asked.or(c.stored).unwrap_or(DEFAULT_SENSITIVITY);
    let Some(env) = c.env.filter(|e| asked.is_none_or(|s| e.sensitivity == s)) else {
        return Ok((None, wanted));
    };
    let now = file_fingerprint(source).map_err(|e| e.to_string())?;
    Ok(((now == env.fingerprint).then_some(env), wanted))
}

/// Size and mtime of a source and of its sidecar: the pair a "still fresh" verdict was made about.
fn pair_stamp(source: &Path) -> Option<String> {
    let a = std::fs::metadata(source).ok()?;
    let b = std::fs::metadata(cache_path(source)).ok()?;
    Some(format!(
        "{}:{}|{}:{}",
        a.len(),
        mtime_ms(&a),
        b.len(),
        mtime_ms(&b)
    ))
}
