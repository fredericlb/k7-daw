//! goldens.rs — `tests/vectors/libraries.json` replayed: every scenario file-hub's TypeScript ran, its steps and the tree it left.

use std::collections::BTreeMap;
use std::sync::Arc;

use k7_libraries::{Body, Libraries, Request, Store};
use serde_json::{Value, json};

use crate::support::Root;

const SECRET: &str = "the-hubs-secret-for-this-service";

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/libraries.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn store(root: &Root, seed: Option<&str>) -> Arc<Store> {
    Arc::new(Store::new(&root.0.join("data"), seed))
}

fn service(store: Arc<Store>) -> Arc<Libraries> {
    Libraries::new(store, SECRET.into())
}

fn str_of<'a>(s: &'a Value, k: &str) -> &'a str {
    s[k].as_str().unwrap_or_default()
}

fn notes(n: &[k7_libraries::Note]) -> Value {
    Value::Array(n.iter().map(k7_libraries::Note::json).collect())
}

/// One store step, its answer as the generator records it.
fn step(root: &Root, store: &Arc<Store>, s: &Value) -> Value {
    match str_of(s, "op") {
        "settings" => json!(
            store
                .settings()
                .iter()
                .map(|n| json!({ "name": n }))
                .collect::<Vec<_>>()
        ),
        "ensure" => notes(&store.ensure(s["target"].as_str(), s["draft"] == json!(true))),
        "versions" => Value::Array(
            store
                .versions(str_of(s, "name"))
                .iter()
                .map(|m| m.json())
                .collect(),
        ),
        "view" => store.view(&[]),
        "renameLegacy" => json!(store.rename_legacy_default()),
        "write" => {
            std::fs::write(
                root.0.join(str_of(s, "path")),
                root.sub(str_of(s, "content")),
            )
            .unwrap();
            Value::Null
        }
        "mkdir" => {
            std::fs::create_dir_all(root.0.join(str_of(s, "path"))).unwrap();
            Value::Null
        }
        "rm" => {
            let _ = std::fs::remove_dir_all(root.0.join(str_of(s, "path")));
            Value::Null
        }
        "parallel" => Value::Array(std::thread::scope(|sc| {
            let ts: Vec<_> = s["steps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| sc.spawn(|| step_alone(store, x)))
                .collect();
            ts.into_iter().map(|t| t.join().unwrap()).collect()
        })),
        other => panic!("no step {other}"),
    }
}

/// A step that touches no file of the fixture (the ones a parallel pair holds).
fn step_alone(store: &Arc<Store>, s: &Value) -> Value {
    match str_of(s, "op") {
        "ensure" => notes(&store.ensure(s["target"].as_str(), s["draft"] == json!(true))),
        other => panic!("no parallel step {other}"),
    }
}

/// Two answers that came in either order are the same answers.
fn same_set(got: &Value, want: &Value) -> bool {
    let (mut g, mut w) = (
        got.as_array().cloned().unwrap_or_default(),
        want.as_array().cloned().unwrap_or_default(),
    );
    let key = |v: &Value| v.to_string();
    g.sort_by_key(key);
    w.sort_by_key(key);
    g == w
}

fn store_scenario(sc: &Value) -> (Root, Vec<Value>) {
    let root = Root::new();
    root.lay(&sc["tree"]);
    let seed = sc["seed"].as_str().map(|s| root.sub(s));
    let store = store(&root, seed.as_deref());
    let out = sc["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| root.norm(&step(&root, &store, s)))
        .collect();
    (root, out)
}

fn target_of(path: &str) -> (&str, &str) {
    path.split_once('?').unwrap_or((path, ""))
}

async fn route_scenario(sc: &Value) -> (Root, Vec<Value>) {
    let root = Root::new();
    root.lay(&sc["tree"]);
    let seed = sc["seed"].as_str().map(|s| root.sub(s));
    let svc = service(store(&root, seed.as_deref()));
    svc.first_ensure().await;
    let mut out = Vec::new();
    for s in sc["steps"].as_array().unwrap() {
        if str_of(s, "op") != "ask" {
            out.push(root.norm(&step(&root, &svc.store, s)));
            continue;
        }
        let (path, query) = target_of(str_of(s, "path"));
        let body = if s["body"].is_null() {
            b"{}".to_vec()
        } else {
            s["body"].to_string().into_bytes()
        };
        let req = Request {
            method: str_of(s, "method"),
            path,
            query,
            secret: Some(SECRET),
            body: &body,
        };
        let r = svc.answer(&req).await;
        let mut answer = json!({ "status": r.status });
        match r.body {
            Body::Json(v) => answer["body"] = v,
        }
        out.push(root.norm(&answer));
    }
    (root, out)
}

fn slashed(v: &Value) -> Value {
    match v {
        Value::String(s) if cfg!(windows) => {
            // A served file is named by its canonical path, `//?/C:/...` once slashed.
            let s = s.replace('\\', "/");
            Value::String(s.replace("//?/", ""))
        }
        Value::Array(a) => Value::Array(a.iter().map(slashed).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), slashed(v))).collect()),
        v => v.clone(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_scenario_of_the_typescript_replays() {
    let mut ran = 0;
    for sc in vectors() {
        let name = str_of(&sc, "name").to_string();
        let (root, results) = match str_of(&sc, "kind") {
            "store" => store_scenario(&sc),
            "routes" => route_scenario(&sc).await,
            other => panic!("no kind {other}"),
        };
        let want = sc["results"].as_array().unwrap();
        assert_eq!(results.len(), want.len(), "{name}");
        for (i, (got, want)) in results.iter().zip(want).enumerate() {
            // A path in a note is joined with the host's separator, which the vectors spell with `/`.
            let got = &slashed(got);
            let parallel = sc["steps"][i]["op"] == json!("parallel");
            let ok = if parallel {
                same_set(got, want)
            } else {
                got == want
            };
            assert!(
                ok,
                "{name}, step {i} {}: got {got}, want {want}",
                sc["steps"][i]
            );
        }
        let want_tree: BTreeMap<String, String> =
            serde_json::from_value(sc["after"].clone()).unwrap();
        assert_eq!(root.dump(), want_tree, "{name}: the tree left behind");
        ran += 1;
    }
    assert!(ran >= 16, "{ran} scenarios");
}
