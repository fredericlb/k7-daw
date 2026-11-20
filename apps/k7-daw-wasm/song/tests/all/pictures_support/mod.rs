// pictures_support/mod.rs — what the pictures tests share: shots from values, a sounder over a memory folder, a seeded generator.
// A reader would get wrong that nothing here reads an audio file: a `.wav` is an empty text the Source can only stat.

#![allow(dead_code)]

mod cases;
mod gate;
#[allow(unused_imports)]
pub use cases::*;
#[allow(unused_imports)]
pub use gate::*;

use serde_json::Value;
use song::pictures::{ClipSounds, ShotIn, SoundPiece, Sounder, Spend};
use song::{Clip, Entry, FamilyRead, Placement, Source, SourceError, Span, Stat};
use std::sync::Mutex;

pub fn placements(events: &[Value]) -> Vec<Placement> {
    events
        .iter()
        .map(|v| serde_json::from_value(v.clone()).expect("a placement"))
        .collect()
}

/// A lane's own shots, as `ShotIn::own` reads them.
pub fn own(events: &[Placement]) -> Vec<ShotIn<'_>> {
    events.iter().map(ShotIn::own).collect()
}

/// A Source that counts the stats it is asked for, by path.
pub struct Counting<'a> {
    pub inner: &'a dyn Source,
    pub stats: Mutex<Vec<String>>,
    pub lists: Mutex<Vec<String>>,
}

impl<'a> Counting<'a> {
    pub fn new(inner: &'a dyn Source) -> Counting<'a> {
        Counting {
            inner,
            stats: Mutex::new(Vec::new()),
            lists: Mutex::new(Vec::new()),
        }
    }

    pub fn lists(&self) -> Vec<String> {
        self.lists.lock().expect("lock").clone()
    }

    pub fn stats(&self) -> Vec<String> {
        self.stats.lock().expect("lock").clone()
    }
}

impl Source for Counting<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.stats.lock().expect("lock").push(path.to_owned());
        self.inner.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.lists.lock().expect("lock").push(dir.to_owned());
        self.inner.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.inner.read(path, span)
    }
}

/// A Source whose every stat fails with `error`.
pub struct Failing(pub SourceError);

impl Source for Failing {
    fn stat(&self, _: &str) -> Result<Stat, SourceError> {
        Err(self.0.clone())
    }

    fn list(&self, _: &str) -> Result<Vec<Entry>, SourceError> {
        Err(self.0.clone())
    }

    fn read(&self, _: &str, _: Span) -> Result<Vec<u8>, SourceError> {
        Err(self.0.clone())
    }
}

/// The pieces of `shots` over `source`, `/p` being the project, at `bps`.
pub fn pieces_with(
    source: &dyn Source,
    clips: &FamilyRead<Clip>,
    bps: f64,
    shots: &[ShotIn<'_>],
    spend: &mut Spend,
) -> Result<Vec<SoundPiece>, SourceError> {
    pieces_in(source, "/p", clips, bps, shots, spend)
}

/// The same with the project named.
pub fn pieces_in(
    source: &dyn Source,
    project: &str,
    clips: &FamilyRead<Clip>,
    bps: f64,
    shots: &[ShotIn<'_>],
    spend: &mut Spend,
) -> Result<Vec<SoundPiece>, SourceError> {
    pieces_ruled(source, project, clips, bps, shots, spend, song::PathRule::HOST)
}

/// The same under a path rule.
pub fn pieces_ruled(
    source: &dyn Source,
    project: &str,
    clips: &FamilyRead<Clip>,
    bps: f64,
    shots: &[ShotIn<'_>],
    spend: &mut Spend,
    rule: song::PathRule,
) -> Result<Vec<SoundPiece>, SourceError> {
    let sounds = ClipSounds::of(clips);
    let mut sounder = Sounder::new(source, project, bps, &sounds, spend).with_rule(rule);
    sounder.pieces(shots, "nodes/main.k7x")
}

/// A seeded generator: xorshift64*, so a failure names its seed and replays.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[usize::try_from(self.below(of.len() as u64)).expect("index")]
    }
}

/// Picture lanes' own shots by name, as `pictures` reads them.
pub fn lane_shots<'t>(lanes: &'t [(String, Vec<Placement>)]) -> song::pictures::LaneShots<'t> {
    lanes.iter().map(|(name, evs)| (name.as_str(), own(evs))).collect()
}

/// The picture lanes of `(name, events)` pairs, events given as values.
pub fn lanes_of(items: &[(&str, Vec<Value>)]) -> Vec<(String, Vec<Placement>)> {
    items
        .iter()
        .map(|(n, evs)| ((*n).to_owned(), placements(evs)))
        .collect()
}
