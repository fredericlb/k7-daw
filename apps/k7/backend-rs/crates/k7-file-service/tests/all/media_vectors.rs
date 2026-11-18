//! 1l's vectors (2403) replayed: the slicing, the grid, the warp lookups, the answer, its signature and the onset corrections,
//! each equal to what the TypeScript itself answered. Numbers are compared as f64; a non-finite one is read from its name.

use k7_file_service::media::answer::{
    Window, answer_sig, material_window, waveform_answer, wf_sig,
};
use k7_file_service::media::onsets::{Edits, resolve_onsets};
use k7_file_service::media::region::{beat_grid, region_waveform};
use k7_file_service::media::warp::{WarpMap, beat_at_source, source_at_beat};
use serde_json::Value;

use crate::support::media::{Env, clip, f, fs, project, same, same_json, section};

fn expect_f64(what: &str, want: &Value, got: f64) {
    assert!(same(f(want), got), "{what}: expected {want}, got {got}");
}

fn expect_list(what: &str, want: &Value, got: &[f64]) {
    let want = fs(want);
    assert_eq!(want.len(), got.len(), "{what}: length");
    for (i, (w, g)) in want.iter().zip(got).enumerate() {
        assert!(same(*w, *g), "{what}[{i}]: expected {w}, got {g}");
    }
}

/// The region and grid rows the section holds, each with its own envelope, clip, project and buckets.
#[test]
fn replays_every_region_and_its_display_probes_and_grid() {
    let rows = section("regions");
    assert!(rows.len() >= 230, "{} regions", rows.len());
    let mut thrown = 0;
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let env = Env::of(&row["env"]);
        let (tempo, num, den) = project(&row["project"]);
        let asked = f(&row["buckets"]);
        // A fractional count is cut to a whole one (2427): the TypeScript's `new Array(16.5)` throws, this answers.
        let r = region_waveform(
            &env.env(),
            &clip(&row["clip"]),
            tempo,
            num,
            den,
            asked.trunc() as usize,
        );
        if row.get("error").is_some() {
            thrown += 1;
            assert!(
                !r.peak.is_empty(),
                "{name}: a thrown row still draws a column"
            );
            continue;
        }
        let out = &row["out"];
        let at = |k: &str| format!("{name} @{asked}: {k}");
        expect_list(&at("peak"), &out["peak"], &r.peak);
        expect_list(&at("rms"), &out["rms"], &r.rms);
        match (&r.bands, out.get("low")) {
            (Some([l, m, h]), Some(_)) => {
                expect_list(&at("low"), &out["low"], l);
                expect_list(&at("mid"), &out["mid"], m);
                expect_list(&at("high"), &out["high"], h);
            }
            (None, None) => {}
            _ => panic!("{}", at("bands present on one side only")),
        }
        expect_f64(&at("sampleRate"), &out["sampleRate"], r.sample_rate());
        expect_f64(&at("seconds"), &out["seconds"], r.seconds);
        expect_f64(&at("startBeat"), &out["startBeat"], r.start_beat);
        expect_f64(&at("clipBeats"), &out["clipBeats"], r.clip_beats);
        expect_f64(&at("barBeats"), &out["barBeats"], r.bar_beats);
        expect_f64(&at("tempo"), &out["tempo"], r.tempo);
        expect_list(&at("transients"), &out["transients"], &r.transients);
        let marks = out["warpMarks"].as_array().unwrap();
        assert_eq!(marks.len(), r.warp_marks.len(), "{}", at("warpMarks"));
        for (w, g) in marks.iter().zip(&r.warp_marks) {
            expect_f64(&at("mark.orig"), &w["orig"], g.orig);
            expect_f64(&at("mark.target"), &w["target"], g.target);
            expect_f64(&at("mark.src"), &w["src"], g.src);
        }
        let s = &out["sourceMap"];
        expect_f64(&at("inPoint"), &s["inPoint"], r.source_map.in_point);
        expect_f64(&at("stretch"), &s["stretch"], r.source_map.stretch);
        expect_f64(&at("zeroBeat"), &s["zeroBeat"], r.source_map.zero_beat);
        for p in row["display"].as_array().unwrap() {
            expect_f64(&at("toDisplay"), &p[1], r.to_display(f(&p[0])));
        }
        let grid = beat_grid(&r).unwrap_or_else(|_| panic!("{}", at("grid refused")));
        let want = row["grid"].as_array().unwrap();
        assert_eq!(want.len(), grid.len(), "{}", at("grid length"));
        for (w, g) in want.iter().zip(&grid) {
            expect_f64(&at("grid.t"), &w["t"], g.t);
            assert_eq!(w["bar"].as_bool().unwrap(), g.bar, "{}", at("grid.bar"));
            assert_eq!(
                w.get("label").and_then(Value::as_str),
                g.label.as_deref(),
                "{}",
                at("grid.label")
            );
        }
    }
    assert_eq!(
        thrown, 6,
        "the rows the TypeScript throws on: three fractional counts and three NaN frame counts"
    );
}

