//! download.rs — the model downloads' client and registry (2440 to 2445): `hyper` over rustls, redirects followed up to 20
//! at http(s) only, identity encoding, a part file that is renamed only when whole. What a reader gets wrong: the job is the
//! machine's (`k7-file-service::media::models`); this is the I/O it describes, and `done` follows the rename.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime};

use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, LOCATION, USER_AGENT};
use k7_file_service::media::models::{Fetch, FetchEvent, Job, Models, uuid_v4};
use k7_file_service::media::names::model_cache_path;
use tokio::io::AsyncWriteExt;

use super::http::client;
use super::{Clock, Counts};

/// A download that receives no byte for this long has failed (2441).
pub const IDLE: Duration = Duration::from_secs(60);
/// Redirects followed before the download fails.
const HOPS: usize = 20;
/// A `.part-*` this old was left by a front that was killed (2442).
const STALE_PART: Duration = Duration::from_secs(3600);

/// Per destination, so two accounts naming one model fetch it once (2440).
static DESTS: LazyLock<Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Default::default);

pub struct Downloads {
    inner: Arc<Inner>,
    handle: tokio::runtime::Handle,
}

struct Inner {
    models: Mutex<Models>,
    root: PathBuf,
    idle: Duration,
    clock: Clock,
    counts: Arc<Counts>,
    moved: Arc<dyn Fn() + Send + Sync>,
}

impl Downloads {
    /// Built inside the runtime that runs the downloads; `moved` is told after each rename (2445).
    pub fn new(
        root: PathBuf,
        idle: Duration,
        clock: Clock,
        counts: Arc<Counts>,
        moved: Arc<dyn Fn() + Send + Sync>,
    ) -> Downloads {
        let mint: Box<dyn FnMut() -> [u8; 16] + Send> = Box::new(|| {
            let mut b = [0u8; 16];
            let _ = getrandom::fill(&mut b);
            b
        });
        Downloads {
            inner: Arc::new(Inner {
                models: Mutex::new(Models::new(mint)),
                root,
                idle,
                clock,
                counts,
                moved,
            }),
            handle: tokio::runtime::Handle::current(),
        }
    }

    /// `startModelDownload`: the job at once; one `symlink_metadata` of the destination is its only I/O here.
    pub fn start(&self, file: &str, url: &str) -> Job {
        let dest = model_cache_path(&self.inner.root.display().to_string(), file);
        let exists = dest
            .as_deref()
            .is_some_and(|d| std::fs::symlink_metadata(d).is_ok());
        let now = (self.inner.clock)();
        let (job, fetch) = self.models().start(file, url, dest.as_deref(), exists, now);
        if let Some(f) = fetch {
            Counts::bump(&self.inner.counts.downloads);
            let inner = self.inner.clone();
            self.handle.spawn(async move { drive(inner, f).await });
        }
        job
    }

    pub fn running(&self) -> usize {
        self.models().running()
    }

    pub fn job(&self, id: &str) -> Option<Job> {
        let now = (self.inner.clock)();
        self.models().job(id, now)
    }

    fn models(&self) -> std::sync::MutexGuard<'_, Models> {
        self.inner.models.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Inner {
    fn on(&self, id: &str, e: FetchEvent) {
        let now = (self.clock)();
        self.models
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .on(id, e, now);
    }

    fn complete(&self, id: &str) -> Result<(), String> {
        self.models
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .complete(id)
    }
}

/// One fetch to its end: the destination's own lock, then the download, then the module watch and the job's last word.
async fn drive(inner: Arc<Inner>, f: Fetch) {
    let lock = DESTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .entry(f.dest.clone())
        .or_default()
        .clone();
    let held = lock.lock().await;
    let outcome = if std::fs::symlink_metadata(&f.dest).is_ok() {
        // Another account's download of the same model finished while this one waited.
        Ok(())
    } else {
        fetch(&inner, &f).await
    };
    drop(held);
    {
        let mut dests = DESTS.lock().unwrap_or_else(|p| p.into_inner());
        if Arc::strong_count(&lock) <= 2 {
            dests.remove(&f.dest);
        }
    }
    match outcome {
        Ok(()) => {
            (inner.moved)();
            inner.on(&f.id, FetchEvent::Renamed);
        }
        Err(why) => inner.on(&f.id, FetchEvent::Failed(why)),
    }
}

/// The folder a model lands in, made inside the real root: no link on the way leads out of it (2444).
fn confined_parent(root: &Path, dest: &Path) -> Result<PathBuf, String> {
    let outside = || "the models folder leads outside the models root".to_string();
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let real_root = root.canonicalize().map_err(|e| e.to_string())?;
    let parent = dest.parent().ok_or_else(outside)?;
    let mut deepest = parent;
    while !deepest.exists() {
        deepest = deepest.parent().ok_or_else(outside)?;
    }
    if !deepest
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&real_root)
    {
        return Err(outside());
    }
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let real = parent.canonicalize().map_err(|e| e.to_string())?;
    real.starts_with(&real_root)
        .then_some(real)
        .ok_or_else(outside)
}

