// views_surfaces.rs — a surface file decoded and a project's surfaces read, after surfaces.test.ts and the hostile table.
// A reader would get wrong that an entry of `null` is a value and `mapping: null` is refused; decisions 601 to 605.

use crate::support;
use crate::views_world;

use serde_json::{Value, json};
use song::views::nullable::Nullable;
use song::views::surfaces::{SurfaceFile, SurfaceLayout, SurfaceType, decode_surface, project_surfaces};
use song::{Cache, Severity};
use std::time::{Duration, Instant};
use support::mem;

fn bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

fn fast<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(t.elapsed() < bound(), "{what} took {:?}", t.elapsed());
    out
}

fn surface(kind: &str, layout: Option<&str>, rest: &str) -> String {
    let layout = layout.map_or(String::new(), |l| format!("layout({l:?})\n"));
    format!("type({kind:?})\n{layout}{rest}")
}

const DESK: &str = "type(\"turntables\")\nlayout(\"split\")\nmapping({\n  crossfade: { module: \"crossfade\", param: \"Mix\" },\n  \"deckA.tempo\": { module: \"tempoA\", param: \"Target BPM\" },\n})\n";

fn read(text: &str) -> Result<SurfaceFile, song::Finding> {
    decode_surface("surfaces/x.k7x", text)
}

#[test]
fn reads_the_type_the_layout_and_the_mapping() {
    // surfaces.test.ts: parseSurface reads the type, the layout and the mapping (602).
    let s = read(DESK).expect("reads");
    assert_eq!((s.kind, s.layout), (SurfaceType::Turntables, SurfaceLayout::Split));
    let m = s.mapping.expect("a mapping");
    assert_eq!(m["crossfade"].0.as_ref().map(|b| b.param.as_str()), Some("Mix"));
}

#[test]
fn accepts_a_surface_with_no_mapping_and_no_playlist() {
    // surfaces.test.ts: accepts a surface with NO mapping at all (602).
    let s = read(&surface("turntables", Some("split"), "")).expect("reads");
    assert!(s.mapping.is_none() && s.playlist.is_none());
}

#[test]
fn reads_the_playlist_of_a_desk() {
    // surfaces.test.ts: reads the PLAYLIST a desk draws its browser from (602).
    let s = read(&surface("turntables", Some("split"), "playlist(\"ete-2026\")\n")).expect("reads");
    assert_eq!(s.playlist.as_deref(), Some("ete-2026"));
}

#[test]
fn refuses_a_playlist_where_there_is_no_song_browser() {
    // surfaces.test.ts: refuses a playlist on a surface with no song browser (602); raised on the playlist key.
    let f = read(&surface("timeline", Some("split"), "playlist(\"ete-2026\")\n")).expect_err("refused");
    assert!(f.message.contains("turntables"), "{}", f.message);
    assert_eq!(f.line, 3);
}

#[test]
fn refuses_stems_on_a_surface_that_loads_none() {
    // decision 602: the stems refinement, after the playlist's, raised on `stems`.
    let f = read(&surface("mastering", Some("split"), "stems(true)\n")).expect_err("refused");
    assert!(f.message.contains("separates a record") && f.line == 3);
}

#[test]
fn refuses_a_type_nothing_can_draw_and_a_layout_nothing_can_place() {
    // surfaces.test.ts: refuses a type nothing can draw / a layout nothing can place (602).
    assert!(read(&surface("mixer", Some("view"), "")).is_err());
    assert!(read(&surface("turntables", Some("panel"), "")).is_err());
}

#[test]
fn reads_the_arrangement_and_the_frame() {
    // surfaces.test.ts: reads the ARRANGEMENT and the FRAME, which are surfaces like any other (602).
    assert_eq!(
        read(&surface("timeline", Some("split"), "")).expect("reads").kind,
        SurfaceType::Timeline
    );
    let f = read(&surface("viewport", Some("floating"), "")).expect("reads");
    assert_eq!((f.kind, f.layout), (SurfaceType::Viewport, SurfaceLayout::Floating));
}

#[test]
fn a_viewport_declared_as_a_page_names_the_layout_it_draws_in() {
    // surfaces.test.ts: refuses a viewport declared as a page (602); the pair is raised on `layout`.
    let f = read(&surface("viewport", Some("split"), "")).expect_err("refused");
    assert!(f.message.contains("'floating'") && f.line == 2, "{f:?}");
}

