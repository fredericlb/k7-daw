// views_gate/web.rs — the compile-based views of the gate: the web project and the error list, with what each counts and compares.
// A reader would get wrong that a replay measures nothing: the bundle holds a binary by its length, so it draws from the first pass's table.

use super::{diff, projection};
use serde_json::{Value, json};
use song::clips::ClipKind;
use song::doc::{ProjectDoc, Request, Vsts, compile};
use song::lanes::rig::Rig;
use song::views::answer::{Want, answer};
use song::views::errors::project_errors;
use song::views::lengths::{ByTable, Lengths};
use song::views::web::web_project_of;
use song::{Cache, Roots, Source};
use std::collections::BTreeSet;

/// What a project is asked with: where it reads, the rig of the machine, and whether the engine hosts no VSTs.
pub struct Ask<'a> {
    pub roots: &'a Roots,
    pub rig: &'a Rig,
    pub vsts_none: bool,
}

impl Ask<'_> {
    fn request(&self) -> Request {
        Request {
            roots: self.roots.clone(),
            rig: self.rig.clone(),
            vsts: if self.vsts_none { Vsts::None } else { Vsts::Scan },
            thawed: Vec::new(),
        }
    }
}

fn text(e: song::SourceError) -> String {
    e.to_string()
}

/// The clips of a document that state no length and draw the one their file measured: where a length came from a header.
fn natural_clips(doc: &ProjectDoc, table: &Lengths) -> usize {
    let tracks = doc.tracks.iter().flat_map(|t| t.clips.iter());
    let lanes = doc
        .timelines
        .iter()
        .flatten()
        .flat_map(|t| t.lanes.iter().flat_map(|l| l.clips.iter()));
    tracks
        .chain(lanes)
        .filter(|c| c.kind == ClipKind::Audio && c.length == 0.0)
        .filter(|c| {
            table
                .get(&c.source)
                .is_some_and(|s| s - c.source_from.unwrap_or(0.0) > 0.0)
        })
        .count()
}

/// Whether the folder is a graph project, as the compile asks it (decision 305): a TypeScript compile reads others as folders of files.
fn graph_project(source: &dyn Source, project: &str) -> Result<bool, String> {
    for name in ["graph.k7x", "modules", "chains"] {
        match source.stat(&song::source::path::join(project, name)) {
            Ok(_) => return Ok(true),
            Err(song::SourceError::Missing) => return Err("Missing".to_owned()),
            Err(_) => {}
        }
    }
    Ok(false)
}

/// The web project and the errors of one compile, as JSON, and the table of lengths the first pass measured.
/// With no `table` the call is `answer`'s; with one it is a replay that compiles and draws through `ByTable`.
pub fn compiled_views(
    source: &dyn Source,
    cache: &mut Cache,
    ask: &Ask<'_>,
    table: Option<&Lengths>,
) -> Result<(Value, Lengths), String> {
    let req = ask.request();
    let (web, errors, lengths, failed, natural) = match table {
        None => {
            let want = Want {
                engine: true,
                web: true,
                errors: true,
                ..Want::default()
            };
            let v = answer(source, cache, &req, &want).map_err(text)?;
            let doc = v.compiled.and_then(|c| c.engine.ok());
            let (web, lengths, failed) = match v.web.ok_or("no web view")? {
                Ok(a) => (Some(a.web), a.lengths, None),
                Err(f) => (None, Lengths::new(), Some(f.diagnostics)),
            };
            let natural = doc.as_ref().map_or(0, |d| natural_clips(d, &lengths));
            (web, v.errors.ok_or("no errors view")?, lengths, failed, natural)
        }
        Some(t) => {
            let c = compile(source, cache, &req).map_err(text)?;
            let errors = project_errors(source, cache, &req, &c, None).map_err(text)?;
            match &c.engine {
                Ok(doc) => {
                    let web = web_project_of(doc, &mut ByTable(t)).map_err(text)?;
                    (Some(web), errors, t.clone(), None, natural_clips(doc, t))
                }
                Err(f) => (None, errors, Lengths::new(), Some(f.diagnostics.clone()), 0),
            }
        }
    };
    let mut value = json!({ "errors": errors, "measured": lengths.len(), "natural": natural });
    if !graph_project(source, &ask.roots.project)? {
        value["legacy"] = json!(true);
    }
    // A null is never written: a view that does not exist has no key.
    if let Some(w) = &web {
        value["web"] = serde_json::to_value(w).map_err(|e| e.to_string())?;
    }
    if let Some(d) = &failed {
        value["failed"] = d.iter().map(|f| f.file.as_str()).collect();
    }
    Ok((value, lengths))
}

/// A diagnostics list with each finding once by (file, line, col, message), in order.
fn once_by_place(v: &Value) -> Value {
    let mut seen = BTreeSet::new();
    let key = |d: &Value| {
        (
            d["file"].to_string(),
            d["line"].to_string(),
            d["col"].to_string(),
            d["message"].to_string(),
        )
    };
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|d| seen.insert(key(d)))
        .cloned()
        .collect()
}

fn files_of(v: &Value) -> BTreeSet<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d["file"].as_str().map(|f| f.trim_start_matches('/').to_owned()))
        .collect()
}

