// views_partial.rs — what hints_of keeps when the budget ends a stage: every list already paid for, media included.
// A reader would get wrong that the media walk has its own spend: a project with a large media folder still lists what it found.

use crate::views_support;
use crate::views_world;

use indexmap::IndexMap;
use song::files::Clip;
use song::modules::{LoadedModule, ModuleKind};
use song::views::hints::{HintsAnswer, hints_of};
use song::{Cache, Kind};
use std::sync::Arc;
use views_support::fast;
use views_world::{Fake, bare, decoded, entry, plugin_module};

fn hints(inputs: &song::lanes::inputs::Inputs, source: &dyn song::Source) -> HintsAnswer {
    let roots = song::Roots {
        project: inputs.project.clone(),
        ..song::Roots::default()
    };
    hints_of(source, &mut Cache::new(), &roots, inputs).expect("hints")
}

fn says(a: &HintsAnswer, what: &str) -> bool {
    a.findings
        .iter()
        .any(|f| f.file == "project.k7x" && f.message.contains(what))
}

#[test]
fn a_media_walk_past_the_budget_keeps_the_folders_it_listed_and_says_so() {
    // 5,200 folders of one file each: the walk ends at the budget, with the folders it reached.
    let mut fake = Fake::default();
    let folders: Vec<_> = (0..5_200).map(|i| entry(&format!("d{i}"), Kind::Dir)).collect();
    fake.dir("/p/ws/proj", folders);
    for i in 0..5_200 {
        fake.dir(&format!("/p/ws/proj/d{i}"), vec![entry("a.wav", Kind::File)]);
    }
    let a = fast("5,200 media folders", || hints(&bare(IndexMap::new()), &fake));
    assert!(a.capped && says(&a, "media folders"));
    assert!(a.hints.media.len() > 4_000, "{}", a.hints.media.len());
    assert!(a.hints.media.values().all(|f| f == &["a.wav"]));
}

#[test]
fn a_list_the_budget_ends_inside_keeps_what_was_copied() {
    // base refs at 2 units each: 600,000 end the budget inside the first list.
    let vocab: IndexMap<String, LoadedModule> = (0..600_000)
        .map(|i| {
            (
                format!("@lib/audio/m{i}"),
                plugin_module("audio", &format!("m{i}"), "reverb"),
            )
        })
        .collect();
    let a = fast("600,000 base refs", || hints(&bare(vocab), &Fake::default()));
    assert!(a.capped && says(&a, "completion vocabulary"));
    assert!(a.hints.base_modules.len() > 100_000, "{}", a.hints.base_modules.len());
}

#[test]
fn the_modules_the_budget_ends_inside_are_kept_after_the_base_list() {
    // 12,000 nodes named with 6,400 bytes cost about 101 units each: the budget ends inside `modules`.
    let vocab: IndexMap<String, LoadedModule> = (0..1_000)
        .map(|i| {
            (
                format!("@lib/audio/m{i}"),
                plugin_module("audio", &format!("m{i}"), "reverb"),
            )
        })
        .collect();
    let mut inputs = bare(vocab);
    for i in 0..12_000 {
        let name = format!("{i:0>6400}");
        inputs
            .nodes
            .nodes
            .insert(name.clone(), LoadedModule::bare(ModuleKind::Plugin, "", &name));
    }
    let a = fast("12,000 long node names", || hints(&inputs, &Fake::default()));
    assert!(a.capped);
    assert_eq!(a.hints.base_modules.len(), 1_000);
    assert!(
        (5_000..12_000).contains(&a.hints.modules.len()),
        "{}",
        a.hints.modules.len()
    );
}

#[test]
fn the_audio_clips_the_budget_ends_inside_are_kept() {
    // 400,000 clips: the clip list and the sorting pass leave room for part of the audio list.
    let mut inputs = bare(IndexMap::new());
    let clip: Arc<Clip> = decoded("clips/a.k7x", "source(\"a.wav\")\n");
    inputs.clips.values = (0..400_000).map(|i| (format!("c{i}"), clip.clone())).collect();
    let a = fast("400,000 clips", || hints(&inputs, &Fake::default()));
    assert!(a.capped);
    let audio = a.hints.audio_clips.as_ref().map_or(0, Vec::len);
    assert!((100_000..400_000).contains(&audio), "{audio}");
    assert_eq!(a.hints.video_clips.as_ref().map(Vec::len), Some(0));
}
