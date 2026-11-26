// chains.rs — the graph's picture runs: each lane's passes, the frame's, the lenses, the shapes and the stretches a fade names.
// A reader would get wrong that a bypassed step is dropped here: it is kept, and only the scene leaves it out.

use super::doc::Plugin;
use super::glsl::{knobs, read_shader};
use indexmap::IndexMap;
use serde_json::{Value as Json, json};
use song::Source;
use song::graph::{GraphDoc, GraphModule, Index};
use song::modules::{IoKind, LoadedModule, Monitor, ParamValue};
use song::routing::{Chain, UseStep, split_port};

/// A camera that feeds no lane: a layer of its own, full frame unless its node says otherwise.
pub struct CameraLayer {
    pub node: String,
    pub label: String,
    pub frame: Option<[f64; 4]>,
    pub monitor: &'static str,
}

/// Where each compiled run lands.
#[derive(Default)]
pub struct Chains {
    pub by_lane: IndexMap<String, Vec<Plugin>>,
    pub frame: Vec<Plugin>,
    pub sound: Option<&'static str>,
    pub camera_by_lane: IndexMap<String, (String, &'static str)>,
    pub cameras: Vec<CameraLayer>,
    pub shapes_by_lane: IndexMap<String, Vec<Json>>,
    pub stretches: IndexMap<String, Vec<Plugin>>,
    pub stretch_lanes: IndexMap<String, Vec<String>>,
    pub warnings: Vec<String>,
}

/// A node's `monitor:`: a boolean maps onto the ends, and absent is `on`.
fn monitor_of(m: Option<&Monitor>) -> &'static str {
    match m {
        Some(Monitor::Flag(false)) => "off",
        Some(Monitor::Mode(s)) if s == "off" => "off",
        Some(Monitor::Mode(s)) if s == "auto" => "auto",
        _ => "on",
    }
}

fn param(m: &LoadedModule, name: &str, dflt: f64) -> f64 {
    match m.authored.params.as_ref().and_then(|p| p.get(name)) {
        Some(ParamValue::Number(n)) if n.is_finite() => *n,
        _ => dflt,
    }
}

fn word<T: serde::Serialize>(v: Option<&T>) -> Option<String> {
    v.and_then(|v| serde_json::to_value(v).ok())
        .and_then(|j| j.as_str().map(str::to_owned))
}

/// One shape module as the primitive it draws, in viewport px; its geometry is its `params:`.
fn shape_of(node: &str, m: &LoadedModule) -> Json {
    let a = &m.authored;
    let color = a
        .color
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "#ffffff".to_owned());
    let (x, y) = (param(m, "x", 0.0), param(m, "y", 0.0));
    match word(a.shape.as_ref()).as_deref() {
        Some("circle") => {
            json!({ "id": node, "type": "circle", "x": x, "y": y, "r": param(m, "r", 0.0), "color": color })
        }
        Some("text") => {
            let mut t = json!({
                "id": node, "type": "text", "content": a.content.clone().unwrap_or_default(),
                "font": a.font.clone().filter(|f| !f.is_empty()).unwrap_or_else(|| "sans-serif".to_owned()),
                "fontSize": param(m, "font_size", 24.0), "x": x, "y": y,
            });
            if let Some(o) = t.as_object_mut() {
                for (k, p) in [("w", "w"), ("lineHeight", "line_height"), ("padding", "padding")] {
                    let v = param(m, p, 0.0);
                    if v > 0.0 {
                        o.insert(k.to_owned(), json!(v));
                    }
                }
                o.insert("color".to_owned(), json!(color));
                if let Some(al) = word(a.align.as_ref()) {
                    o.insert("align".to_owned(), json!(al));
                }
                if let Some(bg) = a.background.clone().filter(|b| !b.is_empty()) {
                    o.insert("background".to_owned(), json!(bg));
                }
                if a.shadow == Some(true) {
                    o.insert("shadow".to_owned(), json!(true));
                }
            }
            t
        }
        _ => json!({
            "id": node, "type": "square", "x": x, "y": y, "w": param(m, "w", 0.0), "h": param(m, "h", 0.0), "color": color,
        }),
    }
}

/// What a step's run needs to read its GLSL.
pub struct Ctx<'a> {
    pub fs: &'a dyn Source,
    pub dir: &'a str,
    pub refs: &'a IndexMap<String, LoadedModule>,
}

fn join_dir(dir: &str, rel: &str) -> String {
    if rel.starts_with('/') {
        rel.to_owned()
    } else {
        song_edit::paths::resolve(&format!("{}/{}", dir.trim_end_matches('/'), rel))
    }
}

