//! jobs_vectors.rs — what `vectors/jobs.json` holds, so a regenerated file that lost a case the review asked for fails here (2303).
//! A reader gets wrong that the replays live in `jobs_<piece>.rs`: this file counts and shapes only, and names the three departures.

use crate::support::jobs::{section, vectors};
use serde_json::Value;

#[test]
fn the_vectors_hold_every_section_at_its_size() {
    let all = vectors();
    for (name, at_least) in [
        ("plugin_scan", 43),
        ("plugin", 29),
        ("separation_id", 10),
        ("manifest", 16),
        ("move_into", 17),
        ("preview", 23),
        ("freezer", 18),
    ] {
        assert!(section(&all, name).len() >= at_least, "{name}");
    }
}

#[test]
fn each_preview_scenario_records_its_clip_table_or_why_it_has_none() {
    let all = vectors();
    let mut with_table = 0;
    for sc in section(&all, "preview") {
        let name = sc["name"].as_str().unwrap();
        let table = sc["clips"].is_array() && sc["grid"].is_object();
        assert!(
            table || sc["doc_error"].is_object(),
            "{name}: no table and no error"
        );
        with_table += usize::from(table);
        let cases = sc["cases"].as_array().unwrap();
        assert!(cases.len() >= 80, "{name}");
        assert!(
            cases.iter().all(|c| c["req"].is_object()
                && (c["out"]["ok"].is_object() || c["out"]["error"].is_string())),
            "{name}"
        );
    }
    assert!(with_table >= 19, "{with_table} scenarios with a document");
}

#[test]
fn the_vectors_carry_the_404_the_links_and_the_windows_spellings() {
    let all = vectors();
    let cases: Vec<Value> = section(&all, "preview")
        .into_iter()
        .flat_map(|s| s["cases"].as_array().unwrap().clone())
        .collect();
    assert!(
        cases.iter().any(|c| c["out"]["code"] == "ENOENT"),
        "an ENOENT"
    );
    let sources: Vec<String> = cases
        .iter()
        .filter_map(|c| c["req"]["source"].as_str().map(str::to_owned))
        .collect();
    for want in [
        "clips\\kick.wav",
        "C:/x.wav",
        "C:\\x.wav",
        "\\\\srv\\s\\x.wav",
        "clips/out.wav",
        "{base}/libraries/k7-linked/samples/l.wav",
    ] {
        assert!(sources.iter().any(|s| s == want), "{want}");
    }
    let traces = section(&all, "freezer");
    assert!(
        traces
            .iter()
            .any(|t| t["dir"].as_str().unwrap().starts_with("C:/")),
        "a Windows folder"
    );
    assert!(
        traces
            .iter()
            .any(|t| t["keys"].as_object().unwrap().len() > 1),
        "two spellings or two folders"
    );
    let moves = section(&all, "move_into");
    let names: Vec<&str> = moves.iter().map(|m| m["name"].as_str().unwrap()).collect();
    for departed in [
        "a reply naming a file outside the staging",
        "a relative path",
        "two stems of one name",
    ] {
        assert!(names.contains(&departed), "{departed}");
    }
}
