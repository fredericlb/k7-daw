// vocab_cache.rs — the vocabulary cache on a read set: reuse, rebuild, replay into a Recorder, and the memory Source's stamps.
// A reader would get wrong what is the key: never a stamp alone, but every file's stamp, every listing's entries, every absent path.

use crate::support;

use song::modules::Vocabulary;
use song::record::Recorder;
use song::{Cache, Library, MemSource, Roots, SourceBundle, SourceError};
use std::sync::Arc;
use support::{Tmp, disk, mem, plugin_text, roots};

fn same(a: &Arc<Vocabulary>, b: &Arc<Vocabulary>) -> bool {
    Arc::ptr_eq(a, b)
}

fn keys(v: &Vocabulary, prefix: &str) -> Vec<String> {
    v.modules.keys().filter(|k| k.starts_with(prefix)).cloned().collect()
}

fn mem_roots() -> Roots {
    Roots {
        project: "/p/proj".into(),
        libraries: vec![Library {
            name: "k7-core".into(),
            root: "/p/lib".into(),
            editable: false,
        }],
        ..Roots::default()
    }
}

fn mem_lib(files: &[(&str, &str)]) -> MemSource {
    let prefixed: Vec<(String, String)> = files
        .iter()
        .map(|(p, t)| (format!("lib/modules/{p}"), (*t).to_owned()))
        .collect();
    let borrowed: Vec<(&str, &str)> = prefixed.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    mem(&borrowed)
}

// moduleStamp.test.ts, "an untouched settled tree is reused": the same value comes back, not a rebuilt equal one.
#[test]
fn an_untouched_settled_tree_is_reused() {
    let t = Tmp::new("vc-reuse");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let source = disk(&r);
    let first = cache.vocabulary(&source, &r).expect("vocabulary");
    let second = cache.vocabulary(&source, &r).expect("vocabulary");
    assert!(same(&first, &second));
    assert_eq!(cache.vocabularies(), 1);
}

// moduleStamp.settle test and decision 26: an unsettled file is never a key, so nothing is held and every call rebuilds.
#[test]
fn an_unsettled_file_is_never_a_cache_key() {
    let t = Tmp::new("vc-unsettled");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    let (r, mut cache) = (roots(&t), Cache::new());
    let source = disk(&r);
    let first = cache.vocabulary(&source, &r).expect("vocabulary");
    assert_eq!(cache.vocabularies(), 0);
    assert!(!same(&first, &cache.vocabulary(&source, &r).expect("vocabulary")));
}

// moduleStamp.test.ts: a rewrite rebuilds, and a rename that keeps the count and every mtime does too (decision 26).
#[test]
fn a_rewrite_and_a_same_count_rename_each_rebuild() {
    let t = Tmp::new("vc-rewrite");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let source = disk(&r);
    let first = cache.vocabulary(&source, &r).expect("vocabulary");
    std::fs::rename(
        t.0.join("lib/k7-core/modules/audio/a"),
        t.0.join("lib/k7-core/modules/audio/b"),
    )
    .expect("rename");
    let renamed = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    assert!(!same(&first, &renamed));
    assert!(keys(&renamed, "@k7-core/audio").contains(&"@k7-core/audio/b".to_owned()));
    assert!(!renamed.modules.contains_key("@k7-core/audio/a"));
    t.age();
    let settled = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    t.write("lib/k7-core/modules/audio/b/module.k7x", &plugin_text("Changed"));
    let rewritten = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    assert!(!same(&settled, &rewritten));
    assert_eq!(
        rewritten.modules["@k7-core/audio/b"]
            .authored
            .plugin
            .as_ref()
            .and_then(|p| p.name.as_deref()),
        Some("Changed")
    );
}

// moduleStamp.test.ts: a scan change rebuilds, since `@engine/…` refs come out of it.
#[test]
fn a_scan_change_rebuilds() {
    let t = Tmp::new("vc-scan");
    t.write(
        "home/exts.json",
        r#"{"entries":[{"plugins":[{"name":"Serum","format":"VST3"}]}]}"#,
    );
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let first = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    assert!(first.modules.contains_key("@engine/serum") && !first.modules.contains_key("@engine/massive"));
    t.write(
        "home/exts.json",
        r#"{"entries":[{"plugins":[{"name":"Massive","format":"VST3"}]}]}"#,
    );
    let second = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    assert!(second.modules.contains_key("@engine/massive") && !second.modules.contains_key("@engine/serum"));
}