/// One step as a pass, or none when it is no picture module.
fn step_doc(cx: &Ctx<'_>, g: &GraphDoc<'_>, owner: &str, s: &UseStep, i: usize) -> Option<Plugin> {
    let gm: &GraphModule<'_> = g.modules.get(s.module.as_str())?;
    let m: &LoadedModule = gm.m;
    let (id, enabled) = (format!("{owner}:{i}"), s.enabled != Some(false));
    match gm.category {
        Some("shaders") => {
            let name = m.authored.shader.as_ref().map_or("", |n| n.0.as_str());
            let shader = (!name.is_empty())
                .then(|| read_shader(cx.fs, cx.dir, m, cx.refs, name))
                .flatten();
            let audio_from = s
                .inputs
                .as_ref()
                .and_then(|l| l.first())
                .map(|p| split_port(p).0.to_owned());
            Some(Plugin {
                id,
                kind: "shader",
                name: s.module.clone(),
                enabled,
                params: knobs(m.authored.params.as_ref()),
                shader,
                audio_from,
                lut: m.authored.lut.as_ref().map(|l| join_dir(cx.dir, &l.0)),
            })
        }
        Some("video_effects") => Some(Plugin {
            id,
            kind: "video_effect",
            name: s.module.clone(),
            enabled,
            params: m
                .authored
                .params
                .iter()
                .flatten()
                .filter_map(|(k, v)| match v {
                    ParamValue::Number(n) => Some((k.clone(), *n)),
                    ParamValue::Text(_) => None,
                })
                .collect(),
            shader: None,
            audio_from: None,
            lut: None,
        }),
        _ => None,
    }
}

fn run_docs(cx: &Ctx<'_>, g: &GraphDoc<'_>, owner: &str, c: &Chain) -> Vec<Plugin> {
    c.use_steps
        .iter()
        .enumerate()
        .filter_map(|(i, s)| step_doc(cx, g, owner, s, i))
        .collect()
}

fn lanes_of(g: &GraphDoc<'_>, c: &Chain) -> Vec<String> {
    c.out
        .iter()
        .map(|o| split_port(&o.to).0)
        .filter(|l| g.video_tracks.contains_key(l))
        .map(str::to_owned)
        .collect()
}

/// Every picture run of the graph, compiled.
pub fn video_chains(cx: &Ctx<'_>, g: &GraphDoc<'_>) -> Chains {
    let mut out = Chains::default();
    let on = |name: &str| !g.disabled.contains(&name);
    for (name, c) in &g.chains {
        if !on(name) {
            continue;
        }
        let cam = c.use_steps.iter().find(|s| {
            g.modules
                .get(s.module.as_str())
                .is_some_and(|m| m.io == Some(IoKind::Camera))
        });
        let Some(cam) = cam.filter(|s| s.enabled != Some(false)) else {
            continue;
        };
        let Some(m) = g.modules.get(cam.module.as_str()).map(|m| m.m) else {
            continue;
        };
        let label = m.authored.camera.clone().unwrap_or_default();
        let monitor = monitor_of(m.authored.monitor.as_ref());
        let lanes = lanes_of(g, c);
        for lane in &lanes {
            out.camera_by_lane
                .entry(lane.clone())
                .or_insert_with(|| (label.clone(), monitor));
        }
        if lanes.is_empty() && !out.cameras.iter().any(|x| x.node == cam.module) {
            out.cameras.push(CameraLayer {
                node: cam.module.clone(),
                label,
                frame: m.authored.frame,
                monitor,
            });
        }
    }
    for (name, c) in &g.chains {
        if !on(name) {
            continue;
        }
        let drawn: Vec<&UseStep> = c
            .use_steps
            .iter()
            .filter(|s| {
                s.enabled != Some(false)
                    && g.modules
                        .get(s.module.as_str())
                        .is_some_and(|m| m.category == Some("shapes"))
            })
            .collect();
        let Some(first) = drawn.first() else { continue };
        let lanes = lanes_of(g, c);
        if lanes.is_empty() {
            let names: Vec<String> = drawn.iter().map(|s| format!("'{}'", s.module)).collect();
            out.warnings.push(format!(
                "chain '{name}': {} draws nothing — a shape is drawn by the picture lane it is wired to (`{}: [ <lane> ]` in graph.k7x)",
                names.join(", "),
                first.module
            ));
            continue;
        }
        for lane in lanes {
            let list = out.shapes_by_lane.entry(lane).or_default();
            for s in &drawn {
                if let Some(m) = g.modules.get(s.module.as_str()) {
                    list.push(shape_of(&s.module, m.m));
                }
            }
        }
    }
    let index = Index::new(g);
    for (i, (name, c)) in g.chains.iter().enumerate() {
        if !on(name) {
            continue;
        }
        if index.is_stretch(i)
            && let Some(head) = c.use_steps.first()
        {
            out.stretches
                .insert(head.module.clone(), run_docs(cx, g, &head.module, c));
            out.stretch_lanes.insert(head.module.clone(), lanes_of(g, c));
            continue;
        }
        let Some(from) = c.from.as_deref() else { continue };
        if from == "RENDER" {
            if c.out.iter().any(|o| split_port(&o.to).0 == "VIDEO_OUT") {
                out.frame = run_docs(cx, g, "RENDER", c);
            }
        } else if from == "MIX" {
            if c.out.iter().any(|o| o.to == "RENDER#audio") {
                out.sound = Some("mix");
            }
        } else if g.video_tracks.contains_key(from) {
            let docs = run_docs(cx, g, from, c);
            if !docs.is_empty() {
                out.by_lane.insert(from.to_owned(), docs);
            }
        }
    }
    out
}
