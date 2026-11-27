// dispatch.rs — screens a vector's call, then hands it to `song_edit::facade`: the replay's own rules around the crate's surface.
// A reader would get wrong what `Skip` means: a vector the harvest recorded without what a replay needs, or a name with no port.

use super::harness::{repo, root, tmp, uuid};
pub use super::shape::Outcome;
use super::shape::outcome;
use super::shape::{ordered_json, sorted_by_position};
use serde_json::{Value as Json, json};
use song_edit::confine::Env;
use song_edit::facade::{self, Side};
use song_edit::{Fs, MemFs};

const SKIP_TMP: &str = "a folder the harvest scrubbed to $TMP";

pub fn env() -> Env {
    let mut e = Env::at(
        &format!("{}/home", tmp()),
        &format!("{}/apps/k7-daw-wasm/song/meta", repo()),
    );
    e.models_root = format!("{}/home/models", tmp());
    e
}

/// Whether an argument names the harvest's own temp folder, whose files no vector holds.
fn names_tmp(a: &[Json]) -> bool {
    a.iter().any(|x| x.to_string().replace(&root(), "").contains(&tmp()))
}

/// What the vector expects, for the replays that must recover what the harvest could not record.
pub struct Hint<'a> {
    pub text: Option<String>,
    pub ret: &'a Json,
}

/// The modules whose vectors need the repository's `@meta` modules mounted and the callback screen.
fn node_module(module: &str) -> bool {
    matches!(
        module,
        "song-edit/projectEdit"
            | "song-edit/graphEdit"
            | "song-edit/rackTaps"
            | "song/assemblyEdit"
            | "song-edit/alternativeSwitch"
            | "song-edit/engineSwitch"
            | "song/chainGraphEdit"
            | "song-edit/tapChain"
            | "song/laneStrip"
            | "song-edit/presets"
    )
}

fn is_module_fn(f: &str) -> bool {
    f.starts_with("setModule")
        || f.contains("ModuleAlternative")
        || f.contains("ModuleEngine")
        || f == "moduleAlternatives"
        || f == "moduleEngines"
}

/// Whether a node of the folder extends a module of the workspace's `_modules/`, which the harvest never recorded.
fn needs_workspace(fs: &MemFs) -> bool {
    fs.files().any(|(_, b)| {
        String::from_utf8_lossy(b).contains("@workspace/") || String::from_utf8_lossy(b).contains("@k7-core")
    })
}

/// The repository's `@meta` modules, copied into the folder the loader reads them from.
fn mount_meta(fs: &mut MemFs) {
    let meta = env().meta_root;
    let _ = Fs::mkdir_all(fs, &meta);
    if let Ok(rd) = std::fs::read_dir(&meta) {
        for e in rd.flatten() {
            if let Ok(bytes) = std::fs::read(e.path()) {
                let _ = Fs::write_atomic(fs, &format!("{meta}/{}", e.file_name().to_string_lossy()), &bytes);
            }
        }
    }
}

/// The ids a call mints, in order: the highest the expected file names, or from 1 when there is no file.
fn ids_for(text: Option<&str>, want: usize) -> Vec<usize> {
    const HEAD: &str = "00000000-0000-4000-8000-";
    match text {
        Some(t) => {
            let mut all: Vec<usize> = t
                .match_indices(HEAD)
                .filter_map(|(i, _)| t.get(i + HEAD.len()..i + HEAD.len() + 12)?.parse::<usize>().ok())
                .collect();
            all.sort_unstable();
            all.dedup();
            all[all.len().saturating_sub(want)..].to_vec()
        }
        None => (1..=want).collect(),
    }
}

/// The source of ids a function draws from: those the expected file names, or a count from 1.
fn id_source(f: &str, text: Option<&str>) -> Box<dyn FnMut() -> String> {
    let mut queue = match f {
        "stripNewLane" => ids_for(text, 1),
        "createPictureLane" => ids_for(text, 2),
        _ => Vec::new(),
    }
    .into_iter();
    let mut n = 0;
    let counted = matches!(f, "addTapChainNode");
    Box::new(move || {
        if counted {
            n += 1;
            uuid(n)
        } else {
            uuid(queue.next().unwrap_or(1))
        }
    })
}

/// The arguments with the key orders the harvest sorted away given back.
fn restore_orders(f: &str, a: &[Json], text: Option<&str>) -> Vec<Json> {
    let mut a = a.to_vec();
    let parsed = text.map(k7x::edit::parse);
    match f {
        "wsWriteData" => {
            if let (Some(Ok(template)), Some(x)) = (&parsed, a.get_mut(1)) {
                *x = ordered_json(x, template);
            }
        }
        "setClipOnsets" => {
            if let (Some(Ok(file)), Some(x)) = (&parsed, a.get_mut(2))
                && let Some(t) = file.get("onsets")
            {
                *x = ordered_json(x, t);
            }
        }
        "setKit" => {
            if let (Some(t), Some(x)) = (text, a.get_mut(2)) {
                *x = sorted_by_position(x, |k| t.find(&format!("{k}: {{")).unwrap_or(usize::MAX));
            }
        }
        "setChainNodePositions" => {
            if let (Some(t), Some(x)) = (text, a.get_mut(1)) {
                *x = sorted_by_position(x, |k| t.rfind(&format!("{k}: {{")).unwrap_or(usize::MAX));
            }
        }
        _ => {}
    }
    a
}

