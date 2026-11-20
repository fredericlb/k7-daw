//! waveform.rs — `/waveform`'s answer: a clip located in the project (by a `Locator`), its envelope ensured, the slice and
//! the sliced memo kept per account (2420, 2424, 2425). What a reader gets wrong: the memo's signature carries the
//! sensitivity and the corrections, so a sidecar rebuilt at another threshold never serves the old answer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::routes::files::service::native::callbacks::BoxFuture;
use k7_file_service::media::answer::{Window, answer_sig, build_waveform, waveform_answer, wf_sig};
use k7_file_service::media::onsets::Edits;
use k7_file_service::media::region::{Envelope as Cut, region_waveform};
use k7_mediatek::sidecar::Envelope;
use serde_json::Value;
use song::clips::ClipDoc;
use song::graph::ModelNeed;

use super::envelopes::Envelopes;
use super::{Clock, Counts};

/// What the sliced memo may hold of answers' text, and how long an entry may sit unused (2424).
pub const MEMO_BYTES: usize = 16 * 1024 * 1024;

/// What a `/waveform` asks, as the route clamped it.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub clip: String,
    pub buckets: usize,
    pub whole: bool,
    pub sensitivity: Option<f64>,
}

/// Why a request is not answered here: it is declined, or it is refused with the TypeScript's status and sentence.
#[derive(Debug, PartialEq)]
pub enum Why {
    Decline(&'static str),
    Refuse(u16, String),
}

/// What the project says of a request: the clip drawn, where its file is, and what the draw needs besides.
#[derive(Clone, Debug)]
pub struct Located {
    pub project: String,
    /// The memo's clip part: the clip id, `::whole` after it for the material.
    pub clip_key: String,
    pub clip: ClipDoc,
    pub window: Option<Window>,
    /// The file, confined and resolved; `rack` is an absolute path drawn whole with no corrections.
    pub source: PathBuf,
    pub edits: Edits,
    pub grid: (f64, f64, f64),
    pub rack: bool,
}

/// The audio clips of the active project a prewarm reads, and what confines their sources.
#[derive(Clone, Debug)]
pub struct ProjectView {
    pub project: String,
    pub clips: Vec<ClipDoc>,
    pub roots: Vec<String>,
    pub grid: (f64, f64, f64),
}

impl ProjectView {
    /// A clip's source as the project's files spell it: relative to the project folder.
    pub fn project_path(&self, source: &str) -> String {
        k7_file_service::paths::resolve_all(&[&self.project, source])
    }
}

/// The project side of a request, asked of the account's song session (`locate.rs`) or a test's fake.
pub trait Locator: Send + Sync + 'static {
    fn locate(&self, req: Request) -> BoxFuture<Result<Located, Why>>;
    fn project(&self) -> BoxFuture<Result<ProjectView, Why>>;
    /// The model a node of the active project lacks (2446); a locator with no song session declines.
    fn need(&self, _node: String) -> BoxFuture<Result<ModelNeed, Why>> {
        Box::pin(async { Err(Why::Decline("no-session")) })
    }
}

struct Memo {
    sig: String,
    text: Arc<str>,
    seq: u64,
    used: Instant,
}

#[derive(Default)]
struct State {
    map: HashMap<String, Memo>,
    bytes: usize,
    seq: u64,
}

/// One account's slicing and its memo; the envelopes under it are the account's too.
#[derive(Clone)]
pub struct Waveforms {
    pub envelopes: Envelopes,
    state: Arc<Mutex<State>>,
    clock: Clock,
    counts: Arc<Counts>,
    limit: usize,
}

/// `ENOENT` is a 404 and the rest a 400, as the route's `catch` says.
pub fn status_of(message: &str) -> u16 {
    if message.starts_with("ENOENT") {
        404
    } else {
        400
    }
}

fn cut_of(e: &Envelope) -> Cut<'_> {
    Cut {
        sample_rate: e.sample_rate,
        frames: e.frames,
        seconds: e.seconds,
        peak: &e.peak,
        rms: &e.rms,
        bands: match (&e.low, &e.mid, &e.high) {
            (Some(l), Some(m), Some(h)) => Some([l, m, h]),
            _ => None,
        },
        transients: &e.transients,
    }
}

