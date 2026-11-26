// pictures_media.rs — M6's media verbs: sound, prepare, interchange, import, render, a timeline's copy and the front's routes they call.
// A reader would get wrong `picture render`: its scene is the page's to build, so it is an ask handed back to the page, never a call.

mod support;

use serde_json::{Value as Json, json};
use song_edit::Fs;
use support::{Project, strs};

fn write(p: &mut Project, rel: &str, text: &str) {
    let path = format!("{}/{rel}", p.dir);
    p.fs.write_atomic(&path, text.as_bytes()).expect("a write");
}

fn with_sound(p: &mut Project) {
    write(p, "clips/sample.sound.wav", "RIFF");
}

/// The extract route's answer, its file left in `/jobs/x` (`Project::job_file`).
fn extracted(asked: &Json) -> Json {
    assert_eq!(asked["route"], "POST /verb-host/extract");
    json!({ "facts": { "frames": 96000, "sampleRate": 48000, "channels": 2 }, "files": [{ "out": "sound.wav" }], "job": "x", "dir": "/jobs/x" })
}

#[test]
fn a_picture_sound_extracts_once_and_is_heard_on_the_lanes_own_half() {
    let mut p = Project::open("video-lane");
    p.job_file("/jobs/x", "sound.wav", b"RIFF");
    let ran = p.ok_answered("picture sound main", extracted);
    assert_eq!(
        ran.changed(&p.dir),
        ["clips/sample.k7x", "clips/sample.sound.wav", "project.k7x"]
    );
    assert_eq!(
        ran.answer.log,
        [
            "sample: + clips/sample.sound.wav (2.0s, 2 ch)",
            "main#audio plays main's sound — to the mix, or wherever `main#audio` is wired"
        ]
    );
    assert!(ran.answer.asks.is_empty());
    assert!(p.text("clips/sample.k7x").contains("sound(\"clips/sample.sound.wav\")"));
    assert!(p.text("project.k7x").contains("format(4)"));
    let again = p.ok("picture sound main");
    assert!(again.answer.asks.is_empty());
    assert_eq!(again.answer.log.len(), 1);
    assert!(
        p.doc()
            .tracks
            .iter()
            .any(|t| t.id == "main#audio" && t.clips.len() == 1)
    );
}

#[test]
fn a_film_with_no_sound_stays_silent_and_writes_nothing() {
    let mut p = Project::open("video-lane");
    let ran = p.answered(
        "picture sound main",
        |_| json!({ "facts": null, "files": [], "job": "x", "dir": "/jobs/x" }),
    );
    assert_eq!(
        ran.answer.log[0],
        "sample: clips/sample.mp4 has no sound — the shot stays silent"
    );
    assert!(!p.text("clips/sample.k7x").contains("sound("));
}

#[test]
fn a_clip_that_is_no_video_or_not_there_is_said_not_refused() {
    let mut p = Project::open("video-lane");
    p.ok("picture lane new b");
    p.ok("picture add b kick 0 2");
    p.ok("picture add b ghost 2 2");
    let ran = p.ok("picture sound b");
    assert_eq!(
        ran.answer.log,
        [
            "kick: not a video clip — nothing to hear",
            "ghost: no clips/ghost.k7x — skipped",
            "nothing on b has a sound"
        ]
    );
    assert!(ran.answer.asks.is_empty());
    p.refused("picture sound", "usage: picture sound <lane>");
    p.refused(
        "picture sound verse>main",
        "picture sound: verse>main draws on the song's picture lane 'main'",
    );
}

#[test]
fn a_chain_project_gets_a_lane_that_plays_the_sound() {
    let mut p = Project::open_crate("chain-lanes");
    p.ok("picture lane new vid");
    write(&mut p, "clips/v.k7x", "source(\"clips/v.mp4\")\n");
    p.ok("picture add vid v 0 2");
    p.job_file("/jobs/x", "sound.wav", b"RIFF");
    let ran = p.ok_answered("picture sound vid dialog", extracted);
    assert!(
        ran.answer
            .log
            .iter()
            .any(|l| l.starts_with("+ track dialog plays vid's sound (tracks/dialog.k7x")),
        "{:?}",
        ran.answer.log
    );
    assert!(
        p.text("tracks/dialog.k7x").contains("video_source(\"vid\")"),
        "{}",
        p.text("tracks/dialog.k7x")
    );
    assert!(!p.text("tracks/dialog.k7x").contains("events"));
    let again = p.ok("picture sound vid dialog");
    assert!(
        again
            .answer
            .log
            .contains(&"dialog already plays vid's sound".to_owned()),
        "{:?}",
        again.answer.log
    );
}