pub fn call(module: &str, f: &str, a: &[Json], fs: &mut MemFs, hint: &Hint) -> Outcome {
    if module == "song/chainItemEdit" {
        let tracks = format!("{}/tracks/", root());
        if fs.files().any(|(p, _)| p.starts_with(&tracks)) && !Fs::exists(fs, &format!("{}/chains", root())) {
            // An empty `chains/` folder is no file, so no vector holds it.
            let _ = Fs::mkdir_all(fs, &format!("{}/chains", root()));
        }
    }
    if (module, f) == ("song/yaml/workspaceFile", "k7xData") {
        let one = call_inner(module, f, a, fs, hint);
        let want = super::harness::canon(hint.ret);
        let same = |o: &Outcome| matches!(o, Outcome::Ret(r) if super::harness::canon(&super::harness::map_strings(r, &super::harness::scrub)) == want);
        if same(&one) {
            return one;
        }
        // The folder a path stood in is lost to `$DIR`: a sequence's short form is read by its `sequences/` folder.
        let name = a[0].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").to_owned();
        let mut moved = a.to_vec();
        moved[0] = json!(format!("{}/sequences/{name}", root()));
        let two = call_inner(module, f, &moved, fs, hint);
        return if same(&two) { two } else { one };
    }
    call_inner(module, f, a, fs, hint)
}

/// The node side's modules with the environment named, for a folder that holds an admin's library overlay.
pub fn call_with(module: &str, f: &str, a: &[Json], fs: &mut MemFs, text: Option<&str>, env: &Env) -> Option<Outcome> {
    if !node_module(module) {
        return None;
    }
    if a.iter().any(|x| x.to_string().contains("\"$\":\"fn\"")) {
        return Some(Outcome::Skip("a callback argument"));
    }
    let shared = a
        .get(1)
        .and_then(Json::as_str)
        .is_some_and(|m| m.starts_with("@workspace/") || m.starts_with("@k7-core"));
    let resolves = matches!(module, "song-edit/alternativeSwitch" | "song-edit/engineSwitch");
    if is_module_fn(f) && (shared || (resolves && needs_workspace(fs))) {
        return Some(Outcome::Skip(
            "a module of `_modules/` outside the folder the harvest recorded",
        ));
    }
    mount_meta(fs);
    Some(run(module, f, a, fs, text, env))
}

pub fn run(module: &str, f: &str, a: &[Json], fs: &mut MemFs, text: Option<&str>, env: &Env) -> Outcome {
    let mut ids = id_source(f, text);
    let args = restore_orders(f, a, text);
    outcome(super::shape::exec(fs, env, &mut *ids, Some(module), f, &args))
}

fn call_inner(module: &str, f: &str, a: &[Json], fs: &mut MemFs, hint: &Hint) -> Outcome {
    if let Some(o) = call_with(module, f, a, fs, hint.text.as_deref(), &env()) {
        return o;
    }
    // A time-side function runs whatever its arguments hold: `undefined` is an argument there.
    if module.starts_with("song-edit/")
        && let Some(name) = time_name(f)
    {
        let mut ids = id_source(f, None);
        let env = env();
        return outcome(super::shape::exec(fs, &env, &mut *ids, None, &name, a));
    }
    let opaque = a
        .iter()
        .any(|x| x.to_string().contains("\"$\":\"fn\"") || x.to_string().contains("\"$\":\"undefined\""));
    if opaque && module != "song/confine" {
        return Outcome::Skip("a callback or undefined argument");
    }
    if names_tmp(a) && module != "song/confine" && f != "readModuleFile" {
        return Outcome::Skip(SKIP_TMP);
    }
    if module == "song/yaml/moduleIO"
        && f == "readModuleFile"
        && let Some(skip) = mount_module_file(fs, &a[0])
    {
        return skip;
    }
    run(module, f, a, fs, hint.text.as_deref(), &env())
}

/// A repository module file is read from the checkout into the memory folder; a scrubbed temp file is skipped.
fn mount_module_file(fs: &mut MemFs, file: &Json) -> Option<Outcome> {
    let path = file["path"].as_str().unwrap_or("");
    if path.starts_with(&repo()) {
        let Ok(text) = std::fs::read(path) else {
            return Some(Outcome::Skip("a repository file that is gone"));
        };
        let _ = Fs::mkdir_all(fs, &song_edit::paths::dirname(path));
        let _ = Fs::write_atomic(fs, path, &text);
    } else if path == tmp() || path.starts_with(&format!("{} ", tmp())) {
        return Some(Outcome::Skip("a file in a folder the harvest scrubbed to $TMP"));
    }
    None
}

/// The qualified name of the time-side function `f`, whatever module the vector names.
pub fn time_name(f: &str) -> Option<String> {
    facade::names()
        .into_iter()
        .find(|n| facade::side_of(n) == Some(Side::Time) && n.rsplit_once('.').is_some_and(|(_, x)| x == f))
}