impl Waveforms {
    pub fn new(envelopes: Envelopes, clock: Clock, counts: Arc<Counts>, limit: usize) -> Waveforms {
        Waveforms {
            envelopes,
            state: Arc::default(),
            clock,
            counts,
            limit,
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The sliced memos let go (`media.shed`).
    pub fn shed(&self) {
        let mut st = self.state();
        st.map.clear();
        st.bytes = 0;
    }

    /// How many answers and bytes the memo holds, for a bound in a test.
    pub fn held(&self) -> (usize, usize) {
        let st = self.state();
        (st.map.len(), st.bytes)
    }

    fn recall(&self, key: &str, sig: &str) -> Option<Arc<str>> {
        let now = (self.clock)();
        let mut st = self.state();
        st.seq += 1;
        let seq = st.seq;
        let m = st.map.get_mut(key).filter(|m| m.sig == sig)?;
        (m.seq, m.used) = (seq, now);
        Some(m.text.clone())
    }

    fn remember(&self, key: String, sig: String, text: Arc<str>) {
        let now = (self.clock)();
        let mut st = self.state();
        st.seq += 1;
        let seq = st.seq;
        st.bytes += text.len();
        if let Some(old) = st.map.insert(
            key,
            Memo {
                sig,
                text,
                seq,
                used: now,
            },
        ) {
            st.bytes -= old.text.len();
        }
        while st.bytes > self.limit && st.map.len() > 1 {
            let Some(oldest) = st
                .map
                .iter()
                .min_by_key(|(_, m)| m.seq)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(m) = st.map.remove(&oldest) {
                st.bytes -= m.text.len();
            }
        }
    }

    /// The answer sliced from a fresh sidecar with no decode and no memo, or none when it is not fresh (2404).
    pub async fn peeked(
        &self,
        loc: Located,
        buckets: usize,
        sensitivity: Option<f64>,
    ) -> Option<Value> {
        let real = real_or(&loc.source).await;
        let env = self.envelopes.peek(&real, sensitivity).await?;
        tokio::task::spawn_blocking(move || draw(&env, &loc, buckets))
            .await
            .ok()?
            .ok()
    }

    /// The answer's text for a located clip: the memo when its signature holds, else the slice of the ensured envelope.
    pub async fn answer(
        &self,
        loc: Located,
        buckets: usize,
        sensitivity: Option<f64>,
    ) -> Result<Arc<str>, (u16, String)> {
        let real = real_or(&loc.source).await;
        let env = self
            .envelopes
            .ensure(&real, sensitivity)
            .await
            .map_err(|m| (status_of(&m), m))?;
        let key = format!("{}::{}::{buckets}", loc.project, loc.clip_key);
        let sig = answer_sig(
            &wf_sig(&env.fingerprint, &loc.clip, loc.grid, buckets, loc.window),
            env.sensitivity,
            &loc.edits,
        );
        if !loc.rack
            && let Some(text) = self.recall(&key, &sig)
        {
            return Ok(text);
        }
        let (name, rack) = (loc.clip.id.clone(), loc.rack);
        let made = tokio::task::spawn_blocking(move || draw(&env, &loc, buckets)).await;
        let text: Arc<str> = match made {
            Ok(Ok(v)) => Arc::from(k7_sync::json::stringify(&v)),
            Ok(Err(())) => return Err((400, format!("'{name}' is too long to draw"))),
            Err(_) => {
                Counts::bump(&self.counts.panics);
                return Err((500, "the waveform panicked".into()));
            }
        };
        self.counts.waveforms.fetch_add(1, Ordering::Relaxed);
        if !rack {
            self.remember(key, sig, text.clone());
        }
        Ok(text)
    }
}

/// The slice itself: a rack's whole file with no corrections, or the clip over the corrected onsets.
fn draw(env: &Envelope, loc: &Located, buckets: usize) -> Result<Value, ()> {
    let (tempo, num, den) = loc.grid;
    if loc.rack {
        let region = region_waveform(&cut_of(env), &loc.clip, tempo, num, den, buckets);
        return build_waveform(&region, None)
            .map(Value::Object)
            .map_err(|_| ());
    }
    waveform_answer(
        &cut_of(env),
        &loc.edits,
        &loc.clip,
        loc.grid,
        buckets,
        loc.window,
    )
    .map_err(|_| ())
}

/// The file's real path, so one file reached by two names is one memo and one decode; as spelled when it cannot be read.
pub fn real_path(p: &Path) -> PathBuf {
    let real = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let s = real.to_string_lossy();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

/// `real_path` on the blocking pool.
pub async fn real_or(p: &Path) -> PathBuf {
    let p = p.to_path_buf();
    tokio::task::spawn_blocking(move || real_path(&p))
        .await
        .unwrap_or_default()
}