#[test]
fn replays_the_grids_that_are_refused_and_the_ones_just_inside() {
    for row in section("longGrids") {
        let length = f(&row["length"]);
        let mut c = clip(
            &serde_json::json!({ "id": "c", "type": "audio", "name": "c", "source": "/x.wav", "start": f(&row["start"]),
            "length": length, "offset": 0.0, "fadeIn": 0.0, "fadeOut": 0.0, "loopStart": 0.0, "loopEnd": 0.0, "notes": [] }),
        );
        c.length = length;
        let env = [0.1; 4];
        let e = k7_file_service::media::region::Envelope {
            sample_rate: 48000.0,
            frames: 960000.0,
            seconds: 20.0,
            peak: &env,
            rms: &env,
            bands: None,
            transients: &[],
        };
        let r = region_waveform(&e, &c, 120.0, 4.0, 4.0, 16);
        let count = f(&row["count"]) as usize;
        match beat_grid(&r) {
            Err(_) => assert!(count > 100_000, "{length}: refused at {count}"),
            Ok(lines) => {
                assert!(count <= 100_000);
                assert_eq!(lines.len(), count);
                assert!(same(lines[0].t, f(&row["first"]["t"])));
                assert!(same(lines[1].t, f(&row["second"]["t"])));
                assert!(same(lines[count - 1].t, f(&row["last"]["t"])));
                assert_eq!(
                    lines.iter().filter(|l| l.bar).count(),
                    row["bar"].as_u64().unwrap() as usize
                );
            }
        }
    }
}

#[test]
fn replays_every_warp_lookup_walked_and_bisected() {
    let rows = section("warpLookups");
    assert!(rows.len() >= 18);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let markers: Vec<(f64, f64)> = row["markers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (f(&m[0]), f(&m[1])))
            .collect();
        let map = WarpMap::new(&markers);
        for p in row["beatAtSource"].as_array().unwrap() {
            let x = f(&p[0]);
            expect_f64(
                &format!("{name}: beatAtSource({x}) walked"),
                &p[1],
                beat_at_source(&markers, x),
            );
            expect_f64(
                &format!("{name}: beatAtSource({x}) bisected"),
                &p[1],
                map.beat_at_source(x),
            );
        }
        for p in row["sourceAtBeat"].as_array().unwrap() {
            let x = f(&p[0]);
            expect_f64(
                &format!("{name}: sourceAtBeat({x}) walked"),
                &p[1],
                source_at_beat(&markers, x),
            );
            expect_f64(
                &format!("{name}: sourceAtBeat({x}) bisected"),
                &p[1],
                map.source_at_beat(x),
            );
        }
    }
}

fn edits_of(v: &Value) -> Edits {
    let list = |k: &str| v.get(k).map(fs);
    Edits {
        added: list("added"),
        removed: list("removed"),
    }
}

fn window_of(v: &Value) -> Option<Window> {
    (!v.is_null()).then(|| Window {
        from: f(&v["from"]),
        to: f(&v["to"]),
    })
}

#[test]
fn replays_every_answer_its_keys_in_order_and_its_signature() {
    let regions = section("regions");
    let rows = section("answers");
    assert!(rows.len() >= 40, "{} answers", rows.len());
    for row in rows {
        let name = row["name"].as_str().unwrap();
        // A row names its region by index, or carries its own envelope, clip, project and buckets.
        let base = row
            .get("region")
            .map_or(row, |i| &regions[i.as_u64().unwrap() as usize]);
        let env = Env::of(&base["env"]);
        let c = clip(&base["clip"]);
        let p = project(&base["project"]);
        let buckets = f(&base["buckets"]) as usize;
        let edits = edits_of(&row["edits"]);
        let placement = (!row["placement"].is_null()).then(|| clip(&row["placement"]));
        let whole = row["whole"].as_bool().unwrap();
        let window = whole
            .then(|| material_window(placement.as_ref().unwrap_or(&c)))
            .flatten();
        assert_eq!(window, window_of(&row["window"]), "{name}: the window");
        let got = waveform_answer(&env.env(), &edits, &c, p, buckets, window).unwrap();
        same_json(&row["answer"], &got, name).unwrap();
        let keys: Vec<&str> = got
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let want: Vec<&str> = row["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap())
            .collect();
        assert_eq!(keys, want, "{name}: the answer's keys, in order");
        let sig = answer_sig(
            &wf_sig(&env.fingerprint, &c, p, buckets, window),
            env.sensitivity,
            &edits,
        );
        assert_eq!(sig, row["sig"].as_str().unwrap(), "{name}: the memo key");
    }
}

#[test]
fn replays_the_rack_whole_file_answer_without_corrections() {
    for row in section("rack") {
        let env = Env::of(&row["env"]);
        let c = clip(&row["clip"]);
        let (tempo, num, den) = project(&row["project"]);
        let r = region_waveform(&env.env(), &c, tempo, num, den, f(&row["buckets"]) as usize);
        let got = Value::Object(k7_file_service::media::answer::build_waveform(&r, None).unwrap());
        same_json(&row["answer"], &got, "rack").unwrap();
        let keys: Vec<&str> = got
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let want: Vec<&str> = row["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap())
            .collect();
        assert_eq!(keys, want);
    }
}

#[test]
fn replays_every_signature() {
    let rows = section("sigs");
    assert!(rows.len() >= 80);
    for row in rows {
        let got = wf_sig(
            row["fingerprint"].as_str().unwrap(),
            &clip(&row["clip"]),
            project(&row["project"]),
            f(&row["buckets"]) as usize,
            window_of(&row["window"]),
        );
        assert_eq!(got, row["sig"].as_str().unwrap());
    }
}

#[test]
fn replays_every_onset_correction() {
    let rows = section("onsets");
    assert!(rows.len() >= 180, "{} onset cases", rows.len());
    for row in rows {
        let got = resolve_onsets(&fs(&row["detected"]), &edits_of(&row["edits"]));
        expect_list(row["name"].as_str().unwrap(), &row["out"], &got);
    }
}
