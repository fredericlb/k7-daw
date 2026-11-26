// renders.rs — the verbs that wait on the front's routes: a render, a peak, a separation, an envelope, on real fixtures.
// A reader would get wrong what answers a route here: a closure standing in for the backend, the line run again from its files with each answer.

mod support;

use serde_json::{Value as Json, json};
use song_edit::Fs;
use support::{Project, STAMP, wav};
use verbs::{VerbRequest, run_request};

const JOB: &str = "/jobs/j1";

/// What a finished render job answers: the mix in the job's folder.
fn rendered() -> Json {
    json!({ "job": "j1", "dir": JOB, "manifest": { "mix": "mix.wav", "stems": [] }, "files": [{ "out": "mix.wav", "len": 100 }] })
}

/// cold-normalize with k7-core as a library and a two-second render waiting in the job's folder.
fn project() -> Project {
    let mut p = Project::open("cold-normalize").with_core();
    p.job_file(JOB, "mix.wav", &wav(2.0));
    p
}

#[test]
fn a_call_with_no_answer_is_pending_and_lands_nothing() {
    let mut p = project();
    let before = p.files();
    let req = VerbRequest {
        line: "clip process tone drums".to_owned(),
        project: p.dir.clone(),
        env: json!({ "studioHome": "/home-none", "metaRoot": "/k7/meta" }),
        answers: Vec::new(),
        stamp: Some(STAMP.to_owned()),
    };
    let a = run_request(&mut p.fs, &mut || "id".to_owned(), &req);
    let file = format!("{}/clips/tone.wav", p.dir);
    assert_eq!(
        a.pending,
        Some(json!({ "route": "POST /verb-host/peak", "args": { "file": file }, "at": 0 }))
    );
    assert!(a.log.is_empty() && !a.refused);
    assert_eq!(p.files(), before, "a pending run lands nothing");
}

#[test]
fn a_measuring_chain_is_peaked_rendered_and_repointed() {
    let mut p = project();
    let source = format!("{}/clips/tone.wav", p.dir);
    let doc = p.doc();
    let ran = p.ok_answered("clip process tone drums", |c| match c["route"].as_str() {
        Some("POST /verb-host/peak") => json!({ "db": -3.5 }),
        Some("POST /verb-host/render") => {
            let d = &c["args"]["doc"];
            assert_eq!(d["tracks"][0]["id"], "process");
            assert_eq!(d["tracks"][0]["clips"][0]["source"], source);
            assert_eq!(
                d["tracks"][0]["plugins"][0]["params"]["Peak"], -3.5,
                "the measured peak is written in"
            );
            assert_eq!(d["name"], "tone · drums");
            rendered()
        }
        r => panic!("unexpected {r:?}"),
    });
    let routes: Vec<&str> = ran.calls.iter().filter_map(|c| c["route"].as_str()).collect();
    assert_eq!(routes, ["POST /verb-host/peak", "POST /verb-host/render"]);
    let out = format!("clips/tone_drums_{STAMP}.wav");
    assert_eq!(
        ran.answer.log,
        [
            "clip 'tone' through 'drums' — 1 module(s):".to_owned(),
            format!("clips/tone.wav → {out} (clip tone now plays it)")
        ]
    );
    assert_eq!(ran.answer.effects, ["reload"]);
    assert_eq!(p.files()[&format!("{}/{out}", p.dir)], wav(2.0));
    assert!(p.text("clips/tone.k7x").contains(&out));
    assert_eq!(ran.changed(&p.dir), ["clips/tone.k7x", out.as_str()]);
    p.undo(&ran);
    assert_eq!(p.files(), ran.before);
    assert_eq!(p.doc(), doc);
}

#[test]
fn a_failed_render_is_said_per_source_and_writes_nothing() {
    let mut p = project();
    let ran = p.ok_answered("clip process tone drums", |c| match c["route"].as_str() {
        Some("POST /verb-host/peak") => json!({ "db": -1 }),
        _ => json!({ "error": "no engine to render with\nsee the log", "code": "no-engine", "status": 409 }),
    });
    assert_eq!(ran.answer.log, ["clips/tone.wav: no engine to render with"]);
    assert!(ran.answer.effects.is_empty());
    assert_eq!(ran.before, ran.after);
    let unknown = p.run("clip process tone ghost");
    assert!(unknown.answer.log[0].starts_with("clip process: no chain 'ghost' — known here: drums, @k7-core/"));
    p.refused("clip process tone", "usage: clip process");
}

