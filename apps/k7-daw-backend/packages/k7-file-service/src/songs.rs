//! The native song compiler's sessions, one per workspace and shared: the front answers from them.
//! What a reader gets wrong: the lock is held while a session works, never across an await; a panic inside `with` retires that session.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use song::roots::spell;
use song::session::Session;
use song::{DiskSource, PathRule, Roots};

/// Workspaces held, the least recently asked dropped (decision 704).
pub const MAX_WORKSPACES: usize = 4;

/// A workspace key longer than this is no path.
const MAX_WS: usize = 8_192;

/// The sessions of an account, shared between its runtime's host and the front.
pub type Songs = Arc<SongSessions>;

/// A session was dropped because it panicked (or the budget took it): the next ask makes another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retired;

pub struct Slot {
    pub ws: String,
    pub session: Session,
    tick: u64,
    pub summarized: Instant,
    /// The renders the TypeScript found ready, by the name without its fingerprint: the native side finds them there (343).
    pub renders: BTreeMap<String, String>,
}

struct Sessions {
    slots: Vec<Slot>,
    tick: u64,
    budget: usize,
}

// A session crosses threads inside the Arc: the account's runtime thread and the front's blocking tasks (decision 1111).
const _: () = {
    const fn sendable<T: Send>() {}
    sendable::<Sessions>();
    sendable::<SongSessions>();
    const fn shareable<T: Send + Sync>() {}
    shareable::<SongSessions>();
};

impl Sessions {
    fn new(heap: usize) -> Sessions {
        // A runtime of a phone's size shares its memory with the whole backend: a quarter, not a half (decision 808).
        let budget = heap / 4;
        Sessions {
            slots: Vec::new(),
            tick: 0,
            budget,
        }
    }

    fn at(&mut self, ws: &str, rule: PathRule) -> usize {
        self.tick += 1;
        if let Some(i) = self.slots.iter().position(|s| s.ws == ws) {
            self.slots[i].tick = self.tick;
            return i;
        }
        if self.slots.len() >= MAX_WORKSPACES
            && let Some(oldest) = (0..self.slots.len()).min_by_key(|i| self.slots[*i].tick)
        {
            self.retire(oldest);
        }
        self.slots.push(Slot {
            ws: ws.to_owned(),
            session: Session::new(rule),
            tick: self.tick,
            summarized: Instant::now(),
            renders: BTreeMap::new(),
        });
        self.slots.len() - 1
    }

    /// Drops a session.
    fn retire(&mut self, i: usize) {
        self.slots.remove(i);
    }

    /// Whole sessions are dropped, the least recently asked first, and the one just asked last, which lets go of its answers.
    fn within_budget(&mut self, keep: &str) {
        let weight = |s: &Sessions| s.slots.iter().map(|s| s.session.bytes()).sum::<usize>();
        while weight(self) > self.budget {
            let victim = (0..self.slots.len())
                .filter(|i| self.slots[*i].ws != keep)
                .min_by_key(|i| self.slots[*i].tick);
            match victim {
                Some(i) => self.retire(i),
                None => break,
            }
        }
        if weight(self) > self.budget
            && let Some(slot) = self.slots.iter_mut().find(|s| s.ws == keep)
        {
            slot.session.shed();
        }
    }
}

/// The sessions behind one lock.
pub struct SongSessions {
    inner: Mutex<Sessions>,
}

impl SongSessions {
    /// Sessions for a runtime of `heap` bytes; the budget is decision 1506's.
    pub fn new(heap: usize) -> Songs {
        Arc::new(SongSessions {
            inner: Mutex::new(Sessions::new(heap)),
        })
    }

    /// A poisoned lock is taken as it stands: every panic inside is caught before it can poison.
    fn lock(&self) -> MutexGuard<'_, Sessions> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `f` over the session of the workspace `key` (as `key_of` spells it), made if none; a panic retires it.
    pub fn with<R>(
        &self,
        key: &str,
        rule: PathRule,
        f: impl FnOnce(&mut Session) -> R,
    ) -> Result<R, Retired> {
        self.with_slot(key, rule, |slot| f(&mut slot.session))
    }

