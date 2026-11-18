// graph_support/gate.rs — a project run to a graph: its JSON, its model, what it refused with and what it counted, for the gate and the corpus.
// A reader would get wrong that a refusal is an answer: `refused` holds the files and the fault keys, and the JSON is null then.

use serde_json::{Map, Value, json};
use song::doc::{Request, Vsts, compile};
use song::graph::{Folder, GraphError, Handed, handed_of, load_compiled_graph, load_graph, web_graph};
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::{Cache, Roots, Severity, Source, SourceError};
use std::collections::BTreeMap;

/// What a project came to: refused, or drawn.
pub struct Ran {
    pub json: Value,
    pub model: Value,
    pub refused_files: Vec<String>,
    pub refused_faults: Vec<String>,
    pub refused: bool,
    pub findings: Vec<(String, bool)>,
    pub texts: Vec<String>,
    pub issues: Vec<String>,
    pub capped: bool,
    pub spent: usize,
    /// The `Handed` the graph was drawn with, as JSON; `real` says `handed_of` made it from the Rust's own compile.
    pub handed: Value,
    pub real: bool,
}

/// Where a run gets its `Handed`: given by the case, or made by `handed_of` over the Rust's own compile (Part B).
#[derive(Clone, Copy)]
pub enum Via<'a> {
    Given(&'a Handed),
    Compile,
}

impl Ran {
    /// Whether two runs drew the same: the graph, the model, the refusal and every finding (file and severity).
    pub fn same_as(&self, other: &Ran) -> bool {
        let sorted = |r: &Ran| {
            let mut f = r.texts.clone();
            f.sort();
            f
        };
        self.json == other.json
            && self.model == other.model
            && self.handed == other.handed
            && self.refused == other.refused
            && sorted(self) == sorted(other)
    }

