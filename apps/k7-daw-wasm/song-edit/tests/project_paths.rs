// project_paths.rs — the path rules of the page's projectPaths, localTargets, editorPaths and moduleBehind, with their TypeScript tests' inputs.
// A reader would get wrong the dot of `.k7x`: the page's patterns leave it unescaped, so `x_k7x` ends a document name, as sugar.rs says.

use song_edit::project_paths::*;
use std::collections::HashMap;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn which_kind_of_file_a_path_is() {
    assert!(is_rack_module_file("modules/instruments/drum_1/module.k7x"));
    // What the file tree hands over on Windows in a multi-project workspace.
    assert!(is_rack_module_file(
        "LEMO - DANSE\\modules\\instruments\\drum_1\\module.k7x"
    ));
    // `bass.k7x` beside `module.k7x` is a node; only the generic file is the rack.
    assert!(!is_rack_module_file("modules/audio/amp/bass.k7x"));
    assert!(is_seq_file("sequences/lead.k7x"));
    assert!(is_clip_file("clips/kick.k7x"));
    assert!(is_track_file("nodes/melody.k7x"));
    assert!(is_track_file("tracks/melody.k7x"));
    assert!(!is_track_file("video_tracks/melody.k7x"));
    assert!(!is_track_file("chains/melody.k7x"));
    let cam = s(&["cam"]);
    assert!(is_video_track_file("nodes/cam.k7x", &cam));
    assert!(is_video_track_file("tracks/cam.k7x", &cam));
    assert!(!is_video_track_file("nodes/melody.k7x", &cam));
}

#[test]
fn a_folder_that_merely_looks_similar_is_no_kind() {
    assert!(!is_clip_file("clips/nested/kick.k7x"));
    assert!(!is_seq_file("sequences.k7x"));
    assert!(!is_track_file("nodes/melody.wav"));
    assert!(!is_seq_file("song/sequences/melody.yaml"));
    assert!(!is_clip_file("song/clips/kick.yaml"));
    assert!(!is_rack_module_file("modules/instruments/drum_1/module.yaml"));
    assert!(!is_seq_file("xsequences/a.k7x"));
}

#[test]
fn the_dot_of_the_extension_is_any_character_but_a_line_end() {
    assert!(is_seq_file("sequences/a_k7x"));
    assert!(is_seq_file("SEQUENCES/A.K7X"));
    assert!(!is_seq_file("sequences/a\nk7x"));
    // The dot may even be the slash: `[^/]+` is `a`, the dot is `/`, and `k7x` follows.
    assert!(is_seq_file("sequences/a/k7x"));
}

#[test]
fn a_stem_loses_its_extension_and_keeps_a_dot_inside_the_name() {
    assert_eq!(file_stem("clips/kick.k7x"), "kick");
    assert_eq!(file_stem("a\\b\\my.clip.k7x"), "my.clip");
    assert_eq!(file_stem("clips/readme.txt"), "readme.txt");
}

#[test]
fn the_generic_module_file_beside_a_node() {
    assert_eq!(
        module_file_of("modules/audio/amp/bass.k7x"),
        "modules/audio/amp/module.k7x"
    );
    assert_eq!(module_file_of("bass"), "bass");
    assert_eq!(module_file_of("a\\b\\c.k7x"), "a/b/module.k7x");
}

#[test]
fn the_builder_opens_both_module_trees() {
    assert!(is_module_file("modules/audio/amp/module.k7x"));
    assert!(is_module_file("_modules/audio/amp/ui.k7x"));
    assert!(is_module_file("song.k7/modules/amp/module.k7x"));
    assert!(!is_module_file("modules/audio/amp/bass.k7x"));
    assert!(!is_module_file("xmodules/audio/module.k7x"));
    assert!(is_own_module_file("modules/audio/amp/module.k7x"));
    assert!(is_own_module_file("module.k7x"));
    assert!(!is_own_module_file("a\\module.k7x"));
    assert!(!is_own_module_file("clips/xmodule.k7x"));
}

#[test]
fn an_entity_file_is_a_lookup_not_a_search() {
    let files = s(&[
        "tracks/kick.k7x",
        "clips/kick.k7x",
        "tracks/ge.lead.k7x",
        "video_tracks/cam1.k7x",
    ]);
    assert_eq!(
        entity_file(&files, "tracks", "kick").as_deref(),
        Some("tracks/kick.k7x")
    );
    assert_eq!(entity_file(&files, "clips", "kick").as_deref(), Some("clips/kick.k7x"));
    assert_eq!(
        entity_file(&files, "tracks", "ge.lead").as_deref(),
        Some("tracks/ge.lead.k7x")
    );
    assert_eq!(entity_file(&s(&["tracks/snare.yaml"]), "tracks", "snare"), None);
    assert_eq!(
        entity_file(&s(&["band/winter.k7/tracks/kick.k7x"]), "tracks", "kick").as_deref(),
        Some("band/winter.k7/tracks/kick.k7x")
    );
    // The id's dots are dots, not any character.
    assert_eq!(entity_file(&s(&["tracks/geXlead.k7x"]), "tracks", "ge.lead"), None);
    assert_eq!(entity_file(&files, "tracks", "snare"), None);
    // The `i` flag folds case, accents included.
    assert_eq!(
        entity_file(&s(&["clips/Été.k7x"]), "clips", "été").as_deref(),
        Some("clips/Été.k7x")
    );
}

