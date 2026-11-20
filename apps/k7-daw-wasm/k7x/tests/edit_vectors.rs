// edit_vectors.rs — replays vectors/edit.json, which the TypeScript editor and printer wrote (scripts/makeEditVectors.mts).
// A failure names its case and op; regenerate with `npx tsx packages/k7x/scripts/makeEditVectors.mts` only when the TypeScript changed.

mod common;

use common::{Json, exec, json_of, parse_json, value_of};
use k7x::edit::{Document, StringifyOptions, Value, stringify};

fn vectors() -> Json {
    parse_json(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/edit.json"
        ))
        .unwrap(),
    )
}

/// One op's recorded result against what the document did.
fn check(case: &str, i: usize, doc: &mut Document, op: &Json, want: &Json) {
    let got = exec(doc, op);
    let at = format!("{case} op {i} {op:?}");
    match (got, want.get("e")) {
        (Err(e), Some(m)) => assert_eq!(e.to_string(), m.str(), "{at}: error"),
        (Err(e), None) => panic!(
            "{at}: Rust failed with {e}, TypeScript wrote {:?}",
            want.get("t").map(Json::str)
        ),
        (Ok(_), Some(m)) => panic!(
            "{at}: TypeScript failed with {}, Rust wrote {:?}",
            m.str(),
            doc.text()
        ),
        (Ok(g), None) => {
            assert_eq!(doc.text(), want.get("t").unwrap().str(), "{at}: text");
            if let Some(w) = want.get("g") {
                assert_eq!(g.as_ref(), Some(w), "{at}: answer");
            }
        }
    }
}

#[test]
fn the_curated_cases_and_the_seeded_fuzz_replay() {
    let v = vectors();
    let cases = v.get("cases").unwrap().arr();
    assert!(cases.len() > 400);
    let mut ops = 0;
    for c in cases {
        let name = c.get("name").unwrap().str();
        let text = c.get("text").unwrap().str();
        let (cops, results) = (c.get("ops").unwrap().arr(), c.get("results").unwrap().arr());
        let chain = c.get("chain") == Some(&Json::Bool(true));
        let mut doc = Document::new(text);
        for (i, (op, want)) in cops.iter().zip(results).enumerate() {
            if !chain {
                doc = Document::new(text);
            }
            check(name, i, &mut doc, op, want);
            ops += 1;
        }
        if chain {
            let got: Vec<Json> = doc
                .changes()
                .iter()
                .map(|c| {
                    Json::Arr(vec![
                        Json::Num(c.start16 as f64),
                        Json::Num(c.end16 as f64),
                        Json::Str(c.insert.clone()),
                    ])
                })
                .collect();
            assert_eq!(got, c.get("changes").unwrap().arr(), "{name}: changes");
        }
    }
    assert!(ops > 3_000, "{ops} ops");
}

#[test]
fn the_printer_writes_what_the_typescript_wrote() {
    let v = vectors();
    for c in v.get("print").unwrap().arr() {
        let value = value_of(c.get("value").unwrap());
        let header: Vec<String> = match c.get("header") {
            Some(Json::Arr(h)) => h.iter().map(|l| l.str().to_owned()).collect(),
            _ => vec![],
        };
        let marks: Vec<Json> = match c.get("inline") {
            Some(Json::Arr(p)) => p.clone(),
            _ => vec![],
        };
        let has_inline = matches!(c.get("inline"), Some(Json::Arr(_)));
        let inline = |path: &[k7x::edit::PathSeg], _: &Value| {
            marks.iter().any(|m| common::path_of(Some(m)) == path)
        };
        let opts = StringifyOptions {
            header: &header,
            inline: has_inline.then_some(&inline),
        };
        let got = stringify(&value, &opts);
        let out = c.get("out").unwrap();
        match (got, out.get("error")) {
            (Ok(t), None) => assert_eq!(t, out.get("text").unwrap().str(), "{:?}", json_of(&value)),
            (Err(e), Some(m)) => assert_eq!(e.to_string(), m.str()),
            (got, _) => panic!("{got:?} against {out:?}"),
        }
    }
}
