// clips_audio_def.rs — `materialRefOf` and `loadAudioClipDoc` against the TypeScript's own answers (2304, clips_audio_def.json).
// A reader would get wrong that the vectors' paths are spelled `{base}` for the scenario's folder, and that each case gets a folder of its own.

use crate::support::Tmp;
use serde_json::Value;
use song::DiskSource;
use song::PathRule;
use song::clips::audio_def::{ClipGrid, audio_clip_def, material_ref_of};

fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/all/clips_audio_def.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn material_ref_of_answers_every_vector_of_the_typescript() {
    let all = vectors();
    let rows = all["material_ref"].as_array().unwrap();
    assert!(rows.len() >= 25);
    for r in rows {
        let (id, want) = (r["id"].as_str().unwrap(), r["out"].as_str().unwrap());
        assert_eq!(material_ref_of(id), want, "{id:?}");
    }
}

#[test]
fn a_placement_id_of_hostile_length_is_cut_once_and_fast() {
    // A quadratic scan over 200,000 `@` would show against the 2,000 of the vector.
    let id = "@".repeat(200_000);
    assert_eq!(material_ref_of(&id), id);
    let id = format!("{}@{}", "a".repeat(100_000), "1".repeat(300));
    assert_eq!(material_ref_of(&id).len(), 100_000);
}

fn run(case: &Value) -> (Option<Value>, Option<String>) {
    let tmp = Tmp::new("audiodef");
    let base = tmp.at("").trim_end_matches('/').to_owned();
    let project = tmp.at("ws/p.k7");
    tmp.mkdir("ws/p.k7");
    if case["folder"].as_bool().unwrap() {
        tmp.write(
            "ws/p.k7/project.k7x",
            "project({ name: \"p\", tempo: 110, sampleRate: 48000, timeSignature: \"4/4\", video: false, layers: [] })\nmarkers({})\n",
        );
    }
    for f in case["files"].as_array().unwrap() {
        tmp.write(
            &format!("ws/p.k7/{}", f["p"].as_str().unwrap()),
            f["t"].as_str().unwrap(),
        );
    }
    let grid = case["grid"].as_object().map(|g| ClipGrid {
        tempo: g["tempo"].as_f64().unwrap(),
        num: g["num"].as_f64().unwrap(),
        den: g["den"].as_f64().unwrap(),
    });
    let source = DiskSource::new(&[tmp.at("ws")]).unwrap();
    match audio_clip_def(&source, &project, case["id"].as_str().unwrap(), grid, PathRule::HOST) {
        Ok(None) => (None, None),
        Ok(Some(d)) => {
            let n = |v: Option<f64>| v.map_or(Value::Null, Value::from);
            let out = serde_json::json!({ "id": d.id, "source": d.source.replace(&base, "{base}"), "source_from": n(d.source_from), "source_to": n(d.source_to) });
            (Some(out), None)
        }
        Err(e) => (None, Some(e)),
    }
}

#[test]
fn audio_clip_def_answers_every_vector_of_the_typescript() {
    let all = vectors();
    let rows = all["audio_def"].as_array().unwrap();
    assert!(rows.len() >= 20);
    for r in rows {
        let (got, err) = run(r);
        let name = r["name"].as_str().unwrap();
        match &r["out"] {
            Value::Null => assert!(got.is_none() && err.is_none(), "{name}: {got:?} {err:?}"),
            Value::Object(o) if o.contains_key("error") => {
                assert_eq!(err.as_deref(), o["error"].as_str(), "{name}");
            }
            want => {
                assert_eq!(err, None, "{name}");
                let got = got.expect(name);
                for k in ["id", "source"] {
                    assert_eq!(got[k], want[k], "{name} {k}");
                }
                // The TypeScript prints 4 as 4 and the port 4.0; compare as numbers.
                for k in ["source_from", "source_to"] {
                    assert_eq!(got[k].as_f64(), want[k].as_f64(), "{name} {k}");
                }
            }
        }
    }
}

#[test]
fn a_broken_file_beside_the_clip_throws_as_the_typescript_does() {
    // The TypeScript reads every clip file of the folder, so any syntax error in it throws, whichever clip was asked for.
    let case = serde_json::json!({
        "folder": true, "id": "kick", "grid": null,
        "files": [{"p": "clips/kick.k7x", "t": "source(\"a.wav\")\n"}, {"p": "clips/zz.k7x", "t": "source(\"a.wav\n"}]
    });
    let (got, err) = run(&case);
    assert!(got.is_none());
    assert_eq!(err.as_deref(), Some("unterminated string (1:8)"));
}
