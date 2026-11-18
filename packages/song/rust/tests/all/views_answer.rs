// views_answer.rs — one call, every view: what `answer` reads once, what it gives when the compile refuses, and how it measures.
// A reader would get wrong that a refused compile loses the other views: hints, surfaces, ports and the catalogue read the files alone.

use crate::support;
use crate::views_support;
use crate::views_world;

use serde_json::json;
use song::doc::{Request, Vsts};
use song::files::Severity;
use song::lanes::rig::Rig;
use song::record::Recorder;
use song::views::answer::{Want, answer};
use song::views::lengths::ByTable;
use song::views::web::web_project_of;
use song::{Cache, Entry, MemSource, Source, SourceError, Span, Stat};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::Tmp;
use views_support::wav;
use views_world::{roots, world};

fn request(roots: song::Roots) -> Request {
    Request {
        roots,
        rig: Rig::default(),
        vsts: Vsts::Scan,
        thawed: Vec::new(),
    }
}

fn everything() -> Want {
    Want {
        engine: true,
        web: true,
        errors: true,
        hints: true,
        surfaces: true,
        io_ports: true,
        catalogue: true,
        base_modules: true,
    }
}

const LANE: &str = "extends(\"@meta/track\")\n";
const GRAPH: &str = "modules({ gtr: [\"MIX\"], MIX: [\"OUT\"] })\n";

// 642: a view not asked for is absent, and nothing is compiled for the lists alone.
#[test]
fn only_the_views_asked_for_are_answered() {
    let src = world(&[("nodes/gtr.k7x", LANE), ("graph.k7x", GRAPH)]);
    let none = answer(&src, &mut Cache::new(), &request(roots()), &Want::default()).expect("an answer");
    assert!(none.compiled.is_none() && none.web.is_none() && none.errors.is_none() && none.hints.is_none());
    assert!(
        none.surfaces.is_none() && none.io_ports.is_none() && none.catalogue.is_none() && none.base_modules.is_none()
    );
    let lists = Want {
        hints: true,
        base_modules: true,
        ..Want::default()
    };
    let v = answer(&src, &mut Cache::new(), &request(roots()), &lists).expect("an answer");
    assert!(v.compiled.is_none() && v.web.is_none() && v.errors.is_none());
    assert!(v.hints.is_some() && v.base_modules.is_some() && v.catalogue.is_none());
    let engine = Want {
        engine: true,
        ..Want::default()
    };
    let v = answer(&src, &mut Cache::new(), &request(roots()), &engine).expect("an answer");
    assert!(v.compiled.is_some_and(|c| c.engine.is_ok()) && v.web.is_none());
}

