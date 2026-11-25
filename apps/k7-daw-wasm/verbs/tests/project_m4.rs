// project_m4.rs — M4's project verbs on real fixtures: time splice, master, aspect, surfaces, tags, console, tasks and libraries.
// A reader would get wrong where tasks and pins land: the WORKSPACE root, beside the project, which undo lays back too.

mod support;

use k7x::edit::Value;
use song_edit::Fs;
use support::{Project, WS, strs};

fn num_list(v: Option<&Value>) -> Vec<f64> {
    match v {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|x| if let Value::Num(n) = x { Some(*n) } else { None })
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn time_is_inserted_and_removed_under_the_events() {
    let mut p = Project::open("clip-lane");
    assert_eq!(p.starts("drums"), [0.0, 1.0, 2.0, 3.0, 4.0]);
    let ran = p.round_trip("project add 1:3:1 1:0");
    assert_eq!(ran.changed(&p.dir), ["nodes/drums.k7x"]);
    assert!(
        ran.answer.log[0].starts_with("inserted 1:0 at 1:3:1: 3 moved"),
        "{:?}",
        ran.answer.log
    );
    p.ok("project add 1:3:1 1:0");
    assert_eq!(p.starts("drums"), [0.0, 1.0, 6.0, 7.0, 8.0]);
    p.ok("project remove 1:3:1 1:0");
    assert_eq!(p.starts("drums"), [0.0, 1.0, 2.0, 3.0, 4.0]);
    p.refused("project add 1:1", "usage: project add <at> <length>");
    p.refused("project add 1:1 0:0", "a length of 0:0 moves nothing");
    p.refused("project add zz 1:0", "expects a bar:beat:sixteenth position");
    p.refused("project add 40:1 1:0", "nothing at or after 40:1 to push");
}

#[test]
fn the_master_is_written_and_unity_removes_it() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("project master -6");
    assert_eq!(ran.answer.effects, ["applyProject"]);
    assert_eq!(ran.answer.log, ["master -6 dB"]);
    p.ok("project master -6");
    assert!((p.doc().master_volume - -6.0).abs() < 1e-9);
    p.ok("project master 0");
    assert!(!p.text("project.k7x").contains("masterVolume"));
    p.refused("project master 13", "outside the fader");
    p.refused("project master loud", "expects a number");
}

#[test]
fn an_aspect_reshapes_the_viewport_keeping_its_long_edge_and_fps() {
    let mut p = Project::open("video-lane");
    let ran = p.round_trip("project aspect 9:16");
    assert_eq!(ran.answer.log, ["frame is 9:16 (720×1280)"]);
    p.ok("project aspect 9:16");
    let viewport = p
        .data("project.k7x")
        .get("project")
        .and_then(|x| x.get("viewport"))
        .cloned();
    assert_eq!(num_list(viewport.as_ref()), [720.0, 1280.0, 30.0]);
    let mut q = Project::open("mute-solo");
    q.ok("project aspect 16:9");
    let viewport = q
        .data("project.k7x")
        .get("project")
        .and_then(|x| x.get("viewport"))
        .cloned();
    assert_eq!(num_list(viewport.as_ref()), [1920.0, 1080.0]);
    q.refused("project aspect 4:3", "is not a frame shape — 16:9 or 9:16");
}

#[test]
fn a_surface_file_lists_what_it_wants_mapped() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("project surface new mastering");
    assert_eq!(ran.changed(&p.dir), ["surfaces/mastering.k7x"]);
    assert_eq!(ran.answer.effects, ["reload"]);
    p.ok("project surface new mastering");
    let text = p.text("surfaces/mastering.k7x");
    assert!(
        text.starts_with("type(\"mastering\")\nlayout(\"split\")\nmapping({\n"),
        "{text}"
    );
    assert!(
        text.contains("  \"comp.lowCut\": null, // the quick-master module"),
        "{text}"
    );
    p.doc();
    p.refused(
        "project surface new mastering master2",
        "already declares a mastering surface",
    );
    p.refused("project surface new timeline", "surface 'timeline' already exists");
    p.refused("project surface new desk", "unknown surface type 'desk'");
    p.refused("project surface new turntables bad.name", "invalid surface name");
}

