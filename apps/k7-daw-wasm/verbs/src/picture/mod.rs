// picture/mod.rs — the PICTURE of a project: the document the page reads and the one scene both the preview and an export draw.
// A reader would get wrong that a refusal is a failure of the call: it is an answer, `{error}`, which the page shows (docs/verbs-port.md).

mod captions;
mod chains;
mod compile;
mod doc;
mod glsl;
mod lanes;
mod scene;
mod stretch;

pub use compile::{Picture, compile, has_picture};
pub use doc::Doc;
pub use scene::{Use, scene_of};

use serde_json::{Value as Json, json};
use song::Cache;
use song_edit::Fs;
use song_edit::confine::Env;

/// The refusal a render of a song with no picture gets.
pub const NO_PICTURE: &str = "this project draws no picture — nothing to render";

/// `Math.max` over two numbers: a NaN wins, as in JavaScript.
pub(crate) fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) }
}

/// `{doc, warnings}` with every path passed through `to_rel`, or `{error}` when the picture does not build.
pub fn doc_answer(fs: &dyn Fs, cache: &mut Cache, env: &Env, dir: &str, to_rel: &dyn Fn(&str) -> String) -> Json {
    match compile(fs, cache, env, dir) {
        Ok(mut p) => {
            p.doc.map_paths(to_rel);
            json!({ "doc": p.doc, "warnings": p.warnings })
        }
        Err(e) => json!({ "error": e }),
    }
}

/// `{scene, mixes}` for `use_`, or `{error}`; `mixes` says whether the mix is the film's sound.
pub fn scene_answer(
    fs: &dyn Fs,
    cache: &mut Cache,
    env: &Env,
    dir: &str,
    armed: Option<&str>,
    use_: Use,
    url_of: &dyn Fn(&str) -> String,
) -> Json {
    match compile(fs, cache, env, dir) {
        Ok(p) if p.doc.video_tracks.is_empty() => json!({ "error": NO_PICTURE }),
        Ok(p) => json!({ "scene": scene_of(&p.doc, armed, use_, url_of), "mixes": p.doc.sound == Some("mix") }),
        Err(e) => json!({ "error": e }),
    }
}
