// assets.rs — the files a module loads, resolved where it declares them: moduleAssets, libraryFiles and confine tests, decisions 65 to 68.
// A reader would get wrong who confines: the Source does, so a path it refuses is no file and a warning, whatever the module named.

use crate::support;

use song::files::Severity;
use song::{Cache, Roots};
use support::{mem_world, mem_world_roots, world};

const KIT: &str = "lib/k7-core/modules/instruments/kit";
const AMP: &str = "lib/k7-core/modules/audio/amp";

fn p(rel: &str) -> String {
    format!("/p/{rel}")
}

fn kit_with(symbols: &str, extra: &[(&str, &str)]) -> support::World {
    let module = format!("type(\"rack\")\nrack({{ symbols: {{ {symbols} }} }})\n");
    let mut files: Vec<(String, String)> = vec![(format!("{KIT}/module.k7x"), module)];
    files.extend(extra.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    mem_world(&borrowed)
}

fn clip(w: &support::World, sym: &str) -> Option<String> {
    w.vocab.modules["@k7-core/instruments/kit"]
        .authored
        .rack
        .as_ref()?
        .symbols
        .get(sym)?
        .clip
        .as_ref()
        .map(|c| c.0.clone())
}

// moduleAssets.md, "rack audio": `sources/`, then `assets/`, then beside the module; a ref escaping a folder ends THAT search.
#[test]
fn rack_audio_is_found_in_sources_then_assets_then_beside_the_module() {
    let w = kit_with(
        "BD: { note: 36, clip: \"kick.wav\" }, SN: { note: 38, clip: \"snare.wav\" }, HH: { note: 42, clip: \"hat.wav\" }, UP: { note: 43, clip: \"../up.wav\" }",
        &[
            (&format!("{KIT}/sources/kick.wav"), "x"),
            (&format!("{KIT}/assets/snare.wav"), "x"),
            (&format!("{KIT}/assets/kick.wav"), "x"),
        ],
    );
    assert_eq!(
        clip(&w, "BD"),
        Some(p(&format!("{KIT}/sources/kick.wav"))),
        "sources first"
    );
    assert_eq!(clip(&w, "SN"), Some(p(&format!("{KIT}/assets/snare.wav"))));
    assert_eq!(
        clip(&w, "HH"),
        Some(p(&format!("{KIT}/hat.wav"))),
        "kept even when the file is absent"
    );
    assert_eq!(
        clip(&w, "UP"),
        Some(p("lib/k7-core/modules/instruments/up.wav")),
        "the join is confined by the Source, not the folder"
    );
}

// moduleAssets.md: an absolute ref is the Source's to admit; a library sample wins over the clip (confine.test.ts, "drops a rack sound that escapes and keeps a library one").
#[test]
fn an_absolute_clip_is_the_sources_to_admit_and_a_library_sample_wins() {
    let w = kit_with(
        "IN: { note: 36, clip: \"/p/ws/proj/x.wav\" }, OUT: { note: 37, clip: \"/etc/passwd\" }, LIB: { note: 38, sample: \"@k7-core/hits/a.wav\", clip: \"ignored.wav\" }",
        &[],
    );
    assert_eq!(clip(&w, "IN").as_deref(), Some("/p/ws/proj/x.wav"));
    assert_eq!(clip(&w, "OUT"), None, "outside every mount: no file");
    assert!(
        w.vocab
            .findings
            .iter()
            .any(|f| f.file == "@k7-core/instruments/kit/module.k7x" && f.severity == Severity::Warning)
    );
    assert_eq!(
        clip(&w, "LIB"),
        None,
        "a library that holds no samples/ folder names nothing"
    );
    let w = kit_with(
        "LIB: { note: 38, sample: \"@k7-core/hits/a.wav\", clip: \"ignored.wav\" }",
        &[("lib/k7-core/samples/hits/a.wav", "x")],
    );
    assert_eq!(clip(&w, "LIB"), Some(p("lib/k7-core/samples/hits/a.wav")));
    let sym = &w.vocab.modules["@k7-core/instruments/kit"]
        .authored
        .rack
        .as_ref()
        .expect("rack")
        .symbols["LIB"];
    assert_eq!(
        sym.sample.as_ref().map(|s| s.0.as_str()),
        Some("@k7-core/hits/a.wav"),
        "the name it was written with stays"
    );
}

// libraryFiles.test.ts, "refuses a ref that climbs out of the folder, or names no library", decision 67.
#[test]
fn a_library_ref_names_a_listed_library_alone_and_never_climbs_out() {
    let w = kit_with(
        "A: { note: 1, sample: \"@k7-core/../../x.wav\" }, B: { note: 2, sample: \"@nobody/a.wav\" }, C: { note: 3, sample: \"@k7-core/a\\u0000.wav\" }, D: { note: 4, sample: \"@k7-core/hits/../a.wav\" }, E: { note: 5, sample: \"plain.wav\" }",
        &[("lib/k7-core/samples/a.wav", "x")],
    );
    for s in ["A", "B", "C", "E"] {
        assert_eq!(clip(&w, s), None, "{s}");
    }
    assert_eq!(
        clip(&w, "D"),
        Some(p("lib/k7-core/samples/a.wav")),
        "a `..` that stays below the folder is a name"
    );
}

fn amp(params: &str, extra: &[(&str, &str)]) -> support::World {
    let mut files: Vec<(String, String)> = vec![(
        format!("{AMP}/module.k7x"),
        format!("plugin({{ type: \"internal\", name: \"Amp\" }})\nparams({{ {params} }})\n"),
    )];
    files.extend(extra.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    mem_world(&borrowed)
}

fn field(w: &support::World, f: impl Fn(&song::modules::LoadedModule) -> Option<String>) -> Option<String> {
    f(&w.vocab.modules["@k7-core/audio/amp"])
}

// libraryFiles.test.ts, "a module's `model` param": from its own assets/, from a library's models/, and the names stay as authored.
#[test]
fn a_model_comes_from_assets_or_a_librarys_models_and_assets_keep_the_authored_names() {
    let own = amp("model: \"jcm.nam\"", &[(&format!("{AMP}/assets/jcm.nam"), "x")]);
    assert_eq!(
        field(&own, |m| m.model.clone()),
        Some(p(&format!("{AMP}/assets/jcm.nam")))
    );
    assert_eq!(
        own.vocab.modules["@k7-core/audio/amp"]
            .assets
            .as_ref()
            .map(|a| a["model"].clone())
            .as_deref(),
        Some("jcm.nam")
    );
    let shared = amp(
        "model: \"@k7-core/amps/jcm.nam\"",
        &[("lib/k7-core/models/amps/jcm.nam", "x")],
    );
    assert_eq!(
        field(&shared, |m| m.model.clone()),
        Some(p("lib/k7-core/models/amps/jcm.nam"))
    );
    let none = amp("model: \"gone.nam\"", &[]);
    assert_eq!(field(&none, |m| m.model.clone()), None);
    assert!(
        none.vocab.modules["@k7-core/audio/amp"].assets.is_some(),
        "the authored name stays"
    );
}

// libraryFiles.test.ts, "a module's `ir` param": the library's irs/, keeping the name it was written with.
#[test]
fn an_ir_resolves_to_a_librarys_irs_folder() {
    let w = amp(
        "ir: \"@k7-core/halls/big.wav\"",
        &[("lib/k7-core/irs/halls/big.wav", "x")],
    );
    assert_eq!(field(&w, |m| m.ir.clone()), Some(p("lib/k7-core/irs/halls/big.wav")));
    let outside = amp("ir: \"@k7-core/../../x.wav\"", &[]);
    assert_eq!(field(&outside, |m| m.ir.clone()), None);
    let w = amp("onnx: \"net.onnx\"", &[(&format!("{AMP}/assets/net.onnx"), "x")]);
    assert_eq!(
        field(&w, |m| m.onnx.clone()),
        Some(p(&format!("{AMP}/assets/net.onnx")))
    );
}

// Decision 68: the models cache is a folder mount that is only STAT'ed; no root means no fallback.
#[test]
fn the_model_cache_is_stat_ed_and_never_read() {
    let files = [
        (
            format!("{AMP}/module.k7x"),
            "params({ model: \"cached.nam\", onnx: \"../escape.onnx\" })\n".to_owned(),
        ),
        ("models/cached.nam".to_owned(), "x".to_owned()),
    ];
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let src = support::mem(&borrowed);
    let spy = support::Spy {
        inner: &src,
        reads: std::sync::Mutex::new(Vec::new()),
    };
    let w = world(&spy, &mut Cache::new(), &mem_world_roots()).expect("a world");
    assert_eq!(field(&w, |m| m.model.clone()), Some("/p/models/cached.nam".to_owned()));
    assert_eq!(
        field(&w, |m| m.onnx.clone()),
        None,
        "a ref climbing out of the cache is no file"
    );
    assert!(
        spy.reads
            .lock()
            .expect("lock")
            .iter()
            .all(|r| !r.starts_with("/p/models")),
        "no read under the models root"
    );
    let no_root = Roots {
        models: None,
        ..mem_world_roots()
    };
    let w = world(&src, &mut Cache::new(), &no_root).expect("a world");
    assert_eq!(field(&w, |m| m.model.clone()), None);
}

// moduleAssetChoices.test.ts, "a file the module INHERITS": resolved against the folder that holds it, the module's own preferred.
#[test]
fn an_inherited_file_resolves_against_the_folder_that_holds_it() {
    let parent = "lib/k7-core/modules/audio/parent";
    let w = mem_world(&[
        (
            &format!("{parent}/module.k7x"),
            "plugin({ type: \"internal\", name: \"P\" })\n",
        ),
        (&format!("{parent}/assets/shared.nam"), "x"),
        (&format!("{parent}/assets/both.nam"), "x"),
        (
            "ws/proj/nodes/kid.k7x",
            "extends(\"@k7-core/audio/parent\")\nparams({ model: \"shared.nam\" })\n",
        ),
        (
            "ws/proj/nodes/both.k7x",
            "extends(\"@k7-core/audio/parent\")\nparams({ model: \"both.nam\" })\n",
        ),
        ("ws/proj/nodes/assets/both.nam", "x"),
        (
            "ws/proj/nodes/up.k7x",
            "extends(\"@k7-core/audio/parent\")\nparams({ model: \"../parent/assets/shared.nam\" })\n",
        ),
    ]);
    assert_eq!(
        w.resolved.modules["kid"].model.as_deref(),
        Some(p(&format!("{parent}/assets/shared.nam")).as_str())
    );
    assert_eq!(
        w.resolved.modules["both"].model.as_deref(),
        Some("/p/ws/proj/nodes/assets/both.nam"),
        "its own first"
    );
    assert_eq!(
        w.resolved.modules["up"].model, None,
        "a ref that climbs out ends the search, the next folder up is not tried"
    );
}

// libraryFiles.test.ts, "a PROJECT's own samples, as @project/…" and decision 66: per compile, outside the vocabulary.
#[test]
fn an_project_ref_is_resolved_per_compile_against_the_project_and_not_in_the_vocabulary() {
    let kit = "type(\"rack\")\nrack({ symbols: { BD: { note: 36, sample: \"@project/kick.wav\" } } })\nparams({ model: \"@project/x.nam\" })\n";
    let src = support::mem(&[
        (&format!("{KIT}/module.k7x"), kit),
        ("ws/proj/samples/kick.wav", "x"),
        ("ws/proj/models/x.nam", "x"),
        ("ws/proj/nodes/n.k7x", "extends(\"@k7-core/instruments/kit\")\n"),
        ("ws/other/nodes/n.k7x", "extends(\"@k7-core/instruments/kit\")\n"),
    ]);
    let mut cache = Cache::new();
    let w = world(&src, &mut cache, &mem_world_roots()).expect("a world");
    let v = &w.vocab.modules["@k7-core/instruments/kit"];
    assert!(
        v.model.is_none() && v.authored.rack.as_ref().expect("rack").symbols["BD"].clip.is_none(),
        "unresolved in the vocabulary"
    );
    let n = &w.resolved.modules["n"];
    assert_eq!(n.model.as_deref(), Some("/p/ws/proj/models/x.nam"));
    assert_eq!(
        n.authored.rack.as_ref().expect("rack").symbols["BD"]
            .clip
            .as_ref()
            .map(|c| c.0.as_str()),
        Some("/p/ws/proj/samples/kick.wav")
    );
    // Another project of the same workspace shares the vocabulary and resolves against its own folder.
    let other = Roots {
        project: "/p/ws/other".into(),
        ..mem_world_roots()
    };
    let w2 = world(&src, &mut cache, &other).expect("a world");
    assert_eq!(
        w2.resolved.modules["n"].model.as_deref(),
        Some("/p/ws/other/models/x.nam")
    );
    assert_eq!(cache.vocabularies(), 1);
    assert!(cache.vocabulary_hits() >= 1);
}

// Decision 65: a file ref that escapes its folder lexically resolves to nothing, silently.
#[test]
fn a_lexical_escape_is_no_file_and_no_warning() {
    let w = amp("model: \"../../../../../etc/x.nam\"", &[]);
    assert_eq!(field(&w, |m| m.model.clone()), None);
    assert!(w.vocab.findings.iter().all(|f| !f.file.ends_with("module.k7x")));
}

// Decision 65: a link out of the mounts is `Outside`: no file, a warning, the authored name kept (disk; Unix links only).
#[test]
fn a_link_out_of_the_mounts_is_no_file_and_a_warning() {
    #[cfg(unix)]
    {
        use support::{Tmp, disk_world};
        let t = Tmp::new("assets-link");
        let outside = Tmp::new("assets-outside");
        outside.write("secret.nam", "x");
        t.write(&format!("{AMP}/module.k7x"), "params({ model: \"leak.nam\" })\n");
        t.mkdir(&format!("{AMP}/assets"));
        std::os::unix::fs::symlink(outside.0.join("secret.nam"), t.0.join(format!("{AMP}/assets/leak.nam")))
            .expect("link");
        let w = disk_world(&t, &[]);
        let m = &w.vocab.modules["@k7-core/audio/amp"];
        assert_eq!(m.model, None);
        assert_eq!(m.assets.as_ref().map(|a| a["model"].as_str()), Some("leak.nam"));
        assert!(
            w.vocab
                .findings
                .iter()
                .any(|f| f.file == "@k7-core/audio/amp/module.k7x" && f.severity == Severity::Warning)
        );
    }
    #[cfg(not(unix))]
    support::skip(
        "a_link_out_of_the_mounts_is_no_file_and_a_warning",
        "file links need Developer Mode on Windows",
    );
}

// libraryFiles.test.ts, "a tap's sample inside an ALTERNATIVE": the set's own rack resolves as the module's does.
#[test]
fn a_tap_sample_inside_an_alternative_resolves_too() {
    let w = mem_world(&[
        (
            &format!("{KIT}/module.k7x"),
            "type(\"rack\")\nalternatives([{ name: \"b\", rack: { symbols: { BD: { note: 36, sample: \"@k7-core/a.wav\" } } } }])\n",
        ),
        ("lib/k7-core/samples/a.wav", "x"),
    ]);
    let alt = &w.vocab.modules["@k7-core/instruments/kit"]
        .authored
        .alternatives
        .as_ref()
        .expect("alternatives")[0];
    assert_eq!(
        alt.rack.as_ref().expect("rack").symbols["BD"]
            .clip
            .as_ref()
            .map(|c| c.0.as_str()),
        Some("/p/lib/k7-core/samples/a.wav")
    );
}

// Decision 82: a library's family folder is there when the root lists it, a link included; a refused path is never read.
#[test]
fn a_linked_family_folder_is_a_family_and_a_dangling_one_is_not() {
    #[cfg(unix)]
    {
        use support::{Tmp, disk_world};
        let t = Tmp::new("assets-family-link");
        let outside = Tmp::new("assets-family-out");
        outside.write("samples/a.wav", "x");
        t.write(&format!("{KIT}/module.k7x"), "type(\"rack\")\nrack({ symbols: { A: { note: 1, sample: \"@k7-core/a.wav\" }, B: { note: 2, sample: \"@account/a.wav\" } } })\n");
        t.mkdir("lib/k7-core");
        std::os::unix::fs::symlink(outside.0.join("samples"), t.0.join("lib/k7-core/samples")).expect("link");
        t.mkdir("lib/account");
        std::os::unix::fs::symlink(outside.0.join("gone"), t.0.join("lib/account/samples")).expect("link");
        let w = disk_world(&t, &[]);
        let clips = &w.vocab.modules["@k7-core/instruments/kit"]
            .authored
            .rack
            .as_ref()
            .expect("rack")
            .symbols;
        assert_eq!(
            clips["A"].clip.as_ref().map(|c| c.0.clone()),
            Some(t.at("lib/k7-core/samples/a.wav"))
        );
        assert_eq!(clips["B"].clip, None);
    }
    #[cfg(not(unix))]
    support::skip(
        "a_linked_family_folder_is_a_family_and_a_dangling_one_is_not",
        "file links need Developer Mode on Windows",
    );
}

// Decision 83: an absolute tail or base names a place of its own and is refused, as the TypeScript's `resolve` leaves it outside.
#[test]
fn an_absolute_tail_or_base_is_refused() {
    let w = kit_with(
        "A: { note: 1, sample: \"@k7-core//etc/x.wav\" }",
        &[("lib/k7-core/samples/etc/x.wav", "x")],
    );
    assert_eq!(clip(&w, "A"), None);
}

// Decision 83 on Windows: a `\` in a library tail is more than a name there, so a climb written with it is refused.
#[cfg(windows)]
#[test]
fn a_backslash_climb_in_a_library_tail_is_refused() {
    let w = kit_with(
        "A: { note: 1, sample: \"@k7-core/..\\\\..\\\\x.wav\" }",
        &[("lib/k7-core/samples/x.wav", "x")],
    );
    assert_eq!(clip(&w, "A"), None);
}
