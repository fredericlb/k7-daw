// views_walk.rs — the media walk's loop test: folders with the same listing are two folders, on disk and in a bundle replay.
// A reader would get wrong that a MemSource stamps a folder by its listing alone: p/a and p/a/a then look like one folder.

use crate::support;
use crate::views_support;
use crate::views_world;

use indexmap::IndexMap;
use song::record::Recorder;
use song::views::hints::{HintsAnswer, hints_of};
use song::{Cache, DiskSource, Entry, MemSource, Roots, Source, SourceError, Span, Stat};
use support::Tmp;
use views_support::fast;
use views_world::bare;

fn media(source: &dyn Source, project: &str) -> HintsAnswer {
    let mut inputs = bare(IndexMap::new());
    inputs.project = project.to_owned();
    let roots = Roots {
        project: project.to_owned(),
        ..Roots::default()
    };
    hints_of(source, &mut Cache::new(), &roots, &inputs).expect("hints")
}

fn folders(a: &HintsAnswer) -> Vec<&str> {
    a.hints.media.keys().map(String::as_str).collect()
}

#[test]
fn nested_folders_with_the_same_listing_are_walked_on_disk_and_in_a_replay() {
    // Each of proj, a and a/a lists only `a`; the audio sits three folders down.
    let t = Tmp::new("walk-same-listing");
    t.write("ws/proj/a/a/a/x.wav", "x");
    t.age();
    let root = support::spell(&t.0.join("ws"));
    let disk = DiskSource::new(std::slice::from_ref(&root)).expect("source");
    let rec = Recorder::new(&disk);
    let on_disk = fast("the disk walk", || media(&rec, &t.at("ws/proj")));
    assert_eq!(folders(&on_disk), ["a/a/a"]);
    let bundle = rec.into_bundle(std::slice::from_ref(&root));
    let mem = MemSource::from_bundle(bundle).expect("a bundle");
    let replayed = fast("the replayed walk", || media(&mem, &t.at("ws/proj")));
    assert_eq!(on_disk.hints.media, replayed.hints.media);
}

/// Every file and folder under `dir` set to one time, which a tar or rsync tree has.
#[cfg(unix)]
fn same_time(dir: &std::path::Path, at: std::time::SystemTime) {
    for e in std::fs::read_dir(dir).expect("list").flatten() {
        let p = e.path();
        if p.is_dir() {
            same_time(&p, at);
        }
        std::fs::File::open(&p)
            .and_then(|f| f.set_modified(at))
            .expect("a time");
    }
    std::fs::File::open(dir)
        .and_then(|f| f.set_modified(at))
        .expect("a time");
}

// 630, the walk's loop test: two real folders with one listing and one mtime are two folders, not a loop (a tar or rsync tree).
#[cfg(unix)]
#[test]
fn real_nested_folders_with_one_listing_and_one_mtime_are_both_walked() {
    let t = Tmp::new("walk-same-mtime");
    // `a` and `a/a` both list [a, b.wav].
    t.write("ws/proj/a/b.wav", "x");
    t.write("ws/proj/a/a/b.wav", "x");
    t.write("ws/proj/a/a/a/c.wav", "x");
    same_time(
        &t.0,
        std::time::SystemTime::now() - std::time::Duration::from_secs(3600),
    );
    let root = support::spell(&t.0.join("ws"));
    let disk = DiskSource::new(std::slice::from_ref(&root)).expect("source");
    let rec = Recorder::new(&disk);
    let on_disk = fast("the disk walk", || media(&rec, &t.at("ws/proj")));
    assert_eq!(folders(&on_disk), ["a", "a/a", "a/a/a"]);
    let bundle = rec.into_bundle(std::slice::from_ref(&root));
    let mem = MemSource::from_bundle(bundle).expect("a bundle");
    let replayed = fast("the replayed walk", || media(&mem, &t.at("ws/proj")));
    assert_eq!(on_disk.hints.media, replayed.hints.media);
}