/// A `.part-<uuid>` older than an hour, left by a front that died, removed from the folder a download starts in.
fn sweep_parts(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let part = name
            .rsplit_once(".part-")
            .is_some_and(|(_, id)| id.len() == 36);
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok());
        if part && old.is_some_and(|d| d >= STALE_PART) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The GET, redirects followed by hand so each hop is http(s) and no body is ever decoded.
async fn get(url: &str, idle: Duration) -> Result<hyper::Response<hyper::body::Incoming>, String> {
    let mut at = url::Url::parse(url).map_err(|e| format!("{url}: {e}"))?;
    for _ in 0..=HOPS {
        if !matches!(at.scheme(), "http" | "https") {
            return Err(format!("'{at}' is not an http(s) URL"));
        }
        let req = hyper::Request::builder()
            .method("GET")
            .uri(at.as_str())
            .header(ACCEPT_ENCODING, "identity")
            .header(USER_AGENT, "k7")
            .body(Empty::<Bytes>::new().map_err(|e| match e {}).boxed())
            .map_err(|e| e.to_string())?;
        let res = tokio::time::timeout(idle, client().request(req))
            .await
            .map_err(|_| format!("{at} sent nothing for {} s", idle.as_secs_f64()))?
            .map_err(|e| format!("{at}: {e}"))?;
        if !res.status().is_redirection() {
            return Ok(res);
        }
        let Some(next) = res.headers().get(LOCATION).and_then(|l| l.to_str().ok()) else {
            return Ok(res);
        };
        at = at
            .join(next)
            .map_err(|e| format!("{at} redirected to '{next}': {e}"))?;
    }
    Err(format!("{url} redirected more than {HOPS} times"))
}

/// The fetch itself: the part written, counted, checked against the announced size, synced, then renamed over the destination.
async fn fetch(inner: &Inner, f: &Fetch) -> Result<(), String> {
    let dest = PathBuf::from(&f.dest);
    let parent = confined_parent(&inner.root, &dest)?;
    sweep_parts(&parent);
    let res = get(&f.url, inner.idle).await?;
    if !res.status().is_success() {
        return Err(format!("{} answered {}", f.url, res.status().as_u16()));
    }
    if let Some(enc) = res
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|e| e.to_str().ok())
        .filter(|e| !e.eq_ignore_ascii_case("identity"))
    {
        return Err(format!("the server sent it compressed ({enc})"));
    }
    let total = res
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|l| l.to_str().ok())
        .and_then(|l| l.parse::<u64>().ok())
        .filter(|n| *n > 0);
    inner.on(&f.id, FetchEvent::Headers { total });
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).map_err(|e| e.to_string())?;
    let mut part = dest.as_os_str().to_owned();
    part.push(format!(".part-{}", uuid_v4(raw)));
    let part = PathBuf::from(part);
    let written = write_body(inner, f, res, &part).await;
    if let Err(e) = written {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }
    // The size is checked before the rename: a short body is a failed download, not a small model (2442).
    let renamed = inner
        .complete(&f.id)
        .and_then(|()| std::fs::rename(&part, &dest).map_err(|e| e.to_string()));
    if renamed.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    renamed
}

async fn write_body(
    inner: &Inner,
    f: &Fetch,
    res: hyper::Response<hyper::body::Incoming>,
    part: &Path,
) -> Result<(), String> {
    let mut out = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part)
        .await
        .map_err(|e| e.to_string())?;
    let mut body = res.into_body();
    loop {
        let frame = tokio::time::timeout(inner.idle, body.frame())
            .await
            .map_err(|_| format!("no byte for {} s", inner.idle.as_secs_f64()))?;
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|e| e.to_string())?;
        if let Some(data) = frame.data_ref() {
            out.write_all(data).await.map_err(|e| e.to_string())?;
            inner.on(&f.id, FetchEvent::Bytes(data.len() as u64));
        }
    }
    out.flush().await.map_err(|e| e.to_string())?;
    out.sync_all().await.map_err(|e| e.to_string())
}
