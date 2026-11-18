//! thawed.rs — the modules a person is editing, left live per project folder, in the order they were thawed (2310), and what the
//! interactive asks tell the freezer: the last admin stamp (APPROVED, review B3 decisions 1 and 2).
//! What a reader gets wrong: the key is `resolve(dir)` as node spelled it, and the order is what 1o's compile key hashes (1505).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use k7_file_service::paths::resolve_all;

/// The thawed modules of every folder of one account; a state of the session, never of the song.
#[derive(Default)]
pub struct Thawed {
    folders: Mutex<HashMap<String, Vec<String>>>,
    admin: AtomicBool,
}

impl Thawed {
    fn dirs(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<String>>> {
        self.folders.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The stamp of the last request that came with one, as the TypeScript's `followAdmin` keeps it.
    pub fn note_admin(&self, admin: bool) {
        self.admin.store(admin, Ordering::Relaxed);
    }

    pub fn admin(&self) -> bool {
        self.admin.load(Ordering::Relaxed)
    }

    /// The modules left live in `dir`, first thawed first.
    pub fn of(&self, dir: &str) -> Vec<String> {
        self.dirs()
            .get(&resolve_all(&[dir]))
            .cloned()
            .unwrap_or_default()
    }

    /// Every folder's list, for the freezer's machine to start from.
    pub fn entries(&self) -> Vec<(String, Vec<String>)> {
        self.dirs()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Replaces the list whole, a repeated name keeping its first place; an empty list forgets the folder.
    pub fn set(&self, dir: &str, modules: impl IntoIterator<Item = String>) {
        let mut list: Vec<String> = Vec::new();
        for m in modules {
            if !list.contains(&m) {
                list.push(m);
            }
        }
        let key = resolve_all(&[dir]);
        if list.is_empty() {
            self.dirs().remove(&key);
        } else {
            self.dirs().insert(key, list);
        }
    }

    /// One module left live (`on`, after those already, an existing one keeping its place) or let go; one lock for the whole update.
    pub fn thaw(&self, dir: &str, module: &str, on: bool) {
        let key = resolve_all(&[dir]);
        let mut dirs = self.dirs();
        let list = dirs.entry(key.clone()).or_default();
        if on {
            if !list.iter().any(|m| m == module) {
                list.push(module.to_owned());
            }
        } else {
            list.retain(|m| m != module);
        }
        if list.is_empty() {
            dirs.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // The list's order is part of 1o's compile key, so a repeated knob turn must not move a module (review K3 fix 1).
    #[test]
    fn a_re_thaw_keeps_the_modules_place() {
        let t = Thawed::default();
        for m in ["b", "a", "b"] {
            t.thaw("/p", m, true);
        }
        assert_eq!(t.of("/p"), ["b", "a"]);
        t.thaw("/p", "b", false);
        t.thaw("/p", "a", false);
        assert!(t.of("/p").is_empty());
    }

    // Four writers, each thawing its own names, lose none (review K3 fix 2).
    #[test]
    fn concurrent_thaws_of_one_folder_lose_no_name() {
        let t = Arc::new(Thawed::default());
        let hs: Vec<_> = (0..4)
            .map(|w| {
                let t = t.clone();
                std::thread::spawn(move || {
                    for i in 0..50 {
                        t.thaw("/p", &format!("m{w}-{i}"), true);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(t.of("/p").len(), 200);
    }

    // What the freezer's compile reads of the interactive asks: the failed builds as the last ask left them, and the admin stamp.
    #[test]
    fn the_freezer_reads_what_the_last_interactive_ask_left() {
        let t = Thawed::default();
        assert!(!t.admin());
        t.note_admin(true);
        assert!(t.admin());
    }
}
