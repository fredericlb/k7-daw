// pictures_join.rs — the join over a project in memory: the walk's order, the files a stop is named by, and a hostile count of lanes.
// A reader would get wrong that the project is read for real: lanes, clips, chains and timelines come through load_inputs.

use crate::support;

use serde_json::json;
use song::clips::{Grid, Materials, Placer};
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::rig::Rig;
use song::pictures::{BUDGET, MAX_FAULTS, PictureSounds, picture_sounds};
use song::{Cache, Entry, Severity, Source, SourceError, Span, Stat};
use song_schema::TimeSig;
use std::time::{Duration, Instant};
use support::{mem, mem_world_roots};

fn project(files: &[(String, String)]) -> song::MemSource {
    let laid: Vec<(String, &str)> = files
        .iter()
        .map(|(p, t)| (format!("ws/proj/{p}"), t.as_str()))
        .collect();
    mem(&laid.iter().map(|(p, t)| (p.as_str(), *t)).collect::<Vec<_>>())
}

fn joined(source: &dyn Source) -> Result<(Inputs, PictureSounds), SourceError> {
    let inputs = load_inputs(
        source,
        &mut Cache::new(),
        &mem_world_roots(),
        Rig::from_json(&json!({})),
    )?;
    let lanes = inputs.lanes();
    let mats = Materials::new(
        &inputs.clips,
        &inputs.sequences,
        &inputs.timelines,
        inputs.audio_paths.clone(),
    );
    let grid = Grid::new(TimeSig::new(4, 4).expect("4/4"), 120.0);
    let out = picture_sounds(source, &inputs, &lanes, &mut Placer::new(grid, &mats))?;
    Ok((inputs, out))
}

fn base() -> Vec<(String, String)> {
    [
        ("project.k7x", "project({ name: \"c\", tempo: 120 })\n"),
        ("clips/a.k7x", "source(\"clips/a.mp4\")\nsound(\"clips/a.wav\")\n"),
        ("clips/a.mp4", ""),
        ("clips/a.wav", ""),
    ]
    .iter()
    .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
    .collect()
}

fn picture(name: &str, events: &str) -> (String, String) {
    (
        format!("nodes/{name}.k7x"),
        format!("extends(\"@meta/picture-track\")\nevents({events})\n"),
    )
}

#[test]
fn the_join_gives_the_lane_its_pictures_sound_and_the_picture_lane_none() {
    // decisions 517 and 526: a lane naming the picture lane sounds it, and the picture lane's own half stays silent
    let mut files = base();
    files.push(picture("cam", "[{ ref: \"a\", start: 0, length: 2 }]"));
    files.push((
        "nodes/dialog.k7x".into(),
        "extends(\"@meta/track\")\nvideo_source(\"cam\")\n".into(),
    ));
    let (_, out) = joined(&project(&files)).expect("a join");
    assert_eq!(out.video_sounds.keys().collect::<Vec<_>>(), ["dialog"]);
    assert_eq!(out.video_sounds["dialog"].len(), 1);
    assert!(out.picture_sounds.is_empty() && out.faults.is_empty() && !out.capped);
}

#[test]
fn a_walk_that_runs_out_is_one_error_on_the_timeline_it_stopped_in_and_the_join_says_it_capped() {
    // decisions 523 and 541: the limit's error names the file the walk stopped in
    let mut files = base();
    let place: Vec<String> = (0..300).map(|_| "{ ref: \"t0\", start: \"1:1\" }".to_owned()).collect();
    for i in 0..4 {
        let next = if i < 3 {
            place
                .iter()
                .map(|p| p.replace("t0", &format!("t{}", i + 1)))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            String::new()
        };
        files.push((
            format!("timelines/t{i}.k7x"),
            format!(
                "length(\"1:0\")\nlanes([{{ lane: \"x\", events: [{next}] }}])\npictures([{{ lane: \"cam\", events: [{{ ref: \"a\", start: 0, length: 1 }}] }}])\n"
            ),
        ));
    }
    files.push(picture("cam", "[]"));
    files.push((
        "nodes/gtr.k7x".into(),
        format!("extends(\"@meta/track\")\nevents([{}])\n", place.join(",")),
    ));
    let (_, out) = joined(&project(&files)).expect("a join");
    assert!(out.capped, "27 million shots must reach the budget");
    let stops: Vec<&str> = out
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.file.as_str())
        .collect();
    assert_eq!(stops.len(), 1);
    assert!(stops[0].starts_with("timelines/t"), "{stops:?}");
    assert!(out.spent <= BUDGET + 8 * BUDGET / 64);
}

/// A source whose every `.wav` is a lost bundle entry: only a sound's stat answers Missing.
struct Lost<'a>(&'a dyn Source);

impl Source for Lost<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        if path.ends_with(".wav") {
            return Err(SourceError::Missing);
        }
        self.0.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.0.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.0.read(path, span)
    }
}

#[test]
fn a_stat_a_bundle_lacks_is_missing_for_the_caller() {
    // the throw-or-finding table: Missing from a MemSource is Err(Missing)
    let mut files = base();
    files.push(picture("cam", "[{ ref: \"a\", start: 0, length: 2 }]"));
    let src = project(&files);
    assert!(matches!(joined(&Lost(&src)), Err(SourceError::Missing)));
}

#[test]
fn twenty_thousand_lanes_naming_nothing_are_a_thousand_faults_and_more() {
    // decision 515, over the real join: the faults stop at MAX_FAULTS whatever the lane count
    let mut files = base();
    files.push(picture("cam", "[]"));
    for i in 0..20_000 {
        files.push((
            format!("nodes/l{i}.k7x"),
            "extends(\"@meta/track\")\nvideo_source(\"nowhere\")\n".to_owned(),
        ));
    }
    let src = project(&files);
    let t = Instant::now();
    let (_, out) = joined(&src).expect("a join");
    assert!(
        t.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 }),
        "took {:?}",
        t.elapsed()
    );
    assert_eq!(out.faults.len(), MAX_FAULTS + 1);
    assert_eq!(out.faults.last().map(|f| f.kind.as_str()), Some("more"));
    assert!(out.video_sounds.is_empty() && out.faults.iter().all(|f| f.holder.len() < 200));
}
