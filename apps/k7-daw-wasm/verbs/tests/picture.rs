// picture.rs — the picture document and its scene, read over fixture projects: lanes, runs, lenses, shapes, captions, timelines.
// A reader would get wrong that the scene is tested apart: it is built from the same document, as the preview and an export build it.

mod support;

use serde_json::{Value as Json, json};
use song_edit::Fs;
use song_edit::confine::Env;
use support::{HOME, META, Project, WS};
use verbs::picture::{NO_PICTURE, Use, doc_answer, scene_answer};

fn env() -> Env {
    Env::at(HOME, META)
}

fn rel(p: &str) -> String {
    p.strip_prefix(&format!("{WS}/"))
        .map_or_else(|| p.to_owned(), str::to_owned)
}

fn doc(p: &Project) -> Json {
    doc_answer(&p.fs, &mut song::Cache::new(), &env(), &p.dir, &rel)
}

fn scene(p: &Project, armed: Option<&str>, use_: Use) -> Json {
    scene_answer(&p.fs, &mut song::Cache::new(), &env(), &p.dir, armed, use_, &|f| {
        format!("url:{f}")
    })
}

fn write(p: &mut Project, path: &str, text: &str) {
    p.fs.mkdir_all(path.rsplit_once('/').map_or("", |(d, _)| d))
        .expect("a folder");
    p.fs.write_atomic(path, text.as_bytes()).expect("a write");
}

fn track<'a>(d: &'a Json, id: &str) -> &'a Json {
    d["doc"]["videoTracks"]
        .as_array()
        .and_then(|l| l.iter().find(|t| t["id"] == id))
        .unwrap_or_else(|| panic!("no lane {id}"))
}

#[test]
fn a_video_lane_is_its_one_shot_in_seconds() {
    let p = Project::open("video-lane");
    let d = doc(&p);
    assert_eq!(d["warnings"], json!([]));
    assert_eq!(d["doc"]["viewport"], json!([1280.0, 720.0]));
    assert_eq!(d["doc"]["fps"], json!(30.0));
    assert_eq!(d["doc"]["duration"], json!(6.0));
    assert_eq!(d["doc"]["sound"], Json::Null);
    let main = track(&d, "main");
    assert_eq!(main["clips"][0]["src"], "video-lane/clips/sample.mp4");
    assert_eq!(
        (main["w"].clone(), main["clips"][0]["length"].clone()),
        (json!(1280.0), json!(6.0))
    );
    let s = scene(&p, None, Use::Export);
    assert_eq!(s["mixes"], false);
    assert_eq!(s["scene"]["durationSec"], json!(6.0));
    assert_eq!(
        s["scene"]["layers"][0]["clips"][0]["src"],
        format!("url:{WS}/video-lane/clips/sample.mp4")
    );
    assert_eq!(s["scene"]["layers"][0]["hidden"], false);
}

/// The rig with a transition module in the workspace's shared folder.
fn rig() -> Project {
    let mut p = Project::open_crate("picture-rig");
    let at = format!("{WS}/_modules/video_transitions/wipe");
    write(
        &mut p,
        &format!("{at}/module.k7x"),
        "type(\"video_transition\")\nparams({ P1: 2, P2: 0.25 })\n",
    );
    write(&mut p, &format!("{at}/vertex.glsl"), "// wipe vert\n");
    write(&mut p, &format!("{at}/fragment.glsl"), "// wipe frag\n");
    p
}