// stampWalk.test.ts: dot entries are skipped by the walk, so a stray one changes no module.
#[test]
fn a_dot_entry_changes_no_module() {
    let t = Tmp::new("vc-dots");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let first = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    t.write("lib/k7-core/modules/audio/.cache/module.k7x", &plugin_text("Ghost"));
    let second = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    assert_eq!(
        first.modules.keys().collect::<Vec<_>>(),
        second.modules.keys().collect::<Vec<_>>()
    );
}

// moduleCache.test.ts: four held, the least recently used dropped, shed keeps the last, forget drops all.
#[test]
fn four_are_held_and_shed_keeps_the_last() {
    let t = Tmp::new("vc-lru");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let mut cache = Cache::new();
    let mut variants = Vec::new();
    for i in 0..5 {
        let mut r = roots(&t);
        r.libraries.push(Library {
            name: format!("extra{i}"),
            root: t.at(&format!("lib/extra{i}")),
            editable: false,
        });
        variants.push(r);
    }
    let held: Vec<Arc<Vocabulary>> = variants
        .iter()
        .map(|r| cache.vocabulary(&disk(r), r).expect("vocabulary"))
        .collect();
    assert_eq!(cache.vocabularies(), 4);
    assert!(same(
        &held[4],
        &cache.vocabulary(&disk(&variants[4]), &variants[4]).expect("vocabulary")
    ));
    assert!(!same(
        &held[0],
        &cache.vocabulary(&disk(&variants[0]), &variants[0]).expect("vocabulary")
    ));
    cache.shed();
    assert_eq!(cache.vocabularies(), 1);
    cache.forget();
    assert_eq!(cache.vocabularies(), 0);
}

// Decision 25: the projects of one workspace share the vocabulary.
#[test]
fn two_projects_of_a_workspace_share_one_vocabulary() {
    let t = Tmp::new("vc-share");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let (a, mut cache) = (roots(&t), Cache::new());
    let mut b = a.clone();
    b.project = t.at("ws/other");
    let first = cache.vocabulary(&disk(&a), &a).expect("vocabulary");
    assert!(same(&first, &cache.vocabulary(&disk(&b), &b).expect("vocabulary")));
}

// Decision 28: a draft in a new module folder rebuilds the vocabulary; an unrelated draft does not.
#[test]
fn a_draft_in_a_new_module_folder_rebuilds_and_an_unrelated_one_does_not() {
    let mut source = mem_lib(&[("audio/x/module.k7x", &plugin_text("X"))]);
    let (r, mut cache) = (mem_roots(), Cache::new());
    let first = cache.vocabulary(&source, &r).expect("vocabulary");
    assert!(first.modules.contains_key("@k7-core/audio/x"));
    source
        .set("/p/proj/nodes/lead.k7x", Some("content(\"x\")\n".into()))
        .expect("draft");
    assert!(
        same(&first, &cache.vocabulary(&source, &r).expect("vocabulary")),
        "an unrelated draft rebuilt it"
    );
    // A folder no listing names, two levels down.
    source
        .set("/p/lib/modules/audio/NEW/module.k7x", Some(plugin_text("New")))
        .expect("draft");
    let second = cache.vocabulary(&source, &r).expect("vocabulary");
    assert!(!same(&first, &second));
    assert!(second.modules.contains_key("@k7-core/audio/NEW"));
    // The folder is in its parent's listing, so a category is found one level down too.
    source
        .set("/p/lib/modules/audio/Cat/leaf/module.k7x", Some(plugin_text("Leaf")))
        .expect("draft");
    assert!(
        cache
            .vocabulary(&source, &r)
            .expect("vocabulary")
            .modules
            .contains_key("@k7-core/audio/Cat/leaf")
    );
    // An edit of a held module rebuilds, and a deletion removes it.
    source
        .set("/p/lib/modules/audio/x/module.k7x", Some(plugin_text("Edited")))
        .expect("draft");
    let edited = cache.vocabulary(&source, &r).expect("vocabulary");
    assert_eq!(
        edited.modules["@k7-core/audio/x"]
            .authored
            .plugin
            .as_ref()
            .and_then(|p| p.name.as_deref()),
        Some("Edited")
    );
    source.set("/p/lib/modules/audio/x/module.k7x", None).expect("draft");
    assert!(
        !cache
            .vocabulary(&source, &r)
            .expect("vocabulary")
            .modules
            .contains_key("@k7-core/audio/x")
    );
}

