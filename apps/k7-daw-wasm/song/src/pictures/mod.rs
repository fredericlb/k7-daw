// pictures/mod.rs — a picture lane's sound and a timeline's pictures: the budget, the findings and the faults one call spends.
// A reader would get wrong that nothing here makes a track: it makes ClipDocs and findings, and 2h places them (decision 500).

mod cover;
mod join;
mod placed;
mod shot;
mod sound;

pub use cover::Cover;
pub use join::{PictureSounds, picture_sounds, placed_shots_of};
pub use placed::{PlacedShots, timeline_shots, with_timeline_shots};
pub(crate) use shot::edge_weight;
pub use shot::{Shot, ShotIn, fade_over, seconds_of, shots_of};
pub use sound::{ClipSounds, LaneShots, SoundLaneIn, SoundPiece, Sounder, picture_lane_sounds, video_sound_lanes};

use crate::clips::{Budget, error_in, warning_in};
use crate::files::Finding;
use std::collections::HashSet;

/// What one call of the picture sounds may spend: shots, spans, stats and pieces each weigh their output (decision 522).
pub const BUDGET: usize = 2_000_000;

/// How many faults a call lists before one `More` stops its walks.
pub const MAX_FAULTS: usize = 1_000;

/// The longest string a fault holds, in characters.
const FAULT_CHARS: usize = 200;

/// The longest word a message quotes, in characters.
const QUOTED: usize = 40;

/// The longest message, in bytes.
const MESSAGE_BYTES: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundFaultKind {
    VideoSourceBoth,
    VideoSourceAbsent,
    SoundMissing,
    More,
}

impl SoundFaultKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SoundFaultKind::VideoSourceBoth => "video_source_both",
            SoundFaultKind::VideoSourceAbsent => "video_source_absent",
            SoundFaultKind::SoundMissing => "sound_missing",
            SoundFaultKind::More => "more",
        }
    }
}

/// A problem the compile refuses on, as data: what the gate compares and what a finding is made from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundFault {
    pub kind: SoundFaultKind,
    pub holder: String,
    pub target: String,
}

impl SoundFault {
    pub fn new(kind: SoundFaultKind, holder: &str, target: &str) -> SoundFault {
        SoundFault {
            kind,
            holder: cut(holder, FAULT_CHARS),
            target: cut(target, FAULT_CHARS),
        }
    }

    /// `<kind>|<holder>|<target>`, what the gate compares.
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.kind.as_str(), self.holder, self.target)
    }
}

/// The first `max` characters of `s`.
pub(crate) fn cut(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// A word as a message quotes it.
pub(crate) fn quoted(s: &str) -> String {
    cut(s, QUOTED)
}

/// What a name or a word weighs to look up: one, and one more for each 64 bytes it hashes.
pub(crate) fn weigh(s: &str) -> usize {
    1 + s.len() / 64
}

/// `message` cut to the longest a finding of this module may be.
fn bounded(mut message: String) -> String {
    if message.len() > MESSAGE_BYTES {
        let mut at = MESSAGE_BYTES;
        while !message.is_char_boundary(at) {
            at -= 1;
        }
        message.truncate(at);
    }
    message
}

/// One call's budget, findings and faults, lent to the walk and the sounder.
pub struct Spend {
    budget: Budget,
    findings: Vec<Finding>,
    seen: HashSet<(String, String)>,
    faults: Vec<SoundFault>,
    more: bool,
    file: String,
}

impl Default for Spend {
    fn default() -> Spend {
        Spend::new()
    }
}

impl Spend {
    pub fn new() -> Spend {
        Spend::with_limit(BUDGET)
    }

    /// A Spend whose budget is `limit`: a test of the cap, never a compile.
    pub fn with_limit(limit: usize) -> Spend {
        Spend {
            budget: Budget::new(limit),
            findings: Vec::new(),
            seen: HashSet::new(),
            faults: Vec::new(),
            more: false,
            file: String::new(),
        }
    }

    pub fn capped(&self) -> bool {
        self.budget.capped()
    }

    pub fn spent(&self) -> usize {
        self.budget.spent()
    }

    /// Spends `n` before the copy it pays for; false for good once the limit is passed.
    pub fn charge(&mut self, n: usize) -> bool {
        self.budget.charge(n)
    }

    /// The file a stop is named by when the walk has no better one: the caller says where it is.
    pub fn in_file(&mut self, file: &str) {
        if self.file != file {
            file.clone_into(&mut self.file);
        }
    }

    /// Whether a fault past the limit has stopped the walks.
    pub fn stopped(&self) -> bool {
        self.more
    }

    /// The file a stop is named by when the walk has no better one.
    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn faults(&self) -> &[SoundFault] {
        &self.faults
    }

    pub fn into_parts(self) -> (Vec<Finding>, Vec<SoundFault>) {
        (self.findings, self.faults)
    }

    fn push(&mut self, f: Finding) {
        if self.seen.insert((f.file.clone(), f.message.clone())) {
            self.findings.push(f);
        }
    }

    pub(crate) fn error(&mut self, file: &str, message: String) {
        self.push(error_in(file, bounded(message)));
    }

    pub(crate) fn warning(&mut self, file: &str, message: String) {
        self.push(warning_in(file, bounded(message)));
    }

    /// The one error a spent budget gives, named by the file the walk stopped in.
    pub(crate) fn limit(&mut self, file: &str) {
        if self.budget.unreported() {
            let named = if file.is_empty() {
                self.file.clone()
            } else {
                file.to_owned()
            };
            self.error(
                &named,
                "this project's pictures ask for more sounds than one compile will make — the rest is left out"
                    .to_owned(),
            );
        }
    }

    /// A fault and its error on `file`; false once the walks must stop (past `MAX_FAULTS`, one `More` says so).
    pub fn fault(&mut self, fault: SoundFault, file: &str, message: String) -> bool {
        if self.more {
            return false;
        }
        if self.faults.len() >= MAX_FAULTS {
            self.more = true;
            self.faults.push(SoundFault::new(SoundFaultKind::More, "", ""));
            self.error(
                file,
                "there are more problems with sounds than are listed here — fix these first".to_owned(),
            );
            return false;
        }
        self.faults.push(fault);
        self.error(file, message);
        true
    }
}
