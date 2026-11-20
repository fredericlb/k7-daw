// pictures_doc.rs — a picture lane's sound in the engine's document: videoSound.test.ts and pictureLaneSound.test.ts over `song::compile`.
// A reader would get wrong that a `toThrow` wording is matched: a stop is asserted by the files it names and the kind of finding (the page's rule).

use crate::doc_support;

use doc_support::{compiled, stopped};
use song::doc::{Compiled, ProjectDoc, TrackDoc};
use song::files::Severity;
use std::time::{Duration, Instant};

// Every expected number reads at 120 bpm: a second is two beats.
const PROJECT: &str = "project({ name: \"c\", tempo: 120 })\n";
const GRAPH: &str = "modules({})\n";
const WAV_A: &str = "/p/ws/proj/clips/a.wav";
const WAV_B: &str = "/p/ws/proj/clips/b.wav";

fn picture(events: &str) -> String {
    format!("extends(\"@meta/picture-track\")\nevents({events})\n")
}

fn dialog(extra: &str) -> String {
    format!("extends(\"@meta/track\")\n{extra}")
}

/// The base project with `extra` laid over it, a later file replacing an earlier one.
fn run(extra: &[(&str, &str)]) -> Compiled {
    let mut files: Vec<(&str, &str)> = vec![
        ("project.k7x", PROJECT),
        ("graph.k7x", GRAPH),
        ("clips/a.k7x", "source(\"clips/a.mp4\")\nsound(\"clips/a.wav\")\n"),
        ("clips/b.k7x", "source(\"clips/b.mp4\")\nsound(\"clips/b.wav\")\n"),
        ("clips/still.k7x", "source(\"clips/still.mp4\")\n"),
        ("clips/a.mp4", ""),
        ("clips/a.wav", ""),
        ("clips/b.mp4", ""),
        ("clips/b.wav", ""),
        ("clips/still.mp4", ""),
    ];
    files.extend_from_slice(extra);
    compiled(&files)
}

fn doc(extra: &[(&str, &str)]) -> ProjectDoc {
    match run(extra).engine {
        Ok(d) => d,
        Err(f) => panic!("the compile failed: {:?}", f.diagnostics),
    }
}

fn track<'a>(d: &'a ProjectDoc, id: &str) -> Option<&'a TrackDoc> {
    d.tracks.iter().find(|t| t.id == id)
}

const TWO: &str = "[{ ref: \"a\", start: 0, length: 5 }, { ref: \"b\", start: 4, length: 5, fadeIn: 1 }]";

// videoSound.test.ts "gives the lane the shots' sound files, the dissolve marked as a crossfade" (516, 517, 526)
#[test]
fn a_lane_naming_a_picture_lane_gets_its_shots_sound_and_the_dissolve_marked() {
    let main = picture(TWO);
    let d = doc(&[
        ("nodes/main.k7x", &main),
        ("nodes/dialog.k7x", &dialog("video_source(\"main\")\n")),
    ]);
    let t = track(&d, "dialog").expect("a track");
    let got: Vec<(&str, Option<bool>, Option<bool>)> = t
        .clips
        .iter()
        .map(|c| {
            let x = c.crossfade.as_ref();
            (c.source.as_str(), x.and_then(|m| m.out), x.and_then(|m| m.in_))
        })
        .collect();
    assert_eq!(got, [(WAV_A, Some(true), None), (WAV_B, None, Some(true))]);
}

// videoSound.test.ts "refuses a picture lane that is not there, and events beside the link" (516): stopped on the lane's file
#[test]
fn a_picture_lane_that_is_not_there_and_events_beside_the_link_stop_the_compile() {
    let main = picture(TWO);
    let linked = dialog("video_source(\"main\")\n");
    assert!(
        run(&[("nodes/main.k7x", &main), ("nodes/dialog.k7x", &linked)])
            .engine
            .is_ok()
    );
    let absent = dialog("video_source(\"nope\")\n");
    let c = run(&[("nodes/main.k7x", &main), ("nodes/dialog.k7x", &absent)]);
    assert_eq!(stopped(&c), ["nodes/dialog.k7x"]);
    let both = dialog("video_source(\"main\")\nevents([{ ref: \"a\", start: \"1:1\" }])\n");
    let c = run(&[("nodes/main.k7x", &main), ("nodes/dialog.k7x", &both)]);
    assert_eq!(stopped(&c), ["nodes/dialog.k7x"]);
}

