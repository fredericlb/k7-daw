//! support.rs — a throwaway root, and a fixture tree laid and dumped.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

/// A fresh canonical folder, removed with the value.
pub struct Root(pub PathBuf);

impl Root {
    pub fn new() -> Root {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "k7-libraries-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // No `\\?\` prefix: the store spells a path with slashes, which a verbatim name does not survive.
        let real = fs::canonicalize(dir).unwrap();
        let text = real.to_string_lossy();
        Root(PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)))
    }

    pub fn text(&self) -> String {
        // Slashes only: the vectors, and every note a store writes, spell a path with `/`.
        self.0.to_string_lossy().replace('\\', "/")
    }

    /// `$ROOT` in a fixture's text is this root.
    pub fn sub(&self, s: &str) -> String {
        s.replace("$ROOT", &self.text())
    }

    pub fn lay(&self, tree: &Value) {
        for (p, text) in tree.as_object().into_iter().flatten() {
            let at = self.0.join(p);
            if p.ends_with('/') {
                fs::create_dir_all(at).unwrap();
            } else {
                fs::create_dir_all(at.parent().unwrap()).unwrap();
                fs::write(at, self.sub(text.as_str().unwrap_or_default())).unwrap();
            }
        }
    }

    /// Every file and empty folder, but the replica's own state (`.sync-*`), as the generator dumps them.
    pub fn dump(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        walk(&self.0, "", &self.text(), &mut out);
        out
    }

    /// A value with this root written `$ROOT`.
    pub fn norm(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => {
                // A served file is named by its canonical path (`\\?\C:\...`), which the root's plain slashed text must still meet.
                let plain = if cfg!(windows) {
                    s.replace('\\', "/").replace("//?/", "")
                } else {
                    s.clone()
                };
                Value::String(plain.replace(&self.text(), "$ROOT"))
            }
            Value::Array(a) => Value::Array(a.iter().map(|x| self.norm(x)).collect()),
            Value::Object(o) => {
                Value::Object(o.iter().map(|(k, x)| (k.clone(), self.norm(x))).collect())
            }
            other => other.clone(),
        }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn walk(at: &Path, rel: &str, root: &str, out: &mut BTreeMap<String, String>) {
    let mut names: Vec<String> = fs::read_dir(at)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    if names.is_empty() && !rel.is_empty() {
        out.insert(format!("{rel}/"), String::new());
    }
    for n in names {
        if n.starts_with(".sync-") {
            continue;
        }
        let abs = at.join(&n);
        let r = if rel.is_empty() {
            n.clone()
        } else {
            format!("{rel}/{n}")
        };
        if abs.is_dir() {
            walk(&abs, &r, root, out);
        } else {
            out.insert(
                r,
                String::from_utf8_lossy(&fs::read(&abs).unwrap()).replace(root, "$ROOT"),
            );
        }
    }
}
