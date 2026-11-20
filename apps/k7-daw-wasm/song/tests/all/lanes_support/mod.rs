// lanes_support/mod.rs — what the 2e tests share: a project read from files held in memory, a rig from JSON, a Source that fails one path.
// A reader would get wrong that the files are laid under the project (`/p/ws/proj`), the libraries under `/p/lib`.

#![allow(dead_code)]

mod cases;
mod gate;
#[allow(unused_imports)]
pub use cases::*;
#[allow(unused_imports)]
pub use gate::*;

use crate::support::{mem, mem_world_roots};
use serde_json::{Value, json};
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::rig::Rig;
use song::{Cache, Entry, Source, SourceError, Span, Stat};

/// The files of a project (paths below the project), plus any library file written as `../../lib/...`.
pub fn laid(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(rel, text)| {
            let path = match rel.strip_prefix("lib/") {
                Some(_) => (*rel).to_owned(),
                None => format!("ws/proj/{rel}"),
            };
            (path, (*text).to_owned())
        })
        .collect()
}

pub fn inputs_with(files: &[(&str, &str)], rig: &Value) -> Inputs {
    let owned = laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    load_inputs(&src, &mut Cache::new(), &mem_world_roots(), Rig::from_json(rig)).expect("inputs")
}

pub fn inputs(files: &[(&str, &str)]) -> Inputs {
    inputs_with(files, &json!({}))
}

/// `extends("@meta/track")` with the lines given after it.
pub fn lane(extra: &str) -> String {
    format!("extends(\"@meta/track\")\n{extra}")
}

/// A Source over another that answers `Io` for one path and everything below it.
pub struct Failing<'a> {
    pub inner: &'a dyn Source,
    pub path: String,
}

impl Source for Failing<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        if path == self.path {
            return Err(SourceError::Io("denied".into()));
        }
        self.inner.read(path, span)
    }
}

/// Runs `f`, asserts it took under `secs` seconds, and gives back what it answered.
pub fn fast<T>(what: &str, secs: u64, f: impl FnOnce() -> T) -> T {
    let t = std::time::Instant::now();
    let out = f();
    let secs = if cfg!(target_env = "musl") { secs * 6 } else { secs };
    assert!(t.elapsed().as_secs() < secs, "{what} took {:?}", t.elapsed());
    out
}

/// Like `fast`, for a closure that can run again: passes when one of three runs is under the bound, since a quadratic cost is slow on every run and a loaded machine on some.
pub fn fast_retry<T>(what: &str, secs: u64, mut f: impl FnMut() -> T) -> T {
    let secs = if cfg!(target_env = "musl") { secs * 6 } else { secs };
    let mut took = Vec::new();
    for _ in 0..3 {
        let t = std::time::Instant::now();
        let out = f();
        if t.elapsed().as_secs() < secs {
            return out;
        }
        took.push(t.elapsed());
    }
    panic!("{what} took {took:?}");
}
