// transitions.rs — the TWO transitions the compositor owns, in the shape a compiled one has so the scene holds one map.
// A reader would get wrong that a module of the same name on disk overrides either: the loader's rule, applied by `with_builtins`.

use crate::scene::Transition;
use indexmap::IndexMap;

/// The name a bare `fadeIn(1)` compiles to; `song-schema`'s `DISSOLVE` is the same word.
pub const DISSOLVE: &str = "dissolve";

/// The one a stretch with no `with:` compiles to; `song-schema`'s `OVER` is the same word.
pub const OVER: &str = "over";

/// The vertex stage every full-target program wants, and the same one a shader module ships.
const VERTEX: &str = "#version 300 es
in vec2 aUnit;
out vec2 vUV;
void main() {
  vUV = aUnit;
  gl_Position = vec4(aUnit * 2.0 - 1.0, 0.0, 1.0);
}";

/// Premultiplied on both sides, so ONE mix serves a cross-dissolve and a fade to nothing.
const DISSOLVE_FRAGMENT: &str = "#version 300 es
precision mediump float;
in vec2 vUV;
uniform sampler2D uFrom;
uniform sampler2D uTo;
uniform float uProgress;
out vec4 outColor;
void main() {
  outColor = mix(texture(uFrom, vUV), texture(uTo, vUV), uProgress);
}";

/// Straight alpha in, as a pass leaves it; PREMULTIPLIED out, as every blend's result is composited.
const OVER_FRAGMENT: &str = "#version 300 es
precision mediump float;
in vec2 vUV;
uniform sampler2D uFrom;
uniform sampler2D uTo;
out vec4 outColor;
void main() {
  vec4 a = texture(uFrom, vUV);
  vec4 b = texture(uTo, vUV);
  float under = a.a * (1.0 - b.a);
  outColor = vec4(b.rgb * b.a + a.rgb * under, b.a + under);
}";

fn builtin(name: &str, fragment: &str) -> Transition {
    Transition {
        name: name.to_owned(),
        vertex: VERTEX.to_owned(),
        fragment: fragment.to_owned(),
        params: ["P1", "P2", "P3", "P4"]
            .into_iter()
            .map(|k| (k.to_owned(), 0.0))
            .collect(),
    }
}

/// The dissolve, and the incoming shot laid OVER the outgoing one by its own alpha (`uProgress` unread).
pub fn builtins() -> IndexMap<String, Transition> {
    IndexMap::from([
        (DISSOLVE.to_owned(), builtin(DISSOLVE, DISSOLVE_FRAGMENT)),
        (OVER.to_owned(), builtin(OVER, OVER_FRAGMENT)),
    ])
}

/// The scene's transitions: the two built in, then the song's own, which replace a built-in of the same name.
pub fn with_builtins(own: &IndexMap<String, Transition>) -> IndexMap<String, Transition> {
    let mut all = builtins();
    all.extend(own.iter().map(|(k, t)| (k.clone(), t.clone())));
    all
}