// videoSound.test.ts "refuses a sound file that is gone, as a missing audio clip is refused" (513): stopped on clips/b.k7x
#[test]
fn a_sound_file_that_is_gone_stops_the_compile_on_its_clip() {
    let main = picture(TWO);
    let lane = dialog("video_source(\"main\")\n");
    let files = |gone: bool| -> Compiled {
        let mut extra = vec![("nodes/main.k7x", main.as_str()), ("nodes/dialog.k7x", lane.as_str())];
        if gone {
            extra.push(("clips/b.k7x", "source(\"clips/b.mp4\")\nsound(\"clips/gone.wav\")\n"));
        }
        run(&extra)
    };
    assert!(files(false).engine.is_ok());
    assert_eq!(stopped(&files(true)), ["clips/b.k7x"]);
}

const ONE: &str = "[{ ref: \"a\", start: 1, length: 2 }]";

// pictureLaneSound.test.ts "plays its shots' sound on `main#audio`, straight to the mix when nothing is wired" (517, 509)
#[test]
fn a_picture_lanes_shots_sound_on_its_own_audio_track_and_the_lane_makes_no_track() {
    let main = picture(ONE);
    let d = doc(&[("nodes/main.k7x", &main)]);
    let t = track(&d, "main#audio").expect("the sound's track");
    assert_eq!(t.content_type, "audio");
    let got: Vec<(&str, f64, f64)> = t.clips.iter().map(|c| (c.source.as_str(), c.start, c.length)).collect();
    assert_eq!(got, [(WAV_A, 2.0, 4.0)]);
    assert!(track(&d, "main").is_none());
}

// pictureLaneSound.test.ts "plays through the chain leaving `main#audio` when one is wired" (333, 517)
#[test]
fn the_chain_leaving_the_audio_half_plays_the_sound() {
    let main = picture("[{ ref: \"a\", start: 0, length: 2 }]");
    let graph = "modules({ main: [\"RENDER\"], \"main#audio\": [\"MIX\"], MIX: [\"OUT\"], RENDER: [\"VIDEO_OUT\"] })\n";
    let d = doc(&[("nodes/main.k7x", &main), ("graph.k7x", graph)]);
    let sounding: Vec<&str> = d
        .tracks
        .iter()
        .filter(|t| t.clips.iter().any(|c| c.source.ends_with("a.wav")))
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(sounding, ["main#audio"]);
}

// pictureLaneSound.test.ts "makes no track for a lane whose shots are silent" (511), a sounding control shot beside the silent one
#[test]
fn a_silent_shot_makes_no_clip_and_a_lane_of_silent_shots_no_track() {
    let both = picture("[{ ref: \"still\", start: 0, length: 2 }, { ref: \"a\", start: 2, length: 2 }]");
    let t = doc(&[("nodes/main.k7x", &both)])
        .tracks
        .into_iter()
        .find(|t| t.id == "main#audio");
    let got: Vec<(&str, f64)> = t
        .iter()
        .flat_map(|t| &t.clips)
        .map(|c| (c.source.as_str(), c.start))
        .collect();
    assert_eq!(got, [(WAV_A, 4.0)]);
    let silent = picture("[{ ref: \"still\", start: 0, length: 2 }]");
    assert!(track(&doc(&[("nodes/main.k7x", &silent)]), "main#audio").is_none());
}

// pictureLaneSound.test.ts "is muted and soloed apart from the picture: hiding the shots keeps their sound" (333, 517)
#[test]
fn the_sound_is_muted_apart_from_the_picture() {
    let main = picture("[{ ref: \"a\", start: 0, length: 2 }]");
    let mute = |lane: &str| -> bool {
        let graph = format!("modules({{}})\nbypassed([\"{lane}\"])\n");
        let d = doc(&[("nodes/main.k7x", &main), ("graph.k7x", &graph)]);
        track(&d, "main#audio").expect("the sound's track").mute
    };
    assert!(!mute("main"));
    assert!(mute("main#audio"));
}

// pictureLaneSound.test.ts "resamples under a speed like tape, or stretches to keep the pitch when the shot says so" (509)
#[test]
fn a_speed_resamples_like_tape_unless_the_shot_keeps_its_pitch() {
    let main = picture(
        "[{ ref: \"a\", start: 0, length: 2, speed: 0.5 }, { ref: \"a\", start: 2, length: 2, speed: 0.5, pitch: \"keep\" }]",
    );
    let d = doc(&[("nodes/main.k7x", &main)]);
    let t = track(&d, "main#audio").expect("the sound's track");
    let got: Vec<Option<&str>> = t
        .clips
        .iter()
        .map(|c| c.warp.as_ref().map(|w| w.algorithm.as_str()))
        .collect();
    assert_eq!(got, [Some("varispeed"), Some("elastique")]);
}