#[test]
fn a_library_chain_bounces_a_whole_lane() {
    let mut p = project();
    let ran = p.ok_answered("track process drums @k7-core/process/normalize", |c| {
        match c["route"].as_str() {
            Some("POST /verb-host/peak") => json!({ "db": -12 }),
            _ => {
                let pl = &c["args"]["doc"]["tracks"][0]["plugins"][0];
                assert_eq!(pl["type"], "DSP Graph");
                assert_eq!(pl["params"], json!({ "Target": -1.0, "Peak": -12.0 }));
                assert!(pl["dspGraph"].as_str().is_some_and(|g| g.contains("nodes")));
                rendered()
            }
        }
    });
    assert_eq!(
        ran.answer.log[0],
        "track 'drums' through '@k7-core/process/normalize' — 1 module(s):"
    );
    assert!(
        p.text("clips/tone.k7x")
            .contains(&format!("tone_normalize_{STAMP}.wav"))
    );
}

#[test]
fn a_cold_chain_separates_beside_the_source() {
    let mut p = project();
    for s in ["drums", "bass", "other", "vocals"] {
        p.job_file("/jobs/j2", &format!("{s}.wav"), &wav(0.5));
    }
    let source = format!("{}/clips/tone.wav", p.dir);
    let ran = p.ok_answered("clip process tone @k7-core/stems", |c| {
        assert_eq!(c["route"], "POST /verb-host/separate");
        assert_eq!(
            c["args"],
            json!({ "source": source, "model": "/home-none/models/htdemucs_fp16weights.onnx", "outputs": ["drums", "bass", "other", "vocals"] })
        );
        let files: Vec<Json> = ["drums", "bass", "other", "vocals"].iter().map(|s| json!({ "out": format!("{s}.wav"), "len": 1 })).collect();
        json!({ "job": "j2", "dir": "/jobs/j2", "files": files })
    });
    assert_eq!(
        ran.answer.log,
        [
            "clip 'tone' through '@k7-core/stems' — 4 file(s):",
            "clips/tone.wav → clips/stems/drums.wav, clips/stems/bass.wav, clips/stems/other.wav, clips/stems/vocals.wav"
        ]
    );
    assert!(p.exists("clips/stems/vocals.wav"));
    let failed = p.ok_answered(
        "clip process tone @k7-core/stems",
        |_| json!({ "error": "the model hung" }),
    );
    assert_eq!(failed.answer.log, ["clip process failed: the model hung"]);
}

#[test]
fn a_trim_reads_the_envelope_and_writes_the_window() {
    let mut p = project();
    let ran = p.ok_answered("event trim drums 1:1:1", |c| {
        assert_eq!(c["route"], "GET /waveform");
        assert_eq!(c["args"], json!({ "clip": "tone@0", "buckets": 2000, "whole": 1 }));
        json!({ "rms": [0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], "seconds": 1.0 })
    });
    assert_eq!(
        ran.answer.log,
        ["drums: tone at 1:1:1 keeps 0.10s → 0.50s of 1.00s (gate -40 dB)"]
    );
    let clip = &p.track("drums").clips[0];
    assert_eq!((clip.source_from, clip.source_to), (Some(0.1), Some(0.5)));
    p.undo(&ran);
    assert_eq!(p.files(), ran.before);
    let none = p.answered("event trim drums 1:1:1", |_| json!({ "error": "no waveform" }));
    assert_eq!(
        none.answer.refuse,
        ["tone: no envelope yet — the engine builds it on first read"]
    );
    let silent = p.answered(
        "event trim drums 1:1:1 -3",
        |_| json!({ "rms": [0.1, 0.1], "seconds": 1.0 }),
    );
    assert!(silent.answer.refuse[0].starts_with("tone: nothing above -3 dB"));
    assert_eq!(silent.before, silent.after);
}

