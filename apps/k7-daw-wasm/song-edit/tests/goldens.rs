// goldens.rs — replays the writers the 3e tap cannot reach: graph.k7x's, the notes short form, the lane strip's and the chain item's.
// A reader would get wrong that an error is compared by its message, except the chain item's unreadable-chain text, whose Zod path differs.

use k7x::edit::{Document, PathSeg, Value, WriteOptions};
use serde_json::{Value as Json, json};
use song_edit::graph_file as g;
use song_edit::lane_strip::{StripKind, StripPlan, apply_lane_strip_plain};
use song_edit::module_file as m;
use song_edit::notes::expand_notes;
use song_edit::workspace_doc::{EditOpts, ws_edit};
use song_edit::{Fs, MemFs, Res};
use std::collections::BTreeMap;

fn goldens() -> Json {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/writers.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default()).unwrap_or(Json::Null)
}

fn val(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(val).collect()),
        Json::Object(o) => Value::object(o.iter().map(|(k, v)| (k.clone(), val(v))).collect()),
    }
}

fn json_of(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Num(n) => json!(n),
        Value::Str(s) => json!(s),
        Value::List(l) => Json::Array(l.iter().map(json_of).collect()),
        Value::Map(m) => Json::Object(m.iter().map(|(k, x)| (k.clone(), json_of(x))).collect()),
    }
}

