// scan.rs — the engine's plugin scan read through a Source: the catalogue it makes, its lookups, and its hostile inputs.
// A reader would get wrong what EMPTY means: the scan cannot say, so the built-in table stands in; it is never "none installed".

use crate::support;

use song::modules::{SCAN_CAP, Scan, builtin_scan, engine_modules, io_modules};
use song::{DiskSource, Source, SourceError, Span};
use support::{Tmp, spell};

fn scan_of(t: &Tmp, json: &str) -> Scan {
    t.write("home/exts.json", json);
    let path = t.at("home/exts.json");
    let source = DiskSource::with_files(&[] as &[&str], &[path.as_str()]).expect("a source");
    Scan::read(&source, Some(&path)).expect("a scan")
}

const THREE: &str = r#"{"entries":[{"plugins":[
  {"name":"Serum","format":"VST3","id":"/p/Serum.vst3","manufacturer":"Xfer","role":"instrument","category":"instrument|synth"},
  {"name":"Neural Amp Modeler","format":"VST3","role":"audioEffect"},
  {"name":"Serum","format":"AU","instrument":true},
  {"name":"Arp","format":"VST3","role":"midiEffect"},
  {"name":"","format":"VST3"},
  {"format":"VST3"},
  null
]}]}"#;

// engineCatalogue.test.ts, "reading the scan": sorted, skipping what has no name or format.
#[test]
fn the_scan_is_read_sorted_and_what_is_nameless_is_skipped() {
    let t = Tmp::new("scan-read");
    let s = scan_of(&t, THREE);
    let seen: Vec<(&str, &str)> = s.plugins.iter().map(|p| (p.name.as_str(), p.format.as_str())).collect();
    assert_eq!(
        seen,
        [
            ("Arp", "VST3"),
            ("Neural Amp Modeler", "VST3"),
            ("Serum", "AU"),
            ("Serum", "VST3")
        ]
    );
    let serum = &s.plugins[3];
    assert_eq!(
        (
            serum.id.as_deref(),
            serum.manufacturer.as_deref(),
            serum.role.as_deref()
        ),
        (Some("/p/Serum.vst3"), Some("Xfer"), Some("instrument"))
    );
    assert!(s.plugins[2].instrument && !s.plugins[3].instrument);
}

// engineCatalogue.test.ts: the key sets and their lookups.
#[test]
fn the_scan_answers_what_the_engine_knows() {
    let t = Tmp::new("scan-keys");
    let s = scan_of(&t, THREE);
    assert!(s.is_plugin_scanned("vst3", "SERUM"));
    assert!(!s.is_plugin_scanned("VST3", "Unknown"));
    assert!(s.hosts_all(&[("VST3", "Serum"), ("AU", "serum")]));
    assert!(!s.hosts_all(&[("VST3", "Serum"), ("VST3", "Nope")]));
    assert_eq!(s.names()["vst3"], ["Arp", "Neural Amp Modeler", "Serum"]);
    // Against a scan that cannot say, nothing is judged unknown and nothing is hosted.
    let none = Scan::default();
    assert!(none.is_plugin_scanned("VST3", "Anything"));
    assert!(!none.hosts_all(&[("VST3", "Serum")]));
}

// engineCatalogue.test.ts, "the catalogue of modules": a ref and plugin block per plugin, the role deciding the folder.
#[test]
fn the_scan_makes_a_module_per_plugin() {
    let t = Tmp::new("scan-modules");
    let m = engine_modules(&scan_of(&t, THREE));
    assert_eq!(
        m.keys().collect::<Vec<_>>(),
        ["@engine/arp", "@engine/neural-amp-modeler", "@engine/serum"]
    );
    let serum = &m["@engine/serum"];
    assert_eq!(serum.category.as_deref(), Some("instruments"));
    // The first of two scans of one plugin wins, and the FORMAT is lowercased.
    assert_eq!(
        serum.authored.plugin.as_ref().and_then(|p| p.plugin_type.as_deref()),
        Some("au")
    );
    assert_eq!(m["@engine/arp"].category.as_deref(), Some("transformers"));
    assert_eq!(m["@engine/neural-amp-modeler"].category.as_deref(), Some("audio"));
    assert_eq!(m["@engine/neural-amp-modeler"].inputs()[0].name, "in");
    assert_eq!(serum.file, "serum");
}

// engineCatalogue.test.ts and decision 30: the scan is the built-in table when it cannot say.
#[test]
fn a_scan_that_cannot_say_is_the_builtin_table() {
    let t = Tmp::new("scan-builtin");
    let builtin = engine_modules(&Scan::default());
    assert!(builtin.len() > 100 && builtin.keys().all(|k| k.starts_with("@engine/")));
    assert_eq!(builtin_scan().len(), song::modules::builtin_plugins().len());
    for json in [
        "",
        "not json",
        "{}",
        "[]",
        "null",
        "{\"entries\":[]}",
        "{\"entries\":[{\"plugins\":[]}]}",
        "{\"entries\":5}",
        "{\"entries\":[null]}",
        "{\"entries\":[{\"plugins\":{}}]}",
    ] {
        let s = scan_of(&t, json);
        assert!(s.is_empty(), "{json:?} read as a scan");
        assert_eq!(engine_modules(&s).len(), builtin.len());
    }
    // A scan file that is not there, and no scan at all.
    let source = DiskSource::new(&[t.at("home")]).expect("a source");
    assert!(
        Scan::read(&source, Some(&t.at("home/absent.json")))
            .expect("a scan")
            .is_empty()
    );
    assert!(Scan::read(&source, None).expect("a scan").is_empty());
    // The built-ins' roles file them: an instrument is under instruments/.
    assert_eq!(builtin["@engine/4osc"].category.as_deref(), Some("instruments"));
}