// A bundle that does not hold what the walk asks for is `Missing`, and the vocabulary is not made.
#[test]
fn a_bundle_without_the_listing_answers_missing() {
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: Default::default(),
        lists: Default::default(),
        ..Default::default()
    };
    let source = MemSource::from_bundle(bundle).expect("a bundle");
    let mut cache = Cache::new();
    assert_eq!(
        cache.vocabulary(&source, &mem_roots()).err(),
        Some(SourceError::Missing)
    );
    assert_eq!(cache.vocabularies(), 0);
}

// Decision 27: a reused vocabulary replays its reads into the Recorder, texts included.
#[test]
fn a_bundle_made_after_a_hit_equals_the_one_made_after_a_miss() {
    let t = Tmp::new("vc-replay");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.write("lib/k7-core/modules/audio/a/lead.k7x", "content(\"x\")\n");
    t.write(
        "home/exts.json",
        r#"{"entries":[{"plugins":[{"name":"Serum","format":"VST3"}]}]}"#,
    );
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let source = disk(&r);
    let (mounts, files) = r.mounts().expect("mounts");
    let miss = Recorder::new(&source);
    let first = cache.vocabulary(&miss, &r).expect("vocabulary");
    let after_miss = miss.into_bundle_with_files(&mounts, &files);
    let hit = Recorder::new(&source);
    let second = cache.vocabulary(&hit, &r).expect("vocabulary");
    assert!(same(&first, &second), "the second call was not a hit");
    let after_hit = hit.into_bundle_with_files(&mounts, &files);
    assert_eq!(after_hit, after_miss);
    assert!(after_hit.files.values().any(|f| {
        f.text
            .as_deref()
            .is_some_and(|t| t.contains("Amp") || t.contains("\"A\""))
    }));
    // The bundle is enough for the browser: a fresh cache builds the same vocabulary from it.
    let browser = MemSource::from_bundle(after_hit).expect("a bundle");
    let rebuilt = Cache::new().vocabulary(&browser, &r).expect("the bundle was enough");
    assert_eq!(
        rebuilt.modules.keys().collect::<Vec<_>>(),
        first.modules.keys().collect::<Vec<_>>()
    );
}

// Decision 27, the file layer: a rebuild that finds a decoded file in the cache still hands its text to the Recorder.
#[test]
fn a_rebuild_over_cached_files_still_records_their_texts() {
    let t = Tmp::new("vc-replay-files");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.write(
        "home/exts.json",
        r#"{"entries":[{"plugins":[{"name":"Serum","format":"VST3"}]}]}"#,
    );
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let source = disk(&r);
    cache.vocabulary(&source, &r).expect("vocabulary");
    t.write(
        "home/exts.json",
        r#"{"entries":[{"plugins":[{"name":"Massive","format":"VST3"}]}]}"#,
    );
    let rec = Recorder::new(&source);
    cache.vocabulary(&rec, &r).expect("vocabulary");
    let bundle = rec.into_bundle(&r.mounts().expect("mounts").0);
    let module = bundle
        .files
        .iter()
        .find(|(p, _)| p.ends_with("audio/a/module.k7x"))
        .expect("the module file");
    assert!(module.1.text.is_some(), "the module file was held by length alone");
}

// Decisions 35 and 795: a mount that answered NotFound is named in `absent`, so the browser reads "absent" and not "missing".
#[test]
fn an_absent_mount_is_named_absent_in_the_bundle() {
    let t = Tmp::new("vc-absent-mount");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let r = roots(&t);
    let source = disk(&r);
    let rec = Recorder::new(&source);
    let v = Cache::new().vocabulary(&rec, &r).expect("vocabulary");
    let (mounts, files) = r.mounts().expect("mounts");
    let bundle = rec.into_bundle_with_files(&mounts, &files);
    // Neither `meta/` nor the scan exists: both are named absent, and no listing is made up for either.
    assert!(bundle.absent.contains(&t.at("meta")) && bundle.absent.contains(&t.at("home/exts.json")));
    assert!(!bundle.lists.contains_key(&t.at("meta")) && !bundle.lists.contains_key(&t.at("home/exts.json")));
    let mem = MemSource::from_bundle(bundle).expect("a bundle");
    let again = Cache::new().vocabulary(&mem, &r).expect("the bundle was enough");
    assert_eq!(again.modules.len(), v.modules.len());
}