/// The two views against the TypeScript's: a refusal must name every file it named, a clean compile lists the same `file|severity`s.
pub fn compare(failures: &mut Vec<String>, name: &str, theirs: &Value, ours: &Value) -> Compared {
    // A folder with no graph, modules or chains is no project to the Rust and a folder of files to the TypeScript (2h's decision 305).
    if !ours["legacy"].is_null() {
        if ours["failed"].is_null() {
            failures.push(format!("{name}: a folder that is no project compiled"));
        }
        // A newer-format root file is refused by the TypeScript as "update k7", graph or none, and the Rust says it the same.
        if theirs["errors_threw"].is_null() && theirs["errors"]["formatTooNew"] != ours["errors"]["formatTooNew"] {
            failures.push(format!(
                "{name} /errors/formatTooNew ts={} rust={}",
                theirs["errors"]["formatTooNew"], ours["errors"]["formatTooNew"]
            ));
        }
        return Compared::No;
    }
    let refused = !theirs["compile_threw"].is_null();
    if !theirs["errors_threw"].is_null() {
        failures.push(format!(
            "{name}: the TypeScript threw in errors: {}",
            theirs["errors_threw"]
        ));
        return Compared::No;
    }
    if refused != !ours["failed"].is_null() {
        failures.push(format!(
            "{name}: the TypeScript {} and the Rust {}",
            if refused { "refused" } else { "compiled" },
            if refused { "compiled" } else { "refused" }
        ));
        return Compared::No;
    }
    if !theirs["web_threw"].is_null() {
        failures.push(format!("{name}: the TypeScript threw in web: {}", theirs["web_threw"]));
        return Compared::No;
    }
    let (te, oe) = (&theirs["errors"], &ours["errors"]);
    if te["formatTooNew"] != oe["formatTooNew"] {
        failures.push(format!(
            "{name} /errors/formatTooNew ts={} rust={}",
            te["formatTooNew"], oe["formatTooNew"]
        ));
    }
    if refused {
        let (named, ours_named) = (files_of(&te["diagnostics"]), files_of(&oe["diagnostics"]));
        for f in named.difference(&ours_named) {
            failures.push(format!(
                "{name}: the refusal does not name {f} (it names {ours_named:?})"
            ));
        }
        // A throw with no located diagnostic is held to what its text names: a file, or a node, a track, a lane in quotes.
        let by = if named.is_empty() {
            let tokens = names_in(theirs["compile_threw"].as_str().unwrap_or(""));
            if !tokens.iter().any(|t| says(&oe["diagnostics"], t)) {
                failures.push(format!(
                    "{name}: the refusal names none of {tokens:?} (it says {:?})",
                    oe["diagnostics"]
                ));
            }
            Compared::ByText
        } else {
            Compared::ByFiles
        };
        if oe["errors"].as_array().is_none_or(Vec::is_empty) || te["errors"].as_array().is_none_or(Vec::is_empty) {
            failures.push(format!("{name}: a refusal lists no errors"));
        }
        if !ours["web"].is_null() {
            failures.push(format!("{name}: a refused compile has a web project"));
        }
        return by;
    }
    if !te["errors"].as_array().is_none_or(Vec::is_empty) || !oe["errors"].as_array().is_none_or(Vec::is_empty) {
        failures.push(format!("{name}: a clean compile lists errors"));
    }
    // The TypeScript lists a finding once by place and message (639); the Rust is compared as it stands, counts and all.
    let (pa, pb) = (
        projection(&once_by_place(&te["diagnostics"])),
        projection(&oe["diagnostics"]),
    );
    if pa != pb {
        failures.push(format!("{name} /errors/diagnostics ts={pa:?} rust={pb:?}"));
    }
    diff(failures, &format!("{name} /web"), &theirs["web"], &ours["web"]);
    Compared::No
}

/// What a refusal's comparison rested on, for the counts: nothing (a clean compile), the files the TypeScript's diagnostics name, or its text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Compared {
    No,
    ByFiles,
    ByText,
}

/// The files (`*.k7x`), the settings (`project.console`) and the quoted names a compile's throw says, which a refusal of the Rust must say too.
fn names_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| c == ':' || c == ',' || c == '\''))
        .filter(|w| w.ends_with(".k7x") || w.starts_with("project."))
        .map(str::to_owned)
        .collect();
    let mut quoted = text.split('\'');
    quoted.next();
    out.extend(quoted.step_by(2).filter(|q| !q.is_empty()).map(str::to_owned));
    out
}

/// Whether a diagnostic's file or message holds `token`; a `file.k7x` token is held by a file of that name or below it.
fn says(diagnostics: &Value, token: &str) -> bool {
    let bare = token.rsplit('/').next().unwrap_or(token);
    diagnostics.as_array().into_iter().flatten().any(|d| {
        let (file, message) = (d["file"].as_str().unwrap_or(""), d["message"].as_str().unwrap_or(""));
        file.trim_start_matches('/') == token
            || file.contains(&format!("/{token}.k7x"))
            || file.contains(&format!("{bare}.k7x"))
            || message.contains(token)
    })
}