    /// The files the answer's findings name, sorted.
    pub fn finding_files(&self) -> Vec<String> {
        let mut f: Vec<String> = self.findings.iter().map(|(f, _)| f.clone()).collect();
        f.sort();
        f.dedup();
        f
    }
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

/// The project at `roots`, read through `source`, drawn with `handed`.
pub fn run(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    rig: &Value,
    via: Via<'_>,
) -> Result<Ran, SourceError> {
    let compiled = match via {
        Via::Compile => Some(compile(
            source,
            cache,
            &Request {
                roots: roots.clone(),
                rig: Rig::from_json(rig),
                vsts: Vsts::Scan,
                thawed: Vec::new(),
            },
        )?),
        Via::Given(_) => None,
    };
    let inputs = load_inputs(source, cache, roots, Rig::from_json(rig))?;
    let g = match load_graph(source, cache, roots, &inputs) {
        Ok(g) => g,
        Err(GraphError::Source(e)) => return Err(e),
        Err(GraphError::Refused(r)) => {
            return Ok(Ran {
                json: Value::Null,
                model: Value::Null,
                refused_files: sorted(r.findings.iter().map(|f| f.file.clone()).collect()),
                refused_faults: sorted(r.faults.iter().map(song::lanes::references::Fault::key).collect()),
                refused: true,
                findings: Vec::new(),
                texts: Vec::new(),
                issues: Vec::new(),
                capped: false,
                spent: 0,
                handed: Value::Null,
                real: false,
            });
        }
    };
    let handed = match (via, &compiled) {
        (Via::Given(h), _) => h.clone(),
        (Via::Compile, Some(c)) => handed_of(source, &inputs, &g, c)?,
        (Via::Compile, None) => unreachable!("a compile was asked for"),
    };
    // The Rust's own compile is drawn through the one call a host makes, over the same steps the dump above is made of.
    let a = match via {
        Via::Given(_) => web_graph(source, cache, roots, &inputs, &g, &handed)?,
        Via::Compile => match load_compiled_graph(
            source,
            cache,
            roots,
            Rig::from_json(rig),
            compiled.as_ref(),
            Folder::Open,
        ) {
            Ok(a) => a,
            Err(GraphError::Source(e)) => return Err(e),
            Err(GraphError::Refused(_)) => unreachable!("the graph was read above"),
        },
    };
    let mut model = Map::new();
    model.insert("chains".into(), serde_json::to_value(&g.chains).expect("json"));
    let tracks: Map<String, Value> = g
        .tracks
        .iter()
        .map(|(k, t)| {
            let placements: Vec<Value> = t
                .placements
                .iter()
                .map(|p| json!({"ref": p.ref_, "start": p.start, "kind": p.kind.name()}))
                .collect();
            let mut o = Map::new();
            o.insert("type".into(), json!(format!("{:?}", t.kind).to_lowercase()));
            if let Some(c) = t.color {
                o.insert("color".into(), json!(c));
            }
            o.insert("mute".into(), json!(t.mute));
            o.insert("placements".into(), json!(placements));
            o.insert("tracks".into(), json!(t.tracks));
            if let Some(s) = t.strudel {
                o.insert("strudel".into(), json!(s));
            }
            if let Some(s) = t.video_source {
                o.insert("videoSource".into(), json!(s));
            }
            ((*k).to_owned(), Value::Object(o))
        })
        .collect();
    model.insert("tracks".into(), Value::Object(tracks));
    let videos: Map<String, Value> = g
        .video_tracks
        .iter()
        .map(|(k, v)| {
            let mut o = Map::new();
            if let Some(c) = v.color {
                o.insert("color".into(), json!(c));
            }
            o.insert("mute".into(), json!(v.mute));
            ((*k).to_owned(), Value::Object(o))
        })
        .collect();
    model.insert("videoTracks".into(), Value::Object(videos));
    model.insert("disabledTracks".into(), json!(g.disabled));
    model.insert("issues".into(), serde_json::to_value(&g.issues).expect("json"));
    model.insert("warnings".into(), json!(g.warnings));
    let mut kinds: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for k in ["plugin", "io", "doc", "track"] {
        kinds.insert(k, Vec::new());
    }
    for (n, m) in &g.modules {
        let k = format!("{:?}", m.kind).to_lowercase();
        if let Some(list) = kinds.get_mut(k.as_str()) {
            list.push(n);
        }
    }
    for list in kinds.values_mut() {
        list.sort_unstable();
    }
    model.insert("modules".into(), json!(kinds));
    let findings = a
        .findings
        .iter()
        .map(|f| (f.file.clone(), f.severity == Severity::Error))
        .collect();
    Ok(Ran {
        json: serde_json::to_value(&a.graph).expect("json"),
        model: Value::Object(model),
        refused_files: Vec::new(),
        refused_faults: Vec::new(),
        refused: false,
        findings,
        texts: a
            .findings
            .iter()
            .map(|f| format!("{}|{:?}|{}", f.file, f.severity, f.message))
            .collect(),
        issues: g.issues.iter().map(song::graph::GraphIssue::key).collect(),
        capped: a.capped,
        spent: a.spent,
        handed: serde_json::to_value(&handed).expect("json"),
        real: matches!(via, Via::Compile),
    })
}

/// Makes the automatic renders a project would play: compiles once and writes a file where each freeze looks for its render.
pub fn lay_renders(roots: &Roots, rig: &Value) {
    let (mounts, files) = roots.mounts().expect("mounts");
    let disk = song::DiskSource::with_files(&mounts, &files).expect("a source");
    let request = Request {
        roots: roots.clone(),
        rig: Rig::from_json(rig),
        vsts: Vsts::Scan,
        thawed: Vec::new(),
    };
    let compiled = compile(&disk, &mut Cache::new(), &request).expect("a compile");
    for f in &compiled.freezes {
        let path = std::path::Path::new(&f.render);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the renders folder");
        std::fs::write(path, b"RIFF").expect("a render");
    }
}

/// What the corpus and the gate count of what happened: occurrences by name.
#[derive(Default, Clone)]
pub struct Counts(pub BTreeMap<String, usize>);

impl Counts {
    pub fn bump(&mut self, what: &str) {
        *self.0.entry(what.to_owned()).or_insert(0) += 1;
    }

    pub fn get(&self, what: &str) -> usize {
        self.0.get(what).copied().unwrap_or(0)
    }

    pub fn add(&mut self, other: &Counts) {
        for (k, v) in &other.0 {
            *self.0.entry(k.clone()).or_insert(0) += v;
        }
    }

