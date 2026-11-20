// pictures/sound.rs — a picture lane's shots made into the audio clips the engine plays, heard where each shot is seen.
// A reader would get wrong the source: it is the clip file's `sound:`, never its `source:`, and it is stat'ed, never read.

use super::cover::Cover;
use super::shot::{Shot, ShotIn, shots_of};
use super::{SoundFault, SoundFaultKind, Spend, edge_weight, quoted, weigh};
use crate::clips::{ClipDoc, ClipKind, WarpDoc, placement_id};
use crate::files::{Clip, Placement};
use crate::read::FamilyRead;
use crate::roots::{is_absolute, spell};
use crate::source::path::{PathRule, join};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// A picture lane's shots by lane name, in the order the lanes were met.
pub type LaneShots<'t> = IndexMap<&'t str, Vec<ShotIn<'t>>>;

struct Entry<'c> {
    file: &'c str,
    word: Option<(usize, &'c str)>,
}

/// Each clip's `sound:` word and file name, built once so no loop runs a `FamilyRead::get` (511).
pub struct ClipSounds<'c> {
    by_id: HashMap<&'c str, Entry<'c>>,
    words: usize,
}

impl<'c> ClipSounds<'c> {
    pub fn of(clips: &'c FamilyRead<Clip>) -> ClipSounds<'c> {
        let mut by_id = HashMap::new();
        let mut interned: HashMap<&'c str, usize> = HashMap::new();
        for ((id, clip), name) in clips.values.iter().zip(&clips.names) {
            let word = match &**clip {
                Clip::Video(v) => v.sound.as_ref().map(|s| s.0.as_str()).filter(|w| !w.is_empty()),
                _ => None,
            };
            let word = word.map(|w| {
                let next = interned.len();
                (*interned.entry(w).or_insert(next), w)
            });
            by_id.entry(id.as_str()).or_insert(Entry {
                file: name.as_str(),
                word,
            });
        }
        ClipSounds {
            by_id,
            words: interned.len(),
        }
    }

    /// The `sound:` word of a video clip that states one.
    pub fn sound(&self, id: &str) -> Option<&'c str> {
        self.by_id.get(id)?.word.map(|(_, w)| w)
    }

    /// The file a clip is read from, for a finding.
    pub fn file(&self, id: &str) -> Option<String> {
        self.by_id.get(id).map(|e| format!("clips/{}", e.file))
    }

    fn word(&self, id: &str) -> Option<(usize, &'c str)> {
        self.by_id.get(id)?.word
    }
}

/// One engine clip of a lane's sound, and the index in the list given of the shot it is the sound of.
pub struct SoundPiece {
    pub shot: usize,
    pub clip: ClipDoc,
}

/// What a `sound:` word came to when its path was stat'ed.
enum Verdict {
    Here(String),
    Gone(String),
    Refused,
    Unreadable,
}

/// Makes the sound of shots: the sounds, the source they are stat'ed through, and the spend of one call.
pub struct Sounder<'a> {
    source: &'a dyn Source,
    project: String,
    bps: f64,
    sounds: &'a ClipSounds<'a>,
    spend: &'a mut Spend,
    verdicts: Vec<Option<Verdict>>,
    rule: PathRule,
}

