// bundle_replay.rs — what a disk answered, a bundle made through a Recorder answers the same (decisions 743 to 746).
// A reader would get wrong that nothing here may end in Missing: each case names a pointer into the bundle that shows the path was asked.

use crate::clips_support;
use crate::support;

use serde_json::json;
use song::clips::{audio_paths, read_pattern};
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots, Source, SourceBundle, Span};
use std::fs;
use support::{Tmp, dump, dump_modules, roots, skip};

const AMP: &str = "lib/k7-core/modules/audio/amp";

/// The tree of one case: roots like a module test's, an `outside` folder no mount covers.
struct Lab {
    t: Tmp,
    roots: Roots,
}

/// What a case asks of a Source, beyond the compile's own readers.
#[derive(Default)]
struct Ask {
    probes: Vec<String>,
    audio: Vec<(&'static str, String)>,
    patterns: Vec<&'static str>,
}

fn lab(tag: &str) -> Lab {
    let t = Tmp::new(tag);
    t.mkdir("ws/proj");
    t.mkdir("outside");
    let roots = roots(&t);
    Lab { t, roots }
}

impl Lab {
    fn at(&self, rel: &str) -> String {
        self.t.at(rel)
    }

    fn project(&self) -> String {
        self.at("ws/proj")
    }

