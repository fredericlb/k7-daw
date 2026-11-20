//! locks.rs — the account's write lock, keyed by the caller (1210).
//! What a reader gets wrong: only native writes take the lock, so holding it across a call
//! to the runtime cannot deadlock.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// Locks remembered before the dead ones are pruned.
const PRUNE_AT: usize = 256;

fn table() -> &'static Mutex<HashMap<String, Weak<AsyncMutex<()>>>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>> = OnceLock::new();
    TABLE.get_or_init(Mutex::default)
}

/// The lock named `root` (an account's studio home), in arrival order, held until the guard is dropped.
pub async fn write(root: &str) -> OwnedMutexGuard<()> {
    let lock = {
        let mut t = table().lock().unwrap_or_else(|p| p.into_inner());
        match t.get(root).and_then(Weak::upgrade) {
            Some(l) => l,
            None => {
                if t.len() >= PRUNE_AT {
                    t.retain(|_, w| w.strong_count() > 0);
                }
                let l = Arc::new(AsyncMutex::new(()));
                t.insert(root.to_string(), Arc::downgrade(&l));
                l
            }
        }
    };
    lock.lock_owned().await
}

/// How many locks the table still remembers, for a test.
#[cfg(test)]
pub fn remembered() -> usize {
    table().lock().unwrap_or_else(|p| p.into_inner()).len()
}