impl<'a> Sounder<'a> {
    pub fn new(
        source: &'a dyn Source,
        project: &str,
        bps: f64,
        sounds: &'a ClipSounds<'a>,
        spend: &'a mut Spend,
    ) -> Sounder<'a> {
        Sounder {
            source,
            project: spell(project, PathRule::HOST).unwrap_or_else(|_| project.to_owned()),
            bps,
            sounds,
            spend,
            verdicts: (0..sounds.words).map(|_| None).collect(),
            rule: PathRule::HOST,
        }
    }

    /// Reads paths by the rule of the roots they came from, which is the host's unless said.
    pub fn with_rule(mut self, rule: PathRule) -> Sounder<'a> {
        self.project = spell(&self.project, rule).unwrap_or(self.project);
        self.rule = rule;
        self
    }

    /// The spend the sounder shares with the walk around it.
    pub fn spend(&mut self) -> &mut Spend {
        self.spend
    }

    /// The pieces of `shots`' sound in file order, a shot's ascending; `Err` is `Missing` alone (509, 511).
    pub fn pieces(&mut self, shots: &[ShotIn<'_>], lane_file: &str) -> Result<Vec<SoundPiece>, SourceError> {
        let weight = shots.iter().fold(0usize, |w, s| {
            w.saturating_add(weigh(ref_of(s.event)) + edge_weight(s.event.from.as_ref()))
        });
        if !self.spend.charge(weight) {
            self.spend.limit(lane_file);
            return Ok(Vec::new());
        }
        let list = shots_of(shots);
        let mut cover = Cover::new();
        let mut groups: Vec<Vec<SoundPiece>> = Vec::new();
        for s in list.iter().rev().filter(|s| !s.ref_.is_empty()) {
            let Some(made) = self.shot_pieces(s, &cover)? else {
                self.spend.limit(lane_file);
                return Ok(Vec::new());
            };
            if !made.is_empty() {
                groups.push(made);
            }
            if !cover.hold(s.start, s.end, s.fade_in, self.spend) {
                self.spend.limit(lane_file);
                return Ok(Vec::new());
            }
        }
        Ok(groups.into_iter().rev().flatten().collect())
    }

    /// One shot's pieces where no later shot covers it; None once the budget is passed.
    fn shot_pieces(&mut self, s: &Shot<'_>, cover: &Cover) -> Result<Option<Vec<SoundPiece>>, SourceError> {
        // A silent, freeze or reverse shot still covers, and a shot with no number for a bound sounds nothing (505).
        if s.speed <= 0.0 || s.silent || s.start.is_nan() || s.end.is_nan() {
            return Ok(Some(Vec::new()));
        }
        let Some((wid, word)) = self.sounds.word(s.ref_) else {
            return Ok(Some(Vec::new()));
        };
        let Some(free) = cover.free(s.start, s.end, self.spend) else {
            return Ok(None);
        };
        if free.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let file = self.sounds.file(s.ref_).unwrap_or_default();
        if self.verdicts.get(wid).is_some_and(Option::is_none) {
            if !self.spend.charge(1) {
                return Ok(None);
            }
            let verdict = self.settle(word, &file)?;
            if let Some(slot) = self.verdicts.get_mut(wid) {
                *slot = Some(verdict);
            }
        }
        let path = match self.verdicts.get(wid) {
            Some(Some(Verdict::Here(p) | Verdict::Gone(p))) => p.as_str(),
            _ => return Ok(Some(Vec::new())),
        };
        let mut out = Vec::new();
        for (a, b) in free {
            if !self.spend.charge(8 + (2 * s.ref_.len() + path.len()) / 64) {
                return Ok(None);
            }
            // Ending where a later shot finishes fading in is that crossfade's outgoing half, and wins over the shot's own fade-out.
            let fade_out = cover.cross_at(b).unwrap_or(if b == s.end { s.fade_out } else { 0.0 });
            let clip = piece(s, (a, b), fade_out, self.bps, path);
            if clip.finite() {
                out.push(SoundPiece { shot: s.index, clip });
            } else {
                let message = format!(
                    "'{}': a number in this shot's sound is not finite, so it plays nothing",
                    quoted(s.ref_)
                );
                self.spend.error(&file, message);
            }
        }
        Ok(Some(out))
    }

    /// Stats a word's path once; a refused or unreadable one costs that sound and gives one warning on the clip's file (512, 514).
    fn settle(&mut self, word: &str, file: &str) -> Result<Verdict, SourceError> {
        let path = if is_absolute(word, self.rule) {
            spell(word, self.rule)
        } else {
            spell(&join(&self.project, word), self.rule)
        };
        let (verdict, why) = match path {
            Err(_) => (Verdict::Refused, "no path reaches it"),
            Ok(p) => match self.source.stat(&p) {
                Ok(_) => (Verdict::Here(p), ""),
                Err(SourceError::NotFound) => (Verdict::Gone(p), ""),
                Err(SourceError::Outside) => (Verdict::Refused, "it lies outside the project's folders"),
                Err(SourceError::Io(_)) => (Verdict::Unreadable, "it cannot be read"),
                Err(SourceError::Missing) => return Err(SourceError::Missing),
            },
        };
        if !why.is_empty() {
            let message = format!("the sound '{}' is not used: {why}", quoted(word));
            self.spend.warning(file, message);
        }
        Ok(verdict)
    }

    /// One fault per (lane, ref) of `pieces` whose sound is not on disk; false once the faults pass their limit (513).
    fn missing(&mut self, pieces: &[SoundPiece], who: &str, holder: &str, lane: &str) -> bool {
        let mut seen: HashSet<&str> = HashSet::new();
        for p in pieces {
            let name = p.clip.name.as_str();
            if !seen.insert(name) {
                continue;
            }
            let Some((wid, word)) = self.sounds.word(name) else {
                continue;
            };
            if !matches!(self.verdicts.get(wid), Some(Some(Verdict::Gone(_)))) {
                continue;
            }
            let file = self.sounds.file(name).unwrap_or_default();
            let message = format!(
                "{who}: shot '{}' sounds {}, which is not there — 'picture sound {}' extracts it again",
                quoted(name),
                quoted(word),
                quoted(lane)
            );
            let fault = SoundFault::new(SoundFaultKind::SoundMissing, holder, name);
            if !self.spend.fault(fault, &file, message) {
                return false;
            }
        }
        true
    }
}

fn ref_of(p: &Placement) -> &str {
    p.ref_.as_ref().map_or("", |r| r.0.as_str())
}