#[test]
fn refuses_a_pair_the_editor_has_no_component_for() {
    // surfaces.test.ts: refuses a PAIR the editor has no component for (602).
    let f = read(&surface("turntables", Some("view"), "")).expect_err("refused");
    assert!(f.message.contains("'split'") && f.line == 2);
    assert!(read(&surface("timeline", Some("view"), "")).is_err());
    assert!(read(&surface("viewport", Some("view"), "")).is_err());
}

#[test]
fn requires_both_halves() {
    // surfaces.test.ts: requires both halves (602).
    assert!(read(&surface("turntables", None, "")).is_err());
    assert!(read("layout(\"view\")\n").is_err());
}

#[test]
fn names_the_unknown_key_on_its_own_line() {
    // surfaces.test.ts: names the unknown key rather than the file (602).
    let f = read(&surface("turntables", Some("split"), "mappings({})\n")).expect_err("refused");
    assert!(f.message.contains("mappings") && f.line == 3, "{f:?}");
}

#[test]
fn says_what_an_empty_file_is_missing() {
    // surfaces.test.ts: says what an empty file is missing (602); the empty file comes first.
    let f = read("").expect_err("refused");
    assert!(f.message.contains("type") && f.message.contains("layout"));
}

#[test]
fn a_null_entry_is_a_value_and_a_null_mapping_is_refused() {
    // decision 601: Nullable is the only reader of `null`; an Option refuses it where a value goes.
    let s = read(&surface("turntables", Some("split"), "mapping({ a: null })\n")).expect("reads");
    assert_eq!(s.mapping.as_ref().map(|m| m["a"].clone()), Some(Nullable(None)));
    assert_eq!(serde_json::to_value(&s).expect("json")["mapping"], json!({"a": null}));
    assert!(read(&surface("turntables", Some("split"), "mapping(null)\n")).is_err());
    assert!(
        read(&surface(
            "turntables",
            Some("split"),
            "mapping({ a: { module: \"m\", param: \"p\", also: null } })\n"
        ))
        .is_err()
    );
}

#[test]
fn refuses_the_forms_the_schema_refuses_and_places_each_on_its_line() {
    // decisions 601, 602: a number, an empty module, an empty key, invert as a number, stems as a number, an empty playlist.
    let line = |rest: &str| read(&surface("turntables", Some("split"), rest)).expect_err(rest).line;
    assert_eq!(line("mapping({ a: 3 })\n"), 3);
    assert_eq!(line("mapping({ a: { module: \"\", param: \"p\" } })\n"), 3);
    assert_eq!(line("mapping({ \"\": { module: \"m\", param: \"p\" } })\n"), 3);
    assert_eq!(line("mapping({ a: { module: \"m\", param: \"p\", invert: 1 } })\n"), 3);
    assert_eq!(line("stems(1)\n"), 3);
    assert_eq!(line("playlist(\"\")\n"), 3);
}

#[test]
fn reads_an_also_and_an_invert() {
    // decision 601: the second target and the mirror ride through the located decode.
    let rest = "mapping({ a: { module: \"m\", param: \"p\", invert: true, also: { module: \"n\", param: \"q\" } } })\n";
    let s = read(&surface("turntables", Some("split"), rest)).expect("reads");
    let b = s.mapping.expect("mapping")["a"].0.clone().expect("a binding");
    assert_eq!((b.invert, b.also.map(|a| a.module)), (Some(true), Some("n".to_owned())));
}

#[test]
fn a_message_about_a_huge_key_stays_short() {
    // hostile table (602): a key of 1 MB; the unknown-key message quotes it, and the bound is on the answer's size.
    let key = "k".repeat(1 << 20);
    let text = surface("turntables", Some("split"), &format!("{key}(1)\n"));
    let f = fast("a 1 MB key", || read(&text).expect_err("refused"));
    assert!(f.message.len() < 300, "{} bytes", f.message.len());
}

#[test]
fn a_mapping_of_two_hundred_thousand_keys_reads_in_time() {
    // hostile table (602): the decode is linear in the file.
    let rows: String = (0..200_000).map(|i| format!("k{i}: null,\n")).collect();
    let text = surface("turntables", Some("split"), &format!("mapping({{\n{rows}}})\n"));
    let s = fast("200,000 keys", || read(&text).expect("reads"));
    assert_eq!(s.mapping.map(|m| m.len()), Some(200_000));
}

#[test]
fn a_nest_of_three_hundred_is_refused_without_overflowing() {
    // hostile table (602): a nest of 300 as a mapping value is a finding, not a stack overflow.
    let nest = format!("{}1{}", "{ a: ".repeat(300), " }".repeat(300));
    let text = surface("turntables", Some("split"), &format!("mapping({{ x: {nest} }})\n"));
    assert!(fast("a nest of 300", || read(&text)).is_err());
}

