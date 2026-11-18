//! Shared by the change-set tests: the body made from ops, a plan over the entries fixture, a disk that fails its Nth rename.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use k7_file_service::atomic::{Fs, Real};
use k7_file_service::changes::Refusal;
use k7_file_service::changes::commit::commit;
use k7_file_service::changes::plan::{Planned, plan};
use k7_file_service::changes::request::parse;
use k7_file_service::space::Space;
use serde_json::{Value, json};

use super::entries::Fx;

pub const PROJECT: &str = "song.k7";

pub fn body(ops: Value) -> Value {
    json!({ "at": PROJECT, "base": "t1", "label": "verb line", "ops": ops })
}

pub fn planned(fx: &Fx, admin: bool, ops: Value) -> Result<Planned, Refusal> {
    let req = parse(&body(ops))?;
    let h = fx.s.home(admin);
    plan(&Space::open(&h, &fx.root), &req.ops)
}

pub fn refused(fx: &Fx, admin: bool, ops: Value) -> Refusal {
    planned(fx, admin, ops).expect_err("the set was planned")
}

pub fn text(file: &str, text: &str, was: Value) -> Value {
    json!({ "kind": "text", "file": file, "text": text, "was": was })
}

/// Every file and folder below `root`, hidden ones included, with its text.
pub fn tree(root: &str) -> BTreeMap<String, String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let rel = e
                .path()
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string()
                .replace('\\', "/");
            if e.path().is_dir() {
                out.insert(format!("{rel}/"), String::new());
                walk(&e.path(), root, out);
            } else {
                out.insert(
                    rel,
                    String::from_utf8_lossy(&std::fs::read(e.path()).unwrap()).into(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(
        std::path::Path::new(root),
        std::path::Path::new(root),
        &mut out,
    );
    out
}

/// The real disk, but the Nth rename (from 0) is refused once.
pub struct Flaky {
    pub renames: AtomicUsize,
    pub fail_at: usize,
}

impl Flaky {
    pub fn at(fail_at: usize) -> Flaky {
        Flaky {
            renames: AtomicUsize::new(0),
            fail_at,
        }
    }
}

impl Fs for Flaky {
    fn write_temp(&self, tmp: &str, data: &mut dyn io::Read, mode: Option<u32>) -> io::Result<()> {
        Real.write_temp(tmp, data, mode)
    }

    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        if self.renames.fetch_add(1, Ordering::SeqCst) == self.fail_at {
            return Err(io::Error::other("injected"));
        }
        Real.rename(from, to)
    }

    fn remove(&self, path: &str) {
        Real.remove(path)
    }

    fn mode_of(&self, path: &str) -> Option<u32> {
        Real.mode_of(path)
    }
}

pub fn land(
    fx: &Fx,
    ops: Value,
) -> Result<k7_file_service::changes::commit::Landed, k7_file_service::changes::commit::Failed> {
    let p = planned(fx, true, ops).expect("planned");
    commit(&Real, &p, &mut |_| {})
}

pub fn seed(fx: &Fx) {
    for (f, t) in [
        ("a.txt", "old a"),
        ("gone.txt", "g"),
        ("c.txt", "c"),
        ("m.txt", "m"),
    ] {
        fx.s.file(&format!("studio/workspace/songs/song.k7/{f}"), t);
    }
}
