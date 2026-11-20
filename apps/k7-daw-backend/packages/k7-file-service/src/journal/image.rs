//! image.rs — a project at one instant from one walk: every text file's content and every binary's stamp (1732). A text file
//! is kept against its `size:mtime`, and never while its mtime is younger than `SETTLE_MS`. What a reader gets wrong: an
//! image is read in the walk's order, which the diff keeps, so a step lists its files as the TypeScript lists them.

use std::collections::HashMap;
use std::io;

use crate::revision::Walked;

/// The window inside which a same-size rewrite may reproduce a stamp (`SETTLE_MS`).
pub const SETTLE_MS: f64 = 2000.0;

/// A file's `size:mtimeMs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileStamp {
    pub size: u64,
    pub mtime: f64,
}

/// A map that lists its keys in the order they were first set, as a JavaScript `Map` does (a deleted key set again goes last).
#[derive(Clone, Debug)]
pub struct OMap<V> {
    slots: Vec<Option<(String, V)>>,
    at: HashMap<String, usize>,
}

impl<V> Default for OMap<V> {
    fn default() -> Self {
        OMap {
            slots: Vec::new(),
            at: HashMap::new(),
        }
    }
}

impl<V> OMap<V> {
    pub fn get(&self, k: &str) -> Option<&V> {
        self.at
            .get(k)
            .and_then(|i| self.slots[*i].as_ref())
            .map(|(_, v)| v)
    }

    pub fn contains(&self, k: &str) -> bool {
        self.at.contains_key(k)
    }

    pub fn len(&self) -> usize {
        self.at.len()
    }

    pub fn is_empty(&self) -> bool {
        self.at.is_empty()
    }

    pub fn set(&mut self, k: String, v: V) {
        match self.at.get(&k) {
            Some(i) => self.slots[*i] = Some((k, v)),
            None => {
                self.at.insert(k.clone(), self.slots.len());
                self.slots.push(Some((k, v)));
            }
        }
    }

    pub fn remove(&mut self, k: &str) -> Option<V> {
        let i = self.at.remove(k)?;
        self.slots[i].take().map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.slots.iter().flatten().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.iter().map(|(k, _)| k)
    }
}

/// The project at one instant.
#[derive(Clone, Debug, Default)]
pub struct Image {
    pub text: OMap<String>,
    pub bin: OMap<FileStamp>,
}

/// What the last image read, so the next one re-reads only what moved.
#[derive(Debug, Default)]
pub struct TextCache(HashMap<String, (FileStamp, String)>);

impl TextCache {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// A text file past the limit: the image would be a lie, so the record journals nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooLarge(pub String);

/// The two images of a walk; `read` is the file read (a test counts its calls), `now_ms` the clock.
pub fn image_of(
    files: &[Walked],
    to_rel: &dyn Fn(&Walked) -> String,
    cache: &mut TextCache,
    now_ms: f64,
    max_text: u64,
    read: &mut dyn FnMut(&str) -> io::Result<Vec<u8>>,
) -> Result<Image, TooLarge> {
    let mut image = Image::default();
    let mut seen = HashMap::new();
    for f in files {
        let rel = to_rel(f);
        let stamp = FileStamp {
            size: f.size,
            mtime: f.mtime_ms,
        };
        if f.binary {
            image.bin.set(rel, stamp);
            continue;
        }
        let settled = now_ms - f.mtime_ms > SETTLE_MS;
        let text = match cache.0.remove(&rel) {
            Some((s, t)) if settled && s == stamp => t,
            _ => {
                if f.size > max_text {
                    return Err(TooLarge(rel));
                }
                // Unreadable (a folder behind a link, a file raced away) is absent, as it always was.
                let Ok(bytes) = read(&f.abs) else { continue };
                match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
                }
            }
        };
        image.text.set(rel.clone(), text.clone());
        // Nothing young is kept either: the next call would find it under the same stamp.
        if settled {
            seen.insert(rel, (stamp, text));
        }
    }
    cache.0 = seen;
    Ok(image)
}
