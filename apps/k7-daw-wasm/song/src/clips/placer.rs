// clips/placer.rs — the materials a compile pass places from, and the Placer that holds its grid, its memo, its budget and its findings.
// A reader would get wrong that a Placer is one pass's: its budget and its length memo are spent and kept across the lanes it places.

use super::comp::comp_clips;
use super::doc::{ClipDoc, NestDoc};
use super::paths::AudioPaths;
use super::placement::Cx;
use super::timelines::place_within;
use super::{BUDGET, Budget, Grid, error_in};
use crate::files::{Clip, Finding, Placement, Sequence, Timeline};
use crate::read::FamilyRead;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// What a pass places from: the project's clips, sequences and timelines by id, each timeline's file, and the audio paths.
pub struct Materials {
    pub(super) clips: HashMap<String, Arc<Clip>>,
    pub(super) sequences: HashMap<String, Arc<Sequence>>,
    pub(super) timelines: IndexMap<String, Arc<Timeline>>,
    pub(super) timeline_files: HashMap<String, Arc<str>>,
    pub(super) paths: AudioPaths,
}

impl Materials {
    pub fn new(
        clips: &FamilyRead<Clip>,
        sequences: &FamilyRead<Sequence>,
        timelines: &FamilyRead<Timeline>,
        paths: AudioPaths,
    ) -> Materials {
        Materials {
            clips: clips.values.iter().cloned().collect(),
            sequences: sequences.values.iter().cloned().collect(),
            timelines: timelines.values.iter().cloned().collect(),
            timeline_files: timelines
                .values
                .iter()
                .zip(&timelines.names)
                .map(|((id, _), name)| (id.clone(), Arc::from(format!("timelines/{name}"))))
                .collect(),
            paths,
        }
    }

    /// A timeline by id, which a neighbour walks without the Placer's lengths.
    pub fn timeline(&self, id: &str) -> Option<&Arc<Timeline>> {
        self.timelines.get(id)
    }

    /// The file a timeline is read from, for a finding.
    pub fn timeline_file(&self, id: &str) -> Arc<str> {
        self.timeline_files
            .get(id)
            .cloned()
            .unwrap_or_else(|| Arc::from(format!("timelines/{id}.k7x")))
    }
}

/// Where an expanded event plays: what the timeline events around it add up to.
#[derive(Clone, Debug, PartialEq)]
pub struct Within {
    /// Beats added to its clips' start.
    pub offset: f64,
    /// The beat past which nothing of it plays.
    pub end: f64,
    /// The timeline events' own trims summed, dB.
    pub gain: f64,
    pub disabled: bool,
    /// Its clips' id prefix, one `<ref>@<start>>` per timeline it sits in.
    pub prefix: String,
    /// The outermost timeline event, the one its lane draws.
    pub nest: NestDoc,
}

/// An event of a lane once its timelines are expanded: as written, where it plays, and the file it came from.
#[derive(Clone, Debug)]
pub struct Leaf<'e> {
    pub p: &'e Placement,
    pub within: Option<Arc<Within>>,
    pub file: Arc<str>,
}

/// The findings of a pass, a repeat of one file's message kept once.
#[derive(Default)]
pub(crate) struct Sink {
    pub findings: Vec<Finding>,
    seen: HashSet<(String, String)>,
}

impl Sink {
    pub fn push(&mut self, f: Finding) {
        if self.seen.insert((f.file.clone(), f.message.clone())) {
            self.findings.push(f);
        }
    }

    pub fn error(&mut self, file: &str, message: impl Into<String>) {
        self.push(error_in(file, message));
    }

    /// The one error a spent budget gives, named by the file the pass stopped in.
    pub fn limit(&mut self, budget: &mut Budget, file: &str) {
        if budget.unreported() {
            self.error(
                file,
                "this project asks for more clips than one compile will place — the rest is left out",
            );
        }
    }
}

