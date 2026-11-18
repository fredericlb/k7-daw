//! 1m's M1 against the TypeScript itself: every case of `vectors/shares.json` that a pure function answers (2502).
//! The manifest's `docSizes` and its compile errors belong to M3, which has the compiled documents.

use std::collections::HashMap;
use std::path::Path;

use k7_file_service::shares::names::{
    check_timeline, content_disposition, mix_name, no_such_timeline, timeline_ask, wants_mp3,
};
use k7_file_service::shares::picture::{
    Format, Quality, SYSTEM_CHROMIUM, Size, VideoSettings, find_chromium, job_text,
    resolve_video_settings,
};
use k7_file_service::shares::qs::query_value;
use serde_json::{Value, json};

pub fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/shares.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn env_of(v: &Value) -> impl Fn(&str) -> Option<String> + '_ {
    move |k| v.get(k).and_then(Value::as_str).map(str::to_owned)
}

#[test]
fn shares_the_export_names_headers_and_refusals_are_the_typescripts() {
    let all = vectors();
    let rows = all["export"].as_array().unwrap();
    assert_eq!(rows.len(), 37);
    for c in rows {
        let q = c["query"].as_str().unwrap();
        let what = format!("{q} / {} / {}", c["name"], c["dir"]);
        let timelines = ["verse", "a.b-c_D1"];
        let body_of = |e: String| json!({ "error": e }).to_string();
        let (status, body, name, rendered) = match check_timeline(&timeline_ask(q)) {
            Err(e) => (400, Some(body_of(e)), None, "no"),
            Ok(Some(t)) if !timelines.contains(&t) => {
                (400, Some(body_of(no_such_timeline(t))), None, "no")
            }
            Ok(t) => {
                let n = mix_name(c["name"].as_str().unwrap(), c["dir"].as_str().unwrap(), t);
                (
                    200,
                    None,
                    Some(n),
                    if t.is_some() { "timeline" } else { "song" },
                )
            }
        };
        assert_eq!(c["status"], status, "{what}");
        assert_eq!(c["rendered"], rendered, "{what}");
        let ext = if wants_mp3(q) { "mp3" } else { "wav" };
        match (name, body) {
            (Some(n), _) => {
                let want = content_disposition(&format!("{n}.{ext}"));
                assert_eq!(c["disposition"].as_str().unwrap(), want, "{what}");
            }
            (None, Some(b)) => assert_eq!(c["body"].as_str().unwrap(), b, "{what}"),
            _ => unreachable!(),
        }
    }
}

#[test]
fn shares_a_query_is_read_as_express_reads_it() {
    let all = vectors();
    let rows = all["queries"].as_array().unwrap();
    assert!(rows.len() >= 50, "{}", rows.len());
    for c in rows {
        let q = c["query"].as_str().unwrap();
        let timeline = query_value(q, "timeline").map(|v| v.json());
        assert_eq!(c["timeline"].as_str().map(str::to_owned), timeline, "{q}");
        let format = query_value(q, "format").map_or_else(|| "wav".to_owned(), |v| v.string());
        assert_eq!(c["format"].as_str().unwrap(), format, "{q}");
    }
}

fn settings_of(c: &Value, format: Format, audio: bool) -> VideoSettings {
    let s = &c["settings"];
    let size = match (s["size"]["width"].as_f64(), s["size"]["height"].as_f64()) {
        (Some(w), Some(h)) => Some(Size::Both(w, h)),
        (None, Some(h)) => Some(Size::Height(h)),
        _ => None,
    };
    VideoSettings {
        format,
        audio,
        size,
        quality: s["quality"].as_str().and_then(Quality::parse),
        from: s["from"].as_f64(),
        to: s["to"].as_f64(),
    }
}

fn scene_of(c: &Value) -> ([f64; 2], f64, f64) {
    let v = c["viewport"].as_array().unwrap();
    (
        [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()],
        c["fps"].as_f64().unwrap(),
        c["duration"].as_f64().unwrap(),
    )
}

#[test]
fn shares_the_pictures_range_and_size_are_the_typescripts() {
    for c in vectors()["settings"].as_array().unwrap() {
        let mut c = c.clone();
        c["settings"] = c["s"].clone();
        let (viewport, fps, duration) = scene_of(&c);
        let r = resolve_video_settings(
            &settings_of(&c, Format::Webm, false),
            viewport,
            fps,
            duration,
        );
        let out = &c["out"];
        assert_eq!(out["from"].as_f64(), Some(r.from), "{}", c["s"]);
        assert_eq!(
            out["frameCount"].as_f64(),
            Some(r.frame_count),
            "{}",
            c["s"]
        );
        // JSON writes NaN and the infinities as null: compare by what the job would say.
        let shown = |a: [f64; 2]| format!("{:?}", a.map(|x| x.is_finite().then_some(x)));
        let size = out["size"]
            .as_array()
            .map(|a| [0, 1].map(|i| a[i].as_f64().unwrap_or(f64::INFINITY)));
        assert_eq!(size.map(shown), r.size.map(shown), "{}", c["s"]);
    }
}

#[test]
fn shares_the_jobs_text_is_byte_equal_to_json_stringify() {
    for c in vectors()["job"].as_array().unwrap() {
        let format = if c["format"] == "mp4" {
            Format::Mp4
        } else {
            Format::Webm
        };
        let audio = c["audio"].as_bool().unwrap();
        let s = settings_of(c, format, audio);
        let (viewport, fps, duration) = scene_of(c);
        let r = resolve_video_settings(&s, viewport, fps, duration);
        let got = job_text(c["scene"].as_str().unwrap(), &s, &r, audio);
        assert_eq!(c["job"].as_str().unwrap(), got, "{}", c["settings"]);
    }
}

#[test]
fn shares_the_browser_is_found_where_the_typescript_finds_it() {
    for c in vectors()["chromium"].as_array().unwrap() {
        let files: Vec<&str> = c["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        let dirs: HashMap<&str, Vec<String>> = c["dirs"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str(),
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|n| n.as_str().unwrap().to_owned())
                        .collect(),
                )
            })
            .collect();
        let env = env_of(&c["env"]);
        let exists = |p: &Path| files.contains(&p.to_str().unwrap());
        let list = |p: &Path| dirs.get(p.to_str().unwrap()).cloned();
        let got = match find_chromium(&env, &exists, &list, &SYSTEM_CHROMIUM) {
            Ok(p) => json!({ "ok": p.to_str().unwrap() }),
            Err(e) => json!({ "error": e }),
        };
        assert_eq!(got, c["out"], "{}", c["name"]);
    }
}
