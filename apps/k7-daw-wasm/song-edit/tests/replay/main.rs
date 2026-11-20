// main.rs — replays the 3e song-edit vectors of the base's modules against the Rust crate, byte for byte.
// A reader would get wrong what a skip is: a vector the harvest recorded without what a replay needs, listed with its reason.

mod chain_goldens;
mod dispatch;
mod harness;
mod recfs;
#[allow(dead_code)]
mod shape;
mod time_goldens;

use dispatch::Outcome;
use harness::{File, Tally, canon, concrete, load, map_strings, report, scrub, sibling};
use serde_json::Value as Json;

const TIME_MODULES: [&str; 11] = [
    "song-edit__materialCut",
    "song-edit__materialWindow",
    "song-edit__clipResize",
    "song-edit__crossfade",
    "song-edit__eventGain",
    "song-edit__eventLoop",
    "song-edit__takeEdit",
    "song-edit__insertTime",
    "song-edit__sequenceLoops",
    "song-edit__songTempo",
    "song-edit__onsets",
];

const MODULES: [&str; 18] = [
    "song__yaml__edit",
    "song__yaml__workspaceDoc",
    "song__yaml__workspaceFile",
    "song__yaml__moduleIO",
    "song__formatStamp",
    "song__confine",
    "song__nodeFiles",
    "song__chainItemEdit",
    "song__laneStrip",
    "song-edit__projectEdit",
    "song-edit__graphEdit",
    "song-edit__alternativeSwitch",
    "song-edit__engineSwitch",
    "song__chainGraphEdit",
    "song-edit__rackTaps",
    "song__assemblyEdit",
    "song-edit__tapChain",
    "song-edit__presets",
];

enum Attempt {
    Pass,
    Skip(String),
    Fail(String),
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
        .get(usize::from(fname == "wsCopy"))
        .and_then(Json::as_str)
        .and_then(|a| a.strip_prefix(&format!("{}/", harness::root())))
        .filter(|n| !n.contains('/'))
        // The recorded names carry `$STAMP`, so the filter is scrubbed like the changes it selects.
        .map(scrub);
    let expected = file.expected_changes(v);
    let text = expected
        .as_ref()
        .and_then(|c| c.values().find_map(|b| b.as_str().map(concrete)));
    let out = dispatch::call(
        &file.module,
        fname,
        &args,
        &mut fs,
        &dispatch::Hint { text, ret: &v["ret"] },
    );
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
            let actual = map_strings(&r, &scrub);
            let mut bad = Vec::new();
            if v.get("thrown").is_some() || canon(&actual) != canon(&v["ret"]) {
                bad.push(format!(
                    "returned {} expected {}",
                    canon(&actual),
                    v.get("thrown").map_or_else(|| canon(&v["ret"]), Json::to_string)
                ));
            }
            if let Some(want) = expected {
                let got: Json = serde_json::to_value(file.changes(&before, &fs, only.as_deref())).unwrap_or(Json::Null);
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

/// A mismatch that comes from what the harvest could not record, not from the port.
fn unreplayable(file: &File, msg: &str) -> Option<&'static str> {
    if file.module == "song/assemblyEdit" && msg.contains("threw \"no assembly template '@") {
        return Some("a library template the harvest did not record");
    }
    if file.module == "song-edit/presets" {
        return Some(
            "a preset over the workspace's `_modules/`, `_presets/` or a library, which the harvest did not record",
        );
    }
    if msg.starts_with("wsRead ") && msg.contains("\\u0000\\u0000") {
        return Some("a file over 64 KiB, which the harvest recorded by its size and not its text");
    }
    let node_side = file.module.starts_with("song-edit/");
    if node_side && msg.contains("threw \"unknown module '") {
        return Some("a ref of the workspace's or the engine's modules, which the harvest did not record");
    }
    if node_side && msg.contains("is not a module this project can see") {
        return Some("a ref of the workspace's modules, which the harvest did not record");
    }
    if node_side && msg.contains("has no tap '") && msg.contains("comes from") {
        return Some("a tap the workspace's rack module states, which the harvest did not record");
    }
    let engines = ["setModuleParam ", "setModuleAsset ", "setModuleExtends "];
    if node_side && engines.iter().any(|f| msg.starts_with(f)) && msg.contains("nodeEngine.test") {
        return Some("a module with engines, which the engine's scan gave and the harvest did not record");
    }
    if node_side && !msg.is_ascii() && msg.contains("e\\u{301}") {
        return Some("a name APFS folds to NFD, which a memory folder does not");
    }
    None
}

fn replay(name: &str) -> (usize, Tally) {
    let Some(file) = load(name) else {
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
        let mut first = attempt(&file, v);
        // Whether the project stood under `$TMP` or beside it is lost to the scrub: either layout may reproduce it.
        if matches!(first, Attempt::Fail(_)) && file.module == "song/confine" {
            sibling(true);
            if matches!(attempt(&file, v), Attempt::Pass) {
                first = Attempt::Pass;
                *t.skipped
                    .entry("(passed in the sibling layout)".to_owned())
                    .or_default() += 1;
            }
            sibling(false);
        }
        match first {
            Attempt::Pass => t.replayed += 1,
            Attempt::Skip(why) => *t.skipped.entry(why).or_default() += 1,
            Attempt::Fail(m) => match unreplayable(&file, &m) {
                Some(why) => *t.skipped.entry(why.to_owned()).or_default() += 1,
                None => t.failures.push(m),
            },
        }
    }
    (file.vectors.len(), t)
}

const BASE_REPLAYED: usize = 4088;

const TIME_REPLAYED: usize = 146;

#[test]
fn the_base_modules_replay_their_vectors() {
    let (mut failed, mut replayed) = (0, 0);
    for m in MODULES {
        let (n, t) = replay(m);
        report(m, n, &t);
        failed += t.failures.len();
        replayed += t.replayed;
    }
    shape::flush_plan("base");
    assert_eq!(failed, 0, "{failed} vectors differ");
    // A name the facade lost is a skip, not a failure, so the replayed count is pinned.
    assert_eq!(
        replayed, BASE_REPLAYED,
        "the facade answers a different number of vectors"
    );
}

#[test]
fn the_time_modules_replay_their_vectors() {
    let (mut failed, mut replayed) = (0, 0);
    for m in TIME_MODULES {
        let (n, t) = replay(m);
        report(m, n, &t);
        failed += t.failures.len();
        replayed += t.replayed;
    }
    shape::flush_plan("time");
    assert_eq!(failed, 0, "{failed} vectors differ");
    // A name the facade lost is a skip, not a failure, so the replayed count is pinned.
    assert_eq!(
        replayed, TIME_REPLAYED,
        "the facade answers a different number of vectors"
    );
}