fn canon(v: &Json) -> String {
    match v {
        Json::Object(o) => {
            let sorted: BTreeMap<&String, &Json> = o.iter().collect();
            format!(
                "{{{}}}",
                sorted
                    .iter()
                    .map(|(k, x)| format!("{}:{}", Json::String((*k).clone()), canon(x)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Json::Array(a) => format!("[{}]", a.iter().map(canon).collect::<Vec<_>>().join(",")),
        Json::Number(n) => format!("{}", n.as_f64().unwrap_or(f64::NAN)),
        other => other.to_string(),
    }
}

fn s(a: &[Json], i: usize) -> &str {
    a.get(i).and_then(Json::as_str).unwrap_or("")
}

fn strs(j: &Json) -> Vec<String> {
    j.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn segments(j: &Json) -> g::Segments {
    let modules = j["modules"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), strs(v))).collect())
        .unwrap_or_default();
    g::Segments {
        modules,
        bypassed: strs(&j["bypassed"]),
        bus_only: strs(&j["bus_only"]),
    }
}

fn pin(j: &Json) -> (f64, f64) {
    (j["x"].as_f64().unwrap_or(0.0), j["y"].as_f64().unwrap_or(0.0))
}

fn graph_op(doc: &mut Document, op: &str, a: &[Json]) -> Res<Json> {
    let mut sc = doc.at(&[]);
    let list = |v: Vec<String>| Json::Array(v.into_iter().map(Json::String).collect());
    Ok(match op {
        "addNode" => g::add_node(&mut sc, s(a, 0)).map(|()| Json::Null)?,
        "setModuleEnabled" => {
            g::set_module_enabled(&mut sc, s(a, 0), a[1].as_bool().unwrap_or(false)).map(|()| Json::Null)?
        }
        "insertBetween" => g::insert_between(&mut sc, s(a, 0), a[1].as_str(), s(a, 2)).map(|()| Json::Null)?,
        "setNodePosition" => {
            g::set_node_position(&mut sc, s(a, 0), (!a[1].is_null()).then(|| pin(&a[1]))).map(|()| Json::Null)?
        }
        "connectNodes" => g::connect_nodes(&mut sc, s(a, 0), s(a, 1)).map(|()| Json::Null)?,
        "mergeSegments" => g::merge_segments(&mut sc, &segments(&a[0])).map(|()| Json::Null)?,
        "disconnectNodes" => g::disconnect_nodes(&mut sc, s(a, 0), s(a, 1)).map(|()| Json::Null)?,
        "unlinkNode" => list(g::unlink_node(&mut sc, s(a, 0))?),
        "removeNode" => list(g::remove_node(&mut sc, s(a, 0))?),
        "setNodePositions" => {
            let at: Vec<(String, (f64, f64))> = a[0]
                .as_object()
                .map(|o| o.iter().map(|(k, v)| (k.clone(), pin(v))).collect())
                .unwrap_or_default();
            g::set_node_positions(&mut sc, &at).map(|()| Json::Null)?
        }
        "clearNodePositions" => g::clear_node_positions(&mut sc).map(|()| Json::Null)?,
        "renameNode" => list(g::rename_node(&mut sc, s(a, 0), s(a, 1))?),
        other => panic!("no such op {other}"),
    })
}

#[test]
fn graph_writers_match_the_typescript() {
    let all = goldens();
    let cases = all["graphCases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() > 200, "the goldens are missing");
    let mut bad = vec![];
    for c in &cases {
        let mut doc = Document::new(c["text"].as_str().unwrap_or(""));
        let args: Vec<Json> = c["args"].as_array().cloned().unwrap_or_default();
        let out = graph_op(&mut doc, c["op"].as_str().unwrap_or(""), &args);
        let label = format!("{} {} on {:?}", c["op"], Json::Array(args.clone()), c["text"]);
        let want_err = c.get("e").and_then(Json::as_str);
        match (&out, want_err) {
            (Err(e), Some(w)) if e.to_string() == w => {}
            (Ok(r), None) if canon(r) == canon(c.get("ret").unwrap_or(&Json::Null)) => {}
            (o, w) => bad.push(format!("{label}: got {o:?} want error {w:?} ret {:?}", c.get("ret"))),
        }
        if want_err.is_none() && doc.text() != c["t"].as_str().unwrap_or("") {
            bad.push(format!("{label}: text {:?} want {:?}", doc.text(), c["t"]));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ, first: {:#?}",
        bad.len(),
        cases.len(),
        bad.iter().take(5).collect::<Vec<_>>()
    );
}

#[test]
fn the_notes_short_form_matches_the_typescript() {
    let cases = goldens()["noteCases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() > 10);
    for c in &cases {
        let got = expand_notes(
            c["src"].as_str().unwrap_or(""),
            c["step"].as_f64().unwrap_or(1.0),
            c["v"].as_f64(),
        );
        match (got, c.get("e").and_then(Json::as_str)) {
            (Err(e), Some(w)) => assert_eq!(e.to_string(), w, "{}", c["src"]),
            (Ok(l), None) => assert_eq!(
                canon(&Json::Array(l.iter().map(json_of).collect())),
                canon(&c["ret"]),
                "{}",
                c["src"]
            ),
            (g, w) => panic!("{}: got {g:?} want {w:?}", c["src"]),
        }
    }
}

#[test]
fn a_lane_strip_is_written_as_the_typescript_writes_it() {
    let cases = goldens()["stripCases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 7);
    for c in &cases {
        let mut fs = MemFs::new();
        let dir = "/proj";
        fs.mkdir_all(&format!("{dir}/nodes")).ok();
        if let Some(t) = c["graph"].as_str() {
            fs.write_atomic(&format!("{dir}/graph.k7x"), t.as_bytes()).ok();
        }
        let kind = if c["kind"] == "picture" {
            StripKind::Picture
        } else {
            StripKind::Audio
        };
        let name = c["name"].as_str().unwrap_or("").to_owned();
        let plan = StripPlan {
            lane: c["lane"].as_str().unwrap_or("").to_owned(),
            kind,
            rel: format!("nodes/{name}.k7x"),
            name,
            after: c["after"].as_str().map(str::to_owned),
        };
        let out = apply_lane_strip_plain(&mut fs, dir, &plan);
        match (&out, c.get("e").and_then(Json::as_str)) {
            (Err(e), Some(w)) => assert_eq!(e.to_string(), w),
            (Ok(r), None) => assert_eq!(r, &strs(&c["ret"])),
            (o, w) => panic!("{c}: got {o:?} want {w:?}"),
        }
        if c.get("e").is_none() {
            for (rel, text) in c["files"].as_object().into_iter().flatten() {
                assert_eq!(
                    fs.text(&format!("{dir}/{rel}")).as_deref(),
                    text.as_str(),
                    "{rel} of {c}"
                );
            }
        }
    }
}

fn chain_edit(name: &str, item: &mut k7x::edit::Scope) -> Res<()> {
    let key = |k: &str| PathSeg::Key(k.to_owned());
    match name {
        "set Gain" => Ok(item.set_in(&[key("params"), key("Gain")], &Value::Num(0.5), WriteOptions::default())?),
        "delete id" => item.delete_in(&[key("id")], false).map(|_| ()).map_err(Into::into),
        "set bypass" => Ok(item.set_in(&[key("bypass")], &Value::Bool(true), WriteOptions::default())?),
        "set id" => Ok(item.set_in(&[key("id")], &Value::Str("other".to_owned()), WriteOptions::default())?),
        "break" => Ok(item.set_in(&[key("extends")], &Value::Num(5.0), WriteOptions::default())?),
        _ => Ok(()),
    }
}

#[test]
fn a_chain_item_is_edited_as_the_typescript_edits_it() {
    let all = goldens();
    let chain = all["chainCase"]["chain"].as_str().unwrap_or("").to_owned();
    let cases = all["chainCase"]["cases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 12);
    for c in &cases {
        let (id, edit) = (c["id"].as_str().unwrap_or(""), c["edit"].as_str().unwrap_or(""));
        let got = song_edit::chain_item_edit::splice_chain_item(&chain, id, |item| chain_edit(edit, item));
        match (got, c.get("e").and_then(Json::as_str)) {
            (Ok(t), None) => assert_eq!(Some(t.as_str()), c["ret"].as_str(), "{id} {edit}"),
            // The unreadable-chain sentence names a Zod path the Rust decoder does not have.
            (Err(e), Some(w)) if w.starts_with("node ") => assert!(
                e.to_string()
                    .starts_with(&format!("node {id}: the edit would make its chain unreadable")),
                "{id} {edit}: {e}"
            ),
            (Err(e), Some(w)) => assert_eq!(e.to_string(), w, "{id} {edit}"),
            (g, w) => panic!("{id} {edit}: got {g:?} want {w:?}"),
        }
    }
}

fn path_of(j: &Json) -> Vec<PathSeg> {
    j.as_array()
        .map(|a| {
            a.iter()
                .map(|x| {
                    x.as_str().map_or_else(
                        || PathSeg::Index(x.as_u64().unwrap_or(0) as usize),
                        |k| PathSeg::Key(k.to_owned()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn module_writers_match_the_typescript() {
    let cases = goldens()["moduleCases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() > 50, "the goldens are missing");
    for c in &cases {
        let mut doc = Document::new(c["text"].as_str().unwrap_or(""));
        let a = c["args"].as_array().cloned().unwrap_or_default();
        let mut sc = doc.at(&[]);
        let out = match c["op"].as_str().unwrap_or("") {
            "setModuleList" => m::set_module_list(
                &mut sc,
                &path_of(&a[0]),
                &a[1]
                    .as_array()
                    .map(|l| l.iter().filter_map(Json::as_f64).collect::<Vec<_>>())
                    .unwrap_or_default(),
            ),
            "setModuleValue" => {
                m::set_module_value(&mut sc, &path_of(&a[0]), (!a[1].is_null()).then(|| val(&a[1])).as_ref())
            }
            _ => m::set_module_automation(&mut sc, s(&a, 0), a[1].as_str(), a.get(2).and_then(Json::as_str)),
        };
        let label = format!("{} {} on {:?}", c["op"], c["args"], c["text"]);
        match (out, c.get("e").and_then(Json::as_str)) {
            (Err(e), Some(w)) => assert_eq!(e.to_string(), w, "{label}"),
            (Ok(()), None) => assert_eq!(doc.text(), c["t"].as_str().unwrap_or(""), "{label}"),
            (o, w) => panic!("{label}: got {o:?} want {w:?}"),
        }
    }
}

fn ws_change(name: &str, doc: &mut Document) -> Res<()> {
    let key = |k: &str| PathSeg::Key(k.to_owned());
    let at = |p: &[PathSeg], v: Value, d: &mut Document| Ok(d.set_in(p, &v, WriteOptions::default())?);
    match name {
        "touch first note" => at(&[key("notes"), PathSeg::Index(0), key("d")], Value::Num(2.0), doc),
        "add a note" => {
            let note = Value::Map(vec![
                ("t".into(), Value::Num(0.5)),
                ("d".into(), Value::Num(0.5)),
                ("k".into(), Value::Num(61.0)),
            ]);
            Ok(doc.insert_in(&[key("notes")], 1, &note, WriteOptions::default())?)
        }
        "drop notes" => doc.delete_in(&[key("notes")], false).map(|_| ()).map_err(Into::into),
        "add a dest" => Ok(doc.insert_in(
            &[key("modules"), key("a")],
            0,
            &Value::Str("fx".into()),
            WriteOptions::default(),
        )?),
        "add a node" => at(
            &[key("modules"), key("z")],
            Value::List(vec![Value::Str("MIX".into())]),
            doc,
        ),
        "set gain" => at(&[key("params"), key("Gain")], Value::Num(0.5), doc),
        "raw set" => at(&[key("step")], Value::Num(3.0), doc),
        _ => Ok(()),
    }
}

#[test]
fn ws_edit_keeps_short_forms_as_the_typescript_does() {
    let cases = goldens()["editCases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 48);
    for c in &cases {
        let path = format!("/proj/{}", c["rel"].as_str().unwrap_or(""));
        let mut fs = MemFs::new();
        fs.mkdir_all(&song_edit::paths::dirname(&path)).ok();
        fs.write_atomic(&path, c["text"].as_str().unwrap_or("").as_bytes()).ok();
        let name = c["edit"].as_str().unwrap_or("");
        let out = ws_edit(
            &mut fs,
            &path,
            EditOpts {
                create: false,
                raw: c["raw"] == true,
            },
            |d| ws_change(name, d),
        );
        let label = format!("{} {name} on {:?}", c["rel"], c["text"]);
        match (out, c.get("e").and_then(Json::as_str)) {
            (Err(e), Some(w)) => assert_eq!(e.to_string(), w, "{label}"),
            (Ok(wrote), None) => {
                assert_eq!(wrote, c["ret"] == true, "{label}");
                assert_eq!(fs.text(&path).as_deref(), c["t"].as_str(), "{label}");
            }
            (o, w) => panic!("{label}: got {o:?} want {w:?}"),
        }
    }
}