#[test]
fn tags_are_kept_once_whatever_their_case() {
    let mut p = Project::open("mute-solo");
    let ran = p.round_trip("project tag  Live   Set ");
    assert_eq!(ran.answer.log, ["tags: Live Set"]);
    assert!(ran.answer.effects.is_empty(), "a tag changes nothing played");
    p.ok("project tag Live Set");
    p.ok("project tag live set");
    p.ok("project tag demo");
    let tags = p
        .data("project.k7x")
        .get("project")
        .and_then(|x| x.get("tags"))
        .cloned();
    assert_eq!(strs(tags.as_ref()), ["Live Set", "demo"]);
    assert_eq!(p.ok("project tag").answer.log, ["Live Set, demo"]);
    p.ok("project untag LIVE SET");
    p.ok("project untag demo");
    assert!(!p.text("project.k7x").contains("tags"));
    p.refused("project untag", "usage: project untag <tag>");
}

#[test]
fn a_console_must_be_a_library_system() {
    let mut p = Project::open("mute-solo");
    let ran = p.ok("project console");
    assert!(ran.answer.log[0].starts_with("no console in this account's libraries"));
    p.refused(
        "project console @k7-core/console/console-6",
        "no console '@k7-core/console/console-6'",
    );
    let ran = p.ok("project console none");
    assert_eq!(ran.answer.log, ["console: none — the lanes sum as they are"]);
}

#[test]
fn tasks_live_in_the_workspace_todos() {
    let mut p = Project::open("mute-solo");
    let todos = format!("{WS}/TODOS.md");
    let ran = p.round_trip("task add bounce   the bass");
    assert_eq!(ran.changed(&p.dir), [todos.as_str()]);
    p.ok("task add bounce   the bass");
    p.ok("task mark 3:1 fix the snare");
    let text = p.fs.text(&todos).unwrap_or_default();
    assert_eq!(
        text,
        "# Tasks\n\n- [ ] bounce the bass\n- [ ] fix the snare `mute-solo @ 3:1`\n"
    );
    p.round_trip("task done 2");
    p.ok("task done 2");
    assert!(
        p.fs.text(&todos)
            .unwrap_or_default()
            .contains("- [x] fix the snare `mute-solo @ 3:1`")
    );
    p.ok("task remove 1");
    assert_eq!(
        p.fs.text(&todos).unwrap_or_default(),
        "# Tasks\n\n- [x] fix the snare `mute-solo @ 3:1`\n"
    );
    p.refused("task done 5", "no task 5 in TODOS.md");
    p.refused("task remove 0", "usage: task remove <number>");
    p.refused("task mark nowhere fix", "expects a bar:beat:sixteenth position");
    p.refused("task add", "usage: task add <text>");
}

fn library(p: &mut Project, version: &str, requires: u32) {
    let home = song_edit::confine::Env::at("/home-none", "").libraries_home;
    let root = format!("{home}/kit/{version}");
    p.fs.mkdir_all(&format!("{root}/modules/audio/x")).expect("a folder");
    p.fs.write_atomic(
        &format!("{root}/library.k7x"),
        format!("name(\"kit\")\nversion({version})\nrequires({requires})\n").as_bytes(),
    )
    .expect("a write");
    p.fs.write_atomic(
        &format!("{root}/modules/audio/x/module.k7x"),
        b"extends(\"@engine/compressor\")\n",
    )
    .expect("a write");
}

#[test]
fn a_library_pin_moves_to_a_version_this_k7_runs() {
    let mut p = Project::open("mute-solo");
    library(&mut p, "1", 1);
    library(&mut p, "2", 2);
    library(&mut p, "3", 9);
    let ran = p.ok("library versions");
    assert_eq!(
        ran.answer.log,
        ["@kit: plays 1; here 1, 2, 3 (needs level 9) — 2 is ready: library upgrade kit"]
    );
    let pins = format!("{WS}/_library-pins.k7x");
    let ran = p.round_trip("library upgrade kit");
    assert_eq!(ran.changed(&p.dir), [pins.as_str()]);
    assert_eq!(ran.answer.log, ["@kit: 1 → 2"]);
    p.ok("library upgrade kit");
    assert_eq!(p.fs.text(&pins).unwrap_or_default(), "kit(2)\n");
    assert_eq!(p.ok("library upgrade kit 2").answer.log, ["@kit already plays 2"]);
    p.refused("library upgrade kit 3", "needs k7 level 9");
    p.refused("library upgrade kit 7", "@kit 7 is not on this machine");
    p.refused("library upgrade kit x", "'x' is no version");
    p.refused("library upgrade drums", "library upgrade: name one of kit");
}