    /// `with` over the whole slot, its renders included.
    pub fn with_slot<R>(
        &self,
        key: &str,
        rule: PathRule,
        f: impl FnOnce(&mut Slot) -> R,
    ) -> Result<R, Retired> {
        let mut sessions = self.lock();
        let at = sessions.at(key, rule);
        let slot = &mut sessions.slots[at];
        match catch_unwind(AssertUnwindSafe(|| f(slot))) {
            Ok(r) => {
                sessions.within_budget(key);
                Ok(r)
            }
            Err(_) => {
                sessions.retire(at);
                Err(Retired)
            }
        }
    }

    /// Drops the session of a workspace, spelled as the sessions are keyed or as given.
    pub fn forget(&self, key: &str, raw: &str) {
        let mut sessions = self.lock();
        if let Some(i) = sessions
            .slots
            .iter()
            .position(|s| s.ws == key || s.ws == raw)
        {
            sessions.retire(i);
        }
    }

    /// Every session but the kept workspace's lets go of its answers.
    pub fn shed_but(&self, key: &str, raw: &str) {
        for slot in &mut self.lock().slots {
            if slot.ws != key && slot.ws != raw {
                slot.session.shed();
            }
        }
    }

    /// How many sessions are held.
    pub fn held(&self) -> usize {
        self.lock().slots.len()
    }

    /// The bytes the sessions may weigh together.
    pub fn budget(&self) -> usize {
        self.lock().budget
    }
}

/// The key of a workspace: its spelling under the rule of the roots that name it (decision 704).
pub fn key_of(ws: &str, rule: PathRule) -> Result<String, String> {
    if ws.len() > MAX_WS {
        return Err(format!("a workspace named by more than {MAX_WS} bytes"));
    }
    spell(ws, rule).map_err(|e| format!("the workspace {ws:?}: {e}"))
}

/// The disk the roots mount, and the folders it holds.
pub fn disk_of(roots: &Roots) -> Result<(DiskSource, Vec<String>), String> {
    let (dirs, files) = roots.mounts().map_err(|e| e.to_string())?;
    let disk = DiskSource::with_files(&dirs, &files).map_err(|e| e.to_string())?;
    Ok((disk, dirs))
}

/// The renders a native compile used, touched so the next sweep keeps them (decision 735); every failure is silent.
pub fn touch_used(renders: &[String]) {
    use std::fs::{FileTimes, OpenOptions};
    use std::time::{Duration, SystemTime};
    // A render used within this long is left alone: a stat is one per render, a write is not.
    const FRESH: Duration = Duration::from_secs(3_600);
    let now = SystemTime::now();
    for path in renders {
        let touched = (|| {
            let meta = std::fs::metadata(path).ok()?;
            let used = meta.accessed().ok()?.max(meta.modified().ok()?);
            if now.duration_since(used).is_ok_and(|age| age < FRESH) {
                return Some(());
            }
            let file = OpenOptions::new().write(true).open(path).ok()?;
            file.set_times(FileTimes::new().set_accessed(now)).ok()
        })();
        let _ = touched;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1111: a lock poisoned by a panic elsewhere is taken as it stands, and the sessions go on answering.
    #[test]
    fn a_poisoned_lock_is_taken_with_into_inner() {
        let songs = SongSessions::new(96 << 20);
        let mine = songs.clone();
        let poisoned = std::thread::spawn(move || {
            let _held = mine.inner.lock().expect("the lock");
            panic!("a thread that dies holding the lock");
        })
        .join();
        assert!(poisoned.is_err() && songs.inner.is_poisoned());
        assert_eq!(songs.with("/w", PathRule::HOST, |s| s.held()), Ok(0));
    }
}
