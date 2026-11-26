//! ledger.rs — what is known to be the same on both sides, one per agent: the size and times of each file as
//! last synced, the downloads in flight (a second asker waits for the first), and the copies placed beside a
//! file something holds open. Keyed by the CANONICAL local path, whichever copy is current.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, MutexGuard};

use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Synced {
    pub size: u64,
    pub local_mtime_ms: f64,
    pub remote_mtime_ms: f64,
}

type Outcome = Option<Result<(), String>>;

#[derive(Default)]
struct Inner {
    synced: HashMap<String, Synced>,
    redirects: HashMap<String, String>,
    downloads: HashMap<String, watch::Receiver<Outcome>>,
}

#[derive(Default)]
pub struct SyncLedger {
    inner: Mutex<Inner>,
}

/// Takes the download's entry back however its owner ends, a cancelled one included.
struct Owner<'a> {
    ledger: &'a SyncLedger,
    local: &'a str,
}

impl Drop for Owner<'_> {
    fn drop(&mut self) {
        self.ledger.lock().downloads.remove(self.local);
    }
}

impl SyncLedger {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, local: &str) -> Option<Synced> {
        self.lock().synced.get(local).copied()
    }

    pub fn set(&self, local: &str, s: Synced) {
        self.lock().synced.insert(local.to_string(), s);
    }

    /// Run `fetch` unless a download of `local` is already in flight, then wait for whichever runs.
    pub async fn download_once(
        &self,
        local: &str,
        fetch: impl Future<Output = Result<(), String>>,
    ) -> Result<(), String> {
        let mine = {
            let mut st = self.lock();
            match st.downloads.get(local) {
                Some(rx) => Err(rx.clone()),
                None => {
                    let (tx, rx) = watch::channel(None);
                    st.downloads.insert(local.to_string(), rx);
                    Ok(tx)
                }
            }
        };
        match mine {
            Ok(tx) => {
                let _owner = Owner {
                    ledger: self,
                    local,
                };
                let done = fetch.await;
                let _ = tx.send(Some(done.clone()));
                done
            }
            Err(mut rx) => match rx.wait_for(Option::is_some).await {
                Ok(v) => v.clone().unwrap_or(Ok(())),
                Err(_) => Err("the download was abandoned".into()),
            },
        }
    }

    /// Where the current version of `canonical` lives when it could not replace a held file, if anywhere.
    pub fn redirect(&self, canonical: &str) -> Option<String> {
        self.lock().redirects.get(canonical).cloned()
    }

    /// The canonical path a redirected copy stands for.
    pub fn canonical_of(&self, actual: &str) -> Option<String> {
        let st = self.lock();
        st.redirects
            .iter()
            .find(|(_, at)| at.as_str() == actual)
            .map(|(c, _)| c.clone())
    }

    /// Point `canonical` at `actual` (itself to clear); answers the copy it pointed at before, if any.
    pub fn place(&self, canonical: &str, actual: &str) -> Option<String> {
        let mut st = self.lock();
        let before = st.redirects.get(canonical).cloned();
        if actual == canonical {
            st.redirects.remove(canonical);
        } else {
            st.redirects
                .insert(canonical.to_string(), actual.to_string());
        }
        before
    }
}
