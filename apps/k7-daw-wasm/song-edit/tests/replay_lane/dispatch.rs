// dispatch.rs — screens a vector of the lane and module side, then hands it to `song_edit::facade`: the replay's rules around the surface.
// A reader would get wrong what `Skip` means: no port yet, or a vector the harvest could not record, counted and named.

use super::harness::{repo, root, tmp, uuid};
use super::shape::{Outcome, ordered_placements, outcome};
use serde_json::Value as Json;
use song_edit::confine::Env;
use song_edit::{Fs, MemFs};

fn names_tmp(a: &[Json]) -> bool {
    a.iter().any(|x| x.to_string().replace(&root(), "").contains(&tmp()))
}

pub fn call(
    module: &str,
    f: &str,
    a: &[Json],
    fs: &mut MemFs,
    first_uuid: usize,
    expected_text: Option<&str>,
) -> Outcome {
    let golden = module.starts_with("golden/");
    let module = module.strip_prefix("golden/").unwrap_or(module);
    let env = if golden {
        let mut e = Env::at(&format!("{}/home", root()), &format!("{}/meta", root()));
        e.models_root = format!("{}/home/models", root());
        e
    } else {
        Env::at(
            &format!("{}/home", tmp()),
            &format!("{}/apps/k7-daw-wasm/song/meta", repo()),
        )
    };
    if names_tmp(a) {
        return Outcome::Skip("a folder the harvest scrubbed to $TMP");
    }
    if module == "song/libraryForks" && f == "forksWhoseBaseMoved" && !golden {
        return Outcome::Skip("the library versions sit in a folder the harvest did not record");
    }
    let mut args = a.to_vec();
    if f == "addTrackPlacements"
        && let Some(x) = args.get_mut(2)
    {
        *x = ordered_placements(x, expected_text);
    }
    let scaffold = (module, f) == ("song/scaffold", "scaffold");
    if scaffold {
        // The video shape copies a sample the harvest did not record.
        args.resize(3, Json::Null);
        args.push(Json::String("sample-video".to_owned()));
    }
    let mut n = first_uuid - 1;
    let mut new_id = || {
        n += 1;
        uuid(n)
    };
    let out = outcome(super::shape::exec(&mut *fs, &env, &mut new_id, Some(module), f, &args));
    if scaffold {
        // The golden records a video's name, not its bytes.
        let _ = Fs::remove(fs, &format!("{}/clips/sample.mp4", args[0].as_str().unwrap_or("")));
    }
    out
}
