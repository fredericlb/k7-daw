//! store.rs — the per-key store the list lives in, and the one the host installed: iCloud's key-value store on Apple
//! devices, nothing elsewhere. What a reader gets wrong: `NoStore` is the default, so a host that installs nothing
//! has no list at all, and no route answers an error for it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

/// The key prefixes of k7's pairs, the workspace list's and the Forgejo servers'; a host's store passes these only.
pub const KEY_PREFIXES: [&str; 2] = [crate::entry::PREFIX, crate::forges::PREFIX];

/// A store of string keys to string values; each key is its own last-writer-wins cell.
pub trait ListStore: Send + Sync {
    /// Every pair in the store.
    fn pairs(&self) -> Vec<(String, String)>;
    /// Writes a pair, or removes the key when `value` is none.
    fn put(&self, key: &str, value: Option<&str>);
    /// Whether this host has a store at all.
    fn available(&self) -> bool {
        true
    }
}

/// The host with no store: Android, Windows, Linux.
pub struct NoStore;

impl ListStore for NoStore {
    fn pairs(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    fn put(&self, _: &str, _: Option<&str>) {}
    fn available(&self) -> bool {
        false
    }
}

/// A store in memory: the seam of every test.
#[derive(Default)]
pub struct Fake(pub Mutex<BTreeMap<String, String>>);

impl ListStore for Fake {
    fn pairs(&self) -> Vec<(String, String)> {
        let m = self.0.lock().unwrap_or_else(|p| p.into_inner());
        m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    fn put(&self, key: &str, value: Option<&str>) {
        let mut m = self.0.lock().unwrap_or_else(|p| p.into_inner());
        match value {
            Some(v) => m.insert(key.to_string(), v.to_string()),
            None => m.remove(key),
        };
    }
}

static HOST: RwLock<Option<Arc<dyn ListStore>>> = RwLock::new(None);

/// The host's store, once, at start; a later call replaces it.
pub fn install(store: Arc<dyn ListStore>) {
    *HOST.write().unwrap_or_else(|p| p.into_inner()) = Some(store);
}

/// The installed store, or `NoStore`.
pub fn current() -> Arc<dyn ListStore> {
    HOST.read()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_else(|| Arc::new(NoStore))
}
