//! files.rs — making a slot's document stand on its own: every file it names is copied into the slot's folder
//! and its path rewritten there, so the slot plays on a machine that has none of the paths it was compiled
//! with. What a reader gets wrong: an SFZ's samples sit BESIDE it, unnamed, so its whole folder is copied.
//! The fields walked are the document's file fields (`apps/k7-daw-engine/k7-runtime/engine/ProjectDoc.h`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::paths::utf8;

/// The formats named by the plugin's own scan entry rather than a file: the engine matches them by name.
const THIRD_PARTY: &[&str] = &[
    "vst3",
    "vst",
    "vst2",
    "au",
    "audiounit",
    "clap",
    "lv2",
    "ladspa",
    "aax",
];

/// The plugin fields that each name one file the engine reads.
const FILE_FIELDS: &[&str] = &["path", "namFile", "irFile", "surgePatch", "deckFile"];

const INSTRUMENTS: &str = "@instruments/";

/// Whether the machine has a plugin, by format word and name.
pub type HasPlugin = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;
/// Whether the machine has an instrument, by name.
pub type HasInstrument = Arc<dyn Fn(&str) -> bool + Send + Sync>;
/// Whether a file may be copied into a slot.
pub type Readable = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

/// What this machine has, asked as the document is walked; an unset question answers no, except `readable`.
#[derive(Clone, Default)]
pub struct Installed {
    /// A third-party plugin by its lowercase format word and name, as the engine's scan lists it.
    pub plugin: Option<HasPlugin>,
    /// An instrument of `<state>/instruments`, by name.
    pub instrument: Option<HasInstrument>,
    /// Whether a file the document names may be copied in; unset, any may: a server's job may name only its own copies.
    pub readable: Option<Readable>,
}

struct Bundler<'a> {
    stage: PathBuf,
    fin: PathBuf,
    installed: &'a Installed,
    done: BTreeMap<String, String>,
    missing: Vec<String>,
    next: u32,
}

fn str_of<'a>(o: &'a Value, key: &str) -> &'a str {
    o.get(key).and_then(Value::as_str).unwrap_or("")
}