#[test]
fn a_shared_event_gets_its_own_clip_before_its_bounce() {
    let mut p = project();
    let lane = format!("{}/nodes/drums.k7x", p.dir);
    let text = p.text("nodes/drums.k7x").replace(
        "events([ref(\"tone\").start(\"1:1:1\")])",
        "events([ref(\"tone\").start(\"1:1:1\"), ref(\"tone\").start(\"3:1:1\")])",
    );
    p.fs.write_atomic(&lane, text.as_bytes()).unwrap();
    assert_eq!(p.track("drums").clips.len(), 2);
    let answer = |c: &Json| match c["route"].as_str() {
        Some("POST /verb-host/peak") => json!({ "db": -2 }),
        _ => rendered(),
    };
    let ran = p.ok_answered("event process drums 3:1:1 drums", answer);
    assert_eq!(
        ran.answer.log[0],
        "drums at 3:1:1: its own clip 'tone-2', the others keep 'tone'"
    );
    assert!(p.text("clips/tone-2.k7x").contains(&format!("tone_drums_{STAMP}.wav")));
    assert!(
        p.text("clips/tone.k7x").contains("clips/tone.wav"),
        "the other event keeps its material"
    );
    assert!(p.text("nodes/drums.k7x").contains("ref(\"tone-2\")"));
    p.undo(&ran);
    assert_eq!(p.files(), ran.before);
    // A bounce that wrote nothing leaves the event on the clip it had.
    let failed = p.ok_answered("event process drums 3:1:1 drums", |_| json!({ "error": "no engine" }));
    assert_eq!(failed.before, failed.after);
}

#[test]
fn an_audio_range_is_consolidated_into_one_render() {
    let mut p = project();
    let ran = p.ok_answered("selection consolidate drums 1:1:1 2:1", |c| {
        assert_eq!(c["route"], "POST /verb-host/render");
        let t = &c["args"]["doc"]["tracks"][0];
        assert_eq!(t["id"], "cut");
        assert_eq!(t["clips"][0]["id"], "cut-0");
        assert_eq!(t["clips"][0]["start"], 0.0);
        rendered()
    });
    let name = format!("drums_{STAMP}");
    assert_eq!(
        ran.answer.log,
        [
            format!("drums: 1 event(s) consolidated into clips/{name}.k7x"),
            "  2s rendered from tone".to_owned()
        ]
    );
    assert!(
        p.text(&format!("clips/{name}.k7x"))
            .contains(&format!("clips/{name}.wav"))
    );
    let clips = p.track("drums").clips;
    assert_eq!(clips.len(), 1);
    assert!(clips[0].source.ends_with(&format!("{name}.wav")));
    p.undo(&ran);
    assert_eq!(p.files(), ran.before);
}

#[test]
fn a_live_node_is_bounced_then_frozen() {
    let mut p = project();
    let ran = p.ok_answered("node freeze norm", |c| match c["route"].as_str() {
        Some("POST /verb-host/engine") => {
            assert_eq!(c["args"], json!({ "op": "transport", "action": "stop" }));
            json!({})
        }
        Some("POST /verb-host/render") => {
            let tracks = c["args"]["doc"]["tracks"].as_array().unwrap().len();
            assert_eq!(tracks, 1, "the cut keeps the lane upstream of norm");
            rendered()
        }
        r => panic!("unexpected {r:?}"),
    });
    assert_eq!(
        ran.answer.log,
        ["norm: frozen to renders/norm.wav (1 node upstream baked in) — everything downstream stays live"]
    );
    assert!(p.exists("renders/norm.wav"));
    assert!(p.text("nodes/norm.k7x").contains("frozen(\"renders/norm.wav\")"));
    p.undo(&ran);
    assert_eq!(p.files(), ran.before);
    let failed = p.answered("node freeze norm", |c| match c["route"].as_str() {
        Some("POST /verb-host/engine") => json!({}),
        _ => json!({ "error": "render timed out" }),
    });
    assert_eq!(failed.answer.refuse, ["freeze failed: render timed out"]);
    assert_eq!(failed.before, failed.after);
    p.refused("node freeze ghost", "freeze: no module \"ghost\" in the chains");
}

// A call whose args moved since the answer was given is asked again, and the answers after it are dropped with it.
#[test]
fn a_call_that_moved_drops_the_answers_after_it() {
    let mut h_fs = song_edit::MemFs::from_tree("", Vec::new());
    let mut ids = || "id".to_owned();
    let mut h = verbs::Host::new(
        &mut h_fs,
        song_edit::confine::Env::at("/home", "/meta"),
        "/w/p",
        &mut ids,
    );
    h.answers = vec![
        json!({ "route": "GET /a", "args": { "n": 1 }, "value": { "ok": 1 } }),
        json!({ "route": "GET /b", "args": {}, "value": { "ok": 2 } }),
    ];
    assert_eq!(h.call("GET /a", json!({ "n": 1 })), Ok(Ok(json!({ "ok": 1 }))));
    assert!(h.call("GET /b", json!({ "moved": true })).is_err());
    assert_eq!(
        h.pending,
        Some(json!({ "route": "GET /b", "args": { "moved": true }, "at": 1 }))
    );
}
