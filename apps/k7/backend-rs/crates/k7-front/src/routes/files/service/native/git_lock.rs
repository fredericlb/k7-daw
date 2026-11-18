//! git_lock.rs — two locks per workspace root: the workspace's, held by every write, and the remote's, held by whatever moves
//! the remote refs. What a reader gets wrong: nobody waits in line, and a commit never takes the remote's (docs/git.md).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::{Mutex as Async, OwnedMutexGuard};

/// Held until dropped; moved into a spawned task it outlives a client that hung up.
pub type Guard = OwnedMutexGuard<()>;

type Table = Mutex<HashMap<String, Arc<Async<()>>>>;

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(Default::default)
}

fn remote_table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(Default::default)
}

/// The workspace's lock, or `None` when another operation holds it.
pub fn try_take(root: &str) -> Option<Guard> {
    take_from(table(), root)
}

/// The lock on the remote refs and what a fetch writes, or `None` when a fetch, pull or push holds it.
pub fn try_take_remote(root: &str) -> Option<Guard> {
    take_from(remote_table(), root)
}

fn take_from(table: &'static Table, root: &str) -> Option<Guard> {
    let lock = table
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .entry(root.to_string())
        .or_default()
        .clone();
    lock.try_lock_owned().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_take_of_one_root_is_refused_until_the_first_is_dropped() {
        let first = try_take("/lock/a").expect("free");
        assert!(try_take("/lock/a").is_none());
        assert!(try_take("/lock/b").is_some(), "another root is free");
        drop(first);
        assert!(try_take("/lock/a").is_some());
        let remote = try_take_remote("/lock/c").expect("free");
        assert!(try_take_remote("/lock/c").is_none());
        assert!(try_take("/lock/c").is_some(), "the two locks are apart");
        drop(remote);
    }
}