fn ids(files: &[(&str, &str)]) -> (Vec<String>, Vec<(String, Severity)>) {
    let p = project_surfaces(&mem(files), &mut Cache::new(), "/p").expect("reads");
    (
        p.surfaces.into_iter().map(|s| s.id).collect(),
        p.diagnostics.into_iter().map(|d| (d.file, d.severity)).collect(),
    )
}

#[test]
fn finds_them_by_file_name_and_the_id_is_the_name() {
    // surfaces.test.ts: finds them by filename, and the id IS the filename (603).
    let p = project_surfaces(&mem(&[("surfaces/desk.k7x", DESK)]), &mut Cache::new(), "/p").expect("reads");
    assert!(p.diagnostics.is_empty());
    assert_eq!(p.surfaces[0].id, "desk");
    assert_eq!(p.surfaces[0].file, "surfaces/desk.k7x");
    assert_eq!(p.surfaces[0].surface.kind, SurfaceType::Turntables);
}

#[test]
fn is_silent_about_a_project_with_no_surfaces_folder() {
    // surfaces.test.ts: is silent about a project with no surfaces/ folder (603).
    assert_eq!(ids(&[("project.k7x", "")]), (vec![], vec![]));
}

#[test]
fn reports_a_file_that_will_not_read_and_keeps_the_others() {
    // surfaces.test.ts: reports a file that will not read, and keeps the others (603); an ERROR at the real place.
    let p = project_surfaces(
        &mem(&[("surfaces/desk.k7x", DESK), ("surfaces/broken.k7x", "type([\n")]),
        &mut Cache::new(),
        "/p",
    )
    .expect("reads");
    assert_eq!(p.surfaces.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["desk"]);
    assert_eq!(p.diagnostics[0].file, "surfaces/broken.k7x");
    assert_eq!(p.diagnostics[0].severity, Severity::Error);
    assert!(p.diagnostics[0].line >= 1);
}

#[test]
fn never_reads_a_surface_under_a_yaml_name() {
    // surfaces.test.ts: never reads a surface under a YAML name (603).
    let (ids, diags) = ids(&[("surfaces/desk.k7x", DESK), ("surfaces/old.yaml", "type: timeline\n")]);
    assert_eq!((ids, diags.len()), (vec!["desk".to_owned()], 0));
}

#[test]
fn gives_a_project_relative_path_like_every_other_diagnostic() {
    // surfaces.test.ts: gives a PROJECT-relative path, like every other Diagnostic (603).
    let (_, diags) = ids(&[("surfaces/desk.k7x", "type(\"turntables\")\n")]);
    assert_eq!(diags[0].0, "surfaces/desk.k7x");
}

#[test]
fn a_repeated_base_keeps_the_last_file_and_the_first_place() {
    // decision 603: read_family's rule on a repeated base.
    let (ids, _) = ids(&[
        ("surfaces/a.k7x", DESK),
        ("surfaces/b.k7x", DESK),
        ("surfaces/a.K7X", DESK),
    ]);
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn surfaces_as_a_file_and_a_folder_in_its_place_are_empty() {
    // hostile table (project_surfaces): `surfaces` a file, and one file that is a folder.
    assert_eq!(ids(&[("surfaces", "x")]), (vec![], vec![]));
    let (ids, diags) = ids(&[("surfaces/dir.k7x/inner.k7x", DESK)]);
    assert_eq!((ids.len(), diags.len()), (0, 0));
}

#[test]
fn a_hundred_thousand_small_files_are_a_hundred_thousand_findings_in_time() {
    // hostile table (project_surfaces): 100,000 small surface files, each a finding.
    let names: Vec<String> = (0..100_000).map(|i| format!("surfaces/s{i}.k7x")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "type(\"x\")\n")).collect();
    let src = mem(&files);
    let p = fast("100,000 files", || {
        project_surfaces(&src, &mut Cache::new(), "/p").expect("reads")
    });
    assert_eq!((p.surfaces.len(), p.diagnostics.len()), (0, 100_000));
}

