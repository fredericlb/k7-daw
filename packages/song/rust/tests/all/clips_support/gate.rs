// clips_support/gate.rs — one project's placements as the Rust makes them, in the shape stageDumpClips.ts writes, and what a case expects.
// A reader would get wrong that a lane is taken from `song::lanes::lanes_of` now: a group and a disabled lane place nothing here.

use super::grid_of;
use serde_json::{Map, Value, json};
use song::clips::{
    Half, Materials, Placer, audio_paths, mark_crossfades, pattern_plugins, pattern_shares, read_pattern, sample_banks,
};
use song::modules::load_nodes;
use song::{Cache, Clip, Finding, Placement, Roots, Sequence, Severity, Source, SourceError, Timeline, read_family};

/// What a run saw, counted by what happened and not by what the files say.
#[derive(Default, Clone, Debug)]
pub struct Counts {
    pub lanes: usize,
    pub events: usize,
    pub audio: usize,
    pub sequences: usize,
    pub pattern_lanes: usize,
    pub takes: usize,
    pub comp_pieces: usize,
    pub crossfades: usize,
    pub timeline_docs: usize,
    pub nested: usize,
    pub seconds_starts: usize,
    pub fractional_ids: usize,
    pub loop_ranges: usize,
    pub event_windows: usize,
    pub clip_windows: usize,
    pub gains: usize,
    pub warps: usize,
    pub warp_offs: usize,
    pub disabled: usize,
    pub pad_effects: usize,
}

impl Counts {
    pub fn add(&mut self, o: &Counts) {
        macro_rules! sum {
            ($($f:ident),*) => { $(self.$f += o.$f;)* };
        }
        sum!(
            lanes,
            events,
            audio,
            sequences,
            pattern_lanes,
            takes,
            comp_pieces,
            crossfades,
            timeline_docs,
            nested,
            seconds_starts,
            fractional_ids,
            loop_ranges,
            event_windows,
            clip_windows,
            gains,
            warps,
            warp_offs,
            disabled,
            pad_effects
        );
    }
}

/// A project placed: the dump, the files that carry an error or a warning, and what the passes spent.
pub struct Ran {
    pub project: String,
    pub json: Value,
    pub chain: bool,
    pub refused: Vec<String>,
    pub warned: Vec<String>,
    pub weight: usize,
    pub counts: Counts,
}

