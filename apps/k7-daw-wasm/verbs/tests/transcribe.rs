// transcribe.rs — `track transcribe`: the film a picture lane places, whisper's answer as subtitles on a picture lane of their own.
// A reader would get wrong whisper here: the route's answer is given by each test, so what is pinned is the file flow and the refusals.

mod support;

use k7x::edit::Value;
use serde_json::{Value as Json, json};
use song_edit::Fs;
use support::Project;

const SRT: &str = "1\n00:00:00,000 --> 00:00:00,900\nhello from the fake whisper\n";

fn write(p: &mut Project, rel: &str, text: &str) {
    let path = format!("{}/{rel}", p.dir);
    p.fs.write_atomic(&path, text.as_bytes()).expect("a write");
}

/// The transcribe job's value: `srt` as whisper wrote it, with the job and folder the backend adds.
fn whisper(srt: Json) -> impl FnMut(&Json) -> Json {
    move |asked| {
        assert_eq!(asked["route"], "POST /verb-host/transcribe");
        assert_eq!(asked["args"], json!({ "video": "/w/ws/video-lane/clips/sample.mp4" }));
        json!({ "srt": srt, "job": "x", "dir": "/jobs/x" })
    }
}

/// The lane `main` with its shot two seconds in, half a second into the film.
fn staged() -> Project {
    let mut p = Project::open("video-lane");
    write(
        &mut p,
        "nodes/main.k7x",
        "extends(\"@meta/picture-track\")\nevents([ref(\"sample\").start(2).length(4).from(0.5)])\nframe([0, 0, 1280, 720])\n",
    );
    p
}

/// Each placement of lane `lane` as `(ref, start, length, from)`.
fn shots(p: &Project, lane: &str) -> Vec<(String, f64, f64, f64)> {
    let num = |v: Option<&Value>| match v {
        Some(Value::Num(n)) => *n,
        _ => 0.0,
    };
    match p.data(&format!("nodes/{lane}.k7x")).get("events") {
        Some(Value::List(l)) => l
            .iter()
            .map(|e| {
                let r = match e.get("ref") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                (r, num(e.get("start")), num(e.get("length")), num(e.get("from")))
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn the_subtitles_land_on_a_new_picture_lane_where_the_shot_is_and_undo_away() {
    let mut p = staged();
    let ran = p.ok_answered("track transcribe main lyrics", whisper(json!(SRT)));
    assert_eq!(
        ran.answer.log,
        [
            "whisper is listening to clips/sample.mp4 — this can take a few minutes",
            "1 subtitles into clips/lyrics.srt, placed on picture lane lyrics under sample"
        ]
    );
    assert_eq!(ran.answer.effects, ["reload"]);
    let changed = ran.changed(&p.dir);
    for f in [
        "clips/lyrics.srt",
        "clips/lyrics.k7x",
        "nodes/lyrics.k7x",
        "project.k7x",
    ] {
        assert!(changed.iter().any(|c| c == f), "{f} not written: {changed:?}");
    }
    assert_eq!(p.text("clips/lyrics.srt"), SRT);
    assert!(matches!(p.data("clips/lyrics.k7x").get("source"), Some(Value::Str(s)) if s == "clips/lyrics.srt"));
    assert_eq!(shots(&p, "lyrics"), [("lyrics".to_owned(), 2.0, 4.0, 0.5)]);
    assert!(support::strs(p.data("project.k7x").get("trackOrder")).contains(&"lyrics".to_owned()));
    p.doc();
    p.undo(&ran);
    assert_eq!(p.files(), ran.before);
}

#[test]
fn a_rerun_replaces_its_own_placement_and_keeps_the_lanes_others() {
    let mut p = staged();
    p.ok_answered("track transcribe main lyrics", whisper(json!(SRT)));
    write(
        &mut p,
        "nodes/lyrics.k7x",
        "extends(\"@meta/picture-track\")\nevents([ref(\"lyrics\").start(9).length(1), ref(\"credits\").start(20).length(3)])\n",
    );
    let two = format!("{SRT}\n2\n00:00:01.000 --> 00:00:02.000\nand again\n");
    let ran = p.ok_answered("track transcribe main lyrics", whisper(json!(two)));
    let refs: Vec<String> = shots(&p, "lyrics").into_iter().map(|s| s.0).collect();
    assert_eq!(refs, ["credits", "lyrics"]);
    assert_eq!(
        ran.answer.log[1..],
        [
            "2 subtitles into clips/lyrics.srt, placed on picture lane lyrics under sample",
            "the lane already existed, so its subtitles were replaced"
        ]
    );
    assert_eq!(p.text("clips/lyrics.srt"), two);
}

#[test]
fn no_speech_or_a_failing_whisper_is_refused_and_writes_nothing() {
    let mut p = staged();
    for srt in [json!(null), json!(""), json!("1\nno timing here\n")] {
        let ran = p.answered("track transcribe main lyrics", whisper(srt));
        assert!(ran.answer.refused);
        assert_eq!(ran.answer.refuse, ["whisper heard no speech in this video"]);
        assert_eq!(ran.before, ran.after);
    }
    let ran = p.answered(
        "track transcribe main lyrics",
        |_| json!({ "error": "whisper is not installed on this machine", "code": "off" }),
    );
    assert_eq!(
        ran.answer.refuse,
        ["track transcribe: whisper is not installed on this machine"]
    );
    assert_eq!(ran.before, ran.after);
}

#[test]
fn the_words_and_the_lanes_are_checked_before_whisper_runs() {
    let mut p = staged();
    p.refused(
        "track transcribe main",
        "usage: track transcribe <track_in> <track_out>",
    );
    p.refused("track transcribe main a b", "usage: track transcribe");
    p.refused("track transcribe bad/x lyrics", "'bad/x' is not a lane name");
    p.refused("track transcribe nope lyrics", "no picture lane \"nope\" in this song");
    p.refused(
        "track transcribe drums lyrics",
        "no picture lane \"drums\" in this song",
    );
    p.refused("track transcribe main bad/x", "'bad/x' is not a track name");
    p.refused(
        "track transcribe main main",
        "the subtitles need a lane of their own — 'main' holds the video",
    );
    p.refused(
        "track transcribe main drums",
        "'drums' is a sound lane — subtitles are drawn on a picture lane",
    );
    p.refused(
        "track transcribe main kick",
        "clip 'kick' is not a subtitle file (clips/kick.k7x) — name another lane",
    );
}

#[test]
fn the_shot_must_be_a_video_of_the_project_on_this_machine() {
    let mut p = staged();
    p.ok("picture lane new e");
    p.refused(
        "track transcribe e lyrics",
        "'e' places no shot — there is no video to transcribe",
    );
    p.ok("picture lane new b");
    p.ok("picture add b kick 0 2");
    p.refused(
        "track transcribe b lyrics",
        "clip 'kick' is not a video (clips/kick.wav) — transcribe listens to picture",
    );
    p.ok("picture lane new g");
    p.ok("picture add g ghost 0 2");
    p.refused(
        "track transcribe g lyrics",
        "clip 'ghost' says no source (clips/ghost.k7x)",
    );
    write(&mut p, "clips/sample.k7x", "source(\"../../elsewhere/sample.mp4\")\n");
    p.refused(
        "track transcribe main lyrics",
        "clip 'sample' names a file outside the project: ../../elsewhere/sample.mp4",
    );
    write(&mut p, "clips/sample.k7x", "source(\"clips/gone.mp4\")\n");
    p.refused(
        "track transcribe main lyrics",
        "the video is not on this machine: clips/gone.mp4",
    );
}
