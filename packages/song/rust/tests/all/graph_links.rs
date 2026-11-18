// graph_links.rs — the hostile table's link rows, on the disk Source: a loop, a folder a link takes out of the mounts, an amp model behind one.
// A reader would get wrong that a link is a kind of its own: the Source lists it as what it points at, and refuses it as `Outside` past the mounts.

use crate::support;

use serde_json::Value;
use song::graph::{Handed, load_web_graph};
use song::lanes::rig::Rig;
use song::{Cache, Severity};
use std::path::Path;
use support::{Tmp, disk, roots, skip};

/// Makes a directory link; false when this machine's rights do not allow it.
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(target, link).is_ok();
    #[cfg(not(any(unix, windows)))]
    return false;
}

fn link_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(target, link).is_ok();
    #[cfg(not(any(unix, windows)))]
    return false;
}

const AMP: &str = "plugin({ type: \"internal\", name: \"Amp\" })\n";
const GRAPH: &str = "modules({ x: [\"OUT\"] })\n";

/// The project of a node `x` over the temporary folder, drawn through the disk Source with every mount `roots` names.
fn drawn(t: &Tmp) -> (Value, Vec<song::Finding>) {
    let roots = roots(t);
    let a = fast_draw(&disk(&roots), &roots);
    (serde_json::to_value(&a.graph).expect("json"), a.findings)
}

fn fast_draw(source: &dyn song::Source, roots: &song::Roots) -> song::graph::Answer {
    let t = std::time::Instant::now();
    let a = load_web_graph(source, &mut Cache::new(), roots, Rig::default(), &Handed::default()).expect("graph");
    let bound = std::time::Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });
    assert!(t.elapsed() < bound, "took {:?}", t.elapsed());
    a
}

fn strings(v: &Value, ptr: &str) -> Vec<String> {
    v.pointer(ptr)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

// 422: a link back to its own folder under samples/ is walked to the depth of six and stops there.
#[test]
fn a_link_loop_under_samples_is_walked_to_depth_six_and_ends() {
    let t = Tmp::new("links-loop");
    t.write("ws/proj/graph.k7x", GRAPH);
    t.write(
        "ws/proj/nodes/x.k7x",
        "plugin({ type: \"internal\", name: \"Sampler\" })\ntype(\"rack\")\nrack({ rack_type: \"samples\", symbols: {} })\n",
    );
    t.write("lib/k7-core/samples/a.wav", "x");
    if !link_dir(&t.0.join("lib/k7-core/samples"), &t.0.join("lib/k7-core/samples/loop")) {
        return skip("a_link_loop_under_samples", "no directory link on this machine");
    }
    let (j, _) = drawn(&t);
    let choices = strings(&j, "/chains/x/use/0/assetChoices");
    let deepest = format!("@k7-core/{}a.wav", "loop/".repeat(6));
    assert_eq!(choices.len(), 7, "{choices:?}");
    assert!(choices.contains(&deepest), "{choices:?}");
}

// 423: a folder a link takes out of the mounts lists nothing and warns once; the files beside it are offered.
#[test]
fn a_folder_a_link_makes_outside_is_empty_with_one_warning() {
    let t = Tmp::new("links-outside");
    t.write("ws/proj/graph.k7x", GRAPH);
    t.write("ws/proj/nodes/x.k7x", AMP);
    t.write("ws/proj/nodes/assets/ok.nam", "x");
    t.write("elsewhere/leak.nam", "x");
    if !link_dir(&t.0.join("elsewhere"), &t.0.join("ws/proj/nodes/assets/out")) {
        return skip("a_folder_a_link_makes_outside", "no directory link on this machine");
    }
    let (j, findings) = drawn(&t);
    assert_eq!(strings(&j, "/chains/x/use/0/assetChoices"), ["ok.nam"]);
    let warnings: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("outside the project"))
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].severity, Severity::Warning);
}

// 425: a model behind a link out of the mounts is none, not a failure and not a read.
#[test]
fn an_amp_model_behind_a_link_out_of_the_mounts_is_none() {
    let t = Tmp::new("links-amp");
    t.write("ws/proj/graph.k7x", GRAPH);
    t.write(
        "ws/proj/nodes/x.k7x",
        &format!("{AMP}params({{ model: \"net.json\" }})\n"),
    );
    t.write("elsewhere/net.json", "{\"layers\":[],\"in_shape\":[1,2]}");
    t.mkdir("ws/proj/nodes/assets");
    if !link_file(
        &t.0.join("elsewhere/net.json"),
        &t.0.join("ws/proj/nodes/assets/net.json"),
    ) {
        return skip("an_amp_model_behind_a_link", "no file link on this machine");
    }
    let (j, _) = drawn(&t);
    assert!(j.pointer("/chains/x/use/0").is_some());
    assert_eq!(j.pointer("/chains/x/use/0/ampModel"), None);
}
