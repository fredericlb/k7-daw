//! journal — the undo journal kept natively, byte for byte the TypeScript's (1720). `Journals` holds, per workspace root, the
//! gate records and undos queue on (1730), the records in flight and the journal loaded for the active project (1725).
//! What a reader gets wrong: nothing here is a second journal, both implementations read and write one folder; and a
//! record's heavy work runs on the caller's thread, so the front puts `begin_with` and `end_now` on a blocking one.

pub mod apply;
pub mod blobs;
pub mod diff;
pub mod format;
pub mod image;
pub mod lossy;
pub mod put;
mod record;
pub mod ring;
pub mod store;
mod undo;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tokio::sync::RwLock;

use self::format::EngineRef;
use self::image::{Image, OMap, TextCache};
use self::store::{Held, Store};
use crate::libraries::Home;
use crate::revisions::Revisions;
use crate::tree::Limits;

pub use record::{Begun, Ended, rel_of};
pub use undo::{Failure, Planned, Undo, Undone};

/// A staged pre-image older than this, owned by no record in flight, is a crash's (1726).
const STAGED_AGE: Duration = Duration::from_secs(600);

/// How a mutation lands in the journal: its own step, folded into the one on top, or not at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Step,
    Join,
    Skip,
}

/// Who began a record: a request, or the runtime of a verb (1745).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Owner(pub u64);