/// One compile pass's placing: a grid, the materials, a memo of timeline lengths, a budget and the findings.
pub struct Placer<'a> {
    pub(super) grid: Grid,
    pub(super) mats: &'a Materials,
    pub(super) lengths: HashMap<String, f64>,
    pub(super) budget: Budget,
    pub(super) sink: Sink,
    pub(super) last_file: Arc<str>,
}

impl<'a> Placer<'a> {
    pub fn new(grid: Grid, materials: &'a Materials) -> Placer<'a> {
        Placer::with_limit(grid, materials, BUDGET)
    }

    /// A Placer whose budget is `limit`: a test of the cap, never a compile.
    pub fn with_limit(grid: Grid, materials: &'a Materials, limit: usize) -> Placer<'a> {
        Placer {
            grid,
            mats: materials,
            lengths: HashMap::new(),
            budget: Budget::new(limit),
            sink: Sink::default(),
            last_file: Arc::from(""),
        }
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The materials this pass places from, borrowed for as long as the pass's own lifetime.
    pub fn materials(&self) -> &'a Materials {
        self.mats
    }

    /// The budget this pass spends, lent to whoever else spends it (`mark_crossfades`).
    pub fn budget(&mut self) -> &mut Budget {
        &mut self.budget
    }

    /// One event of the lane whose file is `file`; None when it is refused or past the budget, the finding kept.
    pub fn placement_to_clip(&mut self, p: &Placement, file: &str) -> Option<ClipDoc> {
        self.last_file = Arc::from(file);
        let mut cx = Cx {
            grid: &self.grid,
            mats: self.mats,
            sink: &mut self.sink,
            budget: &mut self.budget,
        };
        cx.clip_of(p, file)
    }

    /// An expanded event's clips: the event or the pieces of its comp, moved where its timelines play them.
    pub fn place(&mut self, leaf: &Leaf<'_>) -> Vec<ClipDoc> {
        let file: &str = &leaf.file;
        self.last_file = Arc::clone(&leaf.file);
        let Some(own) = self.placement_to_clip(leaf.p, file) else {
            return Vec::new();
        };
        let (grid, mats) = (self.grid, self.mats);
        let (sink, budget) = (&mut self.sink, &mut self.budget);
        let mut take = |r: &str, start: &str, b: &mut Budget| {
            let p = Placement {
                ref_: Some(crate::files::scalar::NonEmpty(r.to_owned())),
                start: Some(crate::files::WindowEdge::Timecode(start.to_owned())),
                ..Placement::default()
            };
            Cx {
                grid: &grid,
                mats,
                sink: &mut *sink,
                budget: b,
            }
            .clip_of(&p, file)
        };
        let Some(clips) = comp_clips(leaf.p, own, &grid, budget, &mut take) else {
            self.sink.limit(&mut self.budget, file);
            return Vec::new();
        };
        let clips = match &leaf.within {
            Some(w) => {
                // Each clip's id grows by the nest prefix, so its bytes are charged before they are made (decision 157).
                if !self.budget.charge(clips.len() * (w.prefix.len() / 64 + 1)) {
                    self.sink.limit(&mut self.budget, file);
                    return Vec::new();
                }
                place_within(clips, w)
            }
            None => clips,
        };
        // A shifted clip can overflow where the event itself did not.
        let (kept, lost): (Vec<ClipDoc>, Vec<ClipDoc>) = clips.into_iter().partition(ClipDoc::finite);
        if let Some(c) = lost.first() {
            self.sink.error(
                file,
                format!(
                    "'{}': a number in this event is not finite, so it plays nothing",
                    c.name
                ),
            );
        }
        kept
    }

    /// The findings of the pass, whether the budget capped it, and what it spent.
    pub fn finish(mut self) -> (Vec<Finding>, bool, usize) {
        let file = Arc::clone(&self.last_file);
        self.sink.limit(&mut self.budget, &file);
        (self.sink.findings, self.budget.capped(), self.budget.spent())
    }
}
