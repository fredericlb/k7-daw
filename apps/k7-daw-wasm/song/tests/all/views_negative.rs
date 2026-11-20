// views_negative.rs — the negative corpus of the views: each case a project, the views it must give by file and key, never by sentence.
// A reader would get wrong that this needs no TypeScript: stages_views.rs adds the TypeScript's answers to the cases that can have one.

use crate::support;
use crate::views_gate;

use serde_json::Value;
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource};
use std::collections::BTreeSet;
use support::{Tmp, case_roots};
use views_gate::{Ask, Counts, ViewCase, cases, lay, no_null, projection, views};

/// The value at a pointer, "#absent" when there is none; `#len` ends a pointer to a list or a map and counts it.
fn at(v: &Value, pointer: &str) -> Value {
    if let Some(list) = pointer.strip_suffix("/#len") {
        let n = v.pointer(list).map_or(0, |x| match x {
            Value::Array(a) => a.len(),
            Value::Object(o) => o.len(),
            _ => 0,
        });
        return serde_json::json!(n);
    }
    v.pointer(pointer)
        .cloned()
        .unwrap_or_else(|| Value::String("#absent".into()))
}

fn holds(got: &Value, want: &Value) -> bool {
    let more = |x: &Value| match got {
        Value::Array(a) => a.contains(x),
        Value::Object(o) => x.as_str().is_some_and(|k| o.contains_key(k)),
        _ => false,
    };
    match want {
        Value::String(s) if s == "#present" => got != "#absent",
        Value::Object(o) if o.contains_key("has") => more(&o["has"]),
        Value::Object(o) if o.contains_key("lacks") => !more(&o["lacks"]),
        Value::Number(a) => got.as_f64().zip(a.as_f64()).is_some_and(|(x, y)| (x - y).abs() < 1e-9),
        _ => got == want,
    }
}

fn pointers(bad: &mut Vec<String>, case: &str, what: &str, root: &Value, want: &Value) {
    for (p, w) in want.as_object().into_iter().flatten() {
        let got = at(root, p);
        if !holds(&got, w) {
            bad.push(format!("{case} {what}{p}: {got} is not {w}"));
        }
    }
}

/// The row of a list by a key: its `ref`, or its `id` for a surface.
fn row<'a>(list: &'a Value, key: &str, name: &str) -> Option<&'a Value> {
    list.as_array()?.iter().find(|x| x[key] == name)
}

/// For a map of `ref` to pointers (or "#absent"): the row must be there with those values, or not there at all.
fn rows(bad: &mut Vec<String>, case: &str, what: &str, list: &Value, key: &str, want: &Value) {
    for (name, w) in want.as_object().into_iter().flatten() {
        match (row(list, key, name), w) {
            (None, Value::String(s)) if s == "#absent" => {}
            (None, _) => bad.push(format!("{case} {what}: no row {name}")),
            (Some(_), Value::String(s)) if s == "#absent" => bad.push(format!("{case} {what}: {name} is listed")),
            (Some(r), w) => pointers(bad, case, &format!("{what}/{name}"), r, w),
        }
    }
}

fn in_order(bad: &mut Vec<String>, case: &str, what: &str, list: &Value, want: &Value) {
    let have: Vec<&str> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut at = 0;
    for w in want.as_array().into_iter().flatten().filter_map(Value::as_str) {
        match have[at..].iter().position(|h| *h == w) {
            Some(i) => at += i + 1,
            None => bad.push(format!("{case} {what}: {w} is not in order in {have:?}")),
        }
    }
}

fn strings(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_owned))
        .collect();
    out.sort();
    out
}

/// What the case expects of what the views gave, as failures.
fn check(case: &ViewCase, ran: &Value) -> Vec<String> {
    let (n, e) = (case.name.as_str(), &case.expect);
    let mut bad = Vec::new();
    if let Some(want) = e.get("surfaces") {
        let got: Vec<&str> = ran["surfaces"]["surfaces"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["id"].as_str())
            .collect();
        let order: Vec<&str> = want
            .as_array()
            .map_or(vec![], |a| a.iter().filter_map(Value::as_str).collect());
        if got != order {
            bad.push(format!("{n} surfaces: {got:?}, expected {want}"));
        }
    }
    if let Some(want) = e.get("surface_findings") {
        let got = projection(&ran["surfaces"]["diagnostics"]);
        if got != strings(want) {
            bad.push(format!("{n} surface findings: {got:?}, expected {want}"));
        }
    }
    pointers(&mut bad, n, "web", &ran["web"], e.get("web").unwrap_or(&Value::Null));
    if let Some(want) = e.get("errors") {
        let got = projection(&ran["errors"]["diagnostics"]);
        if got != strings(want) {
            bad.push(format!("{n} errors: {got:?}, expected {want}"));
        }
    }
    if let Some(want) = e.get("refused") {
        if strings(&ran["failed"]) != strings(want) {
            bad.push(format!("{n} refused: {}, expected {want}", ran["failed"]));
        }
    } else if !ran["failed"].is_null() && e.get("web").is_some() {
        bad.push(format!("{n}: a case of the web view was refused: {}", ran["failed"]));
    }
    if let Some(want) = e.get("format_too_new")
        && ran["errors"]["formatTooNew"] != *want
    {
        bad.push(format!(
            "{n} formatTooNew: {}, expected {want}",
            ran["errors"]["formatTooNew"]
        ));
    }
    if let Some(want) = e.get("issues") {
        let got = projection(&ran["issues"]);
        if got != strings(want) {
            bad.push(format!("{n} issues: {got:?}, expected {want}"));
        }
    }
    rows(
        &mut bad,
        n,
        "surface",
        &ran["surfaces"]["surfaces"],
        "id",
        e.get("surface_pointers").unwrap_or(&Value::Null),
    );
    rows(
        &mut bad,
        n,
        "io",
        &ran["io"],
        "ref",
        e.get("io").unwrap_or(&Value::Null),
    );
    rows(
        &mut bad,
        n,
        "catalogue",
        &ran["catalogue"],
        "ref",
        e.get("catalogue").unwrap_or(&Value::Null),
    );
    rows(
        &mut bad,
        n,
        "base",
        &ran["base"],
        "ref",
        e.get("base").unwrap_or(&Value::Null),
    );
    pointers(
        &mut bad,
        n,
        "hints",
        &ran["hints"],
        e.get("hints").unwrap_or(&Value::Null),
    );
    if let Some(want) = e.get("io_order") {
        let refs: Value = ran["io"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| p["ref"].clone())
            .collect();
        in_order(&mut bad, n, "io order", &refs, want);
    }
    if let Some(want) = e.get("modules_in_order") {
        in_order(&mut bad, n, "modules order", &ran["hints"]["modules"], want);
    }
    for (p, prefix) in e.get("no_prefix").and_then(Value::as_object).into_iter().flatten() {
        let prefix = prefix.as_str().unwrap_or_default();
        if strings(&at(&ran["hints"], p)).iter().any(|s| s.starts_with(prefix)) {
            bad.push(format!("{n} hints{p}: a name begins with {prefix}"));
        }
    }
    bad
}