// Hostile input: a scan of 30,000 plugins and one nested past serde_json's limit both read as "cannot say".
#[test]
fn a_hostile_scan_is_refused_quickly() {
    let t = Tmp::new("scan-hostile");
    let started = std::time::Instant::now();
    let many: String = (0..SCAN_CAP + 10_000)
        .map(|i| format!("{{\"name\":\"P{i}\",\"format\":\"VST3\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(scan_of(&t, &format!("{{\"entries\":[{{\"plugins\":[{many}]}}]}}")).is_empty());
    let ok: String = (0..SCAN_CAP)
        .map(|i| format!("{{\"name\":\"P{i}\",\"format\":\"VST3\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        scan_of(&t, &format!("{{\"entries\":[{{\"plugins\":[{ok}]}}]}}"))
            .plugins
            .len(),
        SCAN_CAP
    );
    let nested = format!("{}1{}", "[".repeat(300), "]".repeat(300));
    assert!(
        scan_of(
            &t,
            &format!("{{\"entries\":[{{\"plugins\":[{{\"name\":\"a\",\"format\":\"b\",\"x\":{nested}}}]}}]}}")
        )
        .is_empty()
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

// Decision 29: a scan mount names one FILE and admits that path alone, lexically and by its real path.
#[test]
fn a_file_mount_admits_that_path_alone() {
    let t = Tmp::new("scan-mount");
    t.write("home/exts.json", THREE);
    t.write("home/secret.json", "{}");
    t.write("home/renders/a.wav", "x");
    let path = t.at("home/exts.json");
    let source = DiskSource::with_files(&[] as &[&str], &[path.as_str()]).expect("a source");
    assert!(source.read(&path, Span::All).is_ok());
    assert!(source.stat(&path).is_ok());
    for other in [
        t.at("home/secret.json"),
        t.at("home/renders/a.wav"),
        t.at("home/exts.json/../secret.json"),
        t.at("home"),
    ] {
        assert_eq!(source.stat(&other), Err(SourceError::Outside), "{other}");
        assert_eq!(source.read(&other, Span::All), Err(SourceError::Outside), "{other}");
    }
    assert_eq!(source.list(&t.at("home")), Err(SourceError::Outside));
}

// Decision 29's real-path half: the scan file as a link leads elsewhere, and is refused.
#[cfg(unix)]
#[test]
fn a_file_mount_that_is_a_link_is_refused() {
    let t = Tmp::new("scan-mount-link");
    t.write("home/secret.json", "{}");
    std::os::unix::fs::symlink(t.0.join("home/secret.json"), t.0.join("home/exts.json")).expect("a link");
    let path = t.at("home/exts.json");
    let source = DiskSource::with_files(&[] as &[&str], &[path.as_str()]).expect("a source");
    assert_eq!(source.read(&path, Span::All), Err(SourceError::Outside));
    // The scan read through it cannot say, and the compile goes on with the built-ins.
    assert!(Scan::read(&source, Some(&path)).expect("a scan").is_empty());
}

// Decision 29 on Windows: the scan's file mount with a drive letter, spelled as the Source spells it.
#[cfg(windows)]
#[test]
fn a_file_mount_with_a_drive_letter_is_admitted() {
    let t = Tmp::new("scan-mount-win");
    t.write("home/exts.json", THREE);
    let spelled = spell(&t.0.join("home/exts.json"));
    let lower = format!("{}{}", spelled[..1].to_lowercase(), &spelled[1..]);
    let source = DiskSource::with_files(&[] as &[&str], &[lower.as_str()]).expect("a source");
    assert!(source.read(&spelled, Span::All).is_ok());
}

// The io roles are a catalogue of their own: no scan behind them.
#[test]
fn the_io_roles_need_no_machine() {
    let io = io_modules();
    assert_eq!(io.len(), 6);
    assert_eq!(serde_json::to_value(&io["@meta/MIDI"]).expect("json")["io"], "midi");
    let _ = spell(std::path::Path::new("/"));
}

// Decision 45: a scan file past 32 MiB reads as "cannot say", without being read.
#[test]
fn a_scan_file_past_the_size_cap_is_not_read() {
    let t = Tmp::new("scan-size");
    t.write("home/exts.json", THREE);
    let path = t.at("home/exts.json");
    let f = std::fs::File::options().write(true).open(&path).expect("open");
    f.set_len(33 * 1024 * 1024).expect("a sparse file");
    drop(f);
    let source = DiskSource::with_files(&[] as &[&str], &[path.as_str()]).expect("a source");
    assert!(Scan::read(&source, Some(&path)).expect("a scan").is_empty());
}

// A file mount that is a folder admits nothing: the path would list, not read.
#[test]
fn a_file_mount_that_is_a_folder_is_refused() {
    let t = Tmp::new("scan-mount-dir");
    t.mkdir("home/exts.json");
    t.write("home/exts.json/inner.json", "{}");
    let path = t.at("home/exts.json");
    let source = DiskSource::with_files(&[] as &[&str], &[path.as_str()]).expect("a source");
    assert_eq!(source.stat(&path), Err(SourceError::Outside));
    assert_eq!(source.list(&path), Err(SourceError::Outside));
    assert!(Scan::read(&source, Some(&path)).expect("a scan").is_empty());
}
