// scene.rs — a picture document as the plain scene `@studio/ui-eyes` draws: lanes folded to shown or hidden, passes kept enabled.
// A reader would get wrong that the preview and the export differ only by `use`: a preview reads the proxy, an export never does.

use super::doc::{Doc, Plugin, Track};
use serde_json::{Map, Value as Json, json};

/// Who reads the scene: the live preview, or an export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Preview,
    Export,
}

/// The shader whose knobs at these values change nothing: the compositor skips it there.
const STRIP_SHADER: &str = "video-mixer";

/// Whether a lane's lens is open: `off` never, `on` always, `auto` while the lane is armed.
fn lens_open(t: &Track, armed: Option<&str>) -> bool {
    let Some(_) = &t.camera else { return false };
    match t.camera_monitor.as_deref().unwrap_or("on") {
        "on" => true,
        "auto" => armed == Some(t.id.as_str()),
        _ => false,
    }
}

/// Whether a lane is off screen: muted, soloed away, or fed by a lens that is shut.
fn hidden(all: &[Track], t: &Track, armed: Option<&str>) -> bool {
    t.mute || (all.iter().any(|l| l.solo) && !t.solo) || (t.camera.is_some() && !lens_open(t, armed))
}

/// A shader pass, enabled or not; none for a video effect or a shader with no source.
fn pass_of(p: &Plugin, url_of: &dyn Fn(&str) -> String) -> Option<Json> {
    let s = p.shader.as_ref().filter(|_| p.kind == "shader")?;
    let mut o = json!({
        "id": p.id, "name": p.name, "vertex": s.vertex, "fragment": s.fragment,
        "enabled": p.enabled, "params": p.params, "audio": 0,
    });
    if let Some(l) = &p.lut {
        o["lut"] = json!(url_of(l));
    }
    if s.name == STRIP_SHADER {
        o["identity"] = json!({ "P1": 1, "P2": 1, "P3": 1, "P4": 0 });
    }
    Some(o)
}

/// The enabled passes of a run: a bypassed one is dropped so no program is made for it.
fn enabled(run: &[Plugin], url_of: &dyn Fn(&str) -> String) -> Vec<Json> {
    run.iter()
        .filter(|p| p.enabled)
        .filter_map(|p| pass_of(p, url_of))
        .collect()
}

fn clip_of(c: &super::doc::Clip, use_: Use, url_of: &dyn Fn(&str) -> String) -> Json {
    let file = match use_ {
        Use::Preview => c.proxy.as_deref().or(c.optimized.as_deref()),
        Use::Export => c.optimized.as_deref(),
    };
    let src = if c.src.is_empty() {
        String::new()
    } else {
        url_of(file.unwrap_or(&c.src))
    };
    let mut o = Map::new();
    o.insert("id".into(), json!(c.id));
    o.insert("src".into(), json!(src));
    o.insert("start".into(), json!(c.start));
    o.insert("length".into(), json!(c.length));
    o.insert("sourceFrom".into(), json!(c.source_from));
    let opt = [
        ("fadeIn", c.fade_in.as_ref().map(|f| json!(f))),
        ("fadeOut", c.fade_out.as_ref().map(|f| json!(f))),
        ("animate", c.animate.as_ref().map(|a| json!(a))),
        ("speed", c.speed.map(|s| json!(s))),
        ("still", c.still.then(|| json!(true))),
        ("effect", c.effect.as_ref().map(|e| json!(e))),
        ("captions", c.captions.as_ref().map(|k| json!(k))),
    ];
    for (k, v) in opt {
        if let Some(v) = v {
            o.insert(k.into(), v);
        }
    }
    Json::Object(o)
}

/// The scene of `doc` for `use_`, the lane `armed` opening an `auto` lens, every file made a URL by `url_of`.
pub fn scene_of(doc: &Doc, armed: Option<&str>, use_: Use, url_of: &dyn Fn(&str) -> String) -> Json {
    let all = &doc.video_tracks;
    let mut duration: f64 = 0.0;
    let layers: Vec<Json> = all
        .iter()
        .map(|t| {
            for c in &t.clips {
                let len = if c.length.is_nan() || c.length == 0.0 {
                    0.0
                } else {
                    c.length
                };
                duration = super::js_max(duration, c.start + len);
            }
            let mut o = Map::new();
            o.insert("id".into(), json!(t.id));
            o.insert("hidden".into(), json!(hidden(all, t, armed)));
            if lens_open(t, armed) {
                o.insert("camera".into(), json!(t.camera));
            }
            for (k, v) in [("x", t.x), ("y", t.y), ("w", t.w), ("h", t.h)] {
                o.insert(k.into(), json!(v));
            }
            let clips: Vec<Json> = t.clips.iter().map(|c| clip_of(c, use_, url_of)).collect();
            o.insert("clips".into(), json!(clips));
            if !t.shapes.is_empty() {
                o.insert("shapes".into(), json!(t.shapes));
            }
            let passes = enabled(&t.plugins, url_of);
            if !passes.is_empty() {
                o.insert("passes".into(), json!(passes));
            }
            Json::Object(o)
        })
        .collect();
    let mut out = json!({ "viewport": doc.viewport, "fps": doc.fps, "layers": layers, "durationSec": duration });
    let frame = enabled(&doc.frame_plugins, url_of);
    if !frame.is_empty() {
        out["framePasses"] = json!(frame);
    }
    if !doc.transitions.is_empty() {
        out["transitions"] = json!(doc.transitions);
    }
    if !doc.stretches.is_empty() {
        let runs: Map<String, Json> = doc
            .stretches
            .iter()
            .map(|(k, run)| (k.clone(), json!(enabled(run, url_of))))
            .collect();
        out["stretches"] = Json::Object(runs);
    }
    out
}
