// clips_support/mod.rs — what the clips tests share: materials built from JSON, a grid, a placer and a placement from a value.
// A reader would get wrong that nothing here reads a file: the families are made from values, so a case states only what it tests.

#![allow(dead_code)]

mod cases;
mod gate;
#[allow(unused_imports)]
pub use cases::*;
#[allow(unused_imports)]
pub use gate::*;

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use song::clips::{AudioPaths, ClipDoc, Grid, Leaf, Materials, Placer};
use song::files::{AudioClip, VideoClip};
use song::{Clip, FamilyRead, Placement, Sequence, Timeline};
use song_schema::TimeSig;
use std::sync::Arc;

pub fn grid(tempo: f64) -> Grid {
    Grid::new(TimeSig::new(4, 4).expect("4/4"), tempo)
}

/// A family of values by id, as the reader would answer it, the file names `<id>.k7x`.
pub fn family<T: DeserializeOwned>(items: &[(&str, Value)]) -> FamilyRead<T> {
    FamilyRead {
        values: items
            .iter()
            .map(|(id, v)| {
                (
                    (*id).to_owned(),
                    Arc::new(serde_json::from_value(v.clone()).expect("a value")),
                )
            })
            .collect(),
        names: items.iter().map(|(id, _)| format!("{id}.k7x")).collect(),
        findings: Vec::new(),
    }
}

pub fn clip_family(items: &[(&str, Value)]) -> FamilyRead<Clip> {
    let source = |v: &Value| v["source"].as_str().unwrap_or("").to_owned();
    let video = |s: &str| [".mp4", ".mov", ".png"].iter().any(|e| s.ends_with(e));
    FamilyRead {
        values: items
            .iter()
            .map(|(id, v)| {
                let clip = if video(&source(v)) {
                    Clip::Video(serde_json::from_value::<VideoClip>(v.clone()).expect("a video"))
                } else {
                    Clip::Audio(serde_json::from_value::<AudioClip>(v.clone()).expect("an audio"))
                };
                ((*id).to_owned(), Arc::new(clip))
            })
            .collect(),
        names: items.iter().map(|(id, _)| format!("{id}.k7x")).collect(),
        findings: Vec::new(),
    }
}

/// Each clip's source under `/p`, the way the reference check would have left it.
pub fn paths(clips: &FamilyRead<Clip>) -> AudioPaths {
    let mut out = IndexMap::new();
    for (id, c) in &clips.values {
        let src = match &**c {
            Clip::Audio(a) => a.source.0.clone(),
            Clip::Video(v) => v.source.0.clone(),
            Clip::Caption(c) => c.source.0.clone(),
        };
        out.insert(id.clone(), format!("/p/{src}"));
    }
    out
}

/// Materials from clips, sequences and timelines given as JSON by id.
pub fn world(clips: &[(&str, Value)], sequences: &[(&str, Value)], timelines: &[(&str, Value)]) -> Materials {
    let c = clip_family(clips);
    let p = paths(&c);
    Materials::new(&c, &family::<Sequence>(sequences), &family::<Timeline>(timelines), p)
}

pub fn event(v: Value) -> Placement {
    serde_json::from_value(v).expect("a placement")
}

/// One placement compiled at `tempo`, 4/4, against `mats`.
pub fn one(mats: &Materials, tempo: f64, p: Value) -> Option<ClipDoc> {
    Placer::new(grid(tempo), mats).placement_to_clip(&event(p), "nodes/x.k7x")
}

/// One lane's events placed as 2h would, no timeline among them, at `tempo`.
pub fn lane(mats: &Materials, tempo: f64, events: &[Value]) -> Vec<ClipDoc> {
    let evs: Vec<Placement> = events.iter().cloned().map(event).collect();
    let mut placer = Placer::new(grid(tempo), mats);
    let file: Arc<str> = Arc::from("nodes/x.k7x");
    let leaves: Vec<Leaf<'_>> = evs
        .iter()
        .map(|p| Leaf {
            p,
            within: None,
            file: Arc::clone(&file),
        })
        .collect();
    leaves.iter().flat_map(|l| placer.place(l)).collect()
}

/// The grid a project states, 4/4 at 120 when it states none.
pub fn grid_of(file: Option<&song::files::ProjectFile>) -> Grid {
    file.and_then(|f| Grid::of_project(f).ok())
        .unwrap_or_else(|| grid(120.0))
}
