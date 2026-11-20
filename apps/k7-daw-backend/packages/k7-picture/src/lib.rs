//! k7-picture — `picture render` on the server: headless
//! chromium (driven by `chromiumoxide` over the DevTools protocol) runs the render page, which fetches its job, its
//! mixdown and its media from a one-off loopback server under a random path and posts the finished file back.
//! What a reader gets wrong: the server serves only the job's files and the browser fetches only the server (2506).

mod chromium;
mod serve;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::browser_protocol::fetch::{
    ContinueRequestParams, EnableParams, EventRequestPaused, FailRequestParams, RequestPattern,
};
use chromiumoxide::cdp::browser_protocol::network::ErrorReason;
use chromiumoxide::cdp::js_protocol::runtime::EventExceptionThrown;
use futures_util::StreamExt;
use tokio::sync::oneshot;

use serve::{RESULT_LIMIT, Served, Settle, serve, settle, token};
pub use serve::{part_for, percent_decode};

/// How long a render may run: SwiftShader draws on the CPU, so a long film takes longer than itself.
const RENDER_WAIT: Duration = Duration::from_secs(4 * 3600);

/// At most this many refused URLs are kept for the caller.
const BLOCKED_KEPT: usize = 32;

/// One render: the browser, the page's script, the job it is handed, the sound and the media by their URL.
pub struct Job {
    pub chromium: PathBuf,
    pub script: PathBuf,
    /// `job.json`'s text, as pictureRender.ts writes it.
    pub job: String,
    pub mix: Option<PathBuf>,
    /// Each `media/<encoded path>` the scene names, to the file it is.
    pub media: HashMap<String, PathBuf>,
    /// The film's destination without its extension: the film is written to a part file beside it (2505).
    pub out: PathBuf,
    /// `STUDIO_RENDER_DEBUG`: every request the page makes is logged.
    pub debug: bool,
    /// The account's environment, chromium's whole one: nothing of the host process's own.
    pub env: Vec<(String, String)>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
}

/// What the page posted: a part file beside `out`, its size and its type; dropped unclaimed, the part is removed.
#[derive(Debug)]
pub struct Rendered {
    pub part: PathBuf,
    pub len: u64,
    pub mime: String,
    /// The URLs the browser asked for and was refused (2506), at most `BLOCKED_KEPT`.
    pub blocked: Vec<String>,
    kept: bool,
}

impl Rendered {
    /// A film already written at `part`, which this value now owns.
    pub fn new(part: PathBuf, mime: String) -> Rendered {
        Rendered {
            part,
            len: 0,
            mime,
            blocked: Vec::new(),
            kept: false,
        }
    }

    /// The part file, left in place for the caller to rename.
    pub fn into_part(mut self) -> PathBuf {
        self.kept = true;
        std::mem::take(&mut self.part)
    }
}

impl Drop for Rendered {
    fn drop(&mut self) {
        if !self.kept {
            let _ = std::fs::remove_file(&self.part);
        }
    }
}

/// Whether the page may fetch `url`: its own job's path on the loopback server, an inline `data:` or `blob:`, or the blank page (2506).
pub fn allowed(url: &str, port: u16, token: &str) -> bool {
    url == "about:blank"
        || url.starts_with("data:")
        || url.starts_with("blob:")
        || url.starts_with(&format!("http://127.0.0.1:{port}/{token}/"))
}

/// The render: the server up on the loopback, chromium launched on SwiftShader, the page's film or its failure.
pub async fn render(job: Job) -> Result<Rendered, String> {
    render_within(job, RESULT_LIMIT).await
}

/// `render` with the film's bound named, which a test shrinks.
pub async fn render_within(job: Job, limit: u64) -> Result<Rendered, String> {
    let (tx, rx) = oneshot::channel();
    let settle_with: Settle = Arc::new(Mutex::new(Some(tx)));
    let blocked: Arc<Mutex<Vec<String>>> = Arc::default();
    let served = Arc::new(Served {
        token: token(),
        script: job.script,
        job: job.job,
        mix: job.mix,
        media: job.media,
        settle: settle_with.clone(),
        debug: job.debug,
        log: job.log,
        out: job.out,
        limit,
        blocked: blocked.clone(),
        film: Default::default(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let app = Router::new().fallback(serve).with_state(served.clone());
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let profile = std::env::temp_dir().join(format!("k7-picture-{}", served.token));
    let gate = Gate {
        port,
        token: served.token.clone(),
        blocked,
        log: served.log.clone(),
    };
    let out = drive(&job.chromium, &job.env, &profile, gate, settle_with, rx).await;
    server.abort();
    let _ = std::fs::remove_dir_all(&profile);
    out
}

/// The request gate's state: what the page may reach, and where a refusal is written down.
struct Gate {
    port: u16,
    token: String,
    blocked: Arc<Mutex<Vec<String>>>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Gate {
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/{}/", self.port, self.token)
    }

    fn refuse(&self, url: &str) {
        (self.log)(&format!("render: refused {url}"));
        let mut b = self.blocked.lock().unwrap_or_else(|p| p.into_inner());
        if b.len() < BLOCKED_KEPT {
            b.push(url.to_string());
        }
    }
}

/// chromium headless on the job's url, until the page posts its result, fails, or throws.
async fn drive(
    chromium: &Path,
    env: &[(String, String)],
    profile: &Path,
    gate: Gate,
    settle_with: Settle,
    rx: oneshot::Receiver<Result<Rendered, String>>,
) -> Result<Rendered, String> {
    let (mut child, ws) = chromium::launch(chromium, env, profile).await?;
    let (mut browser, mut handler) = Browser::connect(ws)
        .await
        .map_err(|e| format!("chromium did not start: {e}"))?;
    let pump = tokio::spawn(async move { while handler.next().await.is_some() {} });
    let run = async {
        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| e.to_string())?;
        let mut thrown = page
            .event_listener::<EventExceptionThrown>()
            .await
            .map_err(|e| e.to_string())?;
        let s = settle_with.clone();
        tokio::spawn(async move {
            if let Some(e) = thrown.next().await {
                let d = &e.exception_details;
                let why = d
                    .exception
                    .as_ref()
                    .and_then(|x| x.description.clone())
                    .unwrap_or_else(|| d.text.clone());
                settle(&s, Err(why));
            }
        });
        // 2506: every request is paused and continued only when it is the job's own.
        let mut paused = page
            .event_listener::<EventRequestPaused>()
            .await
            .map_err(|e| e.to_string())?;
        let all = RequestPattern::builder().url_pattern("*").build();
        page.execute(EnableParams::builder().pattern(all).build())
            .await
            .map_err(|e| e.to_string())?;
        let gated = page.clone();
        let url = gate.url();
        tokio::spawn(async move {
            while let Some(ev) = paused.next().await {
                let id = ev.request_id.clone();
                if allowed(&ev.request.url, gate.port, &gate.token) {
                    let _ = gated.execute(ContinueRequestParams::new(id)).await;
                } else {
                    gate.refuse(&ev.request.url);
                    let _ = gated
                        .execute(FailRequestParams::new(id, ErrorReason::BlockedByClient))
                        .await;
                }
            }
        });
        page.goto(url).await.map_err(|e| e.to_string())?;
        match tokio::time::timeout(RENDER_WAIT, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err("the render page went away".into()),
            Err(_) => Err("the render took longer than four hours".into()),
        }
    };
    let out = run.await;
    let _ = browser.close().await;
    pump.abort();
    if tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
    out
}

#[cfg(test)]
mod tests;