fn files(findings: &[Finding], severity: Severity) -> Vec<String> {
    let mut v: Vec<String> = findings
        .iter()
        .filter(|f| f.severity == severity)
        .map(|f| f.file.clone())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// A lane of the resolved nodes, as `laneOf` projects it: only the sound lanes that are neither a group nor disabled.
struct Lane {
    name: String,
    events: Vec<Placement>,
    strudel: Option<String>,
}

fn lanes_of(resolved: &song::Resolution) -> Vec<Lane> {
    let lanes = song::lanes::lanes_of(&resolved.modules);
    lanes
        .sound
        .iter()
        .filter(|(_, l)| l.kind != song::lanes::LaneKind::Group && !l.disabled)
        .map(|(name, l)| Lane {
            name: (*name).to_owned(),
            events: l.events.to_vec(),
            strudel: l.strudel.map(str::to_owned),
        })
        .collect()
}

/// The project at `roots`, read through `source` and placed, lane by lane, as nodeProject.ts hands the engine.
pub fn run(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<Ran, SourceError> {
    let project = song::roots::spell(&roots.project, roots.paths)?;
    let nodes = load_nodes(source, cache, roots)?;
    if nodes.chain_project {
        return Ok(Ran {
            project,
            json: json!({ "chain": true }),
            chain: true,
            refused: Vec::new(),
            warned: Vec::new(),
            weight: 0,
            counts: Counts::default(),
        });
    }
    let vocab = cache.vocabulary(source, roots)?;
    let resolved = song::resolve_map(source, roots, &nodes.nodes, &vocab.modules)?;
    let head = song::read_head(source, cache, &project)?;
    let grid = grid_of(head.root.as_ref().and_then(|r| r.project.as_ref()));
    let clips = read_family::<Clip>(source, cache, &project)?;
    let sequences = read_family::<Sequence>(source, cache, &project)?;
    let timelines = read_family::<Timeline>(source, cache, &project)?;
    let mats = Materials::new(
        &clips,
        &sequences,
        &timelines,
        audio_paths(source, &project, &clips, roots.paths)?,
    );
    let lanes = lanes_of(&resolved);
    let mut counts = Counts::default();
    let mut findings: Vec<Finding> = Vec::new();
    let mut weight = 0;

    // The lanes, one Placer for the pass as a compile holds it.
    let mut placer = Placer::new(grid, &mats);
    let mut lane_docs = Map::new();
    for lane in &lanes {
        let file = format!("nodes/{}.k7x", lane.name);
        counts.lanes += 1;
        counts.events += lane.events.len();
        counts.seconds_starts += lane
            .events
            .iter()
            .filter(|e| matches!(e.start, Some(song::files::WindowEdge::Seconds(_))))
            .count();
        let leaves = placer.expand_events(&lane.events, &file);
        let placed: Vec<_> = leaves.iter().flat_map(|l| placer.place(l)).collect();
        let marked = mark_crossfades(placed, placer.budget());
        lane_docs.insert(lane.name.clone(), serde_json::to_value(&marked).expect("json"));
    }
    let (f, _, w) = placer.finish();
    findings.extend(f);
    weight = weight.max(w);

    // The lengths and the timelines, each asked of a Placer of its own, as the TypeScript asks of a fresh context.
    let mut lengths = Map::new();
    let mut lp = Placer::new(grid, &mats);
    for (id, _) in &timelines.values {
        lengths.insert(id.clone(), json!(lp.timeline_length(id)));
    }
    let (f, _, w) = lp.finish();
    findings.extend(f);
    weight = weight.max(w);
    let mut tp = Placer::new(grid, &mats);
    let docs = tp.timeline_docs();
    counts.timeline_docs += docs.len();
    let timeline_docs: Map<String, Value> = docs
        .iter()
        .map(|d| (d.id.clone(), serde_json::to_value(d).expect("json")))
        .collect();
    let (f, _, w) = tp.finish();
    findings.extend(f);
    weight = weight.max(w);
    for (_, t) in &timelines.values {
        for l in &t.lanes {
            counts.seconds_starts += l
                .events
                .iter()
                .flatten()
                .filter(|e| matches!(e.start, Some(song::files::WindowEdge::Seconds(_))))
                .count();
        }
    }

    // The pattern lanes: their shares, and the plugin of each half set.
    let pattern_lanes: Vec<(&str, &str)> = lanes
        .iter()
        .filter_map(|l| l.strudel.as_deref().map(|s| (l.name.as_str(), s)))
        .collect();
    counts.pattern_lanes += pattern_lanes.len();
    let (shares, share_findings) = pattern_shares(&project, &pattern_lanes, roots.paths);
    findings.extend(share_findings);
    let banks = sample_banks(roots);
    let mut patterns = Map::new();
    for (lane, strudel) in &pattern_lanes {
        let (text, warning) = read_pattern(source, &project, strudel, roots.paths)?;
        findings.extend(warning);
        let share = &shares[*lane];
        let midi = format!("{lane}#midi");
        let audio = format!("{lane}#audio");
        let single = pattern_plugins(lane, &[Half { id: lane, port: None }], &text, share, &banks);
        let split = pattern_plugins(
            lane,
            &[
                Half {
                    id: &midi,
                    port: Some("midi"),
                },
                Half {
                    id: &audio,
                    port: Some("audio"),
                },
            ],
            &text,
            share,
            &banks,
        );
        patterns.insert(
            (*lane).to_owned(),
            json!({ "single": serde_json::to_value(&single).expect("json"), "split": serde_json::to_value(&split).expect("json") }),
        );
    }
    let share_json: Map<String, Value> = shares
        .iter()
        .map(|(k, s)| (k.clone(), json!({ "claimed": s.claimed, "catchAll": s.catch_all })))
        .collect();

    let dump = json!({
        "chain": false,
        "lanes": lane_docs,
        "lengths": lengths,
        "timelines": timeline_docs,
        "shares": share_json,
        "banks": banks,
        "patterns": patterns,
    });
    count_clips(&mut counts, &dump);
    Ok(Ran {
        project,
        json: dump,
        chain: false,
        refused: files(&findings, Severity::Error),
        warned: files(&findings, Severity::Warning),
        weight,
        counts,
    })
}

/// What the clips of a dump did: windows, loops, takes, nests, effects.
fn count_clips(c: &mut Counts, dump: &Value) {
    let mut all: Vec<&Value> = dump["lanes"].as_object().into_iter().flat_map(|m| m.values()).collect();
    let in_timelines: Vec<&Value> = dump["timelines"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|t| t["lanes"].as_array().into_iter().flatten().map(|l| &l["clips"]))
        .collect();
    all.extend(in_timelines);
    for list in all {
        for clip in list.as_array().into_iter().flatten() {
            let midi = clip["type"] == "midi";
            c.audio += usize::from(!midi);
            c.sequences += usize::from(midi);
            c.takes += clip["event"]["takes"].as_array().map_or(0, Vec::len);
            c.comp_pieces += usize::from(clip.get("comp").is_some());
            c.crossfades += usize::from(clip.get("crossfade").is_some());
            c.nested += usize::from(clip.get("nest").is_some());
            c.loop_ranges += usize::from(clip["loopEnd"].as_f64().unwrap_or(0.0) > 0.0);
            c.event_windows += usize::from(clip["event"].get("from").is_some() || clip["event"].get("to").is_some());
            c.clip_windows += usize::from(clip.get("sourceFrom").is_some() || clip.get("sourceTo").is_some());
            c.gains += usize::from(clip.get("gainDb").is_some());
            c.warps += usize::from(clip.get("warp").is_some());
            c.warp_offs += usize::from(clip.get("warpOff").is_some());
            c.disabled += usize::from(clip.get("disabled").is_some());
            let id = clip["id"].as_str().unwrap_or("");
            let start = id.rsplit('@').next().unwrap_or("");
            c.fractional_ids += usize::from(start.contains('.'));
            c.pad_effects += usize::from(
                clip["notes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|n| n.get("repeat").is_some() || n.get("skip").is_some()),
            );
        }
    }
}

/// The lanes the dump holds, by name, for a check that wants only some.
pub fn lane_names(dump: &Value) -> Vec<String> {
    dump["lanes"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}
