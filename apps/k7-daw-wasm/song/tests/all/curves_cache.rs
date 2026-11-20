// curves_cache.rs — the curves folder read through the cache: drafts, settled stamps, the bundle, and the folders that are not there.
// A reader would get wrong that the vocabulary is untouched by a curve: a curve file is never part of its read set (decision 206).

use crate::support;

use song::curves::read_curves;
use song::record::Recorder;
use song::{Cache, DiskSource, Library, MemSource, Roots, Severity};
use std::sync::Arc;
use support::{Tmp, mem, plugin_text};

const LFO: &str = "type(\"lfo\")\n";
const SAW: &str = "type(\"lfo\")\nshape(\"saw\")\n";

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

// decision 206: a draft of a curve file changes the read and does not rebuild the vocabulary.
#[test]
fn a_draft_changes_the_curves_and_leaves_the_vocabulary_alone() {
    let plugin = plugin_text("A");
    let mut src = mem(&[
        ("lib/modules/audio/a/module.k7x", plugin.as_str()),
        ("proj/curves/a.k7x", LFO),
    ]);
    let (roots, mut cache) = (mem_roots(), Cache::new());
    let first = cache.vocabulary(&src, &roots).expect("vocabulary");
    let before = read_curves(&src, &mut cache, "/p/proj").expect("reads");
    assert_eq!(
        serde_json::to_value(&*before.curves["a"]).expect("json")["shape"],
        "sine"
    );
    src.set("/p/proj/curves/a.k7x", Some(SAW.into())).expect("set");
    let after = read_curves(&src, &mut cache, "/p/proj").expect("reads");
    assert_eq!(serde_json::to_value(&*after.curves["a"]).expect("json")["shape"], "saw");
    let hits = cache.vocabulary_hits();
    let second = cache.vocabulary(&src, &roots).expect("vocabulary");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.vocabulary_hits(), hits + 1);
    src.set("/p/proj/curves/b.k7x", Some(LFO.into())).expect("set");
    assert_eq!(read_curves(&src, &mut cache, "/p/proj").expect("reads").curves.len(), 2);
    src.set("/p/proj/curves/a.k7x", None).expect("set");
    assert_eq!(read_curves(&src, &mut cache, "/p/proj").expect("reads").curves.len(), 1);
}

// decision 4: a second read of a settled, unchanged file is a hit, the same Arc.
#[test]
fn a_second_read_of_a_settled_file_is_a_hit() {
    let src = mem(&[("curves/a.k7x", LFO)]);
    let mut cache = Cache::new();
    let a = read_curves(&src, &mut cache, "/p").expect("reads");
    let b = read_curves(&src, &mut cache, "/p").expect("reads");
    assert!(Arc::ptr_eq(&a.curves["a"], &b.curves["a"]));
    assert_eq!(cache.len(), 1);
}

// decision 15: an unsettled stamp is never a key, and an aged disk tree settles.
#[test]
fn an_unsettled_stamp_is_never_a_key_and_an_aged_one_is() {
    let t = Tmp::new("curves-settle");
    t.write("p/curves/a.k7x", LFO);
    let project = t.at("p");
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
    let mut cache = Cache::new();
    let fresh = read_curves(&disk, &mut cache, &project).expect("reads");
    let again = read_curves(&disk, &mut cache, &project).expect("reads");
    assert!(!Arc::ptr_eq(&fresh.curves["a"], &again.curves["a"]));
    assert!(cache.is_empty());
    t.age();
    let settled = read_curves(&disk, &mut cache, &project).expect("reads");
    let hit = read_curves(&disk, &mut cache, &project).expect("reads");
    assert!(Arc::ptr_eq(&settled.curves["a"], &hit.curves["a"]));
}

// decision 36: a cache hit gives a Recorder its text, so a bundle made after one can still be decoded.
#[test]
fn a_bundle_made_after_a_cache_hit_still_holds_the_text() {
    let t = Tmp::new("curves-hit");
    t.write("p/curves/a.k7x", SAW);
    t.age();
    let project = t.at("p");
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
    let mut cache = Cache::new();
    read_curves(&Recorder::new(&disk), &mut cache, &project).expect("reads");
    let rec = Recorder::new(&disk);
    let hit = read_curves(&rec, &mut cache, &project).expect("reads");
    let bundle = rec.into_bundle(std::slice::from_ref(&project));
    let replay = MemSource::from_bundle(bundle).expect("a bundle");
    let again = read_curves(&replay, &mut Cache::new(), &project).expect("the bundle is enough");
    assert_eq!(*again.curves["a"], *hit.curves["a"]);
}