impl Bundler<'_> {
    fn doc(&mut self, d: &mut Value) {
        for p in d["masterPlugins"].as_array_mut().into_iter().flatten() {
            self.plugin(p);
        }
        for t in d["tracks"].as_array_mut().into_iter().flatten() {
            self.track(t);
        }
        // Takes go into the slot's own folder: the compiling machine's record folder is not on this one.
        d["recordDir"] = utf8(&self.fin.parent().unwrap_or(&self.fin).join("takes")).into();
    }

    fn track(&mut self, t: &mut Value) {
        for p in t["plugins"].as_array_mut().into_iter().flatten() {
            self.plugin(p);
        }
        for c in t["clips"].as_array_mut().into_iter().flatten() {
            self.file(c, "source");
        }
    }

    fn plugin(&mut self, p: &mut Value) {
        let format = str_of(p, "type").to_lowercase();
        let name = str_of(p, "name").to_string();
        let known = self
            .installed
            .plugin
            .as_ref()
            .is_some_and(|f| f(&format, &name));
        if THIRD_PARTY.contains(&format.as_str()) && !known {
            self.missing.push(format!(
                "the plugin '{name}' ({format}) is not in this machine's plugin scan"
            ));
        }
        // A slot never writes back: what it was saved with is what it plays.
        if p.get("presetSave").is_some() {
            p["presetSave"] = "".into();
        }
        for f in FILE_FIELDS {
            self.file(p, f);
        }
        self.sfz(p);
        for r in p["rack"].as_array_mut().into_iter().flatten() {
            self.file(r, "clip");
        }
        if p["patternBanks"].as_array().is_some_and(|b| !b.is_empty()) {
            self.missing
                .push("a pattern lane's sample banks are folders a slot does not carry".into());
        }
    }

    /// An installed instrument is the machine's own; a plain .sfz travels with the folder its samples are in.
    fn sfz(&mut self, p: &mut Value) {
        let v = str_of(p, "sfzFile").to_string();
        if v.is_empty() {
            return;
        }
        if let Some(rest) = v.strip_prefix(INSTRUMENTS) {
            let name = rest.split('/').next().unwrap_or("");
            if !self.installed.instrument.as_ref().is_some_and(|f| f(name)) {
                self.missing.push(format!(
                    "the instrument '{name}' is not installed on this machine"
                ));
            }
            return;
        }
        let src = Path::new(&v);
        let Some(dir) = self.folder_copy(src.parent().unwrap_or(Path::new(""))) else {
            return;
        };
        p["sfzFile"] = utf8(&dir.join(src.file_name().unwrap_or_default())).into();
    }

    fn file(&mut self, o: &mut Value, field: &str) {
        let v = str_of(o, field).to_string();
        if !v.is_empty() {
            o[field] = self.copied(&v).into();
        }
    }

    fn readable(&self, p: &Path) -> bool {
        self.installed.readable.as_ref().is_none_or(|f| f(p))
    }

    /// `v` copied once into the slot, by name under a number of its own; the path it will have there.
    fn copied(&mut self, v: &str) -> String {
        if let Some(done) = self.done.get(v) {
            return done.clone();
        }
        let src = Path::new(v);
        if !self.readable(src) {
            self.missing.push(format!(
                "it names a file outside this machine's copies of the server's folders: {v}"
            ));
            return v.into();
        }
        if !src.is_file() {
            self.missing
                .push(format!("it names a file this machine does not have: {v}"));
            return v.into();
        }
        let name = format!(
            "{}-{}",
            self.next,
            src.file_name().unwrap_or_default().to_string_lossy()
        );
        self.next += 1;
        let _ = std::fs::create_dir_all(&self.stage);
        if let Err(e) = std::fs::copy(src, self.stage.join(&name)) {
            self.missing.push(format!("it could not copy {v}: {e}"));
            return v.into();
        }
        let out = utf8(&self.fin.join(&name));
        self.done.insert(v.into(), out.clone());
        out
    }

    /// A whole folder copied once, for a file whose neighbours it reads unnamed; the folder it will have there.
    fn folder_copy(&mut self, dir: &Path) -> Option<PathBuf> {
        let key = format!("{}/", utf8(dir));
        if let Some(done) = self.done.get(&key) {
            return Some(PathBuf::from(done));
        }
        if !self.readable(dir) {
            self.missing.push(format!(
                "it names a folder outside this machine's copies of the server's folders: {}",
                utf8(dir)
            ));
            return None;
        }
        if !dir.is_dir() {
            self.missing.push(format!(
                "it names a folder this machine does not have: {}",
                utf8(dir)
            ));
            return None;
        }
        let name = format!(
            "{}-{}",
            self.next,
            dir.file_name().unwrap_or_default().to_string_lossy()
        );
        self.next += 1;
        if let Err(e) = copy_tree(dir, &self.stage.join(&name)) {
            self.missing
                .push(format!("it could not copy {}: {e}", utf8(dir)));
            return None;
        }
        let out = self.fin.join(&name);
        self.done.insert(key, utf8(&out));
        Some(out)
    }
}

/// `from` copied under `to`, every file and folder below it.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_tree(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Copy what `doc` names into `stage/files` and rewrite each path as it will be under `fin/files`, the folder
/// the slot is renamed into once whole; Err says what is missing when the machine cannot play it.
pub fn bundle(
    doc: &mut Value,
    stage: &Path,
    fin: &Path,
    installed: &Installed,
) -> Result<(), String> {
    let mut b = Bundler {
        stage: stage.join("files"),
        fin: fin.join("files"),
        installed,
        done: BTreeMap::new(),
        missing: Vec::new(),
        next: 1,
    };
    b.doc(doc);
    if b.missing.is_empty() {
        Ok(())
    } else {
        Err(b.missing.join("; "))
    }
}