    /// A link at `rel` to `outside/<target>`; false where this platform cannot make one.
    fn link(&self, test: &str, rel: &str, target: &str) -> bool {
        #[cfg(unix)]
        {
            let _ = test;
            let from = self.t.0.join(rel);
            fs::create_dir_all(from.parent().expect("a parent")).expect("folder");
            std::os::unix::fs::symlink(self.t.0.join("outside").join(target), from).is_ok()
        }
        #[cfg(not(unix))]
        {
            let _ = (rel, target);
            skip(test, "a link needs a privilege here");
            false
        }
    }
}

/// An answer as text; an `Io` keeps its kind only, since the message is the operating system's.
fn shown<T: std::fmt::Debug>(r: Result<T, song::SourceError>) -> String {
    match r {
        Err(song::SourceError::Io(_)) => "Err(Io)".to_owned(),
        other => format!("{other:?}"),
    }
}

/// Every answer of the readers and of direct calls, as text, so a disk and a bundle compare with `==`.
fn observe(src: &dyn Source, l: &Lab, ask: &Ask) -> Vec<String> {
    let project = l.project();
    let mut out = Vec::new();
    out.push(match dump(src, &project) {
        Ok(d) => d.json.to_string(),
        Err(e) => format!("dump {e:?}"),
    });
    out.push(match dump_modules(src, &mut Cache::new(), &l.roots) {
        Ok(d) => d.json.to_string(),
        Err(e) => format!("modules {e:?}"),
    });
    out.push(
        match song::assemblies::resolve_assemblies(src, &mut Cache::new(), &l.roots) {
            Ok(r) => format!(
                "{:?}",
                r.findings.iter().map(|f| (&f.file, f.severity)).collect::<Vec<_>>()
            ),
            Err(e) => format!("assemblies {e:?}"),
        },
    );
    for p in &ask.probes {
        out.push(format!(
            "stat {p}: {}",
            shown(
                src.stat(p)
                    .map(|s| (s.kind, if s.kind == song::Kind::File { s.len } else { 0 }))
            )
        ));
        out.push(format!("list {p}: {}", shown(src.list(p))));
        out.push(format!(
            "read {p}: {}",
            shown(
                src.read(p, Span::All)
                    .map(|b| std::str::from_utf8(&b).ok().map(str::len))
            )
        ));
    }
    if !ask.audio.is_empty() {
        let items: Vec<(&str, serde_json::Value)> =
            ask.audio.iter().map(|(id, s)| (*id, json!({ "source": s }))).collect();
        out.push(shown(audio_paths(
            src,
            &project,
            &clips_support::clip_family(&items),
            song::PathRule::HOST,
        )));
    }
    for p in &ask.patterns {
        out.push(format!(
            "pattern {p}: {}",
            shown(read_pattern(src, &project, p, song::PathRule::HOST))
        ));
    }
    out
}

/// Reads through a Recorder on the disk, then through the bundle it made: the two must agree and neither says Missing.
fn replayed(l: &Lab, ask: &Ask) -> SourceBundle {
    let (dirs, files) = l.roots.mounts().expect("mounts");
    let disk = DiskSource::with_files(&dirs, &files).expect("a source");
    let rec = Recorder::new(&disk);
    let on_disk = observe(&rec, l, ask);
    assert_eq!(rec.overflow(), None);
    let bundle = rec.into_bundle_with_files(&dirs, &files);
    let json = serde_json::to_string(&bundle).expect("json");
    let mem = MemSource::from_bundle(serde_json::from_str(&json).expect("back")).expect("a source");
    let again = observe(&mem, l, ask);
    same(&on_disk, &again);
    assert!(!again.iter().any(|a| a.contains("Missing")), "{again:?}");
    bundle
}

/// Panics naming the first answer that differs, with the text around the first differing byte.
fn same(disk: &[String], mem: &[String]) {
    assert_eq!(disk.len(), mem.len());
    for (i, (a, b)) in disk.iter().zip(mem).enumerate() {
        if a != b {
            let at = a
                .bytes()
                .zip(b.bytes())
                .position(|(x, y)| x != y)
                .unwrap_or(a.len().min(b.len()));
            let cut = |s: &str| {
                s.get(at.saturating_sub(80)..(at + 120).min(s.len()))
                    .unwrap_or("")
                    .to_owned()
            };
            panic!("answer {i} differs at {at}:\n disk {}\n mem  {}", cut(a), cut(b));
        }
    }
}

/// Panics naming the fields in which two bundles differ.
fn same_bundle(a: &SourceBundle, b: &SourceBundle) {
    let (a, b) = (
        serde_json::to_value(a).expect("json"),
        serde_json::to_value(b).expect("json"),
    );
    for key in a
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
        .chain(b.as_object().into_iter().flatten().map(|(k, _)| k))
    {
        if a[key] != b[key] {
            panic!(
                "{key} differs:\n first  {}\n second {}",
                a[key].to_string().chars().take(900).collect::<String>(),
                b[key].to_string().chars().take(900).collect::<String>()
            );
        }
    }
}

fn probes(l: &Lab, rels: &[&str]) -> Vec<String> {
    rels.iter().map(|r| l.at(r)).collect()
}

#[test]
fn a_link_out_as_a_file_replays_as_outside() {
    let l = lab("replay-link-file");
    l.t.write("outside/kick.wav", "RIFF");
    l.t.write("ws/proj/clips/kick.k7x", "source(\"clips/kick.wav\")\n");
    if !l.link("a_link_out_as_a_file", "ws/proj/clips/kick.wav", "kick.wav") {
        return;
    }
    l.t.age();
    let ask = Ask {
        probes: probes(&l, &["ws/proj/clips/kick.wav"]),
        audio: vec![("kick", "clips/kick.wav".into())],
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    let path = l.at("ws/proj/clips/kick.wav");
    assert!(b.refused.contains(&path), "{:?}", b.refused);
    let mem = MemSource::from_bundle(b).expect("a source");
    assert_eq!(mem.stat(&path), Err(song::SourceError::Outside));
}

#[test]
fn a_link_out_as_a_folder_replays_as_outside_for_the_folder_and_what_is_asked_below() {
    let l = lab("replay-link-dir");
    l.t.write("outside/algo/beat.strudel", "s(\"bd\")");
    if !l.link("a_link_out_as_a_folder", "ws/proj/algo", "algo") {
        return;
    }
    l.t.age();
    let ask = Ask {
        probes: probes(&l, &["ws/proj/algo", "ws/proj/algo/beat.strudel"]),
        patterns: vec!["algo/beat.strudel"],
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    assert!(b.refused.contains(&l.at("ws/proj/algo")));
    assert!(b.refused.contains(&l.at("ws/proj/algo/beat.strudel")));
    // Below a refused folder, a path nobody asked about is refused too.
    let mem = MemSource::from_bundle(b).expect("a source");
    assert_eq!(
        mem.stat(&l.at("ws/proj/algo/other.wav")),
        Err(song::SourceError::Outside)
    );
}

#[test]
fn a_link_out_as_a_familys_entry_and_a_listing_that_names_it_replay_the_same() {
    let l = lab("replay-link-entry");
    l.t.write("outside/x.k7x", "source(\"clips/x.wav\")\n");
    l.t.write("ws/proj/clips/ok.k7x", "source(\"clips/ok.wav\")\n");
    if !l.link("a_link_out_as_a_familys_entry", "ws/proj/clips/x.k7x", "x.k7x") {
        return;
    }
    l.t.age();
    let ask = Ask {
        probes: probes(&l, &["ws/proj/clips", "ws/proj/clips/x.k7x"]),
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    assert!(b.refused.contains(&l.at("ws/proj/clips/x.k7x")), "{:?}", b.refused);
    // The listing still names the link, as the disk listed it.
    let names: Vec<&str> = b.lists[&l.at("ws/proj/clips")]
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(names.contains(&"x.k7x"), "{names:?}");
}

#[test]
fn a_whole_family_folder_that_is_a_link_out_replays_as_outside() {
    let l = lab("replay-link-family");
    l.t.write("outside/clips/x.k7x", "source(\"clips/x.wav\")\n");
    if !l.link("a_whole_family_folder", "ws/proj/clips", "clips") {
        return;
    }
    l.t.age();
    let b = replayed(&l, &Ask::default());
    assert!(b.refused.contains(&l.at("ws/proj/clips")), "{:?}", b.refused);
}

#[test]
fn a_path_below_a_file_replays_as_the_disk_answered_it() {
    let l = lab("replay-enotdir");
    l.t.write("ws/proj/clips/a.wav", "RIFF");
    l.t.age();
    let ask = Ask {
        probes: probes(&l, &["ws/proj/clips/a.wav/sub.wav"]),
        audio: vec![("sub", "clips/a.wav/sub.wav".into())],
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    let path = l.at("ws/proj/clips/a.wav/sub.wav");
    let mem = MemSource::from_bundle(b.clone()).expect("a source");
    // Unix answers Io (not a directory) and keeps its message; Windows answers NotFound, which the bundle names absent (decision 795).
    if cfg!(windows) {
        assert!(b.absent.contains(&path), "{:?}", b.absent);
        assert_eq!(mem.stat(&path), Err(song::SourceError::NotFound));
    } else {
        assert!(b.failed_stats.contains_key(&path), "{:?}", b.failed_stats);
        assert_eq!(
            mem.stat(&path),
            Err(song::SourceError::Io(b.failed_stats[&path].clone()))
        );
    }
}

#[test]
fn a_pattern_that_is_not_utf8_replays_as_the_same_warning() {
    let l = lab("replay-pattern");
    fs::create_dir_all(l.t.0.join("ws/proj/algo")).expect("folder");
    fs::write(l.t.0.join("ws/proj/algo/bad.strudel"), [0xff, 0xfe, 0x00, 0x41]).expect("file");
    l.t.age();
    let ask = Ask {
        patterns: vec!["algo/bad.strudel"],
        probes: probes(&l, &["ws/proj/algo/bad.strudel"]),
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    let f = &b.files[&l.at("ws/proj/algo/bad.strudel")];
    assert_eq!((f.len, f.binary, f.text.as_deref()), (Some(4), Some(true), None));
    // A range of it is Missing, as a bare length is (decision 745).
    let mem = MemSource::from_bundle(b).expect("a source");
    let range = Span::Range { offset: 0, len: 2 };
    assert_eq!(
        mem.read(&l.at("ws/proj/algo/bad.strudel"), range),
        Err(song::SourceError::Missing)
    );
}

// Decision 793: a vocabulary hit reads the script again through the Source it was handed.
#[test]
fn a_widgets_script_that_is_not_utf8_replays_the_same_and_a_vocabulary_hit_keeps_it() {
    let l = lab("replay-widgets");
    l.t.write(
        &format!("{AMP}/module.k7x"),
        "plugin({ type: \"internal\", name: \"Amp\" })\n",
    );
    l.t.write(
        &format!("{AMP}/ui.k7x"),
        "cols(2)\nrows(1)\nwidgets({ gain: { type: \"knob\", col: 0, row: 0, bind: \"gain\" } })\n",
    );
    fs::write(l.t.0.join(AMP).join("widgets.js"), [0xff, 0xfe, 0x41]).expect("write");
    l.t.age();
    let b = replayed(&l, &Ask::default());
    let f = &b.files[&l.at(&format!("{AMP}/widgets.js"))];
    assert_eq!((f.len, f.binary), (Some(3), Some(true)));
    // A second compile off the same Cache hits the vocabulary and must make the same bundle.
    let (dirs, files) = l.roots.mounts().expect("mounts");
    let disk = DiskSource::with_files(&dirs, &files).expect("a source");
    let mut cache = Cache::new();
    let mut bundle = || {
        let rec = Recorder::new(&disk);
        dump_modules(&rec, &mut cache, &l.roots).expect("modules");
        rec.into_bundle_with_files(&dirs, &files)
    };
    let (miss, hit) = (bundle(), bundle());
    assert!(cache.vocabulary_hits() >= 1);
    same_bundle(&miss, &hit);
}

// Decision 793: a decoded file served from the cache is read again through the Source.
#[test]
fn a_family_file_that_is_not_utf8_replays_the_same_through_a_cache_hit_too() {
    let l = lab("replay-family-binary");
    l.t.write("ws/proj/clips/ok.k7x", "source(\"clips/ok.wav\")\n");
    fs::write(l.t.0.join("ws/proj/clips/bad.k7x"), [0x74, 0xff, 0xfe]).expect("write");
    l.t.age();
    let (dirs, files) = l.roots.mounts().expect("mounts");
    let disk = DiskSource::with_files(&dirs, &files).expect("a source");
    let mut cache = Cache::new();
    let mut bundle = || {
        let rec = Recorder::new(&disk);
        song::read_family::<song::Clip>(&rec, &mut cache, &l.project()).expect("reads");
        rec.into_bundle_with_files(&dirs, &files)
    };
    let (miss, hit) = (bundle(), bundle());
    same_bundle(&miss, &hit);
    let f = &miss.files[&l.at("ws/proj/clips/bad.k7x")];
    assert_eq!(f.binary, Some(true));
    let mem = MemSource::from_bundle(hit).expect("a source");
    let read = song::read_family::<song::Clip>(&mem, &mut Cache::new(), &l.project()).expect("replays");
    assert_eq!(read.findings.len(), 1);
}

#[test]
fn an_assembly_template_that_is_not_utf8_replays_the_same() {
    let l = lab("replay-template");
    l.t.write(
        "ws/proj/assemblies/tt.k7x",
        "uses(\"@k7-core/turntable\")\nmembers({ level: \"levelA\" })\n",
    );
    fs::create_dir_all(l.t.0.join("lib/k7-core/assemblies/turntable")).expect("folder");
    let template = "lib/k7-core/assemblies/turntable/assembly.k7x";
    fs::write(l.t.0.join(template), [0xff, 0xfe, 0x41]).expect("write");
    l.t.age();
    let b = replayed(&l, &Ask::default());
    let f = &b.files[&l.at(template)];
    assert_eq!(f.binary, Some(true), "{:?}", b.files.keys().collect::<Vec<_>>());
}

#[cfg(unix)]
fn locked(l: &Lab, rel: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let p = l.t.0.join(rel);
    fs::set_permissions(&p, fs::Permissions::from_mode(0o000)).expect("chmod");
    // Root reads whatever the mode says, which would make the case a lie.
    let still_readable = if p.is_dir() {
        fs::read_dir(&p).is_ok()
    } else {
        fs::File::open(&p).is_ok()
    };
    if still_readable {
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
        return None;
    }
    Some(p)
}

#[cfg(unix)]
fn unlock(p: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(p, fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[cfg(unix)]
#[test]
fn a_family_file_that_will_not_read_replays_as_the_same_io() {
    let l = lab("replay-io-file");
    l.t.write("ws/proj/clips/ok.k7x", "source(\"clips/ok.wav\")\n");
    l.t.write("ws/proj/clips/locked.k7x", "source(\"clips/l.wav\")\n");
    l.t.age();
    let Some(p) = locked(&l, "ws/proj/clips/locked.k7x") else {
        skip("a_family_file_that_will_not_read", "this user can read a mode-000 file");
        return;
    };
    let ask = Ask {
        probes: probes(&l, &["ws/proj/clips/locked.k7x"]),
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    unlock(&p);
    let f = &b.files[&l.at("ws/proj/clips/locked.k7x")];
    assert!(f.io.is_some() && f.len.is_some() && f.text.is_none(), "{f:?}");
}

#[cfg(unix)]
#[test]
fn a_folder_that_will_not_list_replays_as_the_same_io() {
    let l = lab("replay-io-dir");
    l.t.write("ws/proj/sequences/s.k7x", "x(1)\n");
    l.t.write("ws/proj/clips/ok.k7x", "source(\"clips/ok.wav\")\n");
    l.t.age();
    let Some(p) = locked(&l, "ws/proj/sequences") else {
        skip("a_folder_that_will_not_list", "this user can list a mode-000 folder");
        return;
    };
    let ask = Ask {
        probes: probes(&l, &["ws/proj/sequences"]),
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    unlock(&p);
    assert!(
        b.failed_lists.contains_key(&l.at("ws/proj/sequences")),
        "{:?}",
        b.failed_lists
    );
}

#[test]
fn a_listing_that_holds_a_link_out_to_a_folder_replays_the_same_below_it() {
    let l = lab("replay-list-link");
    l.t.write("outside/deep/x.k7x", "x(1)\n");
    l.t.write("ws/proj/nodes/a.k7x", "extends(\"@meta/track\")\n");
    if !l.link("a_listing_that_holds_a_link_out", "ws/proj/nodes/sub", "deep") {
        return;
    }
    l.t.age();
    let ask = Ask {
        probes: probes(&l, &["ws/proj/nodes", "ws/proj/nodes/sub", "ws/proj/nodes/sub/x.k7x"]),
        ..Ask::default()
    };
    let b = replayed(&l, &ask);
    assert!(b.lists[&l.at("ws/proj/nodes")].iter().any(|e| e.name == "sub"));
    assert!(b.refused.contains(&l.at("ws/proj/nodes/sub")));
}