    /// What only the real `handed_of` path can show: the compile's values in the answer, and the marks it made.
    fn count_real(&mut self, ran: &Ran) {
        let h = &ran.handed;
        self.bump("real drawn");
        if h["extents"].is_null() {
            self.bump("real uncompiled");
        } else {
            self.bump("real compiled");
        }
        for (k, what) in [("extents", "extents"), ("mixers", "mixers"), ("colors", "colors")] {
            if h[k].as_object().is_some_and(|o| !o.is_empty()) {
                self.bump(&format!("real handed {what}"));
            }
        }
        for (k, what) in [("manual", "manual plans"), ("auto", "automatic freezes")] {
            let n = h[k].as_array().map_or(0, Vec::len);
            *self.0.entry(format!("real handed {what}")).or_insert(0) += n;
        }
        for f in h["auto"].as_array().into_iter().flatten() {
            let (ready, hosted) = (f["ready"] == json!(true), f["hosted"] == json!(true));
            self.bump(match (ready, hosted) {
                (true, false) => "real auto ready",
                (_, true) => "real auto hosted",
                _ => "real auto waiting",
            });
        }
        for k in [
            "frozen",
            "autoFrozen",
            "autoFreezeWaiting",
            "trackMixers",
            "trackColors",
        ] {
            let n = ran.json[k].as_object().map_or(0, Map::len);
            *self.0.entry(format!("real top {k}")).or_insert(0) += n;
        }
        let with_len = ran.json["trackClips"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
            .flat_map(|c| c.as_array().into_iter().flatten())
            .filter(|p| p.get("len").is_some())
            .count();
        *self.0.entry("real clips with len".into()).or_insert(0) += with_len;
    }

    /// Counts what a drawn graph holds: chains, steps, outs, every key of a step, lanes by kind, clips by kind, curves, issues.
    pub fn of(&mut self, ran: &Ran) {
        if ran.refused {
            self.bump("refused");
            return;
        }
        self.bump("drawn");
        if ran.real {
            self.count_real(ran);
        }
        if ran.capped {
            self.bump("capped");
        }
        let g = &ran.json;
        let chains = g["chains"].as_object().cloned().unwrap_or_default();
        for c in chains.values() {
            self.bump("chains");
            if c.get("fromPort").is_some() {
                self.bump("chains with fromPort");
            }
            for o in c["out"].as_array().into_iter().flatten() {
                self.bump("outs");
                if o["to"].as_str().is_some_and(|t| t.contains('#')) {
                    self.bump("outs to a port");
                }
                if o["gain"].as_f64().is_some_and(|x| (x - 1.0).abs() > 1e-9) {
                    self.bump("outs with a level");
                }
                if o.get("fromPort").is_some() {
                    self.bump("outs with fromPort");
                }
            }
            for s in c["use"].as_array().into_iter().flatten() {
                self.bump("steps");
                for k in s.as_object().into_iter().flatten().map(|(k, _)| k) {
                    self.bump(&format!("step key {k}"));
                }
                if s["stub"] == json!(true) {
                    self.bump("stub true");
                }
                if s["hasNativePluginUI"] == json!(true) {
                    self.bump("hasNativePluginUI true");
                }
                if s["enabled"] == json!(false) {
                    self.bump("bypassed step");
                }
                if s["rack"].as_array().is_some_and(|r| !r.is_empty()) {
                    self.bump("rack with sounds");
                }
                if s["rack"].as_array().is_some_and(Vec::is_empty) {
                    self.bump("empty rack");
                }
                if s["ui"]["script"].is_string() {
                    self.bump("panel with a script");
                }
                if s["ui"].get("vertical").is_some() {
                    self.bump("panel with a vertical");
                }
                if s["ui"].get("tabs").is_some() {
                    self.bump("panel with tabs");
                }
                for a in s["automations"].as_array().into_iter().flatten() {
                    self.bump("automations");
                    if a.get("trigger").is_some() {
                        self.bump("automations with a trigger");
                    }
                }
            }
        }
        for c in g["trackClips"].as_object().into_iter().flat_map(|m| m.values()) {
            for p in c.as_array().into_iter().flatten() {
                self.bump(&format!("clip {}", p["kind"].as_str().unwrap_or("?")));
                if p.get("len").is_some() {
                    self.bump("clip with len");
                }
            }
        }
        for s in g["trackSignals"].as_object().into_iter().flat_map(|m| m.values()) {
            self.bump(&format!("signal {}", s.as_str().unwrap_or("?")));
        }
        for t in ran.model["tracks"].as_object().into_iter().flat_map(|m| m.values()) {
            self.bump(&format!("track {}", t["type"].as_str().unwrap_or("?")));
            if t["mute"] == json!(true) {
                self.bump("bypassed track");
            }
        }
        self.0.insert(
            "picture lanes".into(),
            self.get("picture lanes") + ran.model["videoTracks"].as_object().map_or(0, Map::len),
        );
        for k in [
            "trackColors",
            "curves",
            "assemblies",
            "modules",
            "tapFeeds",
            "trackAlternatives",
            "frozen",
            "autoFrozen",
            "autoFreezeWaiting",
            "trackMixers",
            "positions",
            "stacks",
        ] {
            let n = match &g[k] {
                Value::Object(o) => o.len(),
                Value::Array(a) => a.len(),
                _ => 0,
            };
            *self.0.entry(format!("top {k}")).or_insert(0) += n;
        }
        for c in g["curves"].as_object().into_iter().flat_map(|m| m.values()) {
            self.bump(&format!("curve {}", c["type"].as_str().unwrap_or("?")));
        }
        for i in ran.issues.iter().map(|k| k.split('|').next().unwrap_or("")) {
            self.bump(&format!("issue {i}"));
        }
    }
}
