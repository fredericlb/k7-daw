//! A workspace with one project, a leaked set of tokens and journals over it: what the journal's tests stand on.

use std::time::Instant;

use k7_file_service::atomic::Real;
use k7_file_service::confine::Disk;
use k7_file_service::journal::apply::Writer;
use k7_file_service::journal::format::EngineRef;
use k7_file_service::journal::{Failure, Journals, Mode, Owner, Undo, Undone, wall_ms};
use k7_file_service::libraries::Home;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;
use k7_file_service::watch::Feed;

use super::Scratch;

struct NoFeed;

impl Feed for NoFeed {
    fn generation(&self, _root: &str) -> Option<u64> {
        None
    }
}

pub struct Bed {
    pub s: Scratch,
    pub home: Home,
    pub root: String,
    pub dir: String,
    pub j: &'static Journals,
    pub revs: &'static Revisions,
}

pub const ME: Owner = Owner(1);

impl Bed {
    pub fn new(name: &str) -> Bed {
        Bed::with(name, Limits::default(), 1000)
    }

    pub fn with(name: &str, limits: Limits, ring: usize) -> Bed {
        let s = Scratch::new(name);
        let root = s.dir("ws");
        let dir = s.dir("ws/proj.k7");
        let home = s.home(false);
        let revs: &'static Revisions = Box::leak(Box::new(Revisions::with_feed(
            Box::new(NoFeed),
            Instant::now,
        )));
        let j: &'static Journals = Box::leak(Box::new(Journals::with(revs, limits, ring, wall_ms)));
        Bed {
            s,
            home,
            root,
            dir,
            j,
            revs,
        }
    }

    /// A file of the project, by project-relative path.
    pub fn at(&self, rel: &str) -> String {
        k7_file_service::paths::join(&[&self.dir, rel])
    }

    pub fn put(&self, rel: &str, text: &str) {
        let p = self.at(rel);
        std::fs::create_dir_all(k7_file_service::paths::dirname(&p)).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub fn text(&self, rel: &str) -> String {
        std::fs::read_to_string(self.at(rel)).unwrap()
    }

    pub fn exists(&self, rel: &str) -> bool {
        std::path::Path::new(&self.at(rel)).exists()
    }

    /// A record around `f`, the instant between the two images being the closure.
    pub async fn record(
        &self,
        label: &str,
        mode: Mode,
        f: impl FnOnce(u64),
    ) -> k7_file_service::journal::Ended {
        let b = self
            .j
            .begin(&self.home, &self.root, Some(&self.dir), label, mode, ME)
            .await;
        f(b.id);
        self.j.end(b.id, None).await
    }

    pub async fn step(&self, label: &str, f: impl FnOnce()) -> k7_file_service::journal::Ended {
        self.record(label, Mode::Step, |_| f()).await
    }

    pub fn labels(&self) -> Vec<String> {
        self.j
            .list_of(&self.home, &self.root, &self.dir)
            .iter()
            .map(|e| e["label"].as_str().unwrap().to_string())
            .collect()
    }

    pub fn confine(&self) -> impl Fn(&str) -> Result<String, String> + Sync + '_ {
        move |rel| {
            Space::open(&self.home, &self.root)
                .safe_path(rel, &Disk)
                .map_err(|e| e.to_string())
        }
    }

    pub async fn undo_with(
        &self,
        redo: bool,
        bound: Option<EngineRef>,
        expect: Option<u64>,
    ) -> Result<Undone, Failure> {
        let confine = self.confine();
        let w = Writer {
            fs: &Real,
            retry: false,
            sleep: &std::thread::sleep,
        };
        let u = Undo {
            home: &self.home,
            root: &self.root,
            dir: &self.dir,
            redo,
            expect,
            bound,
            confine: &confine,
            writer: &w,
            fetched: &[],
        };
        self.j.undo(&u).await
    }

    /// An undo or redo on a disk of the test's own (a counting one).
    pub async fn undo_on(&self, redo: bool, fs: &dyn k7_file_service::atomic::Fs) -> Undone {
        // The gates' own cost is not what a counting disk measures: paths are joined to the root.
        let confine = |rel: &str| Ok::<String, String>(format!("{}/{rel}", self.root));
        let w = Writer {
            fs,
            retry: false,
            sleep: &std::thread::sleep,
        };
        let u = Undo {
            home: &self.home,
            root: &self.root,
            dir: &self.dir,
            redo,
            expect: None,
            bound: None,
            confine: &confine,
            writer: &w,
            fetched: &[],
        };
        self.j.undo(&u).await.expect("undo")
    }

    pub async fn undo(&self) -> Undone {
        self.undo_with(false, None, None).await.expect("undo")
    }

    pub async fn redo(&self) -> Undone {
        self.undo_with(true, None, None).await.expect("redo")
    }
}

/// The label an applied step reports.
pub fn moved(u: &Undone) -> String {
    match u {
        Undone::Moved(v) => v["label"].as_str().unwrap().to_string(),
        other => panic!("{other:?}"),
    }
}