impl Owner {
    /// A number unique in the process: the front's requests and every runtime mint theirs here (1745).
    pub fn mint() -> Owner {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Owner(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// A record in flight: its "before" side, rebased by every record that lands while it runs (1737).
pub(crate) struct Rec {
    pub _guard: tokio::sync::OwnedRwLockReadGuard<()>,
    pub home: Home,
    pub root: String,
    pub dir: Option<String>,
    pub label: String,
    pub mode: Mode,
    pub owner: Owner,
    /// `None` when the walk reached a limit: the record journals nothing (1739).
    pub before: Option<Image>,
    /// Staged copies of the files `snapshot` named, by rel path.
    pub pre: OMap<String>,
    pub staged: usize,
}

pub(crate) struct Open {
    pub dir: String,
    pub store: Store,
    pub held: Held,
}

#[derive(Default)]
pub(crate) struct State {
    pub records: HashMap<u64, Rec>,
    pub open: Option<Open>,
    pub said: HashSet<String>,
}

pub(crate) struct Root {
    pub gate: Arc<RwLock<()>>,
    pub state: Mutex<State>,
    /// Ends of this root one at a time, from the record's removal to its step's push (1737).
    pub landing: Mutex<()>,
    pub text: Mutex<TextCache>,
    /// The text bytes of the last image taken, what a record in flight holds (1780a).
    pub image_bytes: std::sync::atomic::AtomicUsize,
    /// Records ever begun on this root, a writer's trace that outlives the record.
    pub begun: AtomicU64,
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The process's journals, one set of records for the front's writes and the verbs.
pub struct Journals {
    pub(crate) roots: Mutex<HashMap<String, Arc<Root>>>,
    pub(crate) by_id: Mutex<HashMap<u64, Arc<Root>>>,
    pub(crate) next: AtomicU64,
    pub(crate) revisions: &'static Revisions,
    pub(crate) limits: Limits,
    pub(crate) max_entries: usize,
    /// The ring's clock in milliseconds; a test's own.
    pub(crate) clock: fn() -> u64,
}

pub fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Journals {
    pub fn global() -> &'static Journals {
        static GLOBAL: OnceLock<Journals> = OnceLock::new();
        GLOBAL.get_or_init(|| {
            Journals::with(
                Revisions::global(),
                Limits::default(),
                ring::MAX_ENTRIES,
                wall_ms,
            )
        })
    }

    pub fn with(
        revisions: &'static Revisions,
        limits: Limits,
        max_entries: usize,
        clock: fn() -> u64,
    ) -> Journals {
        Journals {
            roots: Mutex::new(HashMap::new()),
            by_id: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            revisions,
            limits,
            max_entries,
            clock,
        }
    }

    pub(crate) fn root(&self, root: &str) -> Arc<Root> {
        let mut roots = locked(&self.roots);
        Arc::clone(roots.entry(root.to_string()).or_insert_with(|| {
            Arc::new(Root {
                gate: Arc::new(RwLock::new(())),
                state: Mutex::new(State::default()),
                landing: Mutex::new(()),
                text: Mutex::new(TextCache::default()),
                image_bytes: std::sync::atomic::AtomicUsize::new(0),
                begun: AtomicU64::new(0),
            })
        }))
    }

    /// Records ever begun on `root`: a record that began and ended between two reads still moves it.
    pub fn begun(&self, root: &str) -> u64 {
        self.root(root)
            .begun
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Records in flight on `root`.
    pub fn in_flight(&self, root: &str) -> usize {
        locked(&self.root(root).state).records.len()
    }

    /// Every record a dropped runtime began leaves the gate and journals nothing (1745); how many.
    pub fn abandon(&self, owner: Owner) -> usize {
        self.abandon_where(|r| r.owner == owner)
    }

    /// One record let go, whoever began it: the lease of a record whose runtime is still waiting on a call is not this (1745).
    pub fn abandon_record(&self, id: u64) -> bool {
        let Some(root) = locked(&self.by_id).remove(&id) else {
            return false;
        };
        let rec = locked(&root.state).records.remove(&id);
        rec.inspect(record::unstage).is_some()
    }

    /// Whether the record `id` is still in flight.
    pub fn holds(&self, id: u64) -> bool {
        locked(&self.by_id).contains_key(&id)
    }

    fn abandon_where(&self, gone: impl Fn(&Rec) -> bool) -> usize {
        let roots: Vec<Arc<Root>> = locked(&self.roots).values().cloned().collect();
        let mut n = 0;
        for root in roots {
            let dead: Vec<(u64, Rec)> = {
                let mut st = locked(&root.state);
                let ids: Vec<u64> = st
                    .records
                    .iter()
                    .filter(|(_, r)| gone(r))
                    .map(|(i, _)| *i)
                    .collect();
                ids.into_iter()
                    .filter_map(|i| st.records.remove(&i).map(|r| (i, r)))
                    .collect()
            };
            for (id, rec) in dead {
                locked(&self.by_id).remove(&id);
                record::unstage(&rec);
                n += 1;
            }
        }
        n
    }

    /// The journal and the text image of every root nothing records on are let go (1773); the next use reads the disk.
    pub fn shed(&self) {
        for root in locked(&self.roots).values() {
            let mut st = locked(&root.state);
            if st.records.is_empty() {
                st.open = None;
                locked(&root.text).clear();
            }
        }
    }

    /// The gate taken alone, after every record in flight (1730); the account's lock comes after it (1733).
    pub async fn exclusive(&self, root: &str) -> tokio::sync::OwnedRwLockWriteGuard<()> {
        self.root(root).gate.clone().write_owned().await
    }

    /// What reading the loaded journal repaired, handed out once per read of the file (1796).
    pub fn take_lossy(&self, root: &str) -> lossy::Lossy {
        let r = self.root(root);
        let mut st = locked(&r.state);
        st.open
            .as_mut()
            .map_or_else(Default::default, |o| o.held.take_lossy())
    }

    /// `GET /history`'s answer from the journal on disk (1771).
    pub fn state_of(&self, home: &Home, root: &str, dir: &str) -> Value {
        self.read(home, root, dir, ring::state)
    }

    /// `GET /history/entries`' answer, the paths as recorded.
    pub fn list_of(&self, home: &Home, root: &str, dir: &str) -> Vec<Value> {
        self.read(home, root, dir, ring::list)
    }

    /// The step an undo (or a redo) would take, as the route's peek.
    pub fn peek_of(&self, home: &Home, root: &str, dir: &str, redo: bool) -> Option<Value> {
        self.read(home, root, dir, |j| {
            if redo {
                ring::peek_redo(j)
            } else {
                ring::peek_undo(j)
            }
        })
    }

    fn read<T>(
        &self,
        home: &Home,
        root: &str,
        dir: &str,
        f: impl FnOnce(&format::Journal) -> T,
    ) -> T {
        let r = self.root(root);
        let mut st = locked(&r.state);
        let open = Self::open_in(&mut st, home, dir);
        f(&open.held.journal)
    }

    /// The journal of `dir` loaded or read again if its file moved (1722, 1725); nothing on disk changes.
    pub(crate) fn open_in<'a>(st: &'a mut State, home: &Home, dir: &str) -> &'a mut Open {
        if st.open.as_ref().is_none_or(|o| o.dir != dir) {
            let store = Store::new(store::store_for(home, dir));
            // What a crash left is swept as a journal is first loaded; a record's own pre-images stay (1726).
            let live: HashSet<String> = st
                .records
                .values()
                .flat_map(|r| r.pre.iter().map(|(_, f)| f.clone()))
                .collect();
            store.sweep_except(STAGED_AGE, &|f| live.contains(f));
            st.open = Some(Open {
                dir: dir.to_string(),
                held: Held::read(&store),
                store,
            });
        }
        let open = st.open.as_mut().expect("opened");
        open.held.refresh(&open.store);
        open
    }

    /// Persist the journal of `open`, setting a torn file aside first and saying so once (1754).
    pub(crate) fn save(&self, st: &mut State) -> Result<(), String> {
        let Some(open) = st.open.as_mut() else {
            return Ok(());
        };
        let torn = open.held.torn.clone();
        let aside = open
            .held
            .save(&open.store, (self.clock)())
            .map_err(|e| e.to_string())?;
        if torn.is_some() && st.said.insert(format!("torn {}", open.store.dir)) {
            let name = crate::paths::basename(&open.store.dir);
            eprintln!(
                "[files] native: journal of {name} did not read, kept {}",
                if aside.is_some() {
                    "aside"
                } else {
                    "where it was"
                }
            );
        }
        Ok(())
    }

    /// The engine a step's blobs were handed to is the caller's to name; this only carries it.
    pub fn engine_of(id: &str, name: &str) -> EngineRef {
        EngineRef::new(id, name)
    }
}
