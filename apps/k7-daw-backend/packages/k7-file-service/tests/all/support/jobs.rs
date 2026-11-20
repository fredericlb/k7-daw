//! support/jobs.rs — the engine's jobs vectors (`vectors/jobs.json`, made by `js/makeVectors.mts`) and what every jobs test shares.
//! What a reader gets wrong: the file is read whole once per test, and a path in it is spelled `{base}` for the case's own folder.

use serde_json::Value;

pub fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/jobs.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The rows of one section of the vectors.
pub fn section(all: &Value, name: &str) -> Vec<Value> {
    all[name]
        .as_array()
        .unwrap_or_else(|| panic!("no section {name}"))
        .clone()
}

/// `base` as the vectors spell a folder: `/`-separated, never `\\?\`.
pub fn spelled(base: &std::path::Path) -> String {
    let s = base.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").unwrap_or(&s).to_owned()
}

/// A scratch folder with the helpers the jobs tests use: paths spelled `/`, files written with their folders made.
pub struct Tmp(super::Scratch);

impl Tmp {
    /// The folder is named by a counter too: `Scratch` keys on the name and the process, and two tests of one name raced for it.
    pub fn new(name: &str) -> Tmp {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Tmp(super::Scratch::new(&format!("{name}-{n}")))
    }

    /// `rel` under the folder; `at("")` is the folder itself, with a trailing `/`.
    pub fn at(&self, rel: &str) -> String {
        let root = spelled(&self.0.0);
        if rel.is_empty() {
            format!("{root}/")
        } else {
            format!("{root}/{rel}")
        }
    }

    pub fn mkdir(&self, rel: &str) {
        std::fs::create_dir_all(self.at(rel)).unwrap();
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.at(rel);
        if let Some(parent) = std::path::Path::new(&p).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, text).unwrap();
    }
}