#[test]
fn the_rig_folds_its_runs_lenses_shapes_and_captions_onto_its_lanes() {
    let d = doc(&rig());
    let ids: Vec<&str> = d["doc"]["videoTracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["id"].as_str())
        .collect();
    assert_eq!(ids, ["main", "over", "subs"]);
    let main = track(&d, "main");
    assert_eq!(
        (main["camera"].clone(), main["cameraMonitor"].clone()),
        (json!("FaceTime HD"), json!("auto"))
    );
    assert_eq!(main["plugins"][0]["name"], "glow");
    assert!(
        main["plugins"][0]["shader"]["fragment"]
            .as_str()
            .is_some_and(|f| !f.is_empty())
    );
    let clips = main["clips"].as_array().expect("clips");
    assert_eq!(clips[0]["timeline"], "intro");
    assert_eq!(clips[1]["fadeIn"], json!({ "over": 0.5, "with": "wipe" }));
    assert_eq!(
        (clips[1]["speed"].clone(), clips[1]["sourceFrom"].clone()),
        (json!(2.0), json!(1.0))
    );
    assert_eq!(clips[2]["fadeOut"], json!({ "over": 1.0, "with": "dissolve" }));
    assert!(clips[2].get("effect").is_none());
    assert_eq!(clips[1]["proxy"], "picture-rig/clips/film.proxy.mp4");
    let wipe = &d["doc"]["transitions"]["wipe"];
    assert_eq!(
        (wipe["vertex"].clone(), wipe["params"]["P1"].clone()),
        (json!("// wipe vert\n"), json!(1.0))
    );
    assert_eq!(track(&d, "over")["shapes"][0]["type"], "square");
    let subs = &track(&d, "subs")["clips"][0]["captions"];
    assert_eq!(
        subs["cues"],
        json!([{ "start": 0.5, "end": 2.0, "text": "Hello there" }])
    );
    assert_eq!(subs["style"], json!({ "fontSize": 32.0, "position": "top" }));
    assert_eq!(d["doc"]["sound"], "mix");
    assert_eq!(d["doc"]["markers"].as_array().map(Vec::len), Some(2));
    assert_eq!(d["doc"]["timelines"]["intro"][0]["w"], json!(1280.0));
    assert_eq!(
        d["warnings"],
        json!([
            "main: 'ghost' names no clip — add clips/ghost.k7x, or this shot draws nothing",
            "'subs': clips/subs.srt, block 2: no timing line",
            "main: 'film' names effect 'nowhere', which is no run of shaders landing on this lane — it plays as it is",
            "intro>nolane draws on the song's picture lane 'nolane', and there is none — 'picture lane new nolane' adds it"
        ])
    );
}

#[test]
fn the_scene_folds_solo_and_the_lens_and_reads_the_proxy_only_in_the_preview() {
    let p = rig();
    let shut = scene(&p, None, Use::Export);
    assert_eq!(shut["mixes"], true);
    let layers = |s: &Json| s["scene"]["layers"].as_array().cloned().unwrap_or_default();
    let l = layers(&shut);
    assert_eq!(
        (l[0]["hidden"].clone(), l[1]["hidden"].clone(), l[2]["hidden"].clone()),
        (json!(true), json!(true), json!(false))
    );
    assert!(l[0].get("camera").is_none());
    assert_eq!(l[0]["clips"][1]["src"], format!("url:{WS}/picture-rig/clips/film.mp4"));
    assert_eq!(l[0]["passes"][0]["audio"], json!(0.0));
    assert_eq!(shut["scene"]["durationSec"], json!(10.0));
    let armed = scene(&p, Some("main"), Use::Preview);
    let l = layers(&armed);
    assert_eq!(
        (l[0]["hidden"].clone(), l[0]["camera"].clone()),
        (json!(false), json!("FaceTime HD"))
    );
    assert_eq!(
        l[0]["clips"][1]["src"],
        format!("url:{WS}/picture-rig/clips/film.proxy.mp4")
    );
    assert_eq!(armed["scene"]["transitions"]["wipe"]["name"], "wipe");
}

#[test]
fn a_picture_that_does_not_build_is_an_answer() {
    let mut p = Project::open("video-lane");
    let root = format!("{}/project.k7x", p.dir);
    let text = p.text("project.k7x").replace("viewport: [1280, 720, 30],", "");
    write(&mut p, &root, &text);
    assert!(doc(&p)["error"].as_str().is_some_and(|e| e.contains("viewport")));
    let mut q = Project::open("video-lane");
    let lane = format!("{}/nodes/main.k7x", q.dir);
    write(
        &mut q,
        &lane,
        "extends(\"@meta/picture-track\")\nevents([ref(\"sample\").start(0).fadeOut(1)])\n",
    );
    assert!(
        doc(&q)["error"]
            .as_str()
            .is_some_and(|e| e.starts_with("video config invalid:"))
    );
    let none = Project::open("mute-solo");
    assert_eq!(doc(&none)["doc"]["videoTracks"], json!([]));
    assert_eq!(scene(&none, None, Use::Export), json!({ "error": NO_PICTURE }));
}
