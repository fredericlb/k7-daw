// views_gate/mod.rs — what the views' gate shares: every view of a project run through a Source, the comparison, the counts.
// A reader would get wrong that a diagnostic is compared by `file|severity` alone (decision 647), and a rewritten message is no difference.

#![allow(dead_code, unused_imports)]

pub mod cases;
pub mod counts;
pub mod sweeps;
pub mod web;
pub mod web_counts;

use crate::support;
use serde_json::{Value, json};
use song::compare::{Rules, SetRule, compare_all};
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::modules::types::module_status_of;
use song::views::catalogue::{base_modules, module_catalogue};
use song::views::hints::hints_of;
use song::views::io_ports::io_ports;
use song::views::lengths::Lengths;
use song::views::surfaces::{project_surfaces, surface_issues};
use song::{Cache, Roots, Source};
use std::fs;
use std::path::Path;

pub use cases::{ViewCase, cases, lay};
pub use counts::Counts;
pub use web::Ask;
pub use web_counts::WebCounts;

pub fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).expect("json")
}

pub fn rules(sets: &[&str]) -> Rules {
    Rules {
        max: 10,
        sets: sets
            .iter()
            .map(|p| SetRule {
                pointer: (*p).to_owned(),
                key: String::new(),
            })
            .collect(),
        ..Rules::default()
    }
}

pub fn diff_with(failures: &mut Vec<String>, what: &str, theirs: &Value, ours: &Value, sets: &[&str]) {
    for d in compare_all(theirs, ours, &rules(sets)).differences {
        failures.push(format!("{what}{} ts={:?} rust={:?}", d.pointer, d.left, d.right));
    }
}

pub fn diff(failures: &mut Vec<String>, what: &str, theirs: &Value, ours: &Value) {
    diff_with(failures, what, theirs, ours, &[]);
}

/// `file|severity` of each diagnostic, sorted: a rewritten message is no difference (647), `severity` absent is error.
pub fn projection(diags: &Value) -> Vec<String> {
    let mut out: Vec<String> = diags
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            format!(
                "{}|{}",
                d["file"].as_str().unwrap_or(""),
                d["severity"].as_str().unwrap_or("error")
            )
        })
        .collect();
    out.sort();
    out
}

/// What the Rust answers for one project through one source: every view, as the dump spells them, and the spends.
pub fn views(
    source: &dyn Source,
    cache: &mut Cache,
    ask: &Ask<'_>,
    table: Option<&Lengths>,
) -> Result<(Value, Lengths), String> {
    let (roots, rig) = (ask.roots, ask.rig);
    let e = |e: song::SourceError| e.to_string();
    let inputs = load_inputs(source, cache, roots, rig.clone()).map_err(e)?;
    let vocab = inputs.vocabulary.clone();
    let cat = module_catalogue(source, roots, &vocab).map_err(e)?;
    let base = base_modules(&vocab);
    let surfaces = project_surfaces(source, cache, &roots.project).map_err(e)?;
    let issues = surface_issues(source, &inputs, &surfaces).map_err(e)?;
    let io = io_ports(&inputs);
    let hints = hints_of(source, cache, roots, &inputs).map_err(e)?;
    // The favourites that floated up and the hidden modules dropped: the effect, counted where the inputs are known.
    let status = |n: &String| {
        let m = inputs.nodes.nodes.get(n).or_else(|| vocab.modules.get(n));
        let s = m
            .and_then(|m| m.authored.status)
            .map(|s| format!("{s:?}").to_lowercase());
        module_status_of(s.as_deref())
    };
    let offered: std::collections::HashSet<&String> = hints.hints.modules.iter().collect();
    let candidates = inputs.nodes.nodes.keys().chain(vocab.modules.keys());
    let (mut favourites, mut hidden) = (0, 0);
    for n in candidates {
        match (status(n), offered.contains(n)) {
            ("favorite", true) => favourites += 1,
            ("hidden", false) => hidden += 1,
            _ => {}
        }
    }
    let (compiled, lengths) = web::compiled_views(source, cache, ask, table)?;
    let mut out = json!({
        "catalogue": cat.items,
        "catalogue_capped": cat.capped,
        "base": base.items,
        "surfaces": surfaces,
        "issues": issues.iter().map(|f| json!({ "file": f.file, "severity": f.severity })).collect::<Vec<_>>(),
        "io": io.ports,
        "hints": hints.hints,
        "stats": {
            "spent": { "hints": hints.spent, "io": io.spent, "catalogue": cat.spent, "base": base.spent },
            "capped": hints.capped || io.capped || cat.capped || base.capped,
            "favourites": favourites,
            "hidden": hidden,
        },
    });
    if let (Some(o), Value::Object(b)) = (out.as_object_mut(), compiled) {
        o.extend(b);
    }
    Ok((out, lengths))
}

