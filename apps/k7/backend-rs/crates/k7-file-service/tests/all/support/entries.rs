//! Shared by the entries' tests: a workspace with a project, a bin, two libraries, and a fake disk spelling `C:/`.

#![allow(dead_code)]

use std::collections::HashSet;

use k7_file_service::confine::{Disk, Probe as GateProbe};
use k7_file_service::entries::plan::plan;
use k7_file_service::entries::{EntryError, Op, Outcome, Probe};
use k7_file_service::libraries::Home;
use k7_file_service::space::Space;

use super::{Fake, Scratch};
use k7_file_service::entries::plan::answer_of;

/// One workspace `songs` holding the project `song.k7`, beside a library `k7-core` and the account's own.
pub struct Fx {
    pub s: Scratch,
    pub root: String,
}

impl Fx {
    pub fn new(name: &str) -> Fx {
        let s = Scratch::new(name);
        let root = s.dir("studio/workspace/songs");
        s.dir("studio/workspace/songs/song.k7/clips/takes");
        s.dir("studio/workspace/songs/song.k7/sequences");
        s.file(
            "studio/workspace/songs/song.k7/clips/riff.k7x",
            "source(\"riff.wav\")\n",
        );
        s.file(
            "studio/workspace/songs/song.k7/project.k7x",
            "project({ name: \"song\" })\n",
        );
        s.file("studio/workspace/.active-projects/songs", "song.k7");
        s.file(
            "libraries/k7-core/modules/audio/verb/module.k7x",
            "extends(\"@engine/reverb\")\n",
        );
        s.dir("studio/workspace/songs/_modules/audio");
        s.dir("studio/library");
        Fx { s, root }
    }

    /// An entry of the workspace by its rel.
    pub fn at(&self, rel: &str) -> String {
        format!("{}/{rel}", self.root)
    }

    /// A path of the first library.
    pub fn lib(&self, rel: &str) -> String {
        self.s.path(&format!("libraries/k7-core/{rel}"))
    }

    /// The plan of `op` for an account, an admin or not, over the real disk.
    pub fn run(&self, admin: bool, op: Op<'_>) -> Result<Outcome, EntryError> {
        let h = self.s.home(admin);
        plan(&Space::open(&h, &self.root), &op, &Disk)
    }

    pub fn user(&self, op: Op<'_>) -> Result<Outcome, EntryError> {
        self.run(false, op)
    }

    pub fn admin(&self, op: Op<'_>) -> Result<Outcome, EntryError> {
        self.run(true, op)
    }

    /// The refusal's sentence, or a panic when the plan went through.
    pub fn refusal(&self, admin: bool, op: Op<'_>) -> String {
        match self.run(admin, op) {
            Err(e) => e.to_string(),
            Ok(o) => panic!("{op:?} was planned: {o:?}"),
        }
    }
}

/// A disk spelling a `C:/` workspace: what is present, and which of it is a folder.
pub struct Drive {
    pub gate: Fake,
    pub dirs: HashSet<String>,
}

impl Drive {
    pub fn new(files: &[&str], dirs: &[&str]) -> Drive {
        let all: Vec<&str> = files.iter().chain(dirs.iter()).copied().collect();
        Drive {
            gate: Fake::with(&all, &[]),
            dirs: dirs.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl GateProbe for Drive {
    fn present(&self, p: &str) -> bool {
        self.gate.present(p)
    }

    fn real(&self, p: &str) -> Option<String> {
        self.gate.real(p)
    }
}

impl Probe for Drive {
    fn exists(&self, p: &str) -> bool {
        self.gate.present.contains(p)
    }

    fn is_dir(&self, p: &str) -> bool {
        self.dirs.contains(p)
    }
}

/// The home of a `C:/` account.
pub fn drive_home(admin: bool) -> Home {
    Home {
        studio: "C:/Work/studio".into(),
        libraries: "C:/Work/libs".into(),
        admin,
    }
}

pub fn folder<'a>(dir: &'a str, name: &'a str) -> Op<'a> {
    Op::Folder { dir, name }
}
pub fn rename<'a>(path: &'a str, name: &'a str) -> Op<'a> {
    Op::Rename { path, name }
}
pub fn mv<'a>(path: &'a str, to_dir: &'a str) -> Op<'a> {
    Op::Move { path, to_dir }
}
pub fn delete(path: &str) -> Op<'_> {
    Op::Delete { path }
}
pub fn upload<'a>(dir: &'a str, name: &'a str) -> Op<'a> {
    Op::Upload { dir, name }
}

/// The body a plan answers, spelt as the route does.
pub fn said(fx: &Fx, admin: bool, op: Op<'_>) -> serde_json::Value {
    let h = fx.s.home(admin);
    let space = Space::open(&h, &fx.root);
    answer_of(&space, &fx.run(admin, op).unwrap())
}
