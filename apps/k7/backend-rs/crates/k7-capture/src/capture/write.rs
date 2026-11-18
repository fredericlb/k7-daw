//! write.rs — a file written for ANOTHER replica's batch, and the capture's reads of the files it knows (`writeKnown`, `read`).
//! What a reader gets wrong: the ledger takes the file only AFTER it landed (2133), so a write that fails changes nothing.

use std::io::{self, ErrorKind};
use std::path::Path;

use k7_replica::blob::parse_blob_ref;
use k7_sync::fsx::{write_atomic, write_batched};

use super::LiveCapture;
use crate::ledger::{View, mtime_ms, stamp_of};

impl LiveCapture {
    /// Write (or, with `None`, remove) a file for another replica's batch, recorded so it is not echoed back; no link is followed.
    pub fn write_known(&mut self, file: &str, text: Option<&str>) -> io::Result<()> {
        self.write_text(file, text, None)
    }

    /// As `write_known`, but nothing is made durable here: a replaced file keeps its ordering sync, and `take_unsynced` names the rest.
    pub fn write_known_batched(
        &mut self,
        file: &str,
        text: Option<&str>,
        ordered: bool,
    ) -> io::Result<()> {
        self.write_text(file, text, Some(ordered))
    }

    /// A file brought whole from a shared folder (`None` removes it): written, then learned, so it is never reported as an edit.
    pub fn land(&mut self, file: &str, bytes: Option<&[u8]>) -> io::Result<()> {
        let abs = std::path::PathBuf::from(self.write_path_of(file)?);
        match bytes {
            None => match std::fs::remove_file(&abs) {
                Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
                _ => {}
            },
            Some(b) => {
                if let Some(dir) = abs.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let name = abs
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let part = abs.with_file_name(format!(".{name}.k7-part"));
                std::fs::write(&part, b)?;
                if let Err(e) = k7_sync::fsx::rename_retrying(&part, &abs) {
                    let _ = std::fs::remove_file(&part);
                    return Err(e);
                }
            }
        }
        self.last_active = self.now();
        self.learn(file);
        self.publish(file);
        Ok(())
    }

    /// The files `write_known_batched` wrote since the last call, for the durability pass.
    pub fn take_unsynced(&mut self) -> Vec<std::path::PathBuf> {
        std::mem::take(&mut self.unsynced)
    }

    fn write_text(
        &mut self,
        file: &str,
        text: Option<&str>,
        batched: Option<bool>,
    ) -> io::Result<()> {
        if let (Some(t), Some(blob)) = (text, parse_blob_ref(text)) {
            return self.write_blob(file, t, &blob);
        }
        let waited = self.ledger.get(file).is_some_and(|k| k.pending);
        let abs = self.write_path_of(file)?;
        match text {
            None => match std::fs::remove_file(&abs) {
                Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
                _ => {}
            },
            Some(t) => {
                if let Some(dir) = Path::new(&abs).parent() {
                    std::fs::create_dir_all(dir)?;
                }
                match batched {
                    None => write_atomic(Path::new(&abs), t)?,
                    Some(ordered) => {
                        write_batched(Path::new(&abs), t, ordered)?;
                        self.unsynced.push(abs.clone().into());
                    }
                }
            }
        }
        self.last_active = self.now();
        self.learn(file);
        if waited {
            self.save_pending()?;
        }
        self.publish(file);
        Ok(())
    }

    /// The file's text now, its blob ref when it is opaque, or None when it is absent.
    pub fn read(&mut self, file: &str) -> io::Result<Option<String>> {
        let known = self.ledger.get(file).cloned();
        if let Some(k) = known.as_ref().filter(|k| k.pending) {
            return Ok(k.r#ref.clone());
        }
        let abs = self.path_of(file)?;
        let md = match std::fs::metadata(&abs) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if let Some(k) = known.as_ref()
            && k.r#ref.is_some()
            && k.stamp == stamp_of(md.len(), mtime_ms(&md))
        {
            return Ok(k.r#ref.clone());
        }
        Ok(Some(self.probe(file, false, None)?.text))
    }

    /// What the ledger says the file shows; a file it does not know is absent. Reads nothing.
    pub fn view(&self, file: &str) -> View {
        match self.ledger.get(file) {
            None => View::Absent,
            Some(k) => match (&k.r#ref, &k.text) {
                (Some(r), _) => View::Ref(r.clone()),
                (None, Some(t)) => View::Text(t.clone()),
                (None, None) => View::Hash(k.hash.clone()),
            },
        }
    }

    /// The text of a file the ledger knows, read from the disk without learning anything: a join's plan reads before its lock.
    /// What it returns is only as current as the ledger, so a write phase stat-checks every file whose text it uses.
    pub fn peek(&self, file: &str) -> io::Result<Option<String>> {
        match self.view(file) {
            View::Absent => Ok(None),
            View::Ref(r) => Ok(Some(r)),
            View::Text(_) | View::Hash(_) => {
                let abs = self.path_of(file)?;
                self.count_read(file);
                let bytes = std::fs::read(abs)?;
                Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
            }
        }
    }

    /// Every file the ledger knows, by text or by ref: what a seed sends, and what a snapshot may replace.
    pub fn text_files(&self) -> Vec<String> {
        self.ledger.keys().cloned().collect()
    }
}
