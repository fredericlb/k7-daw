// doc_support/world.rs — a project compiled from files held in memory: the laid-out files, the roots of the memory world, the answer.
// A reader would get wrong that the files sit under the project (`/p/ws/proj`) and a path starting `lib/` or `ws/` is laid as it stands.

use crate::support::{mem, mem_world_roots};
use serde_json::{Value, json};
use song::doc::{Compiled, ProjectDoc, Request, Vsts, compile};
use song::lanes::rig::Rig;
use song::{Cache, Roots};

/// A plugin node: a built-in effect the engine plays.
pub const AMP: &str = "plugin({ type: \"internal\", name: \"Chorus\" })\n";

/// `extends("@meta/track")` with the lines given after it.
pub fn lane(extra: &str) -> String {
    format!("extends(\"@meta/track\")\n{extra}")
}

/// Where a file of a case goes: under the project, unless it names the workspace or a library.
pub fn laid(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(rel, text)| {
            let path = if rel.starts_with("lib/") || rel.starts_with("ws/") || rel.starts_with("renders/") {
                (*rel).to_owned()
            } else {
                format!("ws/proj/{rel}")
            };
            (path, (*text).to_owned())
        })
        .collect()
}

pub fn roots() -> Roots {
    Roots {
        renders: Some("/p/renders".into()),
        ..mem_world_roots()
    }
}

pub fn world_request(rig: &Value, vsts: Vsts, thawed: &[&str]) -> Request {
    Request {
        roots: roots(),
        rig: Rig::from_json(rig),
        vsts,
        thawed: thawed.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// A project of `files` compiled with a rig and the VSTs a scan gives.
pub fn compiled_with(files: &[(&str, &str)], rig: &Value, vsts: Vsts) -> Compiled {
    let owned = laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    compile(&src, &mut Cache::new(), &world_request(rig, vsts, &[])).expect("a compile")
}

pub fn compiled(files: &[(&str, &str)]) -> Compiled {
    compiled_with(files, &json!({}), Vsts::Scan)
}

/// The document of a project that compiles; a failure says why.
pub fn doc_of(files: &[(&str, &str)]) -> ProjectDoc {
    match compiled(files).engine {
        Ok(d) => d,
        Err(f) => panic!("the compile failed: {:?}", f.diagnostics),
    }
}

/// The files of a failure, sorted.
pub fn stopped(c: &Compiled) -> Vec<String> {
    let mut v: Vec<String> = c
        .engine
        .as_ref()
        .err()
        .map(|f| f.diagnostics.iter().map(|d| d.file.clone()).collect())
        .unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

/// A graph with the modules given as `a: ["b"], b: ["MIX"]`.
pub fn graph(modules: &str) -> (&'static str, String) {
    ("graph.k7x", format!("modules({{ {modules} }})\n"))
}

/// A project of lane nodes `gtr`, `drums`, effect nodes `amp`, `fx`, `verb` and the graph given.
pub fn project(modules: &str, extra: &[(&str, &str)]) -> Compiled {
    let (g, text) = graph(modules);
    let mut files: Vec<(&str, String)> = vec![
        ("nodes/gtr.k7x", lane("")),
        ("nodes/drums.k7x", lane("")),
        ("nodes/amp.k7x", AMP.to_owned()),
        ("nodes/fx.k7x", AMP.to_owned()),
        ("nodes/verb.k7x", AMP.to_owned()),
        (g, text),
    ];
    files.extend(extra.iter().map(|(a, b)| (*a, (*b).to_owned())));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    compiled(&borrowed)
}

/// The files of 2h's capacity project: 128 lanes of 200 events of 16 notes, 16 plugins a lane of 60 params, 16 buses, 64 VSTs, 3,000 automations.
pub fn capacity_files() -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = vec![("project.k7x".to_owned(), String::new())];
    let notes: Vec<String> = (0..16).map(|k| format!("t({k}).d(1).k({})", 40 + k)).collect();
    for s in 0..8 {
        files.push((
            format!("sequences/s{s}.k7x"),
            format!("notes([{}])\n", notes.join(", ")),
        ));
    }
    let drawn: Vec<String> = (0..16)
        .map(|i| format!("time(\"{}:1:1\").value({})", 1 + i * 2, (i % 2) as f64))
        .collect();
    files.push((
        "curves/c.k7x".into(),
        format!("type(\"custom\")\npoints([{}])\n", drawn.join(", ")),
    ));
    let params: Vec<String> = (0..60).map(|i| format!("p{i}: {i}")).collect();
    let events: Vec<String> = (0..200)
        .map(|e| format!("{{ ref: \"s{}\", start: \"{}:1\" }}", e % 8, 1 + e))
        .collect();
    let mut pairs = Vec::new();
    for b in 0..16 {
        files.push((format!("nodes/bus{b}.k7x"), AMP.to_owned()));
        pairs.push((format!("bus{b}"), "\"MIX\"".to_owned()));
    }
    for l in 0..128 {
        files.push((
            format!("nodes/l{l}.k7x"),
            lane(&format!("events([{}])\n", events.join(", "))),
        ));
        let mut steps = Vec::new();
        for p in 0..16 {
            let vst = l < 64 && p == 15;
            let plugin = if vst {
                "vst3\", name: \"X"
            } else {
                "internal\", name: \"Chorus"
            };
            let autos = if p < 8 {
                "automations([{ param: \"p1\", curve: \"c\" }, { param: \"p2\", curve: \"c\" }, { param: \"p3\", curve: \"c\" }])\n"
            } else {
                ""
            };
            files.push((
                format!("nodes/p{l}_{p}.k7x"),
                format!(
                    "plugin({{ type: \"{plugin}\" }})\nparams({{ {} }})\n{autos}",
                    params.join(", ")
                ),
            ));
            steps.push(format!("\"p{l}_{p}\""));
        }
        let mut chain = vec![(format!("l{l}"), steps[0].clone())];
        for p in 0..15 {
            chain.push((format!("p{l}_{p}"), steps[p + 1].clone()));
        }
        chain.push((format!("p{l}_15"), format!("\"bus{}\"", l % 16)));
        pairs.extend(chain);
    }
    let list: Vec<String> = pairs.iter().map(|(k, v)| format!("\"{k}\": [{v}]")).collect();
    files.push(("graph.k7x".into(), format!("modules({{ {} }})\n", list.join(", "))));
    files
}