#[test]
fn the_key_a_rack_files_under() {
    assert_eq!(
        rack_key_of("modules/instruments/drum_1/module.k7x").as_deref(),
        Some("drum_1")
    );
    assert_eq!(
        rack_key_of("song.k7/modules/instruments/drum_1/module.k7x").as_deref(),
        Some("drum_1")
    );
    assert_eq!(
        rack_key_of("nodes/da_main-drum_1.k7x").as_deref(),
        Some("da_main-drum_1")
    );
    assert_eq!(
        rack_key_of("song.k7/nodes/da_main-drum_1.k7x").as_deref(),
        Some("da_main-drum_1")
    );
    assert_eq!(rack_key_of("clips/kick.k7x"), None);
    assert_eq!(rack_key_of("chains/da.main.k7x"), None);
    assert_eq!(rack_key_of("modules/instruments/drum_1/panel.k7x"), None);
}

#[test]
fn a_file_of_another_project_is_kept_out() {
    assert!(in_active_project("song.k7/clips/a.k7x", Some("song.k7")));
    assert!(!in_active_project("other.k7/clips/a.k7x", Some("song.k7")));
    assert!(!in_active_project("song.k7-old/clips/a.k7x", Some("song.k7")));
    assert!(in_active_project("clips/a.k7x", Some(".")));
    assert!(!in_active_project("song.k7/clips/a.k7x", None));
    assert!(in_active_project("song.k7\\clips\\a.k7x", Some("song.k7")));
}

#[test]
fn the_project_folder_and_the_paths_relative_to_it() {
    assert_eq!(project_dir(Some("songs\\first.k7\\")), "songs/first.k7");
    assert_eq!(project_dir(Some(".")), "");
    assert_eq!(project_dir(None), "");
    assert_eq!(to_project_relative("a.k7\\clips\\x.k7x", Some("a.k7")), "clips/x.k7x");
    assert_eq!(
        to_project_relative("a.k7x/clips/x.k7x", Some("a.k7")),
        "a.k7x/clips/x.k7x"
    );
    assert_eq!(to_project_relative("clips/x.k7x", Some(".")), "clips/x.k7x");
    assert_eq!(to_project_relative("a.k7", Some("a.k7")), "");
}

#[test]
fn a_parent_a_base_name_and_the_head_of_a_line() {
    assert_eq!(parent_dir("clips/takes/kick.wav"), "clips/takes");
    assert_eq!(parent_dir("kick.wav"), "");
    assert_eq!(base_name("clips/takes/kick.wav"), "kick.wav");
    assert_eq!(base_name("clips/takes/"), "takes");
    assert_eq!(
        head_tokens("clips/kick.wav the loud one.wav", 2),
        s(&["clips/kick.wav", "the loud one.wav"])
    );
    assert_eq!(head_tokens("clips/kick.wav", 2), s(&["clips/kick.wav"]));
    assert!(head_tokens("   ", 2).is_empty());
    assert_eq!(head_tokens("a\u{a0}b c", 3), s(&["a", "b", "c"]));
}

#[test]
fn a_gutter_source_resolves_where_the_compiler_puts_it() {
    let rack = "modules/instruments/drum_1/module.k7x";
    assert_eq!(
        resolve_gutter_source("kick.wav", rack, None),
        "modules/instruments/drum_1/kick.wav"
    );
    assert_eq!(resolve_gutter_source("kick.wav", "clips/a.k7x", None), "kick.wav");
    assert_eq!(resolve_gutter_source("/abs/kick.wav", rack, None), "/abs/kick.wav");
    assert_eq!(resolve_gutter_source("C:\\kick.wav", rack, None), "C:\\kick.wav");
    assert_eq!(resolve_gutter_source("", rack, None), "");
    assert_eq!(
        resolve_gutter_source("k.wav", "song.k7/modules/a/b/module.k7x", Some("song.k7")),
        "modules/a/b/k.wav"
    );
}

#[test]
fn the_module_beside_an_instance() {
    let paths: HashMap<String, String> = [
        ("base:audio/amp".to_owned(), "modules/amp/module.k7x".to_owned()),
        ("module:eq_dj".to_owned(), "modules/eq_dj/module.k7x".to_owned()),
    ]
    .into();
    let behind = |text: &str, own: Option<&str>| module_behind(text, &paths, "/root/song.k7", own);
    assert_eq!(
        behind("extends(\"audio/amp\")\nparams({})\n", None).as_deref(),
        Some("/root/song.k7/modules/amp/module.k7x")
    );
    assert_eq!(
        behind("extends(\"audio/amp\")\nparams({\n", None).as_deref(),
        Some("/root/song.k7/modules/amp/module.k7x")
    );
    assert_eq!(
        behind("extends(\"eq_dj\")\n", None).as_deref(),
        Some("/root/song.k7/modules/eq_dj/module.k7x")
    );
    assert_eq!(behind("extends(\"@meta/track\")\n", None), None);
    assert_eq!(behind("source(\"kick.wav\")\n", None), None);
    assert_eq!(
        behind("extends(\"audio/amp\")\n", Some("/root/song.k7/modules/amp/module.k7x")),
        None
    );
    assert_eq!(behind("extends(\"audio/amp\")\n", Some("modules/amp/module.k7x")), None);
}

#[test]
fn a_clips_kind_comes_off_its_source() {
    assert!(is_audio_clip_file("source(\"clips/kick.wav\")\n"));
    assert!(!is_audio_clip_file("source(\"clips/shot.MP4\")\n"));
    assert!(!is_audio_clip_file("gain(1)\n"));
    assert_eq!(find_use_rack("use_rack(\"drums\")\n").as_deref(), Some("drums"));
    assert_eq!(find_use_rack("notes([\n"), None);
}
