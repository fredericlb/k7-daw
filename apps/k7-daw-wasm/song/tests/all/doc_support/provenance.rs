// doc_support/provenance.rs — where a colour and a lane's place came from, counted off the project's files and the document it made.
// A reader would get wrong that this reads the project from disk: a document alone cannot say a colour was stated or a track order was.

use super::counts::Counts;
use serde_json::Value;
use song::doc::{category_of_text, parse_track_name};
use song::jsorder::{own_key_sort, pair_cmp};
use std::collections::HashSet;
use std::path::Path;

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

/// A hex colour that is not a grey: the colour of a category, where `Other` is nearly grey.
fn saturated(color: &str) -> bool {
    let Some(hex) = color.strip_prefix('#').filter(|h| h.len() == 6) else {
        return false;
    };
    let rgb: Vec<i32> = (0..3)
        .filter_map(|i| i32::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect();
    rgb.len() == 3 && rgb.iter().max().unwrap_or(&0) - rgb.iter().min().unwrap_or(&0) > 40
}

/// Counts the colours the compile gave and the lanes it moved, in the project at `project` and its document `doc`.
pub fn count_provenance(project: &str, doc: &Value, c: &mut Counts) {
    let dir = Path::new(project);
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap_or_default();
    let head = read("project.k7x");
    let mut lanes: Vec<String> = Vec::new();
    for t in doc["tracks"].as_array().into_iter().flatten() {
        if text(t, "role") != "regular" {
            continue;
        }
        let id = text(t, "id");
        let base = id.split('#').next().unwrap_or(&id).to_owned();
        if lanes.contains(&base) {
            continue;
        }
        let stated = ["nodes", "tracks"]
            .iter()
            .any(|d| read(&format!("{d}/{base}.k7x")).contains("color("));
        let color = text(t, "color");
        if !stated && !color.is_empty() {
            let name = text(t, "name");
            let parsed = parse_track_name(&name);
            let named = parsed.prefix.is_some() || category_of_text(parsed.label).is_some();
            if named {
                c.add("colour from a name", 1);
            } else if saturated(&color) {
                c.add("colour from a hint", 1);
            }
        }
        lanes.push(base);
    }
    if dir.join("chains").is_dir() {
        return;
    }
    // A graph project lists its lanes as `[base, name]` pairs sort them; the document's order is the same unless something moved a lane.
    let set: HashSet<&str> = lanes.iter().map(String::as_str).collect();
    let mut listed: Vec<String> = std::fs::read_dir(dir.join("nodes"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".k7x").map(str::to_owned))
        .filter(|b| set.contains(b.as_str()))
        .collect();
    listed.sort_by(|a, b| pair_cmp((a, &format!("{a}.k7x")), (b, &format!("{b}.k7x"))));
    own_key_sort(&mut listed);
    if listed == lanes {
        return;
    }
    let parsed: Vec<_> = lanes.iter().map(|l| parse_track_name(l)).collect();
    c.add(
        "order by a pin",
        usize::from(parsed.iter().any(|p| p.position.is_some())),
    );
    c.add(
        "order by a prefix",
        usize::from(parsed.iter().any(|p| p.prefix.is_some())),
    );
    c.add("order by trackOrder", usize::from(head.contains("trackOrder(")));
}
