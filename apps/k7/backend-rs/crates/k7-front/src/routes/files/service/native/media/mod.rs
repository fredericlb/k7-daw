//! media — an account's waveforms, downloads and takes held natively (1l, `docs/file-media.md`): the envelopes' memo and
//! the decodes in flight, the model downloads' registry and client, and a take filed into the media.
//! What a reader gets wrong: no lock here is held across an await, a decode, a request or a rename (2470).

pub mod direct;
pub mod download;
pub mod envelopes;
pub mod flights;
pub mod host;
pub mod http;
pub mod locate;
pub mod prefetch;
pub mod prewarm;
pub mod routes;
pub mod take;
pub mod waveform;

#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests_download;
#[cfg(test)]
mod tests_envelopes;
#[cfg(test)]
mod tests_flights;
#[cfg(test)]
mod tests_prefetch;
#[cfg(test)]
pub(in crate::routes::files::service::native) mod tests_retired;
#[cfg(test)]
mod tests_routes;
#[cfg(test)]
mod tests_take;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A clock a test turns; the product's is the process's.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// What `[files] native media:` counts (2482).
#[derive(Default)]
pub struct Counts {
    pub waveforms: AtomicU64,
    pub decodes: AtomicU64,
    pub prewarms: AtomicU64,
    pub downloads: AtomicU64,
    pub takes: AtomicU64,
    pub prefetches: AtomicU64,
    pub declines: AtomicU64,
    pub panics: AtomicU64,
}

impl Counts {
    pub fn bump(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }

    /// The log line's body, in the order 2482 names.
    pub fn line(&self) -> String {
        let n = |c: &AtomicU64| c.load(Ordering::Relaxed);
        format!(
            "waveforms={} decodes={} prewarms={} downloads={} takes={} prefetches={} declines={} panics={}",
            n(&self.waveforms),
            n(&self.decodes),
            n(&self.prewarms),
            n(&self.downloads),
            n(&self.takes),
            n(&self.prefetches),
            n(&self.declines),
            n(&self.panics)
        )
    }
}

/// How often the counts are said while they move.
pub const REPORT_EVERY: Duration = Duration::from_secs(30);

/// How long a decode's envelope stays in the memo unused, and a finished download's job in the registry (2410, 2440).
pub const KEEP_IDLE: Duration = Duration::from_secs(10 * 60);

/// What an account's media is built from: its environment, as the hub gives it to its file service, and the engine found for it.
#[derive(Clone)]
pub struct Config {
    pub env: BTreeMap<String, String>,
    pub studio_home: String,
    pub engine: Option<PathBuf>,
    pub wasm_cache: Option<PathBuf>,
    pub clock: Clock,
    /// How long a changed count waits for its line, and the trailing flush after the last move.
    pub report_every: Duration,
}

impl Config {
    /// The account's media: its environment as the hub gives it, its studio home, the engine found for it, the process's clock.
    pub fn of(env: &BTreeMap<String, String>, code_root: &std::path::Path) -> Config {
        Config {
            env: env.clone(),
            studio_home: k7_file_service::libraries::Home::of(env).studio,
            engine: crate::routes::files::service::machine::engine_binary(
                env.get("STUDIO_ENGINE").map(String::as_str),
                code_root,
            ),
            wasm_cache: None,
            clock: Arc::new(Instant::now),
            report_every: REPORT_EVERY,
        }
    }

    pub fn setting(&self, key: &str) -> Option<&str> {
        self.env
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }
}

/// One account's media: no state but what is bounded and let go at the reap.
pub struct Media {
    pub envelopes: envelopes::Envelopes,
    pub waves: waveform::Waveforms,
    pub prewarms: prewarm::Prewarms,
    pub lites: Arc<locate::Memo>,
    pub downloads: download::Downloads,
    pub prefetch: prefetch::Prefetch,
    pub counts: Arc<Counts>,
    pub config: Config,
    said: Arc<std::sync::Mutex<(Option<Instant>, String)>>,
    trailing: Arc<std::sync::atomic::AtomicBool>,
}

impl Media {
    pub fn new(config: Config, moved: Arc<dyn Fn() + Send + Sync>) -> Media {
        let counts = Arc::new(Counts::default());
        let models = k7_file_service::media::names::models_root(
            config.setting("STUDIO_MODELS"),
            &config.studio_home,
        );
        let envelopes = envelopes::Envelopes::new(envelopes::Settings::of(&config), counts.clone());
        Media {
            waves: waveform::Waveforms::new(
                envelopes.clone(),
                config.clock.clone(),
                counts.clone(),
                waveform::MEMO_BYTES,
                KEEP_IDLE,
            ),
            prewarms: prewarm::Prewarms::default(),
            lites: Arc::default(),
            envelopes,
            downloads: download::Downloads::new(
                PathBuf::from(models),
                download::IDLE,
                config.clock.clone(),
                counts.clone(),
                moved,
            ),
            prefetch: prefetch::Prefetch::default(),
            counts,
            config,
            said: Arc::default(),
            trailing: Arc::default(),
        }
    }

    /// `[files] native media: …` once in `report_every` while the counts move; the last move is flushed after the wait.
    pub fn say_if_due(&self, log: &k7_hub::Log) {
        let (now, line) = ((self.config.clock)(), self.counts.line());
        let mut said = self.said.lock().unwrap_or_else(|p| p.into_inner());
        let every = self.config.report_every;
        if said.1 == line {
            return;
        }
        if said
            .0
            .is_none_or(|t| now.saturating_duration_since(t) >= every)
        {
            *said = (Some(now), line.clone());
            drop(said);
            log(&format!("[files] native media: {line}"));
            return;
        }
        drop(said);
        self.flush_later(log);
    }

    /// One task at a time says the counts once the wait is over, so the last move is never left unsaid.
    fn flush_later(&self, log: &k7_hub::Log) {
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            return;
        };
        if self.trailing.swap(true, Ordering::AcqRel) {
            return;
        }
        let (said, counts, flag) = (
            self.said.clone(),
            self.counts.clone(),
            self.trailing.clone(),
        );
        let (log, every) = (log.clone(), self.config.report_every);
        rt.spawn(async move {
            tokio::time::sleep(every).await;
            flag.store(false, Ordering::Release);
            let line = counts.line();
            let mut said = said.lock().unwrap_or_else(|p| p.into_inner());
            if said.1 != line {
                said.1 = line.clone();
                drop(said);
                log(&format!("[files] native media: {line}"));
            }
        });
    }

    /// Whether a prewarm, a download or a decode is running: the account is not reaped while it is (2470).
    pub fn busy(&self) -> bool {
        self.prewarms.warming() || self.downloads.running() > 0 || self.envelopes.flying()
    }

    /// The memos let go: a host short of memory (`media.shed`).
    pub fn shed(&self) {
        self.envelopes.shed();
        self.waves.shed();
    }

    /// The idle memos let go, at the reap.
    pub fn reap(&self) {
        self.envelopes.reap();
        self.waves.reap();
    }
}
