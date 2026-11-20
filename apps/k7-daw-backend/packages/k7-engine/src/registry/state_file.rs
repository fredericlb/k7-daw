//! state_file.rs — the registry's JSON files under its host's state dir, written:
//! whole, through `<file>.tmp` renamed over the old one, 0600 in a 0700 folder, and an unreadable file SET
//! ASIDE as `<file>.unreadable-<ms>` rather than overwritten, so a bad write never costs what it replaced.

use std::io::Write;
use std::path::Path;

use serde_json::Value;

use crate::js;
use crate::log::Sink;

/// Writes `data` as `JSON.stringify(data) + "\n"`; a failure is said, never returned, the memory copy still holding.
pub fn save_json(file: &Path, data: &Value, what: &str, log: &Sink) {
    if let Err(e) = write(file, &format!("{}\n", js::stringify(data))) {
        log(&format!("could not save the {what}: {e}"));
    }
}

fn write(file: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        let mut b = std::fs::DirBuilder::new();
        b.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
        b.create(dir)?;
    }
    let tmp = file.with_file_name(format!(
        "{}.tmp",
        file.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    ));
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(&tmp)?.write_all(text.as_bytes())?;
    std::fs::rename(&tmp, file)
}

/// The parsed file, None when absent; an unreadable one is renamed aside and said.
pub fn load_json(file: &Path, what: &str, log: &Sink, now_ms: i64) -> Option<Value> {
    if !file.exists() {
        return None;
    }
    let read = std::fs::read(file)
        .map_err(|e| e.to_string())
        .and_then(|b| {
            serde_json::from_str::<Value>(&String::from_utf8_lossy(&b)).map_err(|e| e.to_string())
        });
    match read {
        Ok(v) => Some(v),
        Err(e) => {
            let aside = format!("{}.unreadable-{now_ms}", file.display());
            let _ = std::fs::rename(file, &aside);
            log(&format!("{what} unreadable ({e}), set aside as {aside}"));
            None
        }
    }
}