// 605, 642: the compile refuses, `web` is the Failure, and every other view is there all the same.
#[test]
fn a_refused_compile_still_answers_the_lists_and_the_surfaces() {
    let src = world(&[
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\nparams(((\n"),
        ("graph.k7x", GRAPH),
        ("surfaces/t.k7x", "type(\"timeline\")\nlayout(\"split\")\n"),
    ]);
    let v = answer(&src, &mut Cache::new(), &request(roots()), &everything()).expect("an answer");
    let failure = v.web.expect("asked for").expect_err("a refusal");
    assert!(failure.diagnostics.iter().any(|d| d.file == "nodes/gtr.k7x"));
    assert!(v.compiled.is_some_and(|c| c.engine.is_err()));
    let e = v.errors.expect("errors");
    assert!(!e.errors.is_empty() && e.diagnostics.iter().all(|d| d.severity == Severity::Error));
    assert_eq!(v.surfaces.expect("surfaces").surfaces.len(), 1);
    assert!(v.hints.is_some() && v.io_ports.is_some());
    assert!(v.catalogue.is_some_and(|c| !c.items.is_empty()) && v.base_modules.is_some());
}

// 611, 612: on disk `answer` measures every audio source once; the table it made redraws the same web project.
#[test]
fn every_length_is_measured_on_disk_and_the_table_redraws_it() {
    let t = Tmp::new("answer-web");
    t.write(
        "ws/proj/nodes/gtr.k7x",
        &format!("{LANE}events([{{ ref: \"k\", start: \"1:1\" }}])\n"),
    );
    t.write("ws/proj/graph.k7x", GRAPH);
    t.write("ws/proj/project.k7x", "");
    t.write("ws/proj/clips/k.k7x", "source(\"k.wav\")\n");
    t.write("ws/proj/clips/z.k7x", "source(\"z.wav\")\n");
    fs::write(t.0.join("ws/proj/k.wav"), wav(3.0)).expect("a wav");
    fs::write(t.0.join("ws/proj/z.wav"), wav(5.0)).expect("a wav");
    t.age();
    let roots = support::roots(&t);
    let src = support::disk(&roots);
    let want = Want {
        web: true,
        ..Want::default()
    };
    let v = answer(&src, &mut Cache::new(), &request(roots.clone()), &want).expect("an answer");
    let a = v.web.expect("asked for").expect("a web project");
    let path = format!("{}/k.wav", roots.project);
    assert_eq!(a.lengths.get(&path).copied(), Some(3.0));
    // A clip no lane places is measured too, so a draft that places it finds its length (612).
    assert_eq!(a.lengths.get(&format!("{}/z.wav", roots.project)).copied(), Some(5.0));
    assert!(a.findings.is_empty());
    // 3 seconds at 120 beats a minute: the clip states no length, so it draws 6 beats.
    assert_eq!(a.web.tracks[0].clips[0].length, 6.0);
    let again = web_project_of(&compiled_doc(&src, &roots), &mut ByTable(&a.lengths)).expect("a replay");
    assert_eq!(again, a.web);
}

fn compiled_doc(src: &dyn Source, roots: &song::Roots) -> song::doc::ProjectDoc {
    let c = song::doc::compile(src, &mut Cache::new(), &request(roots.clone())).expect("a compile");
    c.engine.expect("a document")
}

// 611: a bundle holds a binary by its length alone, so `answer` over one cannot measure and says it needs the file.
#[test]
fn a_bundle_cannot_measure_a_header_and_answers_missing() {
    let t = Tmp::new("answer-bundle");
    t.write(
        "ws/proj/nodes/gtr.k7x",
        &format!("{LANE}events([{{ ref: \"k\", start: \"1:1\" }}])\n"),
    );
    t.write("ws/proj/graph.k7x", GRAPH);
    t.write("ws/proj/project.k7x", "");
    t.write("ws/proj/clips/k.k7x", "source(\"k.wav\")\n");
    fs::write(t.0.join("ws/proj/k.wav"), wav(1.0)).expect("a wav");
    t.age();
    let roots = support::roots(&t);
    let (mounts, files) = roots.mounts().expect("mounts");
    let disk = song::DiskSource::with_files(&mounts, &files).expect("a source");
    let rec = Recorder::new(&disk);
    let want = Want {
        web: true,
        ..Want::default()
    };
    answer(&rec, &mut Cache::new(), &request(roots.clone()), &want).expect("an answer");
    let mem = MemSource::from_bundle(rec.into_bundle_with_files(&mounts, &files)).expect("a bundle");
    let r = answer(&mem, &mut Cache::new(), &request(roots), &want);
    assert!(matches!(r, Err(SourceError::Missing)), "{:?}", r.map(|_| ()));
}

// 643, 330: a timeline surface whose mapping does not read is no timeline: the recorder is placed and the surface is a finding.
#[test]
fn a_timeline_surface_that_does_not_read_places_the_recorder() {
    let files = |surface: &'static str| {
        let r = answer(
            &world(&[
                ("nodes/gtr.k7x", LANE),
                ("graph.k7x", GRAPH),
                ("surfaces/t.k7x", surface),
            ]),
            &mut Cache::new(),
            &request(roots()),
            &Want {
                engine: true,
                ..Want::default()
            },
        )
        .expect("an answer")
        .compiled
        .expect("a compile");
        let doc = r.engine.expect("a document");
        (doc.master_plugins.len(), r.diagnostics)
    };
    let (recorders, found) = files("type(\"timeline\")\nlayout(\"split\")\n");
    assert_eq!((recorders, found.len()), (0, 0));
    let (recorders, found) = files("type(\"timeline\")\nlayout(\"split\")\nmapping({ a: 3 })\n");
    assert_eq!(recorders, 1);
    assert!(
        found
            .iter()
            .any(|f| f.file == "surfaces/t.k7x" && f.severity == Severity::Error)
    );
}

// 642: the hints, I/O and surfaces of a project that compiles agree with what the lists give alone.
#[test]
fn a_clean_project_answers_every_view_and_a_second_call_agrees() {
    let src = world(&[
        ("nodes/gtr.k7x", LANE),
        ("graph.k7x", GRAPH),
        ("surfaces/t.k7x", "type(\"timeline\")\nlayout(\"split\")\n"),
    ]);
    let mut cache = Cache::new();
    let v = answer(&src, &mut cache, &request(roots()), &everything()).expect("an answer");
    assert!(v.web.expect("web").is_ok());
    let e = v.errors.expect("errors");
    assert!(e.errors.is_empty() && e.format_too_new.is_none());
    assert!(v.hints.expect("hints").hints.tracks.iter().any(|t| t == "gtr"));
    let again = answer(&src, &mut cache, &request(roots()), &everything()).expect("an answer");
    assert_eq!(
        serde_json::to_value(again.surfaces).expect("json"),
        json!({"surfaces": [{"id": "t", "file": "surfaces/t.k7x", "surface": {"type": "timeline", "layout": "split"}}], "diagnostics": []})
    );
}

/// A source that counts the listings of every `nodes` folder it is asked for.
struct Listings<'a>(&'a MemSource, AtomicUsize);

impl Source for Listings<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.0.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        if dir.ends_with("/nodes") {
            self.1.fetch_add(1, Ordering::Relaxed);
        }
        self.0.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.0.read(path, span)
    }
}

// 642: the project's reads are made once for a compile and the views that follow it, not once more for each.
#[test]
fn the_compiles_reads_serve_the_views_of_the_same_call() {
    let src = world(&[("nodes/gtr.k7x", LANE), ("graph.k7x", GRAPH)]);
    let alone = Listings(&src, AtomicUsize::new(0));
    song::doc::compile(&alone, &mut Cache::new(), &request(roots())).expect("a compile");
    let once = alone.1.load(Ordering::Relaxed);
    assert!(once > 0);
    let many = Listings(&src, AtomicUsize::new(0));
    let want = Want {
        engine: true,
        errors: true,
        io_ports: true,
        ..Want::default()
    };
    let v = answer(&many, &mut Cache::new(), &request(roots()), &want).expect("an answer");
    assert!(v.errors.is_some() && v.io_ports.is_some());
    assert_eq!(many.1.load(Ordering::Relaxed), once);
}
