// main.rs — replays the 3e vectors of the lane and module side (3i) against the Rust crate, byte for byte.
// A reader would get wrong that the harness is the base's own, shared by path: only the module list and the dispatch differ.

mod dispatch;
#[path = "../replay/harness.rs"]
#[allow(dead_code)]
mod harness;
#[path = "../replay/shape.rs"]
#[allow(dead_code)]
mod shape;

use harness::{File, Tally, canon, concrete, load, map_strings, report, scrub, sibling};
use serde_json::Value as Json;
use shape::Outcome;

const MODULES: [&str; 17] = [
    "golden:song-edit__trackMix",
    "song-edit__timelineFromRange",
    "song-edit__trackMix",
    "song__scaffold",
    "golden:song__scaffold",
    "golden:song__chainStack",
    "golden:song__moduleFork",
    "song__basePreset",
    "song-edit__timelineRename",
    "song__chainCreate",
    "song__chainStack",
    "song__preferences",
    "song__libraryForks",
    "golden:song__preferences",
    "golden:song__moduleMove",
    "golden:song__accountFork",
    "golden:song__libraryForks",
];

enum Attempt {
    Pass,
    Skip(String),
    Fail(String),
}

/// The number of the first `$UUID<n>` a vector expects: ids count on from earlier calls of the same test.
fn first_uuid(text: &str) -> usize {
    text.match_indices("$UUID")
        .filter_map(|(i, _)| {
            let digits: String = text[i + 5..].chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<usize>().ok()
        })
        .min()
        .unwrap_or(1)
}

/// Renumbers the `$UUID<n>` placeholders by first appearance from `first`: the harvest numbers the ids a vector SHOWS, not those a call drew.
fn by_appearance(first: usize, parts: &mut [&mut Json]) {
    fn walk(j: &mut Json, seen: &mut Vec<String>, first: usize) {
        match j {
            Json::String(s) => {
                let mut out = String::new();
                let mut rest = s.as_str();
                while let Some(i) = rest.find("$UUID") {
                    out.push_str(&rest[..i]);
                    let tail = &rest[i + 5..];
                    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
                    let at = seen.iter().position(|d| *d == digits).unwrap_or_else(|| {
                        seen.push(digits.clone());
                        seen.len() - 1
                    });
                    out.push_str(&format!("$UUID{}", first + at));
                    rest = &tail[digits.len()..];
                }
                out.push_str(rest);
                *s = out;
            }
            Json::Array(a) => a.iter_mut().for_each(|x| walk(x, seen, first)),
            Json::Object(o) => o.values_mut().for_each(|x| walk(x, seen, first)),
            _ => {}
        }
    }
    let mut seen = Vec::new();
    for p in parts {
        walk(p, &mut seen, first);
    }
}

/// Equal as the harvest recorded it, except the two gain conversions, whose log10 and pow are the platform's and may differ from V8's by a last digit.
fn same_ret(fname: &str, got: &Json, want: &Json) -> bool {
    if canon(got) == canon(want) {
        return true;
    }
    if fname == "writeProjectFiles" {
        // The recorded argument's key order is lost, so the written names are compared as a set.
        let sorted = |j: &Json| {
            let mut v: Vec<String> = j
                .as_array()
                .map(|a| a.iter().map(Json::to_string).collect())
                .unwrap_or_default();
            v.sort();
            v
        };
        return sorted(got) == sorted(want);
    }
    match (fname, got.as_f64(), want.as_f64()) {
        ("gainToDb" | "dbToGain", Some(a), Some(b)) => {
            (a - b).abs() <= 4.0 * f64::EPSILON * b.abs().max(f64::MIN_POSITIVE)
        }
        _ => false,
    }
}

