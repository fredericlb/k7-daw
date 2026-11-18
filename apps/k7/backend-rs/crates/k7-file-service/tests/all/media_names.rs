//! 1l's names (2444, 2451, 2460) and the route's clamps (2427) replayed from the TypeScript's own answers.

use k7_file_service::media::names::{
    Take, inside_dir, model_cache_path, models_root, outside_sentence, prefetch_signature,
    take_name,
};
use k7_file_service::media::{clamp_buckets, clamp_sensitivity, is_whole};
use k7_file_service::paths::resolve_all;
use serde_json::Value;

use crate::support::media::{f, section};

#[test]
fn replays_where_every_model_lands() {
    let rows = section("models");
    assert!(rows.len() >= 200, "{} rows", rows.len());
    for row in rows {
        let file = row["file"].as_str().unwrap();
        let models = row["models"].as_str().unwrap();
        let root = match row.get("home") {
            Some(home) => models_root(None, &resolve_all(&[home.as_str().unwrap()])),
            None => models_root(Some(models), ""),
        };
        let want = row["out"].as_str();
        assert_eq!(
            model_cache_path(&root, file).as_deref(),
            want,
            "{models:?} {:?} {file:?}",
            row.get("home")
        );
    }
}

#[test]
fn a_model_root_is_the_environment_or_the_homes_models_folder() {
    assert_eq!(models_root(Some("/srv/m/"), "/home/u/.k7"), "/srv/m");
    assert_eq!(models_root(Some(""), "/home/u/.k7"), "/home/u/.k7/models");
    assert_eq!(models_root(None, "C:/Users/u/.k7"), "C:/Users/u/.k7/models");
}

#[test]
fn replays_every_folder_test() {
    let rows = section("inside");
    assert!(rows.len() >= 190);
    for row in rows {
        let (dir, file) = (row["dir"].as_str().unwrap(), row["file"].as_str().unwrap());
        assert_eq!(
            inside_dir(dir, file),
            row["out"].as_bool().unwrap(),
            "{dir:?} {file:?}"
        );
    }
}

/// What the disk decides is the front's; here a name either lands somewhere named or is refused with the sentence.
#[test]
fn replays_every_take_name() {
    for row in section("takes") {
        let dir = row["dir"].as_str().unwrap().replace("$REC", "/rec");
        let file = row["file"].as_str().unwrap().replace("$REC", "/rec");
        let out = &row["out"];
        let sentence = outside_sentence(&file);
        let said = out
            .get("error")
            .and_then(Value::as_str)
            .map(|e| e.replace("$REC", "/rec"));
        match take_name(&dir, &file) {
            Take::Here(p) => assert_eq!(
                out["path"].as_str().map(|s| s.replace("$REC", "/rec")),
                Some(p),
                "{file}"
            ),
            Take::Refused => {
                assert_eq!(said.as_deref(), Some(sentence.as_str()), "{dir:?} {file:?}")
            }
            Take::Land(p) => match said {
                None => assert_eq!(
                    out["path"].as_str().map(|s| s.replace("$REC", "/rec")),
                    Some(p),
                    "{file}"
                ),
                Some(e) => assert_eq!(e, sentence, "{file}: a name that never landed"),
            },
        }
    }
}

#[test]
fn a_mirrors_take_keeps_only_its_name() {
    assert_eq!(
        take_name("/rec", "C:\\Users\\x\\mirror\\take.wav"),
        Take::Land("/rec/take.wav".into())
    );
    assert_eq!(
        take_name("/rec", "/rec/sub/../take.wav"),
        Take::Here("/rec/sub/../take.wav".into())
    );
    assert_eq!(take_name("/rec", "/m/.."), Take::Refused);
    assert_eq!(take_name("", "/m/take.wav"), Take::Refused);
}

#[test]
fn the_prefetch_signature_is_the_address_and_the_sorted_files() {
    let files = vec!["/m/a.wav".to_string(), "/m/b.wav".to_string()];
    assert_eq!(
        prefetch_signature("http://j", &files),
        "http://j\n/m/a.wav\n/m/b.wav"
    );
    assert_eq!(prefetch_signature("http://j", &[]), "http://j\n");
}

fn query(q: &str) -> Vec<(String, String)> {
    let decode = |s: &str| {
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 2 < b.len() {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    };
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

/// A query value as Express hands it: the one value, or none when the key repeats (an array reads as `NaN` or `false`).
fn single(pairs: &[(String, String)], key: &str) -> Option<String> {
    let all: Vec<&String> = pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v)
        .collect();
    (all.len() == 1).then(|| all[0].clone())
}

#[test]
fn replays_every_clamp_of_the_waveform_route() {
    let rows = section("clamps");
    assert!(rows.len() >= 60);
    for row in rows {
        let q = row["query"].as_str().unwrap();
        let pairs = query(q);
        let clip: Vec<&str> = pairs
            .iter()
            .filter(|(k, _)| k == "clip")
            .map(|(_, v)| v.as_str())
            .collect();
        let asked = &row["asked"];
        if clip.join(",").is_empty() {
            assert!(asked.is_null(), "{q}: no clip, no ask");
            continue;
        }
        assert_eq!(asked["clip"].as_str().unwrap(), clip.join(","), "{q}");
        let b = single(&pairs, "buckets");
        // The TypeScript passes the clamped number on; a whole count is cut from it after the clamp (2427).
        assert_eq!(
            clamp_buckets(b.as_deref()) as f64,
            f(&asked["buckets"]).trunc(),
            "{q}: buckets"
        );
        assert_eq!(
            is_whole(single(&pairs, "whole").as_deref()),
            asked["whole"].as_bool().unwrap(),
            "{q}: whole"
        );
        let s = clamp_sensitivity(single(&pairs, "sensitivity").as_deref());
        match (s, &asked["sensitivity"]) {
            (None, Value::Null) => {}
            (Some(x), v) if !v.is_null() => assert_eq!(x, f(v), "{q}: sensitivity"),
            (got, want) => panic!("{q}: sensitivity {got:?} against {want}"),
        }
    }
}

#[test]
fn a_fractional_bucket_count_is_cut_after_the_clamp() {
    assert_eq!(clamp_buckets(Some("16.5")), 16);
    assert_eq!(clamp_buckets(Some("999.9")), 999);
    assert_eq!(clamp_buckets(Some("3999.99")), 3999);
    assert_eq!(clamp_buckets(Some("4000.5")), 4000);
    assert_eq!(clamp_buckets(Some("0.5")), 16);
    assert_eq!(clamp_buckets(Some("0")), 1000);
    assert_eq!(clamp_buckets(Some("1.5")), 16);
}
