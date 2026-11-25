// complete.rs — the command line's completion over fixture projects: verb names, each provider, partial words, a settled command.
// A reader would get wrong that an item is the whole line: `track mute drums`, not `drums`.

mod support;

use song_edit::Fs;
use support::{META, Project};
use verbs::Item;

fn ask(p: &Project, line: &str) -> Vec<Item> {
    let env = song_edit::confine::Env::at("/home-none", META);
    verbs::complete_in_env(&p.fs, &mut song::Cache::new(), env, &p.dir, line)
}

fn lines(p: &Project, line: &str) -> Vec<String> {
    ask(p, line).into_iter().map(|i| i.line).collect()
}

fn put(p: &mut Project, path: &str, text: &str) {
    p.fs.mkdir_all(path.rsplit_once('/').map_or("", |(d, _)| d))
        .expect("a folder");
    p.fs.write_atomic(path, text.as_bytes()).expect("a file");
}

#[test]
fn verb_names_complete_by_prefix_whatever_the_case() {
    let p = Project::open("clip-lane");
    let got = lines(&p, "track m");
    assert!(got.contains(&"track mute".to_owned()));
    assert!(got.iter().all(|l| l.starts_with("track m")));
    assert_eq!(lines(&p, "TRACK MU"), lines(&p, "track mu"));
    assert_eq!(
        lines(&p, "  track mu"),
        lines(&p, "track mu"),
        "a leading space is not read"
    );
    assert!(lines(&p, "zzz").is_empty());
}

#[test]
fn a_settled_command_is_not_offered_again() {
    let p = Project::open("clip-lane");
    assert!(!lines(&p, "track mute").contains(&"track mute".to_owned()));
    let all = lines(&p, "event");
    assert!(all.contains(&"event move".to_owned()) && all.contains(&"events move".to_owned()));
}

#[test]
fn the_longest_command_picks_the_arguments() {
    let p = Project::open("clip-lane");
    // `event loop points` takes a track first, as `event loop` does: the longer word is the one read.
    assert_eq!(lines(&p, "event loop points "), ["event loop points drums"]);
    assert_eq!(lines(&p, "event loop "), ["event loop drums"]);
}

#[test]
fn tracks_complete_by_partial_word_and_after_a_full_command() {
    let p = Project::open("clip-lane");
    assert_eq!(lines(&p, "track mute "), ["track mute drums"]);
    assert_eq!(lines(&p, "track mute dr"), ["track mute drums"]);
    assert_eq!(lines(&p, "track mute DR"), ["track mute drums"], "case never decides");
    assert!(lines(&p, "track mute x").is_empty());
    // The argument after the last declared one has nothing to offer, and a typed one has no provider.
    assert!(lines(&p, "track mute drums ").is_empty());
    assert!(lines(&p, "event move drums ").is_empty());
}

#[test]
fn an_earlier_word_is_echoed_and_the_next_argument_is_the_one_asked() {
    let p = Project::open("markers-settings");
    assert_eq!(
        lines(&p, "track order drums 1 "),
        Vec::<String>::new(),
        "a position is typed, not offered"
    );
    assert_eq!(
        lines(&p, "project marker delete "),
        ["project marker delete intro", "project marker delete chorus"]
    );
    assert_eq!(lines(&p, "project marker delete ch"), ["project marker delete chorus"]);
}

#[test]
fn nodes_are_the_modules_a_project_uses_and_modules_add_the_inherited_ones() {
    let p = Project::open("curves");
    assert_eq!(lines(&p, "node bypass "), ["node bypass lead"]);
    let modules = lines(&p, "node automate ");
    assert_eq!(modules[0], "node automate lead");
    assert!(modules.len() > 1, "{modules:?}");
    assert!(
        modules[1..].iter().all(|l| l.starts_with("node automate @")),
        "{modules:?}"
    );
    assert!(
        modules.iter().all(|l| !l.contains("@engine/")),
        "a scanned plugin is browsed, not offered"
    );
}

#[test]
fn a_chain_projects_node_is_its_uuid_with_its_module_and_place_as_the_label() {
    let p = Project::open_crate("chain-lanes");
    let items = ask(&p, "node bypass ");
    assert!(!items.is_empty());
    let one = items
        .iter()
        .find(|i| i.line.ends_with("d2"))
        .expect("the drum lane's compressor");
    assert_eq!(one.line, "node bypass 00000000-0000-4000-8000-0000000000d2");
    assert_eq!(
        one.label.as_deref(),
        Some("@workspace/audio/compressor · drums · step 2")
    );
    // A lane has a name of its own and no label.
    assert!(items.iter().all(|i| i.label.as_deref() != Some("")));
}