// Decision 44: a dot name is dropped from the listings the read set keeps, so a dot entry moves nothing.
#[test]
fn a_dot_entry_is_a_hit() {
    let t = Tmp::new("vc-dot-hit");
    t.write("lib/k7-core/modules/audio/a/module.k7x", &plugin_text("A"));
    t.age();
    let (r, mut cache) = (roots(&t), Cache::new());
    let first = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    t.write("lib/k7-core/modules/audio/.cache/x", "x");
    let second = cache.vocabulary(&disk(&r), &r).expect("vocabulary");
    assert!(same(&first, &second));
    assert_eq!(cache.vocabulary_hits(), 1);
}

// One Cache shared by two bundles with the same roots and one module text different: the second carries its own text.
#[test]
fn a_cache_shared_by_two_bundles_serves_each_its_own_text() {
    let a = mem_lib(&[("audio/x/module.k7x", &plugin_text("Alpha"))]);
    let b = mem_lib(&[("audio/x/module.k7x", &plugin_text("Beta"))]);
    let (r, mut cache) = (mem_roots(), Cache::new());
    let name = |v: &Vocabulary| {
        v.modules["@k7-core/audio/x"]
            .authored
            .plugin
            .as_ref()
            .and_then(|p| p.name.clone())
    };
    assert_eq!(
        name(&cache.vocabulary(&a, &r).expect("vocabulary")).as_deref(),
        Some("Alpha")
    );
    assert_eq!(
        name(&cache.vocabulary(&b, &r).expect("vocabulary")).as_deref(),
        Some("Beta")
    );
    assert_eq!(
        name(&cache.vocabulary(&a, &r).expect("vocabulary")).as_deref(),
        Some("Alpha")
    );
}

// Decision 25: two projects in different sub-folders have different `_modules/`, so they never share a vocabulary.
#[test]
fn projects_in_different_sub_folders_do_not_share_a_vocabulary() {
    let t = Tmp::new("vc-subfolders");
    t.write("ws/_modules/audio/top/module.k7x", &plugin_text("Top"));
    t.write("ws/sub/_modules/audio/deep/module.k7x", &plugin_text("Deep"));
    t.age();
    let mut cache = Cache::new();
    let top = roots(&t);
    let mut sub = top.clone();
    sub.project = t.at("ws/sub/proj");
    sub.workspace = Some(t.at("ws/sub"));
    assert_ne!(top.vocabulary_key(), sub.vocabulary_key());
    let a = cache.vocabulary(&disk(&top), &top).expect("vocabulary");
    let b = cache.vocabulary(&disk(&sub), &sub).expect("vocabulary");
    assert!(a.modules.contains_key("@workspace/audio/top") && !a.modules.contains_key("@workspace/audio/deep"));
    assert!(b.modules.contains_key("@workspace/audio/deep") && !b.modules.contains_key("@workspace/audio/top"));
    assert!(same(&a, &cache.vocabulary(&disk(&top), &top).expect("vocabulary")));
}

// A read set that meets what the source cannot answer is not valid: the rebuild decides, and says `Missing`.
#[test]
fn a_source_that_cannot_answer_invalidates_and_the_rebuild_says_missing() {
    let r = mem_roots();
    let full = mem_lib(&[("audio/x/module.k7x", &plugin_text("X"))]);
    let mut cache = Cache::new();
    cache.vocabulary(&full, &r).expect("vocabulary");
    let bundle = SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: Default::default(),
        lists: Default::default(),
        ..Default::default()
    };
    let empty = MemSource::from_bundle(bundle).expect("a bundle");
    assert_eq!(cache.vocabulary(&empty, &r).err(), Some(SourceError::Missing));
}
