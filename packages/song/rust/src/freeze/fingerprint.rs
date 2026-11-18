// freeze/fingerprint.rs — the name of a render: the SHA-256 of the cut's sound, every file it names spelled portably.
// A reader would get wrong that a Source refusal is silent: a refused or absent path stays as written, so a token never reveals one.
// Decisions 343 to 345 and 348; docs/autoFreeze.md.

use super::cut::track_weight;
use crate::cache::Cache;
use crate::clips::Budget;
use crate::doc::ProjectDoc;
use crate::roots::spell;
use crate::source::path::{PathRule, is_within};
use crate::source::{Kind, Source, SourceError};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// At most this many bytes are hashed per compile; past it a file enters the fingerprint by its length (decision 348).
pub const MAX_DIGEST_BYTES: u64 = 256 * 1024 * 1024;

/// Files this size or under enter by content: a VST's `.vstpreset` is its params, rewritten in place.
const CONTENT_HASH_MAX: u64 = 1024 * 1024;

/// What a stat costs in the budget, a system call.
const STAT_COST: usize = 4;

/// What a read costs before its bytes (one per KiB): a system call, so tiny real files are not free (decision 348).
const READ_COST: usize = 32;

/// A string longer than this is never taken for a path; the text of a Faust module starts `// …` and looks absolute.
const MAX_ABSOLUTE: usize = 4096;

/// Fields that say nothing about the SOUND: a destination, a display echo, what the engine ignores.
const NOT_SOUND: [&str; 4] = ["presetSave", "event", "color", "recordDir"];

/// How deep the canonical walk goes; a document's own shape is far shallower.
const MAX_DEPTH: usize = 64;

/// Why a fingerprint stopped.
pub(crate) enum Stop {
    Source(SourceError),
    Spent,
}

impl From<SourceError> for Stop {
    fn from(e: SourceError) -> Stop {
        Stop::Source(e)
    }
}

/// `/…` or a drive letter and a separator, as a text test and on every OS.
fn looks_absolute(s: &str) -> bool {
    let b = s.as_bytes();
    s.starts_with('/')
        || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\'))
}

/// What one compile remembers while it fingerprints: each path's token, and the bytes already hashed.
pub(crate) struct Fingerprinter {
    memo: HashMap<String, Option<String>>,
    hashed: u64,
    cap: u64,
    rule: PathRule,
}

impl Default for Fingerprinter {
    fn default() -> Self {
        Fingerprinter::new(PathRule::HOST)
    }
}

impl Fingerprinter {
    pub(crate) fn new(rule: PathRule) -> Fingerprinter {
        Fingerprinter {
            memo: HashMap::new(),
            hashed: 0,
            cap: MAX_DIGEST_BYTES,
            rule,
        }
    }

    /// The portable name of a file: its path in the project, else `~/<base>`.
    fn name_of(project: &str, at: &str) -> String {
        if let Some(rel) = at.strip_prefix(project).map(|r| r.trim_start_matches('/'))
            && is_within(project, at)
            && !rel.is_empty()
        {
            return rel.to_owned();
        }
        format!("~/{}", at.rsplit('/').next().unwrap_or(at))
    }

    /// The token that replaces `s`, or None to leave it as written.
    fn token(
        &mut self,
        source: &dyn Source,
        cache: &mut Cache,
        project: &str,
        budget: &mut Budget,
        s: &str,
    ) -> Result<Option<String>, Stop> {
        if let Some(t) = self.memo.get(s) {
            return Ok(t.clone());
        }
        let token = self.resolve(source, cache, project, budget, s)?;
        self.memo.insert(s.to_owned(), token.clone());
        Ok(token)
    }

    fn resolve(
        &mut self,
        source: &dyn Source,
        cache: &mut Cache,
        project: &str,
        budget: &mut Budget,
        s: &str,
    ) -> Result<Option<String>, Stop> {
        let Ok(at) = spell(s, self.rule) else { return Ok(None) };
        if !budget.charge(STAT_COST) {
            return Err(Stop::Spent);
        }
        let st = match source.stat(&at) {
            Ok(st) => st,
            Err(SourceError::NotFound | SourceError::Outside) => return Ok(None),
            Err(SourceError::Missing) => return Err(Stop::Source(SourceError::Missing)),
            Err(SourceError::Io(_)) => return Ok(Some(format!("{}#missing", Self::name_of(project, &at)))),
        };
        let name = Self::name_of(project, &at);
        if st.kind == Kind::Dir {
            return Ok(Some(format!("{name}#dir")));
        }
        if st.len > CONTENT_HASH_MAX || self.hashed.saturating_add(st.len) > self.cap {
            return Ok(Some(format!("{name}#{}", st.len)));
        }
        if !budget.charge(READ_COST + usize::try_from(st.len / 1024).unwrap_or(usize::MAX)) {
            return Err(Stop::Spent);
        }
        self.hashed += st.len;
        match cache.digest(source, &at, &st) {
            Ok(hex) => Ok(Some(format!("{name}#{hex}"))),
            Err(SourceError::Missing) => Ok(Some(format!("{name}#{}", st.len))),
            Err(_) => Ok(Some(format!("{name}#missing"))),
        }
    }

