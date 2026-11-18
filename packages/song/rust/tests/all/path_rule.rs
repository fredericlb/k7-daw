// path_rule.rs — the path rule of the machine that spelled the roots (decision 739), the Windows one read on any machine.
// A reader would get wrong that nothing here needs Windows: `Roots::paths` is the rule, so a Mac reads a Windows tray's roots as Windows does.

use crate::clips_support;
use crate::doc_support;
use crate::support;

use doc_support::{AMP, lane};
use serde_json::{Value, json};
use song::doc::{Request, Vsts, compile};
use song::lanes::rig::Rig;
use song::record::Recorder;
use song::roots::{is_absolute, spell};
use song::source::BundleRequest;
use song::{Cache, MemSource, PathRule, Roots};
use std::path::Path;

const WIN: PathRule = PathRule::Windows;
const POSIX: PathRule = PathRule::Posix;

// 739: `spell` and `is_absolute` are functions of the rule, with no global.
#[test]
fn spell_and_is_absolute_take_the_rule() {
    for (path, rule, want) in [
        (r"C:\x\..\y.wav", WIN, Some("C:/y.wav")),
        (r"c:\x.wav", WIN, Some("C:/x.wav")),
        ("/x/y", WIN, Some("/x/y")),
        (r"sounds\a.wav", WIN, None),
        (r"C:\x.wav", POSIX, None),
        (r"/a\b", POSIX, Some(r"/a\b")),
        ("C:/x.wav", POSIX, Some("C:/x.wav")),
    ] {
        assert_eq!(spell(path, rule).ok().as_deref(), want, "{path} under {rule:?}");
    }
    for (path, rule, want) in [
        (r"C:\x.wav", WIN, true),
        ("C:/x.wav", WIN, true),
        ("/x.wav", WIN, true),
        (r"sounds\a.wav", WIN, false),
        ("sounds/a.wav", WIN, false),
        (r"C:\x.wav", POSIX, false),
        ("/x.wav", POSIX, true),
        ("x.wav", POSIX, false),
    ] {
        assert_eq!(is_absolute(path, rule), want, "{path} under {rule:?}");
    }
}

// 739: the crate asks the machine in two places, `PathRule::HOST` and `source/disk.rs`, and nowhere else.
#[test]
fn no_source_file_asks_the_machine_but_the_two_places() {
    fn walk(dir: &Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).expect("a folder") {
            let path = e.expect("an entry").path();
            if path.is_dir() {
                walk(&path, hits);
                continue;
            }
            let name = path.to_string_lossy().replace('\\', "/");
            if !name.ends_with(".rs") || name.ends_with("source/disk.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file");
            for (i, line) in text.lines().enumerate() {
                let code = line.trim_start();
                let asks = ["cfg!(windows)", "cfg!(unix)", "cfg(windows)", "cfg(unix)", "target_os"]
                    .iter()
                    .any(|k| code.contains(k));
                if asks && !code.starts_with("//") && !name.ends_with("source/path.rs") {
                    hits.push(format!("{name}:{}", i + 1));
                }
            }
        }
    }
    let mut hits = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut hits);
    assert!(hits.is_empty(), "the machine is asked in {hits:?}");
    let path =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/source/path.rs")).expect("path.rs");
    assert_eq!(
        path.matches("cfg!(windows)").count(),
        1,
        "PathRule::HOST is the one place"
    );
}

/// A Windows tray's world: every file under `C:/p`, every root spelled the way the tray spelled it, with `\`.
fn windows_world(files: &[(&str, &str)]) -> (MemSource, Roots) {
    let owned = doc_support::laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = support::mem_at("C:/p", &refs);
    let win = |p: &str| format!("C:{}", p.replace('/', "\\"));
    let base = doc_support::roots();
    let roots = Roots {
        project: win(&base.project),
        workspace: base.workspace.as_deref().map(win),
        confine: base.confine.as_deref().map(win),
        libraries: base
            .libraries
            .iter()
            .map(|l| song::Library {
                root: win(&l.root),
                ..l.clone()
            })
            .collect(),
        meta: base.meta.as_deref().map(win),
        models: base.models.as_deref().map(win),
        renders: base.renders.as_deref().map(win),
        paths: WIN,
        ..Roots::default()
    };
    (src, roots)
}

fn request(roots: &Roots) -> Request {
    Request {
        roots: roots.clone(),
        rig: Rig::from_json(&json!({})),
        vsts: Vsts::Scan,
        thawed: Vec::new(),
    }
}

const FILES: [(&str, &str); 7] = [
    ("nodes/amp.k7x", AMP),
    ("clips/rel.k7x", "source(\"sounds\\\\a.wav\")\n"),
    (
        "clips/abs.k7x",
        "source(\"C:\\\\p\\\\ws\\\\proj\\\\sounds\\\\b.wav\")\n",
    ),
    ("sounds/a.wav", "w"),
    ("sounds/b.wav", "w"),
    ("graph.k7x", "modules({ gtr: [\"amp\"], amp: [\"MIX\"] })\n"),
    ("nodes/gtr.k7x", ""),
];

fn gtr() -> String {
    lane("events([{ ref: \"rel\", start: \"1:1\" }, { ref: \"abs\", start: \"2:1\" }])\n")
}

fn sources(c: &song::doc::Compiled) -> Vec<String> {
    let doc = c
        .engine
        .as_ref()
        .unwrap_or_else(|f| panic!("the compile failed: {:?}", f.diagnostics));
    doc.tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.source.clone()))
        .collect()
}