#[test]
fn removing_a_clip_of_a_sounding_lane_silences_its_shot() {
    let mut p = Project::open("video-lane");
    p.job_file("/jobs/x", "sound.wav", b"RIFF");
    p.ok_answered("picture sound main", extracted);
    with_sound(&mut p);
    let ran = p.round_trip("event remove main#audio 1:1:1");
    assert_eq!(
        ran.answer.log,
        ["main#audio: sample's sound silenced — the shot stays on main (picture silence main 0 brings it back)"]
    );
    p.refused("event remove main#audio 9:1:1", "no event on 'main#audio' at 9:1:1");
    p.refused(
        "event move main#audio 1:1:1 2:1:1",
        "plays main's sound — edit the shot instead",
    );
    p.ok("event remove main#audio 1:1:1");
    assert!(p.text("nodes/main.k7x").contains("silent(true)"));
    assert!(p.doc().tracks.iter().all(|t| t.clips.is_empty() || t.id == "drums"));
}

#[test]
fn several_removals_at_once_remove_the_sound_lanes_events_and_silence_the_shots() {
    let mut p = Project::open("video-lane");
    p.job_file("/jobs/x", "sound.wav", b"RIFF");
    p.ok_answered("picture sound main", extracted);
    with_sound(&mut p);
    let ran =
        p.round_trip(r#"events remove [{"track":"drums","start":"1:1:1"},{"track":"main#audio","start":"1:1:1"}]"#);
    assert_eq!(
        ran.answer.log,
        [
            "removed 1 event from drums",
            "silenced sample — the shots stay in the picture"
        ]
    );
    let ran = p.ok(r#"events remove [{"track":"main#audio","start":"1:1:1"}]"#);
    assert_eq!(ran.answer.log, ["silenced sample — the shots stay in the picture"]);
    assert!(p.text("nodes/main.k7x").contains("silent(true)"));
}

#[test]
fn a_timeline_copies_a_picture_lane_and_gives_it_back_to_the_song() {
    let mut p = Project::open("video-lane");
    p.ok("timeline new verse");
    let ran = p.round_trip("timeline lane copy main verse");
    assert_eq!(
        ran.answer.log,
        ["verse: + picture lane main, 1 shot(s) of main — it draws on the song's picture lane 'main'"]
    );
    p.ok("timeline lane copy main verse take");
    assert!(format!("{:?}", p.data("timelines/verse.k7x").get("pictures")).contains("take"));
    let ran = p.round_trip("timeline lane copy verse>take main");
    assert_eq!(ran.answer.log.len(), 1);
    assert!(
        ran.answer.log[0].starts_with("+ picture lane take: 1 shot(s) of verse>take (nodes/take.k7x"),
        "{:?}",
        ran.answer.log
    );
    p.ok("timeline lane copy verse>take main");
    assert!(p.text("nodes/take.k7x").contains("sample"));
    assert!(
        !p.text("nodes/take.k7x").contains("frame"),
        "a timeline's lane has no frame to give"
    );
    let ran = p.ok("timeline lane copy main main");
    assert!(
        ran.answer.log[0].starts_with("+ picture lane main-2: 1 shot(s) of main ("),
        "{:?}",
        ran.answer.log
    );
    assert!(p.text("nodes/main-2.k7x").contains("frame([0, 0, 1280, 720])"));
    p.refused("timeline lane copy main nowhere", "no timeline \"nowhere\"");
    p.refused(
        "timeline lane copy main verse take",
        "'take' cannot name a new lane of 'verse'",
    );
}

fn otio(p: &mut Project) -> Json {
    let ran = p.ok("picture interchange otio");
    serde_json::from_str(&ran.answer.log[0]).expect("an otio that reads")
}

#[test]
fn an_otio_names_the_originals_and_an_edl_the_record_timecodes() {
    let mut p = Project::open("video-lane");
    let doc = otio(&mut p);
    let track = &doc["tracks"]["children"][0];
    assert_eq!(
        (doc["name"].as_str(), track["name"].as_str()),
        (Some("video-lane"), Some("main"))
    );
    let clip = &track["children"][0];
    assert_eq!(clip["media_reference"]["target_url"], "../clips/sample.mp4");
    assert_eq!(clip["source_range"]["duration"]["value"], 180);
    assert_eq!(doc["metadata"]["k7"]["fps"], 30);
    let ran = p.ok("picture interchange edl");
    assert_eq!(
        ran.answer.log[0],
        "TITLE: video-lane\nFCM: NON-DROP FRAME\n\n001  AX       V     C        00:00:00:00 00:00:06:00 01:00:00:00 01:00:06:00\n* FROM CLIP NAME: sample.mp4\n\n"
    );
    assert!(ran.before == ran.after, "an interchange writes nothing");
}

#[test]
fn a_dissolve_is_a_transition_in_an_otio_and_a_pair_in_an_edl() {
    let mut p = Project::open("video-lane");
    write(
        &mut p,
        "nodes/main.k7x",
        "extends(\"@meta/picture-track\")\nevents([ref(\"sample\").start(0).length(4), ref(\"sample\").start(4).length(4).from(1).speed(2).fadeIn(1)])\nframe([0, 0, 1280, 720])\n",
    );
    let doc = otio(&mut p);
    let kinds: Vec<&str> = doc["tracks"]["children"][0]["children"]
        .as_array()
        .expect("children")
        .iter()
        .map(|c| c["OTIO_SCHEMA"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(kinds, ["Clip.1", "Transition.1", "Clip.1"]);
    let edl = p.ok("picture interchange edl main").answer.log[0].clone();
    assert!(edl.contains("002  AX       V     D    030 "), "{edl}");
    assert!(edl.contains("M2   AX       0060.0    00:00:01:00"), "{edl}");
    assert!(edl.contains("* TO CLIP NAME: sample.mp4"));
}

#[test]
fn a_shot_open_to_the_end_of_a_source_this_host_cannot_measure_is_left_out_and_said() {
    let mut p = Project::open("video-lane");
    p.ok("picture length main 0 0");
    let ran = p.ok_answered("picture interchange otio", no_ffprobe);
    assert_eq!(ran.answer.log.len(), 2);
    assert_eq!(
        ran.answer.log[1],
        "left out — main: sample: runs to the end of a source whose length is unknown"
    );
    let doc: Json = serde_json::from_str(&ran.answer.log[0]).expect("an otio");
    assert_eq!(doc["tracks"]["children"].as_array().map(Vec::len), Some(0));
}

#[test]
fn an_interchange_names_what_it_cannot_hand_over() {
    let mut p = Project::open("video-lane");
    p.refused("picture interchange", "usage: picture interchange <otio|edl> [lane]");
    p.refused("picture interchange xml", "usage: picture interchange");
    p.refused(
        "picture interchange edl ghost",
        "no picture lane 'ghost' — this song's are main",
    );
    let mut q = Project::open("mute-solo");
    q.refused(
        "picture interchange otio",
        "picture interchange: project.viewport is required",
    );
    p.ok("picture lane delete main");
    p.refused("picture interchange otio", "this song has no picture lane to hand over");
}

#[test]
fn an_exported_cut_imports_as_the_same_shots_on_the_same_clip() {
    let mut p = Project::open("video-lane");
    let cut = p.ok("picture interchange otio").answer.log[0].clone();
    write(&mut p, "cut.otio", &cut);
    let ran = p.round_trip("picture import cut.otio");
    assert_eq!(ran.answer.log, ["+ picture lane import-1: 1 shot(s) from main"]);
    assert_eq!(ran.changed(&p.dir).len(), 4, "{:?}", ran.changed(&p.dir));
    p.ok("picture import cut.otio");
    let lane = p.data("nodes/import-1.k7x");
    let events = format!("{:?}", lane.get("events"));
    assert!(events.contains("\"sample\"") && events.contains("Num(6.0)"), "{events}");
    assert!(
        p.text("nodes/import-1.k7x")
            .starts_with("// imported from cut.otio, track main\n")
    );
    p.ok("picture import cut.otio nova-");
    assert!(p.exists("nodes/nova-1.k7x"));
    assert_eq!(
        strs(p.data("project.k7x").get("trackOrder")).last().map(String::as_str),
        Some("main")
    );
}

#[test]
fn an_edl_names_its_media_by_clip_and_a_missing_one_is_placed_anyway() {
    let mut p = Project::open("mute-solo");
    let edl = "TITLE: t\nFCM: NON-DROP FRAME\n\n001  AX       V     C        00:00:00:00 00:00:04:00 01:00:00:00 01:00:04:00\n* FROM CLIP NAME: Cam A.mov\n\n";
    write(&mut p, "cut.edl", edl);
    let ran = p.round_trip("picture import cut.edl");
    assert_eq!(
        ran.answer.log,
        [
            "project: viewport [1920, 1080, 25] — the cut's rate",
            "+ picture lane import-1: 1 shot(s) from edl",
            "not found, placed anyway — copy into clips/: Cam A.mov"
        ]
    );
    p.ok("picture import cut.edl");
    assert_eq!(
        p.data("clips/Cam-A.k7x").get("source"),
        Some(&k7x::edit::Value::Str("clips/Cam A.mov".to_owned()))
    );
    assert!(p.text("project.k7x").contains("1920, 1080, 25"));
}

#[test]
fn an_import_refuses_what_it_cannot_read_and_writes_nothing() {
    let mut p = Project::open("video-lane");
    p.refused("picture import", "usage: picture import");
    p.refused("picture import none.otio", "no none.otio in this project");
    p.refused("picture import ../x.otio", "no ../x.otio in this project");
    write(&mut p, "x.txt", "no");
    p.refused("picture import x.txt", "x.txt: an .otio or an .edl is what this reads");
    write(&mut p, "bad.otio", "{}");
    p.refused("picture import bad.otio", "bad.otio: not an OpenTimelineIO timeline");
    write(&mut p, "empty.edl", "TITLE: t\n");
    p.refused("picture import empty.edl", "empty.edl holds no picture to import");
}

#[test]
fn a_render_is_asked_for_in_the_background_with_its_words_checked_here() {
    let mut p = Project::open("video-lane");
    let ran = p.ok("picture render mp4 none size=720p quality=high from=1:30 into=/out/film");
    assert_eq!(
        ran.answer.log,
        ["rendering the picture in the background (mp4, silent) — it lands in /out/film"]
    );
    let ask = &ran.answer.asks[0];
    assert_eq!(ask["route"], "POST /verb-host/picture");
    let args = &ask["args"];
    assert_eq!(
        (args["into"].clone(), args["format"].clone(), args["audio"].clone()),
        (json!("/out/film"), json!("mp4"), json!(false))
    );
    assert_eq!(
        args["settings"],
        json!({ "size": { "height": 720 }, "quality": "high", "from": 90.0 })
    );
    assert_eq!(args["scene"]["viewport"], json!([1280.0, 720.0]));
    assert_eq!(
        args["scene"]["layers"][0]["clips"][0]["src"],
        format!("{}/clips/sample.mp4", p.dir)
    );
    assert!(ran.before == ran.after);
    p.ok("picture render into=\"/out/my film\"");
    p.refused("picture render into=/out size=huge", "usage: picture render");
    p.refused("picture render from=9 to=3 into=/out", "usage: picture render");
    p.refused("picture render mp4", "say where the film goes: into=<folder>");
    p.refused("picture render into=relative/dir", "say where the film goes");
    let mut q = Project::open("mute-solo");
    q.refused(
        "picture render into=/out",
        "this project draws no picture — nothing to render",
    );
}

/// The probe route of a machine with no ffprobe.
fn no_ffprobe(asked: &Json) -> Json {
    assert_eq!(asked["route"], "POST /verb-host/probe");
    json!({ "error": "ffprobe is not installed", "code": "no-ffmpeg", "status": 503 })
}

#[test]
fn a_prepare_with_no_ffmpeg_here_says_what_a_browser_cannot_play() {
    let mut p = Project::open("video-lane");
    assert_eq!(
        p.ok_answered("picture prepare main", no_ffprobe).answer.log,
        ["nothing on main to prepare"]
    );
    write(&mut p, "clips/sample.k7x", "source(\"clips/sample.mov\")\n");
    let ran = p.ok_answered("picture prepare main", no_ffprobe);
    assert_eq!(
        ran.answer.log,
        [
            "sample: this video may not play in the browser (an iPhone .mov, say) — convert it to MP4 (H.264), or install ffmpeg so k7 converts it for you"
        ]
    );
    p.refused("picture prepare", "usage: picture prepare <lane>");
    p.refused("picture prepare ghost", "no picture lane 'ghost'");
}

#[test]
fn a_prepare_probes_the_film_and_asks_the_front_for_its_copies() {
    let mut p = Project::open("video-lane");
    write(&mut p, "clips/sample.k7x", "source(\"clips/sample.mov\")\n");
    let mut asked: Vec<Json> = Vec::new();
    let ran = p.ok_answered("picture prepare main", |a| {
        asked.push(a.clone());
        if a["route"] == "POST /verb-host/probe" {
            json!({ "facts": { "codec": "hevc", "width": 3840, "height": 2160, "pixFmt": "yuv420p10le", "fps": 30, "seconds": 4, "container": "mov,mp4,m4a,3gp,3g2,mj2" } })
        } else {
            json!({ "started": true })
        }
    });
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(asked[1]["route"], "POST /verb-host/prepare");
    assert_eq!(asked[1]["args"]["clip"], "sample");
    assert_eq!(asked[1]["args"]["optimized"], "transcode");
    assert_eq!(asked[1]["args"]["proxy"], true);
    assert_eq!(
        ran.answer.log.last().map(String::as_str),
        Some("sample: preparing in the background — the lane picks it up when it is written")
    );
}