// pictureLaneSound.test.ts "leaves the sound to a lane that still names the picture lane in `video_source`" (517)
#[test]
fn a_lane_naming_the_picture_lane_takes_the_sound_from_the_pictures_own_half() {
    let main = picture("[{ ref: \"a\", start: 0, length: 2 }]");
    let d = doc(&[
        ("nodes/main.k7x", &main),
        ("nodes/dialog.k7x", &dialog("video_source(\"main\")\n")),
    ]);
    assert!(track(&d, "main#audio").is_none());
    assert_eq!(track(&d, "dialog").expect("a track").clips.len(), 1);
}

// 514: a word the Source refuses costs that sound with a WARNING on the clip's file, never the song
#[test]
fn a_sound_outside_the_mounts_is_a_warning_and_the_compile_answers() {
    let main = picture(ONE);
    let c = run(&[
        ("nodes/main.k7x", &main),
        ("clips/a.k7x", "source(\"clips/a.mp4\")\nsound(\"../../../x.wav\")\n"),
    ]);
    let d = c.engine.as_ref().expect("a document");
    assert!(track(&doc(&[("nodes/main.k7x", &main)]), "main#audio").is_some());
    assert!(track(d, "main#audio").is_none());
    let warned: Vec<(&str, Severity)> = c.diagnostics.iter().map(|f| (f.file.as_str(), f.severity)).collect();
    assert_eq!(warned, [("clips/a.k7x", Severity::Warning)]);
}

// 526, 311: the picture errors count toward the same 1,000 as the wiring's, and the wiring list then stops at once
#[test]
fn the_pictures_errors_and_the_wiring_errors_share_one_list() {
    let names: Vec<String> = (0..1_000).map(|i| format!("nodes/v{i}.k7x")).collect();
    let lane = dialog("video_source(\"nope\")\n");
    let unwired = dialog("events([{ ref: \"a\", start: \"1:1\" }])\n");
    let mut extra: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), lane.as_str())).collect();
    extra.push(("nodes/gtr.k7x", &unwired));
    extra.push(("graph.k7x", "modules({ \"gtr#midi\": [\"MIX\"] })\n"));
    let f = run(&extra).engine.expect_err("a stop");
    let on = |file: &str| f.diagnostics.iter().filter(|d| d.file == file).count();
    assert_eq!((on("project.k7x"), on("nodes/gtr.k7x")), (1, 0));
    assert_eq!(
        f.diagnostics.iter().filter(|d| d.file.starts_with("nodes/v")).count(),
        1_000
    );
    // Alone, the unwired event is named.
    extra.retain(|(n, _)| !n.starts_with("nodes/v"));
    let alone = run(&extra).engine.expect_err("a stop");
    assert!(alone.diagnostics.iter().any(|d| d.file == "nodes/gtr.k7x"));
}

fn within<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    let limit = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });
    assert!(t.elapsed() < limit, "{what} took {:?}", t.elapsed());
    out
}

// 523, 304: a picture lane whose sound asks for millions of pieces is one ERROR that stops the compile, named by the lane's file
#[test]
fn a_fan_out_of_pieces_through_the_compile_is_one_error_on_the_lane_and_a_stop() {
    let fades = "{ ref: \"a\", start: 0, length: 20000, fadeIn: 20000 },".repeat(5_000);
    let solid: Vec<String> = (0..5_000)
        .map(|i| format!("{{ ref: \"a\", start: {}, length: 1 }}", 2 * i))
        .collect();
    let main = picture(&format!("[{fades}{}]", solid.join(",")));
    let c = within("fan-out", || run(&[("nodes/main.k7x", &main)]));
    let f = c.engine.as_ref().expect_err("a stop");
    let on_lane = f.diagnostics.iter().filter(|d| d.file == "nodes/main.k7x").count();
    assert_eq!((on_lane, f.diagnostics.len()), (1, 1), "{:?}", f.diagnostics);
}

// 526: a project of 200 lanes of 100 shots each, every lane sounding its own, answers whole and fast
#[test]
fn two_hundred_sounding_lanes_of_a_hundred_shots_compile_whole() {
    let shots: Vec<String> = (0..100)
        .map(|i| format!("{{ ref: \"a\", start: {i}, length: 1 }}"))
        .collect();
    let events = format!("[{}]", shots.join(","));
    let names: Vec<String> = (0..200).map(|i| format!("nodes/p{i}.k7x")).collect();
    let lane = picture(&events);
    let extra: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), lane.as_str())).collect();
    let d = within("200 lanes", || doc(&extra));
    let sounds = d
        .tracks
        .iter()
        .filter(|t| t.id.ends_with("#audio") && t.clips.len() == 100)
        .count();
    assert_eq!(sounds, 200);
}
