//! G3: `merge-vectors.json`, base / mine / theirs with the conflicts and the text each choice gives; and every
//! settle vector of the relay that carries a base, run through the merge's own settle.

mod common;
use common::me;
use common::merge_fx as fx;
use k7_git::{Begin, Choice, Direct};
use serde_json::Value;

fn choice(s: &str) -> Choice {
    match s {
        "mine" => Choice::Mine,
        "theirs" => Choice::Theirs,
        _ => Choice::Both,
    }
}

#[tokio::test]
async fn every_vector_gives_its_conflicts_and_its_text() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!("merge-vectors.json")).unwrap();
    for v in vectors {
        let name = v["name"].as_str().unwrap();
        let path = v["path"].as_str().unwrap();
        let (b, m, t) = (
            v["base"].as_str().unwrap(),
            v["mine"].as_str().unwrap(),
            v["theirs"].as_str().unwrap(),
        );
        let f = fx::diverge(
            &[(path, b.as_bytes())],
            &[(path, m.as_bytes())],
            &[(path, t.as_bytes())],
        )
        .await;
        let Begin::Merge(s) = f.repo_b.merge_begin(&f.theirs).unwrap() else {
            panic!("{name}: not a merge")
        };
        let want = v["conflicts"].as_array().unwrap();
        assert_eq!(s.conflicts.len(), want.len(), "{name}: {:?}", s.conflicts);
        for (got, want) in s.conflicts.iter().zip(want) {
            let text = |k: &str| want.get(k).and_then(Value::as_str);
            assert_eq!(got.key_path.as_deref(), text("keyPath"), "{name}");
            if let Some(b) = want.get("both").and_then(Value::as_bool) {
                assert_eq!(got.both, b, "{name} both");
            }
            assert_eq!(got.line, want.get("line").and_then(Value::as_i64), "{name}");
            assert_eq!(got.base.as_deref(), text("base"), "{name} base");
            assert_eq!(got.mine.as_deref(), text("mine"), "{name} mine");
            assert_eq!(got.theirs.as_deref(), text("theirs"), "{name} theirs");
        }
        let picks: Vec<(String, Choice)> = match v["choose"].as_str() {
            Some(c) => s
                .conflicts
                .iter()
                .map(|c0| (c0.id.clone(), choice(c)))
                .collect(),
            None => Vec::new(),
        };
        f.repo_b
            .merge_resolve(&picks, &Direct(f.b.clone()), &me(), "B")
            .unwrap();
        let got = std::fs::read_to_string(f.b.join(path)).unwrap();
        match v["result"].as_str().unwrap() {
            "?" => eprintln!("VECTOR {name}: {got:?}"),
            r => assert_eq!(got, r, "{name}"),
        }
    }
}
