//! revocations.rs — each subject's "not before" epoch, `session-revocations.json`: read when the file changed,
//! and a revocation re-reads the file and keeps the later epoch of each subject.

use std::path::PathBuf;

use serde_json::{Map, Value};

use crate::json_file::{SharedJsonFile, js_number};

pub struct Revocations {
    file: SharedJsonFile,
}

impl Revocations {
    pub fn new(file: PathBuf) -> Self {
        Revocations {
            file: SharedJsonFile::new(file),
        }
    }

    /// The epoch (ms) before which `sub`'s sessions are refused, if any.
    pub fn not_before(&self, sub: &str) -> Option<f64> {
        self.file.read().get(sub).and_then(Value::as_f64)
    }

    /// Refuses every session of `sub` issued before `at`.
    pub fn revoke(&self, sub: &str, at: f64) -> std::io::Result<()> {
        let read = self.file.read();
        let mut all = match &*read {
            Value::Object(map) => map.clone(),
            _ => Map::new(),
        };
        let before = all.get(sub).and_then(Value::as_f64).unwrap_or(0.0);
        all.insert(sub.to_string(), js_number(before.max(at)));
        self.file.write(&Value::Object(all))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_other_writers_epochs_and_the_later_one() {
        let dir = std::env::temp_dir().join(format!("k7-auth-rev-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session-revocations.json");
        let r = Revocations::new(file.clone());
        assert_eq!(r.not_before("a"), None);
        std::fs::write(&file, "{\"a\": 500, \"b\": 7}").unwrap();
        r.revoke("a", 100.0).unwrap();
        r.revoke("c", 9.0).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "{\n  \"a\": 500,\n  \"b\": 7,\n  \"c\": 9\n}"
        );
        assert_eq!(r.not_before("b"), Some(7.0));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