// decision 204: one broken file beside a good one is a warning for that file and the good one reads.
#[test]
fn one_broken_file_beside_a_good_one_is_a_warning_and_the_good_one_reads() {
    let src = mem(&[
        ("curves/broken.k7x", "type(\"lfo\"\n"),
        ("curves/ok.k7x", LFO),
        ("curves/bad.k7x", "type(\"x\")\n"),
    ]);
    let read = read_curves(&src, &mut Cache::new(), "/p").expect("reads");
    assert_eq!(read.curves.keys().collect::<Vec<_>>(), ["ok"]);
    let mut files: Vec<&str> = read.findings.iter().map(|f| f.file.as_str()).collect();
    files.sort_unstable();
    assert_eq!(files, ["curves/bad.k7x", "curves/broken.k7x"]);
    assert!(read.findings.iter().all(|f| f.severity == Severity::Warning));
    let syntax = read
        .findings
        .iter()
        .find(|f| f.file == "curves/broken.k7x")
        .expect("finding");
    assert!(syntax.line >= 1 && syntax.col >= 1);
}

// no TS test: the folder absent, and the folder a file, are an empty read.
#[test]
fn a_missing_folder_and_a_folder_that_is_a_file_are_empty() {
    let none = mem(&[("clips/a.k7x", "source(\"a.wav\")\n")]);
    let read = read_curves(&none, &mut Cache::new(), "/p").expect("reads");
    assert!(read.curves.is_empty() && read.findings.is_empty());
    let file = mem(&[("curves", "not a folder\n")]);
    let read = read_curves(&file, &mut Cache::new(), "/p").expect("reads");
    assert!(read.curves.is_empty() && read.findings.is_empty());
}

// the throw-or-finding table: `Missing` from a memory Source is an error, so the compile can ask for a larger bundle.
#[test]
fn a_path_the_bundle_does_not_hold_is_missing() {
    let bare = song::SourceBundle {
        format: 1,
        mounts: vec!["/p".into()],
        files: std::collections::BTreeMap::new(),
        lists: std::collections::BTreeMap::new(),
        ..Default::default()
    };
    let src = MemSource::from_bundle(bare).expect("a source");
    assert!(read_curves(&src, &mut Cache::new(), "/p").is_err());
}

// decision 205: a refused curves folder is one warning named `curves`; a refused file costs that file alone.
#[test]
fn a_link_out_of_the_mounts_is_one_warning_named_curves() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        for folder_link in [false, true] {
            let t = Tmp::new("curves-outside");
            t.write("outside/b.k7x", LFO);
            t.write("p/curves/ok.k7x", LFO);
            if folder_link {
                std::fs::remove_dir_all(t.0.join("p/curves")).expect("remove");
                symlink(t.0.join("outside"), t.0.join("p/curves")).expect("link");
            } else {
                symlink(t.0.join("outside/b.k7x"), t.0.join("p/curves/out.k7x")).expect("link");
            }
            let project = t.at("p");
            let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
            let read = read_curves(&disk, &mut Cache::new(), &project).expect("reads");
            assert_eq!(read.curves.is_empty(), folder_link, "folder_link {folder_link}");
            assert_eq!(read.findings.len(), 1);
            let named = if folder_link { "curves" } else { "curves/out.k7x" };
            assert_eq!(read.findings[0].file, named);
            assert_eq!(read.findings[0].severity, Severity::Warning);
        }
    }
    #[cfg(not(unix))]
    support::skip(
        "a_link_out_of_the_mounts_is_one_warning_named_curves",
        "no file link without Developer Mode",
    );
}

// decision 204, the throw-or-finding table: a file the disk will not read is a warning for it, and its siblings still read.
#[test]
fn an_unreadable_file_is_a_warning_and_its_siblings_read() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let t = Tmp::new("curves-locked");
        t.write("p/curves/ok.k7x", LFO);
        t.write("p/curves/locked.k7x", LFO);
        let locked = t.0.join("p/curves/locked.k7x");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::read(&locked).is_ok() {
            support::skip(
                "an_unreadable_file_is_a_warning_and_its_siblings_read",
                "the user reads mode 000",
            );
        } else {
            let project = t.at("p");
            let disk = DiskSource::new(std::slice::from_ref(&project)).expect("a source");
            let read = read_curves(&disk, &mut Cache::new(), &project).expect("reads");
            assert_eq!(read.curves.keys().collect::<Vec<_>>(), ["ok"]);
            assert_eq!(read.findings.len(), 1);
            assert_eq!(read.findings[0].file, "curves/locked.k7x");
            assert_eq!(read.findings[0].severity, Severity::Warning);
        }
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).expect("chmod back");
    }
    #[cfg(not(unix))]
    support::skip("an_unreadable_file_is_a_warning_and_its_siblings_read", "no POSIX mode");
}
