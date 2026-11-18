// views/lengths.rs — how long a clip's audio is, asked of a Measure: the server reads headers, a replay reads a table.
// A reader would get wrong that a bundle cannot answer a header (it holds a binary by its length alone): ByTable is why.

use super::Spend;
use super::wav::{HEAD_BYTES, wav_seconds};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::source::{Kind, Source, SourceError, Span};
use std::collections::{BTreeMap, HashMap};

/// An unmeasurable file is absent: a bundle carries no null.
pub type Lengths = BTreeMap<String, f64>;

/// Header reads one `ByHeader` makes that the cache did not answer.
pub const MAX_HEADERS: usize = 20_000;

/// The length in seconds of the audio at a path, or None when it cannot be told.
pub trait Measure {
    fn seconds(&mut self, path: &str) -> Result<Option<f64>, SourceError>;
}

/// Measures through the Source: a stat, then the first `HEAD_BYTES`, held in the cache by settled stamp.
pub struct ByHeader<'a> {
    source: &'a dyn Source,
    cache: &'a mut Cache,
    memo: HashMap<String, Option<f64>>,
    lengths: Lengths,
    reads: usize,
    capped: bool,
    /// Every distinct path asked is a stat and a copy: the walk is weighed too (decision 614).
    spend: Spend,
}

impl<'a> ByHeader<'a> {
    pub fn new(source: &'a dyn Source, cache: &'a mut Cache) -> ByHeader<'a> {
        ByHeader {
            source,
            cache,
            memo: HashMap::new(),
            lengths: Lengths::new(),
            reads: 0,
            capped: false,
            spend: Spend::new(),
        }
    }

    pub fn measure_all<'p>(&mut self, paths: impl Iterator<Item = &'p str>) -> Result<(), SourceError> {
        for p in paths {
            self.seconds(p)?;
        }
        Ok(())
    }

    /// The table of what was measured, and the one WARNING of a walk that hit its limit.
    pub fn finish(self) -> (Lengths, Vec<Finding>) {
        let mut findings = Vec::new();
        if self.capped {
            findings.push(Finding {
                file: "clips".to_owned(),
                line: 1,
                col: 1,
                message: format!(
                    "too many audio files to measure (past {MAX_HEADERS} headers or the walk's budget) — the rest keep their drawn length"
                ),
                severity: Severity::Warning,
            });
        }
        (self.lengths, findings)
    }

    fn measure(&mut self, path: &str) -> Result<Option<f64>, SourceError> {
        let st = match self.source.stat(path) {
            Ok(s) if s.kind == Kind::File => s,
            Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => return Ok(None),
            Err(e) => return Err(e),
        };
        if let Some(held) = self.cache.length_of(path, st.stamp) {
            return Ok(held);
        }
        // A memory source that measured the file when its bundle was made holds no bytes to read a header from.
        if let Some(secs) = self.source.known_seconds(path) {
            return Ok(Some(secs));
        }
        // A replay's table holds what the server could measure: a file it lacks is one that is no wav, not one to ask the header of.
        if self.source.measured_by_table() {
            return Ok(None);
        }
        if self.reads >= MAX_HEADERS {
            self.capped = true;
            return Ok(None);
        }
        let head = match self.source.read(
            path,
            Span::Range {
                offset: 0,
                len: HEAD_BYTES,
            },
        ) {
            Ok(b) => b,
            Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => return Ok(None),
            Err(e) => return Err(e),
        };
        self.reads += 1;
        let secs = wav_seconds(&head, st.len);
        self.cache.hold_length(path, st.stamp, secs);
        Ok(secs)
    }
}

impl Measure for ByHeader<'_> {
    fn seconds(&mut self, path: &str) -> Result<Option<f64>, SourceError> {
        if let Some(held) = self.memo.get(path) {
            return Ok(*held);
        }
        // Past the walk's budget a new path costs nothing: no stat, no copy.
        if !self.spend.bytes(path.len()) {
            self.capped = true;
            return Ok(None);
        }
        let secs = self.measure(path)?;
        if let Some(s) = secs {
            self.lengths.insert(path.to_owned(), s);
        }
        self.memo.insert(path.to_owned(), secs);
        Ok(secs)
    }
}

/// Measures from a table a server made; a path it does not hold answers None, never Missing.
pub struct ByTable<'t>(pub &'t Lengths);

impl Measure for ByTable<'_> {
    fn seconds(&mut self, path: &str) -> Result<Option<f64>, SourceError> {
        Ok(self.0.get(path).copied())
    }
}
