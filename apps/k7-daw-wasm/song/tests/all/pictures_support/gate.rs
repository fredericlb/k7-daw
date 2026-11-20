// pictures_support/gate.rs — one project's picture sounds and placed shots in the shape stageDumpPictures.ts writes, with what happened counted.
// A reader would get wrong that a count reads the Rust's own answer: the shots behind it are asked again here, never taken from the dump.

use serde_json::{Map, Value, json};
use song::clips::{ClipDoc, Grid, Materials, Placer};
use song::files::Placement;
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::rig::Rig;
use song::lanes::{Lane, Lanes};
use song::pictures::{
    ClipSounds, LaneShots, ShotIn, SoundFaultKind, Sounder, Spend, picture_sounds, placed_shots_of, shots_of,
    with_timeline_shots,
};
use song::{Cache, Finding, Roots, Severity, Source, SourceError};
use song_schema::TimeSig;
use std::collections::HashSet;

/// What a run saw, counted by what happened and not by what the files say.
#[derive(Default, Clone, Debug)]
pub struct Counts {
    pub pic_lanes: usize,
    pub frames: usize,
    pub events: usize,
    pub activations: usize,
    pub pieces: usize,
    pub reentered: usize,
    pub multi_cut: usize,
    pub crossfades: usize,
    pub cross_ties: usize,
    pub open_pieces: usize,
    pub from_pieces: usize,
    pub varispeed: usize,
    pub elastique: usize,
    pub silent_cover: usize,
    pub freeze_cover: usize,
    pub reverse_cover: usize,
    pub video_lanes: usize,
    pub own_lanes: usize,
    pub wired_empty: usize,
    pub fault_both: usize,
    pub fault_absent: usize,
    pub fault_missing: usize,
    pub placed: usize,
    pub nested: usize,
    pub cut: usize,
    pub given: usize,
    pub merged: usize,
    pub sounded: usize,
    pub max_spent: usize,
}

impl Counts {
    pub fn add(&mut self, o: &Counts) {
        macro_rules! sum {
            ($($f:ident),*) => { $(self.$f += o.$f;)* };
        }
        sum!(
            pic_lanes,
            frames,
            events,
            activations,
            pieces,
            reentered,
            multi_cut,
            crossfades,
            cross_ties,
            open_pieces,
            from_pieces,
            varispeed,
            elastique,
            silent_cover,
            freeze_cover,
            reverse_cover,
            video_lanes,
            own_lanes,
            wired_empty,
            fault_both,
            fault_absent,
            fault_missing,
            placed,
            nested,
            cut,
            given,
            merged,
            sounded
        );
        self.max_spent = self.max_spent.max(o.max_spent);
    }
}

/// A project run: the dump, the files that carry an error or a warning, the fault keys, and what the call spent.
pub struct Ran {
    pub json: Value,
    pub stopped: Vec<String>,
    pub refused: Vec<String>,
    pub warned: Vec<String>,
    pub faults: Vec<String>,
    pub capped: bool,
    pub counts: Counts,
}

fn files<'a>(findings: impl Iterator<Item = &'a Finding>, severity: Severity) -> Vec<String> {
    let mut v: Vec<String> = findings
        .filter(|f| f.severity == severity)
        .map(|f| f.file.clone())
        .collect();
    v.sort();
    v.dedup();
    v
}

fn grid_of(inputs: &Inputs) -> Grid {
    let file = inputs.head.root.as_ref().and_then(|r| r.project.as_ref());
    file.and_then(|f| Grid::of_project(f).ok())
        .unwrap_or_else(|| Grid::new(TimeSig::new(4, 4).expect("4/4"), 120.0))
}

fn ref_of(p: &Placement) -> &str {
    p.ref_.as_ref().map_or("", |r| r.0.as_str())
}

