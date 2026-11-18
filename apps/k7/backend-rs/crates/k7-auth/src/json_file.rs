//! json_file.rs — a JSON file the TypeScript backend writes too, handled as `packages/oidc/jsonFile.ts`
//! handles it: a read costs one stat and parses again only when the file changed, a write is a temporary file
//! renamed over it. What a reader gets wrong: there is no lock, so two writers in one instant lose one write
//! (`packages/oidc/docs/apiTokens.md` says why that window is accepted).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde_json::Value;

/// What tells one version of the file from another: a rename gives a new inode, a write a new time or size.
#[derive(Clone, PartialEq, Debug)]
struct Signature {
    inode: u64,
    modified: Option<SystemTime>,
    len: u64,
}

fn signature(path: &Path) -> Option<Signature> {
    let meta = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&meta);
    #[cfg(not(unix))]
    let inode = 0;
    Some(Signature {
        inode,
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

/// The file, and its last parse kept until its signature changes.
pub struct SharedJsonFile {
    path: PathBuf,
    cache: Mutex<Option<(Option<Signature>, Arc<Value>)>>,
}

impl SharedJsonFile {
    pub fn new(path: PathBuf) -> Self {
        SharedJsonFile {
            path,
            cache: Mutex::new(None),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's JSON, `Null` when it is missing or unreadable.
    pub fn read(&self) -> Arc<Value> {
        let now = signature(&self.path);
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((seen, value)) = cache.as_ref()
            && *seen == now
        {
            return value.clone();
        }
        let value = Arc::new(
            std::fs::read(&self.path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or(Value::Null),
        );
        *cache = Some((now, value.clone()));
        value
    }

    /// Replaces the file as `JSON.stringify(value, null, 2)` writes it, mode 0600; the next read parses it again.
    pub fn write(&self, value: &Value) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(format!(".{}.tmp", std::process::id()));
        let tmp = PathBuf::from(tmp);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&tmp)?;
        file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
        drop(file);
        let renamed = std::fs::rename(&tmp, &self.path);
        *self.cache.lock().unwrap_or_else(|p| p.into_inner()) = None;
        renamed
    }
}

/// A millisecond count as `JSON.stringify` writes it: an integer when it is one.
pub fn js_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn writes_what_json_stringify_writes_and_sees_another_writer() {
        let dir = std::env::temp_dir().join(format!("k7-auth-json-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = SharedJsonFile::new(dir.join("a.json"));
        assert_eq!(*f.read(), Value::Null);
        f.write(&json!([{ "id": "x", "n": js_number(1.7e12) }, {}]))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(f.path()).unwrap(),
            "[\n  {\n    \"id\": \"x\",\n    \"n\": 1700000000000\n  },\n  {}\n]"
        );
        assert_eq!(f.read()[0]["id"], "x");
        std::fs::write(f.path(), "{\"other\": true, \"more\": 1}").unwrap();
        assert_eq!(f.read()["other"], true);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