#[test]
fn node_new_offers_its_sources_and_the_category_folders() {
    let p = Project::open("clip-lane");
    let got = lines(&p, "node new ");
    assert!(got.contains(&"node new io/MIDI".to_owned()) && got.contains(&"node new io/FILE".to_owned()));
    assert!(!got.contains(&"node new io/OUT".to_owned()), "an output is no source");
    assert!(
        got.contains(&"node new audio/".to_owned()),
        "a folder leaves you at the name"
    );
    assert_eq!(lines(&p, "node new au"), ["node new audio/"]);
    // The module being extended is the third word, off the base modules.
    let third = lines(&p, "node new audio/x lead ");
    assert!(
        third.iter().all(|l| l.starts_with("node new audio/x lead ")),
        "{third:?}"
    );
}

#[test]
fn module_verbs_offer_the_folders_and_the_scanned_plugins_apart() {
    let p = Project::open("clip-lane");
    assert!(lines(&p, "module new ").contains(&"module new instruments/".to_owned()));
    let own = lines(&p, "module own ");
    assert!(own.len() > 10 && own.iter().all(|l| l.starts_with("module own @engine/")));
    let base = lines(&p, "module fork ");
    assert!(base.iter().all(|l| !l.contains("@engine/")));
}

#[test]
fn audio_clips_then_audio_files_make_the_clip_targets() {
    let p = Project::open("clip-lane");
    let got = lines(&p, "clip process ");
    let kick = got.iter().position(|l| l == "clip process kick@0").expect("a clip");
    let file = got
        .iter()
        .position(|l| l == "clip process clips/kick.wav")
        .expect("a file");
    assert!(kick < file, "{got:?}");
    assert!(
        !got.iter().any(|l| l == "clip process drums"),
        "a track is for `track process`"
    );
    assert_eq!(lines(&p, "clip process clips/s"), ["clip process clips/snare.wav"]);
}

#[test]
fn sequences_and_curves_come_off_their_folders() {
    let mut p = Project::open("curves");
    assert_eq!(lines(&p, "sequence note new "), ["sequence note new melody"]);
    let had = lines(&p, "curve point add ");
    assert!(!had.is_empty(), "the fixture holds curves");
    p.ok("curve new zigzag");
    let now = lines(&p, "curve point add ");
    assert_eq!(now.len(), had.len() + 1);
    assert!(now.contains(&"curve point add zigzag".to_owned()));
    assert_eq!(lines(&p, "curve point add zig"), ["curve point add zigzag"]);
}

#[test]
fn timelines_lanes_and_targets() {
    let mut p = Project::open("clip-lane");
    p.ok("timeline new verse");
    p.ok("timeline new chorus");
    let mut both = lines(&p, "timeline length ");
    both.sort();
    assert_eq!(both, ["timeline length chorus", "timeline length verse"]);
    let targets = lines(&p, "timeline lane copy drums ");
    assert_eq!(
        targets[0], "timeline lane copy drums main",
        "main first, then each timeline"
    );
    assert_eq!(targets.len(), 3);
    // A timeline's lane is a track too, addressed `<timeline>><lane>`.
    p.ok("timeline lane copy drums verse");
    assert!(
        lines(&p, "track mute ")
            .iter()
            .any(|l| l.starts_with("track mute verse>"))
    );
}

#[test]
fn picture_lanes_come_off_the_graph() {
    let p = Project::open("video-lane");
    assert_eq!(lines(&p, "picture mute "), ["picture mute main"]);
    assert!(lines(&p, "picture mute x").is_empty());
}

#[test]
fn engines_alternatives_and_samples() {
    let mut p = Project::open("rack");
    assert!(
        lines(&p, "node alternative select kit ")
            .iter()
            .all(|l| l.starts_with("node alternative select kit "))
    );
    p.ok("node alternative copy kit bright");
    assert_eq!(
        lines(&p, "node alternative select kit "),
        ["node alternative select kit bright"]
    );
    assert!(
        lines(&p, "node engine kit ")
            .iter()
            .all(|l| l.starts_with("node engine kit "))
    );
    put(&mut p, "/home-none/libraries/k7-core/samples/kick.wav", "RIFF");
    let samples = lines(&p, "node tap sample kit hat ");
    assert!(samples.is_empty() || samples.iter().all(|l| l.starts_with("node tap sample kit hat ")));
    let all = lines(&p, "node tap sample kit snare ");
    assert!(
        all.contains(&"node tap sample kit snare @k7-core/kick.wav".to_owned()),
        "{all:?}"
    );
}