fn placed_json(placed: &LaneShots<'_>) -> Value {
    let lanes: Map<String, Value> = placed
        .iter()
        .map(|(name, shots)| {
            let list: Vec<Value> = shots
                .iter()
                .map(|s| json!({"ref": ref_of(s.event), "start": s.start, "length": s.length, "timeline": s.timeline}))
                .collect();
            ((*name).to_owned(), Value::Array(list))
        })
        .collect();
    Value::Object(lanes)
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

/// The structure behind a lane's pieces: re-entries, crossfades and the shots that cover without sounding.
fn count_lane(
    c: &mut Counts,
    sounds: &ClipSounds<'_>,
    shots: &[ShotIn<'_>],
    pieces: &[song::pictures::SoundPiece],
    bps: f64,
) {
    let list = shots_of(shots);
    let mut per = vec![0usize; list.len()];
    for p in pieces {
        per[p.shot] += 1;
        let s = &list[p.shot];
        let clip: &ClipDoc = &p.clip;
        if clip.fade_out > 0.0 && clip.length > 0.0 && !same((clip.start + clip.length) / bps, s.end) {
            c.crossfades += 1;
            let end = (clip.start + clip.length) / bps;
            let covering: Vec<f64> = list[p.shot + 1..]
                .iter()
                .filter(|l| !l.ref_.is_empty() && l.fade_in > 0.0 && same(l.start + l.fade_in, end))
                .map(|l| l.fade_in)
                .collect();
            let last = covering.last().copied().unwrap_or(0.0);
            if covering.len() >= 2 && covering.iter().any(|f| !same(*f, last)) && same(clip.fade_out / bps, last) {
                c.cross_ties += 1;
            }
        }
    }
    c.reentered += per.iter().filter(|n| **n >= 2).count();
    c.multi_cut += per.iter().filter(|n| **n >= 3).count();
    for (i, s) in list.iter().enumerate() {
        let kind = if s.silent {
            0
        } else if s.speed == 0.0 {
            1
        } else if s.speed < 0.0 {
            2
        } else {
            continue;
        };
        let from = s.start + s.fade_in;
        let covers = list[..i].iter().any(|e| {
            !e.ref_.is_empty()
                && sounds.sound(e.ref_).is_some()
                && !e.silent
                && e.speed > 0.0
                && e.start < s.end
                && e.end > from
        });
        if covers && from < s.end {
            match kind {
                0 => c.silent_cover += 1,
                1 => c.freeze_cover += 1,
                _ => c.reverse_cover += 1,
            }
        }
    }
}

/// What the shots and pieces of a project say about how it was made, over the lanes the answer sounds.
fn count_structure(
    inputs: &Inputs,
    lanes: &Lanes<'_>,
    source: &dyn Source,
    mats: &Materials,
    grid: Grid,
    used: &HashSet<&str>,
    c: &mut Counts,
) -> Result<(), SourceError> {
    let mut placer = Placer::new(grid, mats);
    let mut spend = Spend::new();
    let placed = placed_shots_of(lanes, &mut placer, &mut spend);
    for (lane, list) in &placed {
        // A merge has an effect when the lane's own last shot is open: it would end at the first placed one.
        let open_last = lanes.picture.get(lane).is_some_and(|l: &Lane<'_>| {
            l.events
                .last()
                .is_some_and(|e| e.length.is_none_or(|n| n.0 <= 0.0 || n.0.is_nan()))
        });
        for s in list {
            c.placed += 1;
            c.nested += usize::from(s.nested);
            let stated = s.event.length.map_or(0.0, |l| l.0);
            c.cut += usize::from(stated > 0.0 && s.length < stated - 1e-9);
            c.given += usize::from(stated <= 0.0);
            c.merged += usize::from(open_last);
        }
    }
    let bps = grid.tempo / 60.0;
    let held: Vec<(&str, usize)> = placed.iter().map(|(n, v)| (*n, v.len())).collect();
    let pictures = with_timeline_shots(
        lanes.picture.iter().map(|(n, l)| (*n, l.events)),
        placed,
        &mut Spend::new(),
        &|n| format!("nodes/{n}.k7x"),
    );
    let sounds = ClipSounds::of(&inputs.clips);
    for (lane, shots) in &pictures {
        if !used.contains(lane) {
            continue;
        }
        let mut spend = Spend::new();
        let pieces = Sounder::new(source, &inputs.project, bps, &sounds, &mut spend).pieces(shots, "x")?;
        let placed_here = held.iter().find(|(n, _)| n == lane).map_or(0, |(_, n)| *n);
        let heard: HashSet<usize> = pieces.iter().map(|p| p.shot).filter(|s| *s < placed_here).collect();
        c.sounded += heard.len();
        count_lane(c, &sounds, shots, &pieces, bps);
    }
    Ok(())
}

/// The project at `roots`, read through `source` and given its picture sounds, as nodeProject.ts lines 218 to 244 do.
pub fn run(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<Ran, SourceError> {
    let inputs = load_inputs(source, cache, roots, Rig::from_json(&json!({})))?;
    let stopped = files(inputs.stops().into_iter(), Severity::Error);
    let lanes = inputs.lanes();
    let grid = grid_of(&inputs);
    let mats = Materials::new(
        &inputs.clips,
        &inputs.sequences,
        &inputs.timelines,
        inputs.audio_paths.clone(),
    );
    let mut placer = Placer::new(grid, &mats);
    let answer = picture_sounds(source, &inputs, &lanes, &mut placer)?;
    let mut counts = Counts {
        pic_lanes: lanes.picture.len(),
        frames: lanes.picture.values().filter(|l| l.frame.is_some()).count(),
        events: lanes.picture.values().map(|l| l.events.len()).sum(),
        activations: lanes
            .picture
            .values()
            .flat_map(|l| l.events)
            .filter(|e| ref_of(e).is_empty())
            .count(),
        max_spent: answer.spent,
        ..Counts::default()
    };
    let clips = |m: &indexmap::IndexMap<String, Vec<ClipDoc>>| m.values().flatten().cloned().collect::<Vec<_>>();
    let all: Vec<ClipDoc> = clips(&answer.video_sounds)
        .into_iter()
        .chain(clips(&answer.picture_sounds))
        .collect();
    counts.pieces = all.len();
    counts.open_pieces = all.iter().filter(|c| c.length == 0.0).count();
    counts.from_pieces = all.iter().filter(|c| c.source_from.is_some()).count();
    counts.varispeed = all
        .iter()
        .filter(|c| c.warp.as_ref().is_some_and(|w| w.algorithm == "varispeed"))
        .count();
    counts.elastique = all
        .iter()
        .filter(|c| c.warp.as_ref().is_some_and(|w| w.algorithm == "elastique"))
        .count();
    counts.video_lanes = answer.video_sounds.values().filter(|v| !v.is_empty()).count();
    counts.own_lanes = answer.picture_sounds.values().filter(|v| !v.is_empty()).count();
    counts.wired_empty = answer.picture_sounds.values().filter(|v| v.is_empty()).count();
    for f in &answer.faults {
        match f.kind {
            SoundFaultKind::VideoSourceBoth => counts.fault_both += 1,
            SoundFaultKind::VideoSourceAbsent => counts.fault_absent += 1,
            SoundFaultKind::SoundMissing => counts.fault_missing += 1,
            SoundFaultKind::More => {}
        }
    }
    let mut used: HashSet<&str> = answer.picture_sounds.keys().map(String::as_str).collect();
    for lane in answer.video_sounds.keys() {
        used.extend(lanes.sound.get(lane.as_str()).and_then(|l| l.video_source));
    }
    count_structure(&inputs, &lanes, source, &mats, grid, &used, &mut counts)?;
    let mut placer = Placer::new(grid, &mats);
    let placed = placed_shots_of(&lanes, &mut placer, &mut Spend::new());
    let mut faults: Vec<String> = answer.faults.iter().map(song::pictures::SoundFault::key).collect();
    faults.sort();
    faults.dedup();
    let dump = json!({
        "video_sounds": answer.video_sounds,
        "picture_sounds": answer.picture_sounds,
        "placed": placed_json(&placed),
        "faults": faults,
    });
    Ok(Ran {
        json: dump,
        stopped,
        refused: files(answer.findings.iter(), Severity::Error),
        warned: files(answer.findings.iter(), Severity::Warning),
        faults,
        capped: answer.capped,
        counts,
    })
}
