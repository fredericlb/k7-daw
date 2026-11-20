// shape.rs — what the replays share around the facade: its answer as an outcome, and the argument orders the harvest lost.
// A reader would get wrong why keys are reordered: the harvest sorted every object's keys, the editor writes them in call order.

use k7x::edit::Value;
use serde_json::{Map, Value as Json};
use song_edit::MemFs;
use song_edit::confine::Env;
use song_edit::facade::{self, Ctx, Failure, Out};
use std::cell::RefCell;
use std::collections::BTreeMap;

pub enum Outcome {
    Ret(Json),
    Thrown(String),
    Skip(&'static str),
}

pub fn outcome(out: Out) -> Outcome {
    match out {
        Ok(v) => Outcome::Ret(v),
        Err(Failure::Thrown(m)) => Outcome::Thrown(m),
        Err(Failure::Unsupported(why)) => Outcome::Skip(why),
    }
}

/// The JSON object ordered as `template` writes it: its keys first, then the others.
pub fn ordered_json(j: &Json, template: &Value) -> Json {
    match (j, template) {
        (Json::Object(m), Value::Map(t)) => {
            let mut out = Map::new();
            for (k, tv) in t {
                if let Some(x) = m.get(k) {
                    out.insert(k.clone(), ordered_json(x, tv));
                }
            }
            for (k, x) in m {
                if !t.iter().any(|(tk, _)| tk == k) {
                    out.insert(k.clone(), x.clone());
                }
            }
            Json::Object(out)
        }
        (Json::Array(l), Value::List(t)) => Json::Array(
            l.iter()
                .enumerate()
                .map(|(i, x)| t.get(i).map_or_else(|| x.clone(), |tv| ordered_json(x, tv)))
                .collect(),
        ),
        _ => j.clone(),
    }
}

/// An object whose entries are stably sorted by the position `key_of` finds for each key.
pub fn sorted_by_position(j: &Json, key_of: impl Fn(&str) -> usize) -> Json {
    let Json::Object(m) = j else { return j.clone() };
    let mut rows: Vec<(&String, &Json)> = m.iter().collect();
    rows.sort_by_key(|(k, _)| key_of(k));
    Json::Object(rows.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// The placements ordered as the last events of the expected file hold them.
pub fn ordered_placements(list: &Json, expected: Option<&str>) -> Json {
    let Some(items) = list.as_array() else {
        return list.clone();
    };
    let Some(Ok(doc)) = expected.map(k7x::edit::parse) else {
        return list.clone();
    };
    let Some(Value::List(events)) = doc.get("events") else {
        return list.clone();
    };
    if events.len() < items.len() {
        return list.clone();
    }
    let tail = &events[events.len() - items.len()..];
    Json::Array(
        items
            .iter()
            .zip(tail)
            .map(|(p, t)| match (p, t) {
                (Json::Object(m), Value::Map(tm)) => {
                    let mut out = Map::new();
                    for (k, _) in tm {
                        if let Some(x) = m.get(k) {
                            out.insert(k.clone(), x.clone());
                        }
                    }
                    for (k, x) in m {
                        if !tm.iter().any(|(tk, _)| tk == k) {
                            out.insert(k.clone(), x.clone());
                        }
                    }
                    Json::Object(out)
                }
                _ => p.clone(),
            })
            .collect(),
    )
}

/// Where the plan of executed calls goes (a folder), or None when nobody asks for it.
pub fn plan_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("K7_EDIT_PLAN").map(Into::into)
}

#[derive(Default)]
struct Plan {
    execs: Vec<Json>,
    blobs: BTreeMap<String, String>,
}

thread_local! {
    static PLAN: RefCell<Plan> = RefCell::new(Plan::default());
}

fn blob(plan: &mut Plan, bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let h: String = Sha256::digest(bytes)
        .iter()
        .take(10)
        .map(|b| format!("{b:02x}"))
        .collect();
    plan.blobs.entry(h.clone()).or_insert_with(|| b64(bytes));
    h
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn files_of(fs: &MemFs) -> BTreeMap<String, Vec<u8>> {
    fs.files().map(|(p, b)| (p.to_owned(), b.to_vec())).collect()
}

fn env_json(e: &Env) -> Json {
    let pairs = |l: &[(String, String)]| Json::Array(l.iter().map(|(a, b)| serde_json::json!([a, b])).collect());
    let mut o = serde_json::json!({
        "studioHome": e.studio_home, "metaRoot": e.meta_root,
        "librariesHome": e.libraries_home, "modelsRoot": e.models_root,
        "overlays": pairs(&e.overlays),
    });
    if let Some(r) = &e.library_roots {
        o["libraryRoots"] = pairs(r);
    }
    o
}

/// Runs `module.f` (or the name alone) through the facade, recording the call into the plan when one is asked for.
pub fn exec(
    fs: &mut MemFs,
    env: &Env,
    new_id: &mut dyn FnMut() -> String,
    module: Option<&str>,
    f: &str,
    args: &[Json],
) -> Out {
    let recording = plan_dir().is_some();
    let before = recording.then(|| (files_of(fs), fs.dirs().map(str::to_owned).collect::<Vec<_>>()));
    let mut minted: Vec<String> = Vec::new();
    let mut counting = || {
        let id = new_id();
        minted.push(id.clone());
        id
    };
    let out = {
        let mut cx = Ctx {
            fs: &mut *fs,
            env,
            new_id: &mut counting,
        };
        match module {
            Some(m) => facade::run(&mut cx, m, f, args),
            None => facade::run_named(&mut cx, f, args),
        }
    };
    if let Some((was, dirs)) = before {
        let now = files_of(fs);
        PLAN.with(|p| {
            let p = &mut *p.borrow_mut();
            let name = module.map_or_else(|| f.to_owned(), |m| format!("{m}.{f}"));
            let pre: Map<String, Json> = was.iter().map(|(k, v)| (k.clone(), Json::String(blob(p, v)))).collect();
            let mut changed = Map::new();
            for (k, v) in &now {
                if was.get(k) != Some(v) {
                    changed.insert(k.clone(), Json::String(blob(p, v)));
                }
            }
            for k in was.keys().filter(|k| !now.contains_key(*k)) {
                changed.insert(k.clone(), Json::Null);
            }
            let res = match &out {
                Ok(v) => serde_json::json!({ "ret": v }),
                Err(Failure::Thrown(m)) => serde_json::json!({ "thrown": m }),
                Err(Failure::Unsupported(w)) => serde_json::json!({ "unsupported": w }),
            };
            p.execs.push(serde_json::json!({
                "name": name, "args": args, "env": env_json(env), "ids": minted,
                "files": pre, "dirs": dirs, "out": res, "changed": changed,
            }));
        });
    }
    out
}

/// Writes the calls this thread recorded to `<K7_EDIT_PLAN>/<name>.json` and clears them.
pub fn flush_plan(name: &str) {
    let Some(dir) = plan_dir() else { return };
    let plan = PLAN.with(|p| std::mem::take(&mut *p.borrow_mut()));
    let _ = std::fs::create_dir_all(&dir);
    let doc = serde_json::json!({ "blobs": plan.blobs, "execs": plan.execs });
    let _ = std::fs::write(dir.join(format!("{name}.json")), doc.to_string());
}