/// The clip `[a, b)` of a shot makes, field for field as the TypeScript writes it (509).
fn piece(s: &Shot<'_>, (a, b): (f64, f64), fade_out: f64, bps: f64, path: &str) -> ClipDoc {
    let open = !b.is_finite();
    let fade_in = if a == s.start { s.fade_in } else { 0.0 };
    let source_from = s.source_from + (a - s.start) * s.speed;
    ClipDoc {
        id: placement_id(s.ref_, a * bps),
        kind: ClipKind::Audio,
        name: s.ref_.to_owned(),
        source: path.to_owned(),
        start: a * bps,
        length: if open { 0.0 } else { (b - a) * bps },
        offset: 0.0,
        source_from: (source_from > 0.0).then_some(source_from),
        source_to: (!open).then_some(source_from + (b - a) * s.speed),
        fade_in: fade_in * bps,
        fade_out: fade_out * bps,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: Vec::new(),
        warp: (s.speed != 1.0).then(|| WarpDoc {
            algorithm: if s.keep_pitch { "elastique" } else { "varispeed" }.to_owned(),
            markers: vec![(0.0, source_from), (bps, source_from + s.speed)],
        }),
        warp_off: None,
        disabled: None,
        event: None,
        comp: None,
        crossfade: None,
        nest: None,
    }
}

/// A lane as the sound reads it: how many events it states, and the picture lane it names.
pub struct SoundLaneIn<'a> {
    pub name: &'a str,
    pub events: usize,
    pub video_source: Option<&'a str>,
    /// The lane's file, which its findings name.
    pub file: String,
}

/// The clips of each lane naming a picture lane in `video_source:`, by lane; a lane that cannot be honoured is a fault and absent (516).
pub fn video_sound_lanes(
    s: &mut Sounder<'_>,
    lanes: &[SoundLaneIn<'_>],
    pictures: &LaneShots<'_>,
) -> Result<IndexMap<String, Vec<ClipDoc>>, SourceError> {
    let mut out = IndexMap::new();
    for lane in lanes {
        let Some(from) = lane.video_source else {
            continue;
        };
        if s.spend.stopped() {
            break;
        }
        if !s.spend.charge(weigh(from)) {
            s.spend.limit(&lane.file);
            break;
        }
        let holder = format!("lane:{}", lane.name);
        let name = quoted(lane.name);
        if lane.events > 0 {
            let m = format!(
                "lane '{name}': video_source '{}' and events: both — its events ARE that picture lane's shots, so drop one",
                quoted(from)
            );
            if !s.spend.fault(
                SoundFault::new(SoundFaultKind::VideoSourceBoth, &holder, from),
                &lane.file,
                m,
            ) {
                break;
            }
            continue;
        }
        let Some(shots) = pictures.get(from) else {
            let m = format!(
                "lane '{name}': video_source names '{}', which is not a picture lane (a `@meta/picture-track` node)",
                quoted(from)
            );
            if !s.spend.fault(
                SoundFault::new(SoundFaultKind::VideoSourceAbsent, &holder, from),
                &lane.file,
                m,
            ) {
                break;
            }
            continue;
        };
        let pieces = s.pieces(shots, &lane.file)?;
        if s.spend.capped() || !s.missing(&pieces, &format!("lane '{name}'"), &holder, from) {
            break;
        }
        out.insert(lane.name.to_owned(), pieces.into_iter().map(|p| p.clip).collect());
    }
    Ok(out)
}

/// Each picture lane's OWN sound, by lane: one whose shots sound or whose `#audio` is wired, and no `video_source:` lane plays (517).
pub fn picture_lane_sounds(
    s: &mut Sounder<'_>,
    lanes: &[SoundLaneIn<'_>],
    pictures: &LaneShots<'_>,
    wired: &dyn Fn(&str) -> bool,
    file_of: &dyn Fn(&str) -> String,
) -> Result<IndexMap<String, Vec<ClipDoc>>, SourceError> {
    let sounded: HashSet<&str> = lanes.iter().filter_map(|l| l.video_source).collect();
    let mut out = IndexMap::new();
    for (&id, shots) in pictures {
        if s.spend.stopped() {
            break;
        }
        let file = file_of(id);
        if !s.spend.charge(weigh(id)) {
            s.spend.limit(&file);
            break;
        }
        if sounded.contains(id) {
            continue;
        }
        let pieces = s.pieces(shots, &file)?;
        if s.spend.capped() {
            break;
        }
        if pieces.is_empty() && !wired(id) {
            continue;
        }
        if !s.missing(
            &pieces,
            &format!("picture lane '{}'", quoted(id)),
            &format!("picture:{id}"),
            id,
        ) {
            break;
        }
        out.insert(id.to_owned(), pieces.into_iter().map(|p| p.clip).collect());
    }
    Ok(out)
}