/// The views a project's TypeScript dump may lack because it threw there, each by project and view; none is named.
const TS_THROWS: [(&str, &str); 0] = [];

/// The one place a `null` is a value: a surface's mapping entry (decision 601).
const NULL_ALLOWED: &str = "/surfaces/surfaces/*/surface/mapping/*";

fn matches_path(pattern: &str, path: &str) -> bool {
    let (p, q): (Vec<&str>, Vec<&str>) = (pattern.split('/').collect(), path.split('/').collect());
    p.len() == q.len() && p.iter().zip(&q).all(|(a, b)| *a == "*" || a == b)
}

/// Every `null` in what the Rust answered, by its path: the output rule forbids one outside `NULL_ALLOWED`.
pub fn no_null(v: &Value, path: &str, found: &mut Vec<String>) {
    match v {
        Value::Null if !matches_path(NULL_ALLOWED, path) => found.push(path.to_owned()),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .for_each(|(i, x)| no_null(x, &format!("{path}/{i}"), found)),
        Value::Object(o) => o.iter().for_each(|(k, x)| no_null(x, &format!("{path}/{k}"), found)),
        _ => {}
    }
}

/// One project's views against the TypeScript's dump of them; a view the TypeScript threw on is not compared.
pub fn compare_views(failures: &mut Vec<String>, name: &str, theirs: &Value, ours: &Value, c: &mut Counts) {
    // A throw of the TypeScript is visible: counted here, and the view it ended is not compared.
    for k in ["catalogue", "base", "surfaces", "issues", "io", "hints"] {
        let why = &theirs[format!("{k}_threw")];
        c.ts_threw += usize::from(!why.is_null());
        if !why.is_null() && !TS_THROWS.contains(&(name, k)) {
            failures.push(format!(
                "{name}: the TypeScript threw in {k}, so it was not compared: {why}"
            ));
        }
    }
    match web::compare(failures, name, theirs, ours) {
        web::Compared::ByFiles => c.web.refusals_by_files += 1,
        web::Compared::ByText => c.web.refusals_by_text += 1,
        web::Compared::No => {}
    }
    let mut nulls = Vec::new();
    no_null(ours, "", &mut nulls);
    failures.extend(
        nulls
            .into_iter()
            .map(|p| format!("{name}: a null in the Rust answer at {p}")),
    );
    if theirs["catalogue_threw"].is_null() {
        diff(
            failures,
            &format!("{name} /catalogue"),
            &theirs["catalogue"],
            &ours["catalogue"],
        );
    } else if !ours["catalogue"].as_array().is_some_and(Vec::is_empty) {
        failures.push(format!(
            "{name}: the TypeScript's catalogue threw and the Rust listed rows"
        ));
    }
    if theirs["base_threw"].is_null() {
        diff(failures, &format!("{name} /base"), &theirs["base"], &ours["base"]);
    }
    if theirs["surfaces_threw"].is_null() {
        let (a, b) = (&theirs["surfaces"], &ours["surfaces"]);
        diff(failures, &format!("{name} /surfaces"), &a["surfaces"], &b["surfaces"]);
        let (pa, pb) = (projection(&a["diagnostics"]), projection(&b["diagnostics"]));
        if pa != pb {
            failures.push(format!("{name} /surfaces/diagnostics ts={pa:?} rust={pb:?}"));
        }
    }
    if theirs["issues_threw"].is_null() {
        let (pa, pb) = (projection(&theirs["issues"]), projection(&ours["issues"]));
        if pa != pb {
            failures.push(format!("{name} /issues ts={pa:?} rust={pb:?}"));
        }
    }
    if theirs["io_threw"].is_null() {
        diff(failures, &format!("{name} /io"), &theirs["io"], &ours["io"]);
    }
    if theirs["hints_threw"].is_null() {
        // The scan's plugins are sorted by name here and in the file's order there: a set, then.
        diff_with(
            failures,
            &format!("{name} /hints"),
            &theirs["hints"],
            &ours["hints"],
            &["/pluginNames/*"],
        );
    }
    c.add_views(ours);
}

#[allow(dead_code)]
pub fn spelled(p: &Path) -> String {
    support::spell(p)
}
