//! common — what the rooms' tests share: a stand-in for the k7x half, the relay's options, and a folder read whole.
//! What a reader gets wrong: the stand-in merges nothing, so a whole text over a moved value is always set aside.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use k7_sync::relay::Options;
use k7_sync::settle::{Js, JsError, Settled};
use serde_json::Value;

/// k7x ops are whole-file sets of their last op's `k7x`; a merge never succeeds.
pub struct FakeJs;

impl Js for FakeJs {
    fn settle(
        &self,
        current: Option<String>,
        op: Value,
        _bases: Vec<String>,
    ) -> Result<Settled, JsError> {
        if op["kind"] != "k7x" {
            return Ok(Settled {
                apply: None,
                aside: vec![(op, None)],
                next: Ok(None),
            });
        }
        let next = self.apply(current, op.clone()).map_err(|_| ());
        Ok(Settled {
            apply: Some(op),
            aside: Vec::new(),
            next,
        })
    }

    fn apply(&self, _current: Option<String>, op: Value) -> Result<Option<String>, JsError> {
        let last = op["ops"]
            .as_array()
            .and_then(|o| o.last())
            .cloned()
            .unwrap_or(Value::Null);
        match last["k7x"].as_str() {
            Some(t) => Ok(Some(t.to_string())),
            None => Err(JsError {
                name: "Error".into(),
                message: "fake: no k7x".into(),
            }),
        }
    }
}

/// Options for a relay keeping `account`'s rooms under `root/<account>`.
pub fn options(root: &Path) -> Options {
    let root = root.to_path_buf();
    let mut o = Options::new(Arc::new(move |slug: &str| root.join(slug)));
    o.log = Arc::new(|_| {});
    o
}

/// Write `files` (POSIX paths) under `dir`.
pub fn write_files(dir: &Path, files: &serde_json::Map<String, Value>) {
    for (rel, text) in files {
        let p: PathBuf = rel.split('/').fold(dir.to_path_buf(), |p, s| p.join(s));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text.as_str().unwrap()).unwrap();
    }
}

/// A fresh folder under the system's temp.
pub fn temp(name: &str) -> PathBuf {
    let mut n = [0u8; 6];
    getrandom::fill(&mut n).unwrap();
    let hex: String = n.iter().map(|b| format!("{b:02x}")).collect();
    let d = std::env::temp_dir().join(format!("k7-sync-{name}-{hex}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}