#[test]
fn every_case_gives_what_it_expects_on_disk_and_through_its_bundle() {
    let t = Tmp::new("views-negative");
    let (mut failures, mut seen, mut kinds) = (Vec::new(), Counts::default(), BTreeSet::new());
    let all = cases();
    for case in &all {
        let base = t.0.join(&case.name);
        lay(&base, case);
        let roots = case_roots(&base, None);
        let (mounts, files) = roots.mounts().expect("mounts");
        let disk = DiskSource::with_files(&mounts, &files).expect("a source");
        let rec = Recorder::new(&disk);
        let rig = case.rig();
        let ask = Ask {
            roots: &roots,
            rig: &rig,
            vsts_none: case.vsts_none,
        };
        let (ran, lengths) = match views(&rec, &mut Cache::new(), &ask, None) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{}: {e}", case.name));
                continue;
            }
        };
        failures.extend(check(case, &ran));
        let mut nulls = Vec::new();
        no_null(&ran, "", &mut nulls);
        failures.extend(
            nulls
                .into_iter()
                .map(|p| format!("{}: a null in the answer at {p}", case.name)),
        );
        seen.add_views(&ran);
        kinds.insert(case.category.clone());
        if case.mem {
            let bundle = rec.into_bundle_with_files(&mounts, &files);
            let mem = MemSource::from_bundle(bundle).expect("a bundle");
            match views(&mem, &mut Cache::new(), &ask, Some(&lengths)) {
                Ok((again, _)) if again == ran => {}
                Ok(_) => failures.push(format!("{}: the bundle read differently from the disk", case.name)),
                Err(e) => failures.push(format!("{}: the bundle was not enough: {e}", case.name)),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // A case must really exercise what it names: the effects, counted over the whole corpus.
    for (what, n) in [
        ("surfaces", seen.surfaces),
        ("null mapping entries", seen.null_entries),
        ("playlists", seen.playlists),
        ("stems", seen.stems),
        ("surface findings", seen.surface_findings),
        ("surface issue warnings", seen.issue_warnings),
        ("io ports with a resolved label", seen.io_label),
        ("io ports with an unknown label", seen.io_unknown),
        ("io ports of the project's own", seen.io_own),
        ("io ports of a placed node", seen.io_placed),
        ("catalogue rows counting nodes", seen.nodes_above_zero),
        ("catalogue rows with a panel", seen.with_panel),
        ("catalogue rows that extend", seen.extends),
        ("hints channels", seen.channels),
        ("hints favourites that floated up", seen.favourites),
        ("hints hidden modules dropped", seen.hidden),
        ("hints modules with assets", seen.module_assets),
        ("hints slice racks", seen.slice_racks),
        ("hints plugin names", seen.plugin_names),
        ("hints paths", seen.paths),
        ("hints media files", seen.media_files),
        ("hints video tracks", seen.video_tracks),
        ("hints video clips", seen.hint_lists[6]),
        ("hints curves", seen.hint_lists[9]),
        ("web comps", seen.web.comps),
        ("web nests", seen.web.nests),
        ("web timelines", seen.web.timelines),
        ("web crossfades", seen.web.crossfades),
        ("web frozen clips", seen.web.frozen),
        ("web highlights", seen.web.highlights),
        ("web mixers", seen.web.mixers),
        ("web generated lanes", seen.web.generated),
        ("web markers", seen.web.markers),
        ("web clips with a natural length", seen.web.natural),
        ("refused compiles", seen.web.failed),
        ("formatTooNew", seen.web.format_too_new),
        ("not-supported findings", seen.web.not_supported),
        ("unknown-plugin findings", seen.web.unknown_plugin),
        ("dead automations of a plain-state control", seen.web.dead_state),
        ("dead automations of an undeclared name", seen.web.dead_graph),
        ("dead project fields", seen.web.dead_fields),
        ("jsdsp findings", seen.web.jsdsp),
    ] {
        assert!(n >= 1, "the corpus has no {what}");
    }
    for k in ["surfaces", "issues", "catalogue", "io", "hints", "web", "errors"] {
        assert!(kinds.contains(k), "no case of {k}");
    }
    assert_eq!(seen.capped, 0, "a case hit a budget");
    assert!(all.len() >= 90 && all.iter().filter(|c| c.ts).count() >= 70);
}