#[test]
fn libraries_chains_and_assembly_templates_come_off_the_machine() {
    let mut p = Project::open("note-lane");
    put(&mut p, "/home-none/libraries/k7-core/chains/bass/di.k7x", "steps([])\n");
    put(
        &mut p,
        "/home-none/libraries/k7-core/chains/warm/graph.k7x",
        "modules({})\n",
    );
    put(
        &mut p,
        "/home-none/libraries/k7-core/assemblies/duo/assembly.k7x",
        "label(\"Duo\")\n",
    );
    put(
        &mut p,
        "/home-none/libraries/mine/assemblies/deep/trio/assembly.k7x",
        "label(\"Trio\")\n",
    );
    assert_eq!(
        lines(&p, "library upgrade "),
        ["library upgrade k7-core", "library upgrade mine"]
    );
    let chains = lines(&p, "track process melody ");
    assert!(
        chains.contains(&"track process melody @k7-core/bass/di".to_owned()),
        "{chains:?}"
    );
    assert!(
        chains.contains(&"track process melody @k7-core/warm".to_owned()),
        "a folder with a graph is a chain"
    );
    assert_eq!(
        lines(&p, "assembly new "),
        ["assembly new @k7-core/duo", "assembly new @mine/deep/trio"]
    );
    assert_eq!(lines(&p, "assembly new @m"), ["assembly new @mine/deep/trio"]);
    assert!(lines(&p, "assembly delete ").is_empty(), "no box declared yet");
}

#[test]
fn playlists_are_the_workspace_set_lists() {
    let mut p = Project::open("clip-lane");
    assert!(lines(&p, "playlist song add ").is_empty());
    p.ok("playlist new gig");
    p.ok("playlist new live/rehearsal");
    assert_eq!(
        lines(&p, "playlist song add "),
        ["playlist song add gig", "playlist song add live/rehearsal"]
    );
    assert_eq!(lines(&p, "playlist song add li"), ["playlist song add live/rehearsal"]);
}

#[test]
fn surface_types_are_the_builtin_four() {
    let p = Project::open("clip-lane");
    assert_eq!(
        lines(&p, "project surface new t"),
        ["project surface new turntables", "project surface new timeline"]
    );
}

#[test]
fn a_literal_set_answers_for_itself() {
    let p = Project::open("clip-lane");
    let got = lines(&p, "system channel type in ");
    assert!(got.iter().all(|l| l.starts_with("system channel type in ")));
}

#[test]
fn a_project_that_does_not_load_offers_nothing_for_its_values() {
    let p = Project::open("broken-unresolved");
    assert!(lines(&p, "node bypass ").is_empty());
    assert!(!lines(&p, "node ").is_empty(), "the command words need no project");
}

#[test]
fn the_cursor_cuts_the_line_and_counts_utf16_units() {
    let p = Project::open("clip-lane");
    let env = serde_json::json!({ "studioHome": "/home-none", "metaRoot": META });
    let req = |line: &str, cursor: Option<usize>| verbs::CompleteRequest {
        line: line.to_owned(),
        cursor,
        project: p.dir.clone(),
        env: env.clone(),
    };
    let run = |r: verbs::CompleteRequest| verbs::complete_request(&p.fs, &mut song::Cache::new(), &r);
    let at = run(req("track mute dr zzz", Some(13)));
    assert_eq!(
        at.iter().map(|i| i.line.as_str()).collect::<Vec<_>>(),
        ["track mute drums"]
    );
    assert_eq!(run(req("track mute dr", None)), at);
    // An astral character is two units, and a cursor between them cuts before it.
    assert!(run(req("track mute \u{1d11e}", Some(13))).is_empty());
    assert_eq!(run(req("track mute \u{1d11e}", Some(12))).len(), 1);
}

#[test]
fn a_kept_cache_answers_the_same_as_a_fresh_one() {
    let p = Project::open("curves");
    let env = || song_edit::confine::Env::at("/home-none", META);
    let mut cache = song::Cache::new();
    let first = verbs::complete_in_env(&p.fs, &mut cache, env(), &p.dir, "node bypass ");
    let second = verbs::complete_in_env(&p.fs, &mut cache, env(), &p.dir, "node bypass ");
    assert_eq!(first, second);
    assert_eq!(first, ask(&p, "node bypass "));
}
