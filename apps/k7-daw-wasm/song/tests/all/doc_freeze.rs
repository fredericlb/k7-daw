// doc_freeze.rs — what a freeze does to the document (decisions 334 to 348, 362): freezeCompile.test.ts over a manual freeze, then the automatic ones.
// A reader would get wrong that an automatic render's name is the hash of the native JSON: a test reads the name the compile gives and creates that file.

use crate::doc_support;
use crate::support;

use doc_support::{compiled_with, laid, lane, roots, world_request};
use song::Cache;
use song::doc::{Compiled, ProjectDoc, TrackDoc, Vsts, compile};
use song::files::Severity;
use support::mem;

const VST: &str = "plugin({ type: \"vst3\", name: \"X\" })\n";

fn files(frozen: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![
        ("project.k7x".into(), "project({ name: \"f\" })\n".into()),
        ("clips/riff.k7x".into(), "source(\"clips/riff.wav\")\nto(\"2:1\")\n".into()),
        ("clips/riff.wav".into(), String::new()),
        ("fr/r.wav".into(), String::new()),
        (
            "graph.k7x".into(),
            "modules({ gtr: [\"amp\"], amp: [\"cab\"], cab: [\"verb\"], keys: [\"verb\"], verb: [\"plate\"], plate: [\"OUT\"] })\n".into(),
        ),
    ];
    for lane_name in ["gtr", "keys"] {
        out.push((
            format!("nodes/{lane_name}.k7x"),
            lane("events([{ ref: \"riff\", start: \"1:1:1\" }])\n"),
        ));
    }
    for m in ["amp", "cab", "verb", "plate"] {
        let frozen_line = frozen
            .iter()
            .find(|(n, _)| *n == m)
            .map_or(String::new(), |(_, r)| format!("frozen(\"{r}\")\n"));
        out.push((format!("nodes/{m}.k7x"), format!("{VST}{frozen_line}")));
    }
    out
}

fn run(frozen: &[(&str, &str)]) -> ProjectDoc {
    run_all(frozen).engine.expect("a document")
}

fn run_all(frozen: &[(&str, &str)]) -> Compiled {
    let f = files(frozen);
    let refs: Vec<(&str, &str)> = f.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    compiled_with(&refs, &serde_json::json!({}), Vsts::None)
}

fn track<'a>(d: &'a ProjectDoc, id: &str) -> &'a TrackDoc {
    d.tracks.iter().find(|t| t.id == id).expect("a track")
}

fn modules(t: &TrackDoc) -> Vec<String> {
    let mut v: Vec<String> = t
        .plugins
        .iter()
        .map(|p| p.id.split_once(':').map_or(p.id.clone(), |x| x.1.to_owned()))
        .collect();
    v.sort();
    v
}

// freezeCompile.test.ts "plays the render on that lane, and stops building what it contains" (334, 335).
#[test]
fn a_freeze_inside_a_run_plays_the_render_and_builds_what_is_downstream() {
    let d = run(&[("amp", "fr/r.wav")]);
    let gtr = track(&d, "gtr");
    assert_eq!(
        gtr.clips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["gtr__frozen"]
    );
    assert!(gtr.clips[0].source.ends_with("r.wav"));
    assert_eq!(modules(gtr), ["cab"]);
    assert_eq!(
        gtr.frozen_material
            .as_ref()
            .map(|m| m.iter().map(|c| c.name.as_str()).collect::<Vec<_>>()),
        Some(vec!["riff"])
    );
    assert_eq!(track(&d, "keys").clips.len(), 1);
    assert!(track(&d, "keys").frozen_material.is_none());
    assert_eq!(modules(track(&d, "verb")), ["plate", "verb"]);
}

// freezeCompile.test.ts "plays live again the moment the render is not on disk".
#[test]
fn a_render_that_is_not_there_plays_live() {
    let d = run(&[("amp", "fr/gone.wav")]);
    let gtr = track(&d, "gtr");
    assert_eq!(gtr.clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["riff"]);
    assert!(gtr.frozen_material.is_none());
    assert_eq!(modules(gtr), ["amp", "cab"]);
}

