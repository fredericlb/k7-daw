// clips/mod.rs — placements to clips: the ids, the windows, the comps, the timelines and the pattern lanes, and the budget they share.

pub mod audio_def;
mod audition;
mod comp;
mod distinct;
mod doc;
mod id;
mod paths;
mod pattern;
mod placement;
mod placer;
mod takes;
mod timelines;
mod warp;

pub use audition::{audition_hosts, audition_timeline, material_ref_of, with_take_hole};
pub use comp::{comp_clips, cut_hole, mark_crossfades, slice_clip};
pub use distinct::{Distinct, REPEAT_SEP, distinct_ids, placement_start_of, without_repeat};
pub use doc::{
    ClipDoc, ClipKind, CompMark, CrossfadeMark, EventDoc, NestDoc, NoteDoc, PatternPlugin, Plays, TimelineDoc,
    TimelineLaneDoc, WarpDoc,
};
pub use id::{NEST_SEP, js_min, js_number, placement_id, timeline_lane_id};
pub use paths::{AudioPaths, audio_paths};
pub use pattern::{
    Half, MAX_FILE_LANES, MAX_LANES, MAX_PATTERN, PATTERN_PLUGIN_TYPE, PatternShare, pattern_plugins, pattern_shares,
    read_pattern, sample_banks,
};
pub use placer::{Leaf, Materials, Placer, Within};
pub use takes::{CompSpan, FadedPiece, Piece, SeamFade, TakeRef, comp_pieces, seam_crossfades};
pub(crate) use timelines::nest_message;
pub use timelines::{FIXED_FIELDS, fixed_fields_of};
pub use warp::read_clip_warp;

use crate::files::{Finding, ProjectFile, WindowEdge, time_sig_of};
use song_schema::{TimeSig, parse_duration, parse_position};

/// What one Placer may produce: a leaf, a clip made (with its notes, markers and words) and a length visit each weigh their output.
pub const BUDGET: usize = 1_000_000;

/// How many timelines deep an expansion goes before it is cut.
pub const MAX_NEST: usize = 32;

/// The time grid a project's positions are read on: its signature and its tempo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub sig: TimeSig,
    pub tempo: f64,
}

impl Grid {
    pub fn new(sig: TimeSig, tempo: f64) -> Grid {
        Grid { sig, tempo }
    }

    /// The grid `project.k7x` states; the tempo defaults to 120 and the signature to 4/4.
    pub fn of_project(file: &ProjectFile) -> Result<Grid, String> {
        let settings = file.project.as_ref();
        let sig = time_sig_of(settings.and_then(|p| p.time_signature.as_ref()).map(|t| t.0.as_str()))?;
        let tempo = settings.and_then(|p| p.tempo.as_ref()).map_or(120.0, |t| t.0);
        Ok(Grid::new(sig, tempo))
    }

    /// An event's start in beats: a number is seconds, a timecode a 1-based position, none is `1:1:1`.
    pub fn position(&self, e: Option<&WindowEdge>) -> f64 {
        match e {
            Some(WindowEdge::Seconds(s)) => (s * self.tempo) / 60.0,
            Some(WindowEdge::Timecode(t)) => self.position_text(t),
            None => self.position_text("1:1:1"),
        }
    }

    /// A 1-based position in beats; text that is no timecode is NaN, which a clip then refuses (decision 156).
    pub fn position_text(&self, tc: &str) -> f64 {
        parse_position(tc, self.sig).unwrap_or(f64::NAN)
    }

    /// A 0-based duration in beats; text that is no timecode is NaN.
    pub fn duration(&self, tc: &str) -> f64 {
        parse_duration(tc, self.sig).unwrap_or(f64::NAN)
    }

    /// Beats in a bar, as `(num * 4) / den`.
    pub fn bar(&self) -> f64 {
        (f64::from(self.sig.num()) * 4.0) / f64::from(self.sig.den())
    }

    pub fn seconds(&self, beats: f64) -> f64 {
        (beats * 60.0) / self.tempo
    }

    pub fn beats(&self, seconds: f64) -> f64 {
        (seconds * self.tempo) / 60.0
    }
}

/// What a pass may spend: charged BEFORE the copy it pays for, and once past the limit nothing more is placed.
#[derive(Clone, Debug)]
pub struct Budget {
    limit: usize,
    spent: usize,
    capped: bool,
    reported: bool,
}

impl Budget {
    pub fn new(limit: usize) -> Budget {
        Budget {
            limit,
            spent: 0,
            capped: false,
            reported: false,
        }
    }

    /// Spends `n`; false (for good) once the limit is passed.
    pub fn charge(&mut self, n: usize) -> bool {
        if self.capped {
            return false;
        }
        self.spent = self.spent.saturating_add(n);
        if self.spent > self.limit {
            self.capped = true;
        }
        !self.capped
    }

    pub fn capped(&self) -> bool {
        self.capped
    }

    pub fn spent(&self) -> usize {
        self.spent
    }

    /// Whether the limit has been passed and nobody has said so yet; asking marks it said.
    pub(crate) fn unreported(&mut self) -> bool {
        let first = self.capped && !self.reported;
        self.reported |= self.capped;
        first
    }
}

/// An error naming the file an event came from, placed at its first character.
pub(crate) fn error_in(file: &str, message: impl Into<String>) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message: message.into(),
        severity: crate::files::Severity::Error,
    }
}

/// A warning named like an error.
pub(crate) fn warning_in(file: &str, message: impl Into<String>) -> Finding {
    Finding {
        severity: crate::files::Severity::Warning,
        ..error_in(file, message)
    }
}