#[test]
fn an_unreadable_file_is_an_error_naming_it_and_costs_that_file() {
    // decision 604: an Io on a read is an ERROR that names the file, and the others still read.
    use song::{Entry, Kind, SourceError, Span, Stat};
    struct Flaky(song::MemSource);
    impl song::Source for Flaky {
        fn stat(&self, p: &str) -> Result<Stat, SourceError> {
            self.0.stat(p)
        }
        fn list(&self, d: &str) -> Result<Vec<Entry>, SourceError> {
            self.0.list(d)
        }
        fn read(&self, p: &str, s: Span) -> Result<Vec<u8>, SourceError> {
            if p.ends_with("bad.k7x") {
                Err(SourceError::Io("denied".into()))
            } else {
                self.0.read(p, s)
            }
        }
    }
    let _ = Kind::File;
    let src = Flaky(mem(&[("surfaces/a.k7x", DESK), ("surfaces/bad.k7x", DESK)]));
    let p = project_surfaces(&src, &mut Cache::new(), "/p").expect("reads");
    assert_eq!(p.surfaces.len(), 1);
    assert!(p.diagnostics[0].file == "surfaces/bad.k7x" && p.diagnostics[0].severity == Severity::Error);
}

#[test]
fn a_surface_behind_a_link_out_is_a_warning_and_absent() {
    // decision 604 (DEPARTS): the TypeScript reads a link out unconfined; a file costs that file.
    #[cfg(unix)]
    {
        use support::Tmp;
        let outside = Tmp::new("surf-out");
        outside.write("x.k7x", DESK);
        let t = Tmp::new("surf-in");
        t.write("surfaces/ok.k7x", DESK);
        std::os::unix::fs::symlink(outside.at("x.k7x"), t.0.join("surfaces/out.k7x")).expect("link");
        let src = song::DiskSource::new(&[support::spell(&t.0)]).expect("mounts");
        let p = project_surfaces(&src, &mut Cache::new(), &support::spell(&t.0)).expect("reads");
        assert_eq!(p.surfaces.len(), 1);
        assert_eq!(p.diagnostics[0].severity, Severity::Warning);
        assert_eq!(p.diagnostics[0].file, "surfaces/out.k7x");
    }
    #[cfg(not(unix))]
    support::skip("a_surface_behind_a_link_out", "no file symlink without Developer Mode");
}

#[test]
fn reads_the_same_through_the_cache_twice() {
    // decision 603: a decoded surface is cloned into the answer once; a settled stamp is a hit.
    let src = mem(&[("surfaces/a.k7x", DESK)]);
    let mut cache = Cache::new();
    let a = project_surfaces(&src, &mut cache, "/p").expect("reads");
    let b = project_surfaces(&src, &mut cache, "/p").expect("reads");
    assert_eq!(a, b);
    assert_eq!(cache.len(), 1);
    let v: Value = serde_json::to_value(&a).expect("json");
    assert_eq!(v["surfaces"][0]["surface"]["type"], "turntables");
}

// The A2 rows: `surface_issues` over `Inputs` (decision 606).

use song::lanes::inputs::Inputs;
use song::views::surfaces::surface_issues;
use song::{Entry, Source, SourceError, Span, Stat};
use views_world::{inputs, world};

const TIMELINE: &str = "type(\"timeline\")\nlayout(\"split\")\n";
const VIEWPORT: &str = "type(\"viewport\")\nlayout(\"floating\")\n";
const PICTURE: &str = "extends(\"@meta/picture-track\")\nevents([])\n";
const MELODY: &str = "notes([])\n";

fn issues(files: &[(&str, &str)]) -> Vec<(String, Severity, String)> {
    let src = world(files);
    let inp = inputs(files);
    let s = project_surfaces(&src, &mut Cache::new(), &inp.project).expect("reads");
    surface_issues(&src, &inp, &s)
        .expect("issues")
        .into_iter()
        .map(|f| (f.file, f.severity, f.message))
        .collect()
}

fn warnings(files: &[(&str, &str)]) -> Vec<String> {
    issues(files)
        .into_iter()
        .filter(|i| i.1 == Severity::Warning)
        .map(|i| i.2)
        .collect()
}

#[test]
fn material_and_no_timeline_is_warned_about() {
    // surfaces.test.ts: a project holding material and declaring no timeline is warned about (606).
    let w = warnings(&[("surfaces/desk.k7x", DESK), ("sequences/melody.k7x", MELODY)]);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("timeline") && w[0].contains("project surface new timeline"));
    assert!(
        issues(&[("surfaces/desk.k7x", DESK), ("sequences/melody.k7x", MELODY)])
            .iter()
            .all(|i| i.1 == Severity::Warning && i.0 == "surfaces/")
    );
}

#[test]
fn a_declared_timeline_silences_it() {
    // surfaces.test.ts: is silent once it declares one (606).
    assert!(issues(&[("surfaces/timeline.k7x", TIMELINE), ("sequences/melody.k7x", MELODY)]).is_empty());
}