fn attempt(file: &File, v: &Json) -> Attempt {
    let fname = v["fn"].as_str().unwrap_or("");
    let args: Vec<Json> = v["args"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|a| map_strings(a, &concrete))
        .collect();
    let mut fs = file.fs_of(v);
    let before = fs.clone();
    let only = args
        .first()
        .and_then(Json::as_str)
        .and_then(|a| a.strip_prefix(&format!("{}/", harness::root())))
        .filter(|n| !n.contains('/') && !file.module.starts_with("golden/"))
        .map(str::to_owned);
    let expected = file.expected_changes(v);
    let text = expected
        .as_ref()
        .and_then(|c| c.values().find_map(|b| b.as_str().map(concrete)));
    let seen = format!("{}{:?}", v["ret"], expected);
    // A golden draws its ids from the first, as the generator's counter does; a harvest numbers only those it shows.
    let first = if file.module.starts_with("golden/") {
        1
    } else {
        first_uuid(&seen)
    };
    let out = dispatch::call(&file.module, fname, &args, &mut fs, first, text.as_deref());
    let label = format!(
        "{fname} {} (from {})",
        v["args"].to_string().chars().take(160).collect::<String>(),
        v["from"].as_str().unwrap_or("")
    );
    match out {
        Outcome::Skip(why) => Attempt::Skip(why.to_owned()),
        Outcome::Thrown(msg) => match v.get("thrown") {
            Some(e) if scrub(&msg) == e["message"].as_str().unwrap_or("") => Attempt::Pass,
            other => Attempt::Fail(format!(
                "{label}: threw {msg:?}, expected {other:?} / {:?}",
                v.get("ret")
            )),
        },
        Outcome::Ret(r) => {
            let mut actual = map_strings(&r, &scrub);
            let mut got: Json = serde_json::to_value(file.changes(&before, &fs, only.as_deref())).unwrap_or(Json::Null);
            if fname == "scaffold" && !file.module.starts_with("golden/") {
                by_appearance(first_uuid(&seen), &mut [&mut actual, &mut got]);
            }
            let mut bad = Vec::new();
            if v.get("thrown").is_some() || !same_ret(fname, &actual, &v["ret"]) {
                bad.push(format!(
                    "returned {} expected {}",
                    canon(&actual),
                    v.get("thrown").map_or_else(|| canon(&v["ret"]), Json::to_string)
                ));
            }
            if let Some(want) = expected {
                let want: Json = serde_json::to_value(&want).unwrap_or(Json::Null);
                if canon(&got) != canon(&want) {
                    bad.push(format!(
                        "changed {} expected {}",
                        canon(&got).chars().take(400).collect::<String>(),
                        canon(&want).chars().take(400).collect::<String>()
                    ));
                }
            }
            if bad.is_empty() {
                Attempt::Pass
            } else {
                Attempt::Fail(format!("{label}: {}", bad.join("; ")))
            }
        }
    }
}

/// A golden file from `tests/goldens/lane`, in the 3e vectors' own shape.
fn load_golden(name: &str) -> Option<File> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/lane")
        .join(format!("{name}.json"));
    let j: Json = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let obj = |k: &str| {
        j[k].as_object()
            .map(|o| o.iter().map(|(a, b)| (a.clone(), b.clone())).collect())
            .unwrap_or_default()
    };
    Some(File {
        module: j["module"].as_str().unwrap_or("").to_owned(),
        blobs: obj("blobs"),
        trees: obj("trees"),
        vectors: j["vectors"].as_array().cloned().unwrap_or_default(),
    })
}

fn replay(name: &str) -> (usize, Tally) {
    let Some(file) = name.strip_prefix("golden:").map_or_else(|| load(name), load_golden) else {
        return (0, Tally::default());
    };
    let mut t = Tally::default();
    for v in &file.vectors {
        if v["replay"] != Json::Bool(true) {
            *t.skipped
                .entry("replay: false (a callback or a cut folder)".to_owned())
                .or_default() += 1;
            continue;
        }
        sibling(false);
        let first = attempt(&file, v);
        match first {
            Attempt::Pass => t.replayed += 1,
            Attempt::Skip(why) => *t.skipped.entry(why).or_default() += 1,
            Attempt::Fail(m) => t.failures.push(m),
        }
    }
    (file.vectors.len(), t)
}

const LANE_REPLAYED: usize = 1047;

#[test]
fn the_lane_and_module_side_replays_its_vectors() {
    let (mut failed, mut replayed) = (0, 0);
    for m in MODULES {
        let (n, t) = replay(m);
        report(m, n, &t);
        failed += t.failures.len();
        replayed += t.replayed;
    }
    shape::flush_plan("lane");
    assert_eq!(failed, 0, "{failed} vectors differ");
    // A name the facade lost is a skip, not a failure, so the replayed count is pinned.
    assert_eq!(
        replayed, LANE_REPLAYED,
        "the facade answers a different number of vectors"
    );
}