    /// The canonical JSON of `v`: keys sorted bytewise, `NOT_SOUND` dropped, absolute strings replaced by their token.
    fn write(&mut self, ctx: &mut Ctx<'_, '_>, out: &mut Sha256, v: &Value, depth: usize) -> Result<(), Stop> {
        match v {
            Value::String(s) => {
                let tok = if s.len() <= MAX_ABSOLUTE && looks_absolute(s) {
                    self.token(ctx.source, ctx.cache, ctx.project, ctx.budget, s)?
                } else {
                    None
                };
                put_json(out, tok.as_deref().unwrap_or(s));
            }
            Value::Array(items) if depth < MAX_DEPTH => {
                out.update(b"[");
                for (i, it) in items.iter().enumerate() {
                    if i > 0 {
                        out.update(b",");
                    }
                    self.write(ctx, out, it, depth + 1)?;
                }
                out.update(b"]");
            }
            Value::Object(map) if depth < MAX_DEPTH => {
                let mut keys: Vec<&String> = map.keys().filter(|k| !NOT_SOUND.contains(&k.as_str())).collect();
                keys.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
                out.update(b"{");
                for (i, k) in keys.into_iter().enumerate() {
                    if i > 0 {
                        out.update(b",");
                    }
                    put_json(out, k);
                    out.update(b":");
                    if let Some(item) = map.get(k) {
                        self.write(ctx, out, item, depth + 1)?;
                    }
                }
                out.update(b"}");
            }
            Value::Array(_) | Value::Object(_) => out.update(b"null"),
            other => out.update(other.to_string().as_bytes()),
        }
        Ok(())
    }

    /// The fingerprint of `cut`; None when the budget is spent.
    pub(crate) fn fingerprint(
        &mut self,
        source: &dyn Source,
        cache: &mut Cache,
        cut: &ProjectDoc,
        project: &str,
        budget: &mut Budget,
    ) -> Result<Option<String>, SourceError> {
        let project = spell(project, self.rule).unwrap_or_else(|_| project.to_owned());
        let mut ctx = Ctx {
            source,
            cache,
            project: &project,
            budget,
        };
        match self.run(&mut ctx, cut) {
            Ok(hex) => Ok(Some(hex)),
            Err(Stop::Spent) => Ok(None),
            Err(Stop::Source(e)) => Err(e),
        }
    }

    fn run(&mut self, ctx: &mut Ctx<'_, '_>, cut: &ProjectDoc) -> Result<String, Stop> {
        let mut out = Sha256::new();
        out.update(b"{\"sampleRate\":");
        out.update(cut.sample_rate.to_string().as_bytes());
        out.update(b",\"tempo\":");
        put_json(&mut out, &cut.tempo);
        out.update(b",\"timeSig\":[");
        out.update(format!("{},{}", cut.time_sig_numerator, cut.time_sig_denominator).as_bytes());
        out.update(b"],\"tracks\":[");
        for (i, t) in cut.tracks.iter().enumerate() {
            if !ctx.budget.charge(track_weight(t)) {
                return Err(Stop::Spent);
            }
            if i > 0 {
                out.update(b",");
            }
            let v = serde_json::to_value(t).unwrap_or(Value::Null);
            self.write(ctx, &mut out, &v, 0)?;
        }
        out.update(b"]}");
        Ok(out.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect())
    }
}

/// What a canonical walk reads through.
struct Ctx<'a, 'b> {
    source: &'a dyn Source,
    cache: &'a mut Cache,
    project: &'a str,
    budget: &'b mut Budget,
}

fn put_json(out: &mut Sha256, v: &(impl serde::Serialize + ?Sized)) {
    if let Ok(bytes) = serde_json::to_vec(v) {
        out.update(&bytes);
    }
}

/// The fingerprint of a render's inputs: the first 16 hex of the SHA-256 of the cut's canonical sound (decision 343).
pub fn freeze_fingerprint(
    source: &dyn Source,
    cache: &mut Cache,
    cut: &ProjectDoc,
    project: &str,
    budget: &mut Budget,
    rule: PathRule,
) -> Result<Option<String>, SourceError> {
    Fingerprinter::new(rule).fingerprint(source, cache, cut, project, budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{BundleFile, Entry, MemSource, SourceBundle};
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Reads<'a>(&'a MemSource, AtomicUsize);

    impl Source for Reads<'_> {
        fn stat(&self, path: &str) -> Result<crate::source::Stat, SourceError> {
            self.0.stat(path)
        }
        fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
            self.0.list(dir)
        }
        fn read(&self, path: &str, span: crate::source::Span) -> Result<Vec<u8>, SourceError> {
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0.read(path, span)
        }
    }

    #[test]
    fn a_file_past_the_cap_enters_by_its_length_and_is_not_read() {
        // decision 348: the cap on hashed bytes, lowered here because 256 MiB of SHA-256 is seconds in a debug build
        let mut files = BTreeMap::new();
        let mut entries = Vec::new();
        for i in 0..10 {
            files.insert(
                format!("/p/f{i}.txt"),
                BundleFile {
                    text: Some("a".repeat(1000)),
                    len: None,
                    ..Default::default()
                },
            );
            entries.push(Entry {
                name: format!("f{i}.txt"),
                kind: crate::source::Kind::File,
            });
        }
        let lists = BTreeMap::from([("/p".to_owned(), entries)]);
        let mem = MemSource::from_bundle(SourceBundle {
            format: 1,
            mounts: vec!["/p".into()],
            files,
            lists,
            ..Default::default()
        })
        .expect("a bundle");
        let src = Reads(&mem, AtomicUsize::new(0));
        let mut fp = Fingerprinter {
            cap: 4_000,
            ..Fingerprinter::default()
        };
        let mut cache = Cache::new();
        let mut budget = Budget::new(1_000_000);
        for i in 0..10 {
            let t = fp.token(&src, &mut cache, "/p", &mut budget, &format!("/p/f{i}.txt"));
            assert!(matches!(t, Ok(Some(_))));
        }
        assert_eq!(src.1.load(Ordering::SeqCst), 4);
        let late = fp.token(&src, &mut cache, "/p", &mut budget, "/p/f9.txt");
        assert!(matches!(late, Ok(Some(t)) if t == "f9.txt#1000"));
    }
}