#[test]
fn nothing_arranged_is_silent() {
    // surfaces.test.ts: silent for a project holding nothing arranged: a mix, or a bare rig (606).
    assert!(issues(&[("surfaces/desk.k7x", DESK)]).is_empty());
}

#[test]
fn material_with_no_surfaces_folder_is_warned_about() {
    // surfaces.test.ts: warns a project with material and no surfaces/ folder at all (606).
    let all = issues(&[("sequences/melody.k7x", MELODY)]);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1, Severity::Warning);
}

#[test]
fn video_lanes_with_no_viewport_are_warned_about() {
    // surfaces.test.ts: a song with video lanes and no viewport is warned about (606).
    let w = warnings(&[("surfaces/timeline.k7x", TIMELINE), ("nodes/main.k7x", PICTURE)]);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("viewport"));
    let both = [
        ("surfaces/viewport.k7x", VIEWPORT),
        ("surfaces/timeline.k7x", TIMELINE),
        ("nodes/main.k7x", PICTURE),
    ];
    assert!(issues(&both).is_empty());
}

#[test]
fn a_song_without_picture_hears_nothing_about_it_and_one_missing_both_hears_twice() {
    // surfaces.test.ts: says nothing about picture to a song that has none; can be missing BOTH (606).
    assert!(issues(&[("surfaces/timeline.k7x", TIMELINE)]).is_empty());
    let both = warnings(&[("surfaces/desk.k7x", DESK), ("nodes/main.k7x", PICTURE)]);
    assert_eq!(both.len(), 2);
    assert!(both[0].contains("timeline") && both[1].contains("viewport"));
}

#[test]
fn the_read_findings_come_first_and_keep_their_place() {
    // decision 606: `surfaceIssues` starts from what `projectSurfaces` found.
    let all = issues(&[("surfaces/bad.k7x", "type(\"x\")\n"), ("sequences/melody.k7x", MELODY)]);
    assert_eq!(all[0].0, "surfaces/bad.k7x");
    assert_eq!(all[0].1, Severity::Error);
    assert_eq!(all[1].1, Severity::Warning);
}

/// Answers a fixed error for a stat of one folder, and whatever the inner source says for the rest.
struct Stat1<'a>(&'a dyn Source, &'static str, SourceError);

impl Source for Stat1<'_> {
    fn stat(&self, p: &str) -> Result<Stat, SourceError> {
        if p.ends_with(self.1) {
            Err(self.2.clone())
        } else {
            self.0.stat(p)
        }
    }
    fn list(&self, d: &str) -> Result<Vec<Entry>, SourceError> {
        self.0.list(d)
    }
    fn read(&self, p: &str, s: Span) -> Result<Vec<u8>, SourceError> {
        self.0.read(p, s)
    }
}

fn with_clips_stat(error: SourceError) -> Result<usize, SourceError> {
    let files: [(&str, &str); 0] = [];
    let (src, inp): (_, Inputs) = (world(&files), inputs(&files));
    let wrapped = Stat1(&src, "/clips", error);
    let s = project_surfaces(&wrapped, &mut Cache::new(), &inp.project)?;
    surface_issues(&wrapped, &inp, &s).map(|v| v.len())
}

#[test]
fn a_clips_folder_behind_a_link_out_counts_as_material_and_an_unreadable_one_does_not() {
    // decision 606: `Outside` is true (existsSync follows a link), `NotFound` and `Io` false, `Missing` is Err.
    assert_eq!(with_clips_stat(SourceError::Outside), Ok(1));
    assert_eq!(with_clips_stat(SourceError::NotFound), Ok(0));
    assert_eq!(with_clips_stat(SourceError::Io("denied".into())), Ok(0));
    assert_eq!(with_clips_stat(SourceError::Missing), Err(SourceError::Missing));
}

#[test]
fn a_surfaces_folder_the_source_refuses_whole_is_one_warning() {
    // decision 604: a `surfaces/` behind a link out costs the folder only, one WARNING named `surfaces`.
    let src = world(&[("surfaces/desk.k7x", DESK)]);
    let wrapped = Stat1(&src, "/surfaces", SourceError::Outside);
    let p = project_surfaces(&wrapped, &mut Cache::new(), "/p/ws/proj").expect("reads");
    assert!(p.surfaces.is_empty());
    assert_eq!(
        p.diagnostics
            .iter()
            .map(|d| (d.file.as_str(), d.severity))
            .collect::<Vec<_>>(),
        [("surfaces", Severity::Warning)]
    );
}
