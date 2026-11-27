// files_validate.rs — the editor's check: the router over the file readers, held to the cases the TypeScript router was tested on.

use song::files::{FORMAT_LEVEL, ValidityDepth, validate_file};

const MODULE: &str = "modules/audio/cab/module.k7x";

fn first(path: &str, text: &str) -> (String, String, Option<u32>, Option<u32>) {
    let v = validate_file(path, text);
    let i = v.issues.first().cloned().expect("an issue");
    (i.path, i.message, i.line, i.col)
}

#[test]
fn a_module_that_reads_is_ok_and_one_with_a_typo_names_the_key() {
    assert!(validate_file(MODULE, "extends(\"amp\")\n").ok);
    let v = validate_file(MODULE, "extends(\"amp\")\nparamz(1)\n");
    assert!(!v.ok && !v.malformed);
    assert_eq!(v.depth, ValidityDepth::Schema);
    let (path, message, line, col) = first(MODULE, "extends(\"amp\")\nparamz(1)\n");
    assert_eq!(path, "paramz");
    assert!(message.contains("paramz"), "{message}");
    assert_eq!((line, col), (Some(2), Some(1)));
}

#[test]
fn text_that_does_not_parse_is_malformed_with_its_own_position() {
    let v = validate_file(MODULE, "params({ a: 1\n");
    assert!(!v.ok && v.malformed);
    assert_eq!(v.depth, ValidityDepth::Syntax);
    assert!(v.issues[0].line.is_some() && v.issues[0].col.is_some());
    assert!(!v.issues[0].message.contains('('), "{}", v.issues[0].message);
}

#[test]
fn a_file_no_reader_claims_is_judged_by_syntax_alone() {
    let v = validate_file("songs/pearl/notes.txt", "title: Pearl\nanything: yes\n");
    assert!(v.ok);
    assert_eq!(v.depth, ValidityDepth::None);
    let v = validate_file("song/chains/gtr.yaml", "steps: [\n");
    assert!(v.ok && v.depth == ValidityDepth::None);
    let v = validate_file("song/notes.k7x", "title(\"x\")\n");
    assert!(v.ok && v.depth == ValidityDepth::Syntax);
    assert!(validate_file("song/notes.k7x", "title(\n").malformed);
}

#[test]
fn a_newer_project_is_refused_on_its_format_line_even_when_the_rest_is_garbage() {
    let next = FORMAT_LEVEL + 1;
    let v = validate_file(
        "song.k7/project.k7x",
        &format!("project({{\n  tempo: 120\n}})\nformat({next})\n"),
    );
    assert!(!v.ok && !v.malformed);
    assert_eq!(v.issues.len(), 1);
    assert!(v.issues[0].message.contains("update k7 to open it"));
    assert_eq!((v.issues[0].line, v.issues[0].path.as_str()), (Some(4), "format"));
    let v = validate_file(
        "song.k7/project.k7x",
        &format!("format({next})\nproject(@@ new syntax)\n"),
    );
    assert!(!v.ok && !v.malformed);
    assert!(v.issues[0].message.contains("newer k7"));
    assert!(
        validate_file(
            "p.k7/project.k7x",
            &format!("format({FORMAT_LEVEL})\nproject({{ tempo: 120 }})\n")
        )
        .ok
    );
    assert!(validate_file("p.k7/project.k7x", "project({ tempo: 120 })\n").ok);
    let v = validate_file("p.k7/clips/a.k7x", &format!("format({next})\n"));
    assert!(v.issues.iter().all(|i| !i.message.contains("newer k7")));
}

#[test]
fn a_sequence_written_with_a_short_form_reads_and_a_bad_note_lands_on_its_line() {
    assert!(validate_file("song/sequences/m.k7x", "notes(\"c4 d4 e4\")\nto(\"2:1\")\n").ok);
    let (path, _, line, col) = first("song/sequences/m.k7x", "// a riff\nnotes(\"c4 x9\")\n");
    assert_eq!((path.as_str(), line, col), ("notes", Some(2), Some(1)));
}

#[test]
fn chain_folders_route_as_the_router_read_them() {
    let chain = "start(true)\nsteps([\n  { id: \"l1\", track: \"gtr\" },\n  { id: \"a1\", extends: \"@k7-core/audio/chorus\" },\n  { to: [{ chain: \"MIX\" }] },\n])\n";
    let v = validate_file("song/chains/gtr.k7x", chain);
    assert!(v.ok && v.depth == ValidityDepth::Schema);
    assert!(!validate_file("chains/gtr.k7x", "steps([{ id: \"l1\", track: \"gtr\" }])\n").ok);
    assert!(!validate_file("tracks/gtr.k7x", "extends(3)\n").ok);
    assert!(validate_file("_libraries/k7-core/chains/verb/graph.k7x", "modules({})\n").ok);
    assert!(!validate_file("_libraries/k7-core/chains/verb/nodes/v.k7x", "extends(3)\n").ok);
}

#[test]
fn a_column_counts_utf16_units_not_bytes() {
    // The accent is one unit and two bytes, the emoji two units and four bytes.
    let (_, _, line, col) = first(MODULE, "// é😀\nparamz(1)\n");
    assert_eq!((line, col), (Some(2), Some(1)));
    let text = "params({ n: \"é😀\", b: 1 }); paramz(1)\n";
    let (_, _, line, col) = first(MODULE, text);
    let units = u32::try_from(text[..text.find("paramz").unwrap_or(0)].encode_utf16().count()).unwrap_or(0);
    assert_eq!((line, col), (Some(1), Some(units + 1)));
}

#[test]
fn the_small_files_of_the_desk_and_the_presets() {
    assert!(
        validate_file(
            "mix.k7x",
            "cues([\"records/a.flac\", { ref: \"records/b.flac\", markers: [1, null, 3] }])\n"
        )
        .ok
    );
    assert!(!validate_file("mix.k7x", "cues([\"/abs.flac\"])\n").ok);
    assert!(!validate_file("mix.k7x", "cues([{ ref: \"a.flac\", markers: [1,2,3,4,5,6,7,8,9] }])\n").ok);
    assert!(!validate_file("mix.k7x", "cues([{ ref: \"a.flac\", markers: [-1] }])\n").ok);
    assert!(validate_file("views/mix.k7x", "left({ module: \"a\", param: \"b\" })\nright(null)\n").ok);
    assert!(!validate_file("views/mix.k7x", "left({ module: \"\", param: \"b\" })\n").ok);
    assert!(
        validate_file(
            "_presets/k7-core/x/warm.k7x",
            "params({ gain: 0.5 })\nrack({ BD: { note: 36 } })\n"
        )
        .ok
    );
    assert!(!validate_file("_presets/k7-core/x/warm.k7x", "rack({ BD: { note: 200 } })\n").ok);
    assert!(validate_file("_libraries/k7-core/modules/a/config.k7x", "namespace(\"k7-core\")\n").ok);
    assert!(!validate_file("_libraries/k7-core/modules/a/config.k7x", "\n").ok);
    assert!(validate_file("_libraries/k7-core/modules/a/presets.k7x", "anything(1)\n").ok);
    assert!(validate_file("settings.k7x", "metronome(true)\nsomething_else(3)\n").ok);
    assert!(!validate_file("settings.k7x", "metronome_level(0.1)\n").ok);
}
