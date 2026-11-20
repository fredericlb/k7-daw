// doc_support/pictures.rs — what a picture lane's sound DID in a document, counted off the tracks and the project's files.
// A reader would get wrong that the lanes are named by the document: only the project's files say which lane is a picture lane.

use super::counts::Counts;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn names_where(dir: &Path, has: &str) -> BTreeSet<String> {
    std::fs::read_dir(dir.join("nodes"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".k7x").map(str::to_owned))
        .filter(|n| std::fs::read_to_string(dir.join(format!("nodes/{n}.k7x"))).is_ok_and(|t| t.contains(has)))
        .collect()
}

/// Counts what the sound of the picture lanes of the project at `project` did in `doc`.
pub fn count_pictures(project: &str, doc: &Value, c: &mut Counts) {
    let dir = Path::new(project);
    let pictures = names_where(dir, "@meta/picture-track");
    let linked = names_where(dir, "video_source");
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap_or_default();
    let graph = read("graph.k7x");
    let tracks = list(doc, "tracks");
    let track = |id: &str| tracks.iter().find(|t| t["id"].as_str() == Some(id));
    let mut sounded = BTreeSet::new();
    for p in &pictures {
        c.add("picture lanes", 1);
        let Some(t) = track(&format!("{p}#audio")) else {
            let shots = read(&format!("nodes/{p}.k7x")).contains("ref");
            let named = linked
                .iter()
                .any(|l| read(&format!("nodes/{l}.k7x")).contains(&format!("\"{p}\"")));
            c.add("picture lanes without sound", usize::from(shots && !named));
            continue;
        };
        c.add("picture sound tracks", 1);
        c.add(
            "picture sound tracks wired",
            usize::from(graph.contains(&format!("\"{p}#audio\""))),
        );
        c.add(
            "picture sound tracks muted",
            usize::from(t["mute"].as_bool() == Some(true)),
        );
        let clips = list(t, "clips");
        if !clips.is_empty() {
            sounded.insert(p.clone());
        }
        c.add("picture sound clips", clips.len());
        for k in clips {
            c.add("picture sound crossfades", usize::from(k.get("crossfade").is_some()));
            c.add(
                "picture sound varispeed",
                usize::from(k["warp"]["algorithm"].as_str() == Some("varispeed")),
            );
            c.add(
                "picture sound elastique",
                usize::from(k["warp"]["algorithm"].as_str() == Some("elastique")),
            );
            c.add(
                "picture sound trimmed",
                usize::from(k.get("sourceFrom").is_some() || k.get("sourceTo").is_some()),
            );
            c.add(
                "picture sound faded",
                usize::from(num(k, "fadeIn") > 0.0 || num(k, "fadeOut") > 0.0),
            );
        }
    }
    c.add("documents with picture sound", usize::from(!sounded.is_empty()));
    for lane in &linked {
        let n = track(lane).map_or(0, |t| list(t, "clips").len());
        c.add("lanes sounding a picture lane", usize::from(n > 0));
        c.add("clips of lanes sounding a picture lane", n);
    }
    let drawn = std::fs::read_dir(dir.join("timelines"))
        .into_iter()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .filter(|t| {
            sounded
                .iter()
                .any(|p| t.contains(&format!("lane(\"{p}\")")) || t.contains(&format!("\"lane\":\"{p}\"")))
        })
        .count();
    c.add("timelines drawing on a sounding picture lane", drawn);
}
