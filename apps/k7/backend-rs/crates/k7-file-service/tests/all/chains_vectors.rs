//! The chains' answers against the TypeScript's own functions (1p-D5): every home of `vectors/chains.json`, built on disk,
//! its walk, three lists, settings and cut compared.

use k7_file_service::chains::{
    ChainSettings, ColdRun, cold_chains, cold_chains_view, frozen_chains, live_chains,
    resolve_cold_run,
};
use k7_file_service::rig::chain_settings;
use serde_json::{Value, json};

use crate::support::chains::World;

fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/chains.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The home as the vector spells it, with `<base>` and `<repo>` for this machine.
fn spelled(home: &Value, base: &str, repo: &str) -> Value {
    // Substituted into JSON text, so a backslash is escaped.
    let json = |s: &str| {
        serde_json::to_string(s)
            .unwrap()
            .trim_matches('"')
            .to_string()
    };
    let text = home
        .to_string()
        .replace("<base>", &json(base))
        .replace("<repo>", &json(repo));
    serde_json::from_str(&text).unwrap()
}

/// Numbers compared as JavaScript holds them.
pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

pub fn entries_of(home: &Value) -> Vec<(String, String)> {
    home["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["p"].as_str().unwrap().to_owned(),
                e["t"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

/// A parse complaint is worded by each side's own parser: only that a chain does not read is compared.
fn soften(v: Value) -> Value {
    match v {
        Value::String(t) if t.starts_with("it does not read — ") => json!("it does not read"),
        Value::Array(a) => Value::Array(a.into_iter().map(soften).collect()),
        Value::Object(m) => Value::Object(
            m.into_iter()
                .map(|(k, v)| {
                    let v = if k == "unreadable" {
                        json!("-")
                    } else {
                        soften(v)
                    };
                    (k, v)
                })
                .collect(),
        ),
        other => other,
    }
}

#[test]
fn every_home_answers_as_the_typescript_did() {
    let all = vectors();
    let (mut bypassed, mut runs, mut declined) = (0, 0, 0);
    for (i, raw) in all["homes"].as_array().unwrap().iter().enumerate() {
        let owned = entries_of(raw);
        let entries: Vec<(&str, &str)> = owned
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect();
        let w = World::new("chains-vector", &entries);
        let base = w.s.0.to_string_lossy().replace('\\', "/");
        let home = spelled(raw, &base, &w.repo);
        let a = &home["answers"];
        let c = w.chains();
        let tag = format!("home {i}: {}", raw["name"].as_str().unwrap());
        let refs: Vec<&str> = a["dirs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["ref"].as_str().unwrap())
            .collect();
        assert_eq!(c.refs(), refs, "{tag}: the walk");
        let (live, cold, frozen) = (live_chains(&c), cold_chains(&c), frozen_chains(&c));
        if a["live"].get("error").is_some() {
            assert!(
                live.is_err() && cold.is_err() && frozen.is_err(),
                "{tag}: declined whole"
            );
            declined += 1;
        } else {
            let got = live.unwrap_or_else(|d| panic!("{tag}: live {d:?}"));
            assert!(
                same(
                    &soften(json!({ "chains": a["live"] })),
                    &soften(got.clone())
                ),
                "{tag}: live {got}"
            );
            let got = cold.unwrap_or_else(|d| panic!("{tag}: cold {d:?}"));
            assert!(same(&a["cold"], &got), "{tag}: cold {got}");
            let got = frozen.unwrap_or_else(|d| panic!("{tag}: frozen {d:?}"));
            bypassed += got["chains"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|c| c["steps"].as_array().unwrap())
                .filter(|s| s.get("bypass").is_some())
                .count();
            assert!(
                same(
                    &soften(json!({ "chains": a["frozen"] })),
                    &soften(got.clone())
                ),
                "{tag}: frozen {got}"
            );
        }
        let prefs = chain_settings(&w.home).expect("settings");
        let want = (
            a["settings"]["stemsChain"].as_str(),
            a["settings"]["instrumentalChain"].as_str(),
        );
        assert_eq!(
            (prefs.0.as_deref(), prefs.1.as_deref()),
            want,
            "{tag}: settings"
        );
        let settings = ChainSettings {
            stems: prefs.0,
            instrumental: prefs.1,
        };
        if let Ok(view) = cold_chains_view(&c, &settings) {
            assert_eq!(view["stemsChain"], a["settings"]["stemsChain"], "{tag}");
        }
        for r in a["runs"].as_array().unwrap() {
            let (want, chain) = (r["want"].as_str().unwrap(), r["chain"].as_str().unwrap());
            let got = resolve_cold_run(&c, &settings, want, chain);
            let t = &r["answer"];
            let at = format!("{tag}: run {want:?} {chain:?}");
            match got {
                Ok(ColdRun::Ok { model, outputs }) => {
                    assert!(
                        same(
                            &json!({ "ok": true, "model": model, "outputs": outputs }),
                            t
                        ),
                        "{at}: {t}"
                    );
                }
                Ok(ColdRun::Refused { status, error }) => {
                    assert!(
                        same(&json!({ "ok": false, "status": status, "error": error }), t),
                        "{at}: {t}"
                    );
                }
                // A chain with a step that is not offline, or one the TypeScript threw on, is declined.
                Err(_) => {
                    let e = t["error"].as_str().unwrap_or("");
                    let known = ["does not separate", "both separates", "cannot load"];
                    assert!(
                        known.iter().any(|k| e.contains(k)) || t.get("error").is_some(),
                        "{at}: declined, TypeScript says {t}"
                    );
                    declined += 1;
                }
            }
            runs += 1;
        }
    }
    assert!(
        runs > 40 && bypassed > 0,
        "{runs} runs, {bypassed} bypassed steps, {declined} declined"
    );
}