// freezeCompile.test.ts "silences every lane that met there and plays the render on the return track".
#[test]
fn a_freeze_at_a_bus_silences_its_lanes_and_plays_on_the_return() {
    let d = run(&[("verb", "fr/r.wav")]);
    assert!(track(&d, "gtr").clips.is_empty() && track(&d, "keys").clips.is_empty());
    assert_eq!(track(&d, "keys").frozen_material.as_ref().map(Vec::len), Some(1));
    assert!(modules(track(&d, "gtr")).is_empty());
    let bus = track(&d, "verb");
    assert_eq!(
        bus.clips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["verb__frozen"]
    );
    assert_eq!(modules(bus), ["plate"]);
}

// 335: a render outside the mounts is not in force, with one WARNING on the module; a render that is a folder counts.
#[test]
fn a_render_outside_is_a_warning_and_a_folder_counts() {
    let c = run_all(&[("amp", "/etc/passwd")]);
    assert!(
        c.diagnostics
            .iter()
            .any(|f| f.file == "nodes/amp.k7x" && f.severity == Severity::Warning)
    );
    let d = c.engine.expect("a document");
    assert_eq!(
        track(&d, "gtr")
            .clips
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["riff"]
    );
    let mut f = files(&[("amp", "fr/dir")]);
    f.push(("fr/dir/x.wav".into(), String::new()));
    let refs: Vec<(&str, &str)> = f.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = compiled_with(&refs, &serde_json::json!({}), Vsts::None)
        .engine
        .expect("a document");
    assert_eq!(track(&d, "gtr").clips[0].id, "gtr__frozen");
}

/// The project's files as a memory source, with extra ones, compiled with `thawed`.
fn auto(extra: &[(String, String)], vsts: Vsts, thawed: &[&str]) -> Compiled {
    let mut f = files(&[]);
    f.extend(extra.iter().cloned());
    let owned = laid(&f.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect::<Vec<_>>());
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    compile(
        &src,
        &mut Cache::new(),
        &world_request(&serde_json::json!({}), vsts, thawed),
    )
    .expect("a compile")
}

// 340, 346, 347: the last VST of the run is the point; its render is not ready until the file the compile names is there.
#[test]
fn an_automatic_freeze_is_listed_and_plays_once_its_render_is_made() {
    let first = auto(&[], Vsts::None, &[]);
    assert_eq!(first.freezes.len(), 1, "one point: the last of two runs into one bus");
    let f = &first.freezes[0];
    assert_eq!(f.module, "plate");
    assert!(!f.ready && first.renders_used.is_empty());
    let name = f.render.strip_prefix("/p/").expect("under the memory world").to_owned();
    let second = auto(&[(name, String::new())], Vsts::None, &[]);
    assert!(second.freezes[0].ready && second.freezes[0].plays_render());
    assert_eq!(second.renders_used, [second.freezes[0].render.clone()]);
    let d = second.engine.expect("a document");
    assert!(
        track(&d, "verb").clips.iter().any(|c| c.id == "plate__frozen")
            || d.tracks
                .iter()
                .any(|t| t.clips.iter().any(|c| c.id.ends_with("__frozen")))
    );
    assert_eq!(roots().renders.as_deref(), Some("/p/renders"));
}

// 303: a thawed module is left live and has no freeze.
#[test]
fn a_thawed_module_has_no_automatic_freeze() {
    let c = auto(&[], Vsts::None, &["plate"]);
    assert!(c.freezes.iter().all(|f| f.module != "plate"));
}

// 346: with no render cache there is no automatic freeze at all.
#[test]
fn with_no_renders_folder_there_is_no_automatic_freeze() {
    let f = files(&[]);
    let refs: Vec<(&str, &str)> = f.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let owned = laid(&refs);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = mem(&refs);
    let mut req = world_request(&serde_json::json!({}), Vsts::None, &[]);
    req.roots.renders = None;
    assert!(
        compile(&src, &mut Cache::new(), &req)
            .expect("a compile")
            .freezes
            .is_empty()
    );
}

// 344: an automatic render's name differs by what the point contains: another preset byte, another name.
#[test]
fn the_render_name_follows_the_content_of_the_cut() {
    let a = auto(&[], Vsts::None, &[]);
    let b = auto(
        &[("nodes/amp.k7x".into(), format!("{VST}params({{ gain: 3 }})\n"))],
        Vsts::None,
        &[],
    );
    assert_ne!(a.freezes[0].render, b.freezes[0].render);
    assert_eq!(a.freezes[0].render, auto(&[], Vsts::None, &[]).freezes[0].render);
}
