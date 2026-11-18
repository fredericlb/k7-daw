//! shares — the mixdown, a share link and the picture's job held natively (1m, docs/file-shares.md): the front's
//! I/O over ports the account fills in (M3), so a fake renderer and a fake browser stand in for them.
//! What a reader gets wrong: nothing here takes the account's lock, and a link and a picture outlive their request.

pub mod disk;
pub mod encode;
pub mod export;
pub mod host;
pub mod picture;
pub mod ports;
pub mod registry;
pub mod report;
pub mod routes;
pub mod tracks;

#[cfg(test)]
mod fakes;
#[cfg(test)]
mod tests_export;
#[cfg(test)]
mod tests_picture;

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use serde_json::Value;

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What a share reads of the account: its workspace, its stores, its scratch folder and the environment its children get.
pub struct Account {
    pub root: PathBuf,
    pub tmp: PathBuf,
    pub env: BTreeMap<String, String>,
}

impl Account {
    pub fn var(&self, k: &str) -> Option<String> {
        self.env.get(k).cloned()
    }

    /// The account's environment as a child gets it, and nothing of the front's own.
    pub fn env_pairs(&self) -> Vec<(String, String)> {
        self.env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// What a render wrote: the mixdown's WAV and each stem's, by file name in the folder it was given.
pub struct Manifest {
    pub mix: String,
    pub stems: Vec<Stem>,
}

pub struct Stem {
    pub name: String,
    pub file: String,
}

/// 1k's renderer: a compiled engine document rendered into a folder.
pub trait Mixdown: Send + Sync {
    /// Whether some machine can render: `requireRenderer` comes before anything else the route reads.
    fn available(&self) -> bool {
        true
    }
    fn render<'a>(
        &'a self,
        doc: &'a Value,
        out: &'a Path,
        stems: bool,
    ) -> BoxFut<'a, Result<Manifest, String>>;
}

/// A project's engine document for a mixdown, and its name.
pub struct MixDoc {
    pub doc: Value,
    pub name: String,
}

/// 1o's views: what the song compiler answers, here from a session or a fake.
pub trait Views: Send + Sync {
    /// The document of `dir`, or of one `timeline` of it as it is auditioned; a timeline the project lacks is the sentence.
    fn mix_doc<'a>(
        &'a self,
        dir: &'a Path,
        timeline: Option<&'a str>,
    ) -> BoxFut<'a, Result<MixDoc, String>>;
    /// Where each node's params come from (`{ node: { extends, own, from } }`); empty when the files will not read.
    fn origins(&self, _dir: &Path) -> Value {
        Value::Object(Default::default())
    }
}

/// Every port a share's jobs call.
pub struct Ports {
    pub mixdown: Arc<dyn Mixdown>,
    pub views: Arc<dyn Views>,
    pub browser: Arc<dyn picture::Browser>,
}

/// What the front counts for the log line (2573).
#[derive(Default)]
pub struct Counts {
    pub exports: AtomicU64,
    pub pictures: AtomicU64,
    pub declines: AtomicU64,
    pub panics: AtomicU64,
}

/// One account's export state: the picture slot, counting into the front's counts; it outlives a runtime (2540, 2555).
#[derive(Default)]
pub struct Shares {
    pub counts: Arc<Counts>,
    picture: Mutex<bool>,
}

/// The picture render of an account, held until dropped.
pub struct PictureSlot(Arc<Shares>);

impl Shares {
    /// Shares counting into counts of their own, which the tests read apart from the front's.
    #[cfg(test)]
    pub fn new() -> Arc<Shares> {
        Arc::new(Shares::default())
    }

    /// An account's shares counting into the front's own counts, which every account adds to (2573).
    pub fn counting_into(counts: Arc<Counts>) -> Arc<Shares> {
        Arc::new(Shares {
            counts,
            ..Shares::default()
        })
    }

    /// Whether a picture render is in flight: the account is kept while it is.
    pub fn busy(&self) -> bool {
        *self.picture.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Whether the account's picture render is in flight.
    pub fn picturing(&self) -> bool {
        *self.picture.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The account's one picture render, or `None` when one runs (2555).
    pub fn begin_picture(self: &Arc<Self>) -> Option<PictureSlot> {
        let mut running = self.picture.lock().unwrap_or_else(|p| p.into_inner());
        if std::mem::replace(&mut *running, true) {
            return None;
        }
        Some(PictureSlot(self.clone()))
    }
}

impl Drop for PictureSlot {
    fn drop(&mut self) {
        *self.0.picture.lock().unwrap_or_else(|p| p.into_inner()) = false;
    }
}