fn files_with_lane(lane_text: &str) -> Vec<(&str, &str)> {
    let mut files: Vec<(&str, &str)> = FILES.to_vec();
    if let Some(at) = files.iter().position(|(n, _)| *n == "nodes/gtr.k7x") {
        files[at].1 = lane_text;
    }
    files
}

// 739: a Windows tray's roots (`C:\p\ws\proj`) mount, and `C:\x.wav` is absolute where `sounds\a.wav` is relative with `\` read as `/`.
#[test]
fn a_compile_under_the_windows_rule_reads_a_drive_as_absolute_and_a_backslash_as_a_separator() {
    let text = gtr();
    let (src, roots) = windows_world(&files_with_lane(&text));
    let (dirs, _) = roots.mounts().expect("mounts");
    assert_eq!(dirs[0], "C:/p/ws/proj");
    assert!(dirs.iter().all(|d| d.starts_with("C:/")), "{dirs:?}");
    let c = compile(&src, &mut Cache::with_paths(WIN), &request(&roots)).expect("a compile");
    assert_eq!(
        sources(&c),
        ["C:/p/ws/proj/sounds/a.wav", "C:/p/ws/proj/sounds/b.wav"],
        "{:?}",
        c.diagnostics
    );
}

// 739: the same texts under the Posix rule are another project: the drive is a folder name and the backslash a letter.
#[test]
fn the_same_texts_under_the_posix_rule_read_a_drive_as_a_folder_and_a_backslash_as_a_letter() {
    let text = gtr();
    let owned = doc_support::laid(&files_with_lane(&text));
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = support::mem(&refs);
    let roots = Roots {
        paths: POSIX,
        ..doc_support::roots()
    };
    let c = compile(&src, &mut Cache::with_paths(POSIX), &request(&roots)).expect("a compile");
    let failure = c
        .engine
        .as_ref()
        .expect_err("neither clip is where the Windows rule put it");
    let said: Vec<&str> = failure.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        said,
        [
            "track 'gtr' — audio clip 'rel' file missing",
            "track 'gtr' — audio clip 'abs' file missing"
        ]
    );
    let paths = song::clips::audio_paths(&src, "/p/ws/proj", &clips_of(), POSIX).expect("paths");
    assert_eq!(paths["rel"], "/p/ws/proj/sounds\\a.wav");
    assert_eq!(paths["abs"], "/p/ws/proj/C:\\p\\ws\\proj\\sounds\\b.wav");
}

fn clips_of() -> song::FamilyRead<song::Clip> {
    let items: Vec<(&str, Value)> = vec![
        ("rel", json!({ "source": r"sounds\a.wav" })),
        ("abs", json!({ "source": r"C:\p\ws\proj\sounds\b.wav" })),
    ];
    clips_support::clip_family(&items)
}

// 739: a bundle made under the Windows rule replays on any machine, its roots carrying the rule: the same document comes out.
#[test]
fn a_windows_bundle_replays_under_the_rule_it_carries() {
    let text = gtr();
    let (src, roots) = windows_world(&files_with_lane(&text));
    let rec = Recorder::new(&src);
    let first = compile(&rec, &mut Cache::with_paths(WIN), &request(&roots)).expect("a compile");
    let (dirs, files) = roots.mounts().expect("mounts");
    let extras = song::record::Extras {
        request: Some(BundleRequest {
            roots: roots.clone(),
            rig: json!({}),
            vsts: "scan".to_owned(),
            thawed: Vec::new(),
        }),
        ..Default::default()
    };
    let bundle = rec.into_bundle_with(&dirs, &files, extras);
    let wire: Value = serde_json::to_value(&bundle).expect("json");
    assert_eq!(wire["request"]["roots"]["paths"], "windows");
    let back: song::SourceBundle = serde_json::from_value(wire).expect("a bundle");
    let replay = MemSource::from_bundle(back).expect("a source");
    let carried = replay.request().expect("a request").roots.clone();
    assert_eq!(carried.paths, WIN);
    let again = compile(&replay, &mut Cache::with_paths(carried.paths), &request(&carried)).expect("a replay");
    assert_eq!(sources(&again), sources(&first));
    assert_eq!(
        serde_json::to_value(again.engine.expect("a doc")).expect("json"),
        serde_json::to_value(first.engine.expect("a doc")).expect("json")
    );
}

// 739: the cache decodes a chain's lane names by the rule it was made for, and forgets what it decoded under another.
#[test]
fn a_cache_decodes_a_lane_name_by_its_rule_and_forgets_on_a_change() {
    let chain = "start(true)\nsteps([id(\"l\").track(\"a\\\\b\")])\n";
    let src = support::mem(&[("chains/a.k7x", chain)]);
    let mut cache = Cache::with_paths(POSIX);
    let ok = cache
        .read::<song::routing::ChainFile>(&src, "/p/chains/a.k7x")
        .expect("a read");
    assert!(ok.is_ok(), "a backslash is a letter under the Posix rule");
    cache.set_paths(WIN);
    let no = cache
        .read::<song::routing::ChainFile>(&src, "/p/chains/a.k7x")
        .expect("a read");
    assert!(no.is_err(), "and a separator under the Windows one, on the same cache");
    assert_eq!(cache.paths(), WIN);
}

// 739: the rule keys the vocabulary, so one cache serving two rules never answers one's modules to the other.
#[test]
fn the_rule_keys_the_vocabulary_of_a_shared_cache() {
    let base = song::Roots {
        project: "/p/proj".into(),
        paths: POSIX,
        ..Roots::default()
    };
    let win = Roots {
        paths: WIN,
        ..base.clone()
    };
    assert_ne!(base.vocabulary_key(), win.vocabulary_key());
}