/// A memory source that answers one folder's listing with an error, as 2l's bundle refuses what lies outside its mounts.
struct Refusing<'a>(&'a MemSource, &'a str, SourceError);

impl Source for Refusing<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.0.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        if dir == self.1 {
            return Err(self.2.clone());
        }
        self.0.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.0.read(path, span)
    }
}

// 630: only `Missing` says a folder was never listed below; a refused child is a real folder, so its siblings are still walked.
#[test]
fn a_refused_child_is_not_a_loop() {
    let m = support::mem(&[
        ("ws/proj/a/b.wav", "x"),
        ("ws/proj/a/a/b.wav", "x"),
        ("ws/proj/a/a/a/b.wav", "x"),
    ]);
    // `a` and `a/a` list [a, b.wav]; `a/a/a` is refused, so `a/a`'s first child errs with Outside.
    let refusing = Refusing(&m, "/p/ws/proj/a/a/a", SourceError::Outside);
    let a = media(&refusing, "/p/ws/proj");
    assert_eq!(folders(&a), ["a", "a/a"]);
}

// 630: one folder of an incomplete bundle that is not there is no loop while its siblings are, so the walk asks for the files.
#[test]
fn an_incomplete_bundle_is_not_taken_for_a_loop() {
    let m = support::mem(&[
        ("ws/proj/a/b.wav", "x"),
        ("ws/proj/a/c/c.wav", "x"),
        ("ws/proj/a/a/b.wav", "x"),
        ("ws/proj/a/a/c/c.wav", "x"),
        ("ws/proj/a/a/a/b.wav", "x"),
        ("ws/proj/a/a/c/c.wav", "x"),
    ]);
    // `a` and `a/a` list [a, b.wav, c]; `a/a/a` is not in the bundle but `a/a/c` is.
    let gone = Refusing(&m, "/p/ws/proj/a/a/a", SourceError::Missing);
    let mut inputs = bare(IndexMap::new());
    inputs.project = "/p/ws/proj".to_owned();
    let roots = Roots {
        project: "/p/ws/proj".to_owned(),
        ..Roots::default()
    };
    let got = hints_of(&gone, &mut Cache::new(), &roots, &inputs);
    assert_eq!(got.err(), Some(SourceError::Missing));
}

#[test]
fn a_memory_source_holding_such_folders_walks_them_too() {
    let m = support::mem(&[("ws/proj/a/a/a/x.wav", "x")]);
    let a = media(&m, "/p/ws/proj");
    assert_eq!(folders(&a), ["a/a/a"]);
}

#[cfg(unix)]
mod links {
    use super::*;

    #[test]
    fn a_link_loop_is_cut_on_disk_and_in_its_replay_alike() {
        let t = Tmp::new("walk-loop-replay");
        t.write("ws/proj/a/deep.wav", "x");
        t.write("ws/proj/top.wav", "x");
        t.age();
        std::os::unix::fs::symlink(".", t.0.join("ws/proj/a/loop")).expect("a link");
        std::os::unix::fs::symlink(".", t.0.join("ws/proj/current")).expect("a link");
        let root = support::spell(&t.0.join("ws"));
        let disk = DiskSource::new(std::slice::from_ref(&root)).expect("source");
        let rec = Recorder::new(&disk);
        let on_disk = fast("the disk walk", || media(&rec, &t.at("ws/proj")));
        assert_eq!(folders(&on_disk), ["", "a"]);
        let bundle = rec.into_bundle(std::slice::from_ref(&root));
        let mem = MemSource::from_bundle(bundle).expect("a bundle");
        let replayed = fast("the replayed walk", || media(&mem, &t.at("ws/proj")));
        assert_eq!(on_disk.hints.media, replayed.hints.media);
        assert_eq!(on_disk.capped, replayed.capped);
    }
}

#[cfg(not(unix))]
#[test]
fn link_loops_need_a_platform_that_makes_them_cheaply() {
    support::skip("link loop replay", "a symbolic link is not made without Developer Mode");
}
